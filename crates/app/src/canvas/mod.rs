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
//! - click parts to select them (Shift adds), Shift+drag a box around
//!   them; dragging a selected part moves the whole selection; Delete,
//!   Ctrl+C/X/V/D/A work on the selection; Ctrl+Z/Y undo and redo
//!
//! `layout.rs` places everything on screen, `draw.rs` paints it and
//! `menu.rs` holds the right-click menus.

use egui::{pos2, vec2, FontId, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use makine_sim::{mask, Circuit, Library, PartId, PartKind, Sink, Source};

use crate::design::{Design, Port, Symbol};

pub(crate) mod draw;
mod layout;
mod menu;

pub(crate) use draw::gate_outline;
use draw::{bezier, draw_box, draw_gate, pin_dot, port, ERR, GRID, TEXT};
pub use draw::{BG, ON};
pub use layout::{format_value, part_label};
use layout::{pin_label, Layout};
use menu::{part_menu, MenuTarget};

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
    /// Selected parts.
    pub selected: Vec<PartId>,
    /// Where a Shift+drag selection box started (screen).
    select_from: Option<Pos2>,
    /// How many times the same text was pasted, to offset the copies.
    pastes: u32,
}

/// What the canvas wants the app to do after this frame.
#[derive(Default)]
pub struct CanvasOutput {
    pub changed: bool,
    /// Input pin values the user toggled.
    pub toggled_input: Option<(usize, u64)>,
    pub undo: bool,
    pub redo: bool,
    /// The change came from a menu field, which changes every frame while
    /// dragged: one undo step for all of it.
    pub merge: bool,
}

/// What can be dropped onto the canvas from the parts list.
#[derive(Clone)]
pub struct NewPart(pub PartKind);

/// What is under the pointer this frame.
#[derive(Default)]
struct Hover {
    source: Option<(Source, Pos2)>,
    sink: Option<(Sink, Pos2)>,
    part: Option<PartId>,
    /// A chip input box (clicking toggles it).
    input: Option<usize>,
    /// Any input or output box.
    port: Option<Port>,
    /// A wire (index into the design's wires), when nothing else is.
    wire: Option<usize>,
}

impl Hover {
    /// Whether the pointer is on a pin, part or box (which win over wires).
    fn on_something(&self) -> bool {
        self.source.is_some() || self.sink.is_some() || self.part.is_some() || self.port.is_some()
    }
}

/// What the canvas draws besides the design: the live simulation.
pub struct Live<'a> {
    pub sim: Option<&'a Circuit>,
    pub inputs: &'a [u64],
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            symbols: true,
            ..Default::default()
        }
    }

    /// Draw `design` and handle this frame's input. `output_x` is where
    /// unmoved output boxes go (the right edge of the visible area).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        design: &mut Design,
        lib: &Library,
        sim: Option<&Circuit>,
        input_values: &[u64],
        output_x: f32,
    ) -> CanvasOutput {
        let live = Live {
            sim,
            inputs: input_values,
        };
        let mut result = CanvasOutput::default();
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let screen = response.rect;
        if self.zoom == 0.0 {
            self.zoom = 1.0;
        }
        self.zoom_at_pointer(ui, &response);

        let n = design.def.parts.len();
        self.selected.retain(|&i| i < n);
        let layout = Layout::new(
            design,
            lib,
            screen.min + self.offset,
            self.zoom,
            self.symbols,
            output_x,
        );
        self.origin = layout.origin;
        let painter = painter.with_clip_rect(screen);
        let pointer = ui.ctx().pointer_interact_pos();
        let hover = pointer.map_or_else(Hover::default, |p| self.hover(design, &layout, p));

        self.draw_background(&painter, screen, &layout);
        self.draw_design(&painter, design, &layout, &live, &hover);
        self.gestures(
            ui,
            &response,
            &painter,
            design,
            &layout,
            &hover,
            &mut result,
        );
        self.toggle_input(&response, design, &live, &hover, &mut result);
        self.click_select(ui, &response, &hover);
        self.keys(ui, &response, design, &layout, &hover, &mut result);
        self.context_menu(ui, &response, design, lib, &hover, &mut result);

        // Parts dropped from the side panel.
        if let (Some(kind), Some(p)) = (response.dnd_release_payload::<NewPart>(), pointer) {
            let at = layout.to_world(p) - vec2(40.0, 20.0);
            let i = design.add_part(makine_sim::Part::new(kind.0.clone()), at);
            self.selected = vec![i];
            result.changed = true;
        }
        self.set_cursor(ui, &hover);
        result
    }

    /// What is under screen point `p`, wires included.
    fn hover(&self, design: &Design, layout: &Layout, p: Pos2) -> Hover {
        let mut h = layout.hover(p, self.pin_hit());
        if !h.on_something() && self.drag_wire.is_none() {
            h.wire = layout.wire_at(design, p, 6.0 * self.zoom.max(0.8));
        }
        h
    }

    /// A click on a part selects it (Shift: adds or removes it); a click on
    /// empty board clears the selection.
    fn click_select(&mut self, ui: &Ui, response: &egui::Response, hover: &Hover) {
        if !response.clicked() || hover.input.is_some() {
            return;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        match hover.part {
            Some(i) if shift => {
                if let Some(at) = self.selected.iter().position(|&s| s == i) {
                    self.selected.remove(at);
                } else {
                    self.selected.push(i);
                }
            }
            Some(i) => self.selected = vec![i],
            None if !shift => self.selected.clear(),
            None => {}
        }
    }

    /// Keyboard: Delete, copy/cut/paste, duplicate, select all, undo/redo.
    fn keys(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        design: &mut Design,
        layout: &Layout,
        hover: &Hover,
        result: &mut CanvasOutput,
    ) {
        if !response.hovered() || ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        use egui::Key;
        let (delete, undo, redo, dup, all, esc) = ui.input(|i| {
            let cmd = i.modifiers.command;
            (
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                cmd && !i.modifiers.shift && i.key_pressed(Key::Z),
                cmd && (i.key_pressed(Key::Y) || (i.modifiers.shift && i.key_pressed(Key::Z))),
                cmd && i.key_pressed(Key::D),
                cmd && i.key_pressed(Key::A),
                i.key_pressed(Key::Escape),
            )
        });
        result.undo |= undo;
        result.redo |= redo;
        if esc {
            self.selected.clear();
        }
        if all {
            self.selected = (0..design.def.parts.len()).collect();
        }
        if delete {
            // The wire under the pointer first: it is what the player sees
            // highlighted.
            if let Some(wi) = hover.wire {
                design.def.wires.remove(wi);
                result.changed = true;
            } else if !self.selected.is_empty() {
                design.remove_parts(&self.selected);
                self.selected.clear();
                result.changed = true;
            }
        }
        let pointer = ui.ctx().pointer_interact_pos().map(|p| layout.to_world(p));
        if dup {
            if let Some(text) = design.copy(&self.selected) {
                self.paste(design, &text, None, result);
            }
        }
        let events = ui.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Copy => {
                    if let Some(t) = design.copy(&self.selected) {
                        ui.ctx().copy_text(t);
                        self.pastes = 0;
                    }
                }
                egui::Event::Cut => {
                    if let Some(t) = design.copy(&self.selected) {
                        ui.ctx().copy_text(t);
                        self.pastes = 0;
                        design.remove_parts(&self.selected);
                        self.selected.clear();
                        result.changed = true;
                    }
                }
                egui::Event::Paste(t) => self.paste(design, &t, pointer, result),
                _ => {}
            }
        }
    }

    /// Paste copied parts at `at` (world), or next to where they were.
    fn paste(
        &mut self,
        design: &mut Design,
        text: &str,
        at: Option<Pos2>,
        result: &mut CanvasOutput,
    ) {
        self.pastes += 1;
        let at = at.unwrap_or_else(|| {
            let min = self
                .selected
                .iter()
                .map(|&i| design.part_pos(i))
                .fold(pos2(f32::MAX, f32::MAX), |m, p| m.min(p));
            if min.x == f32::MAX {
                pos2(200.0, 200.0)
            } else {
                min + vec2(30.0, 30.0)
            }
        });
        let new = design.paste(text, at);
        if !new.is_empty() {
            self.selected = new;
            result.changed = true;
        }
    }

    /// How close the pointer must be to a pin to grab it, in screen points.
    fn pin_hit(&self) -> f32 {
        10.0 * self.zoom.max(0.8)
    }

    /// The wheel (or a pinch) zooms around the pointer.
    fn zoom_at_pointer(&mut self, ui: &Ui, response: &egui::Response) {
        let Some(hover) = response.hover_pos() else {
            return;
        };
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        let factor = zoom * (scroll.y * 0.0015).exp();
        if factor != 1.0 {
            let new = (self.zoom * factor).clamp(0.3, 3.0);
            let screen_min = response.rect.min;
            let world = (hover - screen_min - self.offset) / self.zoom;
            self.offset = hover - screen_min - world * new;
            self.zoom = new;
        }
    }

    /// Background colour and the dot grid.
    fn draw_background(&self, painter: &egui::Painter, screen: Rect, layout: &Layout) {
        painter.rect_filled(screen, 0.0, BG);
        let step = 24.0 * self.zoom;
        if step <= 6.0 {
            return;
        }
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

    /// Wires, parts and the chip's own input and output boxes.
    fn draw_design(
        &self,
        painter: &egui::Painter,
        design: &Design,
        layout: &Layout,
        live: &Live,
        hover: &Hover,
    ) {
        let z = self.zoom;
        // Wires first, under the parts.
        let wires = design.def.wires.iter().enumerate().filter_map(|(wi, w)| {
            let (a, b) = (layout.source_pos(w.from)?, layout.sink_pos(w.to)?);
            let width = layout.source_width(design, w.from);
            let mut color = draw::wire_color(width, live.sim.map(|s| s.wire(wi)));
            let mut thick = if width > 1 { 4.0 } else { 2.5 } * z;
            if hover.wire == Some(wi) {
                // The wire a right click or Delete would cut.
                color = draw::WIRE_HOVER;
                thick += 2.0 * z;
            }
            Some(bezier(
                a,
                b,
                layout.sink_dir(w.to),
                Stroke::new(thick, color),
            ))
        });
        painter.extend(wires.collect::<Vec<_>>());

        for i in 0..design.def.parts.len() {
            self.draw_part(painter, design, layout, live, hover, i);
        }
        self.draw_ports(painter, design, layout, live, hover);
    }

    /// One part with its pins (and pin names on boxes).
    fn draw_part(
        &self,
        painter: &egui::Painter,
        design: &Design,
        layout: &Layout,
        live: &Live,
        hover: &Hover,
        i: PartId,
    ) {
        let z = self.zoom;
        let font = |size: f32| FontId::proportional(size * z);
        let part = &design.def.parts[i];
        let label = part_label(part);
        let Some((at, g, pins)) = &layout.parts[i] else {
            // A part whose chip no longer exists.
            let r = layout.rect(Rect::from_min_size(design.part_pos(i), vec2(120.0, 40.0)));
            painter.rect_filled(r, 4.0 * z, ERR);
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                font(13.0),
                TEXT,
            );
            return;
        };
        let r = layout.rect(g.rect(*at));
        let selected = self.selected.contains(&i);
        let hl = selected || hover.part == Some(i) || self.drag_part == Some(i);
        if selected {
            painter.rect_stroke(
                r.expand(4.0 * z),
                4.0 * z,
                Stroke::new(1.5, crate::theme::COPPER_BRIGHT),
                egui::StrokeKind::Outside,
            );
        }
        if g.symbol == Symbol::Box {
            draw_box(painter, r, part, &label, z, hl);
        } else {
            // The output value colours the symbol's output lead.
            let out = Source::Part { part: i, pin: 0 };
            let out_on = live.sim.and_then(|s| {
                let wi = design.def.wires.iter().position(|w| w.from == out)?;
                Some(s.wire(wi))
            });
            draw_gate(painter, r, g, part, z, hl, out_on);
        }
        let names = g.symbol == Symbol::Box;
        let dim = draw::DIM;
        for (pin, p) in pins.inputs.iter().enumerate() {
            let (off, dir) = g.inputs[pin];
            let at = layout.to_screen(*at + off);
            let hovered = hover
                .sink
                .is_some_and(|(s, _)| s == Sink::Part { part: i, pin });
            pin_dot(painter, at, p, z, hovered);
            if names {
                let pos = at + vec2(10.0 * z, 0.0);
                painter.text(
                    pos,
                    egui::Align2::LEFT_CENTER,
                    pin_label(p),
                    font(11.0),
                    dim,
                );
            } else if dir.y > 0.0 || p.width > 1 {
                // Name the select input, and buses, next to the symbol.
                let (pos, align) = if dir.y > 0.0 {
                    (at + vec2(8.0 * z, -2.0 * z), egui::Align2::LEFT_BOTTOM)
                } else {
                    (at + vec2(-4.0 * z, -8.0 * z), egui::Align2::RIGHT_BOTTOM)
                };
                painter.text(pos, align, pin_label(p), font(10.0), dim);
            }
        }
        for (pin, p) in pins.outputs.iter().enumerate() {
            let at = layout.to_screen(*at + g.outputs[pin]);
            let hovered = hover
                .source
                .is_some_and(|(s, _)| s == Source::Part { part: i, pin });
            pin_dot(painter, at, p, z, hovered);
            if names {
                let pos = at - vec2(10.0 * z, 0.0);
                painter.text(
                    pos,
                    egui::Align2::RIGHT_CENTER,
                    pin_label(p),
                    font(11.0),
                    dim,
                );
            }
        }
    }

    /// The chip's own inputs (left) and outputs (right).
    fn draw_ports(
        &self,
        painter: &egui::Painter,
        design: &Design,
        layout: &Layout,
        live: &Live,
        hover: &Hover,
    ) {
        let z = self.zoom;
        let lit = |p: Port| hover.port == Some(p) || self.drag_port == Some(p);
        for (i, p) in design.def.inputs.iter().enumerate() {
            let r = layout.rect(layout.inputs[i]);
            let v = live.inputs.get(i).copied().unwrap_or(0);
            port(painter, r, p, v, z, lit(Port::Input(i)));
            let at = layout.to_screen(layout.inputs[i].right_center());
            let hovered = hover.source.is_some_and(|(s, _)| s == Source::Input(i));
            pin_dot(painter, at, p, z, hovered);
        }
        for (i, p) in design.def.outputs.iter().enumerate() {
            let r = layout.rect(layout.outputs[i]);
            let v = live.sim.map(|s| s.output(i));
            port(painter, r, p, v.unwrap_or(0), z, lit(Port::Output(i)));
            if v.is_none() {
                // Not simulated (the design does not compile).
                painter.text(
                    r.center_bottom() + vec2(0.0, 2.0),
                    egui::Align2::CENTER_TOP,
                    "?",
                    FontId::proportional(11.0 * z),
                    draw::DIM,
                );
            }
            let at = layout.to_screen(layout.outputs[i].left_center());
            let hovered = hover.sink.is_some_and(|(s, _)| s == Sink::Output(i));
            pin_dot(painter, at, p, z, hovered);
        }
    }

    /// Dragging: wires from pins, parts, port boxes, or the view itself.
    #[allow(clippy::too_many_arguments)]
    fn gestures(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        design: &mut Design,
        layout: &Layout,
        hover: &Hover,
        result: &mut CanvasOutput,
    ) {
        let z = self.zoom;
        // A drag is recognized only after the pointer has moved a little, so
        // look at what was under the pointer when it was pressed.
        if response.drag_started() {
            let press = ui
                .input(|i| i.pointer.press_origin())
                .or(ui.ctx().pointer_interact_pos());
            let at = press.map_or_else(Hover::default, |p| layout.hover(p, self.pin_hit()));
            let held = at.sink.and_then(|(sink, _)| design.def.wire_into(sink));
            if let Some((s, _)) = at.source {
                self.drag_wire = Some(s);
            } else if let Some(wi) = held {
                // Pulling a wire off an input: drop it on another input to
                // move it, or on empty space to cut it.
                self.drag_wire = Some(design.def.wires.remove(wi).from);
                result.changed = true;
            } else if let Some(port) = at.port {
                self.drag_port = Some(port);
                self.port_moved = false;
            } else if let Some(i) = at.part {
                if !self.selected.contains(&i) {
                    self.selected = vec![i];
                }
                self.drag_part = Some(i);
            } else if ui.input(|i| i.modifiers.shift) {
                self.select_from = press;
            }
        }
        if response.dragged() {
            let delta = response.drag_delta();
            if let Some(port) = self.drag_port {
                design.move_port(port, layout.output_x, delta / z);
                self.port_moved = true;
            } else if self.drag_part.is_some() {
                for &i in &self.selected {
                    let p = &mut design.pos[i];
                    p[0] += delta.x / z;
                    p[1] += delta.y / z;
                }
            } else if self.select_from.is_some() {
                // Drawn below.
            } else if self.drag_wire.is_none() {
                self.offset += delta;
            }
        }
        if let (Some(a), Some(b)) = (self.select_from, ui.ctx().pointer_interact_pos()) {
            let r = Rect::from_two_pos(a, b);
            painter.rect_filled(r, 2.0, crate::theme::COPPER.gamma_multiply(0.12));
            painter.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, crate::theme::COPPER),
                egui::StrokeKind::Inside,
            );
            if response.drag_stopped() {
                for (i, p) in layout.parts.iter().enumerate() {
                    let Some((at, g, _)) = p else { continue };
                    if r.intersects(layout.rect(g.rect(*at))) && !self.selected.contains(&i) {
                        self.selected.push(i);
                    }
                }
            }
        }
        // The wire being drawn follows the pointer; red if the widths
        // under it do not fit.
        let pointer = ui.ctx().pointer_interact_pos();
        if let (Some(from), Some(p)) = (self.drag_wire, pointer) {
            if let Some(a) = layout.source_pos(from) {
                let (to, dir, ok) = match hover.sink {
                    Some((sink, at)) => {
                        (at, layout.sink_dir(sink), layout.fits(design, from, sink))
                    }
                    None => (p, vec2(-1.0, 0.0), true),
                };
                let color = if ok { draw::BUS } else { ERR };
                painter.add(bezier(a, to, dir, Stroke::new(2.5 * z, color)));
            }
        }
        if response.drag_stopped() {
            if let (Some(from), Some((sink, _))) = (self.drag_wire, hover.sink) {
                if layout.fits(design, from, sink) {
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
            self.select_from = None;
            self.port_moved = false;
        }
    }

    /// Clicking a chip input flips a bit, or counts up a bus.
    fn toggle_input(
        &self,
        response: &egui::Response,
        design: &Design,
        live: &Live,
        hover: &Hover,
        result: &mut CanvasOutput,
    ) {
        let (true, Some(i)) = (response.clicked(), hover.input) else {
            return;
        };
        let p = &design.def.inputs[i];
        let v = live.inputs.get(i).copied().unwrap_or(0);
        let new = if p.width == 1 {
            v ^ 1
        } else {
            v.wrapping_add(1) & mask(p.width)
        };
        result.toggled_input = Some((i, new));
    }

    /// Right-click menus: disconnect a pin, configure a part, reset ports.
    fn context_menu(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        design: &mut Design,
        lib: &Library,
        hover: &Hover,
        result: &mut CanvasOutput,
    ) {
        // The target is decided when the menu opens and kept while it stays
        // open, since the pointer moves into the menu.
        let menu_id = Id::unique("canvas_menu_target");
        if response.secondary_clicked() {
            let target = match (hover.sink, hover.part, hover.port) {
                (Some((sink, _)), _, _) => MenuTarget::Sink(sink),
                (None, Some(i), _) => MenuTarget::Part(i),
                (None, None, Some(_)) => MenuTarget::Ports,
                _ => hover.wire.map_or(MenuTarget::None, MenuTarget::Wire),
            };
            ui.ctx().data_mut(|d| d.insert_temp(menu_id, target));
        }
        response.context_menu(|ui| {
            let target = ui
                .ctx()
                .data(|d| d.get_temp::<MenuTarget>(menu_id))
                .unwrap_or(MenuTarget::None);
            match target {
                MenuTarget::Wire(wi) if wi < design.def.wires.len() => {
                    if ui.button("✂ Kabloyu kes").clicked() {
                        design.def.wires.remove(wi);
                        result.changed = true;
                        ui.close();
                    }
                }
                MenuTarget::Sink(sink) => {
                    if ui.button("✂ Kabloyu kes").clicked() {
                        design.def.wires.retain(|w| w.to != sink);
                        result.changed = true;
                        ui.close();
                    }
                }
                MenuTarget::Part(i) if i < design.def.parts.len() => {
                    let n = self.selected.len();
                    if n > 1 && self.selected.contains(&i) {
                        if ui.button(format!("🗑 Seçili {n} parçayı sil")).clicked() {
                            design.remove_parts(&self.selected);
                            self.selected.clear();
                            result.changed = true;
                            ui.close();
                            return;
                        }
                        if ui.button("Çoğalt (Ctrl+D)").clicked() {
                            if let Some(t) = design.copy(&self.selected) {
                                self.paste(design, &t, None, result);
                            }
                            ui.close();
                            return;
                        }
                        ui.separator();
                    }
                    if part_menu(ui, design, i, lib) {
                        result.changed = true;
                        result.merge = true;
                        self.selected.retain(|&s| s < design.def.parts.len());
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
                    ui.label(
                        egui::RichText::new(
                            "Seçmek için: parçaya tıkla, Shift ile ekle ya da Shift+sürükle \
                             ile kutu çiz. Ctrl+C / Ctrl+V / Ctrl+D / Delete seçimle çalışır; \
                             Ctrl+Z geri alır.",
                        )
                        .weak()
                        .small(),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Kablo kesmek için: kablonun üstüne sağ tıkla, ya da üstüne \
                             gelip Delete'e bas, ya da girişteki ucunu tutup boşluğa çek.",
                        )
                        .weak()
                        .small(),
                    );
                }
            }
        });
    }

    fn set_cursor(&self, ui: &Ui, hover: &Hover) {
        let icon = if self.drag_port.is_some() || self.drag_part.is_some() {
            egui::CursorIcon::Grabbing
        } else if hover.source.is_some() || hover.sink.is_some() {
            egui::CursorIcon::Crosshair
        } else if hover.input.is_some() || hover.wire.is_some() {
            egui::CursorIcon::PointingHand
        } else if hover.part.is_some() || hover.port.is_some() {
            egui::CursorIcon::Grab
        } else {
            return;
        };
        ui.ctx().set_cursor_icon(icon);
    }
}
