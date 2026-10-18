use super::*;
use crate::{design::Design, store::Progress};
use egui_kittest::kittest::Queryable;
use std::{cell::RefCell, rc::Rc};

fn app(chip: &str) -> App {
    let mut p = Progress::default();
    for l in levels().iter().take_while(|l| l.chip != chip) {
        p.solutions
            .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
    }
    let mut app = App::new(p, None);
    app.open_level(find(chip).unwrap());
    app
}

fn harness(app: App) -> (Rc<RefCell<App>>, egui_kittest::Harness<'static>) {
    let app = Rc::new(RefCell::new(app));
    let state = app.clone();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280., 800.))
        .build_ui(move |ui| state.borrow_mut().ui(ui));
    h.run_steps(3);
    (app, h)
}

#[test]
fn search_is_bilingual_case_and_turkish_accent_insensitive() {
    let app = app("Cpu");
    let entries = app.palette_entries();
    let has = |query: &str, chip: &str| {
        entries
            .iter()
            .any(|e| e.matches(query) && e.kind == PartKind::Chip(chip.into()))
    };
    for query in [
        "Counter32",
        "program sayaci",
        "PROGRAM SAYACI",
        "PROGRAM SAYACI",
        "program counter",
        "Program Sayacı",
    ] {
        assert!(has(query, "Counter32"), "{query}");
    }
    assert!(has("SOLA KAYDIRICI", "ShiftLeft32"));
    assert!(has("left shifter", "ShiftLeft32"));
    for lang in makine_lang::Lang::ALL {
        makine_lang::set_for_thread(Some(lang));
        assert!(app
            .palette_entries()
            .iter()
            .any(|e| e.matches("birlestir") && matches!(e.kind, PartKind::Join(_))));
        assert!(app
            .palette_entries()
            .iter()
            .any(|e| e.matches("split") && matches!(e.kind, PartKind::Split(_))));
    }
    makine_lang::set_for_thread(None);
}

#[test]
fn palette_keeps_unlocks_optional_sections_and_recursive_chip_rules() {
    let mut app = app("Not");
    assert_eq!(app.palette_entries().len(), 1);
    // Even a stored later completion must not expose a forbidden future chip.
    app.progress.solutions.insert(
        "Adder32".into(),
        makine_levels::reference::reference("Adder32"),
    );
    assert!(app.palette_entries().iter().all(|e| !e.matches("Adder32")));
    let mut app = self::app("Cpu");
    assert!(app.palette_entries().iter().all(|e|!matches!(e.group,Group::Section(section) if section == makine_levels::memory::SECTION || section == makine_levels::mini::SECTION)));
    app.progress
        .sandbox
        .chips
        .insert("Leaf".into(), crate::sandbox::new_chip("Leaf"));
    let mut parent = makine_sim::ChipBuilder::new("Parent", &["a", "b"], &["out"]);
    let a = parent.input("a");
    let value = parent.chip1("Leaf", &[a]);
    parent.output("out", value);
    app.progress
        .sandbox
        .chips
        .insert("Parent".into(), Design::new(parent.build()));
    app.open_free("Leaf".into());
    assert!(app
        .palette_entries()
        .iter()
        .all(|e| !matches!(&e.kind,PartKind::Chip(name) if name == "Parent" || name == "Leaf")));
}

#[test]
fn search_opens_matching_groups_clear_and_escape_restore_the_palette() {
    let (app, mut h) = harness(app("Cpu"));
    h.get_by_label("Parça ara").focus();
    h.run();
    h.get_by_label("Parça ara").type_text("program sayaci");
    h.run();
    assert!(h.query_by_label("Counter32").is_some());
    assert!(h.query_by_label("NAND").is_none());
    h.get_by_label("Aramayı temizle").click();
    h.run();
    assert!(app.borrow().parts_browser.query.is_empty());
    assert!(h.query_by_label("NAND").is_some());
    h.get_by_label("Parça ara").focus();
    h.run();
    h.get_by_label("Parça ara").type_text("doesnotexist");
    h.run();
    assert!(h.query_by_label("Eşleşme yok").is_some());
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(app.borrow().parts_browser.query.is_empty());
    assert!(h.query_by_label("NAND").is_some());
}

#[test]
fn filtered_part_can_still_be_dragged_to_the_circuit() {
    let (app, mut h) = harness(app("Cpu"));
    app.borrow_mut().parts_browser.query = "program sayaci".into();
    h.run_steps(2);
    let from = h.get_by_label("Counter32").rect().center();
    let to = egui::pos2(650., 350.);
    h.drag_at(from);
    h.run();
    h.hover_at(to);
    h.run();
    h.drop_at(to);
    h.run();
    assert!(app
        .borrow_mut()
        .design()
        .def
        .parts
        .iter()
        .any(|p| p.kind == PartKind::Chip("Counter32".into())));
}

#[test]
fn changing_target_clears_search_without_altering_circuits() {
    let mut app = app("Cpu");
    let before = app.progress.clone();
    app.parts_browser.query = "counter".into();
    app.open_level(find("Xor").unwrap());
    assert!(app.parts_browser.query.is_empty());
    assert_eq!(app.progress.solutions, before.solutions);
}

#[test]
#[ignore]
fn parts_browser_screenshots() {
    let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
    std::fs::create_dir_all(&dir).unwrap();
    for (lang, width, height) in [
        (makine_lang::Lang::Tr, 1440., 900.),
        (makine_lang::Lang::En, 900., 560.),
    ] {
        makine_lang::set_for_thread(Some(lang));
        let app = Rc::new(RefCell::new(app("Cpu")));
        let state = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(width, height))
            .wgpu()
            .build_ui(move |ui| state.borrow_mut().ui(ui));
        h.run_steps(3);
        h.render()
            .unwrap()
            .save(format!("{dir}/parts-groups-{}.png", lang.code()))
            .unwrap();
        app.borrow_mut().parts_browser.query = "program sayaci".into();
        h.run_steps(3);
        h.render()
            .unwrap()
            .save(format!("{dir}/parts-search-{}.png", lang.code()))
            .unwrap();
    }
    makine_lang::set_for_thread(None);
}
