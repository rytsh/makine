//! A world becomes a chip; the chip goes back into a world as a block.

use makine_sim::{Circuit, Library};

use makine_world::block::{Block, Dir, Gate};
use makine_world::chip::to_chip;
use makine_world::compile::{compile, InputSource};
use makine_world::device::Device;
use makine_world::region::{Clip, Region};
use makine_world::{Edit, Pos, World};

/// Outputs for every combination of 1-bit inputs.
fn table(sim: &mut Circuit, n: usize) -> Vec<(Vec<u64>, Vec<u64>)> {
    (0..1u64 << n)
        .map(|v| {
            let ins: Vec<u64> = (0..n).map(|i| v >> i & 1).collect();
            let outs = sim.eval_values(&ins).unwrap();
            (ins, outs)
        })
        .collect()
}

fn set(w: &mut World, x: i32, y: i32, b: Block) {
    w.apply(Edit::Set {
        pos: Pos::new(x, y, 0),
        block: Some(b),
    });
}

fn run(w: &mut World, y: i32, x0: i32, x1: i32) {
    for x in x0..=x1 {
        set(w, x, y, Block::Wire);
    }
}

fn port(name: &str, output: bool) -> Block {
    Block::Port {
        name: name.into(),
        output,
        facing: Dir::East,
    }
}

/// Half adder from blocks with ports a, b -> s, c.
fn half_adder() -> World {
    let mut w = World::new("ha");
    set(&mut w, 0, 0, port("a", false));
    run(&mut w, 0, 1, 5);
    set(&mut w, 0, 2, port("b", false));
    run(&mut w, 2, 1, 5);
    set(
        &mut w,
        5,
        1,
        Block::Gate {
            gate: Gate::Xor,
            facing: Dir::East,
        },
    );
    set(&mut w, 6, 1, port("s", true));
    set(&mut w, 3, 1, Block::Wire);
    set(&mut w, 3, 2, Block::Cross);
    set(&mut w, 3, 3, Block::Wire);
    run(&mut w, 4, 3, 5);
    for y in 3..=6 {
        set(&mut w, 1, y, Block::Wire);
    }
    run(&mut w, 6, 2, 5);
    set(
        &mut w,
        5,
        5,
        Block::Gate {
            gate: Gate::And,
            facing: Dir::East,
        },
    );
    set(&mut w, 6, 5, port("c", true));
    w
}

#[test]
fn world_region_becomes_a_chip() {
    let w = half_adder();
    let def = to_chip(&w, "Yarim", &Library::new()).unwrap();
    let pins: Vec<String> = def
        .inputs
        .iter()
        .chain(&def.outputs)
        .map(|p| p.to_string())
        .collect();
    assert_eq!(pins, ["a", "b", "c", "s"]);
    let mut lib = Library::new();
    lib.insert(def);
    let mut sim = Circuit::compile(&lib, "Yarim").unwrap();
    let t = table(&mut sim, 2);
    for (ins, outs) in t {
        let (a, b) = (ins[0], ins[1]);
        assert_eq!(outs, vec![a & b, a ^ b]);
    }
}

#[test]
fn buses_from_numbered_ports() {
    // x0..x3 -> NOT each -> y0..y3: a 4-bit inverter.
    let mut w = World::new("inv");
    for i in 0..4 {
        set(&mut w, 0, i, port(&format!("x{i}"), false));
        set(
            &mut w,
            1,
            i,
            Block::Gate {
                gate: Gate::Not,
                facing: Dir::East,
            },
        );
        set(&mut w, 2, i, port(&format!("y{i}"), true));
    }
    let def = to_chip(&w, "Ters4", &Library::new()).unwrap();
    assert_eq!(def.inputs, makine_sim::parse_pins(&["x[4]"]));
    assert_eq!(def.outputs, makine_sim::parse_pins(&["y[4]"]));
    let mut lib = Library::new();
    lib.insert(def);
    let mut sim = Circuit::compile(&lib, "Ters4").unwrap();
    for v in [0u64, 5, 0xA, 0xF] {
        assert_eq!(sim.eval_values(&[v]).unwrap(), vec![!v & 0xF]);
    }
}

#[test]
fn chips_go_back_into_worlds() {
    // Two half adders placed as blocks plus an OR make a full adder.
    let ha = to_chip(&half_adder(), "Yarim", &Library::new()).unwrap();
    let mut lib = Library::new();
    lib.insert(ha.clone());
    let block = Device::Chip {
        name: "Yarim".into(),
        inputs: ha.inputs.clone(),
        outputs: ha.outputs.clone(),
    };
    // The half adder block is 3 long and 2 wide: inputs a, b on its back
    // edge, outputs c, s on its front edge.
    //
    //   x:   0    1   2  3  4   5   6  7  8   9    10   11
    //   y0   x    ─   [h1 a  c]  ─   ─  ─  ─   ─    ─    ┐     h1.c along row 0
    //   y1   y    ─   [h1 b  s]  ─   [h2 a  c]  ─    OR   ┘     OR faces south
    //   y2   cin  ─   ─  ─  ─   ─   [h2 b  s]  sum  │
    //   y3                                          cout
    //
    // Straight wires only join east-west, so the parallel runs never touch.
    assert_eq!(block.size(), (3, 2));
    let mut w = World::new("fa");
    let st = || Block::Straight { facing: Dir::East };
    w.apply(
        w.place_device(block.clone(), Pos::new(2, 0, 0), Dir::East)
            .unwrap(),
    );
    w.apply(
        w.place_device(block.clone(), Pos::new(6, 1, 0), Dir::East)
            .unwrap(),
    );
    set(&mut w, 0, 0, port("x", false));
    set(&mut w, 1, 0, st());
    set(&mut w, 0, 1, port("y", false));
    set(&mut w, 1, 1, st());
    set(&mut w, 0, 2, port("cin", false));
    for x in 1..=5 {
        set(&mut w, x, 2, st());
    }
    set(&mut w, 5, 1, st()); // h1.s -> h2.a
    for x in 5..=10 {
        set(&mut w, x, 0, st()); // h1.c
    }
    set(&mut w, 11, 0, Block::Wire);
    set(&mut w, 11, 1, Block::Wire);
    set(&mut w, 9, 1, st()); // h2.c
    set(
        &mut w,
        10,
        1,
        Block::Gate {
            gate: Gate::Or,
            facing: Dir::South,
        },
    );
    set(&mut w, 9, 2, port("sum", true));
    set(&mut w, 10, 2, Block::Wire);
    set(
        &mut w,
        10,
        3,
        Block::Port {
            name: "cout".into(),
            output: true,
            facing: Dir::South,
        },
    );

    let c = compile(&w).unwrap();
    let mut sim = Circuit::compile_def(&c.library_with(&lib), &c.def).unwrap();
    for x in 0..2u64 {
        for y in 0..2u64 {
            for cin in 0..2u64 {
                for (i, src) in c.inputs.iter().enumerate() {
                    let InputSource::Port(n) = src else { continue };
                    let v = match n.as_str() {
                        "x" => x,
                        "y" => y,
                        _ => cin,
                    };
                    sim.set_input(i, v);
                }
                sim.eval().unwrap();
                let get =
                    |name: &str| sim.output(c.ports.iter().find(|(n, _)| n == name).unwrap().1);
                let total = x + y + cin;
                assert_eq!(
                    (get("sum"), get("cout")),
                    (total & 1, total >> 1),
                    "{x}+{y}+{cin}"
                );
            }
        }
    }

    // The full adder world is itself a chip that uses Yarim.
    let fa = to_chip(&w, "Tam", &lib).unwrap();
    let halves = fa
        .parts
        .iter()
        .filter(|p| p.kind == makine_sim::PartKind::Chip("Yarim".into()))
        .count();
    assert_eq!(halves, 2);
    // A copy of the whole world pasted elsewhere still works.
    let all = Region::between(Pos::new(-5, -5, 0), Pos::new(40, 40, 0));
    let clip = Clip::copy(&w, all).unwrap();
    let mut w2 = World::new("kopya");
    w2.apply(clip.rotated().paste(&w2, Pos::new(50, 50, 0)).unwrap());
    let fa2 = to_chip(&w2, "Tam", &lib).unwrap();
    let mut l = lib.clone();
    l.insert(fa.clone());
    let mut s1 = Circuit::compile(&l, "Tam").unwrap();
    l.insert(fa2);
    let mut s2 = Circuit::compile(&l, "Tam").unwrap();
    assert_eq!(table(&mut s1, 3), table(&mut s2, 3));
}

#[test]
fn chips_with_changed_pins_are_reported() {
    let ha = to_chip(&half_adder(), "Yarim", &Library::new()).unwrap();
    let block = Device::Chip {
        name: "Yarim".into(),
        inputs: ha.inputs.clone(),
        outputs: vec![],
    };
    let mut w = World::new("t");
    w.apply(w.place_device(block, Pos::new(0, 0, 0), Dir::East).unwrap());
    let mut lib = Library::new();
    lib.insert(ha);
    let e = makine_world::compile::check_chips(&w, &lib).unwrap_err();
    assert!(e.contains("değişmiş"), "{e}");
    assert!(makine_world::compile::check_chips(&w, &Library::new())
        .unwrap_err()
        .contains("yok"));
}

#[test]
fn what_cannot_be_a_chip() {
    let mut w = half_adder();
    w.apply(Edit::Set {
        pos: Pos::new(20, 0, 0),
        block: Some(Block::Clock {
            period: 5,
            facing: Dir::East,
        }),
    });
    assert!(to_chip(&w, "X", &Library::new())
        .unwrap_err()
        .contains("saat"));
    let w = World::new("boş");
    assert!(to_chip(&w, "X", &Library::new()).is_err());
}

/// A circuit built over three layers, joined by vias, becomes one chip.
///
/// Layer 0: a -- wire -- via        b -- wire -- via
/// Layer 1:              via -- AND -- via
/// Layer 2:                     q (output, reading the AND)
///
/// Concretely: the AND sits on layer 1 and reads a (from layer 0, through a
/// via) and b (straight on layer 1); its output goes up a via to layer 2,
/// where the output port reads it.
#[test]
fn multi_layer_world_becomes_a_chip() {
    let mut w = World::new("katlı");
    let at = |w: &mut World, x, y, z, b| {
        w.apply(Edit::Set {
            pos: Pos::new(x, y, z),
            block: Some(b),
        })
    };
    // a on layer 0, up a via at (2,0) to layer 1, then into the AND's
    // north input at (3,1).
    at(&mut w, 0, 0, 0, port("a", false));
    at(&mut w, 1, 0, 0, Block::Wire);
    at(&mut w, 2, 0, 0, Block::Via);
    at(&mut w, 2, 0, 1, Block::Via);
    at(&mut w, 3, 0, 1, Block::Wire);
    // b on layer 1, into the AND's south input.
    at(&mut w, 0, 2, 1, port("b", false));
    at(&mut w, 1, 2, 1, Block::Wire);
    at(&mut w, 2, 2, 1, Block::Wire);
    at(&mut w, 3, 2, 1, Block::Wire);
    at(
        &mut w,
        3,
        1,
        1,
        Block::Gate {
            gate: Gate::And,
            facing: Dir::East,
        },
    );
    // Its output up to layer 2.
    at(&mut w, 4, 1, 1, Block::Via);
    at(&mut w, 4, 1, 2, Block::Via);
    at(&mut w, 5, 1, 2, port("q", true));

    // The region is a rectangle; it takes every layer.
    let r = Region::between(Pos::new(0, 0, 0), Pos::new(5, 2, 0));
    let mut part = World::new("parça");
    part.apply(
        Clip::copy(&w, r)
            .unwrap()
            .paste(&part, Pos::new(0, 0, 0))
            .unwrap(),
    );
    assert_eq!(part.len(), w.len(), "all layers copied");
    let chip = to_chip(&part, "Katlı", &Library::new()).unwrap();
    let names = |pins: &[makine_sim::Pin]| pins.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&chip.inputs), ["a", "b"]);
    assert_eq!(names(&chip.outputs), ["q"]);
    let mut sim = Circuit::compile_def(&Library::new(), &chip).unwrap();
    let outs: Vec<u64> = table(&mut sim, 2).into_iter().map(|(_, o)| o[0]).collect();
    assert_eq!(outs, [0, 0, 0, 1], "an AND over three layers");
}
