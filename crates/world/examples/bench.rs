//! How long compiling and stepping a large world takes.
//!
//! cargo run --release -p makine-world --example bench

use std::time::Instant;

use makine_sim::Circuit;
use makine_world::block::{Block, Dir, Gate};
use makine_world::{compile, Edit, Pos, World};

fn main() {
    // A world full of RAM blocks, with wires between: device pins dominate.
    {
        let mut w = World::new("ram");
        let d = makine_world::Device::Ram { addr: 8, data: 16 };
        let (len, wide) = d.size();
        for i in 0..40 {
            let e = w
                .place_device(d.clone(), Pos::new(i * (len + 2), 0, 0), Dir::East)
                .unwrap();
            w.apply(e);
            for y in 0..wide {
                w.apply(Edit::Set {
                    pos: Pos::new(i * (len + 2) + len, y, 0),
                    block: Some(Block::Straight { facing: Dir::East }),
                });
            }
        }
        let t = Instant::now();
        compile(&w).unwrap();
        println!(
            "{:>6} blocks of RAM: world compile {:>8.2?}",
            w.len(),
            t.elapsed()
        );
    }
    for side in [50, 100, 200] {
        // Rows of NOT gates separated by wires: side*side cells.
        let mut w = World::new("b");
        let mut edits = Vec::new();
        for y in 0..side {
            for x in 0..side {
                let b = if x % 2 == 1 {
                    Block::Gate {
                        gate: Gate::Not,
                        facing: Dir::East,
                    }
                } else if y % 2 == 0 {
                    Block::Wire
                } else {
                    Block::Straight { facing: Dir::East }
                };
                edits.push(Edit::Set {
                    pos: Pos::new(x, y * 2, 0),
                    block: Some(b),
                });
            }
            edits.push(Edit::Set {
                pos: Pos::new(-1, y * 2, 0),
                block: Some(Block::Switch {
                    on: true,
                    facing: Dir::East,
                }),
            });
        }
        w.apply(Edit::Batch(edits));
        let t = Instant::now();
        let c = compile(&w).unwrap();
        let tc = t.elapsed();
        let t = Instant::now();
        let mut sim = Circuit::compile_def(&c.library(), &c.def).unwrap();
        let ts = t.elapsed();
        let t = Instant::now();
        for i in 0..100 {
            sim.set_input(0, i & 1);
            sim.eval().unwrap();
        }
        let te = t.elapsed() / 100;
        println!(
            "{:>6} blocks: world compile {:>8.2?}, circuit {:>8.2?}, step {:>8.2?}",
            w.len(),
            tc,
            ts,
            te
        );
    }
}
