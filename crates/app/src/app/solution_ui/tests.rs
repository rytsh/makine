use super::*;
use crate::{app::View, store::Progress};
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
    let mut app = App::new(p, None);
    app.open_level(makine_levels::find("Xor").unwrap());
    let mut design = Design::new(makine_levels::reference::reference("Xor"));
    design.pos = vec![[180., 100.], [340., 40.], [340., 180.], [500., 100.]];
    *app.design() = design.clone();
    app.snapshot = Some(design);
    app.rebuild();
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
fn example_tabs_preserve_both_circuits_camera_inputs_and_user_history() {
    let mut app = app();
    app.canvas.offset = vec2(23., 47.);
    app.canvas.zoom = 0.8;
    app.canvas.selected = vec![0, 1];
    app.inputs = vec![1, 0];
    let original = app.design().clone();
    app.history.push(Design::new(app.level.blank()), false);
    app.open_solution(app.level);
    let (app, mut h) = harness(app, vec2(1280., 800.));
    assert!(app.borrow().showing_solution());
    let example_pos = {
        let mut app = app.borrow_mut();
        let s = app.solution.as_mut().unwrap();
        s.design.pos[0][1] += 55.;
        s.inputs = vec![0, 1];
        s.run();
        s.design.pos.clone()
    };
    h.get_by_label(makine_levels::find("Xor").unwrap().name())
        .click();
    h.run();
    {
        let mut a = app.borrow_mut();
        assert!(!a.showing_solution());
        assert_eq!(a.design(), &original);
        assert_eq!(a.canvas.offset, vec2(23., 47.));
        assert_eq!(a.canvas.zoom, 0.8);
        assert_eq!(a.canvas.selected, vec![0, 1]);
        assert_eq!(a.inputs, vec![1, 0]);
        assert!(a.history.can_undo());
    }
    let label = format!("Örnek: {}", makine_levels::find("Xor").unwrap().name());
    h.get_by_label(&label).click();
    h.run();
    assert!(app.borrow().showing_solution());
    assert_eq!(
        app.borrow().solution.as_ref().unwrap().design.pos,
        example_pos
    );
    assert_eq!(app.borrow().solution.as_ref().unwrap().inputs, vec![0, 1]);
    h.get_by_label("Örnek sekmesini kapat").click();
    h.run();
    assert!(app.borrow().solution.is_none());
    assert_eq!(app.borrow_mut().design(), &original);
}

#[test]
fn example_can_detach_and_dock_without_closing_or_changing_the_users_work() {
    let mut a = app();
    let original = a.design().clone();
    let progress = a.progress.solutions.clone();
    a.open_solution(a.level);
    let (app, mut h) = harness(a, vec2(1440., 900.));
    app.borrow_mut().detach_solution(None);
    h.run();
    assert!(app.borrow().solution.as_ref().unwrap().detached);
    assert!(!app.borrow().showing_solution());
    // Headless backends use a floating window instead of an OS window.
    h.get_all_by_label("Sekmeye geri al")
        .last()
        .unwrap()
        .click();
    h.run();
    assert!(!app.borrow().solution.as_ref().unwrap().detached);
    assert!(!app.borrow().showing_solution());
    assert_eq!(app.borrow_mut().design(), &original);
    assert_eq!(app.borrow().progress.solutions, progress);
}

#[test]
fn dragging_the_example_tab_down_detaches_it() {
    let mut a = app();
    a.open_solution(a.level);
    let (app, mut h) = harness(a, vec2(1280., 800.));
    let label = format!("Örnek: {}", app.borrow().level.name());
    let from = h.get_by_label(&label).rect().center();
    let to = from + vec2(0., 100.);
    h.drag_at(from);
    h.run();
    h.hover_at(to);
    h.run();
    h.drop_at(to);
    h.run();
    assert!(app.borrow().solution.as_ref().unwrap().detached);
    assert!(!app.borrow().showing_solution());
}

#[test]
fn a_detached_example_stays_on_its_original_level_when_navigating() {
    let mut a = app();
    a.open_solution(a.level);
    a.detach_solution(None);
    a.open_level(makine_levels::find("And").unwrap());
    assert_eq!(a.solution.as_ref().unwrap().chip, "Xor");
    assert_eq!(a.level.chip, "And");
    let (app, mut h) = harness(a, vec2(1440., 900.));
    assert!(h.get_all_by_label_contains("Örnek çözüm:").next().is_some());
    app.borrow_mut().view = View::Wiki;
    h.run();
    assert!(app.borrow().solution.as_ref().unwrap().detached);
    app.borrow_mut().dock_solution();
    h.run();
    assert!(app.borrow().view == View::Wiki);
    assert!(!app.borrow().solution.as_ref().unwrap().detached);
}

#[test]
fn tabs_remain_accessible_in_both_languages_at_the_minimum_window_size() {
    for lang in makine_lang::Lang::ALL {
        makine_lang::set_for_thread(Some(lang));
        let mut a = app();
        a.open_solution(a.level);
        let (app, mut h) = harness(a, vec2(900., 560.));
        for label in [
            makine_levels::find("Xor").unwrap().name(),
            tr!("Örnek sekmesini kapat", "Close example tab"),
        ] {
            let rect = h.get_by_label(label).rect();
            assert!(
                rect.left() >= 0. && rect.right() <= 900. && rect.bottom() < 560.,
                "{label}: {rect:?}"
            );
        }
        assert!(
            app.borrow().solution.as_ref().unwrap().canvas.zoom > 0.5,
            "small windows must leave enough room to read the example"
        );
        h.get_by_label(makine_levels::find("Xor").unwrap().name())
            .click();
        h.run();
        assert!(!app.borrow().showing_solution());
    }
    makine_lang::set_for_thread(None);
}

#[test]
fn see_solution_opens_a_tab_and_can_return_without_revealing_again() {
    let (app, mut h) = harness(app(), vec2(1280., 800.));
    h.get_by_label("Karşılaştır").click();
    h.run();
    h.get_by_label("💡 Örnek çözümü göster").scroll_to_me();
    h.run();
    h.get_by_label("💡 Örnek çözümü göster").click();
    h.run();
    assert!(app.borrow().showing_solution());
    h.get_by_label(makine_levels::find("Xor").unwrap().name())
        .click();
    h.run();
    assert!(!app.borrow().showing_solution());
    let label = format!("Örnek: {}", app.borrow().level.name());
    h.get_by_label(&label).click();
    h.run();
    assert!(app.borrow().showing_solution());
}

#[test]
fn native_viewport_close_docks_the_example_without_closing_the_main_window() {
    // Isolate egui's thread-local renderer. This emulates backend events,
    // not a real OS window or a physical second monitor.
    std::thread::spawn(|| {
        use std::cell::Cell;
        let close = Rc::new(Cell::new(false));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let close_event = close.clone();
        let seen_output = seen.clone();
        egui::Context::set_immediate_viewport_renderer(move |ctx, mut viewport| {
            let id = viewport.ids.this;
            let builder = viewport.builder.clone();
            let mut input = egui::RawInput {
                viewport_id: id,
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.))),
                ..Default::default()
            };
            input.viewports.insert(
                id,
                egui::ViewportInfo {
                    parent: Some(viewport.ids.parent),
                    events: if close_event.get() {
                        vec![egui::ViewportEvent::Close]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
            );
            let mut out = ctx.run_ui(input, |ui| (viewport.viewport_ui_cb)(ui));
            out.textures_delta.clear();
            seen_output.borrow_mut().push((id, builder));
        });
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut app = app();
        let original = app.design().clone();
        app.open_solution(app.level);
        app.detach_solution(Some(egui::pos2(1600., 120.)));
        let id = app.solution.as_ref().unwrap().viewport_id();
        let root_input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1280., 800.))),
            ..Default::default()
        };
        let mut first = ctx.run_ui(root_input(), |ui| app.ui(ui));
        first.textures_delta.clear();
        assert!(app.solution.as_ref().unwrap().detached);
        assert_eq!(seen.borrow()[0].0, id);
        assert_ne!(id, egui::ViewportId::ROOT);
        assert_eq!(
            seen.borrow()[0].1.decorations,
            Some(!app.progress.custom_frame)
        );
        assert_eq!(seen.borrow()[0].1.resizable, Some(true));
        assert_eq!(seen.borrow()[0].1.position, Some(egui::pos2(1600., 120.)));
        close.set(true);
        let mut output = ctx.run_ui(root_input(), |ui| app.ui(ui));
        output.textures_delta.clear();
        assert!(!app.solution.as_ref().unwrap().detached);
        assert!(!output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|cmd| matches!(cmd, egui::ViewportCommand::Close)));
        assert_eq!(app.design(), &original);
        assert!(!app.showing_solution());
    })
    .join()
    .unwrap();
}

#[test]
#[ignore]
fn solution_workspace_screenshots() {
    let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
    std::fs::create_dir_all(&dir).unwrap();
    for (lang, width, height) in [
        (makine_lang::Lang::Tr, 1440., 900.),
        (makine_lang::Lang::Tr, 900., 560.),
        (makine_lang::Lang::En, 900., 560.),
    ] {
        makine_lang::set_for_thread(Some(lang));
        let mut a = app();
        a.open_solution(a.level);
        let a = Rc::new(RefCell::new(a));
        let state = a.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(width, height))
            .wgpu()
            .build_ui(move |ui| state.borrow_mut().ui(ui));
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(format!("{dir}/solution-tab-{}-{width}.png", lang.code()))
            .unwrap();
        if width > 1000. {
            a.borrow_mut().solution.as_mut().unwrap().active = false;
            h.run_steps(3);
            h.render()
                .unwrap()
                .save(format!("{dir}/solution-my-circuit.png"))
                .unwrap();
            a.borrow_mut().detach_solution(None);
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/solution-detached-fallback.png"))
                .unwrap();
        }
    }
    makine_lang::set_for_thread(None);
}
