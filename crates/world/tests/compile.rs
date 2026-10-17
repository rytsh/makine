//! Worlds built in the grid must behave exactly like the circuits they
//! draw: gates match their truth tables, crossings keep signals apart, vias
//! join layers, feedback makes memory, and mistakes are reported.

use makine_sim::{Circuit, Library};
use makine_world::block::{Block, Dir, Gate};
use makine_world::compile::{compile, CompileError, InputSource, Lane};
use makine_world::draw::world;
use makine_world::{Edit, Pos, World};

fn set(w: &mut World, x: i32, y: i32, z: i32, b: Block) {
    w.apply(Edit::Set {
        pos: Pos::new(x, y, z),
        block: Some(b),
    });
}

/// A horizontal or vertical run of wire, inclusive.
fn run(w: &mut World, from: (i32, i32), to: (i32, i32), z: i32) {
    let (mut x, mut y) = from;
    loop {
        set(w, x, y, z, Block::Wire);
        if (x, y) == to {
            break;
        }
        x += (to.0 - x).signum();
        y += (to.1 - y).signum();
    }
}

fn input(name: &str, facing: Dir) -> Block {
    Block::Port {
        name: name.into(),
        output: false,
        facing,
    }
}

fn output(name: &str, facing: Dir) -> Block {
    Block::Port {
        name: name.into(),
        output: true,
        facing,
    }
}

fn gate(gate: Gate, facing: Dir) -> Block {
    Block::Gate { gate, facing }
}

/// A compiled world, ready to run.
struct Sim {
    c: makine_world::compile::Compiled,
    sim: Circuit,
}

impl Sim {
    fn new(w: &World) -> Sim {
        let c = compile(w).unwrap();
        let sim = Circuit::compile_def(&Library::new(), &c.def).unwrap();
        Sim { c, sim }
    }

    /// Set named input ports (others 0), evaluate, return named outputs.
    fn eval(&mut self, inputs: &[(&str, bool)]) -> impl Fn(&str) -> bool + '_ {
        for (i, src) in self.c.inputs.iter().enumerate() {
            let v = match src {
                InputSource::Port(n) => inputs.iter().any(|(m, v)| m == n && *v),
                _ => false,
            };
            self.sim.set_input(i, v as u64);
        }
        self.sim.eval().unwrap();
        move |name| {
            let o = self.c.ports.iter().find(|(n, _)| n == name).unwrap().1;
            self.sim.output(o) == 1
        }
    }
}

#[test]
fn every_gate_matches_its_truth_table() {
    for g in Gate::ALL {
        let mut w = World::new("g");
        if g.inputs() == 1 {
            // a -> NOT -> q
            set(&mut w, 0, 0, 0, input("a", Dir::East));
            set(&mut w, 1, 0, 0, gate(g, Dir::East));
            set(&mut w, 2, 0, 0, output("q", Dir::East));
        } else {
            // Gate at (2,1) facing east reads north (2,0) and south (2,2).
            set(&mut w, 0, 0, 0, input("a", Dir::East));
            run(&mut w, (1, 0), (2, 0), 0);
            set(&mut w, 0, 2, 0, input("b", Dir::East));
            run(&mut w, (1, 2), (2, 2), 0);
            set(&mut w, 2, 1, 0, gate(g, Dir::East));
            set(&mut w, 3, 1, 0, output("q", Dir::East));
        }
        let mut s = Sim::new(&w);
        for a in [false, true] {
            for b in [false, true] {
                let q = s.eval(&[("a", a), ("b", b)])("q");
                let b = b && g.inputs() == 2;
                assert_eq!(q, g.eval(a, b), "{} a={a} b={b}", g.name());
            }
        }
    }
}

#[test]
fn ascii_worlds() {
    // The text format used by examples: a -> NOT -> NOT -> q0, gates back
    // to back.
    let w = world("nn", &["a>>0"]);
    let mut s = Sim::new(&w);
    assert!(s.eval(&[("a", true)])("q0"));
    assert!(!s.eval(&[("a", false)])("q0"));
}

#[test]
fn crossings_keep_signals_apart() {
    //     b
    //     |
    // a - # - q0
    //     |
    //     q1 (reading north)
    let mut w = World::new("x");
    set(&mut w, 0, 2, 0, input("a", Dir::East));
    set(&mut w, 1, 2, 0, Block::Wire);
    set(&mut w, 2, 2, 0, Block::Cross);
    set(&mut w, 3, 2, 0, Block::Wire);
    set(&mut w, 4, 2, 0, output("q0", Dir::East));
    set(&mut w, 2, 0, 0, input("b", Dir::South));
    set(&mut w, 2, 1, 0, Block::Wire);
    set(&mut w, 2, 3, 0, Block::Wire);
    set(&mut w, 2, 4, 0, output("q1", Dir::South));
    let mut s = Sim::new(&w);
    for (a, b) in [(false, true), (true, false), (true, true), (false, false)] {
        let q = s.eval(&[("a", a), ("b", b)]);
        assert_eq!((q("q0"), q("q1")), (a, b));
    }
}

#[test]
fn vias_connect_layers() {
    // a on layer 0, up a via, east on layer 1 over a layer-0 wire that must
    // not touch it, down a via, to q.
    let mut w = World::new("v");
    set(&mut w, 0, 0, 0, input("a", Dir::East));
    set(&mut w, 1, 0, 0, Block::Via);
    set(&mut w, 1, 0, 1, Block::Via);
    run(&mut w, (2, 0), (4, 0), 1);
    set(&mut w, 5, 0, 1, Block::Via);
    set(&mut w, 5, 0, 0, Block::Via);
    set(&mut w, 6, 0, 0, output("q", Dir::East));
    // A separate signal on layer 0 under the bridge.
    set(&mut w, 3, 1, 0, input("b", Dir::North));
    set(&mut w, 3, 0, 0, Block::Wire);
    let mut s = Sim::new(&w);
    for a in [false, true] {
        assert_eq!(s.eval(&[("a", a), ("b", !a)])("q"), a);
    }
    // Without the vias nothing reaches q.
    w.apply(Edit::Set {
        pos: Pos::new(5, 0, 1),
        block: Some(Block::Wire),
    });
    let mut s = Sim::new(&w);
    assert!(!s.eval(&[("a", true)])("q"));
}

/// Place blocks from a grid of characters on layer `z`:
/// `-` wire, `#` crossing, `o` via, `X`/`A`/`N` XOR/AND/NAND facing east,
/// `a`..`z` input ports facing east, `Q` output ports (named in `outs`, in
/// reading order) reading from the west.
fn draw(w: &mut World, z: i32, rows: &[&str], outs: &[&str]) {
    let mut outs = outs.iter();
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            let b = match c {
                '-' => Block::Wire,
                '#' => Block::Cross,
                'o' => Block::Via,
                'X' => gate(Gate::Xor, Dir::East),
                'A' => gate(Gate::And, Dir::East),
                'N' => gate(Gate::Nand, Dir::East),
                'Q' => output(outs.next().expect("name for Q"), Dir::East),
                'a'..='z' => input(&c.to_string(), Dir::East),
                _ => continue,
            };
            set(w, x as i32, y as i32, z, b);
        }
    }
}

/// Net numbers of layer `z`, as text, for test failure messages.
fn net_map(w: &World, z: i32) -> String {
    let c = compile(w).unwrap();
    let (lo, hi) = w.bounds().unwrap();
    let mut out = String::new();
    for y in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            out += &match c.net_at(Pos::new(x, y, z), Lane::Main) {
                Some(n) => format!("{n:>3}"),
                None => "  .".into(),
            };
        }
        out.push('\n');
    }
    out
}

#[test]
fn half_adder() {
    // XOR at (5,1) reads a (north, row 0) and b (south, row 2).
    // AND at (5,5) reads a (north, row 4) and b (south, row 6).
    // b goes down column 1; a goes down column 3, crossing b's row at (3,2).
    let mut w = World::new("ha");
    draw(
        &mut w,
        0,
        &[
            "a-----",  //  y0  a: (1..5, 0)
            "   - XQ", // y1  XOR at (5,1)
            "b--#--",  //  y2  b: (1..5, 2), crossing at (3,2)
            " - -",    //    y3
            " - ---",  //  y4  a: (3..5, 4)
            " -   AQ", // y5  AND at (5,5)
            " -----",  //  y6  b: (1..5, 6)
        ],
        &["sum", "carry"],
    );
    let mut s = Sim::new(&w);
    for a in [false, true] {
        for b in [false, true] {
            let q = s.eval(&[("a", a), ("b", b)]);
            assert_eq!(
                (q("sum"), q("carry")),
                (a ^ b, a & b),
                "a={a} b={b}\nnets:\n{}",
                net_map(&w, 0)
            );
        }
    }
}

#[test]
fn sr_latch_remembers() {
    // Two cross-coupled NANDs with active-low inputs (0 = act). Layer 0:
    //
    //   x:   0   1   2   3   4   5   6   7   8
    //   y0   sn  -   -   -                         sn: N1's north input
    //   y1               N1  -   -   -   q         q:  N1's output
    //   y2               o           -             q̄ comes down a via into
    //   y3                           -             N1's south input
    //   y4               -   -   -   -             q:  N2's north input
    //   y5               N2  #   #   #   o         q̄:  N2's output, east
    //   y6   rn  -   -   -                         rn: N2's south input
    //
    // Layer 1 carries q̄ from the via at (7,5) up column 7 and west along
    // row 2 to the via at (3,2). The crossings let q̄ pass east under q's
    // column without touching it.
    let mut w = World::new("sr");
    set(&mut w, 0, 0, 0, input("sn", Dir::East));
    run(&mut w, (1, 0), (3, 0), 0);
    set(&mut w, 0, 6, 0, input("rn", Dir::East));
    run(&mut w, (1, 6), (3, 6), 0);
    set(&mut w, 3, 1, 0, gate(Gate::Nand, Dir::East));
    set(&mut w, 3, 5, 0, gate(Gate::Nand, Dir::East));
    // q
    run(&mut w, (4, 1), (6, 1), 0);
    set(&mut w, 7, 1, 0, output("q", Dir::East));
    run(&mut w, (6, 2), (6, 4), 0);
    run(&mut w, (5, 4), (3, 4), 0);
    // q̄
    set(&mut w, 4, 5, 0, Block::Cross);
    set(&mut w, 5, 5, 0, Block::Cross);
    set(&mut w, 6, 5, 0, Block::Cross);
    set(&mut w, 7, 5, 0, Block::Via);
    set(&mut w, 7, 5, 1, Block::Via);
    run(&mut w, (7, 4), (7, 2), 1);
    run(&mut w, (6, 2), (4, 2), 1);
    set(&mut w, 3, 2, 1, Block::Via);
    set(&mut w, 3, 2, 0, Block::Via);

    let mut s = Sim::new(&w);
    let mut q = |sn: bool, rn: bool| s.eval(&[("sn", sn), ("rn", rn)])("q");
    assert!(q(false, true), "set");
    assert!(q(true, true), "hold 1");
    assert!(!q(true, false), "reset");
    assert!(!q(true, true), "hold 0");
    assert!(q(false, true), "set again");
}

#[test]
fn short_circuit_is_reported() {
    // Two switches drive the same wire from both ends.
    let mut w = World::new("short");
    set(
        &mut w,
        0,
        0,
        0,
        Block::Switch {
            on: true,
            facing: Dir::East,
        },
    );
    run(&mut w, (1, 0), (2, 0), 0);
    set(
        &mut w,
        3,
        0,
        0,
        Block::Switch {
            on: false,
            facing: Dir::West,
        },
    );
    match compile(&w) {
        Err(CompileError::ShortCircuit(p)) => {
            assert_eq!(p.len(), 2);
            assert!(p.contains(&Pos::new(0, 0, 0)) && p.contains(&Pos::new(3, 0, 0)));
        }
        other => panic!("expected a short circuit, got {other:?}"),
    }
}

#[test]
fn duplicate_ports_are_reported() {
    let w = world("dup", &["a-0", "a-1"]);
    assert_eq!(
        compile(&w).unwrap_err(),
        CompileError::DuplicatePort("a".into())
    );
}

#[test]
fn every_wire_and_lamp_is_readable() {
    // switch -> wire -> lamp, and a branch through a NOT.
    let mut w = World::new("n");
    set(
        &mut w,
        0,
        0,
        0,
        Block::Switch {
            on: true,
            facing: Dir::East,
        },
    );
    run(&mut w, (1, 0), (3, 0), 0);
    set(&mut w, 4, 0, 0, Block::Lamp);
    set(&mut w, 2, 1, 0, gate(Gate::Not, Dir::South));
    set(&mut w, 2, 2, 0, Block::Wire);
    set(&mut w, 2, 3, 0, Block::Lamp);
    let c = compile(&w).unwrap();
    let mut sim = Circuit::compile_def(&Library::new(), &c.def).unwrap();
    sim.set_input(0, 1);
    sim.eval().unwrap();
    let read = |x, y| sim.output(c.output_at(Pos::new(x, y, 0), Lane::Main).unwrap());
    assert_eq!(read(1, 0), 1);
    assert_eq!(read(3, 0), 1);
    assert_eq!(read(2, 1), 0, "a gate shows its output");
    assert_eq!(read(2, 2), 0);
    // Lamps in position order: (2,3) after the NOT is dark, (4,0) is lit.
    let lit: Vec<(Pos, u64)> = c
        .lamps
        .iter()
        .map(|&(p, o)| (p, sim.output(o.unwrap())))
        .collect();
    assert!(lit.contains(&(Pos::new(4, 0, 0), 1)));
    assert!(lit.contains(&(Pos::new(2, 3, 0), 0)));
}

#[test]
fn unconnected_inputs_read_zero() {
    // A NOT with nothing at its input outputs 1.
    let mut w = World::new("z");
    set(&mut w, 0, 0, 0, gate(Gate::Not, Dir::East));
    set(&mut w, 1, 0, 0, output("q", Dir::East));
    let mut s = Sim::new(&w);
    assert!(s.eval(&[])("q"));
}
