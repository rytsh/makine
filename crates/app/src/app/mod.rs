//! The main window: level list, parts palette, canvas and level panel.

use std::path::PathBuf;

use egui::{RichText, Ui};
use makine_lang::{tr, trf};
use makine_levels::{check, find, levels, Level};
use makine_sim::{Circuit, Library, PartKind, Pin, Verdict};

use crate::canvas::{format_value, Canvas, NewPart, ON};
use crate::computer::Computer;
use crate::design::Design;
use crate::sandbox::{self, PinEditor};
use crate::store::Progress;
use crate::wiki::{self, Wiki};

mod chrome;
mod computer_ui;
mod debug_ui;
mod home_ui;
mod journey_ui;
mod mini_ui;
mod programming_ui;
mod sandbox_ui;
mod small_ui;
mod solution_ui;
mod teaching_ui;
#[cfg(test)]
mod tests;
mod truth_ui;
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
    /// The home page: progress and profiles.
    Home,
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
    /// When the message appeared (egui time), and which one.
    status_since: Option<(f64, String)>,
    /// World x of the chip's output column, from the last frame.
    output_x: f32,
    view: View,
    /// Code is a tool inside Levels or World, not a separate destination.
    editor_code: bool,
    world_code: bool,
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
    /// A room code to join over the internet.
    net_room: String,
    /// How the player's design compares with the example solution.
    cost: Option<makine_levels::cost::Cost>,
    delay: Option<makine_levels::depth::Delay>,
    /// The example solution, when open instead of the player's design.
    solution: Option<solution_ui::Solution>,
    reveal: solution_ui::Reveal,
    /// Players on this computer; saving goes to the current one's file.
    profiles: Option<crate::profiles::Profiles>,
    home: home_ui::Home,
    debug: debug_ui::Debug,
    /// The mini processor running a program, once solved.
    mini: Option<mini_ui::MiniRun>,
    /// Undo and redo for the design being edited, and the design as it was
    /// after the last change (what an undo goes back to).
    history: crate::design::History,
    snapshot: Option<Design>,
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
    /// The app for the current profile in `profiles`, saving to its file.
    pub fn with_profiles(progress: Progress, profiles: Option<crate::profiles::Profiles>) -> Self {
        let path = profiles.as_ref().map(|p| p.current_path());
        let mut app = Self::new(progress, path);
        app.profiles = profiles;
        app
    }

    pub fn new(mut progress: Progress, save_path: Option<PathBuf>) -> Self {
        progress.notebook.ensure(progress.program.as_deref());
        let next_programming_lesson = crate::lessons::all()
            .into_iter()
            .find(|l| !progress.notebook.solved.contains(l.id))
            .map_or("register", |l| l.id);
        let prediction = progress
            .notebook
            .lesson_notes
            .get(&format!("predict:{next_programming_lesson}"))
            .cloned()
            .unwrap_or_default();
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
            status_since: None,
            output_x: 0.0,
            view: View::Editor,
            editor_code: false,
            world_code: false,
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
            net_room: String::new(),
            cost: None,
            delay: None,
            solution: None,
            reveal: Default::default(),
            profiles: None,
            home: Default::default(),
            debug: debug_ui::Debug {
                beginner: true,
                lesson: Some(next_programming_lesson.into()),
                prediction,
                selected_line: 1,
                number_format: 1,
                ..Default::default()
            },
            mini: None,
            history: Default::default(),
            snapshot: None,
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
        self.editor_code = false;
        self.target = target;
        self.solution = None;
        self.mini = None;
        self.reveal = Default::default();
        self.canvas = Canvas::new();
        self.canvas.symbols = self.progress.symbols;
        self.inputs = vec![0; self.design().def.inputs.len()];
        self.result = None;
        // Chips used here may have changed; drop wires that no longer fit.
        let lib = self.lib.clone();
        self.design().prune(&lib);
        self.history = Default::default();
        self.snapshot = Some(self.design().clone());
        self.rebuild();
        self.update_cost();
    }

    /// The design changed: remember how it was, for undo, then recompile
    /// and save.
    fn design_changed(&mut self) {
        self.design_changed_merging(false);
    }

    fn design_changed_merging(&mut self, merge: bool) {
        let now = self.design().clone();
        if let Some(before) = self.snapshot.replace(now) {
            self.history.push(before, merge);
        }
        self.after_edit();
    }

    fn after_edit(&mut self) {
        // Sandbox chips are part of the library: refresh it so chips that
        // use this one see the change.
        if matches!(self.target, Target::Free(_)) {
            self.refresh_library();
        }
        self.rebuild();
        self.save_soon();
    }

    fn undo(&mut self) {
        let now = self.design().clone();
        if let Some(d) = self.history.undo(&now) {
            self.restore(d);
        }
    }

    fn redo(&mut self) {
        let now = self.design().clone();
        if let Some(d) = self.history.redo(&now) {
            self.restore(d);
        }
    }

    fn restore(&mut self, d: Design) {
        self.snapshot = Some(d.clone());
        *self.design() = d;
        self.canvas.selected.clear();
        self.after_edit();
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
        self.progress.notebook.sync_custom();
        self.unsaved_since = None;
        if let Some(p) = &self.profiles {
            self.save_path = Some(p.current_path());
        }
        let Some(path) = &self.save_path else { return };
        if let Err(e) = self.progress.save(path) {
            self.status = Some(trf!("Kaydedilemedi: {e}", "Could not save: {e}"));
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
        let mut open = None;
        let mut activity = None;
        let all = levels();
        let mut start = 0;
        while start < all.len() {
            let section = all[start].section;
            let section_name = all[start].section_name();
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
            let header = format!("{section_name}  ({done}/{})", end - start);
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
                        let selected = self.target == Target::Level(level)
                            && self.view == View::Editor
                            && !self.editor_code;
                        let label = format!("{mark}  {}", level.name());
                        let r = ui.add_enabled(unlocked, egui::Button::selectable(selected, label));
                        if r.clicked() && !selected {
                            open = Some(level);
                        }
                        if unlocked {
                            for a in crate::curriculum::activities(level.chip) {
                                let text = format!(
                                    "    {} {}",
                                    if a.done(&self.progress.notebook) {
                                        "✔"
                                    } else {
                                        "↳"
                                    },
                                    a.title()
                                );
                                if ui.selectable_label(false, text).clicked() {
                                    activity = Some(a);
                                }
                            }
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
        if let Some(a) = activity {
            self.open_activity(a);
        }
        ui.add_space(6.0);
        ui.separator();
        self.sandbox_list(ui);
    }

    fn parts_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(tr!("Parçalar", "Parts")).strong());
        ui.label(
            RichText::new(tr!("Tuvale sürükle", "Drag onto the board"))
                .weak()
                .small(),
        );
        let mut kinds: Vec<(String, PartKind, String)> = vec![(
            "NAND".into(),
            PartKind::Nand,
            tr!(
                "Her şeyin temeli: iki giriş de 1 ise 0, değilse 1.",
                "The base of everything: 0 if both inputs are 1, otherwise 1."
            )
            .into(),
        )];
        let (allowed, buses): (Vec<&Level>, bool) = match &self.target {
            Target::Level(level) => (
                level.allowed_chips().collect(),
                level.index() >= find("Mux32").unwrap().index(),
            ),
            Target::Free(_) => (levels().to_vec(), true),
        };
        for l in allowed.iter().filter(|l| self.progress.solved(l.chip)) {
            kinds.push((
                l.chip.into(),
                PartKind::Chip(l.chip.into()),
                l.name().into(),
            ));
        }
        if let Target::Free(me) = &self.target {
            for name in self.progress.sandbox.chips.keys() {
                if !self.progress.sandbox.uses(name, me) {
                    kinds.push((
                        format!("★ {name}"),
                        PartKind::Chip(name.clone()),
                        tr!("Senin çipin", "Your chip").into(),
                    ));
                }
            }
        }
        if buses {
            kinds.push((
                tr!("Böl", "Split").into(),
                PartKind::Split(vec![1; 8]),
                tr!(
                    "Bus'ı parçalara ayır. Sağ tıkla: grupları ayarla.",
                    "Split a bus into parts. Right-click: set the groups."
                )
                .into(),
            ));
            kinds.push((
                tr!("Birleştir", "Join").into(),
                PartKind::Join(vec![1; 8]),
                tr!(
                    "Parçaları tek bus yap. Sağ tıkla: grupları ayarla.",
                    "Join parts into one bus. Right-click: set the groups."
                )
                .into(),
            ));
            kinds.push((
                tr!("Sabit", "Constant").into(),
                PartKind::Const { width: 1, value: 1 },
                tr!(
                    "Hep aynı değeri verir. Sağ tıkla: değeri ayarla.",
                    "Always gives the same value. Right-click: set the value."
                )
                .into(),
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
        ui.heading(level.name());
        ui.add_space(4.0);
        ui.label(level.about());
        self.level_activities(ui);
        self.circuit_teaching(ui);
        if let Some(page) = wiki::page_for_level(level.chip) {
            ui.add_space(4.0);
            if ui
                .link(trf!(
                    "📖 Wiki'de oku: {}",
                    "📖 Read in the wiki: {}",
                    page.title()
                ))
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
        ui.label(trf!("Girişler: {}", "Inputs: {}", pins(level.input_pins())));
        ui.label(trf!(
            "Çıkışlar: {}",
            "Outputs: {}",
            pins(level.output_pins())
        ));
        ui.separator();

        match &self.sim {
            Err(e) if !e.is_empty() => {
                ui.colored_label(crate::theme::BAD, e);
            }
            Ok(sim) => {
                let text = if sim.native_count() > 0 {
                    trf!(
                        "{} NAND kapısı + {} hızlı model (⚡)",
                        "{} NAND gates + {} fast models (⚡)",
                        sim.gate_count(),
                        sim.native_count()
                    )
                } else {
                    trf!("{} NAND kapısı", "{} NAND gates", sim.gate_count())
                };
                ui.label(RichText::new(text).weak());
                if self.oscillating {
                    ui.colored_label(
                        crate::theme::BAD,
                        tr!(
                            "Devre salınıyor: bir geri besleme döngüsü hiç durulmuyor.",
                            "The circuit oscillates: a feedback loop never settles."
                        ),
                    );
                }
            }
            _ => {}
        }
        ui.label(
            RichText::new(tr!(
                "Girişleri tıklayarak değiştir. Bus girişlerinde değeri aşağıdan gir.",
                "Click inputs to change them. For bus inputs, enter the value below."
            ))
            .weak()
            .small(),
        );
        self.bus_inputs(ui);
        self.truth_table(ui);
        self.undo_buttons(ui);
        ui.horizontal(|ui| {
            if ui
                .button(tr!("Durumu sıfırla", "Reset state"))
                .on_hover_text(tr!("Bütün mandalları 0'a çek", "Set every latch to 0"))
                .clicked()
            {
                if let Ok(sim) = &mut self.sim {
                    sim.reset();
                }
                self.run();
            }
            if ui
                .button(tr!("Temizle", "Clear"))
                .on_hover_text(tr!(
                    "Bütün parçaları sil, baştan başla",
                    "Delete every part and start over"
                ))
                .clicked()
            {
                *self.design() = Design::new(level.blank());
                self.design_changed();
            }
        });

        ui.separator();
        if ui
            .add(
                egui::Button::new(RichText::new(tr!("▶  Test et", "▶  Test")).size(16.0))
                    .min_size(egui::vec2(120.0, 32.0)),
            )
            .clicked()
        {
            self.run_test();
        }
        ui.add_space(6.0);
        self.result_view(ui);
        self.mini_view(ui);
        if !self.progress.solved(level.chip) {
            self.hints_view(ui);
        }
        ui.add_space(6.0);
        self.cost_view(ui);
    }

    /// The last message, at the bottom of the window over everything, gone
    /// after a while or when closed.
    fn toast(&mut self, ctx: &egui::Context) {
        let Some(text) = self.status.clone() else {
            self.status_since = None;
            return;
        };
        let now = ctx.input(|i| i.time);
        if self.status_since.as_ref().is_none_or(|(_, t)| *t != text) {
            self.status_since = Some((now, text.clone()));
        }
        let age = now - self.status_since.as_ref().map_or(now, |s| s.0);
        const SHOW: f64 = 8.0;
        if age > SHOW {
            self.status = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -18.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(crate::theme::RAISED)
                    .stroke(egui::Stroke::new(1.0, crate::theme::WARN))
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(screen.width() * 0.7);
                        ui.horizontal(|ui| {
                            ui.colored_label(crate::theme::WARN, "●");
                            ui.label(&text);
                            if ui.small_button("×").clicked() {
                                self.status = None;
                            }
                        });
                    });
            });
    }

    /// Undo / redo buttons (also Ctrl+Z / Ctrl+Y on the canvas).
    fn undo_buttons(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.history.can_undo(),
                    egui::Button::new(tr!("↶ Geri al", "↶ Undo")),
                )
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(
                    self.history.can_redo(),
                    egui::Button::new(tr!("↷ Yinele", "↷ Redo")),
                )
                .on_hover_text("Ctrl+Y")
                .clicked()
            {
                self.redo();
            }
        });
    }

    fn result_view(&mut self, ui: &mut Ui) {
        let level = self.level;
        let Some(result) = &self.result else { return };
        match result {
            TestResult::Pass => {
                ui.colored_label(
                    ON,
                    RichText::new(tr!(
                        "✔ Tebrikler, bütün testler geçti!",
                        "✔ Well done, every test passed!"
                    ))
                    .strong(),
                );
                let next = levels().get(level.index() + 1);
                if let Some(&next) = next {
                    ui.label(trf!(
                        "{} artık bir parça olarak kullanılabilir.",
                        "{} can now be used as a part.",
                        level.chip
                    ));
                    if ui
                        .button(trf!("Sıradaki: {} ▶", "Next: {} ▶", next.name()))
                        .clicked()
                    {
                        self.open_level(next);
                    }
                } else {
                    ui.label(tr!(
                        "İşlemcin hazır! Üstten 'Bilgisayar'ı aç ve onda program çalıştır.",
                        "Your CPU is ready! Open 'Computer' at the top and run programs on it."
                    ));
                    if ui
                        .button(tr!("🖥  Bilgisayara git", "🖥  Go to the computer"))
                        .clicked()
                    {
                        self.view = View::Computer;
                    }
                }
            }
            TestResult::Error(e) => {
                ui.colored_label(crate::theme::BAD, e);
            }
            TestResult::Wrong(m) => {
                ui.colored_label(
                    crate::theme::BAD,
                    tr!("✖ Test geçmedi", "✖ The test failed"),
                );
                ui.label(m);
            }
            TestResult::Oscillates { step } => {
                ui.colored_label(
                    crate::theme::BAD,
                    trf!(
                        "Adım {}: devre salınıyor ve hiç durulmuyor.",
                        "Step {}: the circuit oscillates and never settles.",
                        step + 1
                    ),
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
                    trf!(
                        "✖ Adım {}'de yanlış sonuç",
                        "✖ Wrong result at step {}",
                        step + 1
                    ),
                );
                if matches!(level.test, makine_levels::Test::Sequence(_)) {
                    ui.label(RichText::new(tr!("Bu bir sıralı test: adımlar art arda çalışır, devre önceki adımları hatırlar.", "This is a sequential test: steps run one after another and the circuit remembers earlier steps.")).weak().small());
                }
                egui::Grid::new("failure").striped(true).show(ui, |ui| {
                    for (p, v) in level.input_pins().iter().zip(inputs) {
                        ui.label(&p.name);
                        ui.monospace(format_value(p.width, *v));
                        ui.end_row();
                    }
                    ui.label("");
                    ui.label(RichText::new(tr!("beklenen", "expected")).weak());
                    ui.label(RichText::new(tr!("senin", "yours")).weak());
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
                if ui
                    .button(tr!(
                        "Bu girişleri devreye yükle",
                        "Load these inputs into the circuit"
                    ))
                    .clicked()
                {
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
        self.resize_edges(&ctx);
        self.window_border(&ctx);
        self.top_bar(ui);
        if self.view == View::Editor {
            egui::Panel::left("levels")
                .default_size(220.0)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("levels_scroll")
                        .show(ui, |ui| self.levels_panel(ui));
                });
            if !self.editor_code {
                egui::Panel::left("parts")
                    .default_size(130.0)
                    .resizable(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("parts_scroll")
                            .show(ui, |ui| self.parts_panel(ui));
                    });
            }
        }
        egui::Panel::right("level")
            .default_size(320.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match (&self.view, &self.target) {
                    (View::Editor | View::World, _) if self.showing_code() => {
                        self.programming_panel(ui)
                    }
                    (View::Editor, Target::Level(_)) if self.solution.is_some() => {
                        self.solution_panel(ui)
                    }
                    (View::Editor, Target::Level(_)) => self.level_panel(ui),
                    (View::Editor, Target::Free(_)) => self.sandbox_panel(ui),
                    (View::Computer | View::Play, _) => self.computer_controls(ui),
                    (View::Wiki, _) => self.wiki.contents(ui),
                    (View::World, _) => self.world_panel(ui),
                    (View::Home, _) => self.home_panel(ui),
                });
            });
        self.toast(&ctx);
        if self.showing_code() {
            if self.view == View::World {
                egui::Panel::bottom("live_world_below_code")
                    .default_size(230.)
                    .resizable(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(tr!(
                                    "Aynı Dünya devresi — canlı",
                                    "Same World circuit — live"
                                ))
                                .strong(),
                            );
                            if ui.button(tr!("Devreyi büyüt", "Expand circuit")).clicked() {
                                self.world_code = false;
                            }
                        });
                        if let Some(w) = &mut self.world {
                            let rev = w.world.revision();
                            w.show(ui);
                            if w.world.revision() != rev {
                                self.store_world();
                            }
                        }
                    });
            }
            egui::CentralPanel::default().show(ui, |ui| self.programming_view(ui));
            return;
        }
        if self.view == View::Computer {
            egui::CentralPanel::default().show(ui, |ui| self.computer_view(ui));
            return;
        }
        if self.view == View::Home {
            egui::CentralPanel::default().show(ui, |ui| self.home_view(ui));
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
                            tr!("Bu seviye henüz kilitli: önceki seviyeleri bitirince açılır.", "This level is still locked: it opens once the earlier levels are done.").into(),
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
                    self.design_changed_merging(out.merge);
                }
                if out.undo {
                    self.undo();
                }
                if out.redo {
                    self.redo();
                }
                let rect = ui.max_rect();
                self.guide(ui.ctx(), rect);
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
