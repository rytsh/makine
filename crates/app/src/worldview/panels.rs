//! The world view's side panels: the build palette and the settings of
//! what is selected (a region, a clock or port, a device).

use egui::{Color32, RichText, Ui};
use makine_world::block::{Block, Dir, Gate};
use makine_world::device::{self, Device};
use makine_world::region::Clip;
use makine_world::Edit;

use super::{Brush, Request, WorldView};

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
        (
            "Lamba",
            Block::Lamp,
            "Komşu tellerden biri 1 ise yanar; telleri birbirine bağlamaz",
        ),
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

impl WorldView {
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
}
