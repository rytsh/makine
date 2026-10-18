//! The computer: running programs on the player's CPU, the play mode and
//! its controls.

use egui::{Color32, RichText, Ui};
use makine_lang::{tr, trf};

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
                        ui.colored_label(crate::theme::BAD, e);
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
                        tr!("F10: çık   F11: tam ekran", "F10: leave   F11: full screen"),
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
            let engine = if self.turbo || !self.progress.solved("Cpu") {
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
            if let Ok(p) = makine_rv::asm::assemble(&src) {
                c.source = Some((src, p));
            }
            self.computer = Some(c);
        }
        Ok(self.computer.as_mut().unwrap())
    }

    pub(super) fn load_program(&mut self, name: &str, bytes: &[u8]) {
        let assembly = if !bytes.starts_with(b"\x7fELF") {
            std::str::from_utf8(bytes)
                .ok()
                .and_then(|s| makine_rv::asm::assemble(s).ok().map(|p| (s.to_string(), p)))
        } else {
            None
        };
        let result = computer::parse_program(name, bytes).and_then(|(segs, entry)| {
            // Big programs need hundreds of millions of instructions; on the
            // circuit CPU that would take hours.
            let size: usize = segs.iter().map(|(_, b)| b.len()).sum();
            if (entry != 0 || size > 256 * 1024) && !self.turbo {
                self.turbo = true;
                self.computer = None;
                self.status = Some(trf!(
                    "{name} büyük bir program; turbo açıldı. Kendi işlemcinde denemek için \
                     turboyu kapatabilirsin (çok yavaş olur).",
                    "{name} is a big program; turbo is on. To try it on your own processor, \
                     turn turbo off (it will be very slow)."
                ));
            }
            let c = self.computer()?;
            c.load_program(segs, entry)?;
            c.source = assembly;
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
        ui.heading(tr!("Bilgisayar", "Computer"));
        ui.label(tr!(
            "Senin yaptığın RISC-V işlemci, 16 MB RAM, 320×200 ekran, konsol ve klavye. \
             Bir örnek seç ya da kendi programını yükle (assembly .s ya da derlenmiş RISC-V ELF).",
            "The RISC-V processor you built, 16 MB of RAM, a 320×200 screen, a console and a \
             keyboard. Pick an example or load your own program (assembly .s or a compiled \
             RISC-V ELF)."
        ));
        ui.add_space(6.0);
        ui.label(RichText::new(tr!("Örnekler", "Examples")).strong());
        let mut load = None;
        ui.horizontal_wrapped(|ui| {
            for (name, src) in computer::EXAMPLES {
                let shown = match *name {
                    "Merhaba" => tr!("Merhaba", "Hello"),
                    "Renkler" => tr!("Renkler", "Colors"),
                    "Klavye" => tr!("Klavye", "Keyboard"),
                    n => n,
                };
                if ui.button(shown).clicked() {
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
        if ui
            .button(tr!("📂  Dosyadan yükle…", "📂  Load from file…"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(
                    tr!("RISC-V programı", "RISC-V program"),
                    &["s", "S", "asm", "elf", ""],
                )
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
        ui.checkbox(
            &mut turbo,
            tr!(
                "🚀 Turbo (referans işlemci)",
                "🚀 Turbo (reference processor)"
            ),
        )
        .on_hover_text(tr!(
            "Senin devren yerine doğrudan Rust ile yazılmış bir RISC-V emülatörü kullan. \
             Senin işlemcin testleri bu emülatörle karşılaştırılarak geçti; turbo, Doom gibi \
             büyük programlar için gereken yüz milyonlarca komutu saniyeler içinde çalıştırır.",
            "Use a RISC-V emulator written in Rust instead of your circuit. Your processor \
             passed its tests by being compared with this emulator; turbo runs the hundreds \
             of millions of instructions big programs like Doom need in seconds."
        ));
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
            tr!("Motor: referans emülatör", "Engine: reference emulator")
        } else if self.progress.fast {
            tr!(
                "Motor: senin işlemcin (⚡ hızlı modeller)",
                "Engine: your processor (⚡ fast models)"
            )
        } else {
            tr!(
                "Motor: senin işlemcin (NAND seviyesinde, yavaş)",
                "Engine: your processor (NAND level, slow)"
            )
        };
        ui.label(RichText::new(engine_label).weak());
        ui.add_space(6.0);

        if let Err(e) = self.computer() {
            ui.colored_label(crate::theme::BAD, e);
            return;
        }
        let (c, debug) = (self.computer.as_mut().unwrap(), &mut self.debug);
        ui.horizontal(|ui| {
            let label = if c.running {
                tr!("⏸  Durdur", "⏸  Pause")
            } else {
                tr!("▶  Çalıştır", "▶  Run")
            };
            if ui
                .add_enabled(c.halted.is_none(), egui::Button::new(label))
                .clicked()
            {
                c.running = !c.running;
            }
            if ui
                .add_enabled(
                    !c.running && c.halted.is_none(),
                    egui::Button::new(tr!("Adım", "Step")),
                )
                .on_hover_text(tr!("Bir komut çalıştır", "Run one instruction"))
                .clicked()
            {
                debug.before = c.registers().copied();
                if let Err(e) = c.step_n(1) {
                    c.halted = Some(e);
                }
            }
            if ui.button(tr!("⟲  Baştan", "⟲  Restart")).clicked() {
                debug.before = None;
                if let Err(e) = c.reset() {
                    c.halted = Some(e);
                }
            }
        });
        let mut play = false;
        if ui
            .add(
                egui::Button::new(
                    RichText::new(tr!("🎮  Oyna (tam ekran)", "🎮  Play (full screen)")).size(16.0),
                )
                .min_size(egui::vec2(200.0, 32.0)),
            )
            .on_hover_text(tr!(
                "Sadece ekranı göster, tam ekran yap. Çıkmak için F10.",
                "Show only the screen, in full screen. F10 to leave."
            ))
            .clicked()
        {
            play = true;
        }
        if play {
            self.enter_play(ui.ctx());
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr!(
                "Klavye: ekrana tıkla. Doom'da oklar yürür, Ctrl ateş eder, boşluk kapı açar, \
                 Enter menüde seçer, Esc menüyü açar. F11 tam ekran, oyun modundan çıkmak için F10.",
                "Keyboard: click the screen. In Doom the arrows walk, Ctrl fires, space opens \
                 doors, Enter picks in the menu, Esc opens the menu. F11 is full screen, F10 \
                 leaves play mode."
            ))
            .small(),
        );
        ui.add_space(6.0);
        ui.label(RichText::new(tr!("Bellek haritası", "Memory map")).strong());
        ui.label(
            RichText::new(tr!(
                "0x00000000  RAM (program burada başlar)\n\
                 0x10000000  konsol (yaz: karakter bas, oku: tuş)\n\
                 0x10000004  zaman (milisaniye)\n\
                 0x10000008  klavye olayı\n\
                 0x1000000C  ekranı göster\n\
                 0x11000000  ekran pikselleri (0x00RRGGBB)",
                "0x00000000  RAM (programs start here)\n\
                 0x10000000  console (write: print a char, read: a key)\n\
                 0x10000004  time (milliseconds)\n\
                 0x10000008  keyboard event\n\
                 0x1000000C  show the screen\n\
                 0x11000000  screen pixels (0x00RRGGBB)"
            ))
            .monospace()
            .small(),
        );
    }

    pub(super) fn computer_view(&mut self, ui: &mut Ui) {
        let c = match self.computer() {
            Ok(c) => c,
            Err(e) => {
                ui.colored_label(crate::theme::BAD, e);
                return;
            }
        };
        c.run_for(std::time::Duration::from_millis(12));
        let running = c.running;
        egui::ScrollArea::vertical()
            .id_salt("computer_scroll")
            .show(ui, |ui| {
                if let Some(c) = self.computer.as_mut() {
                    c.ui(ui);
                }
                self.debugger(ui);
            });
        if running {
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
