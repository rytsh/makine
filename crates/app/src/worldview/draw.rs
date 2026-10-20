//! Painting the world: the board, wires (coloured by their values), parts
//! and what the player is doing.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind};
use makine_world::{device, Device, End, Kind};

use super::layout::{Face, Layout, Shape, SCREEN_PX};
use super::{Drag, Hover, WorldView};
use crate::canvas::draw::{
    bezier, draw_box, draw_gate, pin_dot, wire_color, BG, BUS, DIM, ERR, GRID, PART_FILL, TEXT,
    WIRE_HOVER,
};
use crate::canvas::format_value;
use crate::design::Symbol;
use crate::theme;

impl WorldView {
    pub(super) fn draw(&self, painter: &egui::Painter, board: Rect, l: &Layout, hover: &Hover) {
        self.draw_board(painter, board, l);
        if let Some(r) = self.scope_frame(l) {
            painter.rect_stroke(
                r,
                8.0,
                Stroke::new(1.0, theme::COPPER.gamma_multiply(0.5)),
                StrokeKind::Outside,
            );
        }
        for m in &l.modules {
            let r = l.rect(m.rect);
            if !board.intersects(r) {
                continue;
            }
            if !m.closed {
                painter.rect_filled(r, 6.0, Color32::from_black_alpha(24));
            }
            painter.rect_stroke(r, 6.0, Stroke::new(1.0, theme::COPPER), StrokeKind::Inside);
            let head = Rect::from_min_size(r.min, vec2(r.width(), 28.0 * self.zoom));
            painter.rect_filled(head, 4.0, theme::RAISED);
            painter.text(
                head.left_center() + vec2(10.0 * self.zoom, 0.0),
                Align2::LEFT_CENTER,
                format!("{} {}", if m.closed { "+" } else { "−" }, m.name),
                FontId::proportional((14.0 * self.zoom).max(9.0)),
                theme::COPPER_BRIGHT,
            );
        }
        self.draw_wires(painter, l, hover);
        for (id, at, shape) in &l.parts {
            if !l.visible(*id) {
                continue;
            }
            if l.part_rect(*id)
                .is_some_and(|r| !board.intersects(l.rect(r).expand(8.0)))
            {
                continue;
            }
            let Some(p) = self.world.part(*id) else {
                continue;
            };
            let selected = self.selected.contains(id);
            let hl = selected || hover.part == Some(*id);
            if self.zoom < 0.35 {
                if let Some(r) = l.part_rect(*id) {
                    painter.rect_filled(l.rect(r), 1.0, theme::RAISED_HOVER);
                }
                continue;
            }
            match shape {
                Some(s) => self.draw_part(painter, l, *id, &p.kind, *at, s, hl, hover),
                None => {
                    // A chip missing from the library.
                    let r = l.rect(Rect::from_min_size(*at, vec2(120.0, 40.0)));
                    painter.rect_filled(r, 4.0, ERR.gamma_multiply(0.6));
                    painter.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        format!("{}\n(yok)", super::label(&p.kind)),
                        FontId::proportional(12.0 * self.zoom),
                        TEXT,
                    );
                }
            }
            if selected {
                if let Some(r) = l.part_rect(*id) {
                    painter.rect_stroke(
                        l.rect(r).expand(4.0),
                        5.0,
                        Stroke::new(1.5, theme::COPPER),
                        StrokeKind::Outside,
                    );
                }
            }
        }
        for m in l.modules.iter().filter(|m| m.closed) {
            for (output, list) in [(false, &m.inputs), (true, &m.outputs)] {
                for (end, name, width) in list {
                    let p = if output {
                        l.source_pos(*end)
                    } else {
                        l.sink_pos(*end)
                    };
                    if let Some(p) = p {
                        let value = if output {
                            self.sim.value(*end)
                        } else {
                            self.sim.input_value(&self.world, *end)
                        };
                        let col = wire_color(*width, value);
                        painter.circle_filled(p, 4.0 * self.zoom, col);
                        if self.zoom > 0.45 {
                            let text = format!(
                                "{} {}",
                                name,
                                value.map_or("?".into(), |v| format!("{v:X}"))
                            );
                            painter.text(
                                p + vec2(if output { -9.0 } else { 9.0 } * self.zoom, 0.0),
                                if output {
                                    Align2::RIGHT_CENTER
                                } else {
                                    Align2::LEFT_CENTER
                                },
                                text,
                                FontId::proportional(10.0 * self.zoom),
                                theme::INK_DIM,
                            );
                        }
                    }
                }
            }
        }
        self.draw_ports(painter, l);
        self.draw_overlays(painter, l);
    }

    /// The edge of the module looked into, around its insides (not the
    /// ports), on screen.
    fn scope_frame(&self, l: &Layout) -> Option<Rect> {
        if l.ports.is_empty() && self.scope.is_empty() {
            return None;
        }
        let inner = l
            .parts
            .iter()
            .filter(|(id, _, _)| l.visible(*id))
            .filter_map(|(id, _, _)| l.part_rect(*id))
            .chain(l.modules.iter().map(|m| m.rect))
            .reduce(|a, b| a.union(b))?;
        Some(l.rect(inner.expand(60.0)))
    }

    /// The connections of the module looked into to the outside: tabs on
    /// its left (coming in) and right (going out) with the net's value.
    fn draw_ports(&self, painter: &egui::Painter, l: &Layout) {
        let z = self.zoom;
        for p in &l.ports {
            let r = l.rect(p.rect());
            if !painter.clip_rect().intersects(r) {
                continue;
            }
            let value = self.sim.value(p.source);
            let col = wire_color(p.width, value);
            painter.rect_filled(r, 4.0 * z, theme::RAISED);
            painter.rect_stroke(r, 4.0 * z, Stroke::new(1.0, col), StrokeKind::Inside);
            let pin = l.to_screen(p.at);
            painter.circle_filled(pin, 4.0 * z, col);
            if z > 0.3 {
                let v = value.map_or("?".into(), |v| format!("{v:X}"));
                let mut name = p.name.clone();
                if name.chars().count() > 16 {
                    name = name.chars().take(15).collect::<String>() + "…";
                }
                let font = FontId::proportional(11.0 * z);
                painter.text(
                    r.left_center() + vec2(8.0 * z, 0.0),
                    Align2::LEFT_CENTER,
                    if p.incoming {
                        format!("→ {name}")
                    } else {
                        name.clone()
                    },
                    font.clone(),
                    TEXT,
                );
                painter.text(
                    r.right_center() - vec2(8.0 * z, 0.0),
                    Align2::RIGHT_CENTER,
                    if p.incoming { v } else { format!("{v} →") },
                    FontId::monospace(11.0 * z),
                    col,
                );
            }
        }
    }

    fn draw_board(&self, painter: &egui::Painter, board: Rect, l: &Layout) {
        painter.rect_filled(board, 0.0, BG);
        let step = 24.0 * self.zoom;
        if step <= 6.0 {
            return;
        }
        let start = l.origin - board.min;
        let mut x = board.left() + start.x.rem_euclid(step);
        while x < board.right() {
            let mut y = board.top() + start.y.rem_euclid(step);
            while y < board.bottom() {
                painter.circle_filled(pos2(x, y), 1.0, GRID);
                y += step;
            }
            x += step;
        }
    }

    fn draw_wires(&self, painter: &egui::Painter, l: &Layout, hover: &Hover) {
        let z = self.zoom;
        let mut shapes = Vec::new();
        let mut drawn = std::collections::HashSet::new();
        for (from, to) in self.world.wires() {
            if !l.wire_visible(from, to) {
                continue;
            }
            let (Some(a), Some(b)) = (l.source_pos(from), l.sink_pos(to)) else {
                continue;
            };
            if !drawn.insert((
                from,
                (a.x * 4.0) as i32,
                (a.y * 4.0) as i32,
                (b.x * 4.0) as i32,
                (b.y * 4.0) as i32,
            )) {
                continue;
            }
            let width = l.output_pin(from).map_or(1, |p| p.width);
            let bad = l
                .input_pin(to)
                .is_some_and(|t| !makine_world::compile::fits(width, t.width));
            let mut color = if bad {
                ERR
            } else {
                wire_color(width, self.sim.value(from))
            };
            let mut thick = if width > 1 { 4.0 } else { 2.5 } * z;
            if hover.wire == Some(to) {
                color = WIRE_HOVER;
                thick += 2.0 * z;
            }
            // Bezier control points may lie outside the endpoint rectangle.
            // Use their hull for conservative culling, or the full manual route.
            let manual = super::routing::points(&self.world, l, from, to);
            let hull = match &manual {
                Some(pts) => Rect::from_points(pts),
                None => {
                    Rect::from_points(&crate::canvas::draw::bezier_points(a, b, l.sink_dir(to)))
                }
            };
            if !painter.clip_rect().intersects(hull.expand(thick + 8.0)) {
                continue;
            }
            let label_at = manual
                .as_ref()
                .and_then(|p| p.get(p.len() / 2))
                .copied()
                .unwrap_or(a + (b - a) * 0.5);
            if let Some(pts) = manual {
                shapes.push(egui::Shape::line(pts, Stroke::new(thick, color)));
            } else {
                shapes.push(bezier(a, b, l.sink_dir(to), Stroke::new(thick, color)));
            }
            if z > 0.5 {
                if let Some(s) = self.world.wire_style(to).filter(|s| !s.name.is_empty()) {
                    painter.text(
                        label_at - vec2(0.0, 4.0),
                        Align2::CENTER_BOTTOM,
                        &s.name,
                        FontId::monospace(11.0 * z),
                        theme::BUS,
                    );
                }
            }
        }
        painter.extend(shapes);
    }

    /// The wire being drawn, following the pointer; red when the widths do
    /// not fit.
    pub(super) fn draw_new_wire(
        &self,
        painter: &egui::Painter,
        l: &Layout,
        from: End,
        pointer: Pos2,
        hover: &Hover,
    ) {
        let Some(a) = l.source_pos(from) else { return };
        let (to, dir, ok) = match hover.sink {
            Some((sink, at)) => (at, l.sink_dir(sink), self.fits(l, from, sink)),
            None => (pointer, vec2(-1.0, 0.0), true),
        };
        let color = if ok { BUS } else { ERR };
        painter.add(bezier(a, to, dir, Stroke::new(2.5 * self.zoom, color)));
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_part(
        &self,
        painter: &egui::Painter,
        l: &Layout,
        id: u64,
        kind: &Kind,
        at: Pos2,
        s: &Shape,
        hl: bool,
        hover: &Hover,
    ) {
        let z = self.zoom;
        let r = l.rect(s.geometry.rect(at));
        let font = |size: f32| FontId::proportional(size * z);
        let out = |pin| self.sim.value(End::new(id, pin));
        let input = |pin| self.sim.input_value(&self.world, End::new(id, pin));
        let border = if hl { TEXT } else { DIM };
        let face = |fill: Color32| {
            painter.rect_filled(r, CornerRadius::same((6.0 * z) as u8), fill);
            painter.rect_stroke(
                r,
                CornerRadius::same((6.0 * z) as u8),
                Stroke::new(1.0, border),
                StrokeKind::Inside,
            );
        };
        let lit_fill = theme::SIGNAL.gamma_multiply(0.3);
        let mut pin_names = false;
        match (&s.face, kind) {
            (Face::Canvas(part), _) => {
                if s.geometry.symbol == Symbol::Box {
                    draw_box(painter, r, part, &super::label(kind), z, hl);
                    pin_names = true;
                } else {
                    draw_gate(painter, r, &s.geometry, part, z, hl, out(0));
                }
            }
            (Face::Switch, Kind::Switch { on }) => {
                face(if *on { lit_fill } else { PART_FILL });
                // A toggle: a track with a knob at the side it is set to.
                let track = Rect::from_center_size(r.center(), vec2(30.0, 16.0) * z);
                painter.rect_filled(track, 8.0 * z, theme::RAISED_HOVER);
                let x = if *on {
                    track.right() - 8.0 * z
                } else {
                    track.left() + 8.0 * z
                };
                let knob = if *on { theme::SIGNAL } else { DIM };
                painter.circle_filled(pos2(x, track.center().y), 6.0 * z, knob);
            }
            (Face::Clock, Kind::Clock { period }) => {
                let on = out(0) == Some(1);
                face(if on { lit_fill } else { PART_FILL });
                // A square wave and the period.
                let (w, h) = (22.0 * z, 8.0 * z);
                let c = r.center() - vec2(0.0, 5.0 * z);
                let pts = vec![
                    c + vec2(-w, h),
                    c + vec2(-w / 2.0, h),
                    c + vec2(-w / 2.0, -h),
                    c + vec2(w / 2.0, -h),
                    c + vec2(w / 2.0, h),
                    c + vec2(w, h),
                ];
                let col = if on { theme::SIGNAL } else { DIM };
                painter.add(egui::Shape::line(pts, Stroke::new(2.0 * z, col)));
                painter.text(
                    r.center_bottom() - vec2(0.0, 3.0 * z),
                    Align2::CENTER_BOTTOM,
                    format!("{period}"),
                    font(9.0),
                    DIM,
                );
            }
            (Face::Lamp, _) => {
                let on = input(0).is_some_and(|v| v != 0);
                let c = r.center();
                let rad = r.width() * 0.36;
                if on {
                    painter.circle_filled(c, rad * 1.7, theme::SIGNAL.gamma_multiply(0.15));
                }
                let fill = if on {
                    Color32::from_rgb(255, 238, 170)
                } else {
                    Color32::from_rgb(58, 52, 34)
                };
                painter.circle_filled(c, rad, fill);
                painter.circle_stroke(c, rad, Stroke::new(1.5, border));
            }
            (Face::Input, Kind::Input { name, width, value }) => {
                let lit = *width == 1 && *value == 1;
                face(if lit { lit_fill } else { PART_FILL });
                self.port_text(painter, r, name, *width, Some(*value), lit);
            }
            (Face::Output, Kind::Output { name, width }) => {
                let v = input(0);
                let lit = *width == 1 && v == Some(1);
                face(if lit { lit_fill } else { PART_FILL });
                self.port_text(painter, r, name, *width, v, lit);
            }
            (
                Face::Screen,
                Kind::Device(Device::Screen { bits } | Device::ColorScreen { bits }),
            ) => {
                face(PART_FILL);
                let n = 1usize << bits;
                let px = SCREEN_PX * z;
                let pixels = self.screen_pixels(id);
                let left = r.right() - 8.0 * z - n as f32 * px;
                let top = r.center().y - n as f32 * px / 2.0;
                let screen = Rect::from_min_size(pos2(left, top), vec2(n as f32, n as f32) * px);
                painter.rect_filled(screen.expand(2.0 * z), 2.0, Color32::from_rgb(8, 12, 10));
                if let Some(p) = pixels {
                    for y in 0..n {
                        for x in 0..n {
                            if let Some(color) = p[y * n + x] {
                                let cell = Rect::from_min_size(
                                    screen.min + vec2(x as f32, y as f32) * px,
                                    vec2(px, px),
                                );
                                painter.rect_filled(cell, 0.0, color);
                            }
                        }
                    }
                }
                pin_names = true;
            }
            (Face::Display, Kind::Device(Device::Display { bits })) => {
                face(Color32::from_rgb(12, 24, 18));
                let v = input(0).unwrap_or(0);
                let hex = format!("0x{:0w$X}", v, w = (*bits as usize).div_ceil(4));
                painter.text(
                    r.center() + vec2(4.0 * z, -8.0 * z),
                    Align2::CENTER_CENTER,
                    hex,
                    FontId::monospace(16.0 * z),
                    Color32::from_rgb(140, 255, 160),
                );
                painter.text(
                    r.center() + vec2(4.0 * z, 12.0 * z),
                    Align2::CENTER_CENTER,
                    v.to_string(),
                    FontId::monospace(12.0 * z),
                    DIM,
                );
            }
            _ => face(PART_FILL),
        }

        // Pins: dots (lit with their value on 1-bit pins), and names on
        // boxes.
        for (pin, p) in s.pins.inputs.iter().enumerate() {
            let pos = l.to_screen(at + s.geometry.inputs[pin].0);
            let hovered = hover.sink.is_some_and(|(e, _)| e == End::new(id, pin));
            pin_dot(painter, pos, p, z, hovered);
            if pin_names {
                painter.text(
                    pos + vec2(10.0 * z, 0.0),
                    Align2::LEFT_CENTER,
                    p.to_string(),
                    font(11.0),
                    DIM,
                );
            }
        }
        for (pin, p) in s.pins.outputs.iter().enumerate() {
            let pos = l.to_screen(at + s.geometry.outputs[pin]);
            let hovered = hover.source.is_some_and(|(e, _)| e == End::new(id, pin));
            pin_dot(painter, pos, p, z, hovered);
            if pin_names {
                painter.text(
                    pos - vec2(10.0 * z, 0.0),
                    Align2::RIGHT_CENTER,
                    p.to_string(),
                    font(11.0),
                    DIM,
                );
            }
        }
    }

    /// A port's name above and its value below, like the canvas's boxes.
    fn port_text(
        &self,
        painter: &egui::Painter,
        r: Rect,
        name: &str,
        width: u8,
        v: Option<u64>,
        lit: bool,
    ) {
        let z = self.zoom;
        let label = if width == 1 {
            name.to_string()
        } else {
            format!("{name}[{width}]")
        };
        painter.text(
            r.center_top() + vec2(0.0, 4.0 * z),
            Align2::CENTER_TOP,
            label,
            FontId::proportional(11.0 * z),
            DIM,
        );
        let value = v.map_or("?".into(), |v| format_value(width, v));
        painter.text(
            r.center_bottom() - vec2(0.0, 4.0 * z),
            Align2::CENTER_BOTTOM,
            value,
            FontId::monospace(14.0 * z),
            if lit { theme::SIGNAL } else { TEXT },
        );
    }

    fn screen_pixels(&self, id: u64) -> Option<Vec<Option<Color32>>> {
        // Reading needs the circuit mutably; the view keeps a copy fresh
        // each frame (see `show`).
        self.screens.get(&id).cloned()
    }

    /// The selection rectangle, the part in hand and other players.
    fn draw_overlays(&self, painter: &egui::Painter, l: &Layout) {
        if let (Drag::Select(a), Some(b)) = (self.drag, self.hover_pos) {
            let r = l.rect(Rect::from_two_pos(a, b));
            painter.rect_filled(r, 0.0, theme::COPPER.gamma_multiply(0.08));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, theme::COPPER), StrokeKind::Inside);
        }
        if let (Some(kind), Some(p)) = (&self.placing, self.hover_pos) {
            // A ghost of the part in hand where a click would put it.
            if let Some(s) = super::shape(kind, self.sim.library(), self.symbols) {
                let at = p - vec2(30.0, 20.0);
                let r = l.rect(s.geometry.rect(at));
                painter.rect_filled(r, 6.0, theme::COPPER.gamma_multiply(0.15));
                painter.rect_stroke(r, 6.0, Stroke::new(1.0, theme::COPPER), StrokeKind::Inside);
                painter.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    super::label(kind),
                    FontId::proportional(12.0 * self.zoom),
                    theme::COPPER_BRIGHT,
                );
            }
        }
        for (name, color, p) in &self.others {
            let at = l.to_screen(pos2(p.x, p.y));
            // An arrow pointer with the player's name.
            let pts = vec![
                at,
                at + vec2(0.0, 16.0),
                at + vec2(4.5, 12.0),
                at + vec2(11.0, 12.0),
            ];
            painter.add(egui::Shape::convex_polygon(
                pts,
                *color,
                Stroke::new(1.0, Color32::BLACK),
            ));
            painter.text(
                at + vec2(12.0, 14.0),
                Align2::LEFT_TOP,
                name,
                FontId::proportional(12.0),
                *color,
            );
        }
    }
}

/// Screens' pixels, read from the running circuit once per frame: the
/// colour of each lit pixel.
pub(super) fn read_screens(
    v: &mut WorldView,
) -> std::collections::HashMap<u64, Vec<Option<Color32>>> {
    let ids: Vec<u64> = v
        .world
        .parts()
        .filter(|(_, p)| {
            matches!(
                p.kind,
                Kind::Device(Device::Screen { .. } | Device::ColorScreen { .. })
            )
        })
        .map(|(id, _)| id)
        .collect();
    ids.into_iter()
        .filter_map(|id| {
            let (sim, name) = v.sim.device(id)?;
            if let Some(px) = device::screen(sim, &name) {
                let on = Color32::from_rgb(140, 255, 160);
                return Some((id, px.into_iter().map(|p| p.then_some(on)).collect()));
            }
            let px = device::color_screen(sim, &name)?;
            let px = px
                .into_iter()
                .map(|c| {
                    (c != 0).then(|| {
                        let [r, g, b] = device::rgb(c);
                        Color32::from_rgb(r, g, b)
                    })
                })
                .collect();
            Some((id, px))
        })
        .collect()
}
