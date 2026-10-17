//! The isometric world view: draws the block world and lets the player
//! build in it.
//!
//! Projection: world (x, y, z) maps to screen with x going down-right, y
//! down-left and z straight up, the classic 2:1 isometric grid. Layers above
//! the one being edited are hidden; layers below are drawn dimmed, so the
//! player sees what they build on.
//!
//! Controls: left click places the selected block (drag to paint), right
//! click removes, R rotates the selection, Page Up/Down (or Q/E) change
//! layer, middle-drag or Space+drag pans, the wheel zooms.

use std::collections::HashMap;

use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use makine_sim::{Circuit, Library};
use makine_world::block::{Block, Dir, Gate};
use makine_world::compile::{compile, Compiled, InputSource, Lane};
use makine_world::world::MAX_LAYER;
use makine_world::{Edit, Pos, World};

/// Size of one cell on screen at zoom 1: half the tile's width and height.
const TW: f32 = 28.0;
const TH: f32 = 14.0;
/// Height of one layer on screen at zoom 1.
const LAYER_H: f32 = 22.0;

const BG: Color32 = Color32::from_rgb(20, 22, 27);
const GRID: Color32 = Color32::from_rgb(40, 44, 52);
const WIRE_OFF: Color32 = Color32::from_rgb(110, 60, 55);
const WIRE_ON: Color32 = Color32::from_rgb(255, 80, 60);
const GATE_TOP: Color32 = Color32::from_rgb(200, 205, 215);
const GATE_SIDE_L: Color32 = Color32::from_rgb(150, 155, 168);
const GATE_SIDE_R: Color32 = Color32::from_rgb(120, 125, 138);
const TEXT: Color32 = Color32::from_rgb(30, 32, 38);
const LAMP_OFF: Color32 = Color32::from_rgb(70, 64, 40);
const LAMP_ON: Color32 = Color32::from_rgb(255, 220, 90);
const VIA: Color32 = Color32::from_rgb(120, 170, 230);
const PORT: Color32 = Color32::from_rgb(90, 200, 140);
const ERROR: Color32 = Color32::from_rgb(255, 70, 70);

/// The block the player is placing.
#[derive(Clone, Debug, PartialEq)]
pub struct Brush {
    pub block: Block,
}

/// Palette entries: what the build bar offers.
pub fn palette() -> Vec<(&'static str, Block, &'static str)> {
    let g = |gate| Block::Gate {
        gate,
        facing: Dir::East,
    };
    vec![
        ("Tel", Block::Wire, "Komşu tellere bağlanır"),
        ("Köprü", Block::Cross, "İki tel birbirine değmeden kesişir"),
        ("Geçiş", Block::Via, "Üst ve alt katmana bağlanır"),
        ("NOT", g(Gate::Not), "Arkasından okur, önüne yazar"),
        ("AND", g(Gate::And), "İki yanından okur, önüne yazar"),
        ("OR", g(Gate::Or), ""),
        ("XOR", g(Gate::Xor), ""),
        ("NAND", g(Gate::Nand), ""),
        ("NOR", g(Gate::Nor), ""),
        (
            "Anahtar",
            Block::Switch {
                on: false,
                facing: Dir::East,
            },
            "Tıkla: aç/kapa",
        ),
        ("Lamba", Block::Lamp, "Komşu tel 1 ise yanar"),
        (
            "Saat",
            Block::Clock {
                period: 30,
                facing: Dir::East,
            },
            "Kendi kendine yanıp söner",
        ),
    ]
}

pub struct WorldView {
    pub world: World,
    /// Layer being edited.
    pub layer: i32,
    pub brush: Option<Brush>,
    /// Facing applied to placed blocks; R rotates it.
    pub facing: Dir,
    pan: Vec2,
    zoom: f32,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Compiled circuit for the current world revision.
    compiled: Option<Result<(Compiled, Circuit), String>>,
    compiled_rev: u64,
    /// Ticks since start, for clocks.
    tick: u64,
    pub running: bool,
    /// Cell under the pointer last frame.
    hover: Option<Pos>,
    /// The last cell painted in the current drag, so dragging does not
    /// repaint the same cell every frame.
    painted: Option<Pos>,
    /// Centre the camera on the world at the next frame.
    fit: bool,
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
            pan: vec2(0.0, 0.0),
            zoom: 1.0,
            undo: Vec::new(),
            redo: Vec::new(),
            compiled: None,
            compiled_rev: u64::MAX,
            tick: 0,
            running: true,
            hover: None,
            painted: None,
            fit: true,
        }
    }

    /// Centre and zoom the camera so the whole world fits in `rect`.
    fn fit_to(&mut self, rect: Rect, origin: Pos2) {
        let Some((lo, hi)) = self.world.bounds() else {
            self.zoom = 1.6;
            self.pan = rect.center() - origin;
            return;
        };
        self.zoom = 1.0;
        self.pan = vec2(0.0, 0.0);
        let pts = [
            self.to_screen(origin, lo.x as f32, lo.y as f32, 0.0),
            self.to_screen(origin, hi.x as f32 + 1.0, lo.y as f32, 0.0),
            self.to_screen(origin, lo.x as f32, hi.y as f32 + 1.0, 0.0),
            self.to_screen(
                origin,
                hi.x as f32 + 1.0,
                hi.y as f32 + 1.0,
                hi.z as f32 + 1.0,
            ),
        ];
        let b = Rect::from_points(&pts);
        let zoom = (rect.width() * 0.7 / b.width().max(1.0))
            .min(rect.height() * 0.7 / b.height().max(1.0))
            .clamp(0.5, 2.5);
        self.zoom = zoom;
        let b = Rect::from_points(&pts.map(|p| origin + (p - origin) * zoom));
        self.pan = rect.center() - b.center();
    }

    /// Apply an edit made by the player (undoable).
    pub fn edit(&mut self, e: Edit) {
        let inv = self.world.apply(e);
        self.undo.push(inv);
        self.redo.clear();
        if self.undo.len() > 500 {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(e) = self.undo.pop() {
            let inv = self.world.apply(e);
            self.redo.push(inv);
        }
    }

    pub fn redo(&mut self) {
        if let Some(e) = self.redo.pop() {
            let inv = self.world.apply(e);
            self.undo.push(inv);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The circuit, recompiled if the world changed. Errors are kept so the
    /// view can show them.
    fn circuit(&mut self) -> Option<&mut (Compiled, Circuit)> {
        if self.compiled_rev != self.world.revision() {
            self.compiled_rev = self.world.revision();
            self.compiled = Some(
                compile(&self.world)
                    .map_err(|e| e.to_string())
                    .and_then(|c| {
                        let sim = Circuit::compile_def(&Library::new(), &c.def)
                            .map_err(|e| e.to_string())?;
                        Ok((c, sim))
                    }),
            );
        }
        self.compiled.as_mut()?.as_mut().ok()
    }

    pub fn error(&self) -> Option<&str> {
        match &self.compiled {
            Some(Err(e)) => Some(e),
            _ => None,
        }
    }

    /// Advance the simulation one tick: set switch and clock inputs from the
    /// world, evaluate. Returns the value of every chip output.
    fn step(&mut self) -> Vec<u64> {
        let tick = self.tick;
        if self.running {
            self.tick += 1;
        }
        let switches: HashMap<Pos, bool> = self
            .world
            .blocks()
            .filter_map(|(p, b)| match b {
                Block::Switch { on, .. } => Some((p, *on)),
                _ => None,
            })
            .collect();
        let Some((c, sim)) = self.circuit() else {
            return Vec::new();
        };
        for (i, src) in c.inputs.iter().enumerate() {
            let v = match src {
                InputSource::Switch(p) => switches.get(p).copied().unwrap_or(false),
                InputSource::Clock { period, .. } => (tick / (*period).max(1) as u64) % 2 == 1,
                InputSource::Port(_) => false,
            };
            sim.set_input(i, v as u64);
        }
        if sim.eval().is_err() {
            return Vec::new();
        }
        (0..c.def.outputs.len()).map(|o| sim.output(o)).collect()
    }

    fn to_screen(&self, origin: Pos2, x: f32, y: f32, z: f32) -> Pos2 {
        let z = z * self.zoom;
        origin
            + self.pan
            + vec2(
                (x - y) * TW * self.zoom,
                (x + y) * TH * self.zoom - z * LAYER_H,
            )
    }

    /// The cell on `layer` under a screen point.
    fn pick(&self, origin: Pos2, p: Pos2, layer: i32) -> Pos {
        let v = (p - origin - self.pan) / self.zoom;
        let vy = v.y + layer as f32 * LAYER_H;
        // Invert: sx = (x - y) TW, sy = (x + y) TH.
        let a = v.x / TW;
        let b = vy / TH;
        let x = ((a + b) / 2.0).floor() as i32;
        let y = ((b - a) / 2.0).floor() as i32;
        Pos::new(x, y, layer)
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        let origin = rect.center_top() + vec2(0.0, 80.0);
        painter.rect_filled(rect, 0.0, BG);
        if std::mem::take(&mut self.fit) {
            self.fit_to(rect, origin);
        }

        // Camera: wheel zooms around the pointer, middle-drag or Space+drag
        // pans.
        if let Some(hp) = response.hover_pos() {
            let (scroll, zd) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = zd * (scroll * 0.0015).exp();
            if f != 1.0 {
                let new = (self.zoom * f).clamp(0.3, 3.0);
                let anchor = hp - origin - self.pan;
                self.pan += anchor - anchor * (new / self.zoom);
                self.zoom = new;
            }
        }
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if response.dragged_by(egui::PointerButton::Middle)
            || (space && response.dragged_by(egui::PointerButton::Primary))
        {
            self.pan += response.drag_delta();
        }

        // Keys.
        if response.hovered() || response.has_focus() {
            ui.input(|i| {
                if i.key_pressed(egui::Key::R) {
                    self.facing = if i.modifiers.shift {
                        self.facing.ccw()
                    } else {
                        self.facing.cw()
                    };
                }
                if i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::E) {
                    self.layer = (self.layer + 1).min(MAX_LAYER);
                }
                if i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::Q) {
                    self.layer = (self.layer - 1).max(0);
                }
            });
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

        // Pointer: which cell?
        self.hover = response
            .hover_pos()
            .map(|p| self.pick(origin, p, self.layer));

        // Building.
        if !space {
            // A drag is only recognized once the pointer has moved; paint the
            // cell where the press started too.
            if response.drag_started() {
                self.painted = None;
                let press = ui.input(|i| i.pointer.press_origin());
                if let Some(start) = press.map(|p| self.pick(origin, p, self.layer)) {
                    if response.dragged_by(egui::PointerButton::Primary) {
                        self.painted = Some(start);
                        self.place(start, false);
                    } else if response.dragged_by(egui::PointerButton::Secondary)
                        && self.world.get(start).is_some()
                    {
                        self.painted = Some(start);
                        self.edit(Edit::Set {
                            pos: start,
                            block: None,
                        });
                    }
                }
            }
            if let Some(cell) = self.hover {
                let primary =
                    response.clicked() || response.dragged_by(egui::PointerButton::Primary);
                let secondary = response.secondary_clicked()
                    || response.dragged_by(egui::PointerButton::Secondary);
                if response.clicked() || response.secondary_clicked() {
                    self.painted = None;
                }
                if primary && self.painted != Some(cell) {
                    self.painted = Some(cell);
                    self.place(cell, response.clicked());
                } else if secondary && self.painted != Some(cell) {
                    self.painted = Some(cell);
                    if self.world.get(cell).is_some() {
                        self.edit(Edit::Set {
                            pos: cell,
                            block: None,
                        });
                    }
                }
            }
        }

        // Simulate and draw.
        let values = self.step();
        self.draw(&painter, origin, rect, &values);
        if self.running
            && self
                .world
                .blocks()
                .any(|(_, b)| matches!(b, Block::Clock { .. }))
        {
            ui.ctx().request_repaint();
        }
    }

    /// Place the brush at `cell`. A click on a switch with the switch brush
    /// (or any click on a switch while not holding one) toggles it.
    fn place(&mut self, cell: Pos, click: bool) {
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

    fn draw(&self, painter: &egui::Painter, origin: Pos2, clip: Rect, values: &[u64]) {
        let z = self.zoom;
        let compiled = match &self.compiled {
            Some(Ok((c, _))) => Some(c),
            _ => None,
        };
        let value_at = |p: Pos, lane: Lane| -> Option<bool> {
            let o = compiled?.output_at(p, lane)?;
            values.get(o).map(|v| *v == 1)
        };

        // Floor grid on the current layer, around the visible area.
        let corners = [
            clip.left_top(),
            clip.right_top(),
            clip.left_bottom(),
            clip.right_bottom(),
        ];
        let cells: Vec<Pos> = corners
            .iter()
            .map(|&c| self.pick(origin, c, self.layer))
            .collect();
        let (x0, x1) = (
            cells.iter().map(|c| c.x).min().unwrap(),
            cells.iter().map(|c| c.x).max().unwrap(),
        );
        let (y0, y1) = (
            cells.iter().map(|c| c.y).min().unwrap(),
            cells.iter().map(|c| c.y).max().unwrap(),
        );
        if (x1 - x0) * (y1 - y0) < 40_000 {
            let lz = self.layer as f32;
            for x in x0..=x1 + 1 {
                painter.line_segment(
                    [
                        self.to_screen(origin, x as f32, y0 as f32, lz),
                        self.to_screen(origin, x as f32, (y1 + 1) as f32, lz),
                    ],
                    Stroke::new(1.0, GRID),
                );
            }
            for y in y0..=y1 + 1 {
                painter.line_segment(
                    [
                        self.to_screen(origin, x0 as f32, y as f32, lz),
                        self.to_screen(origin, (x1 + 1) as f32, y as f32, lz),
                    ],
                    Stroke::new(1.0, GRID),
                );
            }
        }

        // Blocks, back to front: by layer, then x + y.
        let mut blocks: Vec<(Pos, &Block)> = self
            .world
            .blocks()
            .filter(|(p, _)| p.z <= self.layer)
            .collect();
        blocks.sort_by_key(|(p, _)| (p.z, p.x + p.y, p.x));
        for (p, b) in blocks {
            let dim = p.z < self.layer;
            let tile = [
                self.to_screen(origin, p.x as f32, p.y as f32, p.z as f32),
                self.to_screen(origin, p.x as f32 + 1.0, p.y as f32, p.z as f32),
                self.to_screen(origin, p.x as f32 + 1.0, p.y as f32 + 1.0, p.z as f32),
                self.to_screen(origin, p.x as f32, p.y as f32 + 1.0, p.z as f32),
            ];
            let bounds = Rect::from_points(&tile).expand(LAYER_H * z);
            if !clip.intersects(bounds) {
                continue;
            }
            let fade = |c: Color32| if dim { c.gamma_multiply(0.35) } else { c };
            let center = self.to_screen(origin, p.x as f32 + 0.5, p.y as f32 + 0.5, p.z as f32);
            match b {
                Block::Wire | Block::Cross | Block::Via => {
                    let lane_color = |lane| match value_at(p, lane) {
                        Some(true) => WIRE_ON,
                        _ => WIRE_OFF,
                    };
                    let w = 4.0 * z;
                    // Connections to neighbours that accept a wire.
                    for d in Dir::ALL {
                        let n = self.world.get(p.step(d));
                        if n.is_some_and(|n| n.accepts_wire_from(d.opposite())) {
                            let lane = if matches!(b, Block::Cross) && !d.is_horizontal() {
                                Lane::Cross
                            } else {
                                Lane::Main
                            };
                            let (dx, dy) = d.delta();
                            let edge = self.to_screen(
                                origin,
                                p.x as f32 + 0.5 + dx as f32 * 0.5,
                                p.y as f32 + 0.5 + dy as f32 * 0.5,
                                p.z as f32,
                            );
                            painter.line_segment(
                                [center, edge],
                                Stroke::new(w, fade(lane_color(lane))),
                            );
                        }
                    }
                    let dot = match b {
                        Block::Via => VIA,
                        _ => lane_color(Lane::Main),
                    };
                    painter.circle_filled(
                        center,
                        w * if matches!(b, Block::Via) { 1.6 } else { 0.7 },
                        fade(dot),
                    );
                    if matches!(b, Block::Cross) {
                        painter.circle_stroke(center, w * 1.3, Stroke::new(1.5, fade(GATE_SIDE_L)));
                    }
                }
                Block::Gate { gate, facing } => {
                    self.draw_box(
                        painter,
                        origin,
                        p,
                        0.6,
                        fade(GATE_TOP),
                        fade(GATE_SIDE_L),
                        fade(GATE_SIDE_R),
                    );
                    let top = self.to_screen(
                        origin,
                        p.x as f32 + 0.45,
                        p.y as f32 + 0.45,
                        p.z as f32 + 0.6,
                    );
                    if z > 0.7 {
                        painter.text(
                            top,
                            Align2::CENTER_CENTER,
                            gate.name(),
                            FontId::proportional(8.0 * z),
                            fade(TEXT),
                        );
                    }
                    self.arrow(
                        painter,
                        origin,
                        p,
                        0.6,
                        *facing,
                        fade(match value_at(p, Lane::Main) {
                            Some(true) => WIRE_ON,
                            _ => Color32::from_rgb(60, 64, 74),
                        }),
                    );
                }
                Block::Switch { on, facing } => {
                    let c = if *on {
                        WIRE_ON
                    } else {
                        Color32::from_rgb(90, 90, 100)
                    };
                    self.draw_box(
                        painter,
                        origin,
                        p,
                        0.3,
                        fade(c),
                        fade(GATE_SIDE_L),
                        fade(GATE_SIDE_R),
                    );
                    self.arrow(painter, origin, p, 0.3, *facing, fade(Color32::WHITE));
                }
                Block::Lamp => {
                    let lit = value_at(p, Lane::Main) == Some(true);
                    let c = if lit { LAMP_ON } else { LAMP_OFF };
                    self.draw_box(
                        painter,
                        origin,
                        p,
                        0.8,
                        fade(c),
                        fade(c.gamma_multiply(0.8)),
                        fade(c.gamma_multiply(0.6)),
                    );
                    if lit && !dim {
                        let top = self.to_screen(
                            origin,
                            p.x as f32 + 0.5,
                            p.y as f32 + 0.5,
                            p.z as f32 + 0.8,
                        );
                        painter.circle_filled(top, 18.0 * z, LAMP_ON.gamma_multiply(0.18));
                    }
                }
                Block::Clock { facing, .. } => {
                    let on = value_at(p, Lane::Main) == Some(true);
                    let c = if on {
                        Color32::from_rgb(230, 160, 80)
                    } else {
                        Color32::from_rgb(120, 90, 60)
                    };
                    self.draw_box(
                        painter,
                        origin,
                        p,
                        0.5,
                        fade(c),
                        fade(GATE_SIDE_L),
                        fade(GATE_SIDE_R),
                    );
                    self.arrow(painter, origin, p, 0.5, *facing, fade(Color32::WHITE));
                }
                Block::Port { name, facing, .. } => {
                    self.draw_box(
                        painter,
                        origin,
                        p,
                        0.4,
                        fade(PORT),
                        fade(GATE_SIDE_L),
                        fade(GATE_SIDE_R),
                    );
                    let top = self.to_screen(
                        origin,
                        p.x as f32 + 0.5,
                        p.y as f32 + 0.5,
                        p.z as f32 + 0.4,
                    );
                    painter.text(
                        top,
                        Align2::CENTER_CENTER,
                        name,
                        FontId::proportional(9.0 * z),
                        fade(TEXT),
                    );
                    self.arrow(painter, origin, p, 0.4, *facing, fade(TEXT));
                }
            }
        }

        // Cursor and a preview of the brush.
        if let Some(cell) = self.hover {
            let tile = [
                self.to_screen(origin, cell.x as f32, cell.y as f32, cell.z as f32),
                self.to_screen(origin, cell.x as f32 + 1.0, cell.y as f32, cell.z as f32),
                self.to_screen(
                    origin,
                    cell.x as f32 + 1.0,
                    cell.y as f32 + 1.0,
                    cell.z as f32,
                ),
                self.to_screen(origin, cell.x as f32, cell.y as f32 + 1.0, cell.z as f32),
            ];
            painter.add(Shape::closed_line(
                tile.to_vec(),
                Stroke::new(2.0, Color32::from_white_alpha(160)),
            ));
            if let Some(b) = &self.brush {
                if b.block.facing().is_some() {
                    self.arrow(
                        painter,
                        origin,
                        cell,
                        0.05,
                        self.facing,
                        Color32::from_white_alpha(120),
                    );
                }
            }
        }

        // Short circuits: mark the drivers.
        if let Some(Err(_)) = &self.compiled {
            if let Err(makine_world::compile::CompileError::ShortCircuit(ps)) = compile(&self.world)
            {
                for p in ps {
                    let c = self.to_screen(
                        origin,
                        p.x as f32 + 0.5,
                        p.y as f32 + 0.5,
                        p.z as f32 + 0.6,
                    );
                    painter.circle_stroke(c, 14.0 * z, Stroke::new(3.0, ERROR));
                }
            }
        }
    }

    /// An isometric box on cell `p`, `h` layers tall.
    #[allow(clippy::too_many_arguments)]
    fn draw_box(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        p: Pos,
        h: f32,
        top: Color32,
        left: Color32,
        right: Color32,
    ) {
        let (x, y, z) = (p.x as f32 + 0.1, p.y as f32 + 0.1, p.z as f32);
        let s = 0.8;
        let at = |dx: f32, dy: f32, dz: f32| self.to_screen(origin, x + dx, y + dy, z + dz);
        // Visible sides: +y face (left) and +x face (right).
        painter.add(Shape::convex_polygon(
            vec![at(0.0, s, 0.0), at(s, s, 0.0), at(s, s, h), at(0.0, s, h)],
            left,
            Stroke::NONE,
        ));
        painter.add(Shape::convex_polygon(
            vec![at(s, 0.0, 0.0), at(s, s, 0.0), at(s, s, h), at(s, 0.0, h)],
            right,
            Stroke::NONE,
        ));
        painter.add(Shape::convex_polygon(
            vec![at(0.0, 0.0, h), at(s, 0.0, h), at(s, s, h), at(0.0, s, h)],
            top,
            Stroke::new(1.0, Color32::from_black_alpha(80)),
        ));
    }

    /// Fit the camera to the world at the next frame.
    pub fn refit(&mut self) {
        self.fit = true;
    }

    /// An arrow on top of a block at height `h`, pointing `d`.
    fn arrow(&self, painter: &egui::Painter, origin: Pos2, p: Pos, h: f32, d: Dir, color: Color32) {
        let (dx, dy) = d.delta();
        let (cx, cy) = (p.x as f32 + 0.5, p.y as f32 + 0.5);
        let (px, py) = (-(dy as f32), dx as f32);
        let at = |u: f32, v: f32| {
            self.to_screen(
                origin,
                cx + dx as f32 * u + px * v,
                cy + dy as f32 * u + py * v,
                p.z as f32 + h,
            )
        };
        painter.add(Shape::convex_polygon(
            vec![at(0.40, 0.0), at(0.26, 0.12), at(0.26, -0.12)],
            color,
            Stroke::NONE,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use egui::pos2;

    #[test]
    fn pick_inverts_projection() {
        let v = WorldView::default();
        let origin = pos2(400.0, 100.0);
        for (x, y, z) in [(0, 0, 0), (3, -2, 0), (-5, 7, 2), (10, 10, 1)] {
            // Centre of the cell's floor tile on its layer.
            let c = v.to_screen(origin, x as f32 + 0.5, y as f32 + 0.5, z as f32);
            assert_eq!(v.pick(origin, c, z), Pos::new(x, y, z));
        }
    }

    #[test]
    fn edits_undo_redo() {
        let mut v = WorldView::default();
        v.edit(Edit::Set {
            pos: Pos::new(0, 0, 0),
            block: Some(Block::Wire),
        });
        v.edit(Edit::Set {
            pos: Pos::new(1, 0, 0),
            block: Some(Block::Lamp),
        });
        assert_eq!(v.world.len(), 2);
        v.undo();
        assert_eq!(v.world.len(), 1);
        v.redo();
        assert_eq!(v.world.get(Pos::new(1, 0, 0)), Some(&Block::Lamp));
    }

    #[test]
    fn clock_and_lamp_run() {
        // clock -> wire -> lamp; the lamp blinks with the clock.
        let mut v = WorldView::default();
        v.edit(Edit::Set {
            pos: Pos::new(0, 0, 0),
            block: Some(Block::Clock {
                period: 2,
                facing: Dir::East,
            }),
        });
        v.edit(Edit::Set {
            pos: Pos::new(1, 0, 0),
            block: Some(Block::Wire),
        });
        v.edit(Edit::Set {
            pos: Pos::new(2, 0, 0),
            block: Some(Block::Lamp),
        });
        let mut lit = Vec::new();
        for _ in 0..8 {
            let values = v.step();
            let (c, _) = v.circuit().unwrap();
            let o = c.lamps[0].1.unwrap();
            lit.push(values[o]);
        }
        assert_eq!(lit, vec![0, 0, 1, 1, 0, 0, 1, 1]);
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    /// Paint a wire by dragging across cells, erase one with right click,
    /// and toggle a switch by clicking it.
    #[test]
    fn paint_erase_toggle() {
        let view = Rc::new(RefCell::new(WorldView::default()));
        let v = view.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(move |ui| v.borrow_mut().show(ui));
        h.run();
        // Screen centre of a cell on layer 0 (the camera was fitted on the
        // first frame).
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let cell = |view: &WorldView, x: i32, y: i32| {
            let origin = rect.center_top() + vec2(0.0, 80.0);
            view.to_screen(origin, x as f32 + 0.5, y as f32 + 0.5, 0.0)
        };
        let path: Vec<Pos2> = (0..4).map(|x| cell(&view.borrow(), x, 0)).collect();
        h.drag_at(path[0]);
        h.run();
        for p in &path[1..] {
            h.hover_at(*p);
            h.run();
        }
        h.drop_at(path[3]);
        h.run();
        let wires: Vec<Pos> = view.borrow().world.blocks().map(|(p, _)| p).collect();
        assert_eq!(wires.len(), 4, "{wires:?}");

        // Right click erases.
        let p = cell(&view.borrow(), 2, 0);
        h.event(egui::Event::PointerMoved(p));
        h.event(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
        assert_eq!(view.borrow().world.len(), 3);
        assert!(view.borrow().world.get(Pos::new(2, 0, 0)).is_none());

        // Place a switch, then click it to turn it on.
        view.borrow_mut().brush = Some(Brush {
            block: Block::Switch {
                on: false,
                facing: Dir::East,
            },
        });
        let p = cell(&view.borrow(), 0, 2);
        let click = |h: &mut egui_kittest::Harness| {
            h.event(egui::Event::PointerMoved(p));
            h.event(egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            h.event(egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
        };
        click(&mut h);
        assert_eq!(
            view.borrow().world.get(Pos::new(0, 2, 0)),
            Some(&Block::Switch {
                on: false,
                facing: Dir::East
            })
        );
        click(&mut h);
        assert_eq!(
            view.borrow().world.get(Pos::new(0, 2, 0)),
            Some(&Block::Switch {
                on: true,
                facing: Dir::East
            })
        );
        // Undo turns it off again.
        view.borrow_mut().undo();
        assert_eq!(
            view.borrow().world.get(Pos::new(0, 2, 0)),
            Some(&Block::Switch {
                on: false,
                facing: Dir::East
            })
        );
    }
}
