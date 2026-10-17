//! Worlds come from files and other players, so nothing a world can hold
//! may crash the compiler, the simulator or copy/paste. These tests throw
//! random blocks, half-overwritten devices and stray covers at them.

use makine_sim::{Circuit, Library, Pin, Rng};
use makine_world::block::{Block, Dir, Gate};
use makine_world::compile::compile;
use makine_world::device::Device;
use makine_world::region::{Clip, Region};
use makine_world::{Edit, Pos, World};

fn pick<T: Clone>(r: &mut Rng, items: &[T]) -> T {
    items[(r.next_u64() % items.len() as u64) as usize].clone()
}

fn small(r: &mut Rng, n: i32) -> i32 {
    (r.next_u64() % (2 * n as u64 + 1)) as i32 - n
}

fn random_block(r: &mut Rng) -> Block {
    let facing = pick(r, &Dir::ALL);
    match r.next_u64() % 13 {
        0 => Block::Wire,
        1 => Block::Straight { facing },
        2 => Block::Cross,
        3 => Block::Via,
        4 => Block::Gate {
            gate: pick(r, &Gate::ALL),
            facing,
        },
        5 => Block::Switch {
            on: r.bit() == 1,
            facing,
        },
        6 => Block::Lamp,
        7 => Block::Clock {
            period: r.next_u64() as u32 % 4,
            facing,
        },
        8 => Block::Port {
            name: pick(r, &["a", "b", "q"]).into(),
            output: r.bit() == 1,
            facing,
        },
        // Devices of any size, including absurd ones.
        9 => {
            let (x, y) = (r.next_u64() as u8, r.next_u64() as u8);
            let devices = [
                Device::Ram { addr: x, data: y },
                Device::Rom {
                    addr: x % 4,
                    data: 0,
                    words: vec![u64::MAX; 3],
                },
                Device::Screen { bits: x },
                Device::Keyboard,
                Device::Display { bits: 0 },
            ];
            Block::Device {
                device: pick(r, &devices),
                facing,
            }
        }
        10 => Block::Device {
            device: Device::Chip {
                name: "Yok".into(),
                inputs: vec![Pin::new("a", r.next_u64() as u8 % 70)],
                outputs: vec![],
            },
            facing,
        },
        _ => Block::Cover {
            dx: small(r, 4),
            dy: small(r, 4),
        },
    }
}

/// A world of random blocks in a small area (so they touch), with some
/// devices placed properly and then partly overwritten.
fn random_world(r: &mut Rng) -> World {
    let mut w = World::new("rastgele");
    for _ in 0..3 {
        let d = pick(
            r,
            &[
                Device::Ram { addr: 2, data: 3 },
                Device::Screen { bits: 2 },
                Device::Keyboard,
                Device::Rom {
                    addr: 2,
                    data: 2,
                    words: vec![1, 2, 3],
                },
            ],
        );
        let at = Pos::new(small(r, 8), small(r, 8), 0);
        if let Some(e) = w.place_device(d, at, pick(r, &Dir::ALL)) {
            w.apply(e);
        }
    }
    for _ in 0..60 {
        let pos = Pos::new(small(r, 8), small(r, 8), (r.next_u64() % 3) as i32);
        let block = if r.next_u64() % 5 == 0 {
            None
        } else {
            Some(random_block(r))
        };
        w.apply(Edit::Set { pos, block });
    }
    w
}

#[test]
fn random_worlds_never_crash() {
    let mut r = Rng(7);
    for round in 0..400 {
        let w = random_world(&mut r);
        // Through JSON too: what a file or another player would send.
        let w = World::from_json(&w.to_json()).unwrap();
        if let Ok(c) = compile(&w) {
            if let Ok(mut sim) = Circuit::compile_def(&c.library_with(&Library::new()), &c.def) {
                for i in 0..c.def.inputs.len() {
                    sim.set_input(i, r.bit());
                }
                let _ = sim.eval();
            }
        }
        // Copy, rotate and paste every part of it.
        let all = Region::between(Pos::new(-12, -12, 0), Pos::new(12, 12, 0));
        if let Some(clip) = Clip::copy(&w, all) {
            let clip = Clip::from_json(&clip.to_json()).unwrap().rotated();
            let mut other = World::new("kopya");
            if let Ok(e) = clip.paste(&other, Pos::new(3, -4, 1)) {
                other.apply(e);
                let _ = compile(&other);
            }
        }
        // Remove every cell one by one; the world must end up empty.
        let mut w = w;
        let cells: Vec<Pos> = w.blocks().map(|(p, _)| p).collect();
        for p in cells {
            if let Some(e) = w.remove(p) {
                w.apply(e);
            }
        }
        assert!(w.is_empty(), "round {round}: {} left", w.len());
    }
}
