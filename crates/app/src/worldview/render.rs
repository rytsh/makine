//! Drawing the world in 3D: blocks become boxes in a [`Mesh`] for the GPU,
//! and names and numbers are drawn over it by egui.
//!
//! One cell is a unit cube; layer `z` sits between heights `z` and `z + 1`.
//! Only blocks near the camera are drawn, so the world can be endless.
//! Layers above the one being edited are hidden (or shown as glass), so
//! the player can always see where they build.

use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
use egui::Pos2;
use egui::{Align2, Color32, FontId, Rect};
use glam::DVec3;
use makine_world::block::{Block, Dir};
use makine_world::compile::{CompileError, Compiled, Lane};
use makine_world::device::{self, Device};
use makine_world::Pos;

use super::camera::{render, Camera};
use super::gpu::{self, Mesh, Scene, Vertex};
use super::WorldView;
use crate::worldsim::SimError;

pub(super) const BG: Color32 = Color32::from_rgb(20, 22, 27);
const GRID: Color32 = Color32::from_rgb(52, 57, 68);
const FLOOR: Color32 = Color32::from_rgba_premultiplied(9, 10, 12, 40);
const WIRE_OFF: Color32 = Color32::from_rgb(110, 60, 55);
const WIRE_ON: Color32 = Color32::from_rgb(255, 80, 60);
const GATE: Color32 = Color32::from_rgb(200, 205, 215);
const SWITCH_OFF: Color32 = Color32::from_rgb(90, 90, 100);
const BASE: Color32 = Color32::from_rgb(150, 155, 168);
const TEXT: Color32 = Color32::from_rgb(235, 238, 245);
const LAMP_OFF: Color32 = Color32::from_rgb(70, 64, 40);
const LAMP_ON: Color32 = Color32::from_rgb(255, 220, 90);
const VIA: Color32 = Color32::from_rgb(120, 170, 230);
const PORT: Color32 = Color32::from_rgb(90, 200, 140);
const ERROR: Color32 = Color32::from_rgb(255, 70, 70);
const SELECT: Color32 = Color32::from_rgb(255, 210, 80);
const PASTE: Color32 = Color32::from_rgb(120, 200, 255);

/// How far from the camera's target blocks are drawn, in cells.
fn reach(cam: &Camera) -> i32 {
    (cam.dist * 2.5).clamp(24.0, 256.0) as i32
}

fn shade(c: Color32, f: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let m = |v: u8| (v as f32 * f).min(255.0) as u8;
    Color32::from_rgba_unmultiplied(m(r), m(g), m(b), a)
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn corner(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The rectangle a device covers: (min x, min y, max x, max y) in cell
/// coordinates, edges included.
fn extent(d: &Device, anchor: Pos, facing: Dir) -> (f64, f64, f64, f64) {
    let cells = device::layout(d, anchor, facing).cells;
    let x0 = cells.iter().map(|c| c.x).min().unwrap_or(anchor.x) as f64;
    let y0 = cells.iter().map(|c| c.y).min().unwrap_or(anchor.y) as f64;
    let x1 = cells.iter().map(|c| c.x).max().unwrap_or(anchor.x) as f64 + 1.0;
    let y1 = cells.iter().map(|c| c.y).max().unwrap_or(anchor.y) as f64 + 1.0;
    (x0, y0, x1, y1)
}

/// Builds the frame's mesh, relative to the camera's origin.
struct Builder {
    mesh: Mesh,
    origin: DVec3,
}

impl Builder {
    fn vertex(&self, w: DVec3, c: Color32) -> Vertex {
        Vertex {
            pos: render(w - self.origin).as_vec3().to_array(),
            color: c.to_srgba_unmultiplied(),
        }
    }

    /// A flat four-sided face (two triangles).
    fn quad(&mut self, q: [DVec3; 4], c: Color32) {
        let v = q.map(|p| self.vertex(p, c));
        let list = if c.a() < 255 {
            &mut self.mesh.ghost
        } else {
            &mut self.mesh.solid
        };
        list.extend([v[0], v[1], v[2], v[0], v[2], v[3]]);
    }

    fn triangle(&mut self, t: [DVec3; 3], c: Color32) {
        let v = t.map(|p| self.vertex(p, c));
        let list = if c.a() < 255 {
            &mut self.mesh.ghost
        } else {
            &mut self.mesh.solid
        };
        list.extend(v);
    }

    /// A thin bar from `a` to `b` (along one axis). Bars instead of GPU
    /// lines: lines are one pixel wide and some drivers clip them badly.
    fn line(&mut self, a: DVec3, b: DVec3, c: Color32) {
        let w = 0.025;
        self.cuboid(a.min(b) - DVec3::splat(w), a.max(b) + DVec3::splat(w), c);
    }

    /// A flat line on the floor at height `z`.
    fn floor_line(&mut self, a: DVec3, b: DVec3, c: Color32) {
        let w = 0.02;
        let (lo, hi) = (a.min(b) - DVec3::splat(w), a.max(b) + DVec3::splat(w));
        let z = a.z;
        self.quad(
            [
                corner(lo.x, lo.y, z),
                corner(hi.x, lo.y, z),
                corner(hi.x, hi.y, z),
                corner(lo.x, hi.y, z),
            ],
            c,
        );
    }

    /// A box from `lo` to `hi`, its sides shaded so its shape reads.
    fn cuboid(&mut self, lo: DVec3, hi: DVec3, c: Color32) {
        let (a, b) = (lo, hi);
        let p = |x: bool, y: bool, z: bool| {
            corner(
                if x { b.x } else { a.x },
                if y { b.y } else { a.y },
                if z { b.z } else { a.z },
            )
        };
        let (t, f) = (true, false);
        self.quad([p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t)], c);
        self.quad(
            [p(f, f, f), p(t, f, f), p(t, t, f), p(f, t, f)],
            shade(c, 0.45),
        );
        self.quad(
            [p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t)],
            shade(c, 0.78),
        );
        self.quad(
            [p(f, f, f), p(f, t, f), p(f, t, t), p(f, f, t)],
            shade(c, 0.78),
        );
        self.quad(
            [p(f, t, f), p(t, t, f), p(t, t, t), p(f, t, t)],
            shade(c, 0.62),
        );
        self.quad(
            [p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t)],
            shade(c, 0.62),
        );
    }

    /// The edges of a box.
    fn wire_box(&mut self, lo: DVec3, hi: DVec3, c: Color32) {
        let (a, b) = (lo, hi);
        for z in [a.z, b.z] {
            let q = [
                corner(a.x, a.y, z),
                corner(b.x, a.y, z),
                corner(b.x, b.y, z),
                corner(a.x, b.y, z),
            ];
            for i in 0..4 {
                self.line(q[i], q[(i + 1) % 4], c);
            }
        }
        for (x, y) in [(a.x, a.y), (b.x, a.y), (b.x, b.y), (a.x, b.y)] {
            self.line(corner(x, y, a.z), corner(x, y, b.z), c);
        }
    }

    /// A box inside cell `p`, `h` tall, inset by `m` on each side.
    fn block(&mut self, p: Pos, m: f64, h: f64, c: Color32) {
        let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
        self.cuboid(
            corner(x + m, y + m, z),
            corner(x + 1.0 - m, y + 1.0 - m, z + h),
            c,
        );
    }

    /// An arrow lying on top of cell `p` at height `h`, pointing `d`.
    fn arrow(&mut self, p: Pos, h: f64, d: Dir, c: Color32) {
        let (dx, dy) = d.delta();
        let (dx, dy) = (dx as f64, dy as f64);
        let (cx, cy, z) = (p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + h + 0.01);
        let at = |u: f64, v: f64| corner(cx + dx * u - dy * v, cy + dy * u + dx * v, z);
        self.triangle([at(0.40, 0.0), at(0.16, 0.18), at(0.16, -0.18)], c);
    }
}

/// Text to draw over the scene at a world point, sized in cells.
struct Label {
    at: DVec3,
    text: String,
    size: f64,
    color: Color32,
    monospace: bool,
}

impl WorldView {
    /// Centre the camera on the world (or the current layer if it is
    /// empty).
    pub(super) fn fit_camera(&mut self) {
        match self.world.bounds() {
            Some((lo, hi)) => {
                let lo = corner(lo.x as f64, lo.y as f64, lo.z as f64);
                let hi = corner(hi.x as f64 + 1.0, hi.y as f64 + 1.0, hi.z as f64 + 1.0);
                self.camera.fit(lo, hi);
                self.camera.target.z = self.layer as f64;
            }
            None => {
                self.camera = Camera {
                    target: corner(0.0, 0.0, self.layer as f64),
                    ..Camera::default()
                };
            }
        }
    }

    /// Fit the camera to the world at the next frame.
    pub fn refit(&mut self) {
        self.fit = true;
    }

    /// Where the centre of `cell`'s floor is on screen (last frame's view).
    #[cfg(test)]
    pub(super) fn cell_on_screen(&self, cell: Pos) -> Pos2 {
        self.camera.cell_center(self.rect, cell).unwrap()
    }

    pub(super) fn draw(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
        let cam = &self.camera;
        let mut b = Builder {
            mesh: Mesh::default(),
            origin: cam.origin(),
        };
        let mut labels = Vec::new();
        self.build_floor(&mut b);
        self.build_blocks(&mut b, &mut labels, screens);
        self.build_overlays(&mut b, &mut labels);

        let ppp = painter.ctx().pixels_per_point();
        let size = [
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        ];
        Scene {
            mesh: Arc::new(b.mesh),
            view_proj: cam.view_proj((rect.width() / rect.height()) as f64),
            eye: cam.eye_rel(),
            fog: reach(cam) as f32 * 1.2,
            background: BG,
            size,
            target: gpu::target_format(painter.ctx()),
        }
        .paint(painter, rect);

        // Names and numbers over the scene, scaled with distance.
        let px_per_cell =
            |w: DVec3| rect.height() as f64 / (0.83 * (cam.eye() - w).length().max(0.1));
        for l in labels {
            let px = (l.size * px_per_cell(l.at)) as f32;
            if px < 7.0 {
                continue;
            }
            if let Some(at) = cam.project(rect, l.at).filter(|p| rect.contains(*p)) {
                let font = if l.monospace {
                    FontId::monospace(px.min(40.0))
                } else {
                    FontId::proportional(px.min(40.0))
                };
                painter.text(at, Align2::CENTER_CENTER, l.text, font, l.color);
            }
        }
    }

    /// Grid lines on the layer being edited, and a faint glass floor that
    /// shows what lies below it.
    fn build_floor(&self, b: &mut Builder) {
        let cam = &self.camera;
        let r = reach(cam).min(48) as f64;
        let z = self.layer as f64 + 0.002;
        let (cx, cy) = (cam.target.x.floor(), cam.target.y.floor());
        let (x0, x1, y0, y1) = (cx - r, cx + r, cy - r, cy + r);
        for i in 0..=(2.0 * r) as i32 {
            let k = i as f64;
            b.floor_line(corner(x0 + k, y0, z), corner(x0 + k, y1, z), GRID);
            b.floor_line(corner(x0, y0 + k, z), corner(x1, y0 + k, z), GRID);
        }
        if self.world.bounds().is_some_and(|(lo, _)| lo.z < self.layer) {
            b.quad(
                [
                    corner(x0, y0, z - 0.001),
                    corner(x1, y0, z - 0.001),
                    corner(x1, y1, z - 0.001),
                    corner(x0, y1, z - 0.001),
                ],
                FLOOR,
            );
        }
    }

    /// Every block near the camera.
    fn build_blocks(
        &self,
        b: &mut Builder,
        labels: &mut Vec<Label>,
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
        let cam = &self.camera;
        let compiled = self.sim.compiled();
        let values = self.sim.values();
        let value_at = |p: Pos, lane: Lane| -> Option<bool> {
            let o = compiled?.output_at(p, lane)?;
            self.sim.output(o)
        };
        let lit = |p: Pos, lane: Lane| {
            if value_at(p, lane) == Some(true) {
                WIRE_ON
            } else {
                WIRE_OFF
            }
        };
        let r = reach(cam);
        let (cx, cy) = (cam.target.x.floor() as i32, cam.target.y.floor() as i32);
        let top = if self.show_above {
            i32::MAX - 1
        } else {
            self.layer
        };
        let lo = Pos::new(cx - r, cy - r, (self.layer - r).min(top));
        let hi = Pos::new(cx + r, cy + r, top.min(self.layer + r));
        let near = cam.dist < 45.0;

        for (p, block) in self.world.blocks_in(lo, hi) {
            // Above the edited layer: glass. Below: a little darker.
            let tint = |c: Color32| match p.z.cmp(&self.layer) {
                std::cmp::Ordering::Greater => with_alpha(c, 60),
                std::cmp::Ordering::Less => shade(c, 0.8),
                std::cmp::Ordering::Equal => c,
            };
            let labelled = near && p.z == self.layer;
            let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
            match block {
                Block::Wire | Block::Straight { .. } | Block::Cross | Block::Via => {
                    let t = 0.11;
                    let h = 0.1;
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
                        // Crossing wires: the second lane runs over the first.
                        let lift = if lane == Lane::Cross { 0.12 } else { 0.0 };
                        let (dx, dy) = d.delta();
                        let (ex, ey) = (x + 0.5 + dx as f64 * 0.5, y + 0.5 + dy as f64 * 0.5);
                        let (ax, bx) = (ex.min(x + 0.5 - t), ex.max(x + 0.5 + t));
                        let (ay, by) = (ey.min(y + 0.5 - t), ey.max(y + 0.5 + t));
                        b.cuboid(
                            corner(ax, ay, z + lift),
                            corner(bx, by, z + h + lift),
                            tint(lit(p, lane)),
                        );
                    }
                    match block {
                        Block::Via => {
                            // A post; it reaches the via above, if any.
                            let up =
                                matches!(self.world.get(p.up()), Some(Block::Via)) && (p.z < top);
                            b.block(p, 0.32, if up { 1.0 } else { 0.35 }, tint(VIA));
                        }
                        Block::Cross => b.block(p, 0.36, 0.26, tint(BASE)),
                        _ => b.block(p, 0.5 - t - 0.02, h + 0.02, tint(lit(p, Lane::Main))),
                    }
                }
                Block::Gate { gate, facing } => {
                    b.block(p, 0.1, 0.6, tint(GATE));
                    let out = match value_at(p, Lane::Main) {
                        Some(true) => WIRE_ON,
                        _ => Color32::from_rgb(60, 64, 74),
                    };
                    b.arrow(p, 0.6, *facing, tint(out));
                    if labelled {
                        labels.push(Label {
                            at: corner(x + 0.5, y + 0.5, z + 0.6),
                            text: gate.name().into(),
                            size: 0.24,
                            color: Color32::from_rgb(30, 32, 38),
                            monospace: false,
                        });
                    }
                }
                Block::Switch { on, facing } => {
                    b.block(p, 0.1, 0.15, tint(BASE));
                    b.block(p, 0.22, 0.3, tint(if *on { WIRE_ON } else { SWITCH_OFF }));
                    b.arrow(p, 0.3, *facing, tint(Color32::WHITE));
                }
                Block::Lamp => {
                    let on = self.sim.lamp_lit(p);
                    b.block(p, 0.12, 0.75, tint(if on { LAMP_ON } else { LAMP_OFF }));
                    if on && p.z <= self.layer {
                        b.block(p, -0.08, 0.95, with_alpha(LAMP_ON, 45));
                    }
                }
                Block::Clock { facing, .. } => {
                    let on = value_at(p, Lane::Main) == Some(true);
                    let c = if on {
                        Color32::from_rgb(230, 160, 80)
                    } else {
                        Color32::from_rgb(120, 90, 60)
                    };
                    b.block(p, 0.1, 0.5, tint(c));
                    b.arrow(p, 0.5, *facing, tint(Color32::WHITE));
                }
                Block::Port { name, facing, .. } => {
                    b.block(p, 0.1, 0.4, tint(PORT));
                    b.arrow(p, 0.4, *facing, tint(Color32::from_rgb(30, 32, 38)));
                    if labelled {
                        labels.push(Label {
                            at: corner(x + 0.5, y + 0.5, z + 0.4),
                            text: name.clone(),
                            size: 0.26,
                            color: Color32::from_rgb(20, 40, 30),
                            monospace: false,
                        });
                    }
                }
                Block::Device { device, facing } => {
                    self.build_device(
                        b, labels, p, device, *facing, &tint, &lit, labelled, compiled, values,
                        screens,
                    );
                }
                Block::Cover { .. } => {}
            }
        }
    }

    /// A device: one box over all its cells, pins along its edges and its
    /// face (screen pixels, a number, a name) on top.
    #[allow(clippy::too_many_arguments)]
    fn build_device(
        &self,
        b: &mut Builder,
        labels: &mut Vec<Label>,
        anchor: Pos,
        d: &Device,
        facing: Dir,
        tint: &dyn Fn(Color32) -> Color32,
        lit: &dyn Fn(Pos, Lane) -> Color32,
        labelled: bool,
        compiled: Option<&Compiled>,
        values: &[u64],
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
        let (x0, y0, x1, y1) = extent(d, anchor, facing);
        let h = 0.5;
        let z = anchor.z as f64;
        let body = match d {
            Device::Ram { .. } => Color32::from_rgb(70, 110, 170),
            Device::Rom { .. } => Color32::from_rgb(120, 90, 160),
            Device::Screen { .. } => Color32::from_rgb(40, 44, 52),
            Device::Keyboard => Color32::from_rgb(150, 150, 160),
            Device::Display { .. } => Color32::from_rgb(40, 60, 50),
            Device::Chip { .. } => Color32::from_rgb(55, 58, 66),
        };
        b.cuboid(
            corner(x0 + 0.08, y0 + 0.08, z),
            corner(x1 - 0.08, y1 - 0.08, z + h),
            tint(body),
        );
        let top = z + h + 0.005;

        // Local axes: u along the facing, v across (to its right).
        let (fx, fy) = facing.delta();
        let (sx, sy) = facing.cw().delta();
        let local = |u: f64, v: f64| {
            let (ax, ay) = (anchor.x as f64 + 0.5, anchor.y as f64 + 0.5);
            corner(
                ax + u * fx as f64 + v * sx as f64,
                ay + u * fy as f64 + v * sy as f64,
                top,
            )
        };
        let (len, wide) = d.size();
        let middle = local((len as f64 - 1.0) / 2.0, (wide as f64 - 1.0) / 2.0);

        match d {
            Device::Screen { bits } => {
                let n = 1usize << bits;
                let px = screens.get(&anchor);
                // The screen fills the top, minus a border.
                let cell = (len as f64 - 1.0) / n as f64;
                for py in 0..n {
                    for pxi in 0..n {
                        let on = px.is_some_and(|p| p[py * n + pxi]);
                        // Pixel x runs across (v), y along the facing (u).
                        let (u0, v0) = (py as f64 * cell, pxi as f64 * cell);
                        let c = if on {
                            Color32::from_rgb(140, 255, 160)
                        } else {
                            Color32::from_rgb(25, 30, 28)
                        };
                        b.quad(
                            [
                                local(u0, v0),
                                local(u0 + cell, v0),
                                local(u0 + cell, v0 + cell),
                                local(u0, v0 + cell),
                            ],
                            tint(c),
                        );
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
                            .map(|(bit, o)| {
                                let on = o.is_some_and(|o| values.get(o) == Some(&1));
                                (on as u64) << bit
                            })
                            .sum::<u64>()
                    })
                    .unwrap_or(0);
                if labelled {
                    labels.push(Label {
                        at: middle,
                        text: format!("0x{v:X}\n{v}"),
                        size: 0.42,
                        color: Color32::from_rgb(120, 255, 150),
                        monospace: true,
                    });
                }
            }
            _ => {
                if labelled {
                    let text = match d {
                        Device::Ram { addr, data } | Device::Rom { addr, data, .. } => {
                            format!("{}\n{}×{}", d.name(), 1u32 << addr, data)
                        }
                        _ => d.name().to_string(),
                    };
                    labels.push(Label {
                        at: middle,
                        text,
                        size: 0.36,
                        color: TEXT,
                        monospace: false,
                    });
                }
            }
        }

        // Pins: a small block per pin on the edge, lit with its value.
        let pins = device::layout(d, anchor, facing);
        let sides = pins
            .inputs
            .iter()
            .map(|p| (p, -0.36))
            .chain(pins.outputs.iter().map(|p| (p, 0.36)));
        for (&(cell, _), off) in sides {
            let (x, y) = (
                cell.x as f64 + 0.5 + off * fx as f64,
                cell.y as f64 + 0.5 + off * fy as f64,
            );
            b.cuboid(
                corner(x - 0.09, y - 0.09, top - 0.01),
                corner(x + 0.09, y + 0.09, top + 0.06),
                tint(lit(cell, Lane::Main)),
            );
        }
    }

    /// The cursor, the selection, blocks being pasted, other players and
    /// short circuits.
    fn build_overlays(&self, b: &mut Builder, labels: &mut Vec<Label>) {
        let cell_box = |p: Pos, grow: f64, h: f64| {
            (
                corner(p.x as f64 - grow, p.y as f64 - grow, p.z as f64),
                corner(
                    p.x as f64 + 1.0 + grow,
                    p.y as f64 + 1.0 + grow,
                    p.z as f64 + h,
                ),
            )
        };
        // The block aimed at, outlined; the cell a block would go into is
        // shown below.
        if let Some(t) = self.target.filter(|t| Some(*t) != self.hover) {
            let (lo, hi) = cell_box(t, 0.02, 1.0);
            b.wire_box(lo, hi, Color32::from_white_alpha(90));
        }
        if let Some(cell) = self.hover.filter(|_| self.pasting.is_none()) {
            let (lo, hi) = cell_box(cell, 0.0, 1.0);
            b.wire_box(lo, hi, Color32::from_white_alpha(200));
            b.quad(
                [
                    corner(lo.x, lo.y, lo.z + 0.004),
                    corner(hi.x, lo.y, lo.z + 0.004),
                    corner(hi.x, hi.y, lo.z + 0.004),
                    corner(lo.x, hi.y, lo.z + 0.004),
                ],
                Color32::from_white_alpha(28),
            );
            if self
                .brush
                .as_ref()
                .is_some_and(|b| b.block.facing().is_some())
            {
                b.arrow(cell, 0.0, self.facing, Color32::from_white_alpha(150));
            }
        }

        // The selection spans every layer of its rectangle: show it from the
        // lowest block up to the edited layer.
        if let Some(r) = self.selection {
            let bottom = r
                .blocks(&self.world)
                .iter()
                .map(|(p, _)| p.z)
                .min()
                .unwrap_or(self.layer)
                .min(self.layer);
            let lo = corner(r.x0 as f64, r.y0 as f64, bottom as f64);
            let hi = corner(
                r.x1 as f64 + 1.0,
                r.y1 as f64 + 1.0,
                self.layer as f64 + 1.0,
            );
            b.cuboid(lo, hi, with_alpha(SELECT, 30));
            b.wire_box(lo, hi, SELECT);
        }

        if let (Some(clip), Some(at)) = (&self.pasting, self.hover) {
            for (p, _) in &clip.blocks {
                let cell = Pos::new(at.x + p.x, at.y + p.y, at.z + p.z);
                let (lo, hi) = cell_box(cell, -0.05, 0.6);
                b.cuboid(lo, hi, with_alpha(PASTE, 70));
            }
        }

        for (name, color, p) in &self.others {
            let (lo, hi) = cell_box(*p, 0.02, 1.0);
            b.cuboid(lo, hi, with_alpha(*color, 50));
            b.wire_box(lo, hi, *color);
            labels.push(Label {
                at: corner(p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 1.4),
                text: name.clone(),
                size: 0.4,
                color: *color,
                monospace: false,
            });
        }

        if let Some((anchor, d, f)) = self.selected_device() {
            let (x0, y0, x1, y1) = extent(&d, anchor, f);
            let z = anchor.z as f64;
            b.wire_box(
                corner(x0, y0, z),
                corner(x1, y1, z + 0.62),
                Color32::from_rgb(120, 200, 255),
            );
        }

        if let Some(SimError::Compile(CompileError::ShortCircuit(ps))) = self.sim.error() {
            for p in ps {
                let (lo, hi) = cell_box(p, 0.08, 1.1);
                b.cuboid(lo, hi, with_alpha(ERROR, 80));
                b.wire_box(lo, hi, ERROR);
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
}
