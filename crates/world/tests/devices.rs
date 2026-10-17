//! Device blocks inside worlds: memory, screens and keyboards driven by
//! input ports through ordinary wires.

use makine_sim::Circuit;
use makine_world::block::{Block, Dir};
use makine_world::compile::{compile, Compiled, InputSource};
use makine_world::device::{self, layout, Device};
use makine_world::{Edit, Pos, World};

fn set(w: &mut World, p: Pos, b: Block) {
    w.apply(Edit::Set {
        pos: p,
        block: Some(b),
    });
}

/// Place `device` facing east at `anchor`; put an input port named after
/// each input pin two cells behind it (with a wire between), and an output
/// port after each output pin.
fn rig(device: Device, anchor: Pos) -> World {
    let mut w = World::new("t");
    let e = w.place_device(device.clone(), anchor, Dir::East).unwrap();
    w.apply(e);
    let l = layout(&device, anchor, Dir::East);
    for ((p, _), name) in l.inputs.iter().zip(device.inputs()) {
        set(
            &mut w,
            Pos::new(p.x - 1, p.y, p.z),
            Block::Straight { facing: Dir::East },
        );
        set(
            &mut w,
            Pos::new(p.x - 2, p.y, p.z),
            Block::Port {
                name,
                output: false,
                facing: Dir::East,
            },
        );
    }
    for ((p, _), name) in l.outputs.iter().zip(device.outputs()) {
        set(
            &mut w,
            Pos::new(p.x + 1, p.y, p.z),
            Block::Port {
                name,
                output: true,
                facing: Dir::East,
            },
        );
    }
    w
}

struct Rig {
    c: Compiled,
    sim: Circuit,
}

impl Rig {
    fn new(w: &World) -> Rig {
        let c = compile(w).unwrap();
        let sim = Circuit::compile_def(&c.library(), &c.def).unwrap();
        Rig { c, sim }
    }

    /// Set ports from a bus value: `name` + bit index, e.g. a0, a1.
    fn set(&mut self, prefix: &str, value: u64) {
        for (i, src) in self.c.inputs.iter().enumerate() {
            if let InputSource::Port(n) = src {
                if let Some(bit) = n.strip_prefix(prefix).and_then(|b| b.parse::<u32>().ok()) {
                    self.sim.set_input(i, value >> bit & 1);
                } else if n == prefix {
                    self.sim.set_input(i, value & 1);
                }
            }
        }
    }

    fn eval(&mut self) {
        self.sim.eval().unwrap();
    }

    fn get(&self, prefix: &str) -> u64 {
        self.c
            .ports
            .iter()
            .filter_map(|(n, o)| {
                let rest = n.strip_prefix(prefix)?;
                let bit: u32 = if rest.is_empty() {
                    0
                } else {
                    rest.parse().ok()?
                };
                Some(self.sim.output(*o) << bit)
            })
            .sum()
    }

    fn clock(&mut self) {
        self.set("clk", 0);
        self.eval();
        self.set("clk", 1);
        self.eval();
        self.set("clk", 0);
        self.eval();
    }

    fn native(&self) -> String {
        self.c.devices[0].native.clone().unwrap()
    }
}

#[test]
fn ram_stores_words() {
    let w = rig(Device::Ram { addr: 3, data: 4 }, Pos::new(0, 0, 0));
    let mut r = Rig::new(&w);
    for a in 0..8 {
        r.set("a", a);
        r.set("d", (a * 3 + 1) & 0xF);
        r.set("load", 1);
        r.clock();
    }
    r.set("load", 0);
    for a in 0..8 {
        r.set("a", a);
        r.set("d", 0);
        r.clock();
        assert_eq!(r.get("q"), (a * 3 + 1) & 0xF, "address {a}");
    }
    let name = r.native();
    assert_eq!(device::ram(&mut r.sim, &name).unwrap()[2], 7);
}

#[test]
fn rom_reads_words() {
    let words = vec![0x5, 0xA, 0xF, 0x0];
    let w = rig(
        Device::Rom {
            addr: 2,
            data: 4,
            words: words.clone(),
        },
        Pos::new(0, 0, 0),
    );
    let mut r = Rig::new(&w);
    for (a, &want) in words.iter().enumerate() {
        r.set("a", a as u64);
        r.eval();
        assert_eq!(r.get("q"), want);
    }
}

#[test]
fn screen_draws_pixels() {
    let w = rig(Device::Screen { bits: 2 }, Pos::new(0, 0, 0));
    let mut r = Rig::new(&w);
    let name = r.native();
    r.set("x", 3);
    r.set("y", 1);
    r.set("d", 1);
    r.set("load", 0);
    r.clock();
    assert!(
        device::screen(&mut r.sim, &name)
            .unwrap()
            .iter()
            .all(|p| !p),
        "load=0"
    );
    r.set("load", 1);
    r.clock();
    let px = device::screen(&mut r.sim, &name).unwrap();
    assert_eq!(px.iter().filter(|p| **p).count(), 1);
    assert!(px[4 + 3]);
}

#[test]
fn keyboard_queue() {
    let w = rig(Device::Keyboard, Pos::new(0, 0, 0));
    let mut r = Rig::new(&w);
    let name = r.native();
    r.eval();
    assert_eq!(r.get("ready"), 0);
    assert!(device::press(&mut r.sim, &name, b'h'));
    assert!(device::press(&mut r.sim, &name, b'i'));
    r.eval();
    assert_eq!((r.get("k"), r.get("ready")), (b'h' as u64, 1));
    for (next, want) in [(1, b'i' as u64), (0, b'i' as u64), (1, 0)] {
        r.set("next", next);
        r.eval();
        assert_eq!(r.get("k"), want);
    }
    assert_eq!(r.get("ready"), 0);
}

#[test]
fn device_contents_survive_recompiling() {
    let mut w = rig(Device::Ram { addr: 1, data: 2 }, Pos::new(0, 0, 0));
    let mut r = Rig::new(&w);
    r.set("a", 1);
    r.set("d", 3);
    r.set("load", 1);
    r.clock();
    let name = r.native();
    let saved = device::save(&mut r.sim, &name).unwrap();
    // An unrelated edit, then a fresh compile.
    set(&mut w, Pos::new(20, 20, 0), Block::Lamp);
    let mut r2 = Rig::new(&w);
    device::restore(&mut r2.sim, &name, saved);
    r2.set("a", 1);
    r2.eval();
    assert_eq!(r2.get("q"), 3);
}

#[test]
fn device_pins_show_values() {
    // A switch wired into a display's first input: the wire, the pin cell
    // and the display's reading all see it.
    let mut w = World::new("d");
    let anchor = Pos::new(2, 0, 0);
    let e = w
        .place_device(Device::Display { bits: 4 }, anchor, Dir::East)
        .unwrap();
    w.apply(e);
    set(
        &mut w,
        Pos::new(0, 0, 0),
        Block::Switch {
            on: true,
            facing: Dir::East,
        },
    );
    set(&mut w, Pos::new(1, 0, 0), Block::Wire);
    let c = compile(&w).unwrap();
    let mut sim = Circuit::compile_def(&c.library(), &c.def).unwrap();
    sim.set_input(0, 1);
    sim.eval().unwrap();
    let d = &c.devices[0];
    let value: u64 = d
        .inputs
        .iter()
        .enumerate()
        .map(|(i, o)| o.map_or(0, |o| sim.output(o) << i))
        .sum();
    assert_eq!(value, 1);
    assert!(d.native.is_none());
}
