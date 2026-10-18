//! The main window: level list, parts palette, canvas and level panel.

use std::path::PathBuf;

use egui::{RichText, Ui};
use makine_levels::{check, find, levels, Level};
use makine_sim::{Circuit, Library, PartKind, Pin, Verdict};

use crate::canvas::{format_value, Canvas, NewPart, ON};
use crate::computer::Computer;
use crate::design::Design;
use crate::sandbox::{self, PinEditor};
use crate::store::Progress;
use crate::wiki::{self, Wiki};

mod computer_ui;
mod sandbox_ui;
mod solution_ui;
#[cfg(test)]
mod tests;
mod world_ui;

#[cfg(test)]
use computer_ui::doom_path;

/// What the editor is working on.
#[derive(Clone)]
enum Target {
    Level(&'static Level),
    /// A sandbox chip, by name.
    Free(String),
}

impl PartialEq for Target {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Target::Level(a), Target::Level(b)) => a.chip == b.chip,
            (Target::Free(a), Target::Free(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Editor,
    Computer,
    /// Only the computer's screen, filling the window (and the monitor, in
    /// full screen): like playing a game.
    Play,
    /// The built-in wiki.
    Wiki,
    /// The block world, seen from above layer by layer.
    World,
}

pub struct App {
    progress: Progress,
    save_path: Option<PathBuf>,
    /// Changes not yet written to disk, since when.
    unsaved_since: Option<std::time::Instant>,
    /// The last level opened; the editor shows it when `target` is a level.
    level: &'static Level,
    target: Target,
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
    /// The open block world, if any.
    world: Option<crate::worldview::WorldView>,
    /// Sandbox UI state.
    new_chip_name: String,
    rename_to: String,
    pin_editor: Option<PinEditor>,
    /// World list UI state.
    world_rename: String,
    confirm_delete_world: bool,
    /// Building together on the local network.
    session: Option<crate::session::Session>,
    net_name: String,
    net_addr: String,
    /// How the player's design compares with the example solution.
    cost: Option<makine_levels::cost::Cost>,
    /// The example solution, when open instead of the player's design.
    solution: Option<solution_ui::Solution>,
    reveal: solution_ui::Reveal,
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
            unsaved_since: None,
            level: first_open,
            target: Target::Level(first_open),
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
            world: None,
            new_chip_name: String::new(),
            rename_to: String::new(),
            pin_editor: None,
            world_rename: String::new(),
            confirm_delete_world: false,
            session: None,
            net_name: crate::session::default_name(),
            net_addr: String::new(),
            cost: None,
            solution: None,
            reveal: Default::default(),
        };
        app.open_level(first_open);
        app
    }

    fn design(&mut self) -> &mut Design {
        design_for(&mut self.progress, &self.target)
    }

    fn open_level(&mut self, level: &'static Level) {
        self.level = level;
        self.just_opened = true;
        self.open_target(Target::Level(level));
    }

    fn open_free(&mut self, name: String) {
        self.view = View::Editor;
        self.rename_to = name.clone();
        self.pin_editor = None;
        self.open_target(Target::Free(name));
    }

    fn open_target(&mut self, target: Target) {
        self.target = target;
        self.solution = None;
        self.reveal = Default::default();
        self.canvas = Canvas::new();
        self.canvas.symbols = self.progress.symbols;
        self.inputs = vec![0; self.design().def.inputs.len()];
        self.result = None;
        // Chips used here may have changed; drop wires that no longer fit.
        let lib = self.lib.clone();
        self.design().prune(&lib);
        self.rebuild();
        self.update_cost();
    }

    /// Rebuild the library after sandbox chips changed.
    fn refresh_library(&mut self) {
        self.lib = self.progress.library(self.progress.fast);
        self.computer = None;
        if let Some(w) = &mut self.world {
            w.set_library(self.lib.clone());
        }
    }

    /// Recompile the live simulation after the design changed.
    fn rebuild(&mut self) {
        let def = self.design().def.clone();
        // A sandbox chip's own entry in the library is the saved version;
        // compile against a library without it, so it cannot contain itself.
        let mut lib = self.lib.clone();
        if let Target::Free(name) = &self.target {
            lib.remove(name);
            lib.insert(def.clone());
        }
        self.sim = Circuit::compile_def(&lib, &def).map_err(|e| e.to_string());
        self.inputs.resize(def.inputs.len(), 0);
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
        self.unsaved_since = None;
        let Some(path) = &self.save_path else { return };
        if let Err(e) = self.progress.save(path) {
            self.status = Some(format!("Kaydedilemedi: {e}"));
        }
    }

    /// Save within a couple of seconds, once for many changes.
    fn save_soon(&mut self) {
        self.unsaved_since
            .get_or_insert_with(std::time::Instant::now);
    }

    /// Write pending changes once they are a couple of seconds old.
    fn save_if_due(&mut self, ctx: &egui::Context) {
        let Some(since) = self.unsaved_since else {
            return;
        };
        let wait = std::time::Duration::from_secs(2);
        match wait.checked_sub(since.elapsed()) {
            None => self.save(),
            Some(left) => ctx.request_repaint_after(left),
        }
    }

    fn run_test(&mut self) {
        let Target::Level(level) = self.target else {
            return;
        };
        let def = self.design().def.clone();
        self.result = Some(match check(level, &self.lib, &def) {
            Err(e) => TestResult::Error(e.to_string()),
            Ok(Verdict::Pass) => {
                self.progress.solutions.insert(level.chip.to_string(), def);
                self.refresh_library();
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
        self.update_cost();
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
            let current = matches!(&self.target, Target::Level(t) if all[start..end].iter().any(|l| l.chip == t.chip));
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
                        let selected =
                            self.target == Target::Level(level) && self.view == View::Editor;
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
        self.sandbox_list(ui);
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
        if ui
            .add(egui::Button::selectable(
                self.view == View::World,
                "🌍  Dünya",
            ))
            .on_hover_text("Parçaları serbestçe yerleştirip kendi devreni kur; çalışırken izle")
            .clicked()
        {
            self.open_world();
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
            self.refresh_library();
            self.rebuild();
            self.save();
        }
    }

    fn parts_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Parçalar").strong());
        ui.label(RichText::new("Tuvale sürükle").weak().small());
        let mut kinds: Vec<(String, PartKind, String)> = vec![(
            "NAND".into(),
            PartKind::Nand,
            "Her şeyin temeli: iki giriş de 1 ise 0, değilse 1.".into(),
        )];
        let (allowed, buses): (Vec<&Level>, bool) = match &self.target {
            Target::Level(level) => (
                level.allowed_chips().collect(),
                level.index() >= find("Mux32").unwrap().index(),
            ),
            Target::Free(_) => (levels().to_vec(), true),
        };
        for l in allowed.iter().filter(|l| self.progress.solved(l.chip)) {
            kinds.push((l.chip.into(), PartKind::Chip(l.chip.into()), l.title.into()));
        }
        if let Target::Free(me) = &self.target {
            for name in self.progress.sandbox.chips.keys() {
                if !self.progress.sandbox.uses(name, me) {
                    kinds.push((
                        format!("★ {name}"),
                        PartKind::Chip(name.clone()),
                        "Senin çipin".into(),
                    ));
                }
            }
        }
        if buses {
            kinds.push((
                "Böl".into(),
                PartKind::Split(vec![1; 8]),
                "Bus'ı parçalara ayır. Sağ tıkla: grupları ayarla.".into(),
            ));
            kinds.push((
                "Birleştir".into(),
                PartKind::Join(vec![1; 8]),
                "Parçaları tek bus yap. Sağ tıkla: grupları ayarla.".into(),
            ));
            kinds.push((
                "Sabit".into(),
                PartKind::Const { width: 1, value: 1 },
                "Hep aynı değeri verir. Sağ tıkla: değeri ayarla.".into(),
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
                ui.colored_label(crate::theme::BAD, e);
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
                        crate::theme::BAD,
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
        self.bus_inputs(ui);
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
        ui.add_space(6.0);
        self.cost_view(ui);
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
                ui.colored_label(crate::theme::BAD, e);
            }
            TestResult::Wrong(m) => {
                ui.colored_label(crate::theme::BAD, "✖ Test geçmedi");
                ui.label(m);
            }
            TestResult::Oscillates { step } => {
                ui.colored_label(
                    crate::theme::BAD,
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
                    crate::theme::BAD,
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
                            if ok { ON } else { crate::theme::BAD },
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
        self.save_if_due(&ctx);
        if self.session.is_some() {
            self.net_tick();
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
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
                egui::ScrollArea::vertical().show(ui, |ui| match (&self.view, &self.target) {
                    (View::Editor, Target::Level(_)) if self.solution.is_some() => {
                        self.solution_panel(ui)
                    }
                    (View::Editor, Target::Level(_)) => self.level_panel(ui),
                    (View::Editor, Target::Free(_)) => self.sandbox_panel(ui),
                    (View::Computer | View::Play, _) => self.computer_controls(ui),
                    (View::Wiki, _) => self.wiki.contents(ui),
                    (View::World, _) => self.world_panel(ui),
                });
            });
        if let Some(status) = self.status.clone() {
            egui::Panel::bottom("status").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(crate::theme::WARN, status);
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
        if self.view == View::World {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    if let Some(w) = &mut self.world {
                        let rev = w.world.revision();
                        w.show(ui);
                        if w.world.revision() != rev {
                            self.store_world();
                        }
                    }
                });
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
        if self.solution.is_some() && self.view == View::Editor {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| self.solution_canvas(ui));
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
                let design = design_for(&mut self.progress, &self.target);
                let out = self
                    .canvas
                    .show(ui, design, &lib, sim.as_ref(), &inputs, output_x);
                if let Some((pin, v)) = out.toggled_input {
                    self.inputs[pin] = v;
                    self.run();
                }
                if out.changed {
                    // Sandbox chips are part of the library: refresh it so
                    // chips that use this one see the change.
                    if matches!(self.target, Target::Free(_)) {
                        self.refresh_library();
                    }
                    self.rebuild();
                    self.save();
                }
            });
    }
}

/// The design being edited for a target, created on first use.
fn design_for<'a>(progress: &'a mut Progress, target: &Target) -> &'a mut Design {
    match target {
        Target::Level(level) => progress
            .designs
            .entry(level.chip.to_string())
            .or_insert_with(|| Design::new(level.blank())),
        Target::Free(name) => progress
            .sandbox
            .chips
            .entry(name.clone())
            .or_insert_with(|| sandbox::new_chip(name)),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        App::ui(self, ui);
    }

    fn on_exit(&mut self) {
        self.save();
    }
}
