//! Tests of the world view: projection, editing, devices, selection and
//! settings panels, driven through egui like a player would.

use makine_world::block::Gate;

mod basics {
    use crate::worldview::*;

    /// The floor layer has no limit, nor have blocks.
    #[test]
    fn layers_have_no_limit() {
        let mut v = WorldView::default();
        v.set_layer(20);
        assert_eq!(v.layer, 20);
        v.set_layer(-5);
        assert_eq!(v.layer, -5);
        v.edit(Edit::Set {
            pos: Pos::new(0, 0, -5),
            block: Some(Block::Wire),
        });
        assert_eq!(v.world.len(), 1);
    }

    #[test]
    fn edits_undo_redo() {
        let mut v = WorldView::default();
        v.edit(Edit::Set {
            pos: Pos::new(0, 0, 0),
            block: Some(Block::Wire),
        });
        v.edit(Edit::Set {
            pos: Pos::new(1, 0, 0),
            block: Some(Block::Lamp),
        });
        assert_eq!(v.world.len(), 2);
        v.undo();
        assert_eq!(v.world.len(), 1);
        v.redo();
        assert_eq!(v.world.get(Pos::new(1, 0, 0)), Some(&Block::Lamp));
    }

    #[test]
    fn clock_and_lamp_run() {
        // clock -> wire -> lamp; the lamp blinks with the clock.
        let mut v = WorldView::default();
        v.edit(Edit::Set {
            pos: Pos::new(0, 0, 0),
            block: Some(Block::Clock {
                period: 2,
                facing: Dir::East,
            }),
        });
        v.edit(Edit::Set {
            pos: Pos::new(1, 0, 0),
            block: Some(Block::Wire),
        });
        v.edit(Edit::Set {
            pos: Pos::new(2, 0, 0),
            block: Some(Block::Lamp),
        });
        let mut lit = Vec::new();
        for _ in 0..8 {
            v.sim.run_frame(&v.world, false);
            lit.push(v.sim.lamp_lit(Pos::new(2, 0, 0)) as u64);
        }
        // Steps 1..=8 with period 2: high on steps 2-3 and 6-7.
        assert_eq!(lit, vec![0, 1, 1, 0, 0, 1, 1, 0]);
    }
}

/// Playing like in Minecraft: the harness clicks to take the mouse, the
/// eye is put where it looks at a point, keys fly and buttons break, build
/// and pick.
pub(super) mod play {
    use crate::worldview::*;
    use egui::PointerButton;
    use glam::DVec3;

    pub type H = egui_kittest::Harness<'static, WorldView>;

    pub fn harness(mut v: WorldView) -> H {
        v.fit = false;
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui_state(|ui, v: &mut WorldView| v.show(ui), v);
        h.run();
        // A click takes the mouse.
        let c = h.state().rect.center();
        press(&mut h, c, PointerButton::Primary);
        assert!(h.state().locked);
        h
    }

    pub fn press(h: &mut H, p: egui::Pos2, button: PointerButton) {
        h.event(egui::Event::PointerMoved(p));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: p,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.run();
    }

    /// Look at world point `at` from `from` and press `button`.
    pub fn act(h: &mut H, from: DVec3, at: DVec3, button: PointerButton) {
        h.state_mut().camera.look_from(from, at);
        h.run();
        let c = h.state().rect.center();
        press(h, c, button);
    }

    fn above(x: f64, y: f64, z: f64) -> DVec3 {
        DVec3::new(x - 2.0, y - 1.5, z + 5.0)
    }

    /// Right click builds against the face looked at: on the floor, on top
    /// of a block, beside it, under it; left click breaks.
    #[test]
    fn build_and_break_with_the_crosshair() {
        let mut v = WorldView::new(World::new("s"));
        v.brush = Some(Brush { block: Block::Lamp });
        let mut h = harness(v);
        use PointerButton::{Primary, Secondary};
        let floor = DVec3::new(0.5, 0.5, 0.0);
        act(&mut h, above(0.5, 0.5, 0.0), floor, Secondary);
        assert_eq!(h.state().world.get(Pos::new(0, 0, 0)), Some(&Block::Lamp));
        // Its top (height 0.75): on top.
        let top = DVec3::new(0.5, 0.5, 0.75);
        act(&mut h, above(0.5, 0.5, 0.75), top, Secondary);
        assert_eq!(h.state().world.get(Pos::new(0, 0, 1)), Some(&Block::Lamp));
        // The east side of the bottom lamp, seen from the east: beside it.
        let side = DVec3::new(0.88, 0.5, 0.3);
        act(&mut h, DVec3::new(5.0, 0.5, 0.4), side, Secondary);
        assert_eq!(h.state().world.get(Pos::new(1, 0, 0)), Some(&Block::Lamp));
        // From underground, the bottom of the bottom lamp: under it.
        let bottom = DVec3::new(0.5, 0.5, 0.0);
        act(&mut h, DVec3::new(-1.5, -1.0, -5.0), bottom, Secondary);
        assert_eq!(h.state().world.get(Pos::new(0, 0, -1)), Some(&Block::Lamp));
        assert_eq!(h.state().world.len(), 4);
        // Left click breaks the block looked at.
        act(
            &mut h,
            DVec3::new(5.0, 0.5, 0.4),
            DVec3::new(1.88, 0.5, 0.3),
            Primary,
        );
        assert_eq!(h.state().world.get(Pos::new(1, 0, 0)), None);
        assert_eq!(h.state().world.len(), 3);
    }

    /// Placed blocks face the way the player looks; middle click takes the
    /// block looked at; number keys choose from the palette.
    #[test]
    fn facing_pick_and_hotbar() {
        let mut v = WorldView::new(World::new("s"));
        v.brush = Some(Brush {
            block: Block::Gate {
                gate: makine_world::block::Gate::And,
                facing: Dir::East,
            },
        });
        let mut h = harness(v);
        // Looking north (towards -y) at the floor in front.
        act(
            &mut h,
            DVec3::new(0.5, 3.5, 2.5),
            DVec3::new(0.5, 0.5, 0.0),
            PointerButton::Secondary,
        );
        assert!(matches!(
            h.state().world.get(Pos::new(0, 0, 0)),
            Some(Block::Gate {
                facing: Dir::North,
                ..
            })
        ));
        // Key 1 takes a wire, middle click the gate back.
        h.key_press(egui::Key::Num1);
        h.run();
        assert_eq!(h.state().brush, Some(Brush { block: Block::Wire }));
        act(
            &mut h,
            DVec3::new(0.5, 3.5, 2.5),
            DVec3::new(0.5, 0.5, 0.3),
            PointerButton::Middle,
        );
        assert!(matches!(
            h.state().brush,
            Some(Brush {
                block: Block::Gate { .. }
            })
        ));
    }

    /// Keys fly; Esc gives the mouse back.
    #[test]
    fn keys_fly_and_esc_releases() {
        let mut h = harness(WorldView::new(World::new("f")));
        h.state_mut()
            .camera
            .look_from(DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0));
        let start = h.state().camera.eye;
        h.event(egui::Event::Key {
            key: egui::Key::W,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(10);
        h.event(egui::Event::Key {
            key: egui::Key::W,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
        let moved = h.state().camera.eye - start;
        assert!(moved.x > 0.1 && moved.y.abs() < 1e-6, "{moved:?}");
        h.key_press(egui::Key::Escape);
        h.run();
        assert!(!h.state().locked);
    }
}

mod history {
    use crate::worldview::*;

    fn wire(v: &mut WorldView, x: i32, b: Block) {
        v.edit(Edit::Set {
            pos: Pos::new(x, 0, 0),
            block: Some(b),
        });
    }

    /// In a shared world, undo takes back only what this player did and
    /// what nobody has changed since.
    #[test]
    fn undo_respects_other_players() {
        let mut v = WorldView::new(World::new("h"));
        v.sharing = true;
        wire(&mut v, 0, Block::Wire);
        wire(&mut v, 1, Block::Wire);
        // Another player puts a lamp on my second wire.
        v.apply_remote(Edit::Set {
            pos: Pos::new(1, 0, 0),
            block: Some(Block::Lamp),
        });
        v.outbox.clear();
        // My last edit was entirely overwritten: undo skips it and takes
        // back the first one.
        v.undo();
        assert_eq!(v.world.get(Pos::new(0, 0, 0)), None);
        assert_eq!(v.world.get(Pos::new(1, 0, 0)), Some(&Block::Lamp));
        // What was undone is shared with the others.
        assert_eq!(
            v.outbox,
            vec![Edit::Batch(vec![Edit::Set {
                pos: Pos::new(0, 0, 0),
                block: None
            }])]
        );
        v.redo();
        assert_eq!(v.world.get(Pos::new(0, 0, 0)), Some(&Block::Wire));
        // Nothing of mine is left to undo except that wire.
        v.undo();
        v.undo();
        assert_eq!(v.world.len(), 1, "the lamp stays");
    }

    /// Edits that change nothing do not fill the history.
    #[test]
    fn no_op_edits_are_not_recorded() {
        let mut v = WorldView::new(World::new("h"));
        wire(&mut v, 0, Block::Wire);
        wire(&mut v, 0, Block::Wire);
        v.undo();
        assert!(v.world.is_empty());
        assert!(!v.can_undo());
    }
}

mod ui_tests {
    use super::play::*;
    use crate::worldview::*;
    use egui::PointerButton::{Primary, Secondary};
    use glam::DVec3;

    /// Lay a wire, break one, and turn a switch on by right clicking it.
    #[test]
    fn build_break_toggle() {
        let mut h = harness(WorldView::default());
        let eye = DVec3::new(1.5, 4.0, 4.0);
        for x in 0..4 {
            act(&mut h, eye, DVec3::new(x as f64 + 0.5, 0.5, 0.0), Secondary);
        }
        assert_eq!(h.state().world.len(), 4);
        act(&mut h, eye, DVec3::new(2.5, 0.5, 0.05), Primary);
        assert_eq!(h.state().world.len(), 3);
        assert!(h.state().world.get(Pos::new(2, 0, 0)).is_none());

        h.state_mut().brush = Some(Brush {
            block: Block::Switch {
                on: false,
                facing: Dir::East,
            },
        });
        let at = DVec3::new(0.5, 2.5, 0.0);
        act(&mut h, eye, at, Secondary);
        let off = Block::Switch {
            on: false,
            facing: Dir::North,
        };
        assert_eq!(h.state().world.get(Pos::new(0, 2, 0)), Some(&off));
        // Right clicking a switch turns it on.
        act(&mut h, eye, DVec3::new(0.5, 2.5, 0.25), Secondary);
        assert_eq!(
            h.state().world.get(Pos::new(0, 2, 0)),
            Some(&Block::Switch {
                on: true,
                facing: Dir::North
            })
        );
        // Undo turns it off again.
        h.state_mut().undo();
        assert_eq!(h.state().world.get(Pos::new(0, 2, 0)), Some(&off));
    }
}

mod device_tests {
    use crate::worldview::*;
    use std::{cell::RefCell, rc::Rc};

    /// A keyboard wired straight into a display: select the keyboard, type
    /// a letter, the display shows its code (plus `ready` in bit 7).
    #[test]
    fn type_into_keyboard_and_read_display() {
        let mut w = World::new("k");
        let e = w
            .place_device(Device::Keyboard, Pos::new(0, 0, 0), Dir::East)
            .unwrap();
        w.apply(e);
        for y in 0..8 {
            w.apply(Edit::Set {
                pos: Pos::new(3, y, 0),
                block: Some(Block::Straight { facing: Dir::East }),
            });
        }
        let e = w
            .place_device(Device::Display { bits: 8 }, Pos::new(4, 0, 0), Dir::East)
            .unwrap();
        w.apply(e);
        let view = Rc::new(RefCell::new(WorldView::new(w)));
        view.borrow_mut().selected = Some(Pos::new(0, 0, 0));
        let v = view.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(move |ui| v.borrow_mut().show(ui));
        h.run_steps(2);
        assert!(view.borrow().typing());
        h.event(egui::Event::Text("A".into()));
        h.run_steps(2);
        let shown = {
            let v = view.borrow();
            let values = v.sim.values();
            let d = v
                .sim
                .compiled()
                .unwrap()
                .devices
                .iter()
                .find(|d| matches!(d.device, Device::Display { .. }))
                .unwrap();
            d.inputs
                .iter()
                .enumerate()
                .map(|(i, o)| o.map_or(0, |o| values[o] << i))
                .sum::<u64>()
        };
        assert_eq!(shown, 0x80 | b'A' as u64);
        // Esc leaves the keyboard.
        h.key_press(egui::Key::Escape);
        h.run();
        assert!(!view.borrow().typing());
    }

    /// Placing a device by clicking, resizing it from the panel, and
    /// keeping RAM contents across unrelated edits.
    #[test]
    fn place_resize_and_keep_memory() {
        let mut v = WorldView::new(World::new("t"));
        v.brush = Some(Brush {
            block: Block::Device {
                device: Device::Ram { addr: 2, data: 4 },
                facing: Dir::East,
            },
        });
        v.place(Pos::new(0, 0, 0), true);
        assert_eq!(v.selected, Some(Pos::new(0, 0, 0)));
        // Clicking a device selects it instead of painting.
        v.brush = Some(Brush { block: Block::Wire });
        v.selected = None;
        v.place(Pos::new(1, 1, 0), true);
        assert_eq!(v.selected, Some(Pos::new(0, 0, 0)));
        let n = v.world.len();
        v.replace_device(Pos::new(0, 0, 0), Device::Ram { addr: 3, data: 4 })
            .unwrap();
        assert!(v.world.len() > n);
        // Write into the RAM directly, edit elsewhere, the word is kept.
        v.sim.run_frame(&v.world, false);
        let name = v.sim.native_at(Pos::new(0, 0, 0)).unwrap();
        {
            let sim = v.sim.running_circuit().unwrap();
            let r: &mut makine_sim::Clocked<device::RamState> = sim.native_mut(&name).unwrap();
            r.state.words[5] = 9;
        }
        v.edit(Edit::Set {
            pos: Pos::new(20, 0, 0),
            block: Some(Block::Lamp),
        });
        v.sim.run_frame(&v.world, false);
        assert_eq!(v.ram_words(Pos::new(0, 0, 0)).unwrap()[5], 9);
        // A device that does not fit is refused with a message.
        v.brush = Some(Brush {
            block: Block::Device {
                device: Device::Keyboard,
                facing: Dir::East,
            },
        });
        v.place(Pos::new(19, 0, 0), true);
        assert!(v.status.is_some());
        // Erasing any cell removes the whole device.
        v.erase(Pos::new(2, 3, 0));
        assert_eq!(v.world.len(), 1);
        assert_eq!(v.selected, None);
    }
}

mod selection_tests {
    use super::play::*;
    use crate::worldview::*;
    use egui::PointerButton::{Primary, Secondary};
    use glam::DVec3;

    /// Select with left and right click, copy, paste somewhere else, move
    /// with Alt+arrows, delete.
    #[test]
    fn select_copy_paste_move() {
        let mut w = World::new("s");
        for x in 0..3 {
            w.apply(Edit::Set {
                pos: Pos::new(x, 0, 0),
                block: Some(Block::Wire),
            });
        }
        w.apply(Edit::Set {
            pos: Pos::new(1, 1, 1),
            block: Some(Block::Lamp),
        });
        let mut v = WorldView::new(w);
        v.brush = None;
        let mut h = harness(v);
        let eye = DVec3::new(1.5, 6.0, 6.0);
        let floor = |x: i32, y: i32| DVec3::new(x as f64 + 0.5, y as f64 + 0.5, 0.0);

        // Corners (0,0) and (2,1).
        act(&mut h, eye, floor(0, 0), Primary);
        act(&mut h, eye, floor(2, 1), Secondary);
        let sel = h.state().selection.unwrap();
        assert_eq!(sel, Region::between(Pos::new(0, 0, 0), Pos::new(2, 1, 0)));
        assert_eq!(sel.blocks(&h.state().world).len(), 4, "all layers");

        // Copy, then paste at (0, 5) by right clicking there.
        h.event(egui::Event::Copy);
        h.run();
        assert!(h.state().clipboard.is_some());
        let text = h.state().clipboard.as_ref().unwrap().to_json();
        h.event(egui::Event::Paste(text));
        h.run();
        assert!(h.state().pasting.is_some());
        act(&mut h, eye, floor(0, 5), Secondary);
        assert_eq!(h.state().world.len(), 8);
        assert_eq!(h.state().world.get(Pos::new(1, 6, 1)), Some(&Block::Lamp));

        // The pasted copy is selected: move it right with Alt+arrow.
        h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowRight);
        h.run();
        assert_eq!(h.state().world.get(Pos::new(2, 6, 1)), Some(&Block::Lamp));
        assert!(h.state().world.get(Pos::new(0, 5, 0)).is_none());

        // Delete removes it; undo brings it back in one step.
        h.key_press(egui::Key::Delete);
        h.run();
        assert_eq!(h.state().world.len(), 4);
        h.state_mut().undo();
        assert_eq!(h.state().world.len(), 8);
    }
}

mod settings_tests {
    use super::*;
    use crate::worldview::*;
    use egui_kittest::kittest::Queryable;

    /// The side panels of the world view, as the app shows them.
    fn panels(ui: &mut Ui, v: &mut WorldView) {
        v.block_panel(ui);
        v.device_panel(ui);
    }

    fn harness(v: WorldView) -> egui_kittest::Harness<'static, WorldView> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(500.0, 600.0))
            .build_ui_state(panels, v)
    }

    /// Ports come from the palette and are renamed in their panel; then the
    /// world becomes a chip with that pin. A clock's period is set the same
    /// way.
    #[test]
    fn ports_and_clocks_are_editable() {
        let mut v = WorldView::new(World::new("p"));
        let brush = |name: &str| {
            palette()
                .into_iter()
                .find(|(n, ..)| *n == name)
                .map(|(_, b, _)| Brush { block: b })
        };
        v.brush = brush("Giriş portu");
        v.place(Pos::new(0, 0, 0), false);
        v.brush = Some(Brush {
            block: Block::Gate {
                gate: Gate::Not,
                facing: Dir::East,
            },
        });
        v.place(Pos::new(1, 0, 0), false);
        v.brush = brush("Çıkış portu");
        v.place(Pos::new(2, 0, 0), false);
        v.brush = brush("Saat");
        v.place(Pos::new(0, 3, 0), false);
        // Clicking a port selects it for its settings.
        v.place(Pos::new(2, 0, 0), true);
        assert_eq!(v.selected_block, Some(Pos::new(2, 0, 0)));

        let mut h = harness(v);
        h.run();
        h.get_by_label("Ad").focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_label("Ad").type_text("ters");
        h.run_steps(2);
        let ports = Region::between(Pos::new(0, 0, 0), Pos::new(2, 0, 0));
        let mut part = World::new("Ters");
        part.apply(
            Clip::copy(&h.state().world, ports)
                .unwrap()
                .paste(&part, Pos::new(0, 0, 0))
                .unwrap(),
        );
        let chip = makine_world::chip::to_chip(&part, "Ters", &Library::new()).unwrap();
        assert_eq!(chip.outputs[0].name, "ters");
        // An invalid name is not applied.
        h.get_by_label("Ad").type_text(" x");
        h.run_steps(2);
        assert!(matches!(
            h.state().world.get(Pos::new(2, 0, 0)),
            Some(Block::Port { name, .. }) if name == "ters"
        ));

        h.state_mut().place(Pos::new(0, 3, 0), true);
        h.run();
        h.get_by_label("Periyot (adım)").focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_label("Periyot (adım)").type_text("7");
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        assert!(matches!(
            h.state().world.get(Pos::new(0, 3, 0)),
            Some(Block::Clock { period: 7, .. })
        ));
    }

    /// R turns what is being pasted, not the brush.
    #[test]
    fn r_turns_the_paste() {
        let mut w = World::new("r");
        for x in 0..3 {
            w.apply(Edit::Set {
                pos: Pos::new(x, 0, 0),
                block: Some(Block::Wire),
            });
        }
        let mut v = WorldView::new(w);
        v.pasting = Clip::copy(
            &v.world,
            Region::between(Pos::new(0, 0, 0), Pos::new(2, 0, 0)),
        );
        let f = v.facing;
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 400.0))
            .build_ui_state(|ui, v: &mut WorldView| v.show(ui), v);
        h.run();
        h.hover_at(egui::pos2(300.0, 200.0));
        h.run();
        h.key_press(egui::Key::R);
        h.run();
        assert_eq!(h.state().pasting.as_ref().unwrap().size(), (1, 3));
        assert_eq!(h.state().facing, f, "facing unchanged while pasting");
    }

    /// The ROM text box follows the ROM when someone else changes it.
    #[test]
    fn rom_text_follows_contents() {
        let mut v = WorldView::new(World::new("r"));
        let rom = Device::Rom {
            addr: 1,
            data: 8,
            words: vec![0xAB, 0xCD],
        };
        let e = v
            .world
            .place_device(rom, Pos::new(0, 0, 0), Dir::East)
            .unwrap();
        v.edit(e);
        v.selected = Some(Pos::new(0, 0, 0));
        let mut h = harness(v);
        h.run();
        assert_eq!(h.state().rom_text.as_ref().unwrap().1, "AB CD");
        {
            let v = h.state_mut();
            let e = v.world.remove(Pos::new(0, 0, 0)).unwrap();
            v.apply_remote(e);
            let new = Device::Rom {
                addr: 1,
                data: 8,
                words: vec![0x12, 0x34],
            };
            let e = v
                .world
                .place_device(new, Pos::new(0, 0, 0), Dir::East)
                .unwrap();
            v.apply_remote(e);
            v.selected = Some(Pos::new(0, 0, 0));
        }
        h.run();
        assert_eq!(h.state().rom_text.as_ref().unwrap().1, "12 34");
    }
}
