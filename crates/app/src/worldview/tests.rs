//! World view tests: editing through the same gestures as the canvas.

use egui::{pos2, vec2, Pos2};
use makine_world::{Device, Edit, End, Gate, Id, Kind, Placed, Pos, World};

use super::*;

type Harness = egui_kittest::Harness<'static, WorldView>;

#[test]
fn duplicate_keeps_the_original_clipboard_and_repeated_pastes_separate() {
    let mut w = World::new("clipboard");
    let ids: Vec<_> = (0..4)
        .map(|i| put(&mut w, Kind::Gate(Gate::Nand), 100. + i as f32 * 150., 100.))
        .collect();
    let mut v = view(w);
    v.selected = ids[..2].iter().copied().collect();
    let text = v.copy().unwrap();
    v.selected = ids.iter().copied().collect();
    v.duplicate();
    assert_eq!(v.selected.len(), 4);
    assert_eq!(v.clipboard.as_ref().unwrap().parts.len(), 2);
    v.paste(None, pos2(100., 300.));
    let first = v.world.part(*v.selected.first().unwrap()).unwrap().pos;
    v.paste(Some(&text), pos2(100., 300.));
    let second = v.world.part(*v.selected.first().unwrap()).unwrap().pos;
    assert_ne!(
        first, second,
        "repeated paste must not hide the previous parts under itself"
    );
    assert_eq!(v.selected.len(), 2);
    let count = v.world.parts().count();
    v.paste(Some("unrelated text, not circuit parts"), pos2(100., 300.));
    assert_eq!(
        v.world.parts().count(),
        count,
        "invalid external text must not silently paste the old clipboard"
    );
}

#[test]
fn four_nands_pasted_at_the_edge_are_all_visible_and_undo_together() {
    let mut w = World::new("clipboard");
    let ids: Vec<_> = (0..4)
        .map(|i| put(&mut w, Kind::Gate(Gate::Nand), 100. + i as f32 * 200., 100.))
        .collect();
    let mut h = harness(view(w));
    h.state_mut().selected = ids.iter().copied().collect();
    h.hover_at(pos2(-10., -10.));
    h.run();
    h.event(egui::Event::Copy);
    h.run();
    assert_eq!(h.state().clipboard.as_ref().unwrap().parts.len(), 4);
    let at = h.state().drop_point();
    h.state_mut().paste(None, at + vec2(700., 450.));
    h.run();
    assert_eq!(h.state().selected.len(), 4);
    let v = h.state();
    let layout = v.layout(v.origin);
    for id in &v.selected {
        assert!(v
            .board
            .contains_rect(layout.rect(layout.part_rect(*id).unwrap())));
    }
    h.state_mut().undo();
    assert_eq!(h.state().world.parts().count(), 4);
    h.state_mut().redo();
    assert_eq!(h.state().world.parts().count(), 8);
}

fn harness(v: WorldView) -> Harness {
    let mut h = egui_kittest::Harness::builder()
        .with_size(vec2(900.0, 600.0))
        .build_ui_state(|ui, v: &mut WorldView| v.show(ui), v);
    h.run();
    h
}

/// A view with the camera fixed: world point (x, y) at screen (x, y).
fn view(w: World) -> WorldView {
    let mut v = WorldView::new(w);
    v.fit = false;
    v.minimap = false;
    v.zoom = 1.0;
    v.offset = Vec2::ZERO;
    v
}

fn put(w: &mut World, kind: Kind, x: f32, y: f32) -> Id {
    let id = w.new_id();
    w.apply(Edit::Part {
        id,
        part: Some(Placed {
            kind,
            pos: Pos::new(x, y),
        }),
    });
    id
}

/// Screen position of a part's output / input pin.
fn out_pin(h: &Harness, id: Id, pin: usize) -> Pos2 {
    let v = h.state();
    v.layout(v.origin).source_pos(End::new(id, pin)).unwrap()
}

fn in_pin(h: &Harness, id: Id, pin: usize) -> Pos2 {
    let v = h.state();
    v.layout(v.origin).sink_pos(End::new(id, pin)).unwrap()
}

fn centre(h: &Harness, id: Id) -> Pos2 {
    let v = h.state();
    let l = v.layout(v.origin);
    l.rect(l.part_rect(id).unwrap()).center()
}

fn drag(h: &mut Harness, from: Pos2, to: Pos2) {
    h.drag_at(from);
    h.run();
    let mid = from + (to - from) / 2.0;
    h.hover_at(mid);
    h.run();
    h.hover_at(to);
    h.run();
    h.drop_at(to);
    h.run();
}

fn click(h: &mut Harness, at: Pos2) {
    h.hover_at(at);
    h.run();
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    h.run();
}

fn lit(v: &WorldView, lamp: Id) -> bool {
    v.sim
        .input_value(&v.world, End::new(lamp, 0))
        .is_some_and(|x| x != 0)
}

#[test]
fn wire_by_dragging_and_toggle_a_switch() {
    let mut w = World::new("w");
    let sw = put(&mut w, Kind::Switch { on: false }, 100.0, 100.0);
    let not = put(&mut w, Kind::Gate(Gate::Not), 250.0, 100.0);
    let lamp = put(&mut w, Kind::Lamp, 400.0, 100.0);
    let mut h = harness(view(w));
    let (a, b) = (out_pin(&h, sw, 0), in_pin(&h, not, 0));
    drag(&mut h, a, b);
    let (a, b) = (out_pin(&h, not, 0), in_pin(&h, lamp, 0));
    drag(&mut h, a, b);
    assert_eq!(h.state().world.wires().count(), 2);
    h.run();
    assert!(lit(h.state(), lamp), "NOT of 0 lights the lamp");

    // Clicking the switch flips it, and the lamp goes out.
    let c = centre(&h, sw);
    click(&mut h, c);
    h.run();
    assert!(matches!(
        h.state().world.part(sw).unwrap().kind,
        Kind::Switch { on: true }
    ));
    assert!(!lit(h.state(), lamp));

    // Undo flips it back, undo again cuts the last wire.
    h.state_mut().undo();
    h.run();
    assert!(lit(h.state(), lamp));
    h.state_mut().undo();
    assert_eq!(h.state().world.wires().count(), 1);
    h.state_mut().redo();
    assert_eq!(h.state().world.wires().count(), 2);
}

#[test]
fn pulling_a_wire_off_cuts_it_and_widths_must_fit() {
    let mut w = World::new("w");
    let i = put(
        &mut w,
        Kind::Input {
            name: "a".into(),
            width: 4,
            value: 5,
        },
        50.0,
        100.0,
    );
    let not = put(&mut w, Kind::Gate(Gate::Not), 250.0, 100.0);
    let out = put(
        &mut w,
        Kind::Output {
            name: "q".into(),
            width: 4,
        },
        250.0,
        250.0,
    );
    let mut h = harness(view(w));
    // 4 bits into a 1-bit gate: refused.
    let (a, b) = (out_pin(&h, i, 0), in_pin(&h, not, 0));
    drag(&mut h, a, b);
    assert_eq!(h.state().world.wires().count(), 0);
    assert!(h.state().status.is_some());
    // Into the 4-bit output: fine.
    let (a, b) = (out_pin(&h, i, 0), in_pin(&h, out, 0));
    drag(&mut h, a, b);
    assert_eq!(
        h.state().world.wire_into(End::new(out, 0)),
        Some(End::new(i, 0))
    );
    h.run();
    assert_eq!(h.state().sim.value(End::new(i, 0)), Some(5));
    // Pull the wire's end off the output, onto empty board: cut.
    let b = in_pin(&h, out, 0);
    drag(&mut h, b, pos2(600.0, 500.0));
    assert_eq!(h.state().world.wires().count(), 0);
}

#[test]
fn move_select_copy_paste_delete() {
    let mut w = World::new("w");
    let a = put(&mut w, Kind::Gate(Gate::And), 100.0, 100.0);
    let b = put(&mut w, Kind::Gate(Gate::Or), 300.0, 100.0);
    w.apply(Edit::Wire {
        to: End::new(b, 0),
        from: Some(End::new(a, 0)),
    });
    let mut h = harness(view(w));

    // Dragging a part moves it, as one undoable edit.
    let c = centre(&h, a);
    drag(&mut h, c, c + vec2(0.0, 80.0));
    let p = h.state().world.part(a).unwrap().pos;
    assert_eq!((p.x, p.y), (100.0, 180.0));
    h.state_mut().undo();
    let p = h.state().world.part(a).unwrap().pos;
    assert_eq!((p.x, p.y), (100.0, 100.0));

    let v = h.state_mut();
    v.selected = [a, b].into();
    // Copy, paste: two new parts with a wire between them, selected.
    v.copy().unwrap();
    v.paste(None, pos2(100.0, 300.0));
    assert_eq!(v.world.len(), 4);
    assert_eq!(v.world.wires().count(), 2);
    assert_eq!(v.selected.len(), 2);
    assert!(!v.selected.contains(&a));
    // Delete takes the copies and their wire.
    v.delete_selected();
    assert_eq!(v.world.len(), 2);
    assert_eq!(v.world.wires().count(), 1);
}

#[test]
fn shift_drag_selects() {
    let mut w = World::new("w");
    let a = put(&mut w, Kind::Lamp, 100.0, 100.0);
    let b = put(&mut w, Kind::Lamp, 200.0, 100.0);
    let far = put(&mut w, Kind::Lamp, 600.0, 400.0);
    let mut h = harness(view(w));
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
    drag(&mut h, pos2(60.0, 60.0), pos2(300.0, 200.0));
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    let sel = &h.state().selected;
    assert!(sel.contains(&a) && sel.contains(&b) && !sel.contains(&far));
}

#[test]
fn placing_from_the_palette_names_ports() {
    let mut h = harness(view(World::new("w")));
    let input = Kind::Input {
        name: "a".into(),
        width: 1,
        value: 0,
    };
    h.state_mut().placing = Some(input.clone());
    for x in [100.0, 200.0, 300.0] {
        click(&mut h, pos2(x, 200.0));
    }
    let mut names: Vec<String> = h
        .state()
        .world
        .parts()
        .filter_map(|(_, p)| p.kind.port_name().map(str::to_string))
        .collect();
    names.sort();
    assert_eq!(names, ["a", "b", "c"]);
    // Still in hand; Esc lets go.
    assert!(h.state().placing.is_some());
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.state().placing.is_none());
}

#[test]
fn next_names() {
    assert_eq!(next_name("a"), "b");
    assert_eq!(next_name("z"), "aa");
    assert_eq!(next_name("x0"), "x1");
    assert_eq!(next_name("q9"), "q10");
    assert_eq!(next_name("Z"), "AA");
    assert_eq!(next_name("_"), "_1");
}

#[test]
fn clock_runs_and_keyboard_types() {
    let mut w = World::new("w");
    let clk = put(&mut w, Kind::Clock { period: 1 }, 50.0, 50.0);
    let lamp = put(&mut w, Kind::Lamp, 200.0, 50.0);
    w.apply(Edit::Wire {
        to: End::new(lamp, 0),
        from: Some(End::new(clk, 0)),
    });
    let kbd = put(&mut w, Kind::Device(Device::Keyboard), 50.0, 200.0);
    let disp = put(
        &mut w,
        Kind::Device(Device::Display { bits: 7 }),
        300.0,
        200.0,
    );
    w.apply(Edit::Wire {
        to: End::new(disp, 0),
        from: Some(End::new(kbd, 0)),
    });
    let mut v = view(w);
    v.selected = [kbd].into();
    let mut h = egui_kittest::Harness::builder()
        .with_size(vec2(900.0, 600.0))
        .build_ui_state(|ui, v: &mut WorldView| v.show(ui), v);
    h.run_steps(2);
    let before = lit(h.state(), lamp);
    h.run_steps(1);
    assert_ne!(lit(h.state(), lamp), before, "the clock ticks every frame");
    assert!(h.state().typing());
    h.event(egui::Event::Text("A".into()));
    h.run_steps(2);
    assert_eq!(
        h.state()
            .sim
            .input_value(&h.state().world, End::new(disp, 0)),
        Some(b'A' as u64)
    );
}

#[test]
fn resizing_a_device_cuts_wires_that_no_longer_fit() {
    let mut w = World::new("w");
    let a = put(
        &mut w,
        Kind::Input {
            name: "a".into(),
            width: 4,
            value: 3,
        },
        50.0,
        50.0,
    );
    let ram = put(
        &mut w,
        Kind::Device(Device::Ram { addr: 4, data: 8 }),
        200.0,
        50.0,
    );
    w.apply(Edit::Wire {
        to: End::new(ram, 0),
        from: Some(End::new(a, 0)),
    });
    let mut v = view(w);
    v.set_kind(ram, Kind::Device(Device::Ram { addr: 4, data: 4 }));
    assert_eq!(v.world.wires().count(), 1, "the address still fits");
    v.set_kind(ram, Kind::Device(Device::Ram { addr: 3, data: 4 }));
    assert_eq!(v.world.wires().count(), 0);
}

#[test]
fn chips_from_a_selection() {
    let mut w = World::new("w");
    let a = put(
        &mut w,
        Kind::Input {
            name: "a".into(),
            width: 1,
            value: 0,
        },
        50.0,
        50.0,
    );
    let not = put(&mut w, Kind::Gate(Gate::Not), 200.0, 50.0);
    let q = put(
        &mut w,
        Kind::Output {
            name: "q".into(),
            width: 1,
        },
        350.0,
        50.0,
    );
    w.apply(Edit::Wire {
        to: End::new(not, 0),
        from: Some(End::new(a, 0)),
    });
    w.apply(Edit::Wire {
        to: End::new(q, 0),
        from: Some(End::new(not, 0)),
    });
    let mut v = view(w);
    v.selected = [a, not, q].into();
    v.chip_name = "Ters".into();
    let mut h = egui_kittest::Harness::builder()
        .with_size(vec2(400.0, 700.0))
        .build_ui_state(
            |ui, v: &mut WorldView| {
                crate::theme::install(ui.ctx());
                v.selection_panel(ui);
            },
            v,
        );
    h.run();
    use egui_kittest::kittest::Queryable;
    h.get_by_label("Çip yap").click();
    h.run();
    assert_eq!(
        h.state().request,
        Some(Request::MakeChip {
            name: "Ters".into(),
            parts: {
                let mut v = vec![a, not, q];
                v.sort();
                v
            },
        })
    );
}

#[test]
fn undo_respects_other_players() {
    let mut v = view(World::new("w"));
    let id = v.add(Kind::Lamp, pos2(0.0, 0.0));
    // Someone else turns it into a switch: undoing my add must not delete
    // their part.
    v.apply_remote(Edit::Part {
        id,
        part: Some(Placed {
            kind: Kind::Switch { on: true },
            pos: Pos::new(0.0, 0.0),
        }),
    });
    assert!(!v.can_undo());
    v.undo();
    assert_eq!(v.world.len(), 1);
}

#[test]
fn modules_preserve_wires_state_layout_and_copy() {
    let mut v = view(World::new("modules"));
    let sw = v.add(Kind::Switch { on: true }, pos2(10.0, 60.0));
    let n1 = v.add(Kind::Gate(Gate::Not), pos2(180.0, 60.0));
    let n2 = v.add(Kind::Gate(Gate::Not), pos2(330.0, 60.0));
    let lamp = v.add(Kind::Lamp, pos2(500.0, 60.0));
    for (f, t) in [(sw, n1), (n1, n2), (n2, lamp)] {
        v.edit(Edit::Wire {
            to: End::new(t, 0),
            from: Some(End::new(f, 0)),
        });
    }
    v.selected = [n1, n2].into();
    let revision = v.world.circuit_revision();
    let m = v.group_selected("Tampon").unwrap();
    assert_eq!(v.world.circuit_revision(), revision);
    let l = v.layout(Pos2::ZERO);
    assert!(!l.visible(n1));
    assert!(l.sink_pos(End::new(n1, 0)).is_some());
    assert!(l.source_pos(End::new(n2, 0)).is_some());
    assert!(!l.wire_visible(End::new(n1, 0), End::new(n2, 0)));
    v.sim.run_frame(&v.world);
    assert!(lit(&v, lamp));
    let before = v.world.part(n1).unwrap().pos;
    v.toggle_module(m);
    assert!(v.layout(Pos2::ZERO).visible(n1));
    assert_eq!(v.world.part(n1).unwrap().pos, before);
    assert_eq!(v.world.circuit_revision(), revision);
    v.copy();
    v.paste(None, pos2(100.0, 300.0));
    assert_eq!(v.world.modules().count(), 2);
    assert_eq!(v.world.wires().count(), 4);
    v.undo();
    assert_eq!(v.world.modules().count(), 1);
    v.selected = [sw, n1, n2].into();
    let outer = v.group_selected("Üst modül").unwrap();
    assert_eq!(v.layout(Pos2::ZERO).modules.len(), 1);
    v.toggle_module(outer);
    assert_eq!(v.layout(Pos2::ZERO).modules.len(), 2);
}

#[test]
fn entering_a_module_shows_only_its_insides_and_ports() {
    let mut v = view(World::new("enter"));
    let sw = v.add(Kind::Switch { on: true }, pos2(10.0, 60.0));
    let n1 = v.add(Kind::Gate(Gate::Not), pos2(180.0, 60.0));
    let n2 = v.add(Kind::Gate(Gate::Not), pos2(330.0, 60.0));
    let lamp = v.add(Kind::Lamp, pos2(500.0, 60.0));
    for (f, t) in [(sw, n1), (n1, n2), (n2, lamp)] {
        v.edit(Edit::Wire {
            to: End::new(t, 0),
            from: Some(End::new(f, 0)),
        });
    }
    v.selected = [n1, n2].into();
    let inner = v.group_selected("İç").unwrap();
    v.selected = [sw, n1, n2].into();
    let outer = v.group_selected("Dış").unwrap();

    // Entering the inner module goes through the outer one.
    v.enter_module(inner);
    assert_eq!(v.scope, [outer, inner]);
    let l = v.layout(Pos2::ZERO);
    assert!(l.visible(n1) && l.visible(n2));
    assert!(!l.visible(sw) && !l.visible(lamp));
    assert!(l.modules.is_empty());
    // One port coming in (the switch), one going out (to the lamp).
    assert_eq!(l.ports.iter().filter(|p| p.incoming).count(), 1);
    assert_eq!(l.ports.iter().filter(|p| !p.incoming).count(), 1);
    assert!(l.source_pos(End::new(sw, 0)).is_some());
    assert!(l.sink_pos(End::new(lamp, 0)).is_some());
    let r = l.bounds().unwrap();
    assert!(r.width() < 1000.0);

    // A part placed inside joins the modules looked into.
    let added = v.add(Kind::Gate(Gate::And), pos2(250.0, 200.0));
    assert!(v.world.module(inner).unwrap().members.contains(&added));
    assert!(v.world.module(outer).unwrap().members.contains(&added));

    v.leave_to(1);
    assert_eq!(v.scope, [outer]);
    v.leave_to(0);
    assert!(v.scope.is_empty());
    assert!(v.layout(Pos2::ZERO).ports.is_empty());
}

#[test]
fn moving_parts_inside_does_not_move_the_closed_module() {
    let mut v = view(World::new("still"));
    let a = v.add(Kind::Gate(Gate::Not), pos2(100.0, 100.0));
    let b = v.add(Kind::Gate(Gate::Not), pos2(300.0, 100.0));
    v.selected = [a, b].into();
    let m = v.group_selected("Blok").unwrap();
    let rect = |v: &WorldView| {
        let l = v.layout(Pos2::ZERO);
        l.modules.iter().find(|s| s.id == m).unwrap().rect
    };
    let before = rect(&v);

    // Rearranging inside, as when looking into the module.
    v.enter_module(m);
    v.selected = [a].into();
    v.start_move();
    v.world.apply(Edit::Part {
        id: a,
        part: Some(Placed {
            kind: Kind::Gate(Gate::Not),
            pos: Pos::new(-400.0, -300.0),
        }),
    });
    v.finish_move();
    v.leave_to(0);
    assert_eq!(rect(&v), before);

    // Moving the whole module moves it.
    v.selected = [a, b].into();
    v.start_move();
    for id in [a, b] {
        let mut p = v.world.part(id).unwrap().clone();
        p.pos = Pos::new(p.pos.x + 50.0, p.pos.y);
        v.world.apply(Edit::Part { id, part: Some(p) });
    }
    let mut module = v.world.module(m).unwrap().clone();
    module.pos = module.pos.map(|p| Pos::new(p.x + 50.0, p.y));
    v.world.apply(Edit::Module {
        id: m,
        module: Some(module),
    });
    v.finish_move();
    assert_eq!(rect(&v).min.x, before.min.x + 50.0);
    v.undo();
    assert_eq!(rect(&v), before);
}

#[test]
fn collapsed_fanout_is_one_pin_and_reconnects_all_members() {
    let mut v = view(World::new("fanout"));
    let a = v.add(Kind::Switch { on: false }, pos2(20.0, 100.0));
    let b = v.add(Kind::Switch { on: true }, pos2(20.0, 220.0));
    let n = v.add(Kind::Gate(Gate::Not), pos2(250.0, 100.0));
    let m = v.add(Kind::Gate(Gate::Not), pos2(400.0, 100.0));
    for to in [n, m] {
        v.edit(Edit::Wire {
            to: End::new(to, 0),
            from: Some(End::new(a, 0)),
        });
    }
    v.selected = [n, m].into();
    v.group_selected("Deux").unwrap();
    let mut h = harness(v);
    assert_eq!(
        h.state().layout(h.state().origin).modules[0].inputs.len(),
        1
    );
    let (from, to) = (out_pin(&h, b, 0), in_pin(&h, n, 0));
    drag(&mut h, from, to);
    for to in [n, m] {
        assert_eq!(
            h.state().world.wire_into(End::new(to, 0)),
            Some(End::new(b, 0))
        );
    }
}

#[test]
fn rectangle_selects_collapsed_members_not_their_hidden_coordinates() {
    let mut v = view(World::new("selection"));
    let a = v.add(Kind::Lamp, pos2(100.0, 100.0));
    let b = v.add(Kind::Lamp, pos2(10000.0, 10000.0));
    v.selected = [a, b].into();
    v.group_selected("Together").unwrap();
    v.selected.clear();
    let mut h = harness(v);
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
    drag(&mut h, pos2(80.0, 80.0), pos2(410.0, 210.0));
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    assert_eq!(h.state().selected, [a, b].into());
}

#[test]
fn manual_routes_are_hit_tested_and_copied_relative_to_the_selection() {
    let mut v = view(World::new("routes"));
    let a = v.add(Kind::Switch { on: true }, pos2(50.0, 100.0));
    let b = v.add(Kind::Lamp, pos2(300.0, 100.0));
    let to = End::new(b, 0);
    v.edit(Edit::Wire {
        to,
        from: Some(End::new(a, 0)),
    });
    v.edit(Edit::Style {
        to,
        style: Some(makine_world::WireStyle {
            name: "bus".into(),
            points: vec![Pos::new(200.0, 250.0)],
        }),
    });
    let l = v.layout(Pos2::ZERO);
    assert!(
        super::routing::wire_distance(&v.world, &l, End::new(a, 0), to, pos2(200.0, 200.0))
            .unwrap()
            < 1.0
    );
    v.selected = [a, b].into();
    v.copy();
    let first = v.world.part(*v.selected.first().unwrap()).unwrap().pos;
    v.paste(None, pos2(first.x + 20.0, first.y + 40.0));
    let copied = v
        .world
        .parts()
        .find(|(id, p)| v.selected.contains(id) && matches!(p.kind, Kind::Lamp))
        .unwrap()
        .0;
    let style = v.world.wire_style(End::new(copied, 0)).unwrap();
    assert_eq!(style.name, "bus");
    assert_eq!(style.points, [Pos::new(220.0, 290.0)]);
}
