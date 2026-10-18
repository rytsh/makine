use super::*;
use crate::store::Progress;
use egui_kittest::kittest::Queryable;
use std::{cell::RefCell, rc::Rc};

fn app() -> App {
    let mut p = Progress::default();
    for l in makine_levels::levels()
        .iter()
        .take_while(|l| l.chip != "Mux")
    {
        p.solutions
            .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
    }
    p.guide_done = true;
    App::new(p, None)
}

fn harness(app: App) -> (Rc<RefCell<App>>, egui_kittest::Harness<'static>) {
    let app = Rc::new(RefCell::new(app));
    let state = app.clone();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1440., 900.))
        .build_ui(move |ui| state.borrow_mut().ui(ui));
    h.run_steps(3);
    (app, h)
}

#[test]
fn documents_preserve_independent_camera_selection_inputs_simulation_and_history() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    *a.design() = Design::new(makine_levels::reference::reference("Not"));
    a.rebuild();
    a.canvas.offset = egui::vec2(71., 19.);
    a.canvas.zoom = 0.75;
    a.canvas.selected = vec![0];
    a.inputs = vec![1];
    a.run();
    a.history.push(Design::new(a.level.blank()), false);
    let saved = a.design().clone();
    a.open_level(makine_levels::find("And").unwrap());
    a.canvas.offset = egui::vec2(99., 87.);
    a.inputs = vec![0, 1];
    a.open_level(makine_levels::find("Not").unwrap());
    assert_eq!(a.design(), &saved);
    assert_eq!(a.canvas.offset, egui::vec2(71., 19.));
    assert_eq!(a.canvas.zoom, 0.75);
    assert_eq!(a.canvas.selected, vec![0]);
    assert_eq!(a.inputs, vec![1]);
    assert_eq!(a.sim.as_ref().unwrap().output(0), 0);
    assert!(a.history.can_undo());
    assert_eq!(
        a.workspace.tabs.len(),
        3,
        "revisiting a chip must reuse its tab"
    );
    a.open_level(makine_levels::find("And").unwrap());
    assert_eq!(a.canvas.offset, egui::vec2(99., 87.));
    assert_eq!(a.inputs, vec![0, 1]);
    assert!(!a.history.can_undo());
}

#[test]
fn clocked_state_survives_switching_away_from_a_document() {
    let mut a = app();
    for l in makine_levels::levels()
        .iter()
        .take_while(|l| l.chip != "Ram8")
    {
        a.progress
            .solutions
            .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
    }
    a.refresh_library();
    a.open_level(makine_levels::find("Counter32").unwrap());
    *a.design() = Design::new(makine_levels::reference::reference("Counter32"));
    a.rebuild();
    a.inputs = vec![42, 1, 0, 0, 0];
    a.run();
    a.inputs[4] = 1;
    a.run();
    assert_eq!(a.sim.as_ref().unwrap().output(0), 42);
    a.open_level(makine_levels::find("Not").unwrap());
    // Solving an unrelated chip must not reset the parked counter either.
    a.progress.solutions.insert(
        "ProgramRom".into(),
        makine_levels::reference::reference("ProgramRom"),
    );
    a.refresh_library();
    a.open_level(makine_levels::find("Counter32").unwrap());
    assert_eq!(
        a.sim.as_ref().unwrap().output(0),
        42,
        "restoring a tab must not reset clocked state"
    );
}

#[test]
fn tab_close_saves_circuit_and_does_not_delete_the_chip_or_other_tabs() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    *a.design() = Design::new(makine_levels::reference::reference("Not"));
    let saved = a.design().clone();
    let id = a.workspace.active.unwrap();
    a.open_level(makine_levels::find("And").unwrap());
    a.close_editor_tab(id);
    assert_eq!(a.progress.designs["Not"], saved);
    assert!(!a.workspace.tabs.iter().any(|tab| tab.id == id));
    assert_eq!(a.level.chip, "And");
    a.open_level(makine_levels::find("Not").unwrap());
    assert_eq!(a.design(), &saved);
}

#[test]
fn browser_plus_reuses_existing_tabs_and_drag_reorders_them() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    a.open_level(makine_levels::find("And").unwrap());
    let before: Vec<_> = a.workspace.tabs.iter().map(|t| t.id).collect();
    let (app, mut h) = harness(a);
    let from = h
        .get_all_by_label(makine_levels::find("Not").unwrap().name())
        .last()
        .unwrap()
        .rect()
        .center();
    let to = h
        .get_all_by_label(makine_levels::find("Mux").unwrap().name())
        .last()
        .unwrap()
        .rect()
        .center();
    h.drag_at(from);
    h.run();
    h.hover_at(to);
    h.run();
    h.drop_at(to);
    h.run();
    let after: Vec<_> = app.borrow().workspace.tabs.iter().map(|t| t.id).collect();
    assert_ne!(before, after);
    assert_eq!(before.len(), after.len());
    app.borrow_mut()
        .open_level(makine_levels::find("Not").unwrap());
    h.run();
    assert_eq!(app.borrow().workspace.tabs.len(), before.len());
    assert_eq!(app.borrow().level.chip, "Not");
}

#[test]
fn detached_documents_can_dock_without_losing_unsaved_edits() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    *a.design() = Design::new(makine_levels::reference::reference("Not"));
    a.rebuild();
    let saved = a.design().clone();
    let id = a.workspace.active.unwrap();
    a.open_level(makine_levels::find("And").unwrap());
    a.detach_editor_tab(id, None);
    let (app, mut h) = harness(a);
    assert!(app
        .borrow()
        .workspace
        .tabs
        .iter()
        .any(|t| t.id == id && t.detached));
    assert!(h.get_all_by_label("Sekmeye geri al").next().is_some());
    h.get_all_by_label("Sekmeye geri al")
        .last()
        .unwrap()
        .click();
    h.run();
    assert_eq!(app.borrow().workspace.active, Some(id));
    assert_eq!(app.borrow_mut().design(), &saved);
    assert!(!app.borrow().workspace.has_detached());
}

#[test]
fn closing_last_document_goes_home_but_the_design_remains_saved() {
    let mut a = app();
    let id = a.workspace.active.unwrap();
    let target = a.target.clone();
    let saved = a.design().clone();
    a.close_editor_tab(id);
    assert!(a.view == View::Home);
    assert!(a.workspace.tabs.is_empty());
    a.open_editor_tab(target);
    assert_eq!(a.design(), &saved);
}

#[test]
fn root_and_detached_documents_share_current_library_settings() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    let id = a.workspace.active.unwrap();
    a.open_level(makine_levels::find("And").unwrap());
    a.progress.symbols = false;
    a.refresh_library();
    a.activate_editor_tab(id);
    assert!(!a.canvas.symbols);
    assert_eq!(a.level.chip, "Not");
}

#[test]
fn native_editor_window_uses_custom_chrome_and_closes_back_to_a_tab() {
    std::thread::spawn(|| {
        use std::cell::Cell;
        let close = Rc::new(Cell::new(false));
        let closing = close.clone();
        let builders = Rc::new(RefCell::new(Vec::new()));
        let recorded = builders.clone();
        egui::Context::set_immediate_viewport_renderer(move |ctx, mut viewport| {
            recorded.borrow_mut().push(viewport.builder.clone());
            let id = viewport.ids.this;
            let mut input = egui::RawInput {
                viewport_id: id,
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1100., 760.),
                )),
                ..Default::default()
            };
            input.viewports.insert(
                id,
                egui::ViewportInfo {
                    parent: Some(viewport.ids.parent),
                    events: if closing.get() {
                        vec![egui::ViewportEvent::Close]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
            );
            let mut output = ctx.run_ui(input, |ui| (viewport.viewport_ui_cb)(ui));
            output.textures_delta.clear();
        });
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut a = app();
        a.progress.custom_frame = true;
        a.open_level(makine_levels::find("Not").unwrap());
        *a.design() = Design::new(makine_levels::reference::reference("Not"));
        a.rebuild();
        a.inputs = vec![1];
        a.run();
        let id = a.workspace.active.unwrap();
        let saved = a.design().clone();
        a.open_level(makine_levels::find("And").unwrap());
        a.detach_editor_tab(id, None);
        let root = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1440., 900.),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(root(), |ui| a.ui(ui));
        out.textures_delta.clear();
        assert!(a.workspace.has_detached());
        assert_eq!(a.level.chip, "And");
        assert_eq!(builders.borrow().last().unwrap().decorations, Some(false));
        a.progress.custom_frame = false;
        let mut out = ctx.run_ui(root(), |ui| a.ui(ui));
        out.textures_delta.clear();
        assert_eq!(builders.borrow().last().unwrap().decorations, Some(true));
        close.set(true);
        let mut out = ctx.run_ui(root(), |ui| a.ui(ui));
        out.textures_delta.clear();
        assert!(!a.workspace.has_detached());
        assert_eq!(a.workspace.active, Some(id));
        assert_eq!(a.design(), &saved);
        assert_eq!(a.inputs, vec![1]);
        assert!(!out.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Close)));
    })
    .join()
    .unwrap();
}

#[test]
fn a_return_drag_can_dock_a_native_window_over_the_main_tab_bar() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    let id = a.workspace.active.unwrap();
    a.open_level(makine_levels::find("And").unwrap());
    a.detach_editor_tab(id, None);
    a.workspace
        .tabs
        .iter_mut()
        .find(|tab| tab.id == id)
        .unwrap()
        .restore_drag = true;
    let ctx = egui::Context::default();
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1440., 900.),
        )),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .inner_rect = Some(egui::Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(1440., 900.),
    ));
    input.viewports.insert(
        editor_viewport_id(id),
        egui::ViewportInfo {
            outer_rect: Some(egui::Rect::from_min_size(
                egui::pos2(250., 0.),
                egui::vec2(1100., 760.),
            )),
            ..Default::default()
        },
    );
    // Render just the strip at a known location, emulating native positions.
    let mut output = ctx.run_ui(input, |ui| a.workspace_tabs(ui));
    output.textures_delta.clear();
    assert!(!a.workspace.has_detached());
    assert_eq!(a.workspace.active, Some(id));
}

#[test]
fn many_tabs_scroll_the_selected_document_into_view() {
    let mut a = app();
    for l in makine_levels::levels().iter().take(12) {
        a.open_level(l);
    }
    let app = Rc::new(RefCell::new(a));
    let state = app.clone();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(900., 560.))
        .build_ui(move |ui| state.borrow_mut().ui(ui));
    h.run_steps(4);
    let name = app.borrow().level.name();
    let rect = h.get_by_role_and_label(egui::Role::Button, name).rect();
    assert!(
        rect.left() >= 0. && rect.right() <= 900. && rect.center().x >= 350.,
        "selected tab must be visible: {rect:?}"
    );
    for label in ["+", "Sekmeler", "Sekmeyi ayır"] {
        assert!(h.query_by_label(label).is_none(), "{label} was removed");
    }
}

#[test]
fn only_tabs_opened_after_the_first_have_a_close_button() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    a.open_level(makine_levels::find("And").unwrap());
    let first = a.workspace.tabs[0].id;
    let first_name = target_name(&a.workspace.tabs[0].target);
    let (app, mut h) = harness(a);
    assert!(h
        .query_by_label(&format!("Sekmeyi kapat: {first_name}"))
        .is_none());
    let and = makine_levels::find("And").unwrap().name();
    let close = h.get_by_label(&format!("Sekmeyi kapat: {and}")).rect();
    let tab = h
        .get_by_role_and_label(egui::Role::Button, and)
        .rect();
    assert!(tab.contains_rect(close), "× sits inside its tab");
    app.borrow_mut().activate_editor_tab(first);
    h.run();
    h.hover_at(egui::pos2(700., 400.));
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
    h.run();
    assert!(app.borrow().workspace.tabs.iter().any(|t| t.id == first));
}

#[test]
fn keyboard_cycles_documents_and_close_keeps_saved_work() {
    let mut a = app();
    a.open_level(makine_levels::find("Not").unwrap());
    a.open_level(makine_levels::find("And").unwrap());
    let (app, mut h) = harness(a);
    h.hover_at(egui::pos2(700., 400.));
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Tab);
    h.run();
    assert_eq!(app.borrow().level.chip, "Mux");
    h.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Tab,
    );
    h.run();
    assert_eq!(app.borrow().level.chip, "And");
    let before = app.borrow().progress.designs.clone();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
    h.run();
    assert!(!app
        .borrow()
        .workspace
        .tabs
        .iter()
        .any(|tab| matches!(tab.target,Target::Level(level) if level.chip == "And")));
    assert_eq!(app.borrow().progress.designs, before);
}

#[test]
fn a_free_chip_is_not_duplicated_by_rename_or_tab_navigation() {
    let mut a = app();
    a.progress
        .sandbox
        .chips
        .insert("First".into(), crate::sandbox::new_chip("First"));
    a.open_free("First".into());
    let id = a.workspace.active.unwrap();
    a.progress.sandbox.rename("First", "Renamed");
    a.target = Target::Free("Renamed".into());
    a.open_level(makine_levels::find("Not").unwrap());
    a.open_free("Renamed".into());
    assert_eq!(a.workspace.active, Some(id));
    assert_eq!(
        a.workspace
            .tabs
            .iter()
            .filter(|tab| matches!(&tab.target,Target::Free(name) if name == "Renamed"))
            .count(),
        1
    );
}

#[test]
#[ignore]
fn browser_workspace_screenshots() {
    let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
    std::fs::create_dir_all(&dir).unwrap();
    for (lang, width, height) in [
        (makine_lang::Lang::Tr, 1440., 900.),
        (makine_lang::Lang::En, 900., 560.),
    ] {
        makine_lang::set_for_thread(Some(lang));
        let mut a = app();
        a.open_level(makine_levels::find("Not").unwrap());
        a.open_level(makine_levels::find("And").unwrap());
        a.open_level(makine_levels::find("Xor").unwrap());
        *a.design() = Design::new(makine_levels::reference::reference("Xor"));
        a.design().pos = vec![[150., 80.], [300., 20.], [300., 150.], [450., 80.]];
        a.rebuild();
        a.open_solution(a.level);
        let a = Rc::new(RefCell::new(a));
        let state = a.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(width, height))
            .wgpu()
            .build_ui(move |ui| state.borrow_mut().ui(ui));
        h.run_steps(3);
        h.render()
            .unwrap()
            .save(format!("{dir}/browser-tabs-{}.png", lang.code()))
            .unwrap();
        a.borrow_mut().view = View::Home;
        a.borrow_mut().home.settings = true;
        h.run_steps(3);
        h.render()
            .unwrap()
            .save(format!("{dir}/home-settings-{}.png", lang.code()))
            .unwrap();
    }
    makine_lang::set_for_thread(None);
}
