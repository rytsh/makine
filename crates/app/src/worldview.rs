//! The isometric world view: draws the block world and lets the player
//! build in it.
//!
//! Projection: world (x, y, z) maps to screen with x going down-right, y
//! down-left and z straight up, the classic 2:1 isometric grid. Layers above
//! the one being edited are hidden; layers below are drawn dimmed, so the
//! player sees what they build on.
//!
//! Controls: left click places the selected block (drag to paint) or, on a
//! device, selects it; right click removes; R turns the block being placed
//! (or the blocks being pasted); Page Up/Down or Q/E change layer;
//! middle-drag or Space+drag pans; the wheel zooms. With the select tool,
//! dragging selects a region (Ctrl+C/X/V, arrows move it, Del deletes).

use std::collections::HashMap;

use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2};
use makine_sim::Library;
use makine_world::block::{Block, Dir, Gate};
use makine_world::compile::{CompileError, Compiled, Lane};
use makine_world::device::{self, Device};

use makine_world::region::{self, Clip, Region};
use makine_world::world::MAX_LAYER;
use makine_world::{Edit, Pos, World};

use crate::worldsim::{SimError, WorldSim};

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
            "Kendi kendine yanıp söner. Seçip periyodunu ayarla.",
        ),
        (
            "Giriş portu",
            Block::Port {
                name: "a".into(),
                output: false,
                facing: Dir::East,
            },
            "Dünyayı çipe çevirince çipin girişi olur",
        ),
        (
            "Çıkış portu",
            Block::Port {
                name: "q".into(),
                output: true,
                facing: Dir::East,
            },
            "Arkasındaki teli okur; çipin çıkışı olur",
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
    /// The running circuit of the world.
    pub sim: WorldSim,
    /// Rebuilding the circuit waits for the end of a drag (large worlds).
    defer: bool,
    /// Cell under the pointer last frame.
    hover: Option<Pos>,
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
            pan: vec2(0.0, 0.0),
            zoom: 1.0,
            undo: Vec::new(),
            redo: Vec::new(),
            sim: WorldSim::default(),
            defer: false,
            hover: None,
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

    /// Tools for the selected region.
    pub fn selection_panel(&mut self, ui: &mut Ui) {
        if let Some(clip) = &self.pasting {
            ui.separator();
            ui.label(RichText::new(format!("Yapıştırılıyor: {} blok", clip.blocks.len())).strong());
            ui.horizontal(|ui| {
                if ui.button("↻ Döndür (R)").clicked() {
                    self.pasting = self.pasting.as_ref().map(Clip::rotated);
                }
                if ui.button("Bırak (Esc)").clicked() {
                    self.pasting = None;
                    self.status = None;
                }
            });
            return;
        }
        let Some(r) = self.selection else {
            if self.selecting() {
                ui.label(
                    RichText::new(
                        "Bir alan seçmek için sürükle. (Başka bir blok tutarken Shift+sürükle.)",
                    )
                    .weak()
                    .small(),
                );
            }
            return;
        };
        let n = r.blocks(&self.world).len();
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Seçim: {}×{}, {n} blok", r.width(), r.height())).strong(),
            );
            if ui
                .small_button("✖")
                .on_hover_text("Seçimi bırak (Esc)")
                .clicked()
            {
                self.selection = None;
            }
        });
        ui.label(
            RichText::new("Seçim bütün katmanları kapsar. Oklar: taşı · Del: sil · Ctrl+C/X/V")
                .weak()
                .small(),
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("Kopyala").clicked() {
                if let Some(t) = self.copy() {
                    ui.ctx().copy_text(t);
                }
            }
            if ui.button("Kes").clicked() {
                if let Some(t) = self.cut() {
                    ui.ctx().copy_text(t);
                }
            }
            if ui
                .add_enabled(self.clipboard.is_some(), egui::Button::new("Yapıştır"))
                .clicked()
            {
                self.start_paste(None);
            }
            if ui.button("Sil").clicked() {
                self.delete_selection();
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Çipe çevir").strong());
        ui.label(
            RichText::new(
                "Seçimdeki portlar çipin uçları olur. x0, x1, … gibi sıralı adlar tek bir \
                 bus'a birleşir. Anahtarlar o anki değerleriyle sabit olur.",
            )
            .weak()
            .small(),
        );
        ui.horizontal(|ui| {
            let l = ui.label("Ad").id;
            ui.add(
                egui::TextEdit::singleline(&mut self.chip_name)
                    .hint_text("Toplayici")
                    .desired_width(110.0),
            )
            .labelled_by(l);
            if ui.button("Çip yap").clicked() {
                self.request = Some(Request::MakeChip {
                    name: self.chip_name.trim().to_string(),
                    region: r,
                });
            }
        });
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

    /// Settings of a selected clock (period) or port (name).
    pub fn block_panel(&mut self, ui: &mut Ui) {
        let Some(pos) = self.selected_block else {
            return;
        };
        let Some(block) = self.world.get(pos).cloned() else {
            self.selected_block = None;
            return;
        };
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Seçili: {}", block.label())).strong());
            if ui
                .small_button("✖")
                .on_hover_text("Seçimi bırak (Esc)")
                .clicked()
            {
                self.selected_block = None;
            }
        });
        let new = match &block {
            Block::Clock { period, facing } => {
                let mut p = *period;
                ui.horizontal(|ui| {
                    let l = ui.label("Periyot (adım)").id;
                    ui.add(egui::DragValue::new(&mut p).range(1..=100_000))
                        .labelled_by(l);
                });
                (p != *period).then_some(Block::Clock {
                    period: p,
                    facing: *facing,
                })
            }
            Block::Port {
                name,
                output,
                facing,
            } => {
                let mut n = name.clone();
                ui.horizontal(|ui| {
                    let l = ui.label("Ad").id;
                    ui.add(egui::TextEdit::singleline(&mut n).desired_width(80.0))
                        .labelled_by(l);
                });
                ui.label(
                    RichText::new("x0, x1, … gibi sıralı adlar çipte tek bir bus olur.")
                        .weak()
                        .small(),
                );
                let n = n.trim().to_string();
                let valid = !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_');
                (valid && n != *name).then_some(Block::Port {
                    name: n,
                    output: *output,
                    facing: *facing,
                })
            }
            _ => None,
        };
        if let Some(b) = new {
            self.edit(Edit::Set {
                pos,
                block: Some(b),
            });
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
            Device::Keyboard | Device::Chip { .. } => {}
        }
        if let Some(new) = changed {
            if let Err(e) = self.replace_device(anchor, new) {
                self.status = Some(e);
            }
            return;
        }

        match &d {
            Device::Rom { addr, data, words } => {
                // Refresh the text when another ROM is selected or its
                // contents changed (undo, another player).
                let fresh = device::words_text(words, *data);
                let stale = self.rom_text.as_ref().is_none_or(|(a, _, shown, _)| {
                    *a != anchor || shown.as_deref() != Some(fresh.as_str())
                });
                if stale {
                    self.rom_text = Some((anchor, fresh.clone(), Some(fresh), None));
                }
                let l = ui
                    .label(
                        RichText::new(format!(
                            "İçerik: {} kelime, onaltılık (ya da #onluk), ; yorum",
                            1u32 << addr
                        ))
                        .small(),
                    )
                    .id;
                let (_, text, _, err) = self.rom_text.as_mut().unwrap();
                ui.add(
                    egui::TextEdit::multiline(text)
                        .code_editor()
                        .desired_rows(6)
                        .desired_width(f32::INFINITY),
                )
                .labelled_by(l);
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
        if self.sharing {
            self.outbox.push(e.clone());
        }
        let inv = self.world.apply(e);
        self.undo.push(inv);
        self.redo.clear();
        if self.undo.len() > 500 {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(e) = self.undo.pop() {
            if self.sharing {
                self.outbox.push(e.clone());
            }
            let inv = self.world.apply(e);
            self.redo.push(inv);
        }
    }

    pub fn redo(&mut self) {
        if let Some(e) = self.redo.pop() {
            if self.sharing {
                self.outbox.push(e.clone());
            }
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
            ui.input(|i| {
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

        // Pasting: a click puts the blocks down; right click stops.
        let shift = ui.input(|i| i.modifiers.shift);
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
                self.sel_start = press.map(|p| self.pick(origin, p, self.layer));
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

        // Large worlds recompile only when the pointer is released, so
        // painting stays smooth.
        let held = ui.input(|i| i.pointer.any_down());
        self.defer = held && self.sim.slow();

        // Simulate and draw.
        self.sim.run_frame(&self.world, self.defer);
        let screens = self.screens();
        self.draw(&painter, origin, rect, &screens);
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

    fn draw(
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
    fn screens(&mut self) -> HashMap<Pos, Vec<bool>> {
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
            v.sim.run_frame(&v.world, false);
            let o = v.sim.compiled().unwrap().lamps[0].1.unwrap();
            lit.push(v.sim.values()[o]);
        }
        // Steps 1..=8 with period 2: high on steps 2-3 and 6-7.
        assert_eq!(lit, vec![0, 1, 1, 0, 0, 1, 1, 0]);
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
            let v = view.borrow();
            let values = v.sim.values();
            let d = v
                .sim
                .compiled()
                .unwrap()
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
        v.sim.run_frame(&v.world, false);
        let name = v.sim.native_at(Pos::new(0, 0, 0)).unwrap();
        {
            let sim = v.sim.running_circuit().unwrap();
            let r: &mut makine_sim::Clocked<device::RamState> = sim.native_mut(&name).unwrap();
            r.state.words[5] = 9;
        }
        v.edit(Edit::Set {
            pos: Pos::new(20, 0, 0),
            block: Some(Block::Lamp),
        });
        v.sim.run_frame(&v.world, false);
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

#[cfg(test)]
mod selection_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    fn press(h: &mut egui_kittest::Harness, p: Pos2, button: egui::PointerButton, down: bool) {
        h.event(egui::Event::PointerButton {
            pos: p,
            button,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        });
    }

    /// Drag-select, copy, paste somewhere else, move with arrows, delete.
    #[test]
    fn select_copy_paste_move() {
        let mut w = World::new("s");
        for x in 0..3 {
            w.apply(Edit::Set {
                pos: Pos::new(x, 0, 0),
                block: Some(Block::Wire),
            });
        }
        w.apply(Edit::Set {
            pos: Pos::new(1, 1, 1),
            block: Some(Block::Lamp),
        });
        let mut v = WorldView::new(w);
        v.brush = None;
        let view = Rc::new(RefCell::new(v));
        let vv = view.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(move |ui| vv.borrow_mut().show(ui));
        h.run();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let origin = rect.center_top() + vec2(0.0, 80.0);
        let cell = |v: &WorldView, x: i32, y: i32| {
            v.to_screen(origin, x as f32 + 0.5, y as f32 + 0.5, 0.0)
        };

        // Drag from (0,0) to (2,1).
        let (a, b) = (cell(&view.borrow(), 0, 0), cell(&view.borrow(), 2, 1));
        h.event(egui::Event::PointerMoved(a));
        press(&mut h, a, egui::PointerButton::Primary, true);
        h.run();
        for t in 1..=4 {
            h.event(egui::Event::PointerMoved(a + (b - a) * (t as f32 / 4.0)));
            h.run();
        }
        press(&mut h, b, egui::PointerButton::Primary, false);
        h.run();
        let sel = view.borrow().selection.unwrap();
        assert_eq!(sel, Region::between(Pos::new(0, 0, 0), Pos::new(2, 1, 0)));
        assert_eq!(sel.blocks(&view.borrow().world).len(), 4, "all layers");

        // Copy, then paste at (0, 5) by clicking there.
        h.event(egui::Event::Copy);
        h.run();
        assert!(view.borrow().clipboard.is_some());
        let text = view.borrow().clipboard.as_ref().unwrap().to_json();
        h.event(egui::Event::Paste(text));
        h.run();
        assert!(view.borrow().pasting.is_some());
        let p = cell(&view.borrow(), 0, 5);
        h.event(egui::Event::PointerMoved(p));
        h.run();
        press(&mut h, p, egui::PointerButton::Primary, true);
        press(&mut h, p, egui::PointerButton::Primary, false);
        h.run();
        let v = view.borrow();
        assert_eq!(v.world.len(), 8);
        assert_eq!(v.world.get(Pos::new(1, 6, 1)), Some(&Block::Lamp));
        drop(v);

        // The pasted copy is selected: move it right with the arrow key.
        h.key_press(egui::Key::ArrowRight);
        h.run();
        let v = view.borrow();
        assert_eq!(
            v.world.get(Pos::new(2, 6, 1)),
            Some(&Block::Lamp),
            "sel={:?} status={:?} blocks={:?}",
            v.selection,
            v.status,
            v.world.blocks().map(|(p, _)| p).collect::<Vec<_>>()
        );
        assert!(v.world.get(Pos::new(0, 5, 0)).is_none());
        drop(v);

        // Delete removes it.
        h.key_press(egui::Key::Delete);
        h.run();
        assert_eq!(view.borrow().world.len(), 4);
        // Undo brings it back in one step.
        view.borrow_mut().undo();
        assert_eq!(view.borrow().world.len(), 8);
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    /// The side panels of the world view, as the app shows them.
    fn panels(ui: &mut Ui, v: &mut WorldView) {
        v.block_panel(ui);
        v.device_panel(ui);
    }

    fn harness(v: WorldView) -> egui_kittest::Harness<'static, WorldView> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(500.0, 600.0))
            .build_ui_state(panels, v)
    }

    /// Ports come from the palette and are renamed in their panel; then the
    /// world becomes a chip with that pin. A clock's period is set the same
    /// way.
    #[test]
    fn ports_and_clocks_are_editable() {
        let mut v = WorldView::new(World::new("p"));
        let brush = |name: &str| {
            palette()
                .into_iter()
                .find(|(n, ..)| *n == name)
                .map(|(_, b, _)| Brush { block: b })
        };
        v.brush = brush("Giriş portu");
        v.place(Pos::new(0, 0, 0), false);
        v.brush = Some(Brush {
            block: Block::Gate {
                gate: Gate::Not,
                facing: Dir::East,
            },
        });
        v.place(Pos::new(1, 0, 0), false);
        v.brush = brush("Çıkış portu");
        v.place(Pos::new(2, 0, 0), false);
        v.brush = brush("Saat");
        v.place(Pos::new(0, 3, 0), false);
        // Clicking a port selects it for its settings.
        v.place(Pos::new(2, 0, 0), true);
        assert_eq!(v.selected_block, Some(Pos::new(2, 0, 0)));

        let mut h = harness(v);
        h.run();
        h.get_by_label("Ad").focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_label("Ad").type_text("ters");
        h.run_steps(2);
        let ports = Region::between(Pos::new(0, 0, 0), Pos::new(2, 0, 0));
        let mut part = World::new("Ters");
        part.apply(
            Clip::copy(&h.state().world, ports)
                .unwrap()
                .paste(&part, Pos::new(0, 0, 0))
                .unwrap(),
        );
        let chip = makine_world::chip::to_chip(&part, "Ters", &Library::new()).unwrap();
        assert_eq!(chip.outputs[0].name, "ters");
        // An invalid name is not applied.
        h.get_by_label("Ad").type_text(" x");
        h.run_steps(2);
        assert!(matches!(
            h.state().world.get(Pos::new(2, 0, 0)),
            Some(Block::Port { name, .. }) if name == "ters"
        ));

        h.state_mut().place(Pos::new(0, 3, 0), true);
        h.run();
        h.get_by_label("Periyot (adım)").focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_label("Periyot (adım)").type_text("7");
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        assert!(matches!(
            h.state().world.get(Pos::new(0, 3, 0)),
            Some(Block::Clock { period: 7, .. })
        ));
    }

    /// R turns what is being pasted, not the brush.
    #[test]
    fn r_turns_the_paste() {
        let mut w = World::new("r");
        for x in 0..3 {
            w.apply(Edit::Set {
                pos: Pos::new(x, 0, 0),
                block: Some(Block::Wire),
            });
        }
        let mut v = WorldView::new(w);
        v.pasting = Clip::copy(
            &v.world,
            Region::between(Pos::new(0, 0, 0), Pos::new(2, 0, 0)),
        );
        let f = v.facing;
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 400.0))
            .build_ui_state(|ui, v: &mut WorldView| v.show(ui), v);
        h.run();
        h.hover_at(egui::pos2(300.0, 200.0));
        h.run();
        h.key_press(egui::Key::R);
        h.run();
        assert_eq!(h.state().pasting.as_ref().unwrap().size(), (1, 3));
        assert_eq!(h.state().facing, f, "facing unchanged while pasting");
    }

    /// The ROM text box follows the ROM when someone else changes it.
    #[test]
    fn rom_text_follows_contents() {
        let mut v = WorldView::new(World::new("r"));
        let rom = Device::Rom {
            addr: 1,
            data: 8,
            words: vec![0xAB, 0xCD],
        };
        let e = v
            .world
            .place_device(rom, Pos::new(0, 0, 0), Dir::East)
            .unwrap();
        v.edit(e);
        v.selected = Some(Pos::new(0, 0, 0));
        let mut h = harness(v);
        h.run();
        assert_eq!(h.state().rom_text.as_ref().unwrap().1, "AB CD");
        {
            let v = h.state_mut();
            let e = v.world.remove(Pos::new(0, 0, 0)).unwrap();
            v.apply_remote(e);
            let new = Device::Rom {
                addr: 1,
                data: 8,
                words: vec![0x12, 0x34],
            };
            let e = v
                .world
                .place_device(new, Pos::new(0, 0, 0), Dir::East)
                .unwrap();
            v.apply_remote(e);
            v.selected = Some(Pos::new(0, 0, 0));
        }
        h.run();
        assert_eq!(h.state().rom_text.as_ref().unwrap().1, "12 34");
    }
}
