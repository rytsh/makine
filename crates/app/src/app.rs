//! The main window: level list, parts palette, canvas and level panel.

use std::path::PathBuf;

use egui::{Color32, RichText, Ui};
use makine_levels::{check, find, levels, Level};
use makine_sim::{Circuit, Library, PartKind, Pin, Verdict};

use crate::canvas::{format_value, Canvas, NewPart, ON};
use crate::computer::{self, Computer, Engine};
use crate::design::Design;
use crate::sandbox::{self, PinEditor};
use crate::store::Progress;
use crate::wiki::{self, Wiki};

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
    /// The isometric block world.
    World,
}

pub struct App {
    progress: Progress,
    save_path: Option<PathBuf>,
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
        self.canvas = Canvas::new();
        self.canvas.symbols = self.progress.symbols;
        self.inputs = vec![0; self.design().def.inputs.len()];
        self.result = None;
        // Chips used here may have changed; drop wires that no longer fit.
        let lib = self.lib.clone();
        self.design().prune(&lib);
        self.rebuild();
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
        let Some(path) = &self.save_path else { return };
        if let Err(e) = self.progress.save(path) {
            self.status = Some(format!("Kaydedilemedi: {e}"));
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
            .on_hover_text("Blokları yerleştirerek kendi devreni kur (izometrik)")
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

impl App {
    fn open_world(&mut self) {
        if self.world.is_none() {
            let p = &self.progress;
            let world = p
                .last_world
                .as_ref()
                .and_then(|n| p.worlds.get(n))
                .or_else(|| p.worlds.values().next())
                .cloned()
                .unwrap_or_else(|| makine_world::World::new("dünya"));
            self.switch_world(world);
        }
        self.view = View::World;
    }

    /// Show `world` in the world view (the current one is already saved).
    fn switch_world(&mut self, world: makine_world::World) {
        self.progress.last_world = Some(world.name.clone());
        self.world_rename = world.name.clone();
        let mut v = crate::worldview::WorldView::new(world);
        v.set_library(self.lib.clone());
        self.world = Some(v);
        self.store_world();
    }

    /// Move edits between the world view and the network. Runs every
    /// frame, whatever view is shown, so other players are never kept
    /// waiting.
    fn net_tick(&mut self) {
        let (Some(s), Some(w)) = (&mut self.session, &mut self.world) else {
            return;
        };
        let rev = w.world.revision();
        let (closed, new_chips) = s.tick(w, &self.lib, &mut self.progress.sandbox);
        let changed = w.world.revision() != rev;
        if changed {
            self.store_world();
        }
        if new_chips {
            self.refresh_library();
            self.save();
        }
        if let Some(reason) = closed {
            self.leave_session(Some(reason));
        }
    }

    fn leave_session(&mut self, reason: Option<String>) {
        let Some(s) = self.session.take() else { return };
        if let Some(w) = &mut self.world {
            w.others.clear();
            w.sharing = false;
            w.outbox.clear();
        }
        let msg = reason.unwrap_or_else(|| "Paylaşım bitti.".into());
        if s.is_host() {
            self.status = Some(msg);
            return;
        }
        // A guest goes back to their own last world.
        self.status = Some(format!("{msg} Kendi dünyana döndün."));
        self.world = None;
        self.open_world();
    }

    /// Hosting and joining.
    fn network_section(&mut self, ui: &mut Ui) {
        let title = match &self.session {
            Some(s) => format!("🔗 Birlikte kur ({} kişi)", s.players().len().max(1)),
            None => "🔗 Birlikte kur".into(),
        };
        egui::CollapsingHeader::new(RichText::new(title).strong())
            .id_salt("network")
            .show(ui, |ui| {
                let Some(s) = &self.session else {
                    ui.label(
                        RichText::new(
                            "Aynı ağdaki (aynı Wi-Fi) arkadaşlarınla aynı dünyada birlikte kur.",
                        )
                        .weak()
                        .small(),
                    );
                    ui.horizontal(|ui| {
                        let l = ui.label("Adın").id;
                        ui.add(egui::TextEdit::singleline(&mut self.net_name).desired_width(120.0))
                            .labelled_by(l);
                    });
                    if ui
                        .button("📡 Bu dünyayı paylaşıma aç")
                        .on_hover_text("Arkadaşların bu dünyaya bağlanabilir")
                        .clicked()
                    {
                        match crate::session::Session::host(&self.net_name, makine_net::PORT) {
                            Ok(s) => self.session = Some(s),
                            Err(e) => {
                                self.status = Some(format!(
                                    "Paylaşım açılamadı ({e}). Başka bir makine zaten açık olabilir."
                                ))
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        let l = ui.label("Adres").id;
                        ui.add(
                            egui::TextEdit::singleline(&mut self.net_addr)
                                .hint_text("192.168.1.20")
                                .desired_width(120.0),
                        )
                        .labelled_by(l);
                        if ui.button("Bağlan").clicked() {
                            // Save our own world before showing theirs.
                            self.store_world();
                            match crate::session::Session::join(&self.net_addr, &self.net_name) {
                                Ok(s) => self.session = Some(s),
                                Err(e) => {
                                    self.status = Some(format!(
                                        "Bağlanılamadı: {e}. Adresi ve aynı ağda olduğunuzu kontrol et."
                                    ))
                                }
                            }
                        }
                    });
                    return;
                };
                ui.label(RichText::new(&s.status).small());
                let me = s.my_id();
                for p in s.players() {
                    let tag = match (p.id == makine_net::HOST_ID, Some(p.id) == me) {
                        (true, true) => " (sen, ev sahibi)",
                        (true, false) => " (ev sahibi)",
                        (false, true) => " (sen)",
                        _ => "",
                    };
                    ui.colored_label(crate::session::color(p.id), format!("● {}{tag}", p.name));
                }
                let label = if s.is_host() { "Paylaşımı kapat" } else { "Ayrıl" };
                if ui.button(label).clicked() {
                    self.leave_session(None);
                }
            });
    }

    fn worlds_section(&mut self, ui: &mut Ui) {
        let current = self.world.as_ref().map(|w| w.world.name.clone());
        let n = self.progress.worlds.len();
        let mut open = None;
        egui::CollapsingHeader::new(RichText::new(format!("Dünyalar ({n})")).strong())
            .id_salt("worlds")
            .show(ui, |ui| {
                for name in self.progress.worlds.keys() {
                    let me = current.as_deref() == Some(name.as_str());
                    if ui.selectable_label(me, format!("🌍 {name}")).clicked() && !me {
                        open = Some(name.clone());
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("+ Yeni").clicked() {
                        let name = crate::worlds::unique_name("dünya", &self.progress.worlds);
                        self.switch_to_new(makine_world::World::new(name));
                    }
                    if ui.button("Kopyasını al").clicked() {
                        if let Some(w) = &self.world {
                            let mut copy = w.world.clone();
                            copy.name =
                                crate::worlds::unique_name(&w.world.name, &self.progress.worlds);
                            self.switch_to_new(copy);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let l = ui.label("Ad").id;
                    ui.add(egui::TextEdit::singleline(&mut self.world_rename).desired_width(120.0))
                        .labelled_by(l);
                    if ui.button("Değiştir").clicked() {
                        self.rename_world();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("📤 Dosyaya kaydet…").clicked() {
                        self.export_world();
                    }
                    if ui.button("📥 Dosyadan aç…").clicked() {
                        self.import_world();
                    }
                });
                let can_delete = n > 1;
                let r = ui.add_enabled(can_delete, egui::Button::new("🗑 Bu dünyayı sil"));
                let r = if can_delete {
                    r
                } else {
                    r.on_disabled_hover_text("Son dünya silinemez")
                };
                if r.clicked() {
                    self.confirm_delete_world = true;
                }
                if self.confirm_delete_world {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::LIGHT_RED, "Emin misin?");
                        if ui.button("Evet, sil").clicked() {
                            self.delete_world();
                        }
                        if ui.button("Vazgeç").clicked() {
                            self.confirm_delete_world = false;
                        }
                    });
                }
            });
        if let Some(name) = open {
            if let Some(w) = self.progress.worlds.get(&name).cloned() {
                self.switch_world(w);
            }
        }
    }

    fn switch_to_new(&mut self, world: makine_world::World) {
        self.progress
            .worlds
            .insert(world.name.clone(), world.clone());
        self.switch_world(world);
    }

    fn rename_world(&mut self) {
        let to = self.world_rename.trim().to_string();
        let Some(w) = &mut self.world else { return };
        let from = w.world.name.clone();
        if to == from {
            return;
        }
        let err = crate::worlds::check_name(&to).err().or_else(|| {
            self.progress
                .worlds
                .contains_key(&to)
                .then(|| format!("'{to}' adında bir dünya var"))
        });
        if let Some(e) = err {
            w.status = Some(e);
            return;
        }
        self.progress.worlds.remove(&from);
        w.world.name = to.clone();
        self.progress.last_world = Some(to);
        self.store_world();
    }

    fn delete_world(&mut self) {
        self.confirm_delete_world = false;
        let Some(w) = &self.world else { return };
        let name = w.world.name.clone();
        if self.progress.worlds.len() < 2 {
            return;
        }
        self.progress.worlds.remove(&name);
        let next = self.progress.worlds.values().next().cloned().unwrap();
        self.world = None;
        self.switch_world(next);
    }

    fn export_world(&mut self) {
        let Some(w) = &self.world else { return };
        let text = match crate::worlds::export(&w.world, &self.lib, &self.progress.sandbox) {
            Ok(t) => t,
            Err(e) => {
                self.status = Some(e);
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("makine dünyası", &[crate::worlds::EXT])
            .set_file_name(format!("{}.{}", w.world.name, crate::worlds::EXT))
            .save_file()
        else {
            return;
        };
        self.status = Some(match std::fs::write(&path, text) {
            Ok(()) => format!("Kaydedildi: {}", path.display()),
            Err(e) => format!("{}: {e}", path.display()),
        });
    }

    fn import_world(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("makine dünyası", &[crate::worlds::EXT])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|t| crate::worlds::import(&t, &self.lib, &mut self.progress.sandbox));
        match result {
            Ok(mut world) => {
                world.name = crate::worlds::unique_name(&world.name, &self.progress.worlds);
                self.refresh_library();
                self.status = Some(format!("'{}' açıldı.", world.name));
                self.switch_to_new(world);
            }
            Err(e) => self.status = Some(e),
        }
    }

    /// Save the open world into the progress file. Called after edits;
    /// saving is cheap for worlds of reasonable size.
    fn store_world(&mut self) {
        // A world joined over the network belongs to its host. (Until it
        // arrives, the view still shows the player's own world.)
        if self
            .session
            .as_ref()
            .is_some_and(|s| !s.is_host() && s.ready)
        {
            return;
        }
        if let Some(w) = &self.world {
            self.progress
                .worlds
                .insert(w.world.name.clone(), w.world.clone());
            self.save();
        }
    }

    /// Right panel in the world view: build bar, layer, simulation.
    fn world_panel(&mut self, ui: &mut Ui) {
        if self.world.is_none() {
            return;
        }
        let title = self.world.as_ref().unwrap().world.name.clone();
        ui.heading(format!("🌍 {title}"));
        // Switching worlds mid-session would leave the others behind.
        if self.session.is_none() {
            self.worlds_section(ui);
        }
        self.network_section(ui);
        ui.separator();
        let Some(w) = &mut self.world else { return };
        ui.label(
            RichText::new(
                "Sol tık: yerleştir (sürükleyerek boya) · Sağ tık: sil · R: döndür · \
                 Q/E: katman · Tekerlek: yakınlaştır · Orta tık ya da Boşluk+sürükle: kaydır · \
                 Anahtara tıkla: aç/kapa",
            )
            .weak()
            .small(),
        );
        ui.add_space(6.0);
        ui.label(RichText::new("Bloklar").strong());
        let current = w.brush.as_ref().map(|b| std::mem::discriminant(&b.block));
        let current_gate = match w.brush.as_ref().map(|b| &b.block) {
            Some(makine_world::Block::Gate { gate, .. }) => Some(*gate),
            _ => None,
        };
        ui.horizontal_wrapped(|ui| {
            for (name, block, tip) in crate::worldview::palette() {
                let gate = match &block {
                    makine_world::Block::Gate { gate, .. } => Some(*gate),
                    _ => None,
                };
                let selected =
                    current == Some(std::mem::discriminant(&block)) && gate == current_gate;
                let r = ui.selectable_label(selected, name);
                let r = if tip.is_empty() {
                    r
                } else {
                    r.on_hover_text(tip)
                };
                if r.clicked() {
                    w.brush = Some(crate::worldview::Brush { block });
                }
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Cihazlar").strong());
        w.device_palette_ui(ui);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .selectable_label(w.selecting(), "⬚ Seç")
                .on_hover_text("Alan seç: kopyala, taşı, çipe çevir")
                .clicked()
            {
                w.brush = None;
            }
        });
        // Chips from the library, as blocks.
        let mut chips: Vec<String> = levels()
            .iter()
            .filter(|l| self.progress.solved(l.chip))
            .map(|l| l.chip.to_string())
            .collect();
        chips.extend(self.progress.sandbox.chips.keys().map(|n| format!("★{n}")));
        if !chips.is_empty() {
            egui::CollapsingHeader::new(
                RichText::new(format!("Çiplerin ({})", chips.len())).strong(),
            )
            .id_salt("world-chips")
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Çözdüğün seviyeler ve serbest çiplerin, blok olarak.")
                        .weak()
                        .small(),
                );
                let current = match w.brush.as_ref().map(|b| &b.block) {
                    Some(makine_world::Block::Device {
                        device: makine_world::Device::Chip { name, .. },
                        ..
                    }) => Some(name.clone()),
                    _ => None,
                };
                ui.horizontal_wrapped(|ui| {
                    for label in &chips {
                        let name = label.trim_start_matches('★');
                        let r = ui.selectable_label(current.as_deref() == Some(name), label);
                        if r.clicked() {
                            if let Some(p) = self.lib.kind_pins(&PartKind::Chip(name.into())) {
                                w.brush = Some(crate::worldview::Brush {
                                    block: makine_world::Block::Device {
                                        device: makine_world::Device::Chip {
                                            name: name.into(),
                                            inputs: p.inputs,
                                            outputs: p.outputs,
                                        },
                                        facing: makine_world::Dir::East,
                                    },
                                });
                            }
                        }
                    }
                });
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Yön:");
            let arrow = match w.facing {
                makine_world::Dir::East => "↘ doğu",
                makine_world::Dir::South => "↙ güney",
                makine_world::Dir::West => "↖ batı",
                makine_world::Dir::North => "↗ kuzey",
            };
            if ui.button(arrow).on_hover_text("R ile de döner").clicked() {
                w.facing = w.facing.cw();
            }
        });
        ui.horizontal(|ui| {
            ui.label(format!("Katman: {}", w.layer));
            if ui.small_button("▼").on_hover_text("Q").clicked() {
                w.layer = (w.layer - 1).max(0);
            }
            if ui.small_button("▲").on_hover_text("E").clicked() {
                w.layer = (w.layer + 1).min(makine_world::world::MAX_LAYER);
            }
        });
        ui.label(
            RichText::new(
                "Üst katmanlar gizlenir, alttakiler soluk görünür. Katmanlar arası bağlantı için \
                 iki katmana da 'Geçiş' koy.",
            )
            .weak()
            .small(),
        );
        ui.separator();
        ui.horizontal(|ui| {
            let label = if w.sim.running {
                "⏸ Durdur"
            } else {
                "▶ Çalıştır"
            };
            if ui
                .button(label)
                .on_hover_text("Saatleri durdur/başlat")
                .clicked()
            {
                w.sim.running = !w.sim.running;
            }
            if ui
                .add_enabled(w.can_undo(), egui::Button::new("↶ Geri al"))
                .clicked()
            {
                w.undo();
            }
            if ui
                .add_enabled(w.can_redo(), egui::Button::new("↷ Yinele"))
                .clicked()
            {
                w.redo();
            }
            if ui
                .button("⌖ Ortala")
                .on_hover_text("Bütün dünyayı göster")
                .clicked()
            {
                w.refit();
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!w.sim.running, egui::Button::new("⏭ Bir adım"))
                .on_hover_text("Durmuşken saati bir adım ilerlet")
                .clicked()
            {
                w.sim.single_step();
            }
            let l = ui.label("Hız").id;
            let name = |v: u32| match v {
                0 => "En hızlı".to_string(),
                v => format!("{v} adım/kare"),
            };
            egui::ComboBox::from_id_salt("world-speed")
                .selected_text(name(w.sim.speed))
                .show_ui(ui, |ui| {
                    for v in [1, 10, 100, 1000, 0] {
                        ui.selectable_value(&mut w.sim.speed, v, name(v));
                    }
                })
                .response
                .labelled_by(l);
        });
        ui.label(
            RichText::new(format!(
                "{} blok · adım {} · {}",
                w.world.len(),
                w.sim.tick,
                if w.sim.running {
                    format!("{:.0} adım/sn", w.sim.rate)
                } else {
                    "durdu".into()
                }
            ))
            .weak(),
        );
        ui.label(
            RichText::new(
                "Saat bloğu her 'periyot' adımda bir değişir. Hızı artırınca saat ve ona \
                 bağlı devreler hızlanır.",
            )
            .weak()
            .small(),
        );
        if let Some(e) = w.sim.error() {
            ui.colored_label(Color32::LIGHT_RED, e.to_string());
        }
        if let Some(e) = &w.status {
            ui.colored_label(Color32::LIGHT_YELLOW, e);
        }
        w.selection_panel(ui);
        w.block_panel(ui);
        w.device_panel(ui);
        if let Some(req) = w.request.take() {
            self.handle_world_request(req);
        }
    }

    fn handle_world_request(&mut self, req: crate::worldview::Request) {
        let crate::worldview::Request::MakeChip { name, region } = req;
        let Some(w) = &mut self.world else { return };
        if let Err(e) = sandbox::check_name(&name, &self.lib, &self.progress.sandbox, None) {
            w.status = Some(e);
            return;
        }
        let part = w.region_world(region, &name);
        match makine_world::chip::to_chip(&part, &name, &self.lib) {
            Ok(def) => {
                let n = def.parts.len();
                let mut d = Design::new(def);
                // Lay the parts out in a grid so the 2D editor is usable.
                for (i, p) in d.pos.iter_mut().enumerate() {
                    *p = [200.0 + 90.0 * (i % 8) as f32, 60.0 + 70.0 * (i / 8) as f32];
                }
                self.progress.sandbox.chips.insert(name.clone(), d);
                self.refresh_library();
                if let Some(w) = &mut self.world {
                    w.set_library(self.lib.clone());
                    w.chip_name.clear();
                    w.status = Some(format!(
                        "'{name}' çipi oluşturuldu ({n} parça). Serbest çiplerde ve 'Çiplerin' \
                         listesinde."
                    ));
                }
                self.save();
            }
            Err(e) => w.status = Some(e),
        }
    }

    /// The sandbox section of the left panel: list of free chips and a
    /// field to create a new one.
    fn sandbox_list(&mut self, ui: &mut Ui) {
        let n = self.progress.sandbox.chips.len();
        let editing = match &self.target {
            Target::Free(name) if self.view == View::Editor => Some(name.clone()),
            _ => None,
        };
        let mut open = None;
        egui::CollapsingHeader::new(RichText::new(format!("🔧 Serbest çipler  ({n})")).strong())
            .id_salt("sandbox")
            .default_open(editing.is_some())
            .show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "Seviye olmadan kendi çiplerini tasarla, paylaş, birbirinde kullan.",
                    )
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
                    let l = ui.label("Yeni:").id;
                    let r = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.new_chip_name)
                                .hint_text("çip adı")
                                .desired_width(100.0),
                        )
                        .labelled_by(l);
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("+ Oluştur").clicked() || enter {
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
                if ui.button("📥  Çip dosyası aç…").clicked() {
                    self.import_package();
                }
            });
        if let Some(name) = open {
            self.open_free(name);
        }
    }

    fn import_package(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("makine çipi", &[sandbox::EXT])
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
                self.status = Some(format!("'{name}' eklendi."));
                self.open_free(name);
            }
            Err(e) => self.status = Some(e),
        }
    }

    fn export_package(&mut self, name: &str) {
        let package = match sandbox::export(&self.lib, &self.progress.sandbox, name) {
            Ok(p) => p,
            Err(e) => {
                self.status = Some(e);
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("makine çipi", &[sandbox::EXT])
            .set_file_name(format!("{name}.{}", sandbox::EXT))
            .save_file()
        else {
            return;
        };
        self.status = Some(match std::fs::write(&path, package.to_json()) {
            Ok(()) => format!("Kaydedildi: {}", path.display()),
            Err(e) => format!("{}: {e}", path.display()),
        });
    }

    /// Right panel for a sandbox chip: name, pins, simulation, sharing.
    fn sandbox_panel(&mut self, ui: &mut Ui) {
        let Target::Free(name) = self.target.clone() else {
            return;
        };
        ui.heading(format!("★ {name}"));
        ui.label(RichText::new("Serbest çip: test yok, kural yok.").weak());
        ui.add_space(6.0);

        // Rename.
        ui.horizontal(|ui| {
            let l = ui.label("Ad").id;
            ui.add(egui::TextEdit::singleline(&mut self.rename_to).desired_width(140.0))
                .labelled_by(l);
            if ui.button("Değiştir").clicked() && self.rename_to.trim() != name {
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
        ui.label(RichText::new("Uçlar").strong());
        let def = self.design().def.clone();
        let editor = self.pin_editor.get_or_insert_with(|| PinEditor::new(&def));
        if let Some((ins, outs)) = editor.show(ui) {
            self.progress.sandbox.set_pins(&name, ins, outs);
            self.pin_editor = None;
            self.refresh_library();
            let lib = self.lib.clone();
            self.design().prune(&lib);
            self.rebuild();
            self.save();
        }
        ui.separator();

        // Live simulation.
        match &self.sim {
            Err(e) if !e.is_empty() => {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            Ok(sim) => {
                ui.label(
                    RichText::new(format!(
                        "{} NAND kapısı + {} hızlı model",
                        sim.gate_count(),
                        sim.native_count()
                    ))
                    .weak(),
                );
                if self.oscillating {
                    ui.colored_label(Color32::LIGHT_RED, "Devre salınıyor.");
                }
            }
            _ => {}
        }
        self.bus_inputs(ui);
        ui.horizontal(|ui| {
            if ui.button("Durumu sıfırla").clicked() {
                if let Ok(sim) = &mut self.sim {
                    sim.reset();
                }
                self.run();
            }
        });

        // Sharing and deleting.
        ui.separator();
        ui.label(RichText::new("Paylaş").strong());
        ui.label(
            RichText::new(
                "Çipi, kullandığı diğer serbest çiplerle birlikte tek bir .makine dosyası \
                 olarak kaydet. Başkası 'Çip dosyası aç' ile ekleyebilir.",
            )
            .weak()
            .small(),
        );
        if ui.button("📤  Çip dosyası olarak kaydet…").clicked() {
            self.export_package(&name);
        }
        ui.add_space(10.0);
        let users = self.progress.sandbox.users_of(&name);
        let can_delete = users.is_empty();
        let r = ui.add_enabled(can_delete, egui::Button::new("🗑  Çipi sil"));
        let r = if can_delete {
            r
        } else {
            r.on_disabled_hover_text(format!("Önce şu çiplerden çıkar: {}", users.join(", ")))
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
    fn bus_inputs(&mut self, ui: &mut Ui) {
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

        // A block world: two switches into a half adder lighting two lamps,
        // and a clock blinking a lamp on layer 1.
        {
            use makine_world::{Block, Dir, Edit, Gate, Pos};
            let mut w = makine_world::World::new("örnek");
            let mut set = |x, y, z, b| {
                w.apply(Edit::Set {
                    pos: Pos::new(x, y, z),
                    block: Some(b),
                });
            };
            set(
                0,
                0,
                0,
                Block::Switch {
                    on: true,
                    facing: Dir::East,
                },
            );
            for x in 1..=5 {
                set(x, 0, 0, Block::Wire);
            }
            set(
                0,
                2,
                0,
                Block::Switch {
                    on: false,
                    facing: Dir::East,
                },
            );
            for x in 1..=5 {
                set(x, 2, 0, Block::Wire);
            }
            set(
                5,
                1,
                0,
                Block::Gate {
                    gate: Gate::Xor,
                    facing: Dir::East,
                },
            );
            set(6, 1, 0, Block::Wire);
            set(7, 1, 0, Block::Lamp);
            set(3, 1, 0, Block::Wire);
            set(3, 2, 0, Block::Cross);
            set(3, 3, 0, Block::Wire);
            for x in 3..=5 {
                set(x, 4, 0, Block::Wire);
            }
            for y in 3..=6 {
                set(1, y, 0, Block::Wire);
            }
            for x in 2..=5 {
                set(x, 6, 0, Block::Wire);
            }
            set(
                5,
                5,
                0,
                Block::Gate {
                    gate: Gate::And,
                    facing: Dir::East,
                },
            );
            set(6, 5, 0, Block::Wire);
            set(7, 5, 0, Block::Lamp);
            set(
                2,
                9,
                1,
                Block::Clock {
                    period: 30,
                    facing: Dir::East,
                },
            );
            for x in 3..=5 {
                set(x, 9, 1, Block::Wire);
            }
            set(6, 9, 1, Block::Lamp);
            let mut a = app.borrow_mut();
            a.view = View::World;
            a.world = Some(crate::worldview::WorldView::new(w));
        }
        harness.run_steps(3);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-world.png")).unwrap();

        // Devices: a keyboard into a display, and a screen with a pattern.
        {
            use makine_world::device::Device;
            use makine_world::{Block, Dir, Edit, Pos};
            let mut w = makine_world::World::new("cihazlar");
            let place = |w: &mut makine_world::World, d: Device, p: Pos| {
                let e = w.place_device(d, p, Dir::East).unwrap();
                w.apply(e);
            };
            place(&mut w, Device::Keyboard, Pos::new(0, 0, 0));
            for y in 0..8 {
                w.apply(Edit::Set {
                    pos: Pos::new(3, y, 0),
                    block: Some(Block::Straight { facing: Dir::East }),
                });
            }
            place(&mut w, Device::Display { bits: 8 }, Pos::new(4, 0, 0));
            place(&mut w, Device::Screen { bits: 3 }, Pos::new(0, 11, 0));
            place(&mut w, Device::Ram { addr: 3, data: 8 }, Pos::new(12, 0, 0));
            let mut v = crate::worldview::WorldView::new(w);
            v.selected = Some(Pos::new(0, 0, 0));
            app.borrow_mut().world = Some(v);
        }
        harness.run_steps(2);
        harness.event(egui::Event::Text("M".into()));
        harness.run_steps(3);
        // Draw a smiley on the screen through its native state.
        {
            let mut a = app.borrow_mut();
            let v = a.world.as_mut().unwrap();
            v.set_screen_for_test(makine_world::Pos::new(0, 11, 0), |x, y| {
                matches!((x, y), (2, 2) | (5, 2) | (1, 5) | (6, 5))
                    || (y == 6 && (2..=5).contains(&x))
            });
        }
        harness.run_steps(3);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-world-devices.png")).unwrap();

        // Chips as blocks: Xor and And from the levels, a selected region.
        {
            use makine_world::{Block, Device, Dir, Edit, Pos};
            let mut a = app.borrow_mut();
            let lib = a.lib.clone();
            let chip = |name: &str| {
                let p = lib.kind_pins(&PartKind::Chip(name.into())).unwrap();
                Device::Chip {
                    name: name.into(),
                    inputs: p.inputs,
                    outputs: p.outputs,
                }
            };
            let mut w = makine_world::World::new("çipler");
            w.apply(
                w.place_device(chip("Xor"), Pos::new(2, 0, 0), Dir::East)
                    .unwrap(),
            );
            w.apply(
                w.place_device(chip("And"), Pos::new(2, 3, 0), Dir::East)
                    .unwrap(),
            );
            let st = || Some(Block::Straight { facing: Dir::East });
            for (x, y, on) in [(0, 0, true), (0, 1, true), (0, 3, true), (0, 4, false)] {
                w.apply(Edit::Set {
                    pos: Pos::new(x, y, 0),
                    block: Some(Block::Switch {
                        on,
                        facing: Dir::East,
                    }),
                });
                w.apply(Edit::Set {
                    pos: Pos::new(1, y, 0),
                    block: st(),
                });
            }
            for y in [0, 3] {
                w.apply(Edit::Set {
                    pos: Pos::new(5, y, 0),
                    block: st(),
                });
                w.apply(Edit::Set {
                    pos: Pos::new(6, y, 0),
                    block: Some(Block::Lamp),
                });
            }
            let mut v = crate::worldview::WorldView::new(w);
            v.set_library(lib);
            v.brush = None;
            v.selection = Some(makine_world::region::Region::between(
                Pos::new(0, 3, 0),
                Pos::new(6, 4, 0),
            ));
            a.world = Some(v);
        }
        harness.run_steps(3);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-world-chips.png")).unwrap();

        // Building together: a guest's cursor in the host's world.
        {
            let mut a = app.borrow_mut();
            a.session = Some(crate::session::Session::host("ray", 0).unwrap());
            a.world.as_mut().unwrap().selection = None;
        }
        let mut guest = crate::session::Session::join(
            &format!(
                "127.0.0.1:{}",
                match &app.borrow().session.as_ref().unwrap().link {
                    crate::session::Link::Host(h) => h.port(),
                    _ => unreachable!(),
                }
            ),
            "ayşe",
        )
        .unwrap();
        let mut gview = crate::worldview::WorldView::default();
        let mut gsand = crate::sandbox::Sandbox::default();
        let glib = app.borrow().lib.clone();
        for _ in 0..40 {
            guest.tick(&mut gview, &glib, &mut gsand);
            harness.run();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if let crate::session::Link::Client(c) = &mut guest.link {
            c.send_cursor(Some(makine_world::Pos::new(4, 1, 0)));
        }
        for _ in 0..20 {
            guest.tick(&mut gview, &glib, &mut gsand);
            harness.run();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        egui_kittest::kittest::Queryable::get_by_label_contains(&harness, "Birlikte kur").click();
        harness.run_steps(3);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-world-together.png"))
            .unwrap();
        drop(guest);
        app.borrow_mut().leave_session(None);

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
        // The app installs the DejaVu fallbacks on its first frame.
        crate::wiki::install_fonts(&ctx);
        let sources = [
            include_str!("app.rs"),
            include_str!("canvas.rs"),
            include_str!("computer.rs"),
            include_str!("wiki.rs"),
            include_str!("worldview/mod.rs"),
            include_str!("worldview/panels.rs"),
            include_str!("worldview/render.rs"),
            include_str!("worldsim.rs"),
            include_str!("session.rs"),
            include_str!("worlds.rs"),
            include_str!("design.rs"),
            include_str!("sandbox.rs"),
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

#[cfg(test)]
mod sandbox_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use makine_levels::reference::reference;
    use std::{cell::RefCell, rc::Rc};

    fn app_with_levels() -> Rc<RefCell<App>> {
        let mut progress = Progress::default();
        for l in levels() {
            progress.solutions.insert(l.chip.into(), reference(l.chip));
        }
        Rc::new(RefCell::new(App::new(progress, None)))
    }

    /// Create a chip through the UI, then use it inside another one.
    #[test]
    fn create_and_nest() {
        let app = app_with_levels();
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 900.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(2);
        h.get_by_label_contains("Serbest çipler").click();
        h.run_steps(2);
        h.get_by_label("Yeni:").focus();
        h.run();
        h.get_by_label("Yeni:").type_text("Tampon");
        h.run();
        h.get_by_label("+ Oluştur").click();
        h.run_steps(2);
        let (name, status) = {
            let a = app.borrow();
            (a.new_chip_name.clone(), a.status.clone())
        };
        assert!(
            app.borrow().target == Target::Free("Tampon".into()),
            "field={name:?} status={status:?}"
        );

        // Make Tampon = out = a via two NOTs, by editing its definition
        // directly (wiring by mouse is covered by the level test).
        {
            let mut a = app.borrow_mut();
            let mut b = makine_sim::ChipBuilder::new("Tampon", &["a", "b"], &["out"]);
            let x = b.input("a");
            let y = b.chip1("Not", &[x]);
            let z = b.chip1("Not", &[y]);
            b.output("out", z);
            *a.design() = Design::new(b.build());
            a.refresh_library();
            a.rebuild();
        }
        // A second chip sees Tampon in its parts list, Tampon does not see
        // the second one if the second one uses it.
        h.get_by_label("Yeni:").focus();
        h.run();
        h.get_by_label("Yeni:").type_text("Dis");
        h.run();
        h.get_by_label("+ Oluştur").click();
        h.run_steps(2);
        assert!(
            h.query_by_label("★ Tampon").is_some(),
            "Tampon not in parts"
        );
        {
            let mut a = app.borrow_mut();
            let mut b = makine_sim::ChipBuilder::new("Dis", &["a", "b"], &["out"]);
            let x = b.input("a");
            let y = b.chip1("Tampon", &[x]);
            b.output("out", y);
            *a.design() = Design::new(b.build());
            a.refresh_library();
            a.rebuild();
            a.inputs = vec![1, 0];
            a.run();
            assert_eq!(a.sim.as_ref().unwrap().output(0), 1);
        }
        h.run_steps(2);
        let tampon = app
            .borrow()
            .progress
            .sandbox
            .chips
            .keys()
            .position(|k| k == "Tampon");
        assert!(tampon.is_some());
        app.borrow_mut().open_free("Tampon".into());
        h.run_steps(2);
        assert!(h.query_by_label("★ Dis").is_none(), "cycle offered");
        // Tampon is used by Dis, so it cannot be deleted.
        assert!(!app.borrow().progress.sandbox.users_of("Tampon").is_empty());
    }

    /// Blocks in the world become a chip, which goes back in as a block.
    #[test]
    fn world_region_to_chip_and_back() {
        use makine_world::{Block, Dir, Edit, Gate, Pos};
        let app = app_with_levels();
        app.borrow_mut().open_world();
        {
            let mut a = app.borrow_mut();
            let w = a.world.as_mut().unwrap();
            let port = |n: &str, output| Block::Port {
                name: n.into(),
                output,
                facing: Dir::East,
            };
            for (i, b) in [
                port("a", false),
                Block::Gate {
                    gate: Gate::Not,
                    facing: Dir::East,
                },
                port("q", true),
            ]
            .into_iter()
            .enumerate()
            {
                w.edit(Edit::Set {
                    pos: Pos::new(i as i32, 0, 0),
                    block: Some(b),
                });
            }
            w.selection = Some(makine_world::region::Region::between(
                Pos::new(0, 0, 0),
                Pos::new(2, 0, 0),
            ));
            w.brush = None;
        }
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 900.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(2);
        h.get_by_label("Ad").focus();
        h.run();
        h.get_by_label("Ad").type_text("Ters");
        h.run();
        h.get_by_label("Çip yap").click();
        h.run_steps(2);
        let def = app.borrow().progress.sandbox.chips["Ters"].def.clone();
        assert_eq!(def.inputs, makine_sim::parse_pins(&["a"]));
        let mut sim = Circuit::compile(&app.borrow().lib, "Ters").unwrap();
        assert_eq!(sim.eval_values(&[0]).unwrap(), vec![1]);

        // The chip list offers it as a block; place it and drive it.
        h.get_by_label_contains("Çiplerin (").click();
        h.run_steps(2);
        h.get_by_label("★Ters").click();
        h.run_steps(2);
        {
            let mut a = app.borrow_mut();
            let w = a.world.as_mut().unwrap();
            let Some(crate::worldview::Brush {
                block: Block::Device { device, .. },
            }) = w.brush.clone()
            else {
                panic!("no chip brush");
            };
            let e = w
                .world
                .place_device(device, Pos::new(0, 5, 0), Dir::East)
                .unwrap();
            w.edit(e);
            w.edit(Edit::Set {
                pos: Pos::new(-1, 5, 0),
                block: Some(Block::Switch {
                    on: false,
                    facing: Dir::East,
                }),
            });
            w.edit(Edit::Set {
                pos: Pos::new(3, 5, 0),
                block: Some(Block::Lamp),
            });
        }
        h.run_steps(2);
        let a = app.borrow();
        let w = a.world.as_ref().unwrap();
        assert!(w.sim.error().is_none(), "{:?}", w.sim.error());
    }

    /// Several worlds: create, switch, rename, delete; edits stay in the
    /// world they were made in.
    #[test]
    fn many_worlds() {
        use makine_world::{Block, Edit, Pos};
        let app = app_with_levels();
        app.borrow_mut().open_world();
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1000.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(2);
        let edit = |app: &Rc<RefCell<App>>, x| {
            let mut a = app.borrow_mut();
            a.world.as_mut().unwrap().edit(Edit::Set {
                pos: Pos::new(x, 0, 0),
                block: Some(Block::Wire),
            });
            a.store_world();
        };
        edit(&app, 0);
        h.get_by_label_contains("Dünyalar (").click();
        h.run_steps(2);
        h.get_by_label("+ Yeni").click();
        h.run_steps(2);
        assert_eq!(app.borrow().world.as_ref().unwrap().world.name, "dünya 2");
        assert!(app.borrow().world.as_ref().unwrap().world.is_empty());
        edit(&app, 5);
        edit(&app, 6);
        // Back to the first one.
        h.get_by_label("🌍 dünya").click();
        h.run_steps(2);
        assert_eq!(app.borrow().world.as_ref().unwrap().world.len(), 1);
        assert_eq!(app.borrow().progress.worlds["dünya 2"].len(), 2);
        // Rename it.
        let f = h.get_by_label("Ad");
        f.focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_label("Ad").type_text("şehir");
        h.run();
        h.get_by_label("Değiştir").click();
        h.run_steps(2);
        let names: Vec<String> = app.borrow().progress.worlds.keys().cloned().collect();
        assert_eq!(names, ["dünya 2", "şehir"]);
        assert_eq!(app.borrow().progress.last_world.as_deref(), Some("şehir"));
        // Delete it, with confirmation.
        h.get_by_label("🗑 Bu dünyayı sil").click();
        h.run_steps(2);
        h.get_by_label("Evet, sil").click();
        h.run_steps(2);
        let names: Vec<String> = app.borrow().progress.worlds.keys().cloned().collect();
        assert_eq!(names, ["dünya 2"]);
        assert_eq!(app.borrow().world.as_ref().unwrap().world.name, "dünya 2");
    }

    /// Two apps on this machine build together: edits and free chips
    /// travel both ways, and the guest's own worlds are left alone.
    #[test]
    fn build_together() {
        use makine_world::{Block, Device, Dir, Edit, Pos};
        let host = app_with_levels();
        let guest = app_with_levels();
        for a in [&host, &guest] {
            a.borrow_mut().open_world();
        }
        // The host has a free chip and places it in the world.
        {
            let mut h = host.borrow_mut();
            let mut b = makine_sim::ChipBuilder::new("Ikili", &["a"], &["q"]);
            let x = b.input("a");
            let y = b.chip1("Not", &[x]);
            let z = b.chip1("Not", &[y]);
            b.output("q", z);
            h.progress
                .sandbox
                .chips
                .insert("Ikili".into(), Design::new(b.build()));
            h.refresh_library();
            let w = h.world.as_mut().unwrap();
            w.edit(Edit::Set {
                pos: Pos::new(0, 0, 0),
                block: Some(Block::Lamp),
            });
            h.session = Some(crate::session::Session::host("ev", 0).unwrap());
        }
        let port = match &host.borrow().session.as_ref().unwrap().link {
            crate::session::Link::Host(h) => h.port(),
            _ => unreachable!(),
        };
        let guest_world_before = guest.borrow().world.as_ref().unwrap().world.name.clone();
        guest.borrow_mut().session =
            Some(crate::session::Session::join(&format!("127.0.0.1:{port}"), "misafir").unwrap());
        let pump = |until: &dyn Fn() -> bool| {
            let t = std::time::Instant::now();
            while !until() {
                host.borrow_mut().net_tick();
                guest.borrow_mut().net_tick();
                assert!(t.elapsed() < std::time::Duration::from_secs(5), "timed out");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        pump(&|| guest.borrow().session.as_ref().unwrap().ready);
        assert_eq!(guest.borrow().world.as_ref().unwrap().world.len(), 1);

        // Host places the free chip; the guest gets the block and the chip.
        {
            let mut h = host.borrow_mut();
            let p = h.lib.kind_pins(&PartKind::Chip("Ikili".into())).unwrap();
            let d = Device::Chip {
                name: "Ikili".into(),
                inputs: p.inputs,
                outputs: p.outputs,
            };
            let w = h.world.as_mut().unwrap();
            let e = w
                .world
                .place_device(d, Pos::new(3, 3, 0), Dir::East)
                .unwrap();
            w.edit(e);
        }
        pump(&|| guest.borrow().progress.sandbox.chips.contains_key("Ikili"));
        pump(&|| {
            guest.borrow().world.as_ref().unwrap().world.len()
                == host.borrow().world.as_ref().unwrap().world.len()
        });
        assert!(guest.borrow().lib.contains("Ikili"));

        // Guest edits; the host sees it and saves it.
        guest.borrow_mut().world.as_mut().unwrap().edit(Edit::Set {
            pos: Pos::new(9, 9, 0),
            block: Some(Block::Wire),
        });
        pump(&|| {
            host.borrow()
                .world
                .as_ref()
                .unwrap()
                .world
                .get(Pos::new(9, 9, 0))
                .is_some()
        });
        let hname = host.borrow().world.as_ref().unwrap().world.name.clone();
        assert!(host.borrow().progress.worlds[&hname]
            .get(Pos::new(9, 9, 0))
            .is_some());
        // The shared world is not saved among the guest's worlds.
        assert!(guest
            .borrow()
            .progress
            .worlds
            .values()
            .all(|w| w.get(Pos::new(9, 9, 0)).is_none()));

        // The host stops sharing: the guest goes back to their own world.
        host.borrow_mut().leave_session(None);
        pump(&|| guest.borrow().session.is_none());
        assert_eq!(
            guest.borrow().world.as_ref().unwrap().world.name,
            guest_world_before
        );
    }

    /// Changing pins in the panel applies them to the chip.
    #[test]
    fn edit_pins() {
        let app = app_with_levels();
        app.borrow_mut()
            .progress
            .sandbox
            .chips
            .insert("P".into(), sandbox::new_chip("P"));
        app.borrow_mut().refresh_library();
        app.borrow_mut().open_free("P".into());
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 900.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(2);
        let inputs = h.get_by_label("Girişler");
        inputs.focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_label("Girişler").type_text("x[8], y");
        h.run();
        h.get_by_label("Uçları uygula").click();
        h.run_steps(2);
        let ins = app.borrow().progress.sandbox.chips["P"].def.inputs.clone();
        assert_eq!(ins, makine_sim::parse_pins(&["x[8]", "y"]));
        assert_eq!(app.borrow().inputs.len(), 2);
    }
}
