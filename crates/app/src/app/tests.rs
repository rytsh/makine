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
        crate::wiki::install_fonts(&ctx);
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
            include_str!("../worldview/render.rs"),
            include_str!("../worldsim.rs"),
            include_str!("../session.rs"),
            include_str!("../worlds.rs"),
            include_str!("../design.rs"),
            include_str!("../sandbox.rs"),
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

    /// World edits reach the disk a moment later, once for many edits,
    /// and right away when the app closes.
    #[test]
    fn world_edits_are_saved_in_batches() {
        use makine_world::{Block, Edit, Pos};
        let dir = std::env::temp_dir().join(format!("makine-save-{}", std::process::id()));
        let path = dir.join("progress.json");
        let mut app = App::new(Progress::default(), Some(path.clone()));
        app.open_world();
        let _ = std::fs::remove_file(&path);
        for x in 0..50 {
            let w = app.world.as_mut().unwrap();
            w.edit(Edit::Set {
                pos: Pos::new(x, 0, 0),
                block: Some(Block::Wire),
            });
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
        w.edit(Edit::Set {
            pos: Pos::new(0, 5, 0),
            block: Some(Block::Lamp),
        });
        app.store_world();
        eframe::App::on_exit(&mut app);
        let saved = Progress::load(&path).unwrap();
        assert_eq!(saved.worlds.values().next().unwrap().len(), 51);
        let _ = std::fs::remove_dir_all(dir);
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
