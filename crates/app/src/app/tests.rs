//! Tests of the app, driven through egui like a player would.

mod editor {
    use crate::app::*;
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
            let app = std::rc::Rc::new(std::cell::RefCell::new(app));
            let a = app.clone();
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1280.0, 720.0))
                .wgpu()
                .build_ui(move |ui| a.borrow_mut().ui(ui));
            harness.run();
            let img = harness.render().unwrap();
            let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
            std::fs::create_dir_all(&dir).unwrap();
            img.save(format!("{dir}/makine-{chip}.png")).unwrap();
            if chip == "Mux" {
                // The clear but big MUX, then the example solution.
                app.borrow_mut().run_test();
                harness.run();
                harness
                    .render()
                    .unwrap()
                    .save(format!("{dir}/makine-cost.png"))
                    .unwrap();
                let lib = app.borrow().lib.clone();
                let s = super::super::solution_ui::Solution::new(find("Mux").unwrap(), &lib, true);
                app.borrow_mut().solution = Some(s);
                harness.run();
                harness
                    .render()
                    .unwrap()
                    .save(format!("{dir}/makine-solution.png"))
                    .unwrap();
                app.borrow_mut().solution = None;
                {
                    let d = std::env::temp_dir().join("makine-shot-profiles");
                    let _ = std::fs::remove_dir_all(&d);
                    let mut list = crate::profiles::Profiles::load(&d);
                    list.add("Ayşe", &Progress::default()).unwrap();
                    app.borrow_mut().profiles = Some(list);
                }
                app.borrow_mut().go_home();
                harness.run_steps(3);
                harness
                    .render()
                    .unwrap()
                    .save(format!("{dir}/makine-home.png"))
                    .unwrap();
            }
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
            let src = crate::computer::EXAMPLES
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

        // A world: two switches into a half adder lighting two lamps, and a
        // clock blinking a lamp.
        {
            use crate::worldview::WorldView;
            use makine_world::{End, Gate, Kind, World};
            let mut w = World::new("örnek");
            let mut v = WorldView::new(World::new("örnek"));
            let mut put = |kind, x, y| v.add(kind, egui::pos2(x, y));
            let a = put(Kind::Switch { on: true }, 40.0, 40.0);
            let b = put(Kind::Switch { on: false }, 40.0, 160.0);
            let x = put(Kind::Gate(Gate::Xor), 220.0, 60.0);
            let n = put(Kind::Gate(Gate::And), 220.0, 160.0);
            let l1 = put(Kind::Lamp, 380.0, 64.0);
            let l2 = put(Kind::Lamp, 380.0, 164.0);
            let clk = put(Kind::Clock { period: 30 }, 40.0, 280.0);
            let l3 = put(Kind::Lamp, 220.0, 280.0);
            let q = put(
                Kind::Output {
                    name: "toplam".into(),
                    width: 1,
                },
                540.0,
                160.0,
            );
            for (f, t, pin) in [
                (a, x, 0),
                (b, x, 1),
                (a, n, 0),
                (b, n, 1),
                (x, l1, 0),
                (n, l2, 0),
                (clk, l3, 0),
                (x, q, 0),
            ] {
                v.edit(makine_world::Edit::Wire {
                    to: End::new(t, pin),
                    from: Some(End::new(f, 0)),
                });
            }
            std::mem::swap(&mut w, &mut v.world);
            let mut v = WorldView::new(w);
            v.selected = [x].into();
            let mut a = app.borrow_mut();
            a.view = View::World;
            a.world = Some(v);
        }
        harness.run_steps(3);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-world.png")).unwrap();

        // Devices: a keyboard into a display, a screen and a RAM; library
        // chips; a part in hand.
        let screen_id;
        {
            use crate::worldview::WorldView;
            use makine_sim::Part;
            use makine_world::{Device, End, Kind, World};
            let mut v = WorldView::new(World::new("cihazlar"));
            v.set_library(app.borrow().lib.clone());
            let kbd = v.add(Kind::Device(Device::Keyboard), egui::pos2(40.0, 40.0));
            let disp = v.add(
                Kind::Device(Device::Display { bits: 8 }),
                egui::pos2(330.0, 40.0),
            );
            let split = v.add(
                Kind::Part(Part::new(PartKind::Join(vec![7, 1]))),
                egui::pos2(160.0, 40.0),
            );
            screen_id = v.add(
                Kind::Device(Device::Screen { bits: 3 }),
                egui::pos2(40.0, 200.0),
            );
            v.add(
                Kind::Device(Device::Ram { addr: 3, data: 8 }),
                egui::pos2(300.0, 200.0),
            );
            v.add(
                Kind::Part(Part::new(PartKind::Chip("Mux".into()))),
                egui::pos2(500.0, 200.0),
            );
            for (f, fp, t, tp) in [(kbd, 0, split, 0), (kbd, 1, split, 1), (split, 0, disp, 0)] {
                v.edit(makine_world::Edit::Wire {
                    to: End::new(t, tp),
                    from: Some(End::new(f, fp)),
                });
            }
            v.selected = [kbd].into();
            v.placing = Some(Kind::Lamp);
            v.refit();
            app.borrow_mut().world = Some(v);
        }
        harness.run_steps(2);
        harness.event(egui::Event::Text("M".into()));
        harness.run_steps(3);
        // Draw a smiley on the screen through its native state.
        {
            let mut a = app.borrow_mut();
            let v = a.world.as_mut().unwrap();
            v.set_screen_for_test(screen_id, |x, y| {
                matches!((x, y), (2, 2) | (5, 2) | (1, 5) | (6, 5))
                    || (y == 6 && (2..=5).contains(&x))
            });
        }
        harness.run_steps(3);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-world-devices.png")).unwrap();

        // The workshop is an ordinary saved world, initially collapsed.
        app.borrow_mut().open_mcs4_project();
        harness.run_steps(2);
        for _ in 0..5 {
            let mut a = app.borrow_mut();
            let w = a.world.as_mut().unwrap();
            w.sim.clock_cycle(&w.world);
            w.sim.run_frame(&w.world);
        }
        harness.run_steps(2);
        harness
            .render()
            .unwrap()
            .save(format!("{dir}/makine-world-4004.png"))
            .unwrap();

        // Look into the outer module; its child modules stay closed.
        {
            let mut a = app.borrow_mut();
            let w = a.world.as_mut().unwrap();
            let root = w
                .world
                .modules()
                .max_by_key(|(_, m)| m.members.len())
                .unwrap()
                .0;
            w.enter_module(root);
        }
        harness.run_steps(2);
        harness
            .render()
            .unwrap()
            .save(format!("{dir}/makine-world-modules.png"))
            .unwrap();

        // Go back to the compact view before the multiplayer capture.
        {
            let mut a = app.borrow_mut();
            let w = a.world.as_mut().unwrap();
            w.leave_to(0);
        }

        // Building together: a guest's cursor in the host's world.
        {
            let mut a = app.borrow_mut();
            a.session = Some(crate::session::Session::host("ray", 0).unwrap());
            a.world.as_mut().unwrap().selected.clear();
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
            c.send_cursor(Some(makine_world::Pos::new(200.0, 120.0)));
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
        // A wide window: the page stays centred, the scroll bar at the edge.
        harness.set_size(egui::vec2(1920.0, 900.0));
        harness.run_steps(4);
        let img = harness.render().unwrap();
        img.save(format!("{dir}/makine-wiki-wide.png")).unwrap();
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

        // Cut the output wire: Delete over its middle (the curve is
        // symmetric, so the middle is halfway between its ends).
        let (gate_out, chip_out) = (screen(at + g.outputs[0]), screen(out_rect.left_center()));
        let mid = gate_out + (chip_out - gate_out) / 2.0;
        h.hover_at(mid);
        h.run();
        h.key_press(egui::Key::Delete);
        h.run();
        assert_eq!(app.borrow_mut().design().def.wires.len(), 2, "Delete cuts");
        drag(&mut h, gate_out, chip_out);
        assert_eq!(app.borrow_mut().design().def.wires.len(), 3);
        // Pull it off the output's end onto empty space: cut again.
        drag(&mut h, chip_out, egui::pos2(400.0, 650.0));
        assert_eq!(app.borrow_mut().design().def.wires.len(), 2, "pulled off");
        drag(&mut h, gate_out, chip_out);
        assert_eq!(app.borrow_mut().design().def.wires.len(), 3);

        h.get_by_label_contains("Test et").click();
        h.run();
        assert!(app.borrow().progress.solved("Not"));
        assert!(h.query_by_label_contains("Tebrikler").is_some());
        assert!(Progress::load(&path).unwrap().solved("Not"));

        // Undo takes back the last wire; redo puts it back.
        h.get_by_label_contains("Geri al").click();
        h.run();
        assert_eq!(app.borrow_mut().design().def.wires.len(), 2, "undo");
        h.hover_at(egui::pos2(700.0, 600.0));
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Y);
        h.run();
        assert_eq!(app.borrow_mut().design().def.wires.len(), 3, "redo");

        // Box select the gate, copy and paste it: a second NAND.
        let gate = screen(at + g.size / 2.0);
        h.key_press(egui::Key::Escape);
        h.run();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
        drag(
            &mut h,
            gate - egui::vec2(60.0, 50.0),
            gate + egui::vec2(60.0, 50.0),
        );
        h.event(egui::Event::ModifiersChanged(Default::default()));
        h.run();
        assert_eq!(app.borrow().canvas.selected, vec![0], "box select");
        let text = app.borrow_mut().design().copy(&[0]).unwrap();
        h.hover_at(egui::pos2(700.0, 500.0));
        h.run();
        h.event(egui::Event::Paste(text));
        h.run();
        assert_eq!(app.borrow_mut().design().def.parts.len(), 2, "paste");
        assert_eq!(app.borrow().canvas.selected, vec![1]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

mod glyph_tests {
    /// Every non-Turkish, non-ASCII character in the UI sources must be in
    /// the bundled fonts, or egui panics when drawing it.
    #[test]
    fn ui_symbols_have_glyphs() {
        let ctx = egui::Context::default();
        // The app installs the DejaVu fallbacks on its first frame.
        crate::theme::install(&ctx);
        let sources = [
            include_str!("../app/mod.rs"),
            include_str!("../app/world_ui.rs"),
            include_str!("../app/sandbox_ui.rs"),
            include_str!("../app/computer_ui.rs"),
            include_str!("../canvas/mod.rs"),
            include_str!("../canvas/draw.rs"),
            include_str!("../canvas/layout.rs"),
            include_str!("../canvas/menu.rs"),
            include_str!("../computer.rs"),
            include_str!("../wiki.rs"),
            include_str!("../worldview/mod.rs"),
            include_str!("../worldview/panels.rs"),
            include_str!("../worldview/draw.rs"),
            include_str!("../worldview/layout.rs"),
            include_str!("../worldview/workbench.rs"),
            include_str!("../worldview/scope.rs"),
            include_str!("../worldview/routing.rs"),
            include_str!("solution_ui.rs"),
            include_str!("chrome.rs"),
            include_str!("home_ui.rs"),
            include_str!("../profiles.rs"),
            include_str!("../../../levels/src/cost.rs"),
            include_str!("../worldsim.rs"),
            include_str!("../session.rs"),
            include_str!("../worlds.rs"),
            include_str!("../design.rs"),
            include_str!("../sandbox.rs"),
            include_str!("../theme.rs"),
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

mod play_tests {
    use crate::app::*;
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
        progress.program = Some(crate::computer::EXAMPLES[1].1.to_string()); // Renkler: runs forever
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

mod sandbox_tests {
    use crate::app::*;
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

    /// Parts in the world become a chip, which goes back in as a part.
    #[test]
    fn world_selection_to_chip_and_back() {
        use makine_world::{End, Gate, Kind};
        let app = app_with_levels();
        app.borrow_mut().open_world();
        {
            let mut a = app.borrow_mut();
            let w = a.world.as_mut().unwrap();
            let i = w.add(
                Kind::Input {
                    name: "a".into(),
                    width: 1,
                    value: 0,
                },
                egui::pos2(0.0, 0.0),
            );
            let n = w.add(Kind::Gate(Gate::Not), egui::pos2(150.0, 0.0));
            let q = w.add(
                Kind::Output {
                    name: "q".into(),
                    width: 1,
                },
                egui::pos2(300.0, 0.0),
            );
            w.edit(makine_world::Edit::Wire {
                to: End::new(n, 0),
                from: Some(End::new(i, 0)),
            });
            w.edit(makine_world::Edit::Wire {
                to: End::new(q, 0),
                from: Some(End::new(n, 0)),
            });
            w.selected = [i, n, q].into();
        }
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1300.0))
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

        // The chip list offers it; pick it and click the board to place it.
        h.get_by_label_contains("Çiplerin (").click();
        h.run_steps(2);
        h.get_by_label("Ters").click();
        h.run_steps(2);
        let placing = app.borrow().world.as_ref().unwrap().placing.clone();
        assert_eq!(placing.as_ref().and_then(|k| k.chip_name()), Some("Ters"));
        {
            let mut a = app.borrow_mut();
            let w = a.world.as_mut().unwrap();
            let t = w.add(placing.unwrap(), egui::pos2(150.0, 200.0));
            let s = w.add(Kind::Switch { on: false }, egui::pos2(0.0, 200.0));
            let l = w.add(Kind::Lamp, egui::pos2(300.0, 200.0));
            w.edit(makine_world::Edit::Wire {
                to: End::new(t, 0),
                from: Some(End::new(s, 0)),
            });
            w.edit(makine_world::Edit::Wire {
                to: End::new(l, 0),
                from: Some(End::new(t, 0)),
            });
            w.placing = None;
        }
        h.run_steps(2);
        let a = app.borrow();
        let w = a.world.as_ref().unwrap();
        assert!(w.sim.error().is_none(), "{:?}", w.sim.error());
        assert_eq!(w.sim.compiled().unwrap().problems, vec![]);
    }

    #[test]
    fn workshop_opens_without_replacing_user_world_or_unlocking_levels() {
        let mut app = App::new(Progress::default(), None);
        app.open_world();
        app.world
            .as_mut()
            .unwrap()
            .add(makine_world::Kind::Lamp, egui::pos2(0.0, 0.0));
        app.store_world();
        let before = app.progress.solutions.len();
        app.open_mcs4_project();
        assert_eq!(app.progress.worlds["dünya"].len(), 1);
        assert_eq!(app.progress.solutions.len(), before);
        let w = app.world.as_mut().unwrap();
        assert!(!w.sim.running);
        assert!(!w.collapsed.is_empty());
        w.sim.run_frame(&w.world);
        assert!(w.sim.error().is_none());
        assert!(w.sim.compiled().unwrap().problems.is_empty());
        assert_eq!(w.sim.traces.len(), 5);
    }

    /// Several worlds: create, switch, rename, delete; edits stay in the
    /// world they were made in.
    #[test]
    fn many_worlds() {
        let app = app_with_levels();
        app.borrow_mut().open_world();
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1000.0))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(2);
        let edit = |app: &Rc<RefCell<App>>, x: f32| {
            let mut a = app.borrow_mut();
            a.world
                .as_mut()
                .unwrap()
                .add(makine_world::Kind::Lamp, egui::pos2(x, 0.0));
            a.store_world();
        };
        edit(&app, 0.0);
        h.get_by_label_contains("Dünyalar (").click();
        h.run_steps(2);
        h.get_by_label("+ Yeni").click();
        h.run_steps(2);
        assert_eq!(app.borrow().world.as_ref().unwrap().world.name, "dünya 2");
        assert!(app.borrow().world.as_ref().unwrap().world.is_empty());
        edit(&app, 5.0);
        edit(&app, 6.0);
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
        use makine_world::Kind;
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
            w.add(Kind::Lamp, egui::pos2(0.0, 0.0));
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

        // Host places the free chip; the guest gets the part and the chip.
        {
            let mut h = host.borrow_mut();
            let w = h.world.as_mut().unwrap();
            w.add(
                Kind::Part(makine_sim::Part::new(PartKind::Chip("Ikili".into()))),
                egui::pos2(100.0, 100.0),
            );
        }
        pump(&|| guest.borrow().progress.sandbox.chips.contains_key("Ikili"));
        pump(&|| {
            guest.borrow().world.as_ref().unwrap().world.len()
                == host.borrow().world.as_ref().unwrap().world.len()
        });
        assert!(guest.borrow().lib.contains("Ikili"));

        // Guest edits; the host sees it and saves it.
        let id = guest
            .borrow_mut()
            .world
            .as_mut()
            .unwrap()
            .add(Kind::Clock { period: 5 }, egui::pos2(300.0, 300.0));
        pump(&|| {
            host.borrow()
                .world
                .as_ref()
                .unwrap()
                .world
                .part(id)
                .is_some()
        });
        let hname = host.borrow().world.as_ref().unwrap().world.name.clone();
        assert!(host.borrow().progress.worlds[&hname].part(id).is_some());
        // The shared world is not saved among the guest's worlds.
        assert!(guest
            .borrow()
            .progress
            .worlds
            .values()
            .all(|w| w.part(id).is_none()));

        // The host stops sharing: the guest goes back to their own world.
        host.borrow_mut().leave_session(None);
        pump(&|| guest.borrow().session.is_none());
        assert_eq!(
            guest.borrow().world.as_ref().unwrap().world.name,
            guest_world_before
        );
    }

    /// World edits reach the disk a moment later, once for many edits,
    /// and right away when the app closes.
    #[test]
    fn world_edits_are_saved_in_batches() {
        let dir = std::env::temp_dir().join(format!("makine-save-{}", std::process::id()));
        let path = dir.join("progress.json");
        let mut app = App::new(Progress::default(), Some(path.clone()));
        app.open_world();
        let _ = std::fs::remove_file(&path);
        for x in 0..50 {
            let w = app.world.as_mut().unwrap();
            w.add(makine_world::Kind::Lamp, egui::pos2(x as f32 * 50.0, 0.0));
            app.store_world();
        }
        let ctx = egui::Context::default();
        app.save_if_due(&ctx);
        assert!(!path.exists(), "not on every edit");
        app.unsaved_since = Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
        app.save_if_due(&ctx);
        let saved = Progress::load(&path).unwrap();
        assert_eq!(saved.worlds.values().next().unwrap().len(), 50);
        // Closing writes what is pending at once.
        let w = app.world.as_mut().unwrap();
        w.add(makine_world::Kind::Lamp, egui::pos2(0.0, 200.0));
        app.store_world();
        eframe::App::on_exit(&mut app);
        let saved = Progress::load(&path).unwrap();
        assert_eq!(saved.worlds.values().next().unwrap().len(), 51);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Profiles keep their own progress; switching saves the one left.
    #[test]
    fn profiles_switch_reset_and_paste() {
        let dir = std::env::temp_dir().join(format!("makine-profiles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let list = crate::profiles::Profiles::load(&dir);
        let mut app = App::with_profiles(Progress::default(), Some(list));
        app.progress
            .solutions
            .insert("Not".into(), reference("Not"));
        app.save();
        app.go_home();
        assert!(app.view == View::Home);

        app.add_profile("İkinci", Progress::default());
        assert_eq!(app.profiles.as_ref().unwrap().current().name, "İkinci");
        assert!(!app.progress.solved("Not"));
        assert!(app.view == View::Home);

        // Paste the first profile's progress over this one.
        app.home.paste = crate::profiles::export(&app.profiles.as_ref().unwrap().progress(0));
        app.confirmed(home_ui::Confirm::Replace);
        assert!(app.progress.solved("Not"));
        app.progress.symbols = false;
        app.confirmed(home_ui::Confirm::Reset);
        assert!(!app.progress.solved("Not"));
        assert!(!app.progress.symbols, "settings survive a reset");

        app.switch_profile(0);
        assert!(app.progress.solved("Not"));
        let reopened = crate::profiles::Profiles::load(&dir);
        assert_eq!(reopened.current, 0);
        assert!(!reopened.progress(1).solved("Not"));
        let _ = std::fs::remove_dir_all(dir);
    }
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
