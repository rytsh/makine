//! Clipboard regressions driven through real pointer/keyboard events.
use super::*;
use makine_sim::{out, ChipBuilder, ChipDef, Part};

struct State {
    canvas: Canvas,
    design: Design,
    board: Rect,
    copied: Option<String>,
    status: Option<String>,
    text: String,
}

fn four_nands() -> Design {
    let mut b = ChipBuilder::new("T", &["a"], &["q"]);
    let mut previous = b.input("a");
    for _ in 0..4 {
        previous = b.nand(previous, previous);
    }
    b.output("q", previous);
    let mut design = Design::new(b.build());
    design.pos = vec![[160., 80.], [360., 80.], [160., 220.], [360., 220.]];
    design
}

fn harness() -> egui_kittest::Harness<'static, State> {
    let state = State {
        canvas: Canvas::new(),
        design: four_nands(),
        board: Rect::NOTHING,
        copied: None,
        status: None,
        text: "notes".into(),
    };
    let mut h = egui_kittest::Harness::builder()
        .with_size(vec2(900., 600.))
        .build_ui_state(
            |ui, state: &mut State| {
                egui::Panel::left("controls")
                    .exact_size(150.)
                    .show(ui, |ui| {
                        let label = ui.label("Notes").id;
                        ui.text_edit_singleline(&mut state.text).labelled_by(label);
                    });
                egui::CentralPanel::default().show(ui, |ui| {
                    state.board = ui.max_rect();
                    let output =
                        state
                            .canvas
                            .show(ui, &mut state.design, &Library::new(), None, &[0], 600.);
                    if let Some(message) = output.status {
                        state.status = Some(message);
                    }
                    if let Some(text) = ui.ctx().output(|o| {
                        o.commands.iter().find_map(|cmd| match cmd {
                            egui::OutputCommand::CopyText(text) => Some(text.clone()),
                            _ => None,
                        })
                    }) {
                        state.copied = Some(text);
                    }
                });
            },
            state,
        );
    h.run();
    h
}

#[test]
fn box_select_and_copy_four_nands_even_after_pointer_leaves_canvas() {
    let mut h = harness();
    let origin = h.state().canvas.origin;
    let from = origin + vec2(130., 50.);
    let to = origin + vec2(460., 300.);
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
    h.drag_at(from);
    h.run();
    h.hover_at(to);
    h.run();
    h.drop_at(to);
    h.run();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
    assert_eq!(h.state().canvas.selected.len(), 4);
    h.hover_at(pos2(20., 150.));
    h.run();
    h.event(egui::Event::Copy);
    h.run();
    let clip = h
        .state()
        .copied
        .clone()
        .expect("copy must not silently keep the old clipboard");
    let mut pasted = Design::new(ChipDef::new("T", &[], &[]));
    assert_eq!(pasted.paste(&clip, pos2(0., 0.)).len(), 4);
    assert_eq!(
        pasted.def.wires.len(),
        6,
        "both NAND inputs preserve their internal wires"
    );
    assert!(h.state().status.as_ref().unwrap().contains('4'));
}

#[test]
fn paste_near_canvas_edge_keeps_every_gate_visible_and_relative_positions() {
    let mut h = harness();
    let clip = h.state().design.copy(&[0, 1, 2, 3]).unwrap();
    let at = h.state().board.max - vec2(5., 5.);
    h.hover_at(at);
    h.run();
    h.event(egui::Event::Paste(clip));
    h.run();
    assert_eq!(h.state().design.def.parts.len(), 8);
    assert_eq!(h.state().canvas.selected, vec![4, 5, 6, 7]);
    assert_eq!(
        h.state().design.part_pos(5) - h.state().design.part_pos(4),
        vec2(200., 0.)
    );
    let s = h.state();
    let layout = Layout::new(
        &s.design,
        &Library::new(),
        s.canvas.origin,
        s.canvas.zoom,
        true,
        600.,
    );
    for id in &s.canvas.selected {
        let (at, geometry, _) = layout.parts[*id].as_ref().unwrap();
        assert!(
            s.board.contains_rect(layout.rect(geometry.rect(*at))),
            "pasted NAND {id} must be visible"
        );
    }
}

#[test]
fn repeated_pastes_do_not_overlap_and_preserve_repeat_counts() {
    let mut design = four_nands();
    design.def.parts[0] = Part {
        kind: PartKind::Nand,
        repeat: 4,
    };
    let text = design.copy(&[0, 1, 2, 3]).unwrap();
    let mut canvas = Canvas::new();
    let mut result = CanvasOutput::default();
    canvas.paste(&mut design, &text, Some(pos2(200., 200.)), &mut result);
    canvas.paste(&mut design, &text, Some(pos2(200., 200.)), &mut result);
    assert_ne!(design.pos[4], design.pos[8]);
    assert_eq!(design.def.parts[4].repeat, 4);
    assert_eq!(design.def.parts[8].repeat, 4);
    assert_eq!(canvas.selected, vec![8, 9, 10, 11]);
}

#[test]
fn text_field_cut_does_not_remove_selected_gates() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    h.state_mut().canvas.selected = vec![0, 1, 2, 3];
    h.get_by_label("Notes").focus();
    h.run();
    h.event(egui::Event::Cut);
    h.run();
    assert_eq!(h.state().design.def.parts.len(), 4);
    assert!(h.state().status.is_none());
}

#[test]
fn copied_subcircuit_can_be_reconnected_and_simulated() {
    let original = four_nands();
    let text = original.copy(&[3, 0, 2, 1, 1]).unwrap();
    let mut design = Design::new(ChipDef::new("T", &["a"], &["q"]));
    design.paste(&text, pos2(100., 100.));
    design.def.connect(Source::Input(0), makine_sim::into(0, 0));
    design.def.connect(Source::Input(0), makine_sim::into(0, 1));
    design.def.connect(out(3, 0), Sink::Output(0));
    let mut c = Circuit::compile_def(&Library::new(), &design.def).unwrap();
    assert_eq!(c.eval_values(&[0]).unwrap(), vec![0]);
    assert_eq!(c.eval_values(&[1]).unwrap(), vec![1]);
}
