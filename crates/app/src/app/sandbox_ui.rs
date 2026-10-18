//! Free chips (sandbox): the list in the left panel, the chip's panel, and
//! chip files.

use egui::{RichText, Ui};
use makine_lang::{tr, trf};

use super::{App, Target, View};
use crate::sandbox::{self, PinEditor};

impl App {
    /// The sandbox section of the left panel: list of free chips and a
    /// field to create a new one.
    pub(super) fn sandbox_list(&mut self, ui: &mut Ui) {
        let n = self.progress.sandbox.chips.len();
        let editing = match &self.target {
            Target::Free(name) if self.view == View::Editor => Some(name.clone()),
            _ => None,
        };
        let mut open = None;
        egui::CollapsingHeader::new(
            RichText::new(trf!("🔧 Serbest çipler  ({n})", "🔧 Free chips  ({n})")).strong(),
        )
        .id_salt("sandbox")
        .default_open(editing.is_some())
        .show(ui, |ui| {
            ui.label(
                RichText::new(tr!(
                    "Seviye olmadan kendi çiplerini tasarla, paylaş, birbirinde kullan.",
                    "Design your own chips without levels, share them, use them in each other."
                ))
                .weak()
                .small(),
            );
            for name in self.progress.sandbox.chips.keys() {
                let selected = editing.as_deref() == Some(name.as_str());
                if ui
                    .selectable_label(selected, format!("★  {name}"))
                    .clicked()
                    && !selected
                {
                    open = Some(name.clone());
                }
            }
            ui.horizontal(|ui| {
                let l = ui.label(tr!("Yeni:", "New:")).id;
                let r = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.new_chip_name)
                            .hint_text(tr!("çip adı", "chip name"))
                            .desired_width(100.0),
                    )
                    .labelled_by(l);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button(tr!("+ Oluştur", "+ Create")).clicked() || enter {
                    let name = self.new_chip_name.trim().to_string();
                    match sandbox::check_name(&name, &self.lib, &self.progress.sandbox, None) {
                        Ok(()) => {
                            self.progress
                                .sandbox
                                .chips
                                .insert(name.clone(), sandbox::new_chip(&name));
                            self.new_chip_name.clear();
                            self.refresh_library();
                            self.save();
                            open = Some(name);
                        }
                        Err(e) => self.status = Some(e),
                    }
                }
            });
            if ui
                .button(tr!("📥  Çip dosyası aç…", "📥  Open chip file…"))
                .clicked()
            {
                self.import_package();
            }
        });
        if let Some(name) = open {
            self.open_free(name);
        }
    }

    pub(super) fn import_package(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr!("makine çipi", "makine chip"), &[sandbox::EXT])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|text| sandbox::import(&self.lib, &mut self.progress.sandbox, &text));
        match result {
            Ok(name) => {
                self.refresh_library();
                self.save();
                self.status = Some(trf!("'{name}' eklendi.", "'{name}' added."));
                self.open_free(name);
            }
            Err(e) => self.status = Some(e),
        }
    }

    pub(super) fn export_package(&mut self, name: &str) {
        let package = match sandbox::export(&self.lib, &self.progress.sandbox, name) {
            Ok(p) => p,
            Err(e) => {
                self.status = Some(e);
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr!("makine çipi", "makine chip"), &[sandbox::EXT])
            .set_file_name(format!("{name}.{}", sandbox::EXT))
            .save_file()
        else {
            return;
        };
        self.status = Some(match std::fs::write(&path, package.to_json()) {
            Ok(()) => trf!("Kaydedildi: {}", "Saved: {}", path.display()),
            Err(e) => format!("{}: {e}", path.display()),
        });
    }

    /// Right panel for a sandbox chip: name, pins, simulation, sharing.
    pub(super) fn sandbox_panel(&mut self, ui: &mut Ui) {
        let Target::Free(name) = self.target.clone() else {
            return;
        };
        ui.heading(format!("★ {name}"));
        ui.label(
            RichText::new(tr!(
                "Serbest çip: test yok, kural yok.",
                "Free chip: no test, no rules."
            ))
            .weak(),
        );
        ui.add_space(6.0);

        // Rename.
        ui.horizontal(|ui| {
            let l = ui.label(tr!("Ad", "Name")).id;
            ui.add(egui::TextEdit::singleline(&mut self.rename_to).desired_width(140.0))
                .labelled_by(l);
            if ui.button(tr!("Değiştir", "Rename")).clicked() && self.rename_to.trim() != name {
                let to = self.rename_to.trim().to_string();
                match sandbox::check_name(&to, &self.lib, &self.progress.sandbox, None) {
                    Ok(()) => {
                        self.progress.sandbox.rename(&name, &to);
                        self.target = Target::Free(to);
                        self.refresh_library();
                        self.rebuild();
                        self.save();
                    }
                    Err(e) => self.status = Some(e),
                }
            }
        });

        // Pins.
        ui.add_space(6.0);
        ui.label(RichText::new(tr!("Uçlar", "Pins")).strong());
        let def = self.design().def.clone();
        let editor = self.pin_editor.get_or_insert_with(|| PinEditor::new(&def));
        if let Some((ins, outs)) = editor.show(ui) {
            self.progress.sandbox.set_pins(&name, ins, outs);
            self.pin_editor = None;
            self.refresh_library();
            let lib = self.lib.clone();
            self.design().prune(&lib);
            self.design_changed();
        }
        ui.separator();
        self.undo_buttons(ui);

        // Live simulation.
        match &self.sim {
            Err(e) if !e.is_empty() => {
                ui.colored_label(crate::theme::BAD, e);
            }
            Ok(sim) => {
                ui.label(
                    RichText::new(trf!(
                        "{} NAND kapısı + {} hızlı model",
                        "{} NAND gates + {} fast models",
                        sim.gate_count(),
                        sim.native_count()
                    ))
                    .weak(),
                );
                if self.oscillating {
                    ui.colored_label(
                        crate::theme::BAD,
                        tr!("Devre salınıyor.", "The circuit oscillates."),
                    );
                }
            }
            _ => {}
        }
        self.bus_inputs(ui);
        ui.horizontal(|ui| {
            if ui.button(tr!("Durumu sıfırla", "Reset state")).clicked() {
                if let Ok(sim) = &mut self.sim {
                    sim.reset();
                }
                self.run();
            }
        });

        // Sharing and deleting.
        ui.separator();
        ui.label(RichText::new(tr!("Paylaş", "Share")).strong());
        ui.label(
            RichText::new(tr!(
                "Çipi, kullandığı diğer serbest çiplerle birlikte tek bir .makine dosyası \
                 olarak kaydet. Başkası 'Çip dosyası aç' ile ekleyebilir.",
                "Save the chip, with the other free chips it uses, as one .makine file. \
                 Someone else can add it with 'Open chip file'."
            ))
            .weak()
            .small(),
        );
        if ui
            .button(tr!(
                "📤  Çip dosyası olarak kaydet…",
                "📤  Save as chip file…"
            ))
            .clicked()
        {
            self.export_package(&name);
        }
        ui.add_space(10.0);
        let users = self.progress.sandbox.users_of(&name);
        let can_delete = users.is_empty();
        let r = ui.add_enabled(
            can_delete,
            egui::Button::new(tr!("🗑  Çipi sil", "🗑  Delete chip")),
        );
        let r = if can_delete {
            r
        } else {
            r.on_disabled_hover_text(trf!(
                "Önce şu çiplerden çıkar: {}",
                "Remove it from these chips first: {}",
                users.join(", ")
            ))
        };
        if r.clicked() {
            self.progress.sandbox.chips.remove(&name);
            self.refresh_library();
            self.save();
            let level = self.level;
            self.open_level(level);
        }
    }

    /// Value fields for bus inputs (1-bit inputs are toggled on the canvas).
    pub(super) fn bus_inputs(&mut self, ui: &mut Ui) {
        let def_inputs = self.design().def.inputs.clone();
        let mut changed = false;
        for (i, p) in def_inputs.iter().enumerate().filter(|(_, p)| p.width > 1) {
            ui.horizontal(|ui| {
                let label = ui.label(&p.name).id;
                let mut v = self.inputs[i];
                if ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .hexadecimal(1, false, true)
                            .prefix("0x")
                            .range(0..=makine_sim::mask(p.width)),
                    )
                    .labelled_by(label)
                    .changed()
                {
                    self.inputs[i] = v;
                    changed = true;
                }
            });
        }
        if changed {
            self.run();
        }
    }
}
