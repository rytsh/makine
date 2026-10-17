//! The 3D world view: draws the block world and lets the player build in
//! it.
//!
//! The world has no edges, in height either. The player edits one layer at
//! a time; the camera orbits around a point on it. Layers above the edited
//! one are hidden (or shown as glass), layers below are visible, so the
//! player sees what they build on.
//!
//! Controls: left click places the selected block (drag to paint) or, on a
//! device, selects it. Clicking a face of a block builds against that face,
//! on top of it or beside it (see `aim.rs`); building on top climbs to the
//! next layer; Ctrl+click (Cmd on a Mac) builds under it. The camera can
//! also go underground, where clicking a block's bottom builds under it.
//! Right click removes the block under the pointer; R turns the block being placed
//! (or the blocks being pasted); Page Up/Down or Q/E change layer;
//! middle-drag or Alt+drag turns the camera, Space+drag or Shift+middle-drag
//! pans; the wheel zooms. With the select tool, dragging selects a region
//! (Ctrl+C/X/V, arrows move it, Del deletes).

use egui::{Color32, Rect, Sense, Ui};
use makine_sim::Library;
use makine_world::block::{Block, Dir};
use makine_world::device::{self, Device};

use makine_world::region::{self, Clip, Region};
use makine_world::{Change, Edit, Pos, World};

use crate::worldsim::WorldSim;

mod aim;
mod camera;
pub mod gpu;
mod panels;
mod render;
#[cfg(test)]
mod tests;

pub use camera::Camera;
pub use panels::palette;

/// The block the player is placing.
#[derive(Clone, Debug, PartialEq)]
pub struct Brush {
    pub block: Block,
}

pub struct WorldView {
    pub world: World,
    /// Layer being edited.
    pub layer: i32,
    pub brush: Option<Brush>,
    /// Facing applied to placed blocks; R rotates it.
    pub facing: Dir,
    pub camera: Camera,
    /// Show the layers above the edited one, as glass.
    pub show_above: bool,
    /// Where the view was drawn last frame.
    rect: Rect,
    /// This player's changes, newest last. Undo only takes back cells
    /// nobody else has changed since.
    undo: Vec<Change>,
    redo: Vec<Change>,
    /// The running circuit of the world.
    pub sim: WorldSim,
    /// Rebuilding the circuit waits for the end of a drag (large worlds).
    defer: bool,
    /// Cell under the pointer last frame: where a block would go.
    hover: Option<Pos>,
    /// The block under the pointer last frame, when building.
    target: Option<Pos>,
    /// The last cell painted in the current drag, so dragging does not
    /// repaint the same cell every frame.
    painted: Option<Pos>,
    /// Centre the camera on the world at the next frame.
    fit: bool,
    /// Anchor of the selected device.
    pub selected: Option<Pos>,
    /// A selected clock or port, for its settings.
    pub selected_block: Option<Pos>,
    /// A message for the player (e.g. why a device does not fit).
    pub status: Option<String>,
    /// ROM contents being edited: device anchor, text in the box, the
    /// contents the text was made from, parse error.
    rom_text: Option<(Pos, String, Option<String>, Option<String>)>,
    /// Selected region, and the corner where a selection drag started.
    pub selection: Option<Region>,
    sel_start: Option<Pos>,
    /// Blocks being pasted: they follow the pointer until clicked down.
    pub pasting: Option<Clip>,
    /// The last copied blocks.
    pub clipboard: Option<Clip>,
    /// Name typed for a new chip made from the selection.
    pub chip_name: String,
    /// Something the app must do (it owns the chip library).
    pub request: Option<Request>,
    /// Edits made here since the session last took them, while
    /// `sharing`.
    pub outbox: Vec<Edit>,
    /// Whether this world is shared (edits are collected in `outbox`).
    pub sharing: bool,
    /// Other players' cursors: name, colour, cell.
    pub others: Vec<(String, Color32, Pos)>,
}

/// Requests from the world view to the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Turn the blocks in a region into a free chip named `name`.
    MakeChip { name: String, region: Region },
}

impl Default for WorldView {
    fn default() -> Self {
        Self::new(World::new("dünya"))
    }
}

impl WorldView {
    pub fn new(world: World) -> Self {
        Self {
            world,
            layer: 0,
            brush: Some(Brush { block: Block::Wire }),
            facing: Dir::East,
            camera: Camera::default(),
            show_above: false,
            rect: Rect::NOTHING,
            undo: Vec::new(),
            redo: Vec::new(),
            sim: WorldSim::default(),
            defer: false,
            hover: None,
            target: None,
            painted: None,
            fit: true,
            selected: None,
            selected_block: None,
            status: None,
            rom_text: None,
            selection: None,
            sel_start: None,
            pasting: None,
            clipboard: None,
            chip_name: String::new(),
            request: None,
            outbox: Vec::new(),
            sharing: false,
            others: Vec::new(),
        }
    }

    /// Apply another player's edit. It is not undoable here (undo only
    /// takes back this player's own edits).
    pub fn apply_remote(&mut self, e: Edit) {
        self.world.apply(e);
        self.after_remote_edits();
    }

    /// Tidy up after other players changed the world.
    pub fn after_remote_edits(&mut self) {
        // A device deleted by someone else cannot stay selected.
        if self
            .selected
            .is_some_and(|a| self.world.device_at(a).is_none())
        {
            self.selected = None;
        }
    }

    /// The cell under the pointer, if any.
    pub fn hover(&self) -> Option<Pos> {
        self.hover
    }

    /// Set the player's chip library (used by chip blocks).
    pub fn set_library(&mut self, lib: Library) {
        self.sim.set_library(lib);
    }

    /// Whether the select tool is active (no brush).
    pub fn selecting(&self) -> bool {
        self.brush.is_none()
    }

    /// Copy the selection; also returns it as text for the system clipboard.
    pub fn copy(&mut self) -> Option<String> {
        let clip = Clip::copy(&self.world, self.selection?)?;
        let text = clip.to_json();
        self.clipboard = Some(clip);
        Some(text)
    }

    pub fn cut(&mut self) -> Option<String> {
        let text = self.copy()?;
        self.delete_selection();
        Some(text)
    }

    pub fn delete_selection(&mut self) {
        if let Some(r) = self.selection {
            let e = r.clear(&self.world);
            if matches!(&e, Edit::Batch(v) if !v.is_empty()) {
                if self.selected.is_some_and(|a| r.contains(a)) {
                    self.selected = None;
                }
                self.edit(e);
            }
        }
    }

    /// Start pasting `text` if it holds blocks, else the last copy.
    pub fn start_paste(&mut self, text: Option<&str>) {
        let clip = text
            .and_then(Clip::from_json)
            .or_else(|| self.clipboard.clone());
        if let Some(c) = clip {
            self.pasting = Some(c);
            self.status =
                Some("Yapıştırmak için tıkla · R: döndür · Esc ya da sağ tık: bırak".into());
        }
    }

    /// Paste the floating clip with its corner at `at`.
    fn paste_at(&mut self, at: Pos) {
        let Some(clip) = &self.pasting else { return };
        match clip.paste(&self.world, at) {
            Ok(e) => {
                let (w, h) = clip.size();
                self.edit(e);
                self.selection = Some(Region::between(
                    at,
                    Pos::new(at.x + w - 1, at.y + h - 1, at.z),
                ));
                self.pasting = None;
                self.status = None;
            }
            Err(e) => self.status = Some(e),
        }
    }

    /// Move the selection and its blocks by (dx, dy).
    pub fn move_selection(&mut self, dx: i32, dy: i32) {
        let Some(r) = self.selection else { return };
        match region::move_region(&self.world, r, dx, dy) {
            Ok(e) => {
                self.edit(e);
                self.selection = Some(Region {
                    x0: r.x0 + dx,
                    y0: r.y0 + dy,
                    x1: r.x1 + dx,
                    y1: r.y1 + dy,
                });
                if let Some(a) = self.selected.filter(|a| r.contains(*a)) {
                    self.selected = Some(Pos::new(a.x + dx, a.y + dy, a.z));
                }
            }
            Err(e) => self.status = Some(e),
        }
    }

    /// A world containing only the blocks of `r`.
    pub fn region_world(&self, r: Region, name: &str) -> World {
        let mut w = World::new(name);
        if let Some(clip) = Clip::copy(&self.world, r) {
            if let Ok(e) = clip.paste(&w, Pos::new(0, 0, 0)) {
                w.apply(e);
            }
        }
        w
    }

    /// The selected device: anchor, device and facing.
    pub fn selected_device(&self) -> Option<(Pos, Device, Dir)> {
        let (anchor, d, f) = self.world.device_at(self.selected?)?;
        Some((anchor, d.clone(), f))
    }

    /// Replace the device at `anchor` with `device` (same facing), if it
    /// fits where it is.
    pub fn replace_device(&mut self, anchor: Pos, device: Device) -> Result<(), String> {
        let Some((_, _, facing)) = self.world.device_at(anchor) else {
            return Err("cihaz yok".into());
        };
        let remove = self.world.remove(anchor).ok_or("cihaz yok")?;
        let mut trial = self.world.clone();
        trial.apply(remove.clone());
        let place = trial
            .place_device(device, anchor, facing)
            .ok_or("yeni boyutu sığmıyor: önündeki ya da yanındaki blokları kaldır")?;
        self.edit(Edit::Batch(vec![remove, place]));
        Ok(())
    }

    /// Contents of a RAM device, if it is running.
    pub fn ram_words(&mut self, anchor: Pos) -> Option<Vec<u64>> {
        let name = self.sim.native_at(anchor)?;
        device::ram(self.sim.running_circuit()?, &name)
    }

    /// Set a screen's pixels directly (for screenshots).
    #[cfg(test)]
    pub fn set_screen_for_test(&mut self, anchor: Pos, f: impl Fn(usize, usize) -> bool) {
        let name = self.sim.native_at(anchor).unwrap();
        let sim = self.sim.running_circuit().unwrap();
        let s: &mut makine_sim::Clocked<device::ScreenState> = sim.native_mut(&name).unwrap();
        let n = 1usize << s.state.bits;
        for y in 0..n {
            for x in 0..n {
                s.state.pixels[y * n + x] = f(x, y);
            }
        }
    }

    /// Whether typed text goes to a keyboard device.
    pub fn typing(&self) -> bool {
        matches!(self.selected_device(), Some((_, Device::Keyboard, _)))
    }

    /// Apply an edit made by the player (undoable).
    pub fn edit(&mut self, e: Edit) {
        if self.sharing {
            self.outbox.push(e.clone());
        }
        let change = self.world.apply_change(e);
        if change.is_empty() {
            return;
        }
        self.undo.push(change);
        self.redo.clear();
        if self.undo.len() > 500 {
            self.undo.remove(0);
        }
    }

    /// Take back this player's last change, skipping changes that other
    /// players have entirely overwritten.
    pub fn undo(&mut self) {
        self.step_history(true);
    }

    pub fn redo(&mut self) {
        self.step_history(false);
    }

    fn step_history(&mut self, back: bool) {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        while let Some(change) = from.pop() {
            let Some(e) = self.world.revert(&change, back) else {
                continue;
            };
            if self.sharing {
                self.outbox.push(e.clone());
            }
            self.world.apply(e);
            to.push(change);
            return;
        }
    }

    /// Whether undo would change anything (others may have overwritten
    /// all of this player's changes).
    pub fn can_undo(&self) -> bool {
        self.undo
            .iter()
            .any(|c| self.world.revert(c, true).is_some())
    }

    pub fn can_redo(&self) -> bool {
        self.redo
            .iter()
            .any(|c| self.world.revert(c, false).is_some())
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        self.rect = rect;
        if std::mem::take(&mut self.fit) {
            self.fit_camera();
        }
        let (space, camera_drag) = self.camera_input(ui, &response);

        // Keys. While a keyboard device is selected, typing goes to it.
        if self.typing() {
            let keys: Vec<u8> = ui.input(|i| {
                let mut out = Vec::new();
                for e in &i.events {
                    match e {
                        egui::Event::Text(t) => {
                            out.extend(t.chars().filter(char::is_ascii).map(|c| c as u8))
                        }
                        egui::Event::Key {
                            key, pressed: true, ..
                        } => match key {
                            egui::Key::Enter => out.push(b'\n'),
                            egui::Key::Backspace => out.push(8),
                            egui::Key::Escape => out.push(27),
                            _ => {}
                        },
                        _ => {}
                    }
                }
                out
            });
            if keys.contains(&27) {
                self.selected = None;
            } else {
                if let Some(anchor) = self.selected {
                    for k in keys {
                        self.sim.press(anchor, k);
                    }
                }
            }
        } else if response.hovered() || response.has_focus() {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.pasting.take().is_some() {
                    self.status = None;
                } else if self.selected.take().is_none() && self.selected_block.take().is_none() {
                    self.selection = None;
                }
            }
            // Clipboard: egui turns Ctrl+C/X/V into these events.
            let events = ui.input(|i| i.events.clone());
            for e in events {
                match e {
                    egui::Event::Copy => {
                        if let Some(t) = self.copy() {
                            ui.ctx().copy_text(t);
                        }
                    }
                    egui::Event::Cut => {
                        if let Some(t) = self.cut() {
                            ui.ctx().copy_text(t);
                        }
                    }
                    egui::Event::Paste(t) => self.start_paste(Some(&t)),
                    _ => {}
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
                self.delete_selection();
            }
            if self.selection.is_some() && self.pasting.is_none() {
                let step = ui.input(|i| {
                    let mut d = (0, 0);
                    if i.key_pressed(egui::Key::ArrowRight) {
                        d.0 += 1
                    }
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        d.0 -= 1
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        d.1 += 1
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        d.1 -= 1
                    }
                    d
                });
                if step != (0, 0) {
                    self.move_selection(step.0, step.1);
                }
            }
            let (rotate, back) = ui.input(|i| (i.key_pressed(egui::Key::R), i.modifiers.shift));
            if rotate {
                match &mut self.pasting {
                    // Three turns clockwise are one counter-clockwise.
                    Some(c) => {
                        for _ in 0..if back { 3 } else { 1 } {
                            *c = c.rotated();
                        }
                    }
                    None if back => self.facing = self.facing.ccw(),
                    None => self.facing = self.facing.cw(),
                }
            }
            let layer = ui.input(|i| {
                let up = i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::E);
                let down = i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::Q);
                up as i32 - down as i32
            });
            if layer != 0 {
                self.set_layer(self.layer + layer);
            }
            let (undo, redo) = ui.input(|i| {
                let cmd = i.modifiers.command;
                (
                    cmd && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                    cmd && (i.key_pressed(egui::Key::Y)
                        || (i.modifiers.shift && i.key_pressed(egui::Key::Z))),
                )
            });
            if undo {
                self.undo();
            }
            if redo {
                self.redo();
            }
        }

        // Pointer: which cell? Selecting and pasting work on the edited
        // layer; building aims at the blocks themselves.
        let shift = ui.input(|i| i.modifiers.shift);
        let building = self.pasting.is_none() && !self.selecting() && !shift;
        let pick = |v: &Self, p: egui::Pos2| v.camera.pick(rect, p, v.layer);
        let aim_at = |v: &Self, p: egui::Pos2| aim::aim(&v.world, &v.camera.ray(rect, p), v.layer);
        let aimed = response.hover_pos().and_then(|p| aim_at(self, p));
        if building {
            self.hover = aimed.map(|a| a.place);
            self.target = aimed.and_then(|a| a.target);
        } else {
            self.hover = response.hover_pos().and_then(|p| pick(self, p));
            self.target = None;
        }
        // Buttons used to move the camera do not build.
        let space = space || camera_drag;

        // Pasting: a click puts the blocks down; right click stops.
        if self.pasting.is_some() && !space {
            if let (true, Some(cell)) = (response.clicked(), self.hover) {
                self.paste_at(cell);
            }
            if response.secondary_clicked() {
                self.pasting = None;
                self.status = None;
            }
        } else if (self.selecting() || shift) && !space {
            // Selecting: drag a rectangle; a click selects one cell (and the
            // device there).
            if response.drag_started_by(egui::PointerButton::Primary) {
                let press = ui.input(|i| i.pointer.press_origin());
                self.sel_start = press.and_then(|p| pick(self, p));
            }
            if let (Some(a), Some(b)) = (self.sel_start, self.hover) {
                if response.dragged_by(egui::PointerButton::Primary) {
                    self.selection = Some(Region::between(a, b));
                }
            }
            if response.drag_stopped() {
                self.sel_start = None;
            }
            if let (true, Some(cell)) = (response.clicked(), self.hover) {
                self.selection = Some(Region::between(cell, cell));
                self.selected = self.world.device_at(cell).map(|d| d.0);
            }
            if response.secondary_clicked() {
                self.selection = None;
            }
        } else if !space {
            self.build_input(ui, &response, aimed, aim_at, pick);
        }

        // Large worlds recompile only when the pointer is released, so
        // painting stays smooth.
        let held = ui.input(|i| i.pointer.any_down());
        self.defer = held && self.sim.slow();

        // Simulate and draw.
        self.sim.run_frame(&self.world, self.defer);
        let screens = self.screens();
        self.draw(&painter, rect, &screens);
        if self.sim.running
            && self
                .world
                .blocks()
                .any(|(_, b)| matches!(b, Block::Clock { .. }))
            || self.typing()
            || self.defer
        {
            ui.ctx().request_repaint();
        }
    }

    /// Edit layer `z`; the camera rises or sinks with it.
    pub fn set_layer(&mut self, z: i32) {
        let lim = makine_world::world::MAX_COORD;
        let z = z.clamp(-lim, lim);
        self.camera.target.z += (z - self.layer) as f64;
        self.layer = z;
    }

    /// Wheel zooms towards the pointer; middle-drag (or Alt+drag) turns the
    /// camera, Space+drag (or Shift+middle-drag) pans. Returns whether Space
    /// is held and whether the camera is being dragged.
    fn camera_input(&mut self, ui: &Ui, response: &egui::Response) -> (bool, bool) {
        let rect = response.rect;
        if let Some(hp) = response.hover_pos() {
            let (scroll, zd) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = (zd * (scroll * 0.0015).exp()) as f64;
            if f != 1.0 {
                // Keep the ground point under the pointer where it is.
                let h = self.layer as f64;
                let before = self.camera.ray(rect, hp).at_height(h);
                self.camera.zoom(f);
                let after = self.camera.ray(rect, hp).at_height(h);
                if let (Some(a), Some(b)) = (before, after) {
                    self.camera.target += a - b;
                }
            }
        }
        let (space, shift, alt) = ui.input(|i| {
            (
                i.key_down(egui::Key::Space),
                i.modifiers.shift,
                i.modifiers.alt,
            )
        });
        let middle = response.dragged_by(egui::PointerButton::Middle);
        let primary = response.dragged_by(egui::PointerButton::Primary);
        let pan = (middle && shift) || (space && primary);
        let orbit = (middle && !shift) || (alt && primary && !space);
        if let (true, Some(p)) = (pan, response.interact_pointer_pos()) {
            self.camera.pan(rect, p - response.drag_delta(), p);
        } else if orbit {
            let d = response.drag_delta();
            self.camera.orbit(d.x, d.y);
        }
        (space, alt && primary)
    }

    /// Building with the pointer. A click builds against the face under the
    /// pointer (or uses a switch, clock, port or device there); a drag
    /// paints on the edited layer only, so it never stacks up towers. Right
    /// click and right drag remove what is under the pointer.
    fn build_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        aimed: Option<aim::Aim>,
        aim_at: impl Fn(&Self, egui::Pos2) -> Option<aim::Aim>,
        floor: impl Fn(&Self, egui::Pos2) -> Option<Pos>,
    ) {
        use egui::PointerButton::{Primary, Secondary};
        // A drag is only recognized once the pointer has moved; act on the
        // cell where the press started too.
        // Cells to paint (on the edited layer's floor) and blocks to remove.
        let (mut paint, mut remove) = (Vec::new(), Vec::new());
        let pointer = response.interact_pointer_pos();
        if response.drag_started() {
            self.painted = None;
            let press = ui.input(|i| i.pointer.press_origin());
            if let Some(p) = press {
                if response.dragged_by(Primary) {
                    paint.extend(floor(self, p));
                } else if response.dragged_by(Secondary) {
                    remove.extend(aim_at(self, p).and_then(|a| a.target));
                }
            }
        }
        if response.clicked() {
            self.painted = None;
            let below = ui.input(|i| i.modifiers.command);
            if let Some(a) = aimed {
                self.click(a, below);
            }
            return;
        }
        if response.secondary_clicked() {
            self.painted = None;
            remove.extend(aimed.and_then(|a| a.target));
        } else if response.dragged_by(Primary) {
            paint.extend(pointer.and_then(|p| floor(self, p)));
        } else if response.dragged_by(Secondary) {
            remove.extend(aimed.and_then(|a| a.target));
        }
        for cell in paint {
            if self.painted != Some(cell) {
                self.painted = Some(cell);
                self.place(cell, false);
            }
        }
        for t in remove {
            if self.painted != Some(t) {
                self.painted = Some(t);
                self.erase(t);
            }
        }
    }

    /// A left click at what the pointer aims at; `below` builds under the
    /// block (or under the empty cell) instead.
    fn click(&mut self, a: aim::Aim, below: bool) {
        if below {
            let over = a.target.unwrap_or(a.place);
            self.place(Pos::new(over.x, over.y, over.z - 1), true);
            return;
        }
        if let Some(t) = a
            .target
            .filter(|t| self.world.get(*t).is_some_and(aim::interactive))
        {
            self.place(t, true);
            return;
        }
        let before = self.world.revision();
        self.place(a.place, true);
        // Building on top climbs to that layer, so the new block shows.
        if a.place.z > self.layer && self.world.revision() != before {
            self.set_layer(a.place.z);
        }
    }

    /// Remove the block at `cell` (a whole device if it is part of one).
    fn erase(&mut self, cell: Pos) {
        if let Some(e) = self.world.remove(cell) {
            if self.world.device_at(cell).map(|d| d.0) == self.selected {
                self.selected = None;
            }
            self.edit(e);
        }
    }

    /// Place the brush at `cell` (`click`: a single click, not a drag).
    /// Clicking a device selects it, clicking a clock or a port selects it
    /// for its settings, and clicking a switch toggles it unless the brush
    /// is a switch facing another way (then it is replaced).
    fn place(&mut self, cell: Pos, click: bool) {
        if click
            && matches!(
                self.world.get(cell),
                Some(Block::Clock { .. } | Block::Port { .. })
            )
        {
            self.selected_block = Some(cell);
            return;
        }
        // Devices are selected by clicking them, never painted over.
        if let Some((anchor, _, _)) = self.world.device_at(cell) {
            if click {
                self.selected = Some(anchor);
            }
            return;
        }
        if let Some(Brush {
            block: Block::Device { device, .. },
        }) = &self.brush
        {
            if !click {
                return;
            }
            match self.world.place_device(device.clone(), cell, self.facing) {
                Some(e) => {
                    self.edit(e);
                    self.selected = Some(cell);
                    self.status = None;
                }
                None => {
                    self.status = Some(format!("{} buraya sığmıyor", device.name()));
                }
            }
            return;
        }
        if let (true, Some(Block::Switch { on, facing })) = (click, self.world.get(cell).cloned()) {
            if !matches!(
                self.brush.as_ref().map(|b| &b.block),
                Some(Block::Switch { .. })
            ) || self.world.get(cell).and_then(|b| b.facing()) == Some(self.facing)
            {
                self.edit(Edit::Set {
                    pos: cell,
                    block: Some(Block::Switch { on: !on, facing }),
                });
                return;
            }
        }
        let Some(brush) = &self.brush else { return };
        let mut block = brush.block.clone();
        while block.facing().is_some_and(|f| f != self.facing) {
            block = block.rotated();
        }
        if self.world.get(cell) != Some(&block) {
            self.edit(Edit::Set {
                pos: cell,
                block: Some(block),
            });
        }
    }
}
