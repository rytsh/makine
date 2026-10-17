//! World → circuit. The world becomes a [`ChipDef`] made of NAND gates, so
//! the existing simulator (and its tests and packaging) runs it unchanged.
//!
//! 1. Every connection point is a *node*: one per wire/via/lamp cell, two
//!    per crossing (east-west and north-south), one per gate or device pin.
//! 2. Touching nodes are merged (union-find) into *nets*.
//! 3. Each net has at most one driver: a gate output, a switch, a clock or
//!    an input port. Two drivers on one net are a short circuit.
//! 4. Gates become NAND parts, switches/clocks/input ports become chip
//!    inputs, devices become native parts or library chips, and every
//!    driven net gets a chip output so the view can read any wire.
//!
//! The world may be broken (a device cell overwritten by another player):
//! pins that are gone are simply left unconnected.

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
    /// Net of each (cell, lane). Gates and inputs map to the net they
    /// drive, output ports and device pin cells to the net they read.
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
    /// (While building, this temporarily holds the pin's net.)
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

/// The lane of a conductor that a wire on side `side` joins.
fn lane(b: &Block, side: Dir) -> Lane {
    match b {
        Block::Cross if !side.is_horizontal() => Lane::Cross,
        _ => Lane::Main,
    }
}

/// Nets of a world: union-find over connection points.
struct Nets {
    uf: Uf,
    /// Conductor cells, per lane.
    cell: HashMap<(Pos, Lane), usize>,
    /// Pins of gates, devices and other blocks, per side.
    pin: HashMap<(Pos, Dir), usize>,
}

impl Nets {
    fn build(world: &World, blocks: &BTreeMap<Pos, &Block>) -> Nets {
        let mut n = Nets {
            uf: Uf(Vec::new()),
            cell: HashMap::new(),
            pin: HashMap::new(),
        };
        for (&pos, &block) in blocks {
            if is_conductor(block) {
                n.cell.insert((pos, Lane::Main), n.uf.make());
                if matches!(block, Block::Cross) {
                    n.cell.insert((pos, Lane::Cross), n.uf.make());
                }
                continue;
            }
            for d in Dir::ALL {
                if world.pin(pos, d).is_some() {
                    n.pin.insert((pos, d), n.uf.make());
                }
            }
        }
        for (&pos, &block) in blocks {
            for d in Dir::ALL {
                n.join_neighbour(world, blocks, pos, block, d);
            }
            if matches!(block, Block::Via) && matches!(blocks.get(&pos.up()), Some(Block::Via)) {
                let (a, b) = (n.cell[&(pos, Lane::Main)], n.cell[&(pos.up(), Lane::Main)]);
                n.uf.union(a, b);
            }
        }
        n
    }

    /// The node of the block at `pos` facing side `side`, if it connects.
    fn node(&self, world: &World, pos: Pos, block: &Block, side: Dir) -> Option<usize> {
        if is_conductor(block) {
            block.accepts_wire_from(side).then_some(())?;
            self.cell.get(&(pos, lane(block, side))).copied()
        } else {
            self.pin
                .get(&(pos, side))
                .copied()
                .filter(|_| world.pin(pos, side).is_some())
        }
    }

    fn join_neighbour(
        &mut self,
        world: &World,
        blocks: &BTreeMap<Pos, &Block>,
        pos: Pos,
        block: &Block,
        d: Dir,
    ) {
        let npos = pos.step(d);
        let Some(&nb) = blocks.get(&npos) else { return };
        // Pin to pin connects only an output to an input (gates back to
        // back); two outputs or two inputs side by side do not touch.
        if !is_conductor(block)
            && !is_conductor(nb)
            && world.pin(pos, d) == world.pin(npos, d.opposite())
        {
            return;
        }
        let here = self.node(world, pos, block, d);
        let there = self.node(world, npos, nb, d.opposite());
        if let (Some(a), Some(b)) = (here, there) {
            self.uf.union(a, b);
        }
    }

    /// Renumber nets densely: (cell nets, pin nets, net count).
    #[allow(clippy::type_complexity)]
    fn number(
        mut self,
    ) -> (
        HashMap<(Pos, Lane), usize>,
        HashMap<(Pos, Dir), usize>,
        usize,
    ) {
        let mut root_net: HashMap<usize, usize> = HashMap::new();
        let mut net_of = |uf: &mut Uf, n: usize| -> usize {
            let r = uf.find(n);
            let next = root_net.len();
            *root_net.entry(r).or_insert(next)
        };
        let cell = self
            .cell
            .iter()
            .map(|(&k, &n)| (k, net_of(&mut self.uf, n)))
            .collect();
        let pin = self
            .pin
            .iter()
            .map(|(&k, &n)| (k, net_of(&mut self.uf, n)))
            .collect();
        (cell, pin, root_net.len())
    }
}

/// Builds the chip of a world, net by net.
struct Builder {
    def: ChipDef,
    pin_net: HashMap<(Pos, Dir), usize>,
    /// The driver of each net and where it is.
    driver: Vec<Option<(Source, Pos)>>,
    shorts: BTreeMap<usize, Vec<Pos>>,
    /// Part inputs to wire from their net's driver once all are known.
    pending: Vec<(Sink, usize)>,
    port_names: Vec<String>,
}

impl Builder {
    fn net(&self, pos: Pos, side: Dir) -> Option<usize> {
        self.pin_net.get(&(pos, side)).copied()
    }

    /// `src` drives the net at (`pos`, `side`), if that pin is connected.
    fn drive(&mut self, pos: Pos, side: Dir, src: Source) {
        let Some(net) = self.net(pos, side) else {
            return;
        };
        match self.driver[net] {
            None => self.driver[net] = Some((src, pos)),
            Some((_, first)) => self
                .shorts
                .entry(net)
                .or_insert_with(|| vec![first])
                .push(pos),
        }
    }

    /// Part input `sink` reads the net at (`pos`, `side`) (0 if none).
    fn read(&mut self, sink: Sink, pos: Pos, side: Dir) {
        if let Some(net) = self.net(pos, side) {
            self.pending.push((sink, net));
        }
    }

    fn add_part(&mut self, kind: PartKind) -> usize {
        self.def.parts.push(Part::new(kind));
        self.def.parts.len() - 1
    }

    fn claim_port(&mut self, name: &str) -> Result<(), CompileError> {
        if self.port_names.iter().any(|n| n == name) {
            return Err(CompileError::DuplicatePort(name.to_string()));
        }
        self.port_names.push(name.to_string());
        Ok(())
    }

    /// Switches, clocks and input ports become chip inputs.
    fn input(&mut self, pos: Pos, block: &Block) -> Result<Option<InputSource>, CompileError> {
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
                self.claim_port(name)?;
                (name.clone(), InputSource::Port(name.clone()), *facing)
            }
            _ => return Ok(None),
        };
        let i = self.def.inputs.len();
        self.def.inputs.push(Pin::new(pin_name, 1));
        self.drive(pos, facing, Source::Input(i));
        Ok(Some(source))
    }

    /// A gate, as NAND parts.
    fn gate(&mut self, pos: Pos, gate: Gate, facing: Dir) {
        let sides = match gate.inputs() {
            1 => vec![facing.opposite()],
            _ => vec![facing.ccw(), facing.cw()],
        };
        let mut ins = Vec::new();
        let out = gate_parts(&mut self.def, gate, sides.len(), &mut ins);
        for (input, sink) in ins {
            self.read(sink, pos, sides[input]);
        }
        self.drive(pos, facing, out);
    }

    /// A device: a native part, a library chip, or (a display) nothing but
    /// the nets it reads. Pins whose cells were overwritten are left out.
    fn device(&mut self, anchor: Pos, device: &Device, facing: Dir) -> DeviceInfo {
        let l = device::layout(device, anchor, facing);
        let in_nets: Vec<Option<usize>> = l.inputs.iter().map(|&(p, d)| self.net(p, d)).collect();
        let native = device.native().map(|_| {
            let name = device.native_name(anchor);
            let part = self.add_part(PartKind::Chip(name.clone()));
            for (pin, &(p, d)) in l.inputs.iter().enumerate() {
                self.read(Sink::Part { part, pin }, p, d);
            }
            for (pin, &(p, d)) in l.outputs.iter().enumerate() {
                self.drive(p, d, Source::Part { part, pin });
            }
            name
        });
        if let Device::Chip {
            name,
            inputs,
            outputs,
        } = device
        {
            self.chip(name, inputs, outputs, &l);
        }
        DeviceInfo {
            anchor,
            device: device.clone(),
            native,
            inputs: in_nets,
        }
    }

    /// A library chip: one part, with a Join gathering each input bus from
    /// its 1-bit cells and a Split fanning each output bus out to them.
    fn chip(&mut self, name: &str, inputs: &[Pin], outputs: &[Pin], l: &device::Layout) {
        let chip = self.add_part(PartKind::Chip(name.to_string()));
        let mut cells = l.inputs.iter();
        for (j, p) in inputs.iter().enumerate() {
            let w = p.width as usize;
            let (part, first) = if w == 1 {
                (chip, j)
            } else {
                let join = self.add_part(PartKind::Join(vec![1; w]));
                self.def.wires.push(Wire {
                    from: Source::Part { part: join, pin: 0 },
                    to: Sink::Part { part: chip, pin: j },
                });
                (join, 0)
            };
            for (bit, &(c, d)) in cells.by_ref().take(w).enumerate() {
                self.read(
                    Sink::Part {
                        part,
                        pin: first + bit,
                    },
                    c,
                    d,
                );
            }
        }
        let mut cells = l.outputs.iter();
        for (j, p) in outputs.iter().enumerate() {
            let w = p.width as usize;
            let (part, first) = if w == 1 {
                (chip, j)
            } else {
                let split = self.add_part(PartKind::Split(vec![1; w]));
                self.def.wires.push(Wire {
                    from: Source::Part { part: chip, pin: j },
                    to: Sink::Part {
                        part: split,
                        pin: 0,
                    },
                });
                (split, 0)
            };
            for (bit, &(c, d)) in cells.by_ref().take(w).enumerate() {
                self.drive(
                    c,
                    d,
                    Source::Part {
                        part,
                        pin: first + bit,
                    },
                );
            }
        }
    }

    /// An output port: a named chip output, 0 if nothing drives it.
    fn output_port(&mut self, pos: Pos, name: &str, facing: Dir) -> Result<usize, CompileError> {
        self.claim_port(name)?;
        let src = match self
            .net(pos, facing.opposite())
            .and_then(|n| self.driver[n])
        {
            Some((src, _)) => src,
            None => Source::Part {
                part: self.add_part(PartKind::Const { width: 1, value: 0 }),
                pin: 0,
            },
        };
        let o = self.def.outputs.len();
        self.def.outputs.push(Pin::new(name, 1));
        self.def.wires.push(Wire {
            from: src,
            to: Sink::Output(o),
        });
        Ok(o)
    }
}

pub fn compile(world: &World) -> Result<Compiled, CompileError> {
    let blocks: BTreeMap<Pos, &Block> = world.blocks().collect();
    let (mut nets, pin_net, net_count) = Nets::build(world, &blocks).number();
    let mut b = Builder {
        def: ChipDef::new(&world.name, &[], &[]),
        pin_net,
        driver: vec![None; net_count],
        shorts: BTreeMap::new(),
        pending: Vec::new(),
        port_names: Vec::new(),
    };

    let mut inputs = Vec::new();
    for (&pos, &block) in &blocks {
        inputs.extend(b.input(pos, block)?);
    }
    let mut devices = Vec::new();
    for (&pos, &block) in &blocks {
        match block {
            Block::Gate { gate, facing } => b.gate(pos, *gate, *facing),
            Block::Device { device, facing } => devices.push(b.device(pos, device, *facing)),
            _ => {}
        }
    }
    if let Some((_, list)) = std::mem::take(&mut b.shorts).into_iter().next() {
        return Err(CompileError::ShortCircuit(list));
    }

    // Wire part inputs from their nets' drivers; undriven nets read 0.
    for (sink, net) in std::mem::take(&mut b.pending) {
        if let Some((src, _)) = b.driver[net] {
            b.def.wires.push(Wire {
                from: src,
                to: sink,
            });
        }
    }

    // Every driven net gets a chip output, so the view can show any wire.
    let mut net_output: Vec<Option<usize>> = vec![None; net_count];
    for (net, d) in b.driver.iter().enumerate() {
        if let Some((src, _)) = d {
            let o = b.def.outputs.len();
            b.def.outputs.push(Pin::new(format!("n{net}"), 1));
            b.def.wires.push(Wire {
                from: *src,
                to: Sink::Output(o),
            });
            net_output[net] = Some(o);
        }
    }
    for d in &mut devices {
        for o in &mut d.inputs {
            *o = o.and_then(|n| net_output.get(n).copied().flatten());
        }
    }

    let mut ports = Vec::new();
    for (&pos, &block) in &blocks {
        if let Block::Port {
            name,
            output: true,
            facing,
        } = block
        {
            ports.push((name.clone(), b.output_port(pos, name, *facing)?));
        }
    }

    let lamps = blocks
        .iter()
        .filter(|(_, b)| matches!(b, Block::Lamp))
        .map(|(&p, _)| (p, net_output[nets[&(p, Lane::Main)]]))
        .collect();

    // Non-conductor cells show the net they drive (gates, inputs) or read
    // (output ports, device pins).
    for (&pos, &block) in &blocks {
        if is_conductor(block) {
            continue;
        }
        let net = match block {
            Block::Port {
                output: true,
                facing,
                ..
            } => b.net(pos, facing.opposite()),
            Block::Device { .. } | Block::Cover { .. } => {
                Dir::ALL.iter().find_map(|&d| b.net(pos, d))
            }
            other => other.facing().and_then(|f| b.net(pos, f)),
        };
        if let Some(n) = net {
            nets.insert((pos, Lane::Main), n);
        }
    }

    let natives = devices
        .iter()
        .filter_map(|d| Some((d.native.clone()?, d.device.native()?)))
        .collect();
    Ok(Compiled {
        def: b.def,
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

/// Add NAND parts computing `gate` with `n` inputs. Returns the output;
/// the parts' external inputs are recorded in `ins` as (gate input, sink).
fn gate_parts(def: &mut ChipDef, gate: Gate, n: usize, ins: &mut Vec<(usize, Sink)>) -> Source {
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
    debug_assert_eq!(n, gate.inputs());
    let mut ext = |input: usize, part: usize, pin: usize| {
        ins.push((input, Sink::Part { part, pin }));
    };
    match gate {
        Gate::Nand => {
            let n = nand(def);
            ext(0, n, 0);
            ext(1, n, 1);
            out(n)
        }
        Gate::Not => {
            let n = nand(def);
            ext(0, n, 0);
            ext(0, n, 1);
            out(n)
        }
        Gate::And => {
            let n = nand(def);
            ext(0, n, 0);
            ext(1, n, 1);
            let m = nand(def);
            link(def, n, m, 0);
            link(def, n, m, 1);
            out(m)
        }
        Gate::Or | Gate::Nor => {
            let na = nand(def);
            ext(0, na, 0);
            ext(0, na, 1);
            let nb = nand(def);
            ext(1, nb, 0);
            ext(1, nb, 1);
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
            ext(0, n, 0);
            ext(1, n, 1);
            let l = nand(def);
            ext(0, l, 0);
            link(def, n, l, 1);
            let r = nand(def);
            ext(1, r, 0);
            link(def, n, r, 1);
            let o = nand(def);
            link(def, l, o, 0);
            link(def, r, o, 1);
            out(o)
        }
    }
}
