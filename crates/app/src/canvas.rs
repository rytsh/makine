//! The circuit canvas: draws a design and handles editing it.
//!
//! Interaction:
//! - drag a part to move it; drag the background to pan; scroll to zoom
//! - drag from an output pin (or a chip input on the left edge) to an input
//!   pin to wire them; dropping on empty space cancels
//! - right-click a part to delete or configure it; right-click an input pin
//!   to disconnect it
//! - click a chip input to toggle its value; drag input and output boxes to
//!   move them (right-click one to put them all back)

use egui::{
    epaint::CubicBezierShape, pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect,
    Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
};
use makine_sim::{mask, Circuit, Library, PartId, PartKind, PartPins, Pin, Sink, Source};

use crate::design::{
    geometry, Design, Geometry, Port, Symbol, BUBBLE, GATE_W, HEADER, LEAD, PIN_RADIUS,
};

pub const BG: Color32 = Color32::from_rgb(24, 26, 31);
const GRID: Color32 = Color32::from_rgb(34, 37, 44);
const PART_FILL: Color32 = Color32::from_rgb(45, 50, 60);
const PART_HEADER: Color32 = Color32::from_rgb(62, 70, 86);
const NAND_HEADER: Color32 = Color32::from_rgb(110, 70, 50);
const BUS_HEADER: Color32 = Color32::from_rgb(55, 80, 95);
const GATE_FILL: Color32 = Color32::from_rgb(40, 45, 55);
const GATE_LINE: Color32 = Color32::from_rgb(200, 206, 216);
const TEXT: Color32 = Color32::from_rgb(220, 224, 230);
const DIM: Color32 = Color32::from_rgb(140, 146, 158);
pub const ON: Color32 = Color32::from_rgb(90, 220, 120);
pub const OFF: Color32 = Color32::from_rgb(90, 96, 108);
const BUS: Color32 = Color32::from_rgb(110, 170, 240);
const ERR: Color32 = Color32::from_rgb(230, 90, 90);

/// Canvas camera and in-progress gestures. Persists across frames.
#[derive(Default)]
pub struct Canvas {
    pub offset: Vec2,
    pub zoom: f32,
    drag_wire: Option<Source>,
    drag_part: Option<PartId>,
    drag_port: Option<Port>,
    /// Whether the port being dragged actually moved (a click toggles it).
    port_moved: bool,
    /// Screen position of the world origin in the last frame.
    pub origin: Pos2,
    /// Draw basic gates as logic symbols instead of labelled boxes.
    pub symbols: bool,
}

/// What the canvas wants the app to do after this frame.
#[derive(Default)]
pub struct CanvasOutput {
    pub changed: bool,
    /// Input pin values the user toggled.
    pub toggled_input: Option<(usize, u64)>,
}

/// What can be dropped onto the canvas from the parts list.
#[derive(Clone)]
pub struct NewPart(pub PartKind);

struct Layout {
    origin: Pos2,
    zoom: f32,
    /// World position, geometry and pins of each part.
    parts: Vec<Option<(Pos2, Geometry, PartPins)>>,
    inputs: Vec<Rect>,
    outputs: Vec<Rect>,
}

impl Layout {
    fn to_screen(&self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.zoom
    }

    fn to_world(&self, p: Pos2) -> Pos2 {
        ((p - self.origin) / self.zoom).to_pos2()
    }

    fn rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(r.min), self.to_screen(r.max))
    }

    fn source_pos(&self, s: Source) -> Option<Pos2> {
        Some(match s {
            Source::Input(i) => self.to_screen(self.inputs.get(i)?.right_center()),
            Source::Part { part, pin } => {
                let (at, g, _) = self.parts.get(part)?.as_ref()?;
                self.to_screen(*at + *g.outputs.get(pin)?)
            }
        })
    }

    fn sink_pos(&self, s: Sink) -> Option<Pos2> {
        Some(match s {
            Sink::Output(i) => self.to_screen(self.outputs.get(i)?.left_center()),
            Sink::Part { part, pin } => {
                let (at, g, _) = self.parts.get(part)?.as_ref()?;
                self.to_screen(*at + g.inputs.get(pin)?.0)
            }
        })
    }

    fn source_width(&self, design: &Design, s: Source) -> u8 {
        match s {
            Source::Input(i) => design.def.inputs[i].width,
            Source::Part { part, pin } => self.parts[part]
                .as_ref()
                .map_or(1, |(_, _, p)| p.outputs[pin].width),
        }
    }

    fn sink_width(&self, design: &Design, s: Sink) -> u8 {
        match s {
            Sink::Output(i) => design.def.outputs[i].width,
            Sink::Part { part, pin } => self.parts[part]
                .as_ref()
                .map_or(1, |(_, _, p)| p.inputs[pin].width),
        }
    }

    fn sources(&self) -> impl Iterator<Item = (Source, Pos2)> + '_ {
        let inputs = (0..self.inputs.len()).map(Source::Input);
        let parts = self.parts.iter().enumerate().flat_map(|(part, p)| {
            let n = p.as_ref().map_or(0, |(_, _, pins)| pins.outputs.len());
            (0..n).map(move |pin| Source::Part { part, pin })
        });
        inputs
            .chain(parts)
            .filter_map(|s| Some((s, self.source_pos(s)?)))
    }

    fn sinks(&self) -> impl Iterator<Item = (Sink, Pos2)> + '_ {
        let outputs = (0..self.outputs.len()).map(Sink::Output);
        let parts = self.parts.iter().enumerate().flat_map(|(part, p)| {
            let n = p.as_ref().map_or(0, |(_, _, pins)| pins.inputs.len());
            (0..n).map(move |pin| Sink::Part { part, pin })
        });
        outputs
            .chain(parts)
            .filter_map(|s| Some((s, self.sink_pos(s)?)))
    }

    /// Direction a wire leaves a sink in (towards its source).
    fn sink_dir(&self, s: Sink) -> Vec2 {
        match s {
            Sink::Part { part, pin } => self.parts[part]
                .as_ref()
                .and_then(|(_, g, _)| g.inputs.get(pin))
                .map_or(vec2(-1.0, 0.0), |(_, d)| *d),
            Sink::Output(_) => vec2(-1.0, 0.0),
        }
    }

    fn part_at(&self, p: Pos2) -> Option<PartId> {
        self.parts.iter().enumerate().rev().find_map(|(i, r)| {
            r.as_ref()
                .filter(|(at, g, _)| self.rect(g.rect(*at)).contains(p))
                .map(|_| i)
        })
    }
}

/// Text shown for a value of the given width.
pub fn format_value(width: u8, v: u64) -> String {
    match width {
        1 => v.to_string(),
        w if w <= 8 => format!("{v}"),
        _ => format!("0x{v:X}"),
    }
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            symbols: true,
            ..Default::default()
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        design: &mut Design,
        lib: &Library,
        sim: Option<&Circuit>,
        input_values: &[u64],
        output_x: f32,
    ) -> CanvasOutput {
        let mut result = CanvasOutput::default();
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let screen = response.rect;
        if self.zoom == 0.0 {
            self.zoom = 1.0;
        }

        // Zoom around the pointer, pan with the background.
        if let Some(hover) = response.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
            let factor = zoom * (scroll.y * 0.0015).exp();
            if factor != 1.0 {
                let new = (self.zoom * factor).clamp(0.3, 3.0);
                let world = (hover - screen.min - self.offset) / self.zoom;
                self.offset = hover - screen.min - world * new;
                self.zoom = new;
            }
        }

        let layout = Layout {
            origin: screen.min + self.offset,
            zoom: self.zoom,
            parts: design
                .def
                .parts
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let pins = lib.part_pins(p)?;
                    let g = geometry(&p.kind, &pins, &part_label(p), self.symbols);
                    Some((design.part_pos(i), g, pins))
                })
                .collect(),
            inputs: (0..design.def.inputs.len())
                .map(|i| design.port_rect(Port::Input(i), output_x))
                .collect(),
            outputs: (0..design.def.outputs.len())
                .map(|i| design.port_rect(Port::Output(i), output_x))
                .collect(),
        };
        self.origin = layout.origin;
        let z = self.zoom;
        let font = |size: f32| FontId::proportional(size * z);
        let painter = painter.with_clip_rect(screen);

        painter.rect_filled(screen, 0.0, BG);
        let step = 24.0 * z;
        if step > 6.0 {
            let start = layout.origin.to_vec2();
            let mut x = screen.left() + start.x.rem_euclid(step);
            while x < screen.right() {
                let mut y = screen.top() + start.y.rem_euclid(step);
                while y < screen.bottom() {
                    painter.circle_filled(pos2(x, y), 1.0, GRID);
                    y += step;
                }
                x += step;
            }
        }

        let pointer = ui.ctx().pointer_interact_pos();
        let pin_hit = 10.0 * z.max(0.8);
        let hovered_source =
            pointer.and_then(|p| layout.sources().find(|(_, sp)| sp.distance(p) < pin_hit));
        let hovered_sink =
            pointer.and_then(|p| layout.sinks().find(|(_, sp)| sp.distance(p) < pin_hit));
        let hovered_part = pointer.and_then(|p| layout.part_at(p));
        let hovered_input = pointer.and_then(|p| {
            layout
                .inputs
                .iter()
                .position(|r| layout.rect(*r).contains(p))
        });
        let port_at = |p: Pos2| -> Option<Port> {
            let find = |rs: &[Rect]| rs.iter().position(|r| layout.rect(*r).contains(p));
            find(&layout.inputs)
                .map(Port::Input)
                .or_else(|| find(&layout.outputs).map(Port::Output))
        };
        let hovered_port = pointer.and_then(port_at);

        // Wires.
        let mut wire_shapes = Vec::new();
        for (wi, w) in design.def.wires.iter().enumerate() {
            let (Some(a), Some(b)) = (layout.source_pos(w.from), layout.sink_pos(w.to)) else {
                continue;
            };
            let width = layout.source_width(design, w.from);
            let value = sim.map(|s| s.wire(wi));
            let color = wire_color(width, value);
            let thick = if width > 1 { 4.0 } else { 2.5 } * z;
            wire_shapes.push(bezier(
                a,
                b,
                layout.sink_dir(w.to),
                Stroke::new(thick, color),
            ));
        }
        painter.extend(wire_shapes);

        // Parts.
        for (i, part) in design.def.parts.iter().enumerate() {
            let label = part_label(part);
            let Some((at, g, pins)) = &layout.parts[i] else {
                let r = layout.rect(Rect::from_min_size(design.part_pos(i), vec2(120.0, 40.0)));
                painter.rect_filled(r, 4.0 * z, ERR);
                painter.text(r.center(), Align2::CENTER_CENTER, &label, font(13.0), TEXT);
                continue;
            };
            let r = layout.rect(g.rect(*at));
            let hl = hovered_part == Some(i) || self.drag_part == Some(i);
            if g.symbol == Symbol::Box {
                draw_box(&painter, r, part, &label, z, hl);
            } else {
                // Output value colours the symbol's output lead.
                let out_on = sim.and_then(|s| {
                    design
                        .def
                        .wires
                        .iter()
                        .position(|w| w.from == Source::Part { part: i, pin: 0 })
                        .map(|wi| s.wire(wi))
                });
                draw_gate(&painter, r, g, part, z, hl, out_on);
            }
            let pin_names = g.symbol == Symbol::Box;
            for (pin, p) in pins.inputs.iter().enumerate() {
                let (off, dir) = g.inputs[pin];
                let at = layout.to_screen(*at + off);
                pin_dot(
                    &painter,
                    at,
                    p,
                    z,
                    hovered_sink.is_some_and(|(s, _)| s == Sink::Part { part: i, pin }),
                );
                if pin_names {
                    painter.text(
                        at + vec2(10.0 * z, 0.0),
                        Align2::LEFT_CENTER,
                        pin_label(p),
                        font(11.0),
                        DIM,
                    );
                } else if dir.y > 0.0 || p.width > 1 {
                    // Name the select input, and buses, next to the symbol.
                    let (pos, align) = if dir.y > 0.0 {
                        (at + vec2(8.0 * z, -2.0 * z), Align2::LEFT_BOTTOM)
                    } else {
                        (at + vec2(-4.0 * z, -8.0 * z), Align2::RIGHT_BOTTOM)
                    };
                    painter.text(pos, align, pin_label(p), font(10.0), DIM);
                }
            }
            for (pin, p) in pins.outputs.iter().enumerate() {
                let at = layout.to_screen(*at + g.outputs[pin]);
                pin_dot(
                    &painter,
                    at,
                    p,
                    z,
                    hovered_source.is_some_and(|(s, _)| s == Source::Part { part: i, pin }),
                );
                if pin_names {
                    painter.text(
                        at - vec2(10.0 * z, 0.0),
                        Align2::RIGHT_CENTER,
                        pin_label(p),
                        font(11.0),
                        DIM,
                    );
                }
            }
        }

        // The chip's own inputs (left) and outputs (right).
        for (i, p) in design.def.inputs.iter().enumerate() {
            let r = layout.rect(layout.inputs[i]);
            let v = input_values.get(i).copied().unwrap_or(0);
            port(
                &painter,
                r,
                p,
                v,
                z,
                hovered_port == Some(Port::Input(i)) || self.drag_port == Some(Port::Input(i)),
            );
            let at = layout.to_screen(layout.inputs[i].right_center());
            pin_dot(
                &painter,
                at,
                p,
                z,
                hovered_source.is_some_and(|(s, _)| s == Source::Input(i)),
            );
        }
        for (i, p) in design.def.outputs.iter().enumerate() {
            let r = layout.rect(layout.outputs[i]);
            let v = sim.map(|s| s.output(i));
            port(
                &painter,
                r,
                p,
                v.unwrap_or(0),
                z,
                hovered_port == Some(Port::Output(i)) || self.drag_port == Some(Port::Output(i)),
            );
            if v.is_none() {
                painter.text(
                    r.center_bottom() + vec2(0.0, 2.0),
                    Align2::CENTER_TOP,
                    "?",
                    font(11.0),
                    DIM,
                );
            }
            let at = layout.to_screen(layout.outputs[i].left_center());
            pin_dot(
                &painter,
                at,
                p,
                z,
                hovered_sink.is_some_and(|(s, _)| s == Sink::Output(i)),
            );
        }

        // Gestures. A drag is recognized only after the pointer has moved a
        // little, so look at what was under the pointer when it was pressed.
        if response.drag_started() {
            let press = ui.input(|i| i.pointer.press_origin()).or(pointer);
            let source_at =
                press.and_then(|p| layout.sources().find(|(_, sp)| sp.distance(p) < pin_hit));
            let part_at = press.and_then(|p| layout.part_at(p));
            let port = press.and_then(port_at);
            if let Some((s, _)) = source_at {
                self.drag_wire = Some(s);
            } else if let Some(port) = port {
                self.drag_port = Some(port);
                self.port_moved = false;
            } else if let Some(i) = part_at {
                self.drag_part = Some(i);
            }
        }
        if response.dragged() {
            let delta = response.drag_delta();
            if let Some(port) = self.drag_port {
                design.move_port(port, output_x, delta / z);
                self.port_moved = true;
            } else {
                match self.drag_part {
                    Some(i) => {
                        let p = &mut design.pos[i];
                        p[0] += delta.x / z;
                        p[1] += delta.y / z;
                    }
                    None if self.drag_wire.is_none() => self.offset += delta,
                    None => {}
                }
            }
        }
        if let (Some(from), Some(p)) = (self.drag_wire, pointer) {
            if let Some(a) = layout.source_pos(from) {
                let (to, dir, ok) = match hovered_sink {
                    Some((sink, at)) => {
                        let fw = layout.source_width(design, from);
                        let tw = layout.sink_width(design, sink);
                        (at, layout.sink_dir(sink), fw == tw || fw == 1)
                    }
                    None => (p, vec2(-1.0, 0.0), true),
                };
                let color = if ok { BUS } else { ERR };
                painter.add(bezier(a, to, dir, Stroke::new(2.5 * z, color)));
            }
        }
        if response.drag_stopped() {
            if let (Some(from), Some((sink, _))) = (self.drag_wire, hovered_sink) {
                let fw = layout.source_width(design, from);
                let tw = layout.sink_width(design, sink);
                if fw == tw || fw == 1 {
                    design.def.connect(from, sink);
                    result.changed = true;
                }
            }
            if self.drag_part.is_some() || self.port_moved {
                result.changed = true;
            }
            self.drag_wire = None;
            self.drag_part = None;
            self.drag_port = None;
            self.port_moved = false;
        }

        if response.clicked() {
            if let Some(i) = hovered_input {
                let p = &design.def.inputs[i];
                let v = input_values.get(i).copied().unwrap_or(0);
                let new = if p.width == 1 {
                    v ^ 1
                } else {
                    v.wrapping_add(1) & mask(p.width)
                };
                result.toggled_input = Some((i, new));
            }
        }

        // Context menu target is decided when the menu opens and kept while
        // it stays open, since the pointer moves into the menu.
        let menu_id = Id::unique("canvas_menu_target");
        if response.secondary_clicked() {
            let target = match (hovered_sink, hovered_part, hovered_port) {
                (Some((sink, _)), _, _) => MenuTarget::Sink(sink),
                (None, Some(i), _) => MenuTarget::Part(i),
                (None, None, Some(_)) => MenuTarget::Ports,
                _ => MenuTarget::None,
            };
            ui.ctx().data_mut(|d| d.insert_temp(menu_id, target));
        }
        response.context_menu(|ui| {
            let target = ui
                .ctx()
                .data(|d| d.get_temp::<MenuTarget>(menu_id))
                .unwrap_or(MenuTarget::None);
            match target {
                MenuTarget::Sink(sink) => {
                    if ui.button("Bağlantıyı kopar").clicked() {
                        design.def.wires.retain(|w| w.to != sink);
                        result.changed = true;
                        ui.close();
                    }
                }
                MenuTarget::Part(i) if i < design.def.parts.len() => {
                    if part_menu(ui, design, i, lib) {
                        result.changed = true;
                    }
                }
                MenuTarget::Ports => {
                    if ui.button("Giriş/çıkışları yerine diz").clicked() {
                        design.reset_ports();
                        result.changed = true;
                        ui.close();
                    }
                }
                _ => {
                    ui.label("Parça eklemek için soldaki listeden sürükle.");
                }
            }
        });

        // Parts dropped from the side panel.
        if let Some(kind) = response.dnd_release_payload::<NewPart>() {
            if let Some(p) = pointer {
                let at = layout.to_world(p) - vec2(40.0, 20.0);
                design.add_part(makine_sim::Part::new(kind.0.clone()), at);
                result.changed = true;
            }
        }

        if self.drag_port.is_some() || self.drag_part.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if hovered_source.is_some() || hovered_sink.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else if hovered_input.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        } else if hovered_part.is_some() || hovered_port.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        result
    }
}

#[derive(Clone, Copy, Debug)]
enum MenuTarget {
    None,
    Sink(Sink),
    Part(PartId),
    Ports,
}

/// Context menu for a part. Returns whether the design changed.
fn part_menu(ui: &mut Ui, design: &mut Design, i: PartId, lib: &Library) -> bool {
    let mut changed = false;
    let part = &mut design.def.parts[i];
    ui.label(egui::RichText::new(part_label(part)).strong());
    ui.separator();

    match &mut part.kind {
        PartKind::Split(widths) | PartKind::Join(widths) => {
            let label = ui.label("Gruplar (düşük bitten başlayarak):").id;
            let mut text = widths
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join(",");
            if ui
                .text_edit_singleline(&mut text)
                .labelled_by(label)
                .changed()
            {
                if let Some(ws) = parse_widths(&text) {
                    *widths = ws;
                    changed = true;
                }
            }
            ui.label(egui::RichText::new("örn. 8,8,8,8 ya da 1,1,1,1,1,1,1,1").small());
        }
        PartKind::Const { width, value } => {
            ui.horizontal(|ui| {
                let label = ui.label("Genişlik:").id;
                changed |= ui
                    .add(egui::DragValue::new(width).range(1..=64))
                    .labelled_by(label)
                    .changed();
            });
            ui.horizontal(|ui| {
                let label = ui.label("Değer:").id;
                changed |= ui
                    .add(egui::DragValue::new(value).hexadecimal(1, false, true))
                    .labelled_by(label)
                    .changed();
            });
            *value &= mask(*width);
        }
        _ => {}
    }

    if !matches!(part.kind, PartKind::Const { .. }) {
        ui.horizontal(|ui| {
            let label = ui.label("Tekrar:").id;
            changed |= ui
                .add(egui::DragValue::new(&mut part.repeat).range(1..=64))
                .labelled_by(label)
                .changed();
        })
        .response
        .on_hover_text("Parçanın kaç kopyası yan yana çalışsın. Her uç bu kadar genişler.");
    }

    if changed {
        design.prune(lib);
    }
    ui.separator();
    if ui.button("Sil").clicked() {
        design.remove_part(i);
        changed = true;
        ui.close();
    }
    changed
}

fn parse_widths(text: &str) -> Option<Vec<u8>> {
    let ws: Vec<u8> = text
        .split(',')
        .map(|s| s.trim().parse::<u8>().ok().filter(|&w| w >= 1))
        .collect::<Option<_>>()?;
    let total: u32 = ws.iter().map(|&w| w as u32).sum();
    (!ws.is_empty() && total <= 64).then_some(ws)
}

pub fn part_label(p: &makine_sim::Part) -> String {
    let name = match &p.kind {
        PartKind::Nand => "NAND".to_string(),
        PartKind::Chip(n) => n.clone(),
        PartKind::Split(w) => format!("Böl {}", join_widths(w)),
        PartKind::Join(w) => format!("Birleştir {}", join_widths(w)),
        PartKind::Const { width, value } => format!("Sabit {}", format_value(*width, *value)),
    };
    if p.repeat > 1 {
        format!("{name} ×{}", p.repeat)
    } else {
        name
    }
}

fn join_widths(w: &[u8]) -> String {
    if w.len() > 1 && w.iter().all(|&x| x == w[0]) {
        format!("{}×{}", w.len(), w[0])
    } else {
        w.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn pin_label(p: &Pin) -> String {
    p.to_string()
}

fn wire_color(width: u8, value: Option<u64>) -> Color32 {
    match (width, value) {
        (1, Some(1)) => ON,
        (1, _) => OFF,
        (_, Some(0) | None) => BUS.gamma_multiply(0.55),
        _ => BUS,
    }
}

fn pin_dot(painter: &egui::Painter, at: Pos2, p: &Pin, z: f32, hovered: bool) {
    let r = PIN_RADIUS * z * if hovered { 1.4 } else { 1.0 };
    let fill = if p.width > 1 { BUS } else { DIM };
    painter.circle_filled(at, r, fill);
    painter.circle_stroke(at, r, Stroke::new(1.0, BG));
}

fn port(painter: &egui::Painter, r: Rect, p: &Pin, v: u64, z: f32, hovered: bool) {
    let lit = p.width == 1 && v == 1;
    let fill = if lit {
        ON.gamma_multiply(0.35)
    } else {
        PART_FILL
    };
    painter.rect_filled(r, CornerRadius::same((6.0 * z) as u8), fill);
    let border = if hovered {
        TEXT
    } else if lit {
        ON
    } else {
        DIM
    };
    painter.rect_stroke(
        r,
        CornerRadius::same((6.0 * z) as u8),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.text(
        r.center_top() + vec2(0.0, 4.0 * z),
        Align2::CENTER_TOP,
        pin_label(p),
        FontId::proportional(11.0 * z),
        DIM,
    );
    painter.text(
        r.center_bottom() - vec2(0.0, 4.0 * z),
        Align2::CENTER_BOTTOM,
        format_value(p.width, v),
        FontId::monospace(14.0 * z),
        if lit { ON } else { TEXT },
    );
}

/// A wire from source `a` (leaving to the right) to sink `b`, arriving from
/// direction `dir` (pointing away from the sink, towards the wire).
fn bezier(a: Pos2, b: Pos2, dir: Vec2, stroke: Stroke) -> Shape {
    let d = ((b.x - a.x).abs() * 0.5).max(40.0);
    let end = if dir.y != 0.0 {
        let dy = ((b.y - a.y).abs() * 0.5).max(40.0);
        b + dir * dy
    } else {
        b + dir * d
    };
    CubicBezierShape::from_points_stroke(
        [a, a + vec2(d, 0.0), end, b],
        false,
        Color32::TRANSPARENT,
        stroke,
    )
    .into()
}

/// A part drawn as a labelled box with a coloured header.
fn draw_box(
    painter: &egui::Painter,
    r: Rect,
    part: &makine_sim::Part,
    label: &str,
    z: f32,
    hl: bool,
) {
    let header = match part.kind {
        PartKind::Nand => NAND_HEADER,
        PartKind::Chip(_) => PART_HEADER,
        _ => BUS_HEADER,
    };
    let radius = (5.0 * z) as u8;
    painter.rect_filled(r, CornerRadius::same(radius), PART_FILL);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEADER * z));
    painter.rect_filled(
        head,
        CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        },
        header,
    );
    painter.text(
        head.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0 * z),
        TEXT,
    );
    if hl {
        painter.rect_stroke(
            r,
            CornerRadius::same(radius),
            Stroke::new(1.5, DIM),
            StrokeKind::Outside,
        );
    }
}

/// Outline of a gate body, in screen space. `x0..x1` is the body (leads
/// excluded), `y0..y1` its height.
fn gate_outline(symbol: Symbol, x0: f32, x1: f32, y0: f32, y1: f32) -> Vec<Pos2> {
    let (w, h, cy) = (x1 - x0, y1 - y0, (y0 + y1) / 2.0);
    let mut pts = Vec::new();
    // Cubic Bezier sampled into points.
    let cubic = |pts: &mut Vec<Pos2>, p: [Pos2; 4], n: usize| {
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            let v = p[0].to_vec2() * (u * u * u)
                + p[1].to_vec2() * (3.0 * u * u * t)
                + p[2].to_vec2() * (3.0 * u * t * t)
                + p[3].to_vec2() * (t * t * t);
            pts.push(v.to_pos2());
        }
    };
    match symbol {
        Symbol::Not => {
            pts.extend([pos2(x0, y0), pos2(x1, cy), pos2(x0, y1)]);
        }
        Symbol::And | Symbol::Nand => {
            // Flat back, half-circle front.
            let mid = x0 + w - h / 2.0;
            pts.push(pos2(x0, y0));
            pts.push(pos2(mid, y0));
            for k in 1..=16 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 16.0;
                pts.push(pos2(mid + a.cos() * h / 2.0, cy + a.sin() * h / 2.0));
            }
            pts.push(pos2(x0, y1));
        }
        Symbol::Or | Symbol::Xor => {
            // Curved back, pointed front.
            let back = w * 0.22;
            pts.push(pos2(x0, y0));
            cubic(
                &mut pts,
                [
                    pos2(x0, y0),
                    pos2(x0 + w * 0.55, y0),
                    pos2(x0 + w * 0.85, y0 + h * 0.2),
                    pos2(x1, cy),
                ],
                12,
            );
            cubic(
                &mut pts,
                [
                    pos2(x1, cy),
                    pos2(x0 + w * 0.85, y1 - h * 0.2),
                    pos2(x0 + w * 0.55, y1),
                    pos2(x0, y1),
                ],
                12,
            );
            cubic(
                &mut pts,
                [
                    pos2(x0, y1),
                    pos2(x0 + back, y1 - h * 0.25),
                    pos2(x0 + back, y0 + h * 0.25),
                    pos2(x0, y0),
                ],
                10,
            );
            pts.pop();
        }
        Symbol::Mux => {
            // Trapezoid, wide on the input side.
            let s = h * 0.18;
            pts.extend([
                pos2(x0, y0),
                pos2(x1, y0 + s),
                pos2(x1, y1 - s),
                pos2(x0, y1),
            ]);
        }
        Symbol::Demux => {
            let s = h * 0.18;
            pts.extend([
                pos2(x0, y0 + s),
                pos2(x1, y0),
                pos2(x1, y1),
                pos2(x0, y1 - s),
            ]);
        }
        Symbol::Box => {}
    }
    pts
}

/// A gate drawn as its logic symbol.
fn draw_gate(
    painter: &egui::Painter,
    r: Rect,
    g: &Geometry,
    part: &makine_sim::Part,
    z: f32,
    hl: bool,
    out_value: Option<u64>,
) {
    let lead = LEAD * z;
    let body_w = GATE_W * z;
    let x0 = r.left() + lead;
    let x1 = x0 + body_w;
    let tall = matches!(g.symbol, Symbol::Mux | Symbol::Demux);
    let (y0, y1) = if tall {
        (r.top(), r.bottom() - lead)
    } else {
        (r.top(), r.bottom())
    };
    let line = if hl { TEXT } else { GATE_LINE };
    let stroke = Stroke::new(2.0 * z.max(0.6), line);

    // Leads from the body edge to each pin.
    let at = r.min.to_vec2();
    for (off, dir) in &g.inputs {
        let p = (*off * z + at).to_pos2();
        let edge = if dir.y > 0.0 {
            // Select: up to the slanted bottom edge.
            pos2(p.x, y1 - (y1 - y0) * 0.09)
        } else {
            let mut x = x0;
            if matches!(g.symbol, Symbol::Or | Symbol::Xor) {
                // Meet the curved back.
                let t = ((p.y - (y0 + y1) / 2.0) / ((y1 - y0) / 2.0)).abs();
                x += body_w * 0.22 * 0.75 * (1.0 - t * t);
            }
            pos2(x, p.y)
        };
        painter.line_segment([p, edge], stroke);
    }
    let out_x = if g.symbol.bubble() {
        x1 + 2.0 * BUBBLE * z
    } else {
        x1
    };
    let out_color = match out_value {
        Some(1) => ON,
        _ => line,
    };
    for off in &g.outputs {
        let p = (*off * z + at).to_pos2();
        let from = if matches!(g.symbol, Symbol::Demux) {
            pos2(x1, p.y)
        } else {
            pos2(out_x, p.y)
        };
        painter.line_segment([from, p], Stroke::new(stroke.width, out_color));
    }

    let outline = gate_outline(g.symbol, x0, x1, y0, y1);
    if g.symbol == Symbol::Xor {
        // The extra curve behind an XOR.
        let gap = 6.0 * z;
        let back = body_w * 0.22;
        let n = 10;
        let pts: Vec<Pos2> = (0..=n)
            .map(|k| {
                let t = k as f32 / n as f32;
                let u = 1.0 - t;
                let p = [
                    pos2(x0 - gap, y1),
                    pos2(x0 - gap + back, y1 - (y1 - y0) * 0.25),
                    pos2(x0 - gap + back, y0 + (y1 - y0) * 0.25),
                    pos2(x0 - gap, y0),
                ];
                (p[0].to_vec2() * (u * u * u)
                    + p[1].to_vec2() * (3.0 * u * u * t)
                    + p[2].to_vec2() * (3.0 * u * t * t)
                    + p[3].to_vec2() * (t * t * t))
                    .to_pos2()
            })
            .collect();
        painter.add(Shape::line(pts, stroke));
    }
    painter.add(Shape::convex_polygon(
        outline.clone(),
        GATE_FILL,
        Stroke::NONE,
    ));
    // Or/Xor outlines are not convex at the back; fill the notch with
    // background so the shape reads correctly.
    if matches!(g.symbol, Symbol::Or | Symbol::Xor) {
        let back = body_w * 0.22;
        let n = 10;
        let mut notch = vec![pos2(x0, y0)];
        for k in 1..n {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            let p = [
                pos2(x0, y0),
                pos2(x0 + back, y0 + (y1 - y0) * 0.25),
                pos2(x0 + back, y1 - (y1 - y0) * 0.25),
                pos2(x0, y1),
            ];
            notch.push(
                (p[0].to_vec2() * (u * u * u)
                    + p[1].to_vec2() * (3.0 * u * u * t)
                    + p[2].to_vec2() * (3.0 * u * t * t)
                    + p[3].to_vec2() * (t * t * t))
                    .to_pos2(),
            );
        }
        notch.push(pos2(x0, y1));
        painter.add(Shape::convex_polygon(notch, BG, Stroke::NONE));
    }
    painter.add(Shape::closed_line(outline, stroke));
    if g.symbol.bubble() {
        let c = pos2(x1 + BUBBLE * z, (y0 + y1) / 2.0);
        painter.circle(c, BUBBLE * z, GATE_FILL, stroke);
    }

    let mid = pos2((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    match g.symbol {
        Symbol::Mux | Symbol::Demux => {
            painter.text(
                mid,
                Align2::CENTER_CENTER,
                if g.symbol == Symbol::Mux {
                    "MUX"
                } else {
                    "DMX"
                },
                FontId::proportional(11.0 * z),
                DIM,
            );
            if g.symbol == Symbol::Mux {
                // Which input is which.
                for (k, (off, _)) in g.inputs.iter().take(2).enumerate() {
                    let p = (*off * z + at).to_pos2();
                    painter.text(
                        pos2(x0 + 4.0 * z, p.y),
                        Align2::LEFT_CENTER,
                        k.to_string(),
                        FontId::proportional(10.0 * z),
                        DIM,
                    );
                }
            } else {
                for (k, off) in g.outputs.iter().enumerate() {
                    let p = (*off * z + at).to_pos2();
                    painter.text(
                        pos2(x1 - 4.0 * z, p.y),
                        Align2::RIGHT_CENTER,
                        k.to_string(),
                        FontId::proportional(10.0 * z),
                        DIM,
                    );
                }
            }
        }
        _ => {}
    }
    if part.repeat > 1 {
        painter.text(
            pos2(mid.x, y0 - 2.0 * z),
            Align2::CENTER_BOTTOM,
            format!("×{}", part.repeat),
            FontId::proportional(11.0 * z),
            BUS,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_parsing() {
        assert_eq!(parse_widths("8, 8,8,8"), Some(vec![8; 4]));
        assert_eq!(parse_widths("1"), Some(vec![1]));
        assert_eq!(parse_widths("0,1"), None);
        assert_eq!(parse_widths("32,33"), None);
        assert_eq!(parse_widths("x"), None);
    }
}
