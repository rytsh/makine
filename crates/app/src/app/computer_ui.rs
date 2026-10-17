//! The computer: running programs on the player's CPU, the play mode and
//! its controls.

use egui::{Color32, RichText, Ui};

use super::{App, View};
use crate::computer::{self, Computer, Engine};

impl App {
    pub(super) fn set_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
        self.progress.fullscreen = on;
        self.save();
    }

    pub(super) fn enter_play(&mut self, ctx: &egui::Context) {
        self.view = View::Play;
        self.play_started = ctx.input(|i| i.time);
        self.play_was_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        if let Some(c) = self.computer.as_mut() {
            if c.halted.is_none() {
                c.running = true;
            }
        }
    }

    pub(super) fn leave_play(&mut self, ctx: &egui::Context) {
        self.view = View::Computer;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.play_was_fullscreen));
    }

    pub(super) fn play_view(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ui, |ui| {
                let c = match self.computer() {
                    Ok(c) => c,
                    Err(e) => {
                        ui.colored_label(Color32::LIGHT_RED, e);
                        return;
                    }
                };
                c.run_for(std::time::Duration::from_millis(14));
                c.screen_only(ui);
                if c.running {
                    ui.ctx().request_repaint();
                }
                // A hint that fades after a few seconds.
                let t = ui.input(|i| i.time) - self.play_started;
                if t < 4.0 {
                    let alpha = ((4.0 - t).min(1.0) * 200.0) as u8;
                    ui.painter().text(
                        ui.max_rect().right_top() + egui::vec2(-16.0, 16.0),
                        egui::Align2::RIGHT_TOP,
                        "F10: çık   F11: tam ekran",
                        egui::FontId::proportional(16.0),
                        Color32::from_white_alpha(alpha),
                    );
                    ui.ctx().request_repaint();
                }
            });
    }

    /// The computer, created on first use from the current CPU.
    pub(super) fn computer(&mut self) -> Result<&mut Computer, String> {
        if self.computer.is_none() {
            let engine = if self.turbo {
                Engine::Reference(makine_rv::emu::Cpu::new())
            } else {
                Computer::circuit_engine(&self.lib)?
            };
            let mut c = Computer::new(engine);
            let src = self
                .progress
                .program
                .clone()
                .unwrap_or_else(|| computer::EXAMPLES[0].1.to_string());
            let (segs, entry) = computer::parse_program("program", src.as_bytes())?;
            c.load_program(segs, entry)?;
            self.computer = Some(c);
        }
        Ok(self.computer.as_mut().unwrap())
    }

    pub(super) fn load_program(&mut self, name: &str, bytes: &[u8]) {
        let result = computer::parse_program(name, bytes).and_then(|(segs, entry)| {
            // Big programs need hundreds of millions of instructions; on the
            // circuit CPU that would take hours.
            let size: usize = segs.iter().map(|(_, b)| b.len()).sum();
            if (entry != 0 || size > 256 * 1024) && !self.turbo {
                self.turbo = true;
                self.computer = None;
                self.status = Some(format!(
                    "{name} büyük bir program; turbo açıldı. Kendi işlemcinde denemek için \
                     turboyu kapatabilirsin (çok yavaş olur)."
                ));
            }
            let c = self.computer()?;
            c.load_program(segs, entry)?;
            c.running = true;
            Ok(())
        });
        match result {
            Ok(()) => {
                // Only assembly sources are remembered.
                if let Ok(src) = std::str::from_utf8(bytes) {
                    if !bytes.starts_with(b"\x7fELF") {
                        self.progress.program = Some(src.to_string());
                        self.save();
                    }
                }
            }
            Err(e) => self.status = Some(e),
        }
    }

    pub(super) fn computer_controls(&mut self, ui: &mut Ui) {
        ui.heading("Bilgisayar");
        ui.label(
            "Senin yaptığın RISC-V işlemci, 16 MB RAM, 320×200 ekran, konsol ve klavye. \
             Bir örnek seç ya da kendi programını yükle (assembly .s ya da derlenmiş RISC-V ELF).",
        );
        ui.add_space(6.0);
        ui.label(RichText::new("Örnekler").strong());
        let mut load = None;
        ui.horizontal_wrapped(|ui| {
            for (name, src) in computer::EXAMPLES {
                if ui.button(*name).clicked() {
                    load = Some((name.to_string(), src.as_bytes().to_vec()));
                }
            }
        });
        let doom = doom_path();
        if let Some(doom) = doom.filter(|_| ui.button("👹  Doom").clicked()) {
            match std::fs::read(&doom) {
                Ok(b) => load = Some(("Doom".to_string(), b)),
                Err(e) => self.status = Some(format!("{}: {e}", doom.display())),
            }
        }
        if ui.button("📂  Dosyadan yükle…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("RISC-V programı", &["s", "S", "asm", "elf", ""])
                .pick_file()
            {
                match std::fs::read(&path) {
                    Ok(b) => load = Some((path.display().to_string(), b)),
                    Err(e) => self.status = Some(format!("{}: {e}", path.display())),
                }
            }
        }
        if let Some((name, bytes)) = load {
            self.load_program(&name, &bytes);
        }
        ui.separator();

        let mut turbo = self.turbo;
        ui.checkbox(&mut turbo, "🚀 Turbo (referans işlemci)")
            .on_hover_text(
                "Senin devren yerine doğrudan Rust ile yazılmış bir RISC-V emülatörü kullan. \
             Senin işlemcin testleri bu emülatörle karşılaştırılarak geçti; turbo, Doom gibi \
             büyük programlar için gereken yüz milyonlarca komutu saniyeler içinde çalıştırır.",
            );
        if turbo != self.turbo {
            self.turbo = turbo;
            let program = self.computer.as_ref().map(|c| (c.running,));
            self.computer = None;
            if let Err(e) = self.computer() {
                self.status = Some(e);
            }
            if let (Some((running,)), Some(c)) = (program, self.computer.as_mut()) {
                c.running = running;
            }
        }
        let engine_label = if self.turbo {
            "Motor: referans emülatör"
        } else if self.progress.fast {
            "Motor: senin işlemcin (⚡ hızlı modeller)"
        } else {
            "Motor: senin işlemcin (NAND seviyesinde, yavaş)"
        };
        ui.label(RichText::new(engine_label).weak());
        ui.add_space(6.0);

        let c = match self.computer() {
            Ok(c) => c,
            Err(e) => {
                ui.colored_label(Color32::LIGHT_RED, e);
                return;
            }
        };
        ui.horizontal(|ui| {
            let label = if c.running {
                "⏸  Durdur"
            } else {
                "▶  Çalıştır"
            };
            if ui
                .add_enabled(c.halted.is_none(), egui::Button::new(label))
                .clicked()
            {
                c.running = !c.running;
            }
            if ui
                .add_enabled(!c.running && c.halted.is_none(), egui::Button::new("Adım"))
                .clicked()
            {
                if let Err(e) = c.step_n(1) {
                    c.halted = Some(e);
                }
            }
            if ui.button("⟲  Baştan").clicked() {
                if let Err(e) = c.reset() {
                    c.halted = Some(e);
                }
            }
        });
        let mut play = false;
        if ui
            .add(
                egui::Button::new(RichText::new("🎮  Oyna (tam ekran)").size(16.0))
                    .min_size(egui::vec2(200.0, 32.0)),
            )
            .on_hover_text("Sadece ekranı göster, tam ekran yap. Çıkmak için F10.")
            .clicked()
        {
            play = true;
        }
        if play {
            self.enter_play(ui.ctx());
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Klavye: ekrana tıkla. Doom'da oklar yürür, Ctrl ateş eder, boşluk kapı açar, \
                 Enter menüde seçer, Esc menüyü açar. F11 tam ekran, oyun modundan çıkmak için F10.",
            )
            .small(),
        );
        ui.add_space(6.0);
        ui.label(RichText::new("Bellek haritası").strong());
        ui.label(
            RichText::new(
                "0x00000000  RAM (program burada başlar)\n\
                 0x10000000  konsol (yaz: karakter bas, oku: tuş)\n\
                 0x10000004  zaman (milisaniye)\n\
                 0x10000008  klavye olayı\n\
                 0x1000000C  ekranı göster\n\
                 0x11000000  ekran pikselleri (0x00RRGGBB)",
            )
            .monospace()
            .small(),
        );
    }

    pub(super) fn computer_view(&mut self, ui: &mut Ui) {
        let c = match self.computer() {
            Ok(c) => c,
            Err(e) => {
                ui.colored_label(Color32::LIGHT_RED, e);
                return;
            }
        };
        c.run_for(std::time::Duration::from_millis(12));
        c.ui(ui);
        if c.running {
            ui.ctx().request_repaint();
        }
    }
}

/// Where a built Doom is looked for: next to the executable (for a packaged
/// release), then in the source tree (for `cargo run`).
pub(super) fn doom_path() -> Option<std::path::PathBuf> {
    let next_to_exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("doom.elf")));
    let source_tree =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/doom/build/doom.elf");
    next_to_exe
        .into_iter()
        .chain([source_tree])
        .find(|p| p.exists())
}
