//! Drawing the world from above: a flat grid, one layer at a time.
//!
//! Each cell is a square. The edited layer is drawn in full; the layer just
//! below shows through faintly, so the player sees what they build on.
//! Gates are drawn as their logic symbols (the canvas's outlines), with
//! short leads on the sides wires attach to.

use std::collections::HashMap;

use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke};
use makine_world::block::{Block, Dir, Gate};
use makine_world::compile::{CompileError, Compiled, Lane};
use makine_world::device::{self, Device};
use makine_world::Pos;

use super::WorldView;
use crate::canvas::gate_outline;
use crate::design::Symbol;
use crate::theme;
use crate::worldsim::SimError;

/// Size of one cell on screen at zoom 1.
const CELL: f32 = 32.0;

pub(super) const BG: Color32 = theme::BOARD;
const GRID: Color32 = theme::BOARD_GRID;
const WIRE_OFF: Color32 = theme::SIGNAL_OFF;
const WIRE_ON: Color32 = theme::SIGNAL;
const GATE_FILL: Color32 = Color32::from_rgb(20, 26, 24);
const GATE_LINE: Color32 = theme::INK;
const BASE: Color32 = Color32::from_rgb(46, 58, 53);
const TEXT: Color32 = theme::INK;
const DARK: Color32 = Color32::from_rgb(12, 18, 15);
const LAMP_OFF: Color32 = Color32::from_rgb(66, 58, 36);
const LAMP_ON: Color32 = Color32::from_rgb(255, 238, 170);
/// Plated through-holes: a tin ring around a dark hole.
const VIA: Color32 = Color32::from_rgb(196, 204, 198);
/// Ports are silkscreened pads.
const PORT: Color32 = Color32::from_rgb(208, 220, 210);
const ERROR: Color32 = theme::BAD;
const SELECT: Color32 = Color32::from_rgb(130, 206, 255);
const PASTE: Color32 = Color32::from_rgb(130, 206, 255);

/// The rectangle a device covers: (min x, min y, max x, max y) in cell
/// coordinates, edges included.
fn extent(d: &Device, anchor: Pos, facing: Dir) -> (f32, f32, f32, f32) {
    let cells = device::layout(d, anchor, facing).cells;
    let x0 = cells.iter().map(|c| c.x).min().unwrap_or(anchor.x) as f32;
    let y0 = cells.iter().map(|c| c.y).min().unwrap_or(anchor.y) as f32;
    let x1 = cells.iter().map(|c| c.x).max().unwrap_or(anchor.x) as f32 + 1.0;
    let y1 = cells.iter().map(|c| c.y).max().unwrap_or(anchor.y) as f32 + 1.0;
    (x0, y0, x1, y1)
}

fn faded(c: Color32, below: bool) -> Color32 {
    if below {
        c.gamma_multiply(0.22)
    } else {
        c
    }
}

impl WorldView {
    /// Centre and zoom the camera so the world (its blocks on every layer)
    /// fits in `rect`.
    pub(super) fn fit_to(&mut self, rect: Rect, origin: Pos2) {
        let Some((lo, hi)) = self.world.bounds() else {
            self.zoom = 1.4;
            self.pan = rect.center() - origin;
            return;
        };
        let (w, h) = ((hi.x - lo.x + 1) as f32, (hi.y - lo.y + 1) as f32);
        // Leave room around the world to build in.
        self.zoom = (rect.width() * 0.6 / (w * CELL))
            .min(rect.height() * 0.6 / (h * CELL))
            .clamp(0.4, 1.5);
        let centre = vec2(lo.x as f32 + w / 2.0, lo.y as f32 + h / 2.0) * CELL * self.zoom;
        self.pan = rect.center() - origin - centre;
    }

    /// Screen position of world point (x, y) on any layer.
    pub(super) fn to_screen(&self, origin: Pos2, x: f32, y: f32) -> Pos2 {
        origin + self.pan + vec2(x, y) * CELL * self.zoom
    }

    /// The cell on `layer` under a screen point.
    pub(super) fn pick(&self, origin: Pos2, p: Pos2, layer: i32) -> Pos {
        let v = (p - origin - self.pan) / (CELL * self.zoom);
        Pos::new(v.x.floor() as i32, v.y.floor() as i32, layer)
    }

    /// The screen square of cell (x, y).
    fn cell_rect(&self, origin: Pos2, x: f32, y: f32) -> Rect {
        Rect::from_min_max(
            self.to_screen(origin, x, y),
            self.to_screen(origin, x + 1.0, y + 1.0),
        )
    }

    pub(super) fn draw(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        clip: Rect,
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
        let z = self.zoom;
        let compiled = self.sim.compiled();
        let values = self.sim.values();
        let value_at = |p: Pos, lane: Lane| -> Option<bool> {
            let o = compiled?.output_at(p, lane)?;
            self.sim.output(o)
        };

        // The visible cells.
        let a = self.pick(origin, clip.left_top(), self.layer);
        let b = self.pick(origin, clip.right_bottom(), self.layer);

        // The board: a dot at every cell corner, and a faint line every
        // eight cells to count by.
        if CELL * z >= 8.0 {
            let major = Stroke::new(1.0, GRID.gamma_multiply(0.55));
            for x in (a.x..=b.x + 1).filter(|x| x.rem_euclid(8) == 0) {
                painter.line_segment(
                    [
                        self.to_screen(origin, x as f32, a.y as f32),
                        self.to_screen(origin, x as f32, (b.y + 1) as f32),
                    ],
                    major,
                );
            }
            for y in (a.y..=b.y + 1).filter(|y| y.rem_euclid(8) == 0) {
                painter.line_segment(
                    [
                        self.to_screen(origin, a.x as f32, y as f32),
                        self.to_screen(origin, (b.x + 1) as f32, y as f32),
                    ],
                    major,
                );
            }
            let r = (1.1 * z).clamp(0.8, 1.6);
            for x in a.x..=b.x + 1 {
                for y in a.y..=b.y + 1 {
                    painter.circle_filled(self.to_screen(origin, x as f32, y as f32), r, GRID);
                }
            }
        }

        // The layer below, faintly, then the edited one.
        for z_layer in [self.layer - 1, self.layer] {
            let below = z_layer < self.layer;
            let lo = Pos::new(a.x - 1, a.y - 1, z_layer);
            let hi = Pos::new(b.x + 1, b.y + 1, z_layer);
            for (p, block) in self.world.blocks_in(lo, hi) {
                self.draw_block(
                    painter, origin, p, block, below, &value_at, compiled, values, screens,
                );
            }
        }

        self.draw_overlays(painter, origin);
    }

    /// One block on its cell.
    #[allow(clippy::too_many_arguments)]
    fn draw_block(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        p: Pos,
        block: &Block,
        below: bool,
        value_at: &dyn Fn(Pos, Lane) -> Option<bool>,
        compiled: Option<&Compiled>,
        values: &[u64],
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
        let z = self.zoom;
        let fade = |c: Color32| faded(c, below);
        let (x, y) = (p.x as f32, p.y as f32);
        let r = self.cell_rect(origin, x, y);
        let c = r.center();
        let lit = |lane| match value_at(p, lane) {
            Some(true) => WIRE_ON,
            _ => WIRE_OFF,
        };
        let font = |s: f32| FontId::proportional(s * z);
        match block {
            Block::Wire
            | Block::Trace { .. }
            | Block::Straight { .. }
            | Block::Cross
            | Block::Via => {
                let w = 6.0 * z;
                let mut arms = Vec::new();
                for d in Dir::ALL {
                    if !(self.world.accepts_wire(p, d)
                        && self.world.accepts_wire(p.step(d), d.opposite()))
                    {
                        continue;
                    }
                    let lane = if matches!(block, Block::Cross) && !d.is_horizontal() {
                        Lane::Cross
                    } else {
                        Lane::Main
                    };
                    let (dx, dy) = d.delta();
                    // A hair past the cell edge, so neighbouring arms overlap.
                    let edge = c + vec2(dx as f32, dy as f32) * (CELL * z / 2.0 + 0.5);
                    arms.push((edge, lit(lane)));
                }
                // Each arm starts a little behind the centre, so arms overlap
                // instead of leaving an anti-aliased seam where they meet.
                let back = |edge: Pos2, by: f32| c + (c - edge).normalized() * by;
                let bend = !(arms.len() == 2 && (arms[0].0 - c + (arms[1].0 - c)).length() < 0.5);
                if !bend && arms[0].1 == arms[1].1 {
                    // Straight through: one segment, so no seam in the middle.
                    let col = arms[0].1;
                    painter.line_segment([arms[0].0, arms[1].0], Stroke::new(w, fade(col)));
                    if col == WIRE_ON && !below {
                        let core = Color32::from_rgb(255, 238, 196);
                        painter.line_segment([arms[0].0, arms[1].0], Stroke::new(w * 0.3, core));
                    }
                    arms.clear();
                }
                for (edge, col) in &arms {
                    painter.line_segment([back(*edge, w * 0.5), *edge], Stroke::new(w, fade(*col)));
                }
                if !arms.is_empty() && bend && !matches!(block, Block::Via | Block::Cross) {
                    // Round the joint where arms meet at a corner or a tee.
                    painter.circle_filled(c, w * 0.5, fade(arms[0].1));
                }
                // A lit trace has a bright core.
                if !below {
                    let core = Color32::from_rgb(255, 238, 196);
                    for (edge, col) in &arms {
                        if *col == WIRE_ON {
                            painter.line_segment(
                                [back(*edge, w * 0.15), *edge],
                                Stroke::new(w * 0.3, core),
                            );
                        }
                    }
                    if bend && arms.iter().any(|(_, col)| *col == WIRE_ON) {
                        painter.circle_filled(c, w * 0.15, core);
                    }
                }
                match block {
                    Block::Via => {
                        painter.circle_filled(c, w * 1.4, fade(VIA));
                        painter.circle_filled(c, w * 0.6, fade(DARK));
                    }
                    Block::Cross => {
                        // The vertical lane hops over the horizontal one.
                        painter.circle_filled(c, w * 1.1, fade(BG));
                        painter.circle_stroke(c, w * 1.1, Stroke::new(1.5 * z, fade(GATE_LINE)));
                    }
                    // A wire end with no connection yet: a dot to grab.
                    Block::Trace { sides: 0 } => {
                        painter.circle_filled(c, w * 0.9, fade(lit(Lane::Main)));
                    }
                    // Joints are drawn with the arms above.
                    _ => {}
                }
            }
            Block::Gate { gate, facing } => {
                self.draw_gate(painter, origin, p, *gate, *facing, below, value_at);
            }
            Block::Switch { on, facing } => {
                painter.rect_filled(r.shrink(4.0 * z), 4.0 * z, fade(BASE));
                let knob = if *on { WIRE_ON } else { DARK };
                painter.rect_filled(r.shrink(9.0 * z), 3.0 * z, fade(knob));
                self.lead(painter, c, *facing, fade(lit(Lane::Main)));
                painter.text(
                    c,
                    Align2::CENTER_CENTER,
                    if *on { "1" } else { "0" },
                    font(12.0),
                    fade(TEXT),
                );
            }
            Block::Lamp => {
                let on = self.sim.lamp_lit(p);
                if on && !below {
                    painter.circle_filled(c, CELL * z * 0.75, LAMP_ON.gamma_multiply(0.18));
                }
                painter.circle_filled(
                    c,
                    CELL * z * 0.36,
                    fade(if on { LAMP_ON } else { LAMP_OFF }),
                );
                painter.circle_stroke(c, CELL * z * 0.36, Stroke::new(1.5 * z, fade(DARK)));
            }
            Block::Clock { facing, period } => {
                let on = value_at(p, Lane::Main) == Some(true);
                let fill = if on {
                    Color32::from_rgb(230, 160, 80)
                } else {
                    Color32::from_rgb(120, 90, 60)
                };
                painter.rect_filled(r.shrink(4.0 * z), 4.0 * z, fade(fill));
                self.lead(painter, c, *facing, fade(lit(Lane::Main)));
                if z > 0.7 {
                    painter.text(
                        c,
                        Align2::CENTER_CENTER,
                        format!("⏱{period}"),
                        font(9.0),
                        fade(DARK),
                    );
                }
            }
            Block::Port { name, facing, .. } => {
                painter.rect_filled(r.shrink(4.0 * z), 4.0 * z, fade(PORT));
                self.lead(painter, c, *facing, fade(DARK));
                painter.text(c, Align2::CENTER_CENTER, name, font(11.0), fade(DARK));
            }
            Block::Device { device, facing } => {
                self.draw_device(
                    painter, origin, p, device, *facing, below, value_at, compiled, values, screens,
                );
            }
            Block::Cover { .. } => {}
        }
    }

    /// A short arrow-lead from the centre of a cell towards its edge `d`.
    fn lead(&self, painter: &egui::Painter, c: Pos2, d: Dir, color: Color32) {
        let z = self.zoom;
        let (dx, dy) = d.delta();
        let dir = vec2(dx as f32, dy as f32);
        let side = vec2(-dir.y, dir.x);
        let tip = c + dir * (CELL * z * 0.5);
        let base = c + dir * (CELL * z * 0.3);
        painter.add(Shape::convex_polygon(
            vec![tip, base + side * 5.0 * z, base - side * 5.0 * z],
            color,
            Stroke::NONE,
        ));
    }

    /// A gate as its logic symbol, turned to face `facing`, with leads on
    /// the sides it reads (coloured with the wires there) and in front.
    #[allow(clippy::too_many_arguments)]
    fn draw_gate(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        p: Pos,
        gate: Gate,
        facing: Dir,
        below: bool,
        value_at: &dyn Fn(Pos, Lane) -> Option<bool>,
    ) {
        let z = self.zoom;
        let fade = |c: Color32| faded(c, below);
        let c = self.cell_rect(origin, p.x as f32, p.y as f32).center();
        let s = CELL * z;
        // Local frame: u along the facing, v to its right; units of a cell.
        let (fx, fy) = facing.delta();
        let (sx, sy) = facing.cw().delta();
        let at = |u: f32, v: f32| {
            c + vec2(u * fx as f32 + v * sx as f32, u * fy as f32 + v * sy as f32) * s
        };
        let symbol = match gate {
            Gate::Not => Symbol::Not,
            Gate::And => Symbol::And,
            Gate::Nand => Symbol::Nand,
            Gate::Or | Gate::Nor => Symbol::Or,
            Gate::Xor => Symbol::Xor,
        };
        let bubble = matches!(gate, Gate::Not | Gate::Nand | Gate::Nor);
        let (u0, u1, half) = (-0.3, if bubble { 0.2 } else { 0.28 }, 0.3);

        // Leads: the wire values reaching each input, and the output.
        let wire = |d: Dir| {
            let n = p.step(d);
            let on =
                self.world.accepts_wire(n, d.opposite()) && value_at(n, Lane::Main) == Some(true);
            fade(if on { WIRE_ON } else { WIRE_OFF })
        };
        let stroke = |col| Stroke::new(3.0 * z, col);
        let out = fade(match value_at(p, Lane::Main) {
            Some(true) => WIRE_ON,
            _ => WIRE_OFF,
        });
        painter.line_segment([at(u1, 0.0), at(0.5, 0.0)], stroke(out));
        if gate.inputs() == 1 {
            let d = facing.opposite();
            painter.line_segment([at(-0.5, 0.0), at(u0, 0.0)], stroke(wire(d)));
        } else {
            for (d, v) in [(facing.ccw(), -1.0), (facing.cw(), 1.0)] {
                // From the side edge round into the back of the body.
                let k = v * 0.17;
                let back = u0 - 0.08;
                painter.line_segment([at(back, v * 0.5), at(back, k)], stroke(wire(d)));
                painter.line_segment([at(back, k), at(u0 + 0.06, k)], stroke(wire(d)));
            }
        }

        // The body, in the local frame then turned onto the cell.
        let outline: Vec<Pos2> = gate_outline(symbol, u0, u1, -half, half)
            .into_iter()
            .map(|q| at(q.x, q.y))
            .collect();
        let line = Stroke::new(2.0 * z.max(0.6), fade(GATE_LINE));
        if gate == Gate::Xor {
            let pts: Vec<Pos2> = (0..=10)
                .map(|k| {
                    let t = k as f32 / 10.0;
                    let v = -half + 2.0 * half * t;
                    at(u0 - 0.09 + 4.0 * t * (1.0 - t) * (u1 - u0) * 0.2, v)
                })
                .collect();
            painter.add(Shape::line(pts, line));
        }
        // Concave outlines (OR) cannot use a convex fill: fan from inside.
        let mid = at((u0 + u1) / 2.0, 0.0);
        let mut mesh = egui::Mesh::default();
        let fill = fade(GATE_FILL);
        mesh.colored_vertex(mid, fill);
        for q in &outline {
            mesh.colored_vertex(*q, fill);
        }
        let n = outline.len() as u32;
        for i in 0..n {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        painter.add(Shape::mesh(mesh));
        painter.add(Shape::closed_line(outline, line));
        if bubble {
            painter.circle(at(u1 + 0.06, 0.0), 0.06 * s, fill, line);
        }
        if z > 0.8 && !below {
            painter.text(
                at((u0 + u1) / 2.0 - 0.02, 0.0),
                Align2::CENTER_CENTER,
                gate.name(),
                FontId::proportional(7.5 * z),
                TEXT,
            );
        }
    }

    /// The cursor, the selection, blocks being pasted, the selected
    /// device, other players and short circuits.
    fn draw_overlays(&self, painter: &egui::Painter, origin: Pos2) {
        let z = self.zoom;
        if let Some(cell) = self.hover.filter(|_| self.pasting.is_none()) {
            let r = self.cell_rect(origin, cell.x as f32, cell.y as f32);
            painter.rect_stroke(
                r,
                2.0,
                Stroke::new(2.0, Color32::from_white_alpha(170)),
                egui::StrokeKind::Inside,
            );
            if self
                .brush
                .as_ref()
                .is_some_and(|b| b.block.facing().is_some())
            {
                self.lead(
                    painter,
                    r.center(),
                    self.facing,
                    Color32::from_white_alpha(120),
                );
            }
        }
        if let Some(r) = self.selection {
            let rect = Rect::from_min_max(
                self.to_screen(origin, r.x0 as f32, r.y0 as f32),
                self.to_screen(origin, r.x1 as f32 + 1.0, r.y1 as f32 + 1.0),
            );
            painter.rect_filled(rect, 0.0, SELECT.gamma_multiply(0.1));
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(2.0, SELECT),
                egui::StrokeKind::Inside,
            );
        }
        if let (Some(clip), Some(at)) = (&self.pasting, self.hover) {
            for (p, _) in &clip.blocks {
                let r = self.cell_rect(origin, (at.x + p.x) as f32, (at.y + p.y) as f32);
                painter.rect_filled(r.shrink(1.0), 2.0, PASTE.gamma_multiply(0.3));
            }
        }
        if let Some((anchor, d, f)) = self.selected_device() {
            let (x0, y0, x1, y1) = extent(&d, anchor, f);
            let rect = Rect::from_min_max(
                self.to_screen(origin, x0, y0),
                self.to_screen(origin, x1, y1),
            );
            painter.rect_stroke(
                rect,
                3.0,
                Stroke::new(2.5, Color32::from_rgb(120, 200, 255)),
                egui::StrokeKind::Outside,
            );
        }
        for (name, color, p) in &self.others {
            let r = self.cell_rect(origin, p.x as f32, p.y as f32);
            painter.rect_stroke(r, 2.0, Stroke::new(2.5, *color), egui::StrokeKind::Outside);
            let label = if p.z == self.layer {
                name.clone()
            } else {
                format!("{name} (katman {})", p.z)
            };
            painter.text(
                r.center_top() - vec2(0.0, 2.0),
                Align2::CENTER_BOTTOM,
                label,
                FontId::proportional(12.0),
                *color,
            );
        }
        if let Some(SimError::Compile(CompileError::ShortCircuit(ps))) = self.sim.error() {
            for p in ps.iter().filter(|p| p.z == self.layer) {
                let c = self.cell_rect(origin, p.x as f32, p.y as f32).center();
                painter.circle_stroke(c, 0.6 * CELL * z, Stroke::new(3.0, ERROR));
            }
        }
    }

    /// Pixels of every screen device, by anchor.
    pub(super) fn screens(&mut self) -> HashMap<Pos, Vec<bool>> {
        let Some(c) = self.sim.compiled() else {
            return HashMap::new();
        };
        let wanted: Vec<(Pos, String)> = c
            .devices
            .iter()
            .filter(|d| matches!(d.device, Device::Screen { .. }))
            .filter_map(|d| Some((d.anchor, d.native.clone()?)))
            .collect();
        let Some(sim) = self.sim.running_circuit() else {
            return HashMap::new();
        };
        wanted
            .into_iter()
            .filter_map(|(a, n)| Some((a, device::screen(sim, &n)?)))
            .collect()
    }

    /// A device: one rectangle over all its cells, pins along its edges and
    /// its face (screen pixels, a number, a name) inside.
    #[allow(clippy::too_many_arguments)]
    fn draw_device(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        anchor: Pos,
        d: &Device,
        facing: Dir,
        below: bool,
        value_at: &dyn Fn(Pos, Lane) -> Option<bool>,
        compiled: Option<&Compiled>,
        values: &[u64],
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
        let fade = |c: Color32| faded(c, below);
        let z = self.zoom;
        let (x0, y0, x1, y1) = extent(d, anchor, facing);
        let body = match d {
            Device::Ram { .. } => Color32::from_rgb(30, 54, 78),
            Device::Rom { .. } => Color32::from_rgb(58, 42, 78),
            Device::Screen { .. } => Color32::from_rgb(26, 30, 30),
            Device::Keyboard => Color32::from_rgb(48, 54, 50),
            Device::Display { .. } => Color32::from_rgb(16, 38, 28),
            Device::Chip { .. } => Color32::from_rgb(24, 30, 27),
        };
        let rect = Rect::from_min_max(
            self.to_screen(origin, x0 + 0.08, y0 + 0.08),
            self.to_screen(origin, x1 - 0.08, y1 - 0.08),
        );
        painter.rect_filled(rect, 4.0 * z, fade(body));
        painter.rect_stroke(
            rect,
            4.0 * z,
            Stroke::new(1.5, fade(body.gamma_multiply(1.6))),
            egui::StrokeKind::Inside,
        );

        // Local axes: u along the facing, v across (to its right).
        let (fx, fy) = facing.delta();
        let (sx, sy) = facing.cw().delta();
        let local = |u: f32, v: f32| {
            let (ax, ay) = (anchor.x as f32 + 0.5, anchor.y as f32 + 0.5);
            self.to_screen(
                origin,
                ax + u * fx as f32 + v * sx as f32,
                ay + u * fy as f32 + v * sy as f32,
            )
        };
        let (len, wide) = d.size();
        let middle = local((len as f32 - 1.0) / 2.0, (wide as f32 - 1.0) / 2.0);

        match d {
            Device::Screen { bits } => {
                let n = 1usize << bits;
                let px = screens.get(&anchor);
                let cell = (len as f32 - 1.0) / n as f32;
                for py in 0..n {
                    for pxi in 0..n {
                        let on = px.is_some_and(|p| p[py * n + pxi]);
                        // Seen from above, facing east: pixel x along the
                        // facing (u), y across it (v).
                        let (u0, v0) = (pxi as f32 * cell, py as f32 * cell);
                        let r = Rect::from_points(&[local(u0, v0), local(u0 + cell, v0 + cell)]);
                        let c = if on {
                            Color32::from_rgb(140, 255, 160)
                        } else {
                            Color32::from_rgb(25, 30, 28)
                        };
                        painter.rect_filled(r, 0.0, fade(c));
                    }
                }
            }
            Device::Display { .. } => {
                let v = compiled
                    .and_then(|c| c.devices.iter().find(|i| i.anchor == anchor))
                    .map(|i| {
                        i.inputs
                            .iter()
                            .enumerate()
                            .map(|(b, o)| {
                                let on = o.is_some_and(|o| values.get(o) == Some(&1));
                                (on as u64) << b
                            })
                            .sum::<u64>()
                    })
                    .unwrap_or(0);
                painter.text(
                    middle,
                    Align2::CENTER_CENTER,
                    format!("0x{v:X}\n{v}"),
                    FontId::monospace(14.0 * z),
                    fade(Color32::from_rgb(120, 255, 150)),
                );
            }
            _ => {
                let label = match d {
                    Device::Ram { addr, data } | Device::Rom { addr, data, .. } => {
                        format!("{}\n{}×{}", d.name(), 1u32 << addr, data)
                    }
                    _ => d.name().to_string(),
                };
                painter.text(
                    middle,
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(12.0 * z),
                    fade(Color32::WHITE),
                );
            }
        }

        // Pins: a dot per pin on its edge, lit with its value, and the
        // pin's name when zoomed in.
        let pins = device::layout(d, anchor, facing);
        let names = d.inputs().into_iter().chain(d.outputs());
        let sides = pins
            .inputs
            .iter()
            .map(|p| (p, -0.36))
            .chain(pins.outputs.iter().map(|p| (p, 0.36)));
        for ((&(cell, _), off), name) in sides.zip(names) {
            let at = self.to_screen(
                origin,
                cell.x as f32 + 0.5 + off * fx as f32,
                cell.y as f32 + 0.5 + off * fy as f32,
            );
            let c = match value_at(cell, Lane::Main) {
                Some(true) => WIRE_ON,
                _ => WIRE_OFF,
            };
            painter.circle_filled(at, 3.5 * z, fade(c));
            if z > 1.2 && !below {
                let inward = vec2(fx as f32, fy as f32) * (-off.signum() * 9.0 * z);
                painter.text(
                    at + inward,
                    Align2::CENTER_CENTER,
                    name,
                    FontId::proportional(7.0 * z),
                    Color32::from_white_alpha(170),
                );
            }
        }
    }

    pub fn refit(&mut self) {
        self.fit = true;
    }
}

/// A small drawing of `block` for the palette, in `r`.
pub fn palette_icon(painter: &egui::Painter, r: Rect, block: &Block, lit: bool) {
    let c = r.center();
    let s = r.height().min(r.width());
    let wire = if lit { WIRE_ON } else { theme::COPPER };
    let line = Stroke::new(1.6, if lit { Color32::WHITE } else { GATE_LINE });
    let trace = |a: Pos2, b: Pos2| painter.line_segment([a, b], Stroke::new(s * 0.16, wire));
    let h = s * 0.5;
    match block {
        Block::Wire | Block::Trace { .. } => {
            trace(c - vec2(h, 0.0), c + vec2(0.0, 0.0));
            trace(c, c + vec2(0.0, h));
            painter.circle_filled(c, s * 0.08, wire);
        }
        Block::Straight { .. } => {
            for dy in [-0.22, 0.22] {
                trace(c + vec2(-h, dy * s), c + vec2(h, dy * s));
            }
        }
        Block::Cross => {
            trace(c - vec2(h, 0.0), c + vec2(h, 0.0));
            trace(c - vec2(0.0, h), c + vec2(0.0, h));
            painter.circle(c, s * 0.18, BG, line);
        }
        Block::Via => {
            painter.circle_filled(c, s * 0.26, VIA);
            painter.circle_filled(c, s * 0.11, DARK);
        }
        Block::Gate { gate, .. } => {
            let symbol = match gate {
                Gate::Not => Symbol::Not,
                Gate::And => Symbol::And,
                Gate::Nand => Symbol::Nand,
                Gate::Or | Gate::Nor => Symbol::Or,
                Gate::Xor => Symbol::Xor,
            };
            let (x0, x1) = (c.x - s * 0.34, c.x + s * 0.26);
            let (y0, y1) = (c.y - s * 0.3, c.y + s * 0.3);
            let outline = gate_outline(symbol, x0, x1, y0, y1);
            if *gate == Gate::Xor {
                let pts: Vec<Pos2> = (0..=10)
                    .map(|k| {
                        let t = k as f32 / 10.0;
                        Pos2::new(
                            x0 - s * 0.08 + 4.0 * t * (1.0 - t) * s * 0.1,
                            y0 + (y1 - y0) * t,
                        )
                    })
                    .collect();
                painter.add(Shape::line(pts, line));
            }
            painter.add(Shape::closed_line(outline, line));
            if matches!(gate, Gate::Not | Gate::Nand | Gate::Nor) {
                painter.circle_stroke(Pos2::new(x1 + s * 0.06, c.y), s * 0.06, line);
            }
        }
        Block::Switch { .. } => {
            let body = Rect::from_center_size(c, vec2(s * 0.8, s * 0.42));
            painter.rect_filled(body, s * 0.21, BASE);
            painter.circle_filled(body.right_center() - vec2(s * 0.21, 0.0), s * 0.15, wire);
        }
        Block::Lamp => {
            painter.circle_filled(c, s * 0.42, LAMP_ON.gamma_multiply(0.15));
            painter.circle_filled(c, s * 0.26, LAMP_ON);
        }
        Block::Clock { .. } => {
            let w = s * 0.4;
            let (y0, y1) = (c.y + s * 0.2, c.y - s * 0.2);
            let pts = vec![
                Pos2::new(c.x - w, y0),
                Pos2::new(c.x - w / 2.0, y0),
                Pos2::new(c.x - w / 2.0, y1),
                Pos2::new(c.x + w / 2.0, y1),
                Pos2::new(c.x + w / 2.0, y0),
                Pos2::new(c.x + w, y0),
            ];
            painter.add(Shape::line(pts, Stroke::new(2.0, wire)));
        }
        Block::Port { output, .. } => {
            let body = Rect::from_center_size(c, vec2(s * 0.62, s * 0.5));
            painter.rect_filled(body, 2.0, PORT);
            let x = if *output { body.left() } else { body.right() };
            let d = if *output { -1.0 } else { 1.0 };
            painter.add(Shape::convex_polygon(
                vec![
                    Pos2::new(x + d * s * 0.2, c.y),
                    Pos2::new(x, c.y - s * 0.12),
                    Pos2::new(x, c.y + s * 0.12),
                ],
                PORT,
                Stroke::NONE,
            ));
        }
        Block::Device { device, .. } => {
            let body = Rect::from_center_size(c, vec2(s * 0.86, s * 0.62));
            painter.rect_filled(body, 2.0, Color32::from_rgb(22, 27, 25));
            painter.rect_stroke(body, 2.0, line, egui::StrokeKind::Inside);
            match device {
                Device::Screen { .. } => {
                    for k in 0..4 {
                        let x = body.left() + body.width() * (0.2 + 0.2 * k as f32);
                        painter.circle_filled(
                            Pos2::new(x, c.y),
                            s * 0.04,
                            Color32::from_rgb(140, 255, 160),
                        );
                    }
                }
                _ => {
                    // Pins along both long edges, like a DIP package.
                    for k in 0..4 {
                        let x = body.left() + body.width() * (0.2 + 0.2 * k as f32);
                        for y in [body.top(), body.bottom()] {
                            painter.line_segment(
                                [Pos2::new(x, y - s * 0.07), Pos2::new(x, y + s * 0.07)],
                                Stroke::new(1.6, VIA),
                            );
                        }
                    }
                }
            }
        }
        Block::Cover { .. } => {}
    }
}
