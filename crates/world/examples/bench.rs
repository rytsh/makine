//! How long compiling and stepping a large world takes.
//!
//! cargo run --release -p makine-world --example bench

use std::time::Instant;

use makine_sim::{Circuit, Library};
use makine_world::{compile, Device, Edit, End, Gate, Kind, Placed, Pos, World};

fn put(w: &mut World, kind: Kind) -> u64 {
    let id = w.new_id();
    w.apply(Edit::Part {
        id,
        part: Some(Placed {
            kind,
            pos: Pos::new(0.0, 0.0),
        }),
    });
    id
}

fn main() {
    let lib = Library::new();
    for n in [1_000, 10_000, 50_000] {
        // A chain of NOT gates fed by a switch, with a few RAMs on the side.
        let mut w = World::new("b");
        let mut prev = put(&mut w, Kind::Switch { on: true });
        for _ in 0..n {
            let g = put(&mut w, Kind::Gate(Gate::Not));
            w.apply(Edit::Wire {
                to: End::new(g, 0),
                from: Some(End::new(prev, 0)),
            });
            prev = g;
        }
        for _ in 0..20 {
            put(&mut w, Kind::Device(Device::Ram { addr: 8, data: 16 }));
        }
        let t = Instant::now();
        let c = compile(&w, &lib);
        let compile_time = t.elapsed();
        let t = Instant::now();
        let mut sim = Circuit::compile_def(&c.library_with(&lib), &c.def).unwrap();
        let build_time = t.elapsed();
        let t = Instant::now();
        for i in 0..100 {
            sim.set_input(0, i % 2);
            sim.eval().unwrap();
        }
        println!(
            "{n:>6} gates: world compile {compile_time:>8.2?}, circuit {build_time:>8.2?}, \
             100 steps {:>8.2?}",
            t.elapsed()
        );
    }
}
