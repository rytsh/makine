//! World → circuit. The world becomes a [`ChipDef`] made of NAND gates, so
//! the existing simulator (and its tests and packaging) runs it unchanged.
//!
//! 1. Every connection point is a *node*: one per wire/via/lamp cell, two
//!    per crossing (east-west and north-south), one per gate or device pin.
//! 2. Touching nodes are merged (union-find) into *nets*.
//! 3. Each net has at most one driver: a gate output, a switch, a clock or
//!    an input port. Two drivers on one net are a short circuit.
//! 4. Gates become NAND parts, switches/clocks/input ports become chip
//!    inputs, and every driven net gets a chip output so the view can read
//!    any wire.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use makine_sim::{ChipDef, Library, NativeDef, Part, PartKind, Pin, Sink, Source, Wire};

use crate::block::{Block, Dir, Gate};
use crate::device::{self, Device};
use crate::world::{Pos, World};

/// Which signal of a cell: crossings carry two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lane {
    /// The cell's only signal (or a crossing's east-west one).
    Main,
    /// A crossing's north-south signal.
    Cross,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    /// More than one output drives the same net; the positions are the
    /// drivers.
    ShortCircuit(Vec<Pos>),
    /// Two ports with the same name.
    DuplicatePort(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::ShortCircuit(p) => write!(
                f,
                "kısa devre: {} çıkış aynı tele bağlı ({})",
                p.len(),
                p.iter()
                    .map(|p| format!("{},{},{}", p.x, p.y, p.z))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            CompileError::DuplicatePort(n) => write!(f, "'{n}' adında iki port var"),
        }
    }
}

impl std::error::Error for CompileError {}

/// Where a chip input's value comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputSource {
    Switch(Pos),
    Clock { pos: Pos, period: u32 },
    Port(String),
}

/// The circuit of a world, plus how to find each cell's signal in it.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub def: ChipDef,
    /// Net of each (cell, lane). Gates and devices map to their output net.
    pub nets: HashMap<(Pos, Lane), usize>,
    /// For each net, the chip output observing it (driven nets only).
    pub net_output: Vec<Option<usize>>,
    /// Chip inputs in order.
    pub inputs: Vec<InputSource>,
    /// Lamps: position and the chip output to read.
    pub lamps: Vec<(Pos, Option<usize>)>,
    /// Output ports: name and chip output.
    pub ports: Vec<(String, usize)>,
    /// Devices by anchor, with their native part's name (if they have
    /// behaviour) and the chip outputs showing their input wires.
    pub devices: Vec<DeviceInfo>,
    /// Native parts the circuit uses, see [`Compiled::library`].
    pub natives: Vec<(String, NativeDef)>,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub anchor: Pos,
    pub device: Device,
    pub native: Option<String>,
    /// For each input pin: the chip output carrying its value, if driven.
    pub inputs: Vec<Option<usize>>,
}

impl Compiled {
    pub fn net_at(&self, pos: Pos, lane: Lane) -> Option<usize> {
        self.nets.get(&(pos, lane)).copied()
    }

    /// The chip output showing a cell's value, if the cell is on a driven net.
    pub fn output_at(&self, pos: Pos, lane: Lane) -> Option<usize> {
        self.net_output[self.net_at(pos, lane)?]
    }

    /// The library to simulate [`Compiled::def`] with: the devices' native
    /// parts.
    pub fn library(&self) -> Library {
        self.library_with(&Library::new())
    }

    /// `base` (the player's chips, for chip blocks) plus the devices'
    /// native parts.
    pub fn library_with(&self, base: &Library) -> Library {
        let mut lib = base.clone();
        for (name, n) in &self.natives {
            lib.insert_native(name.clone(), n.clone());
        }
        lib
    }
}

struct Uf(Vec<usize>);

impl Uf {
    fn make(&mut self) -> usize {
        self.0.push(self.0.len());
        self.0.len() - 1
    }

    fn find(&mut self, mut n: usize) -> usize {
        while self.0[n] != n {
            self.0[n] = self.0[self.0[n]];
            n = self.0[n];
        }
        n
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[b] = a;
        }
    }
}

fn is_conductor(b: &Block) -> bool {
    matches!(
        b,
        Block::Wire | Block::Straight { .. } | Block::Cross | Block::Via | Block::Lamp
    )
}

/// Device pins of a world, computed once per compile: (cell, side) ->
/// whether it is an output.
struct DevicePins(HashMap<(Pos, Dir), bool>);

impl DevicePins {
    fn new(world: &World) -> Self {
        let mut m = HashMap::new();
        for (anchor, b) in world.blocks() {
            if let Block::Device { device, facing } = b {
                let l = device::layout(device, anchor, *facing);
                m.extend(l.inputs.iter().map(|&k| (k, false)));
                m.extend(l.outputs.iter().map(|&k| (k, true)));
            }
        }
        DevicePins(m)
    }

    /// Whether the block at `pos` connects to a wire on side `side`.
    fn accepts(&self, world: &World, pos: Pos, side: Dir) -> bool {
        match world.get(pos) {
            Some(Block::Device { .. } | Block::Cover { .. }) => self.0.contains_key(&(pos, side)),
            Some(b) => b.accepts_wire_from(side),
            None => false,
        }
    }
}

/// Whether `b`'s pin on side `d` is an output (drives the net).
fn is_output_pin(pins: &DevicePins, pos: Pos, b: &Block, d: Dir) -> bool {
    match b {
        Block::Device { .. } | Block::Cover { .. } => pins.0.get(&(pos, d)) == Some(&true),
        Block::Gate { facing, .. }
        | Block::Switch { facing, .. }
        | Block::Clock { facing, .. }
        | Block::Port {
            output: false,
            facing,
            ..
        } => d == *facing,
        _ => false,
    }
}

struct Nodes {
    cell: HashMap<(Pos, Lane), usize>,
    pin: HashMap<(Pos, Dir), usize>,
}

impl Nodes {
    /// The node of `block` (at `pos`) that a neighbour on side `side`
    /// connects to, if any.
    fn facing(
        &self,
        world: &World,
        pins: &DevicePins,
        block: &Block,
        pos: Pos,
        side: Dir,
    ) -> Option<usize> {
        if !pins.accepts(world, pos, side) {
            return None;
        }
        match block {
            Block::Wire | Block::Straight { .. } | Block::Via | Block::Lamp => {
                self.cell.get(&(pos, Lane::Main)).copied()
            }
            Block::Cross => {
                let lane = if side.is_horizontal() {
                    Lane::Main
                } else {
                    Lane::Cross
                };
                self.cell.get(&(pos, lane)).copied()
            }
            _ => self.pin.get(&(pos, side)).copied(),
        }
    }
}

pub fn compile(world: &World) -> Result<Compiled, CompileError> {
    let blocks: BTreeMap<Pos, &Block> = world.blocks().collect();
    let pins = DevicePins::new(world);
    let mut uf = Uf(Vec::new());
    let mut nodes = Nodes {
        cell: HashMap::new(),
        pin: HashMap::new(),
    };

    for (&pos, block) in &blocks {
        match block {
            Block::Wire | Block::Straight { .. } | Block::Via | Block::Lamp => {
                nodes.cell.insert((pos, Lane::Main), uf.make());
            }
            Block::Cross => {
                nodes.cell.insert((pos, Lane::Main), uf.make());
                nodes.cell.insert((pos, Lane::Cross), uf.make());
            }
            _ => {
                for d in Dir::ALL {
                    if pins.accepts(world, pos, d) {
                        nodes.pin.insert((pos, d), uf.make());
                    }
                }
            }
        }
    }

    for (&pos, block) in &blocks {
        for d in Dir::ALL {
            let Some(here) = nodes.facing(world, &pins, block, pos, d) else {
                continue;
            };
            let npos = pos.step(d);
            let Some(nb) = blocks.get(&npos) else {
                continue;
            };
            // Pin to pin connects only output to input (gates placed back to
            // back); two outputs or two inputs side by side do not touch.
            if !is_conductor(block)
                && !is_conductor(nb)
                && is_output_pin(&pins, pos, block, d)
                    == is_output_pin(&pins, npos, nb, d.opposite())
            {
                continue;
            }
            if let Some(there) = nodes.facing(world, &pins, nb, npos, d.opposite()) {
                uf.union(here, there);
            }
        }
        if matches!(block, Block::Via) && matches!(blocks.get(&pos.up()), Some(Block::Via)) {
            uf.union(
                nodes.cell[&(pos, Lane::Main)],
                nodes.cell[&(pos.up(), Lane::Main)],
            );
        }
    }

    // Dense net numbers.
    let mut root_net: HashMap<usize, usize> = HashMap::new();
    let mut net_of = |uf: &mut Uf, n: usize| -> usize {
        let r = uf.find(n);
        let next = root_net.len();
        *root_net.entry(r).or_insert(next)
    };
    let mut nets: HashMap<(Pos, Lane), usize> = nodes
        .cell
        .iter()
        .map(|(&k, &n)| (k, net_of(&mut uf, n)))
        .collect();
    let pin_net: HashMap<(Pos, Dir), usize> = nodes
        .pin
        .iter()
        .map(|(&k, &n)| (k, net_of(&mut uf, n)))
        .collect();
    let net_count = root_net.len();

    let mut def = ChipDef::new(&world.name, &[], &[]);
    let mut driver: Vec<Option<(Source, Pos)>> = vec![None; net_count];
    let mut shorts: BTreeMap<usize, Vec<Pos>> = BTreeMap::new();
    let mut drive =
        |driver: &mut Vec<Option<(Source, Pos)>>, net: usize, src: Source, pos: Pos| match driver
            [net]
        {
            None => driver[net] = Some((src, pos)),
            Some((_, first)) => shorts.entry(net).or_insert_with(|| vec![first]).push(pos),
        };

    // Inputs.
    let mut inputs = Vec::new();
    let mut port_names: Vec<String> = Vec::new();
    for (&pos, block) in &blocks {
        let (pin_name, source, facing) = match block {
            Block::Switch { facing, .. } => (
                format!("sw{},{},{}", pos.x, pos.y, pos.z),
                InputSource::Switch(pos),
                *facing,
            ),
            Block::Clock { facing, period } => (
                format!("clk{},{},{}", pos.x, pos.y, pos.z),
                InputSource::Clock {
                    pos,
                    period: *period,
                },
                *facing,
            ),
            Block::Port {
                name,
                output: false,
                facing,
            } => {
                if port_names.contains(name) {
                    return Err(CompileError::DuplicatePort(name.clone()));
                }
                port_names.push(name.clone());
                (name.clone(), InputSource::Port(name.clone()), *facing)
            }
            _ => continue,
        };
        let i = def.inputs.len();
        def.inputs.push(Pin::new(pin_name, 1));
        inputs.push(source);
        drive(&mut driver, pin_net[&(pos, facing)], Source::Input(i), pos);
    }

    // Gates, as NAND parts. Their inputs are wired once all drivers are known.
    let mut gate_inputs: Vec<(Sink, usize)> = Vec::new();
    for (&pos, block) in &blocks {
        let Block::Gate { gate, facing } = block else {
            continue;
        };
        let ins: Vec<usize> = match gate.inputs() {
            1 => vec![pin_net[&(pos, facing.opposite())]],
            _ => vec![pin_net[&(pos, facing.ccw())], pin_net[&(pos, facing.cw())]],
        };
        let out = gate_parts(&mut def, *gate, &ins, &mut gate_inputs);
        drive(&mut driver, pin_net[&(pos, *facing)], out, pos);
    }

    // Devices: native parts, outputs drive their nets; inputs are wired
    // like gate inputs.
    let mut devices = Vec::new();
    let mut natives = Vec::new();
    for (&pos, block) in &blocks {
        let Block::Device { device, facing } = block else {
            continue;
        };
        let l = device::layout(device, pos, *facing);
        let in_nets: Vec<usize> = l.inputs.iter().map(|k| pin_net[k]).collect();
        let native = device.native().map(|n| {
            let name = device.native_name(pos);
            let part = def.parts.len();
            def.parts.push(Part::new(PartKind::Chip(name.clone())));
            for (i, &net) in in_nets.iter().enumerate() {
                gate_inputs.push((Sink::Part { part, pin: i }, net));
            }
            for (i, k) in l.outputs.iter().enumerate() {
                drive(&mut driver, pin_net[k], Source::Part { part, pin: i }, pos);
            }
            natives.push((name.clone(), n));
            name
        });
        if let Device::Chip {
            name,
            inputs: ins,
            outputs: outs,
        } = device
        {
            let mut d = |net, src, at| drive(&mut driver, net, src, at);
            chip_parts(
                &mut def,
                name,
                ins,
                outs,
                &in_nets,
                &l.outputs,
                &pin_net,
                pos,
                &mut gate_inputs,
                &mut d,
            );
        }
        devices.push((pos, device.clone(), native, in_nets));
    }

    if let Some((_, list)) = shorts.into_iter().next() {
        return Err(CompileError::ShortCircuit(list));
    }

    // Undriven nets read 0: the sink is simply left unconnected.
    for (sink, net) in gate_inputs {
        if let Some((src, _)) = driver[net] {
            def.wires.push(Wire {
                from: src,
                to: sink,
            });
        }
    }

    let mut net_output: Vec<Option<usize>> = vec![None; net_count];
    for (net, d) in driver.iter().enumerate() {
        if let Some((src, _)) = d {
            let o = def.outputs.len();
            def.outputs.push(Pin::new(format!("n{net}"), 1));
            def.wires.push(Wire {
                from: *src,
                to: Sink::Output(o),
            });
            net_output[net] = Some(o);
        }
    }

    let devices = devices
        .into_iter()
        .map(|(anchor, device, native, in_nets)| DeviceInfo {
            anchor,
            device,
            native,
            inputs: in_nets.iter().map(|&n| net_output[n]).collect(),
        })
        .collect();

    let lamps = blocks
        .iter()
        .filter(|(_, b)| matches!(b, Block::Lamp))
        .map(|(&p, _)| (p, net_output[nets[&(p, Lane::Main)]]))
        .collect();

    let mut ports = Vec::new();
    for (&pos, block) in &blocks {
        let Block::Port {
            name,
            output: true,
            facing,
        } = block
        else {
            continue;
        };
        if port_names.contains(name) {
            return Err(CompileError::DuplicatePort(name.clone()));
        }
        port_names.push(name.clone());
        let net = pin_net[&(pos, facing.opposite())];
        let o = def.outputs.len();
        def.outputs.push(Pin::new(name.clone(), 1));
        let src = match driver[net] {
            Some((src, _)) => src,
            None => {
                def.parts
                    .push(Part::new(PartKind::Const { width: 1, value: 0 }));
                Source::Part {
                    part: def.parts.len() - 1,
                    pin: 0,
                }
            }
        };
        def.wires.push(Wire {
            from: src,
            to: Sink::Output(o),
        });
        ports.push((name.clone(), o));
    }

    // Gates and devices show the value of the net they drive (or read).
    for (&pos, block) in &blocks {
        if matches!(block, Block::Device { .. }) {
            continue;
        }
        if let Some(f) = block.facing() {
            let side = match block {
                Block::Port { output: true, .. } => f.opposite(),
                _ => f,
            };
            if let Some(&n) = pin_net.get(&(pos, side)) {
                nets.insert((pos, Lane::Main), n);
            }
        }
    }

    // Device pin cells show their pin's value.
    for (&(pos, _), &n) in &pin_net {
        if matches!(blocks[&pos], Block::Device { .. } | Block::Cover { .. }) {
            nets.insert((pos, Lane::Main), n);
        }
    }

    Ok(Compiled {
        def,
        nets,
        net_output,
        inputs,
        lamps,
        ports,
        devices,
        natives,
    })
}

/// Check that every chip block matches its chip in `lib` (exists, same
/// pins). A chip whose pins changed must be placed again.
pub fn check_chips(world: &World, lib: &Library) -> Result<(), String> {
    for (_, b) in world.blocks() {
        let Block::Device {
            device:
                Device::Chip {
                    name,
                    inputs,
                    outputs,
                },
            ..
        } = b
        else {
            continue;
        };
        let pins = lib
            .kind_pins(&PartKind::Chip(name.clone()))
            .ok_or_else(|| format!("'{name}' çipi kütüphanende yok"))?;
        if &pins.inputs != inputs || &pins.outputs != outputs {
            return Err(format!(
                "'{name}' çipinin uçları değişmiş: dünyadakini silip yeniden koy"
            ));
        }
    }
    Ok(())
}

/// A library chip: one part, with a Join gathering each input bus from its
/// 1-bit nets and a Split fanning each output bus out to them.
#[allow(clippy::too_many_arguments)]
fn chip_parts(
    def: &mut ChipDef,
    name: &str,
    inputs: &[Pin],
    outputs: &[Pin],
    in_nets: &[usize],
    out_pins: &[(Pos, Dir)],
    pin_net: &HashMap<(Pos, Dir), usize>,
    pos: Pos,
    pending: &mut Vec<(Sink, usize)>,
    drive: &mut dyn FnMut(usize, Source, Pos),
) {
    let chip = def.parts.len();
    def.parts.push(Part::new(PartKind::Chip(name.to_string())));
    let mut nets = in_nets.iter();
    for (j, p) in inputs.iter().enumerate() {
        if p.width == 1 {
            pending.push((Sink::Part { part: chip, pin: j }, *nets.next().unwrap()));
            continue;
        }
        let join = def.parts.len();
        def.parts
            .push(Part::new(PartKind::Join(vec![1; p.width as usize])));
        for bit in 0..p.width as usize {
            pending.push((
                Sink::Part {
                    part: join,
                    pin: bit,
                },
                *nets.next().unwrap(),
            ));
        }
        def.wires.push(Wire {
            from: Source::Part { part: join, pin: 0 },
            to: Sink::Part { part: chip, pin: j },
        });
    }
    let mut outs = out_pins.iter();
    for (j, p) in outputs.iter().enumerate() {
        if p.width == 1 {
            drive(
                pin_net[outs.next().unwrap()],
                Source::Part { part: chip, pin: j },
                pos,
            );
            continue;
        }
        let split = def.parts.len();
        def.parts
            .push(Part::new(PartKind::Split(vec![1; p.width as usize])));
        def.wires.push(Wire {
            from: Source::Part { part: chip, pin: j },
            to: Sink::Part {
                part: split,
                pin: 0,
            },
        });
        for bit in 0..p.width as usize {
            drive(
                pin_net[outs.next().unwrap()],
                Source::Part {
                    part: split,
                    pin: bit,
                },
                pos,
            );
        }
    }
}

/// Add NAND parts computing `gate` over the nets `ins`. Returns the output
/// source; external inputs are recorded in `pending` as (sink, net).
fn gate_parts(
    def: &mut ChipDef,
    gate: Gate,
    ins: &[usize],
    pending: &mut Vec<(Sink, usize)>,
) -> Source {
    fn nand(def: &mut ChipDef) -> usize {
        def.parts.push(Part::new(PartKind::Nand));
        def.parts.len() - 1
    }
    fn out(p: usize) -> Source {
        Source::Part { part: p, pin: 0 }
    }
    fn link(def: &mut ChipDef, from: usize, to: usize, pin: usize) {
        def.wires.push(Wire {
            from: out(from),
            to: Sink::Part { part: to, pin },
        });
    }
    let mut ext = |part: usize, pin: usize, net: usize| {
        pending.push((Sink::Part { part, pin }, net));
    };
    match gate {
        Gate::Nand => {
            let n = nand(def);
            ext(n, 0, ins[0]);
            ext(n, 1, ins[1]);
            out(n)
        }
        Gate::Not => {
            let n = nand(def);
            ext(n, 0, ins[0]);
            ext(n, 1, ins[0]);
            out(n)
        }
        Gate::And => {
            let n = nand(def);
            ext(n, 0, ins[0]);
            ext(n, 1, ins[1]);
            let m = nand(def);
            link(def, n, m, 0);
            link(def, n, m, 1);
            out(m)
        }
        Gate::Or | Gate::Nor => {
            let na = nand(def);
            ext(na, 0, ins[0]);
            ext(na, 1, ins[0]);
            let nb = nand(def);
            ext(nb, 0, ins[1]);
            ext(nb, 1, ins[1]);
            let o = nand(def);
            link(def, na, o, 0);
            link(def, nb, o, 1);
            if gate == Gate::Or {
                return out(o);
            }
            let inv = nand(def);
            link(def, o, inv, 0);
            link(def, o, inv, 1);
            out(inv)
        }
        Gate::Xor => {
            let n = nand(def);
            ext(n, 0, ins[0]);
            ext(n, 1, ins[1]);
            let l = nand(def);
            ext(l, 0, ins[0]);
            link(def, n, l, 1);
            let r = nand(def);
            ext(r, 0, ins[1]);
            link(def, n, r, 1);
            let o = nand(def);
            link(def, l, o, 0);
            link(def, r, o, 1);
            out(o)
        }
    }
}
