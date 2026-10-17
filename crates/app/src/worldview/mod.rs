//! The 3D world view: draws the block world and lets the player fly
//! through it and build, like Minecraft's creative mode.
//!
//! Clicking the view takes the mouse: moving it looks around, and the
//! crosshair in the middle aims. WASD or the arrow keys fly, Space rises,
//! Shift sinks, Ctrl goes faster. Left click breaks the block aimed at,
//! right click builds against the face aimed at (or uses a switch, clock,
//! port or device; Shift+right click builds on it anyway), middle click
//! takes the block aimed at in hand. Holding a button repeats it. 1–9 and
//! the wheel choose the block in hand; placed blocks face the way the
//! player looks. Esc gives the mouse back, for the side panels.
//!
//! With the select tool, left click marks one corner of a region and right
//! click the other (Ctrl+C/X/V, Alt+arrows move it, Del deletes). While
//! pasting, right click puts the blocks down and R turns them.
//!
//! The world has no edges. The floor layer is an invisible ground (with a
//! grid) to build on in empty space; Page Up/Down move it.

use egui::{Color32, Key, PointerButton, Rect, Sense, Ui, Vec2, ViewportCommand};
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
    /// The floor: an invisible ground at this layer, to build on in empty
    /// space.
    pub layer: i32,
    pub brush: Option<Brush>,
    /// Facing applied to placed blocks: the way the player looks.
    pub facing: Dir,
    pub camera: Camera,
    /// Whether the view has the mouse (looking around, crosshair aiming).
    pub locked: bool,
    /// Raw mouse movement has been seen (else pointer movement is used).
    raw_mouse: bool,
    /// The cursor was just moved back to the middle: ignore its jump.
    warped: bool,
    /// A held mouse button and when it acts again.
    repeat: Option<(PointerButton, f64)>,
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
    /// Selected region, and its first corner.
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
            locked: false,
            raw_mouse: false,
            warped: false,
            repeat: None,
            rect: Rect::NOTHING,
            undo: Vec::new(),
            redo: Vec::new(),
            sim: WorldSim::default(),
            defer: false,
            hover: None,
            target: None,
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
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
        let rect = response.rect;
        self.rect = rect;
        if std::mem::take(&mut self.fit) {
            self.fit_camera();
        }
        let ctx = ui.ctx().clone();
        // A click takes the mouse (and does nothing else); leaving the
        // window gives it back.
        if !self.locked && response.clicked() {
            self.lock(&ctx);
        } else if self.locked && !ui.input(|i| i.focused) {
            self.release(&ctx);
        }
        let keyboard = !ctx.egui_wants_keyboard_input();
        if self.typing() {
            self.type_keys(ui);
        } else if keyboard && (self.locked || response.hovered()) {
            self.keys(ui);
        }
        if self.locked {
            self.look(ui);
            if keyboard && !self.typing() && self.fly(ui) {
                ui.ctx().request_repaint();
            }
            self.keep_pointer(ui, rect);
            self.facing = self.camera.facing();
        }

        // What the crosshair aims at.
        let aimed = self
            .locked
            .then(|| {
                aim::aim(
                    &self.world,
                    &self.camera.ray(rect, rect.center()),
                    self.layer,
                )
            })
            .flatten();
        self.hover = aimed.map(|a| a.place);
        self.target = aimed.and_then(|a| a.target);
        if self.locked {
            self.buttons(ui, aimed);
        }

        // Large worlds recompile only when no button is held, so building
        // stays smooth.
        let held = ui.input(|i| i.pointer.any_down());
        self.defer = held && self.sim.slow();

        // Simulate and draw.
        self.sim.run_frame(&self.world, self.defer);
        let screens = self.screens();
        self.draw(&painter, rect, &screens);
        self.draw_hud(&painter, rect);
        if self.repeat.is_some()
            || self.sim.running
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

    /// Take the mouse: hide it and keep it in the window.
    fn lock(&mut self, ctx: &egui::Context) {
        self.locked = true;
        // Locked where supported (Wayland, macOS), else confined (X11,
        // Windows), where the pointer is also put back in the middle.
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(egui::CursorGrab::Confined));
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(egui::CursorGrab::Locked));
        ctx.send_viewport_cmd(ViewportCommand::CursorVisible(false));
    }

    /// Give the mouse back (Esc, leaving the window or the world view).
    pub fn release(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.locked) {
            ctx.send_viewport_cmd(ViewportCommand::CursorGrab(egui::CursorGrab::None));
            ctx.send_viewport_cmd(ViewportCommand::CursorVisible(true));
        }
        self.repeat = None;
    }

    /// Where the pointer can still move, keep it in the middle of the view,
    /// so clicks land here and it never reaches an edge.
    fn keep_pointer(&mut self, ui: &Ui, rect: Rect) {
        let p = ui.input(|i| i.pointer.latest_pos());
        if p.is_some_and(|p| (p - rect.center()).length() > rect.width().min(rect.height()) / 4.0) {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::CursorPosition(rect.center()));
            self.warped = true;
        }
    }

    /// The mouse turns the eye.
    fn look(&mut self, ui: &Ui) {
        let (raw, moved) = ui.input(|i| {
            let raw = i.events.iter().fold(Vec2::ZERO, |sum, e| match e {
                egui::Event::MouseMoved(d) => sum + *d,
                _ => sum,
            });
            (raw, i.pointer.delta())
        });
        if raw != Vec2::ZERO {
            self.raw_mouse = true;
        }
        let d = if self.raw_mouse {
            raw
        } else if std::mem::take(&mut self.warped) {
            Vec2::ZERO
        } else {
            moved
        };
        self.camera.turn(d.x, d.y);
    }

    /// WASD or the arrows fly along the ground, Space and Shift up and
    /// down, Ctrl faster. (Alt+arrows move the selection instead.) Returns
    /// whether a key is held.
    fn fly(&mut self, ui: &Ui) -> bool {
        let (f, r, u, fast, dt) = ui.input(|i| {
            let k = |a: Key, b: Key| (i.key_down(a) || i.key_down(b)) as i32 as f64;
            let arrows = !i.modifiers.alt;
            let a = |key: Key| (arrows && i.key_down(key)) as i32 as f64;
            let f =
                k(Key::W, Key::W).max(a(Key::ArrowUp)) - k(Key::S, Key::S).max(a(Key::ArrowDown));
            let r = k(Key::D, Key::D).max(a(Key::ArrowRight))
                - k(Key::A, Key::A).max(a(Key::ArrowLeft));
            let u = k(Key::Space, Key::Space) - i.modifiers.shift as i32 as f64;
            (f, r, u, i.modifiers.ctrl, i.stable_dt.min(0.1) as f64)
        });
        let speed = camera::FLY_SPEED * if fast { 3.0 } else { 1.0 };
        self.camera.fly(f, r, u, speed * dt);
        f != 0.0 || r != 0.0 || u != 0.0
    }

    /// Keys typed while a keyboard device is selected go to it; Esc leaves.
    fn type_keys(&mut self, ui: &Ui) {
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
                        Key::Enter => out.push(b'\n'),
                        Key::Backspace => out.push(8),
                        Key::Escape => out.push(27),
                        _ => {}
                    },
                    _ => {}
                }
            }
            out
        });
        if keys.contains(&27) {
            self.selected = None;
        } else if let Some(anchor) = self.selected {
            for k in keys {
                self.sim.press(anchor, k);
            }
        }
    }

    /// Keyboard commands: Esc, the clipboard, Delete, moving the selection,
    /// turning a paste, the floor, undo, the block in hand.
    fn keys(&mut self, ui: &Ui) {
        let ctx = ui.ctx().clone();
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            if self.pasting.take().is_some() {
                self.status = None;
            } else if self.locked {
                self.release(&ctx);
            } else if self.selected.take().is_none() && self.selected_block.take().is_none() {
                self.selection = None;
            }
        }
        // Clipboard: egui turns Ctrl+C/X/V into these events.
        let events = ui.input(|i| i.events.clone());
        for e in &events {
            match e {
                egui::Event::Copy => {
                    if let Some(t) = self.copy() {
                        ctx.copy_text(t);
                    }
                }
                egui::Event::Cut => {
                    if let Some(t) = self.cut() {
                        ctx.copy_text(t);
                    }
                }
                egui::Event::Paste(t) => self.start_paste(Some(t)),
                _ => {}
            }
        }
        if ui.input(|i| i.key_pressed(Key::Delete)) {
            self.delete_selection();
        }
        if self.selection.is_some() && self.pasting.is_none() {
            let step = ui.input(|i| {
                if !i.modifiers.alt {
                    return (0, 0);
                }
                let k = |key| i.key_pressed(key) as i32;
                (
                    k(Key::ArrowRight) - k(Key::ArrowLeft),
                    k(Key::ArrowDown) - k(Key::ArrowUp),
                )
            });
            if step != (0, 0) {
                self.move_selection(step.0, step.1);
            }
        }
        let (rotate, back) = ui.input(|i| (i.key_pressed(Key::R), i.modifiers.shift));
        if let (true, Some(c)) = (rotate, &mut self.pasting) {
            // Three turns clockwise are one counter-clockwise.
            for _ in 0..if back { 3 } else { 1 } {
                *c = c.rotated();
            }
        }
        let floor =
            ui.input(|i| i.key_pressed(Key::PageUp) as i32 - i.key_pressed(Key::PageDown) as i32);
        if floor != 0 {
            self.set_layer(self.layer + floor);
        }
        let (undo, redo) = ui.input(|i| {
            let cmd = i.modifiers.command;
            (
                cmd && !i.modifiers.shift && i.key_pressed(Key::Z),
                cmd && (i.key_pressed(Key::Y) || (i.modifiers.shift && i.key_pressed(Key::Z))),
            )
        });
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        self.hotbar_keys(ui);
    }

    /// 1–9 take a block of the palette in hand; the wheel (while flying)
    /// goes through all of them.
    fn hotbar_keys(&mut self, ui: &Ui) {
        const DIGITS: [Key; 9] = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        let blocks: Vec<Block> = palette().into_iter().map(|(_, b, _)| b).collect();
        let (digit, wheel) = ui.input(|i| {
            let digit = DIGITS.iter().position(|k| i.key_pressed(*k));
            let wheel = i.events.iter().fold(0.0, |sum, e| match e {
                egui::Event::MouseWheel { delta, .. } => sum + delta.y,
                _ => sum,
            });
            (digit, wheel)
        });
        let pick = match (digit, self.locked && wheel != 0.0) {
            (Some(d), _) => Some(d),
            (None, true) => {
                let n = blocks.len() as i32;
                let now = self.hotbar_index().map_or(-1, |i| i as i32);
                // Wheel down goes to the next block.
                let step = if wheel < 0.0 { 1 } else { -1 };
                Some((now + step).rem_euclid(n) as usize)
            }
            _ => None,
        };
        if let Some(b) = pick.and_then(|i| blocks.get(i)) {
            self.brush = Some(Brush { block: b.clone() });
            self.pasting = None;
        }
    }

    /// Where the block in hand is in the palette.
    pub fn hotbar_index(&self) -> Option<usize> {
        let hand = &self.brush.as_ref()?.block;
        palette().iter().position(|(_, b, _)| same_kind(b, hand))
    }

    /// Mouse buttons while flying. Holding left or right repeats it.
    fn buttons(&mut self, ui: &Ui, aimed: Option<aim::Aim>) {
        use PointerButton::{Middle, Primary, Secondary};
        let (now, pressed, down, shift) = ui.input(|i| {
            (
                i.time,
                [Primary, Secondary, Middle].map(|b| i.pointer.button_pressed(b)),
                [Primary, Secondary].map(|b| i.pointer.button_down(b)),
                i.modifiers.shift,
            )
        });
        let mut act = None;
        if let Some(b) = [Primary, Secondary, Middle]
            .into_iter()
            .zip(pressed)
            .find_map(|(b, p)| p.then_some(b))
        {
            act = Some(b);
            self.repeat = (b != Middle).then_some((b, now + 0.3));
        } else if let Some((b, at)) = self.repeat {
            let held = if b == Primary { down[0] } else { down[1] };
            if !held {
                self.repeat = None;
            } else if now >= at {
                act = Some(b);
                self.repeat = Some((b, now + 0.2));
            }
        }
        let (Some(button), Some(a)) = (act, aimed) else {
            return;
        };
        match button {
            Primary => self.primary(a),
            Secondary => self.secondary(a, shift),
            _ => self.pick_block(a),
        }
    }

    /// Left click: break the block aimed at. With the select tool, mark a
    /// region's first corner; while pasting, stop.
    fn primary(&mut self, a: aim::Aim) {
        if self.pasting.take().is_some() {
            self.status = None;
            return;
        }
        if self.selecting() {
            let p = a.target.unwrap_or(a.place);
            self.sel_start = Some(p);
            self.selection = Some(Region::between(p, p));
            self.selected = self.world.device_at(p).map(|d| d.0);
            return;
        }
        if let Some(t) = a.target {
            self.erase(t);
        }
    }

    /// Right click: build against the face aimed at, or use what is there
    /// (`force`: build anyway). With the select tool, mark the second
    /// corner; while pasting, put the blocks down.
    fn secondary(&mut self, a: aim::Aim, force: bool) {
        if self.pasting.is_some() {
            self.paste_at(a.place);
            return;
        }
        if self.selecting() {
            let p = a.target.unwrap_or(a.place);
            let start = *self.sel_start.get_or_insert(p);
            self.selection = Some(Region::between(start, p));
            return;
        }
        if let Some(t) = a.target {
            let used = self.world.get(t).is_some_and(aim::interactive);
            if used && !force {
                self.place(t, true);
                return;
            }
        }
        // Devices are never painted over: building against one goes beside.
        if self.world.device_at(a.place).is_none() {
            self.place(a.place, true);
        }
    }

    /// Middle click: take the block aimed at in hand.
    fn pick_block(&mut self, a: aim::Aim) {
        let Some(t) = a.target else { return };
        let block = match self.world.device_at(t) {
            Some((_, d, f)) => Block::Device {
                device: d.clone(),
                facing: f,
            },
            None => match self.world.get(t) {
                Some(b) => b.clone(),
                None => return,
            },
        };
        self.brush = Some(Brush { block });
        self.pasting = None;
    }

    /// Move the floor to layer `z`.
    pub fn set_layer(&mut self, z: i32) {
        let lim = makine_world::world::MAX_COORD;
        self.layer = z.clamp(-lim, lim);
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

/// Whether two blocks are the same palette entry (whatever their facing,
/// state or name).
fn same_kind(a: &Block, b: &Block) -> bool {
    match (a, b) {
        (Block::Gate { gate: x, .. }, Block::Gate { gate: y, .. }) => x == y,
        (Block::Port { output: x, .. }, Block::Port { output: y, .. }) => x == y,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}
