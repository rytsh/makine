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

use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2};
use makine_sim::Circuit;
use makine_world::block::{Block, Dir, Gate};
use makine_world::compile::{compile, Compiled, InputSource, Lane};
use makine_world::device::{self, Device};
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
        (
            "Düz tel",
            Block::Straight { facing: Dir::East },
            "Sadece önüne ve arkasına bağlanır: yan yana döşeyince bus olur",
        ),
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

/// Device entries of the build bar.
pub fn device_palette() -> Vec<(&'static str, Device, &'static str)> {
    vec![
        (
            "RAM",
            Device::Ram { addr: 4, data: 8 },
            "Okunup yazılabilen bellek",
        ),
        (
            "ROM",
            Device::Rom {
                addr: 4,
                data: 8,
                words: Vec::new(),
            },
            "İçeriği senin yazdığın sabit bellek",
        ),
        (
            "Ekran",
            Device::Screen { bits: 3 },
            "Siyah beyaz piksel ekran",
        ),
        ("Klavye", Device::Keyboard, "Yazdığın tuşları devreye verir"),
        (
            "Gösterge",
            Device::Display { bits: 8 },
            "Tellerdeki sayıyı gösterir",
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
    /// Anchor of the selected device.
    pub selected: Option<Pos>,
    /// Keys typed while a keyboard is selected, not yet delivered.
    typed: Vec<u8>,
    /// A message for the player (e.g. why a device does not fit).
    pub status: Option<String>,
    /// ROM contents being edited: device anchor, text, parse error.
    rom_text: Option<(Pos, String, Option<String>)>,
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
            selected: None,
            typed: Vec::new(),
            status: None,
            rom_text: None,
        }
    }

    /// Device buttons of the build bar.
    pub fn device_palette_ui(&mut self, ui: &mut Ui) {
        let current = match self.brush.as_ref().map(|b| &b.block) {
            Some(Block::Device { device, .. }) => Some(std::mem::discriminant(device)),
            _ => None,
        };
        ui.horizontal_wrapped(|ui| {
            for (name, device, tip) in device_palette() {
                let selected = current == Some(std::mem::discriminant(&device));
                if ui
                    .selectable_label(selected, name)
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.brush = Some(Brush {
                        block: Block::Device {
                            device,
                            facing: Dir::East,
                        },
                    });
                }
            }
        });
        if current.is_some() {
            ui.label(
                RichText::new(
                    "Tıklayarak yerleştir. Girişler arka kenarda, çıkışlar ön kenarda: \
                     yan yana bağlamak için 'Düz tel' kullan.",
                )
                .weak()
                .small(),
            );
        }
    }

    /// Settings of the selected device.
    pub fn device_panel(&mut self, ui: &mut Ui) {
        let Some((anchor, d, _)) = self.selected_device() else {
            return;
        };
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Seçili: {}", d.name())).strong());
            if ui
                .small_button("✖")
                .on_hover_text("Seçimi bırak (Esc)")
                .clicked()
            {
                self.selected = None;
            }
        });
        ui.label(RichText::new(d.help()).weak().small());
        let pins = |names: Vec<String>| names.join(" ");
        ui.label(RichText::new(format!("Girişler: {}", pins(d.inputs()))).small());
        if !d.outputs().is_empty() {
            ui.label(RichText::new(format!("Çıkışlar: {}", pins(d.outputs()))).small());
        }

        // Size.
        let mut changed: Option<Device> = None;
        let size = |ui: &mut Ui, label: &str, v: u8, lo: u8, hi: u8| -> Option<u8> {
            let mut x = v;
            let r = ui
                .horizontal(|ui| {
                    let l = ui.label(label).id;
                    ui.add(egui::DragValue::new(&mut x).range(lo..=hi))
                        .labelled_by(l)
                })
                .inner;
            (r.changed() && x != v).then_some(x)
        };
        match &d {
            Device::Ram { addr, data } => {
                if let Some(a) = size(ui, "Adres biti", *addr, 1, device::MAX_ADDR) {
                    changed = Some(Device::Ram {
                        addr: a,
                        data: *data,
                    });
                }
                if let Some(b) = size(ui, "Veri biti", *data, 1, device::MAX_DATA) {
                    changed = Some(Device::Ram {
                        addr: *addr,
                        data: b,
                    });
                }
            }
            Device::Rom { addr, data, words } => {
                if let Some(a) = size(ui, "Adres biti", *addr, 1, device::MAX_ADDR) {
                    changed = Some(Device::Rom {
                        addr: a,
                        data: *data,
                        words: words.clone(),
                    });
                }
                if let Some(b) = size(ui, "Veri biti", *data, 1, device::MAX_DATA) {
                    changed = Some(Device::Rom {
                        addr: *addr,
                        data: b,
                        words: words.clone(),
                    });
                }
            }
            Device::Screen { bits } => {
                if let Some(b) = size(ui, "Boyut biti", *bits, 2, device::MAX_SCREEN_BITS) {
                    changed = Some(Device::Screen { bits: b });
                }
            }
            Device::Display { bits } => {
                if let Some(b) = size(ui, "Bit sayısı", *bits, 1, device::MAX_DATA) {
                    changed = Some(Device::Display { bits: b });
                }
            }
            Device::Keyboard => {}
        }
        if let Some(new) = changed {
            if let Err(e) = self.replace_device(anchor, new) {
                self.status = Some(e);
            }
            return;
        }

        match &d {
            Device::Rom { addr, data, words } => {
                if self.rom_text.as_ref().is_none_or(|(a, ..)| *a != anchor) {
                    self.rom_text = Some((anchor, device::words_text(words, *data), None));
                }
                ui.label(
                    RichText::new(format!(
                        "İçerik: {} kelime, onaltılık (ya da #onluk), ; yorum",
                        1u32 << addr
                    ))
                    .small(),
                );
                let (_, text, err) = self.rom_text.as_mut().unwrap();
                ui.add(
                    egui::TextEdit::multiline(text)
                        .code_editor()
                        .desired_rows(6)
                        .desired_width(f32::INFINITY),
                );
                let mut apply = None;
                if ui.button("İçeriği yükle").clicked() {
                    match device::parse_words(text) {
                        Ok(w) if w.len() > 1 << addr => {
                            *err = Some(format!("{} kelime fazla", w.len() - (1 << addr)))
                        }
                        Ok(w) => {
                            *err = None;
                            apply = Some(w);
                        }
                        Err(e) => *err = Some(e),
                    }
                }
                if let Some(e) = err {
                    ui.colored_label(Color32::LIGHT_RED, e.as_str());
                }
                if let Some(words) = apply {
                    let new = Device::Rom {
                        addr: *addr,
                        data: *data,
                        words,
                    };
                    if let Err(e) = self.replace_device(anchor, new) {
                        self.status = Some(e);
                    }
                }
            }
            Device::Ram { data, .. } => {
                if let Some(words) = self.ram_words(anchor) {
                    ui.label(RichText::new("İçerik").small());
                    let text = device::words_text(&words, *data);
                    ui.label(RichText::new(text).monospace().small());
                }
            }
            Device::Keyboard => {
                ui.colored_label(
                    Color32::LIGHT_GREEN,
                    "⌨ Yazdıkların bu klavyeye gidiyor. Bitirince Esc.",
                );
            }
            _ => {}
        }
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
        let name = self.native_of(anchor)?;
        let (_, sim) = self.circuit()?;
        device::ram(sim, &name)
    }

    fn native_of(&mut self, anchor: Pos) -> Option<String> {
        let (c, _) = self.circuit()?;
        c.devices
            .iter()
            .find(|d| d.anchor == anchor)?
            .native
            .clone()
    }

    /// Set a screen's pixels directly (for screenshots).
    #[cfg(test)]
    pub fn set_screen_for_test(&mut self, anchor: Pos, f: impl Fn(usize, usize) -> bool) {
        let name = self.native_of(anchor).unwrap();
        let (_, sim) = self.circuit().unwrap();
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
    /// Device contents (memory, screen, keys) carry over to the new circuit.
    fn circuit(&mut self) -> Option<&mut (Compiled, Circuit)> {
        if self.compiled_rev != self.world.revision() {
            self.compiled_rev = self.world.revision();
            let mut saved = Vec::new();
            if let Some(Ok((c, sim))) = &mut self.compiled {
                for name in c.devices.iter().filter_map(|d| d.native.as_ref()) {
                    if let Some(s) = device::save(sim, name) {
                        saved.push((name.clone(), s));
                    }
                }
            }
            self.compiled = Some(
                compile(&self.world)
                    .map_err(|e| e.to_string())
                    .and_then(|c| {
                        let mut sim = Circuit::compile_def(&c.library(), &c.def)
                            .map_err(|e| e.to_string())?;
                        for (name, s) in saved {
                            device::restore(&mut sim, &name, s);
                        }
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
        let typed = std::mem::take(&mut self.typed);
        let keyboard = self.selected.and_then(|a| self.native_of(a));
        let Some((c, sim)) = self.circuit() else {
            return Vec::new();
        };
        if let Some(k) = keyboard {
            for key in typed {
                device::press(sim, &k, key);
            }
        }
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
                self.typed.extend(keys);
            }
        } else if response.hovered() || response.has_focus() {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.selected = None;
            }
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
                    } else if response.dragged_by(egui::PointerButton::Secondary) {
                        self.painted = Some(start);
                        self.erase(start);
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
                    self.erase(cell);
                }
            }
        }

        // Simulate and draw.
        let values = self.step();
        let screens = self.screens();
        self.draw(&painter, origin, rect, &values, &screens);
        if self.running
            && self
                .world
                .blocks()
                .any(|(_, b)| matches!(b, Block::Clock { .. }))
            || self.typing()
        {
            ui.ctx().request_repaint();
        }
    }

    /// Place the brush at `cell`. A click on a switch with the switch brush
    /// (or any click on a switch while not holding one) toggles it.
    fn erase(&mut self, cell: Pos) {
        if let Some(e) = self.world.remove(cell) {
            if self.world.device_at(cell).map(|d| d.0) == self.selected {
                self.selected = None;
            }
            self.edit(e);
        }
    }

    fn place(&mut self, cell: Pos, click: bool) {
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

    fn draw(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        clip: Rect,
        values: &[u64],
        screens: &HashMap<Pos, Vec<bool>>,
    ) {
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
    fn screens(&mut self) -> HashMap<Pos, Vec<bool>> {
        let Some(Ok((c, sim))) = &mut self.compiled else {
            return HashMap::new();
        };
        c.devices
            .iter()
            .filter(|d| matches!(d.device, Device::Screen { .. }))
            .filter_map(|d| Some((d.anchor, device::screen(sim, d.native.as_ref()?)?)))
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

#[cfg(test)]
mod device_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    /// A keyboard wired straight into a display: select the keyboard, type
    /// a letter, the display shows its code (plus `ready` in bit 7).
    #[test]
    fn type_into_keyboard_and_read_display() {
        let mut w = World::new("k");
        let e = w
            .place_device(Device::Keyboard, Pos::new(0, 0, 0), Dir::East)
            .unwrap();
        w.apply(e);
        for y in 0..8 {
            w.apply(Edit::Set {
                pos: Pos::new(3, y, 0),
                block: Some(Block::Straight { facing: Dir::East }),
            });
        }
        let e = w
            .place_device(Device::Display { bits: 8 }, Pos::new(4, 0, 0), Dir::East)
            .unwrap();
        w.apply(e);
        let view = Rc::new(RefCell::new(WorldView::new(w)));
        view.borrow_mut().selected = Some(Pos::new(0, 0, 0));
        let v = view.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(move |ui| v.borrow_mut().show(ui));
        h.run_steps(2);
        assert!(view.borrow().typing());
        h.event(egui::Event::Text("A".into()));
        h.run_steps(2);
        let shown = {
            let mut v = view.borrow_mut();
            let values = v.step();
            let (c, _) = v.circuit().unwrap();
            let d = c
                .devices
                .iter()
                .find(|d| matches!(d.device, Device::Display { .. }))
                .unwrap();
            d.inputs
                .iter()
                .enumerate()
                .map(|(i, o)| o.map_or(0, |o| values[o] << i))
                .sum::<u64>()
        };
        assert_eq!(shown, 0x80 | b'A' as u64);
        // Esc leaves the keyboard.
        h.key_press(egui::Key::Escape);
        h.run();
        assert!(!view.borrow().typing());
    }

    /// Placing a device by clicking, resizing it from the panel, and
    /// keeping RAM contents across unrelated edits.
    #[test]
    fn place_resize_and_keep_memory() {
        let mut v = WorldView::new(World::new("t"));
        v.brush = Some(Brush {
            block: Block::Device {
                device: Device::Ram { addr: 2, data: 4 },
                facing: Dir::East,
            },
        });
        v.place(Pos::new(0, 0, 0), true);
        assert_eq!(v.selected, Some(Pos::new(0, 0, 0)));
        // Clicking a device selects it instead of painting.
        v.brush = Some(Brush { block: Block::Wire });
        v.selected = None;
        v.place(Pos::new(1, 1, 0), true);
        assert_eq!(v.selected, Some(Pos::new(0, 0, 0)));
        let n = v.world.len();
        v.replace_device(Pos::new(0, 0, 0), Device::Ram { addr: 3, data: 4 })
            .unwrap();
        assert!(v.world.len() > n);
        // Write into the RAM directly, edit elsewhere, the word is kept.
        v.step();
        let name = v.native_of(Pos::new(0, 0, 0)).unwrap();
        {
            let (_, sim) = v.circuit().unwrap();
            let r: &mut makine_sim::Clocked<device::RamState> = sim.native_mut(&name).unwrap();
            r.state.words[5] = 9;
        }
        v.edit(Edit::Set {
            pos: Pos::new(20, 0, 0),
            block: Some(Block::Lamp),
        });
        v.step();
        assert_eq!(v.ram_words(Pos::new(0, 0, 0)).unwrap()[5], 9);
        // A device that does not fit is refused with a message.
        v.brush = Some(Brush {
            block: Block::Device {
                device: Device::Keyboard,
                facing: Dir::East,
            },
        });
        v.place(Pos::new(19, 0, 0), true);
        assert!(v.status.is_some());
        // Erasing any cell removes the whole device.
        v.erase(Pos::new(2, 3, 0));
        assert_eq!(v.world.len(), 1);
        assert_eq!(v.selected, None);
    }
}
