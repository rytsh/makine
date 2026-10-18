//! Fixed context/navigation/actions, with only the selected subject scrolling.
//! This is local UI state, never part of learner progress.
use super::{App, TestResult, View};
use crate::{programming::Architecture, theme, wiki};
use egui::{RichText, Ui};
use makine_lang::{tr, trf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CircuitSection {
    #[default]
    Circuit,
    Learn,
    Compare,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CodeSection {
    #[default]
    Task,
    Files,
    Inspect,
}

#[derive(Default)]
pub(super) struct Inspector {
    pub circuit: CircuitSection,
    pub code: CodeSection,
}

impl App {
    pub(super) fn circuit_inspector(&mut self, ui: &mut Ui) {
        let level = self.level;
        ui.heading(level.name());
        ui.horizontal_wrapped(|ui| {
            let (color, text) = if self.progress.solved(level.chip) {
                (theme::GOOD, tr!("Tamamlandı", "Completed"))
            } else {
                (theme::INK_DIM, tr!("Devre çalışması", "Circuit exercise"))
            };
            ui.colored_label(color, RichText::new(text).small());
            ui.label(RichText::new(level.chip).monospace().small().weak());
        });
        ui.add_space(8.);
        ui.horizontal_wrapped(|ui| {
            for (section, label) in [
                (CircuitSection::Circuit, tr!("Devre", "Circuit")),
                (CircuitSection::Learn, tr!("Öğren", "Learn")),
                (CircuitSection::Compare, tr!("Karşılaştır", "Compare")),
            ] {
                ui.selectable_value(&mut self.inspector.circuit, section, label);
            }
        });
        ui.separator();
        egui::Panel::bottom("circuit-inspector-actions")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(0, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.undo_buttons(ui);
                    if ui.small_button(tr!("İpucu", "Hint")).clicked() {
                        self.inspector.circuit = CircuitSection::Learn;
                    }
                });
                if primary_action(ui, tr!("▶  Test et", "▶  Test"), true).clicked() {
                    self.run_test();
                    self.inspector.circuit = CircuitSection::Circuit;
                }
                if let Some(result) = &self.result {
                    let (color, label) = match result {
                        TestResult::Pass => (
                            theme::GOOD,
                            tr!(
                                "Son test: bütün adımlar geçti",
                                "Last test: every step passed"
                            ),
                        ),
                        _ => (
                            theme::WARN,
                            tr!(
                                "Test sonucunu Devre bölümünde incele",
                                "Inspect the result in Circuit"
                            ),
                        ),
                    };
                    ui.colored_label(color, RichText::new(label).small());
                }
            });
        egui::ScrollArea::vertical()
            .id_salt((
                "circuit-inspector",
                level.chip,
                self.inspector.circuit as u8,
            ))
            .auto_shrink([false, false])
            .show(ui, |ui| match self.inspector.circuit {
                CircuitSection::Circuit => self.level_panel(ui),
                CircuitSection::Learn => {
                    self.level_activities(ui);
                    ui.separator();
                    self.hints_view(ui);
                    self.circuit_teaching(ui);
                    if let Some(page) = wiki::page_for_level(level.chip) {
                        ui.add_space(8.);
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
                }
                CircuitSection::Compare => self.cost_view(ui),
            });
    }

    pub(super) fn code_inspector(&mut self, ui: &mut Ui) {
        self.workspace_header(ui);
        ui.horizontal_wrapped(|ui| {
            for (section, label) in [
                (CodeSection::Task, tr!("Görev", "Task")),
                (CodeSection::Files, tr!("Dosyalar", "Files")),
                (CodeSection::Inspect, tr!("İzle", "Inspect")),
            ] {
                ui.selectable_value(&mut self.inspector.code, section, label);
            }
        });
        ui.separator();
        if self.inspector.code == CodeSection::Task {
            self.code_task_actions(ui);
        }
        let draft = self.progress.notebook.selected;
        egui::ScrollArea::vertical()
            .id_salt(("code-inspector", draft, self.inspector.code as u8))
            .auto_shrink([false, false])
            .show(ui, |ui| match self.inspector.code {
                CodeSection::Task => self.programming_panel(ui),
                CodeSection::Files => self.program_files(ui),
                CodeSection::Inspect => {
                    if self.view == View::World {
                        self.world_code_actions(ui);
                    }
                    self.program_inspection(ui);
                }
            });
    }

    fn code_task_actions(&mut self, ui: &mut Ui) {
        let draft = self.progress.notebook.current();
        let mini_id = draft
            .and_then(|d| d.activity.as_deref())
            .and_then(|key| key.strip_prefix("mini-lesson:"))
            .and_then(crate::mini_lessons::find)
            .map(|l| l.id);
        let architecture = draft.map(|d| d.architecture);
        let lesson = self
            .debug
            .lesson
            .as_deref()
            .and_then(|id| crate::lessons::all().into_iter().find(|l| l.id == id));
        if mini_id.is_none() && (architecture != Some(Architecture::RiscV) || lesson.is_none()) {
            return;
        }
        egui::Panel::bottom("code-inspector-actions")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(0, 8)),
            )
            .show(ui, |ui| {
                if let Some(id) = mini_id.filter(|_| architecture == Some(Architecture::Mini)) {
                    let ready = crate::mini_lessons::unlocked(id, &self.progress.notebook.solved);
                    if primary_action(
                        ui,
                        tr!("Mini kod görevini sına", "Check Mini code task"),
                        ready,
                    )
                    .clicked()
                    {
                        self.check_mini_task();
                    }
                } else if let Some(lesson) =
                    lesson.filter(|_| architecture == Some(Architecture::RiscV))
                {
                    let ready = crate::lessons::unlocked(lesson.id, &self.progress.notebook.solved);
                    if primary_action(ui, tr!("Görevi test et", "Test task"), ready).clicked() {
                        self.test_program(lesson.id);
                    }
                }
            });
    }
}

fn primary_action(ui: &mut Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).strong().color(if enabled {
            theme::ON_COPPER
        } else {
            theme::INK_DIM
        }))
        .min_size(egui::vec2(ui.available_width(), 36.))
        .fill(if enabled {
            theme::COPPER
        } else {
            theme::RAISED
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{curriculum::Activity, store::Progress};
    use egui_kittest::kittest::Queryable;
    use std::{cell::RefCell, rc::Rc};

    fn app(chip: &str) -> App {
        let mut p = Progress::default();
        for l in makine_levels::levels()
            .iter()
            .take_while(|l| l.chip != chip)
        {
            p.solutions
                .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
        }
        p.guide_done = true;
        let mut app = App::new(p, None);
        app.open_level(makine_levels::find(chip).unwrap());
        app
    }

    fn harness(app: App, size: egui::Vec2) -> (Rc<RefCell<App>>, egui_kittest::Harness<'static>) {
        let app = Rc::new(RefCell::new(app));
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(size)
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(3);
        (app, h)
    }

    #[test]
    fn circuit_actions_stay_visible_while_learning_content_scrolls() {
        let mut a = app("RomSequencer");
        a.progress.hints.insert("RomSequencer".into(), 3);
        let (app, mut h) = harness(a, egui::vec2(900., 560.));
        h.get_by_label("Öğren").click();
        h.run();
        let button = h.get_by_label("▶  Test et").rect();
        assert!(button.bottom() < 560. && button.top() > 400.);
        h.hover_at(egui::pos2(760., 300.));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0., -1800.),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: Default::default(),
        });
        h.run_steps(3);
        assert_eq!(h.get_by_label("▶  Test et").rect(), button);
        assert!(h.get_by_label("Devre").rect().top() < 220.);
        h.get_by_label("▶  Test et").click();
        h.run();
        assert!(app.borrow().result.is_some());
        assert_eq!(app.borrow().inspector.circuit, CircuitSection::Circuit);
    }

    #[test]
    fn sections_keep_learning_comparison_and_files_out_of_the_main_task() {
        let (app, mut h) = harness(app("ProgramRom"), egui::vec2(1280., 800.));
        assert!(h.query_by_label("💡 İpucu al").is_none());
        assert!(h.query_by_label("💡 Çözümü gör").is_none());
        h.get_by_label("Karşılaştır").click();
        h.run();
        assert!(h.query_by_label("💡 Çözümü gör").is_some());
        h.get_by_label("Öğren").click();
        h.run();
        assert!(h.query_by_label("💡 İpucu al").is_some());
        app.borrow_mut().open_activity(Activity::Mini("mini-rom"));
        h.run();
        assert!(h.query_by_label("Mini kod görevini sına").is_some());
        assert!(h.query_by_label("Yeni").is_none());
        h.get_by_label("Dosyalar").click();
        h.run();
        assert!(h.query_by_label("Yeni").is_some());
        assert!(h.query_by_label("Mini kod görevini sına").is_none());
        h.get_by_label("Görev").click();
        h.run();
        assert!(h.query_by_label("Mini kod görevini sına").is_some());
    }

    #[test]
    fn saved_drafts_and_notes_survive_inspector_navigation() {
        let mut a = app("ProgramRom");
        a.open_activity(Activity::Mini("mini-rom"));
        a.progress.notebook.current_mut().unwrap().source = "unfinished work".into();
        a.progress
            .notebook
            .lesson_notes
            .insert("mini:mini-rom".into(), "my prediction".into());
        let before = a.progress.clone();
        let (app, mut h) = harness(a, egui::vec2(900., 560.));
        for tab in ["Dosyalar", "İzle", "Görev"] {
            h.get_by_label(tab).click();
            h.run();
        }
        assert_eq!(app.borrow().progress, before);
        let button = h.get_by_label("Mini kod görevini sına").rect();
        assert!(button.bottom() <= 560.);
    }

    #[test]
    fn inspect_shows_live_mini_and_risc_v_state_without_replacing_the_draft() {
        let mut a = app("ProgramRom");
        a.open_activity(Activity::Mini("mini-rom"));
        let (app, mut h) = harness(a, egui::vec2(1440., 900.));
        h.get_by_label("Derle").click();
        h.run();
        h.get_by_label("Devreye yükle").click();
        h.run();
        h.get_by_label("Adım").click();
        h.run();
        h.get_by_label("İzle").click();
        h.run();
        assert!(h.query_by_label_contains("PC: 1   acc: 3").is_some());
        assert!(h.query_by_label("Canlı RAM").is_some());
        let source = app
            .borrow()
            .progress
            .notebook
            .current()
            .unwrap()
            .source
            .clone();
        h.get_by_label("Dosyalar").click();
        h.run();
        h.get_by_label("Görev").click();
        h.run();
        assert_eq!(
            app.borrow().progress.notebook.current().unwrap().source,
            source
        );
        app.borrow_mut()
            .open_activity(Activity::Program("register"));
        app.borrow_mut()
            .progress
            .notebook
            .current_mut()
            .unwrap()
            .source = "li a0,42\nebreak".into();
        h.run();
        h.get_by_label("Derle").click();
        h.run();
        h.get_by_label("Baştan yükle").click();
        h.run();
        h.get_by_label("Bir kaynak satırı").click();
        h.run();
        h.get_by_label("İzle").click();
        h.run();
        assert!(h.query_by_label("Motor: referans işlemci").is_some());
        assert_eq!(
            app.borrow().computer.as_ref().unwrap().registers().unwrap()[10],
            42
        );
    }

    #[test]
    fn circuit_navigation_and_actions_fit_both_languages() {
        for lang in makine_lang::Lang::ALL {
            makine_lang::set_for_thread(Some(lang));
            let (_, mut h) = harness(app("Counter32"), egui::vec2(900., 560.));
            for label in [
                tr!("Devre", "Circuit"),
                tr!("Öğren", "Learn"),
                tr!("Karşılaştır", "Compare"),
                tr!("▶  Test et", "▶  Test"),
            ] {
                let rect = h.get_by_label(label).rect();
                assert!(
                    rect.left() >= 0. && rect.right() <= 900. && rect.bottom() <= 560.,
                    "{label}: {rect:?}"
                );
            }
            h.get_by_label(tr!("İpucu", "Hint")).click();
            h.run();
            assert!(h
                .query_by_label(tr!("💡 İpucu al", "💡 Get a hint"))
                .is_some());
        }
        makine_lang::set_for_thread(None);
    }

    #[test]
    #[ignore]
    fn inspector_screenshots() {
        let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
        std::fs::create_dir_all(&dir).unwrap();
        for (lang, width, height) in [
            (makine_lang::Lang::Tr, 1440., 900.),
            (makine_lang::Lang::Tr, 900., 560.),
            (makine_lang::Lang::En, 900., 560.),
        ] {
            makine_lang::set_for_thread(Some(lang));
            let mut a = app("ProgramRom");
            a.open_level(makine_levels::find("Xor").unwrap());
            *a.design() = crate::design::Design::new(makine_levels::reference::reference("Xor"));
            a.design().pos = vec![[180., 100.], [320., 40.], [320., 180.], [460., 100.]];
            a.rebuild();
            a.update_cost();
            let app = Rc::new(RefCell::new(a));
            let state = app.clone();
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(width, height))
                .wgpu()
                .build_ui(move |ui| state.borrow_mut().ui(ui));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!(
                    "{dir}/inspector-circuit-{}-{width}.png",
                    lang.code()
                ))
                .unwrap();
            app.borrow_mut().open_activity(Activity::Mini("mini-rom"));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/inspector-code-{}-{width}.png", lang.code()))
                .unwrap();
            if width > 1000. {
                h.get_by_label(tr!("Derle", "Build")).click();
                h.run();
                h.get_by_label(tr!("Devreye yükle", "Load into circuit"))
                    .click();
                h.run();
                h.get_by_label(tr!("Adım", "Step")).click();
                h.run();
                for (section, name) in [
                    (CodeSection::Files, "files"),
                    (CodeSection::Inspect, "inspect"),
                ] {
                    app.borrow_mut().inspector.code = section;
                    h.run_steps(3);
                    h.render()
                        .unwrap()
                        .save(format!("{dir}/inspector-code-{name}.png"))
                        .unwrap();
                }
                app.borrow_mut().editor_code = false;
                app.borrow_mut()
                    .open_level(makine_levels::find("Xor").unwrap());
                for (section, name) in [
                    (CircuitSection::Learn, "learn"),
                    (CircuitSection::Compare, "compare"),
                ] {
                    app.borrow_mut().inspector.circuit = section;
                    h.run_steps(3);
                    h.render()
                        .unwrap()
                        .save(format!("{dir}/inspector-circuit-{name}.png"))
                        .unwrap();
                }
            }
        }
        makine_lang::set_for_thread(None);
    }
}
