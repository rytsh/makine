//! Drawing the world: the isometric projection, blocks, devices and
//! overlays (selection, pasting, other players, short circuits).

use std::collections::HashMap;

use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke};
use makine_world::block::{Block, Dir};
use makine_world::compile::{CompileError, Compiled, Lane};
use makine_world::device::{self, Device};
use makine_world::Pos;

use super::WorldView;
use crate::worldsim::SimError;

/// Size of one cell on screen at zoom 1: half the tile's width and height.
const TW: f32 = 28.0;
const TH: f32 = 14.0;
/// Height of one layer on screen at zoom 1.
const LAYER_H: f32 = 22.0;

pub(super) const BG: Color32 = Color32::from_rgb(20, 22, 27);
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

impl WorldView {
    /// Centre and zoom the camera so the whole world fits in `rect`.
    pub(super) fn fit_to(&mut self, rect: Rect, origin: Pos2) {
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

    pub(super) fn to_screen(&self, origin: Pos2, x: f32, y: f32, z: f32) -> Pos2 {
        let z = z * self.zoom;
        origin
            + self.pan
            + vec2(
                (x - y) * TW * self.zoom,
                (x + y) * TH * self.zoom - z * LAYER_H,
            )
    }

    /// The cell on `layer` under a screen point.
    pub(super) fn pick(&self, origin: Pos2, p: Pos2, layer: i32) -> Pos {
        let v = (p - origin - self.pan) / self.zoom;
        let vy = v.y + layer as f32 * LAYER_H;
        // Invert: sx = (x - y) TW, sy = (x + y) TH.
        let a = v.x / TW;
        let b = vy / TH;
        let x = ((a + b) / 2.0).floor() as i32;
        let y = ((b - a) / 2.0).floor() as i32;
        Pos::new(x, y, layer)
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
        let key = |p: &Pos, b: &Block| match b {
            Block::Device { device, facing } => {
                let cells = device::layout(device, *p, *facing).cells;
                let far = cells
                    .iter()
                    .max_by_key(|c| (c.x + c.y, c.x))
                    .copied()
                    .unwrap_or(*p);
                (far.z, far.x + far.y, far.x)
            }
            _ => (p.z, p.x + p.y, p.x),
        };
        blocks.sort_by_key(|(p, b)| key(p, b));
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
                Block::Wire | Block::Straight { .. } | Block::Cross | Block::Via => {
                    let lane_color = |lane| match value_at(p, lane) {
                        Some(true) => WIRE_ON,
                        _ => WIRE_OFF,
                    };
                    let w = 4.0 * z;
                    // Connections to neighbours that accept a wire.
                    for d in Dir::ALL {
                        if self.world.accepts_wire(p, d)
                            && self.world.accepts_wire(p.step(d), d.opposite())
                        {
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
                    let lit = self.sim.lamp_lit(p);
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
                Block::Device { device, facing } => {
                    self.draw_device(
                        painter, origin, p, device, *facing, dim, &value_at, compiled, values,
                        screens,
                    );
                }
                Block::Cover { .. } => {}
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

        // Selection rectangle on the current layer.
        if let Some(r) = self.selection {
            let lz = self.layer as f32;
            let (x0, y0, x1, y1) = (
                r.x0 as f32,
                r.y0 as f32,
                r.x1 as f32 + 1.0,
                r.y1 as f32 + 1.0,
            );
            let pts = vec![
                self.to_screen(origin, x0, y0, lz),
                self.to_screen(origin, x1, y0, lz),
                self.to_screen(origin, x1, y1, lz),
                self.to_screen(origin, x0, y1, lz),
            ];
            painter.add(Shape::convex_polygon(
                pts.clone(),
                Color32::from_rgba_unmultiplied(255, 210, 80, 24),
                Stroke::NONE,
            ));
            painter.add(Shape::closed_line(
                pts,
                Stroke::new(2.0, Color32::from_rgb(255, 210, 80)),
            ));
        }
        // Blocks being pasted, as ghosts under the pointer.
        if let (Some(clip), Some(at)) = (&self.pasting, self.hover) {
            for (p, _) in &clip.blocks {
                let (x, y, zz) = (
                    (at.x + p.x) as f32,
                    (at.y + p.y) as f32,
                    (at.z + p.z) as f32,
                );
                let tile = vec![
                    self.to_screen(origin, x, y, zz),
                    self.to_screen(origin, x + 1.0, y, zz),
                    self.to_screen(origin, x + 1.0, y + 1.0, zz),
                    self.to_screen(origin, x, y + 1.0, zz),
                ];
                painter.add(Shape::convex_polygon(
                    tile,
                    Color32::from_rgba_unmultiplied(120, 200, 255, 60),
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 200, 255, 160)),
                ));
            }
        }

        // Other players' cursors, with their names.
        for (name, color, p) in &self.others {
            let (x, y, zz) = (p.x as f32, p.y as f32, p.z as f32);
            let tile = vec![
                self.to_screen(origin, x, y, zz),
                self.to_screen(origin, x + 1.0, y, zz),
                self.to_screen(origin, x + 1.0, y + 1.0, zz),
                self.to_screen(origin, x, y + 1.0, zz),
            ];
            painter.add(Shape::closed_line(tile, Stroke::new(2.5, *color)));
            painter.text(
                self.to_screen(origin, x + 0.5, y + 0.5, zz + 1.2),
                Align2::CENTER_BOTTOM,
                name,
                FontId::proportional(12.0),
                *color,
            );
        }

        // The selected device.
        if let Some((anchor, d, f)) = self.selected_device() {
            let (x0, y0, x1, y1) = extent(&d, anchor, f);
            let z = anchor.z as f32;
            let outline = [
                self.to_screen(origin, x0, y0, z),
                self.to_screen(origin, x1, y0, z),
                self.to_screen(origin, x1, y1, z),
                self.to_screen(origin, x0, y1, z),
            ];
            painter.add(Shape::closed_line(
                outline.to_vec(),
                Stroke::new(2.5, Color32::from_rgb(120, 200, 255)),
            ));
        }

        // Short circuits: mark the drivers.
        if let Some(SimError::Compile(CompileError::ShortCircuit(ps))) = self.sim.error() {
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
        let at = [p.x as f32 + 0.1, p.y as f32 + 0.1, p.z as f32];
        self.cuboid(painter, origin, at, [0.8, 0.8, h], top, left, right);
    }

    /// A box with its low corner at `at` and size `size` (x, y, height).
    #[allow(clippy::too_many_arguments)]
    fn cuboid(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        at: [f32; 3],
        size: [f32; 3],
        top: Color32,
        left: Color32,
        right: Color32,
    ) {
        let [x, y, z] = at;
        let [sx, sy, h] = size;
        let at = |dx: f32, dy: f32, dz: f32| self.to_screen(origin, x + dx, y + dy, z + dz);
        // Visible sides: +y face (left) and +x face (right).
        painter.add(Shape::convex_polygon(
            vec![
                at(0.0, sy, 0.0),
                at(sx, sy, 0.0),
                at(sx, sy, h),
                at(0.0, sy, h),
            ],
            left,
            Stroke::NONE,
        ));
        painter.add(Shape::convex_polygon(
            vec![
                at(sx, 0.0, 0.0),
                at(sx, sy, 0.0),
                at(sx, sy, h),
                at(sx, 0.0, h),
            ],
            right,
            Stroke::NONE,
        ));
        painter.add(Shape::convex_polygon(
            vec![
                at(0.0, 0.0, h),
                at(sx, 0.0, h),
                at(sx, sy, h),
                at(0.0, sy, h),
            ],
            top,
            Stroke::new(1.0, Color32::from_black_alpha(80)),
        ));
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

    /// A device: one box over all its cells, with pins along its edges and
    /// its face (screen pixels, a number, a name) on top.
    #[allow(clippy::too_many_arguments)]
    fn draw_device(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        anchor: Pos,
        d: &Device,
        facing: Dir,
        dim: bool,
        value_at: &dyn Fn(Pos, Lane) -> Option<bool>,
        compiled: Option<&Compiled>,
        values: &[u64],
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
        let fade = |c: Color32| if dim { c.gamma_multiply(0.35) } else { c };
        let z = self.zoom;
        let (x0, y0, x1, y1) = extent(d, anchor, facing);
        let h = 0.5;
        let lz = anchor.z as f32;
        let body = match d {
            Device::Ram { .. } => Color32::from_rgb(70, 110, 170),
            Device::Rom { .. } => Color32::from_rgb(120, 90, 160),
            Device::Screen { .. } => Color32::from_rgb(40, 44, 52),
            Device::Keyboard => Color32::from_rgb(150, 150, 160),
            Device::Display { .. } => Color32::from_rgb(40, 60, 50),
            Device::Chip { .. } => Color32::from_rgb(55, 58, 66),
        };
        self.cuboid(
            painter,
            origin,
            [x0 + 0.08, y0 + 0.08, lz],
            [x1 - x0 - 0.16, y1 - y0 - 0.16, h],
            fade(body),
            fade(body.gamma_multiply(0.75)),
            fade(body.gamma_multiply(0.55)),
        );
        let top = |x: f32, y: f32| self.to_screen(origin, x, y, lz + h);

        // Local axes: u along the facing, v across (to its right).
        let (fx, fy) = facing.delta();
        let (sx, sy) = facing.cw().delta();
        let local = |u: f32, v: f32| {
            let (ax, ay) = (anchor.x as f32 + 0.5, anchor.y as f32 + 0.5);
            (
                ax + u * fx as f32 + v * sx as f32,
                ay + u * fy as f32 + v * sy as f32,
            )
        };
        let (len, wide) = d.size();

        match d {
            Device::Screen { bits } => {
                let n = 1usize << bits;
                let px = screens.get(&anchor);
                // The screen fills the top, minus a border.
                let span = len as f32 - 1.0;
                let cell = span / n as f32;
                for py in 0..n {
                    for pxi in 0..n {
                        let on = px.is_some_and(|p| p[py * n + pxi]);
                        if !on && z < 0.6 {
                            continue;
                        }
                        // Pixel x runs across (v), y along the facing (u).
                        let (u0, v0) = (py as f32 * cell, pxi as f32 * cell);
                        let corners = [
                            local(u0 - 0.0, v0),
                            local(u0 + cell, v0),
                            local(u0 + cell, v0 + cell),
                            local(u0, v0 + cell),
                        ]
                        .map(|(x, y)| top(x, y));
                        let c = if on {
                            Color32::from_rgb(140, 255, 160)
                        } else {
                            Color32::from_rgb(25, 30, 28)
                        };
                        painter.add(Shape::convex_polygon(
                            corners.to_vec(),
                            fade(c),
                            Stroke::NONE,
                        ));
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
                let (cx, cy) = local((len as f32 - 1.0) / 2.0, (wide as f32 - 1.0) / 2.0);
                painter.text(
                    top(cx, cy),
                    Align2::CENTER_CENTER,
                    format!("0x{v:X}\n{v}"),
                    FontId::monospace(13.0 * z),
                    fade(Color32::from_rgb(120, 255, 150)),
                );
            }
            _ => {
                let (cx, cy) = local((len as f32 - 1.0) / 2.0, (wide as f32 - 1.0) / 2.0);
                let label = match d {
                    Device::Ram { addr, data } | Device::Rom { addr, data, .. } => {
                        format!("{}\n{}×{}", d.name(), 1u32 << addr, data)
                    }
                    _ => d.name().to_string(),
                };
                painter.text(
                    top(cx, cy),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(11.0 * z),
                    fade(Color32::WHITE),
                );
            }
        }

        // Pins: a dot per pin on the top edge, lit with its value, and the
        // pin's name when zoomed in.
        let pins = device::layout(d, anchor, facing);
        let names = d.inputs().into_iter().chain(d.outputs());
        let sides = pins
            .inputs
            .iter()
            .map(|p| (p, -0.32))
            .chain(pins.outputs.iter().map(|p| (p, 0.32)));
        for ((&(cell, _), off), name) in sides.zip(names) {
            let (x, y) = (
                cell.x as f32 + 0.5 + off * fx as f32,
                cell.y as f32 + 0.5 + off * fy as f32,
            );
            let c = match value_at(cell, Lane::Main) {
                Some(true) => WIRE_ON,
                _ => WIRE_OFF,
            };
            painter.circle_filled(top(x, y), 3.0 * z, fade(c));
            if z > 1.3 && !dim {
                let (x, y) = (x - off * 0.6 * fx as f32, y - off * 0.6 * fy as f32);
                painter.text(
                    top(x, y),
                    Align2::CENTER_CENTER,
                    name,
                    FontId::proportional(7.0 * z),
                    Color32::from_white_alpha(170),
                );
            }
        }
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
