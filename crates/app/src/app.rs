//! The main window: level list, parts palette, canvas and level panel.

use std::path::PathBuf;

use egui::{Color32, RichText, Ui};
use makine_levels::{check, find, levels, Level};
use makine_sim::{Circuit, Library, PartKind, Pin, Verdict};

use crate::canvas::{format_value, Canvas, NewPart, ON};
use crate::computer::{self, Computer, Engine};
use crate::design::Design;
use crate::store::Progress;
use crate::wiki::{self, Wiki};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Editor,
    Computer,
    /// Only the computer's screen, filling the window (and the monitor, in
    /// full screen): like playing a game.
    Play,
    /// The built-in wiki.
    Wiki,
}

pub struct App {
    progress: Progress,
    save_path: Option<PathBuf>,
    level: &'static Level,
    canvas: Canvas,
    /// Library of solved chips, rebuilt when a level is passed.
    lib: Library,
    sim: Result<Circuit, String>,
    oscillating: bool,
    inputs: Vec<u64>,
    result: Option<TestResult>,
    status: Option<String>,
    /// World x of the chip's output column, from the last frame.
    output_x: f32,
    view: View,
    computer: Option<Computer>,
    /// Use the reference emulator instead of the player's CPU.
    turbo: bool,
    /// The level changed this frame: expand its section in the list.
    just_opened: bool,
    /// Full screen state before entering play mode, restored on leaving.
    play_was_fullscreen: bool,
    /// When play mode was entered (egui time), for the fading hint.
    play_started: f64,
    wiki: Wiki,
    /// Wiki images are registered with egui on the first frame.
    wiki_installed: bool,
}

enum TestResult {
    Pass,
    Fail {
        step: usize,
        inputs: Vec<u64>,
        expected: Vec<Option<u64>>,
        actual: Vec<u64>,
    },
    Oscillates {
        step: usize,
    },
    Wrong(String),
    Error(String),
}

impl App {
    pub fn new(progress: Progress, save_path: Option<PathBuf>) -> Self {
        let first_open = levels()
            .iter()
            .enumerate()
            .find(|(i, l)| progress.unlocked(*i) && !progress.solved(l.chip))
            .map_or(levels()[0], |(_, l)| *l);
        let lib = progress.library(progress.fast);
        let mut app = Self {
            progress,
            save_path,
            level: first_open,
            canvas: Canvas::new(),
            lib,
            sim: Err(String::new()),
            oscillating: false,
            inputs: Vec::new(),
            result: None,
            status: None,
            output_x: 0.0,
            view: View::Editor,
            computer: None,
            turbo: false,
            just_opened: true,
            play_was_fullscreen: false,
            play_started: 0.0,
            wiki: Wiki::default(),
            wiki_installed: false,
        };
        app.open_level(first_open);
        app
    }

    fn design(&mut self) -> &mut Design {
        let level = self.level;
        self.progress
            .designs
            .entry(level.chip.to_string())
            .or_insert_with(|| Design::new(level.blank()))
    }

    fn open_level(&mut self, level: &'static Level) {
        self.level = level;
        self.just_opened = true;
        self.canvas = Canvas::new();
        self.canvas.symbols = self.progress.symbols;
        self.inputs = vec![0; level.inputs.len()];
        self.result = None;
        // Earlier levels may have been re-solved; drop wires that no longer fit.
        let lib = self.lib.clone();
        self.design().prune(&lib);
        self.rebuild();
    }

    /// Recompile the live simulation after the design changed.
    fn rebuild(&mut self) {
        let def = self.design().def.clone();
        self.sim = Circuit::compile_def(&self.lib, &def).map_err(|e| e.to_string());
        self.run();
    }

    /// Evaluate the live simulation with the current input values. State
    /// (latches) carries over, so toggling clk steps sequential circuits.
    fn run(&mut self) {
        self.oscillating = false;
        if let Ok(sim) = &mut self.sim {
            for (pin, &v) in self.inputs.iter().enumerate() {
                sim.set_input(pin, v);
            }
            self.oscillating = sim.eval().is_err();
        }
    }

    fn save(&mut self) {
        let Some(path) = &self.save_path else { return };
        if let Err(e) = self.progress.save(path) {
            self.status = Some(format!("Kaydedilemedi: {e}"));
        }
    }

    fn run_test(&mut self) {
        let level = self.level;
        let def = self.design().def.clone();
        self.result = Some(match check(level, &self.lib, &def) {
            Err(e) => TestResult::Error(e.to_string()),
            Ok(Verdict::Pass) => {
                self.progress.solutions.insert(level.chip.to_string(), def);
                self.lib = self.progress.library(self.progress.fast);
                self.computer = None;
                self.save();
                TestResult::Pass
            }
            Ok(Verdict::Fail(f)) => TestResult::Fail {
                step: f.step,
                inputs: f.inputs,
                expected: f.expected,
                actual: f.actual,
            },
            Ok(Verdict::Oscillates { step }) => TestResult::Oscillates { step },
            Ok(Verdict::Wrong { message, .. }) => TestResult::Wrong(message),
        });
    }

    fn levels_panel(&mut self, ui: &mut Ui) {
        ui.heading("makine");
        ui.label(RichText::new("NAND'den bilgisayara").weak());
        ui.separator();
        let mut open = None;
        let all = levels();
        let mut start = 0;
        while start < all.len() {
            let section = all[start].section;
            let end = start
                + all[start..]
                    .iter()
                    .take_while(|l| l.section == section)
                    .count();
            let done = all[start..end]
                .iter()
                .filter(|l| self.progress.solved(l.chip))
                .count();
            let current = all[start..end].iter().any(|l| l.chip == self.level.chip);
            let header = format!("{section}  ({done}/{})", end - start);
            egui::CollapsingHeader::new(RichText::new(header).strong())
                .id_salt(section)
                .default_open(current)
                .open(
                    current
                        .then_some(true)
                        .filter(|_| self.view == View::Editor && self.just_opened),
                )
                .show(ui, |ui| {
                    for (i, &level) in all.iter().enumerate().take(end).skip(start) {
                        let unlocked = self.progress.unlocked(i);
                        let mark = if self.progress.solved(level.chip) {
                            "✔"
                        } else if unlocked {
                            "○"
                        } else {
                            "🔒"
                        };
                        let selected = self.level.chip == level.chip && self.view == View::Editor;
                        let label = format!("{mark}  {}", level.title);
                        let r = ui.add_enabled(unlocked, egui::Button::selectable(selected, label));
                        if r.clicked() && !selected {
                            open = Some(level);
                        }
                    }
                });
            start = end;
        }
        self.just_opened = false;
        if let Some(level) = open {
            self.view = View::Editor;
            self.open_level(level);
        }
        ui.add_space(6.0);
        ui.separator();
        if ui
            .add(egui::Button::selectable(
                self.view == View::Wiki,
                "📖  Wiki",
            ))
            .on_hover_text("Elektronik ve bilgisayar mimarisi anlatımları")
            .clicked()
        {
            self.view = View::Wiki;
        }
        let cpu_done = self.progress.solved("Cpu");
        let r = ui
            .add_enabled(
                cpu_done,
                egui::Button::selectable(self.view == View::Computer, "🖥  Bilgisayar"),
            )
            .on_disabled_hover_text("İşlemciyi bitirince açılır");
        if r.clicked() {
            self.view = View::Computer;
        }
        ui.add_space(4.0);
        let fast = ui
            .checkbox(&mut self.progress.fast, "⚡ Hızlı simülasyon")
            .on_hover_text(
                "Geçtiğin çipleri kapı kapı simüle etmek yerine aynı davranışı gösteren \
                 hızlı bir modelle çalıştır. Çipin testi geçtiği için sonuç aynıdır, \
                 sadece çok daha hızlıdır. Kapalıyken her şey NAND seviyesinde simüle edilir.",
            );
        if ui
            .checkbox(&mut self.progress.symbols, "Kapı sembolleri")
            .on_hover_text("NOT, AND, OR, XOR, NAND, MUX'u standart mantık sembolleriyle çiz")
            .changed()
        {
            self.canvas.symbols = self.progress.symbols;
            self.save();
        }
        if ui
            .checkbox(&mut self.progress.fullscreen, "Tam ekran (F11)")
            .changed()
        {
            let on = self.progress.fullscreen;
            self.set_fullscreen(ui.ctx(), on);
        }
        if fast.changed() {
            self.lib = self.progress.library(self.progress.fast);
            self.computer = None;
            self.rebuild();
            self.save();
        }
    }

    fn parts_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Parçalar").strong());
        ui.label(RichText::new("Tuvale sürükle").weak().small());
        let mut kinds: Vec<(String, PartKind, &str)> = vec![(
            "NAND".into(),
            PartKind::Nand,
            "Her şeyin temeli: iki giriş de 1 ise 0, değilse 1.",
        )];
        let solved: Vec<&Level> = self
            .level
            .allowed_chips()
            .filter(|l| self.progress.solved(l.chip))
            .collect();
        for l in &solved {
            kinds.push((l.chip.into(), PartKind::Chip(l.chip.into()), l.title));
        }
        let buses = self.level.index() >= find("Mux32").unwrap().index();
        if buses {
            kinds.push((
                "Böl".into(),
                PartKind::Split(vec![1; 8]),
                "Bus'ı parçalara ayır. Sağ tıkla: grupları ayarla.",
            ));
            kinds.push((
                "Birleştir".into(),
                PartKind::Join(vec![1; 8]),
                "Parçaları tek bus yap. Sağ tıkla: grupları ayarla.",
            ));
            kinds.push((
                "Sabit".into(),
                PartKind::Const { width: 1, value: 1 },
                "Hep aynı değeri verir. Sağ tıkla: değeri ayarla.",
            ));
        }
        for (i, (name, kind, tip)) in kinds.into_iter().enumerate() {
            let id = egui::Id::unique(("palette", i));
            ui.dnd_drag_source(id, NewPart(kind), |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(name);
                });
            })
            .response
            .on_hover_text(tip);
        }
    }

    fn level_panel(&mut self, ui: &mut Ui) {
        let level = self.level;
        ui.heading(level.title);
        ui.add_space(4.0);
        ui.label(level.description);
        if let Some(page) = wiki::page_for_level(level.chip) {
            ui.add_space(4.0);
            if ui
                .link(format!("📖 Wiki'de oku: {}", page.title()))
                .clicked()
            {
                self.wiki.open(page);
                self.view = View::Wiki;
            }
        }
        ui.add_space(8.0);

        let pins = |ps: Vec<Pin>| {
            ps.iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        ui.label(format!("Girişler: {}", pins(level.input_pins())));
        ui.label(format!("Çıkışlar: {}", pins(level.output_pins())));
        ui.separator();

        match &self.sim {
            Err(e) if !e.is_empty() => {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            Ok(sim) => {
                let text = if sim.native_count() > 0 {
                    format!(
                        "{} NAND kapısı + {} hızlı model (⚡)",
                        sim.gate_count(),
                        sim.native_count()
                    )
                } else {
                    format!("{} NAND kapısı", sim.gate_count())
                };
                ui.label(RichText::new(text).weak());
                if self.oscillating {
                    ui.colored_label(
                        Color32::LIGHT_RED,
                        "Devre salınıyor: bir geri besleme döngüsü hiç durulmuyor.",
                    );
                }
            }
            _ => {}
        }
        ui.label(
            RichText::new("Girişleri tıklayarak değiştir. Bus girişlerinde değeri aşağıdan gir.")
                .weak()
                .small(),
        );
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
        ui.horizontal(|ui| {
            if ui
                .button("Durumu sıfırla")
                .on_hover_text("Bütün mandalları 0'a çek")
                .clicked()
            {
                if let Ok(sim) = &mut self.sim {
                    sim.reset();
                }
                self.run();
            }
            if ui
                .button("Temizle")
                .on_hover_text("Bütün parçaları sil, baştan başla")
                .clicked()
            {
                *self.design() = Design::new(level.blank());
                self.rebuild();
                self.save();
            }
        });

        ui.separator();
        if ui
            .add(
                egui::Button::new(RichText::new("▶  Test et").size(16.0))
                    .min_size(egui::vec2(120.0, 32.0)),
            )
            .clicked()
        {
            self.run_test();
        }
        ui.add_space(6.0);
        self.result_view(ui);
    }

    fn result_view(&mut self, ui: &mut Ui) {
        let level = self.level;
        let Some(result) = &self.result else { return };
        match result {
            TestResult::Pass => {
                ui.colored_label(
                    ON,
                    RichText::new("✔ Tebrikler, bütün testler geçti!").strong(),
                );
                let next = levels().get(level.index() + 1);
                if let Some(&next) = next {
                    ui.label(format!(
                        "{} artık bir parça olarak kullanılabilir.",
                        level.chip
                    ));
                    if ui.button(format!("Sıradaki: {} ▶", next.title)).clicked() {
                        self.open_level(next);
                    }
                } else {
                    ui.label("İşlemcin hazır! Soldan 'Bilgisayar'ı aç ve onda program çalıştır.");
                    if ui.button("🖥  Bilgisayara git").clicked() {
                        self.view = View::Computer;
                    }
                }
            }
            TestResult::Error(e) => {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            TestResult::Wrong(m) => {
                ui.colored_label(Color32::LIGHT_RED, "✖ Test geçmedi");
                ui.label(m);
            }
            TestResult::Oscillates { step } => {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    format!("Adım {}: devre salınıyor ve hiç durulmuyor.", step + 1),
                );
            }
            TestResult::Fail {
                step,
                inputs,
                expected,
                actual,
            } => {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    format!("✖ Adım {}'de yanlış sonuç", step + 1),
                );
                if matches!(level.test, makine_levels::Test::Sequence(_)) {
                    ui.label(RichText::new("Bu bir sıralı test: adımlar art arda çalışır, devre önceki adımları hatırlar.").weak().small());
                }
                egui::Grid::new("failure").striped(true).show(ui, |ui| {
                    for (p, v) in level.input_pins().iter().zip(inputs) {
                        ui.label(&p.name);
                        ui.monospace(format_value(p.width, *v));
                        ui.end_row();
                    }
                    ui.label("");
                    ui.label(RichText::new("beklenen").weak());
                    ui.label(RichText::new("senin").weak());
                    ui.end_row();
                    for ((p, e), a) in level.output_pins().iter().zip(expected).zip(actual) {
                        let ok = e.is_none_or(|e| e == *a);
                        ui.label(&p.name);
                        ui.monospace(e.map_or("-".into(), |e| format_value(p.width, e)));
                        ui.colored_label(
                            if ok { ON } else { Color32::LIGHT_RED },
                            RichText::new(format_value(p.width, *a)).monospace(),
                        );
                        ui.end_row();
                    }
                });
                if ui.button("Bu girişleri devreye yükle").clicked() {
                    self.inputs = inputs.clone();
                    self.run();
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if !self.wiki_installed {
            wiki::install(&ctx);
            self.wiki_installed = true;
        }
        let (f11, f10) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::F11),
                i.consume_key(egui::Modifiers::NONE, egui::Key::F10),
            )
        });
        if f11 {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            self.set_fullscreen(&ctx, !full);
        }
        if self.view == View::Play {
            if f10 {
                self.leave_play(&ctx);
            } else {
                self.play_view(ui);
                return;
            }
        }
        egui::Panel::left("levels")
            .default_size(220.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("levels_scroll")
                    .show(ui, |ui| self.levels_panel(ui));
            });
        if self.view == View::Editor {
            egui::Panel::left("parts")
                .default_size(130.0)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("parts_scroll")
                        .show(ui, |ui| self.parts_panel(ui));
                });
        }
        egui::Panel::right("level")
            .default_size(320.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match self.view {
                    View::Editor => self.level_panel(ui),
                    View::Computer | View::Play => self.computer_controls(ui),
                    View::Wiki => self.wiki.contents(ui),
                });
            });
        if let Some(status) = self.status.clone() {
            egui::Panel::bottom("status").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::LIGHT_YELLOW, status);
                    if ui.small_button("×").clicked() {
                        self.status = None;
                    }
                });
            });
        }
        if self.view == View::Computer {
            egui::CentralPanel::default().show(ui, |ui| self.computer_view(ui));
            return;
        }
        if self.view == View::Wiki {
            let mut go = None;
            egui::CentralPanel::default().show(ui, |ui| go = self.wiki.show(ui));
            if let Some(chip) = go {
                let index = levels().iter().position(|l| l.chip == chip);
                match (find(chip), index) {
                    (Some(level), Some(i)) if self.progress.unlocked(i) => {
                        self.view = View::Editor;
                        self.open_level(level);
                    }
                    _ => {
                        self.status = Some(
                            "Bu seviye henüz kilitli: önceki seviyeleri bitirince açılır.".into(),
                        )
                    }
                }
            }
            return;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let output_x =
                    (ui.available_width() / self.canvas.zoom.max(0.3) - 110.0).max(400.0);
                self.output_x = output_x;
                let lib = self.lib.clone();
                let inputs = self.inputs.clone();
                let sim = self
                    .sim
                    .as_ref()
                    .ok()
                    .filter(|_| !self.oscillating)
                    .cloned();
                let level = self.level;
                let design = self
                    .progress
                    .designs
                    .entry(level.chip.to_string())
                    .or_insert_with(|| Design::new(level.blank()));
                let out = self
                    .canvas
                    .show(ui, design, &lib, sim.as_ref(), &inputs, output_x);
                if let Some((pin, v)) = out.toggled_input {
                    self.inputs[pin] = v;
                    self.run();
                }
                if out.changed {
                    self.rebuild();
                    self.save();
                }
            });
    }
}

impl App {
    fn set_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
        self.progress.fullscreen = on;
        self.save();
    }

    fn enter_play(&mut self, ctx: &egui::Context) {
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

    fn leave_play(&mut self, ctx: &egui::Context) {
        self.view = View::Computer;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.play_was_fullscreen));
    }

    fn play_view(&mut self, ui: &mut Ui) {
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
    fn computer(&mut self) -> Result<&mut Computer, String> {
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

    fn load_program(&mut self, name: &str, bytes: &[u8]) {
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

    fn computer_controls(&mut self, ui: &mut Ui) {
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

    fn computer_view(&mut self, ui: &mut Ui) {
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
fn doom_path() -> Option<std::path::PathBuf> {
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        App::ui(self, ui);
    }

    fn on_exit(&mut self) {
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_levels::reference::reference;

    /// Render the editor with a reference solution loaded and write PNGs to
    /// $MAKINE_SHOT_DIR (default /tmp/opencode) for manual inspection.
    /// Run with `--ignored`; needs a GPU (try WGPU_BACKEND=vulkan).
    #[test]
    #[ignore]
    fn screenshot() {
        type Shot = (&'static str, &'static [[f32; 2]], Vec<u64>);
        let shots: [Shot; 4] = [
            (
                "Xor",
                &[
                    [180.0, 100.0],
                    [320.0, 40.0],
                    [320.0, 180.0],
                    [460.0, 100.0],
                ],
                vec![1, 0],
            ),
            (
                "Mux",
                &[[160.0, 200.0], [300.0, 40.0], [300.0, 160.0], [460.0, 90.0]],
                vec![1, 0, 0],
            ),
            (
                "FullAdder",
                &[[180.0, 60.0], [180.0, 200.0], [380.0, 130.0]],
                vec![1, 0, 1],
            ),
            (
                "Counter32",
                &[
                    [600.0, 470.0],
                    [140.0, 330.0],
                    [140.0, 400.0],
                    [260.0, 150.0],
                    [420.0, 120.0],
                    [420.0, 260.0],
                    [260.0, 470.0],
                    [420.0, 400.0],
                    [420.0, 560.0],
                ],
                vec![0x1000, 0, 1, 0, 0],
            ),
        ];
        for (chip, pos, inputs) in shots {
            let mut progress = Progress::default();
            for l in levels().iter().take_while(|l| l.chip != chip) {
                progress.solutions.insert(l.chip.into(), reference(l.chip));
            }
            let mut d = Design::new(reference(chip));
            for (p, at) in d.pos.iter_mut().zip(pos) {
                *p = *at;
            }
            progress.designs.insert(chip.into(), d);
            let mut app = App::new(progress, None);
            app.inputs = inputs;
            app.run();
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1280.0, 720.0))
                .wgpu()
                .build_ui(move |ui| app.ui(ui));
            harness.run();
            let img = harness.render().unwrap();
            let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
            std::fs::create_dir_all(&dir).unwrap();
            img.save(format!("{dir}/makine-{chip}.png")).unwrap();
        }

        // The computer, running Doom if it has been built.
        let mut progress = Progress::default();
        for l in levels() {
            progress.solutions.insert(l.chip.into(), reference(l.chip));
        }
        let mut app = App::new(progress, None);
        app.view = View::Computer;
        // Doom if it has been built (examples/doom/build.sh), else a demo.
        if let Some(bytes) = doom_path().and_then(|p| std::fs::read(p).ok()) {
            app.load_program("Doom", &bytes);
            let c = app.computer.as_mut().unwrap();
            for ms in 0..3000 {
                c.machine.fixed_time = Some(ms);
                c.step_n(100_000).unwrap();
            }
        } else {
            let src = computer::EXAMPLES
                .iter()
                .find(|e| e.0 == "Renkler")
                .unwrap()
                .1;
            app.load_program("Renkler", src.as_bytes());
            app.computer.as_mut().unwrap().step_n(300_000).unwrap();
        }
        app.computer.as_mut().unwrap().running = false;
        let app = std::rc::Rc::new(std::cell::RefCell::new(app));
        let a = app.clone();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .wgpu()
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        harness.run_steps(3);
        let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-computer.png")).unwrap();

        // Play mode: just the screen.
        app.borrow_mut().view = View::Play;
        harness.run_steps(3);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-play.png")).unwrap();

        // Wiki pages, with their images loaded.
        for slug in ["nand", "kapilar", "hafiza", "islemci"] {
            {
                let mut a = app.borrow_mut();
                a.view = View::Wiki;
                a.wiki
                    .open(wiki::PAGES.iter().find(|p| p.slug == slug).unwrap());
            }
            // Images decode on a background thread; give them time.
            for _ in 0..20 {
                harness.run_steps(2);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let img = harness.render().unwrap();
            img.save(format!("{dir}/makine-wiki-{slug}.png")).unwrap();
        }
    }

    /// Solve the first level with the mouse only: drag a NAND from the
    /// palette, wire it up, press "Test et". Checks the level is saved.
    #[test]
    fn solve_not_with_mouse() {
        use crate::design::geometry;
        use crate::design::Port;
        use egui::Pos2;
        use egui_kittest::kittest::Queryable;
        use std::{cell::RefCell, rc::Rc};

        let dir = std::env::temp_dir().join(format!("makine-ui-{}", std::process::id()));
        let path = dir.join("p.json");
        let app = Rc::new(RefCell::new(App::new(
            Progress::default(),
            Some(path.clone()),
        )));
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 720.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run();

        let drag = |h: &mut egui_kittest::Harness, from: Pos2, to: Pos2| {
            h.drag_at(from);
            h.run();
            for i in 1..=8 {
                h.hover_at(from + (to - from) * (i as f32 / 8.0));
                h.run();
            }
            h.drop_at(to);
            h.run();
        };

        let nand = h.get_by_label("NAND").rect().center();
        drag(&mut h, nand, egui::pos2(600.0, 300.0));
        assert_eq!(app.borrow_mut().design().def.parts.len(), 1);

        let (origin, output_x, at, g) = {
            let mut app = app.borrow_mut();
            let pins = Library::new().kind_pins(&PartKind::Nand).unwrap();
            let g = geometry(&PartKind::Nand, &pins, "NAND", app.canvas.symbols);
            (app.canvas.origin, app.output_x, app.design().part_pos(0), g)
        };
        let screen = |p: Pos2| origin + p.to_vec2();
        let (in_rect, out_rect) = {
            let mut a = app.borrow_mut();
            let d = a.design();
            (
                d.port_rect(Port::Input(0), output_x),
                d.port_rect(Port::Output(0), output_x),
            )
        };
        let input = screen(in_rect.right_center());
        drag(&mut h, input, screen(at + g.inputs[0].0));
        drag(&mut h, input, screen(at + g.inputs[1].0));
        drag(
            &mut h,
            screen(at + g.outputs[0]),
            screen(out_rect.left_center()),
        );
        let wires = app.borrow_mut().design().def.wires.clone();
        assert_eq!(wires.len(), 3, "{wires:?}");

        h.get_by_label_contains("Test et").click();
        h.run();
        assert!(app.borrow().progress.solved("Not"));
        assert!(h.query_by_label_contains("Tebrikler").is_some());
        assert!(Progress::load(&path).unwrap().solved("Not"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod glyph_tests {
    /// Every non-Turkish, non-ASCII character in the UI sources must be in
    /// the bundled fonts, or egui panics when drawing it.
    #[test]
    fn ui_symbols_have_glyphs() {
        let ctx = egui::Context::default();
        let sources = [
            include_str!("app.rs"),
            include_str!("canvas.rs"),
            include_str!("computer.rs"),
            include_str!("wiki.rs"),
        ];
        let mut missing = Vec::new();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            ui.ctx().fonts_mut(|f| {
                for c in sources.iter().flat_map(|s| s.chars()) {
                    if !c.is_ascii() && !"çÇğĞıİöÖşŞüÜ".contains(c) {
                        let id = egui::FontId::proportional(14.0);
                        if !f.has_glyph(&id, c) && !missing.contains(&c) {
                            missing.push(c);
                        }
                    }
                }
            });
        });
        out.textures_delta.clear();
        assert!(missing.is_empty(), "no glyph for {missing:?}");
    }
}

#[cfg(test)]
mod play_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use makine_levels::reference::reference;
    use std::{cell::RefCell, rc::Rc};

    /// "Oyna" switches to the screen-only view and asks for full screen;
    /// F10 goes back.
    #[test]
    fn play_mode_round_trip() {
        let mut progress = Progress::default();
        for l in levels() {
            progress.solutions.insert(l.chip.into(), reference(l.chip));
        }
        progress.program = Some(computer::EXAMPLES[1].1.to_string()); // Renkler: runs forever
        let app = Rc::new(RefCell::new(App::new(progress, None)));
        app.borrow_mut().view = View::Computer;
        app.borrow_mut().turbo = true;
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(2);
        h.get_by_label_contains("Oyna").click();
        h.run_steps(2);
        assert!(app.borrow().view == View::Play);
        assert!(app.borrow().computer.as_ref().unwrap().running);
        // The side panels are gone.
        assert!(h.query_by_label_contains("Oyna").is_none());
        h.key_press(egui::Key::F10);
        h.run_steps(2);
        assert!(app.borrow().view == View::Computer);
    }

    /// Keys pressed in play mode reach the machine.
    #[test]
    fn play_mode_keys() {
        let mut progress = Progress::default();
        for l in levels() {
            progress.solutions.insert(l.chip.into(), reference(l.chip));
        }
        let app = Rc::new(RefCell::new(App::new(progress, None)));
        {
            let mut a = app.borrow_mut();
            a.turbo = true;
            a.view = View::Play;
            let c = a.computer().unwrap();
            c.running = false;
        }
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.key_press(egui::Key::ArrowUp);
        h.run_steps(2);
        let keys: Vec<_> = app
            .borrow()
            .computer
            .as_ref()
            .unwrap()
            .machine
            .keys
            .iter()
            .copied()
            .collect();
        assert!(
            keys.contains(&(makine_rv::machine::key::UP, true)),
            "{keys:?}"
        );
    }
}
