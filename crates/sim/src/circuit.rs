//! Compiles a hierarchical [`ChipDef`] into a flat list of NAND gates and
//! evaluates it.
//!
//! Every net is one bit wide and holds a `u64`: 64 independent simulations
//! ("lanes") run in parallel, so up to 64 test vectors are checked in a
//! single pass. Buses are just lists of nets.
//!
//! Gates are ordered by dependency. Feedback loops (latches built from NANDs)
//! are allowed: the gates of each loop are grouped and evaluated repeatedly
//! until they settle. Net values persist between calls so loops hold state.

use std::fmt;

use makine_lang::{tr, trf};

use crate::chip::{ChipDef, Library, PartKind, PartPins, Pin, Sink, Source, MAX_WIDTH};
use crate::native::{Behavior, NativeDef};

pub type Net = u32;

const MAX_SETTLE_ROUNDS: usize = 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    UnknownChip {
        name: String,
    },
    UnknownPart {
        chip: String,
        part: String,
    },
    PinOutOfRange {
        chip: String,
        pin: String,
    },
    MultipleDrivers {
        chip: String,
        sink: String,
    },
    WidthMismatch {
        chip: String,
        sink: String,
        from: u8,
        to: u8,
    },
    Recursive {
        chip: String,
    },
    /// A repeated part whose pins would be wider than 64 bits.
    TooWide {
        chip: String,
        part: String,
    },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::UnknownChip { name } => {
f.write_str(&trf!("bilinmeyen çip: {name}", "unknown chip: {name}"))
}
            CompileError::UnknownPart { chip, part } => {
                f.write_str(&trf!(
"{chip}: bilinmeyen parça kullanılmış: {part}",
"{chip}: uses an unknown part: {part}"
))
            }
            CompileError::PinOutOfRange { chip, pin } => {
                f.write_str(&trf!(
"{chip}: tel olmayan bir uca bağlı: {pin}",
"{chip}: connected to a pin that does not exist: {pin}"
))
            }
            CompileError::MultipleDrivers { chip, sink } => {
                f.write_str(&trf!(
"{chip}: {sink} birden fazla kaynağa bağlı",
"{chip}: {sink} is connected to more than one source"
))
            }
            CompileError::WidthMismatch {
                chip,
                sink,
                from,
                to,
            } => {
                f.write_str(&trf!(
                    "{chip}: {sink} {to} bit genişliğinde ama {from} bitlik bir tele bağlı",
                    "{chip}: {sink} is {to} bits wide but connected to a {from}-bit wire"
                ))
            }
            CompileError::Recursive { chip } => {
f.write_str(&trf!("{chip} kendi kendini içeriyor", "{chip} contains itself"))
}
            CompileError::TooWide { chip, part } => f.write_str(&trf!(
                "{chip}: {part} parçasının uçları 64 bitten geniş olamaz; tekrar sayısını azalt",
                "{chip}: the pins of the {part} part cannot be wider than 64 bits; lower the repeat count"
            )),
        }
    }
}

impl std::error::Error for CompileError {}

/// The circuit did not settle: some feedback loop keeps flipping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Oscillation;

impl fmt::Display for Oscillation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(tr!(
            "devre salınıyor, kararlı hale gelmiyor",
            "the circuit oscillates and does not settle"
        ))
    }
}

impl std::error::Error for Oscillation {}

#[derive(Clone, Copy, Debug)]
struct Gate {
    a: Net,
    b: Net,
    out: Net,
}

/// A native part instance: one behaviour per lane.
struct NativeNode {
    name: String,
    inputs: Vec<Vec<Net>>,
    outputs: Vec<Vec<Net>>,
    /// Behaviour per lane, created on first use: a memory should not be
    /// allocated 64 times when only one lane runs.
    lanes: Vec<Box<dyn Behavior>>,
    make: crate::native::Factory,
    /// Scratch buffers.
    ins: Vec<u64>,
    outs: Vec<u64>,
}

impl NativeNode {
    /// Evaluate in every active lane. Returns lanes whose outputs changed.
    fn ensure(&mut self, lanes: u32) {
        while self.lanes.len() < lanes as usize {
            self.lanes.push((self.make)());
        }
    }

    fn eval(&mut self, v: &mut [u64], lanes: u32) -> u64 {
        self.ensure(lanes);
        let mut changed = 0u64;
        for lane in 0..lanes {
            for (i, bus) in self.inputs.iter().enumerate() {
                self.ins[i] = read_bus(v, bus, lane);
            }
            self.lanes[lane as usize].eval(&self.ins, &mut self.outs);
            for (o, bus) in self.outputs.iter().enumerate() {
                let val = self.outs[o];
                for (bit, &n) in bus.iter().enumerate() {
                    let b = (val >> bit & 1) << lane;
                    let old = v[n as usize];
                    let new = (old & !(1 << lane)) | b;
                    changed |= old ^ new;
                    v[n as usize] = new;
                }
            }
        }
        changed
    }

    fn commit(&mut self, v: &[u64], lanes: u32) {
        self.ensure(lanes);
        for lane in 0..lanes {
            for (i, bus) in self.inputs.iter().enumerate() {
                self.ins[i] = read_bus(v, bus, lane);
            }
            self.lanes[lane as usize].commit(&self.ins);
        }
    }
}

impl Clone for NativeNode {
    /// Clones get fresh (power-on) state.
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            lanes: Vec::new(),
            make: self.make.clone(),
            ins: self.ins.clone(),
            outs: self.outs.clone(),
        }
    }
}

impl fmt::Debug for NativeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeNode")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

fn read_bus(v: &[u64], bus: &[Net], lane: u32) -> u64 {
    bus.iter()
        .enumerate()
        .fold(0, |acc, (bit, &n)| acc | (v[n as usize] >> lane & 1) << bit)
}

/// One step of the evaluation schedule.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// A run of gates in `Circuit::gates`, none on a loop.
    Gates { start: u32, end: u32 },
    /// A strongly connected component (feedback loop): these gates and
    /// natives are re-evaluated until nothing changes. Only gates whose
    /// inputs changed are revisited (see `Circuit::loop_fanout`).
    Loop {
        start: u32,
        end: u32,
        nstart: u32,
        nend: u32,
    },
    /// A native part not on a loop, by index into `Circuit::natives`.
    Native(u32),
}

#[derive(Clone, Debug)]
pub struct Circuit {
    values: Vec<u64>,
    gates: Vec<Gate>,
    natives: Vec<NativeNode>,
    /// Indices into `natives` in schedule order; loops refer to ranges.
    order: Vec<u32>,
    steps: Vec<Step>,
    /// Natives with state to commit after each evaluation.
    stateful: Vec<u32>,
    /// For loop gates: indices (into `gates`) of the gates in the same loop
    /// that read its output. CSR layout: `fanout_idx[g]..fanout_idx[g+1]`
    /// into `fanout`, offset by the loop's `start`.
    fanout_idx: Vec<u32>,
    fanout: Vec<u32>,
    /// Scratch bitset for loop evaluation, one bit per gate.
    dirty: Vec<u64>,
    /// Number of lanes native parts simulate; the rest are left undefined.
    /// Defaults to 1, since a native may be large (a memory).
    native_lanes: u32,
    inputs: Vec<Vec<Net>>,
    outputs: Vec<Vec<Net>>,
    /// Source nets of each wire of the top-level chip, for display.
    wire_nets: Vec<Vec<Net>>,
}

impl Circuit {
    pub fn compile(lib: &Library, chip: &str) -> Result<Self, CompileError> {
        let def = lib.get(chip).ok_or_else(|| CompileError::UnknownChip {
            name: chip.to_string(),
        })?;
        Self::compile_def(lib, def)
    }

    /// Compile a chip that is not (yet) part of the library, e.g. one the
    /// user is still editing.
    pub fn compile_def(lib: &Library, def: &ChipDef) -> Result<Self, CompileError> {
        let mut f = Flattener {
            lib,
            uf: UnionFind::default(),
            gates: Vec::new(),
            natives: Vec::new(),
            rails: None,
        };
        let inputs: Vec<Vec<Net>> = def.inputs.iter().map(|p| f.fresh(p.width)).collect();
        let mut stack = Vec::new();
        let (outputs, wire_nets) = f.instantiate(def, &inputs, &mut stack)?;
        Ok(f.finish(inputs, outputs, wire_nets))
    }

    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }

    pub fn native_count(&self) -> usize {
        self.natives.len()
    }

    pub fn has_feedback(&self) -> bool {
        self.steps.iter().any(|s| matches!(s, Step::Loop { .. }))
    }

    /// Size of the evaluation schedule, for profiling.
    pub fn debug_stats(&self) -> String {
        let loops: Vec<u32> = self
            .steps
            .iter()
            .filter_map(|s| match s {
                Step::Loop { start, end, .. } => Some(end - start),
                _ => None,
            })
            .collect();
        format!(
            "{} gates, {} natives, {} steps, {} loops (gates in loops: {}, largest {})",
            self.gates.len(),
            self.natives.len(),
            self.steps.len(),
            loops.len(),
            loops.iter().sum::<u32>(),
            loops.iter().max().unwrap_or(&0)
        )
    }

    /// Simulate native parts in the first `lanes` lanes (1..=64; default 1).
    pub fn set_native_lanes(&mut self, lanes: u32) {
        let lanes = lanes.clamp(1, 64);
        self.native_lanes = lanes;
        for n in &mut self.natives {
            n.lanes.truncate(lanes as usize);
        }
    }

    /// The behaviour of the first native part named `name`, in lane 0, e.g.
    /// to inspect or load memory.
    pub fn native_mut<T: Behavior>(&mut self, name: &str) -> Option<&mut T> {
        let n = self.natives.iter_mut().find(|n| n.name == name)?;
        n.ensure(1);
        (n.lanes[0].as_mut() as &mut dyn std::any::Any).downcast_mut::<T>()
    }

    /// Set an input pin to `value` in every lane.
    pub fn set_input(&mut self, pin: usize, value: u64) {
        for (bit, &net) in self.inputs[pin].iter().enumerate() {
            self.values[net as usize] = if value >> bit & 1 == 1 { u64::MAX } else { 0 };
        }
    }

    /// Set one bit of an input pin, lane by lane.
    pub fn set_input_lanes(&mut self, pin: usize, bit: usize, lanes: u64) {
        let net = self.inputs[pin][bit];
        self.values[net as usize] = lanes;
    }

    /// Value of an output pin in lane 0.
    pub fn output(&self, pin: usize) -> u64 {
        read_bus(&self.values, &self.outputs[pin], 0)
    }

    /// One bit of an output pin, lane by lane.
    pub fn output_lanes(&self, pin: usize, bit: usize) -> u64 {
        self.values[self.outputs[pin][bit] as usize]
    }

    /// Value of an output pin in the given lane.
    pub fn output_in_lane(&self, pin: usize, lane: u32) -> u64 {
        read_bus(&self.values, &self.outputs[pin], lane)
    }

    /// Value on a top-level wire (by its index in [`ChipDef::wires`]), lane 0.
    pub fn wire(&self, wire: usize) -> u64 {
        read_bus(&self.values, &self.wire_nets[wire], 0)
    }

    /// Clear all stored state (latches, memories) to power-on values.
    pub fn reset(&mut self) {
        self.values.fill(0);
        for n in &mut self.natives {
            for b in &mut n.lanes {
                b.reset();
            }
        }
    }

    pub fn eval(&mut self) -> Result<(), Oscillation> {
        // A clocked native may have read its clock before the gates driving
        // it were evaluated (clock inputs are not ordering dependencies).
        // Re-check them after each pass and repeat until nothing changes.
        for _ in 0..8 {
            self.pass()?;
            let mut changed = 0u64;
            for &n in &self.stateful {
                changed |= self.natives[n as usize].eval(&mut self.values, self.native_lanes);
            }
            if changed == 0 {
                for &n in &self.stateful {
                    self.natives[n as usize].commit(&self.values, self.native_lanes);
                }
                return Ok(());
            }
        }
        Err(Oscillation)
    }

    fn pass(&mut self) -> Result<(), Oscillation> {
        let v = &mut self.values;
        let lanes = self.native_lanes;
        for step in &self.steps {
            match *step {
                Step::Gates { start, end } => {
                    for g in &self.gates[start as usize..end as usize] {
                        v[g.out as usize] = !(v[g.a as usize] & v[g.b as usize]);
                    }
                }
                Step::Native(i) => {
                    self.natives[i as usize].eval(v, lanes);
                }
                Step::Loop {
                    start,
                    end,
                    nstart,
                    nend,
                } => {
                    let natives = &self.order[nstart as usize..nend as usize];
                    if natives.is_empty() {
                        settle_gates(
                            v,
                            &self.gates,
                            start,
                            end,
                            &self.fanout_idx,
                            &self.fanout,
                            &mut self.dirty,
                        )?;
                        continue;
                    }
                    // Loops through native parts: evaluate everything until
                    // stable (these are small in practice).
                    let gates = &self.gates[start as usize..end as usize];
                    let mut settled = false;
                    for _ in 0..MAX_SETTLE_ROUNDS {
                        let mut changed = 0u64;
                        for g in gates {
                            let new = !(v[g.a as usize] & v[g.b as usize]);
                            changed |= new ^ v[g.out as usize];
                            v[g.out as usize] = new;
                        }
                        for &n in natives {
                            changed |= self.natives[n as usize].eval(v, lanes);
                        }
                        if changed == 0 {
                            settled = true;
                            break;
                        }
                    }
                    if !settled {
                        return Err(Oscillation);
                    }
                }
            }
        }
        Ok(())
    }

    /// Set every input, evaluate, and return every output (lane 0).
    pub fn eval_values(&mut self, inputs: &[u64]) -> Result<Vec<u64>, Oscillation> {
        assert_eq!(inputs.len(), self.inputs.len(), "wrong number of inputs");
        for (pin, &v) in inputs.iter().enumerate() {
            self.set_input(pin, v);
        }
        self.eval()?;
        Ok((0..self.outputs.len()).map(|p| self.output(p)).collect())
    }
}

/// Settle the gates `start..end` of a feedback loop.
///
/// Gates are stored in dependency order as far as a loop allows, so a sweep
/// in index order settles most of the loop at once. The first sweep visits
/// every gate; later sweeps only visit gates whose inputs changed ("dirty"),
/// still in index order. A loop can span a whole CPU (registers feed their
/// own inputs through the logic) while only a few latches move per clock,
/// so skipping clean gates matters.
fn settle_gates(
    v: &mut [u64],
    gates: &[Gate],
    start: u32,
    end: u32,
    fanout_idx: &[u32],
    fanout: &[u32],
    dirty: &mut [u64],
) -> Result<(), Oscillation> {
    let (start, end) = (start as usize, end as usize);
    let (w0, w1) = (start / 64, end.div_ceil(64));
    let mark = |dirty: &mut [u64], g: usize| {
        for &r in &fanout[fanout_idx[g] as usize..fanout_idx[g + 1] as usize] {
            dirty[r as usize / 64] |= 1 << (r % 64);
        }
    };
    for (gi, g) in gates.iter().enumerate().take(end).skip(start) {
        let new = !(v[g.a as usize] & v[g.b as usize]);
        if new != v[g.out as usize] {
            v[g.out as usize] = new;
            mark(dirty, gi);
        }
    }
    // Gates after the one that changed were swept with fresh inputs already;
    // their marks are harmless (a re-visit finds nothing changed). Keep
    // sweeping dirty gates in index order until none are left. A gate marked
    // during a sweep is visited in the same sweep if it lies ahead, else in
    // the next one.
    let budget = (end - start).max(64) * MAX_SETTLE_ROUNDS;
    let mut work = 0usize;
    loop {
        let mut any = false;
        for w in w0..w1 {
            loop {
                let bits = dirty[w];
                if bits == 0 {
                    break;
                }
                any = true;
                let bit = bits.trailing_zeros() as usize;
                dirty[w] &= !(1 << bit);
                let gi = w * 64 + bit;
                work += 1;
                let g = gates[gi];
                let new = !(v[g.a as usize] & v[g.b as usize]);
                if new != v[g.out as usize] {
                    v[g.out as usize] = new;
                    mark(dirty, gi);
                }
                if work > budget {
                    dirty[w0..w1].iter_mut().for_each(|w| *w = 0);
                    return Err(Oscillation);
                }
            }
        }
        if !any {
            return Ok(());
        }
    }
}

/// Union-find over nets. Wires merge the net of their source and sink.
#[derive(Default)]
struct UnionFind {
    parent: Vec<Net>,
}

impl UnionFind {
    fn make(&mut self) -> Net {
        let n = self.parent.len() as Net;
        self.parent.push(n);
        n
    }

    fn find(&mut self, mut n: Net) -> Net {
        while self.parent[n as usize] != n {
            let p = self.parent[n as usize];
            self.parent[n as usize] = self.parent[p as usize];
            n = p;
        }
        n
    }

    fn union(&mut self, a: Net, b: Net) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent[b as usize] = a;
        }
    }
}

struct PendingNative {
    name: String,
    def: NativeDef,
    inputs: Vec<Bus>,
    outputs: Vec<Bus>,
}

struct Flattener<'a> {
    lib: &'a Library,
    uf: UnionFind,
    gates: Vec<Gate>,
    natives: Vec<PendingNative>,
    /// Ground and power nets for constants, created on first use. Ground is
    /// never written so it stays 0; power is NAND(ground, ground).
    rails: Option<(Net, Net)>,
}

type Bus = Vec<Net>;

impl<'a> Flattener<'a> {
    fn fresh(&mut self, width: u8) -> Bus {
        (0..width).map(|_| self.uf.make()).collect()
    }

    /// Instantiate `def` with its inputs on `inputs`. Returns the nets of its
    /// outputs and the source nets of each of its wires.
    fn instantiate(
        &mut self,
        def: &ChipDef,
        inputs: &[Bus],
        stack: &mut Vec<String>,
    ) -> Result<(Vec<Bus>, Vec<Bus>), CompileError> {
        if stack.iter().any(|n| n == &def.name) {
            return Err(CompileError::Recursive {
                chip: def.name.clone(),
            });
        }
        stack.push(def.name.clone());

        let pins: Vec<PartPins> = def
            .parts
            .iter()
            .map(|p| {
                self.lib
                    .part_pins(p)
                    .ok_or_else(|| CompileError::UnknownPart {
                        chip: def.name.clone(),
                        part: p.kind.name(),
                    })
            })
            .collect::<Result<_, _>>()?;
        for p in &def.parts {
            let kind = self.lib.kind_pins(&p.kind).expect("checked above");
            let widest = kind
                .inputs
                .iter()
                .chain(&kind.outputs)
                .map(|p| p.width as u32)
                .max();
            if widest.unwrap_or(0) * p.repeat.max(1) as u32 > MAX_WIDTH as u32 {
                return Err(CompileError::TooWide {
                    chip: def.name.clone(),
                    part: p.kind.name(),
                });
            }
        }
        check_wires(def, &pins)?;

        let part_in: Vec<Vec<Bus>> = pins
            .iter()
            .map(|p| p.inputs.iter().map(|pin| self.fresh(pin.width)).collect())
            .collect();
        let outputs: Vec<Bus> = def.outputs.iter().map(|p| self.fresh(p.width)).collect();

        let mut part_out: Vec<Vec<Bus>> = Vec::with_capacity(def.parts.len());
        for (idx, part) in def.parts.iter().enumerate() {
            let copies = part.repeat.max(1) as usize;
            let mut outs: Vec<Bus> = vec![Vec::new(); pins[idx].outputs.len()];
            for k in 0..copies {
                let ins: Vec<Bus> = part_in[idx]
                    .iter()
                    .map(|bus| slice(bus, k, copies))
                    .collect();
                for (pin, bus) in self.place(&part.kind, &ins, stack)?.into_iter().enumerate() {
                    outs[pin].extend(bus);
                }
            }
            part_out.push(outs);
        }

        let mut wire_nets = Vec::with_capacity(def.wires.len());
        for w in &def.wires {
            let from = match w.from {
                Source::Input(i) => &inputs[i],
                Source::Part { part, pin } => &part_out[part][pin],
            };
            let to = match w.to {
                Sink::Output(i) => &outputs[i],
                Sink::Part { part, pin } => &part_in[part][pin],
            };
            for (bit, &t) in to.iter().enumerate() {
                // A 1-bit source drives every line of a wider sink.
                let f = if from.len() == 1 { from[0] } else { from[bit] };
                self.uf.union(f, t);
            }
            wire_nets.push(from.clone());
        }

        stack.pop();
        Ok((outputs, wire_nets))
    }

    /// One copy of a part. Returns its output buses.
    fn place(
        &mut self,
        kind: &PartKind,
        ins: &[Bus],
        stack: &mut Vec<String>,
    ) -> Result<Vec<Bus>, CompileError> {
        Ok(match kind {
            PartKind::Nand => {
                let out = self.uf.make();
                self.gates.push(Gate {
                    a: ins[0][0],
                    b: ins[1][0],
                    out,
                });
                vec![vec![out]]
            }
            PartKind::Chip(name) => {
                if let Some(n) = self.lib.native_for(name) {
                    let outputs: Vec<Bus> = n.outputs.iter().map(|p| self.fresh(p.width)).collect();
                    self.natives.push(PendingNative {
                        name: name.clone(),
                        def: n.clone(),
                        inputs: ins.to_vec(),
                        outputs: outputs.clone(),
                    });
                    return Ok(outputs);
                }
                let sub = self.lib.get(name).expect("checked by part_pins");
                self.instantiate(sub, ins, stack)?.0
            }
            PartKind::Split(widths) => {
                let mut rest = &ins[0][..];
                widths
                    .iter()
                    .map(|&w| {
                        let (group, tail) = rest.split_at(w as usize);
                        rest = tail;
                        group.to_vec()
                    })
                    .collect()
            }
            PartKind::Join(_) => vec![ins.concat()],
            PartKind::Const { width, value } => {
                let (gnd, vcc) = self.rails();
                vec![(0..*width)
                    .map(|b| if value >> b & 1 == 1 { vcc } else { gnd })
                    .collect()]
            }
        })
    }

    fn rails(&mut self) -> (Net, Net) {
        *self.rails.get_or_insert_with(|| {
            let gnd = self.uf.make();
            let vcc = self.uf.make();
            self.gates.push(Gate {
                a: gnd,
                b: gnd,
                out: vcc,
            });
            (gnd, vcc)
        })
    }

    fn finish(mut self, inputs: Vec<Bus>, outputs: Vec<Bus>, wire_nets: Vec<Bus>) -> Circuit {
        // Renumber the union-find roots densely.
        let mut dense = vec![Net::MAX; self.uf.parent.len()];
        let mut next: Net = 0;
        let uf = &mut self.uf;
        let mut map = |n: Net| -> Net {
            let r = uf.find(n) as usize;
            if dense[r] == Net::MAX {
                dense[r] = next;
                next += 1;
            }
            dense[r]
        };
        let gates: Vec<Gate> = self
            .gates
            .iter()
            .map(|g| Gate {
                a: map(g.a),
                b: map(g.b),
                out: map(g.out),
            })
            .collect();
        let mut map_bus = |b: &Bus| b.iter().map(|&n| map(n)).collect::<Bus>();
        let natives: Vec<NativeNode> = self
            .natives
            .iter()
            .map(|p| NativeNode {
                name: p.name.clone(),
                inputs: p.inputs.iter().map(&mut map_bus).collect(),
                outputs: p.outputs.iter().map(&mut map_bus).collect(),
                lanes: Vec::new(),
                make: p.def.make.clone(),
                ins: vec![0; p.inputs.len()],
                outs: vec![0; p.outputs.len()],
            })
            .collect();
        let inputs: Vec<Bus> = inputs.iter().map(&mut map_bus).collect();
        let outputs: Vec<Bus> = outputs.iter().map(&mut map_bus).collect();
        let wire_nets: Vec<Bus> = wire_nets.iter().map(&mut map_bus).collect();
        let net_count = next as usize;

        let comb: Vec<Vec<bool>> = self.natives.iter().map(|p| p.def.comb.clone()).collect();
        let (gates, order, steps) = schedule(&gates, &natives, &comb, net_count);
        let (fanout_idx, fanout) = loop_fanout(&gates, &steps, net_count);
        let stateful = (0..natives.len() as u32)
            .filter(|&i| self.natives[i as usize].def.stateful)
            .collect();
        let dirty = vec![0u64; gates.len().div_ceil(64) + 1];
        Circuit {
            values: vec![0; net_count],
            gates,
            natives,
            order,
            steps,
            stateful,
            dirty,
            fanout_idx,
            fanout,
            native_lanes: 1,
            inputs,
            outputs,
            wire_nets,
        }
    }
}

/// Readers of each loop gate within its own loop, in CSR form.
fn loop_fanout(gates: &[Gate], steps: &[Step], net_count: usize) -> (Vec<u32>, Vec<u32>) {
    let mut lists: Vec<Vec<u32>> = vec![Vec::new(); gates.len()];
    let mut driver = vec![u32::MAX; net_count];
    for step in steps {
        if let Step::Loop { start, end, .. } = *step {
            for gi in start..end {
                driver[gates[gi as usize].out as usize] = gi;
            }
            for gi in start..end {
                let g = gates[gi as usize];
                for net in [g.a, g.b] {
                    let d = driver[net as usize];
                    if d != u32::MAX && d >= start && d < end {
                        let l = &mut lists[d as usize];
                        if l.last() != Some(&gi) {
                            l.push(gi);
                        }
                    }
                }
            }
        }
    }
    let mut idx = Vec::with_capacity(gates.len() + 1);
    let mut flat = Vec::new();
    idx.push(0);
    for l in lists {
        flat.extend(l);
        idx.push(flat.len() as u32);
    }
    (idx, flat)
}

/// Copy `k` of `copies`' share of a bus.
fn slice(bus: &[Net], k: usize, copies: usize) -> Bus {
    let w = bus.len() / copies;
    bus[k * w..(k + 1) * w].to_vec()
}

fn check_wires(def: &ChipDef, pins: &[PartPins]) -> Result<(), CompileError> {
    let bad_pin = |pin: String| CompileError::PinOutOfRange {
        chip: def.name.clone(),
        pin,
    };
    let mut driven_outputs = vec![false; def.outputs.len()];
    let mut driven_parts: Vec<Vec<bool>> =
        pins.iter().map(|p| vec![false; p.inputs.len()]).collect();

    for w in &def.wires {
        let from: &Pin = match w.from {
            Source::Input(i) => def
                .inputs
                .get(i)
                .ok_or_else(|| bad_pin(trf!("giriş #{i}", "input #{i}")))?,
            Source::Part { part, pin } => pins
                .get(part)
                .and_then(|p| p.outputs.get(pin))
                .ok_or_else(|| {
                    bad_pin(trf!(
                        "parça #{part} çıkış #{pin}",
                        "part #{part} output #{pin}"
                    ))
                })?,
        };
        let (slot, to, label) = match w.to {
            Sink::Output(i) => {
                let slot = driven_outputs
                    .get_mut(i)
                    .ok_or_else(|| bad_pin(trf!("çıkış #{i}", "output #{i}")))?;
                (
                    slot,
                    &def.outputs[i],
                    trf!("çıkış {}", "output {}", def.outputs[i].name),
                )
            }
            Sink::Part { part, pin } => {
                let slot = driven_parts
                    .get_mut(part)
                    .and_then(|p| p.get_mut(pin))
                    .ok_or_else(|| {
                        bad_pin(trf!(
                            "parça #{part} giriş #{pin}",
                            "part #{part} input #{pin}"
                        ))
                    })?;
                let to = &pins[part].inputs[pin];
                (
                    slot,
                    to,
                    trf!(
                        "{} parçasının {} girişi",
                        "input {1} of the {0} part",
                        def.parts[part].kind.name(),
                        to.name
                    ),
                )
            }
        };
        if from.width != to.width && from.width != 1 {
            return Err(CompileError::WidthMismatch {
                chip: def.name.clone(),
                sink: label,
                from: from.width,
                to: to.width,
            });
        }
        if *slot {
            return Err(CompileError::MultipleDrivers {
                chip: def.name.clone(),
                sink: label,
            });
        }
        *slot = true;
    }
    Ok(())
}

/// Order gates and natives by dependency. Nodes that depend on each other in
/// a loop form one strongly connected component (SCC), which becomes a
/// [`Step::Loop`].
///
/// Node ids: gates are `0..gates.len()`, natives follow.
fn schedule(
    gates: &[Gate],
    natives: &[NativeNode],
    comb: &[Vec<bool>],
    net_count: usize,
) -> (Vec<Gate>, Vec<u32>, Vec<Step>) {
    let ng = gates.len();
    let n = ng + natives.len();
    let mut driver = vec![u32::MAX; net_count];
    for (i, g) in gates.iter().enumerate() {
        driver[g.out as usize] = i as u32;
    }
    for (i, nat) in natives.iter().enumerate() {
        for bus in &nat.outputs {
            for &net in bus {
                driver[net as usize] = (ng + i) as u32;
            }
        }
    }
    // deps[v]: the nodes v reads from (combinationally).
    let mut deps: Vec<Vec<u32>> = Vec::with_capacity(n);
    for g in gates {
        deps.push(
            [driver[g.a as usize], driver[g.b as usize]]
                .into_iter()
                .filter(|&d| d != u32::MAX)
                .collect(),
        );
    }
    for (i, nat) in natives.iter().enumerate() {
        let mut d: Vec<u32> = nat
            .inputs
            .iter()
            .zip(&comb[i])
            .filter(|(_, &c)| c)
            .flat_map(|(bus, _)| bus.iter().map(|&net| driver[net as usize]))
            .filter(|&d| d != u32::MAX)
            .collect();
        d.sort_unstable();
        d.dedup();
        deps.push(d);
    }

    // Tarjan's algorithm, iterative. SCCs come out dependencies first.
    const UNVISITED: u32 = u32::MAX;
    let mut index = vec![UNVISITED; n];
    let mut low = vec![0u32; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<u32> = Vec::new();
    let mut call: Vec<(u32, u32)> = Vec::new();
    let mut counter = 0u32;
    // DFS finish order. Edges point to dependencies, so within a loop this
    // puts most dependencies first and the loop settles in a few rounds.
    let mut finish = vec![0u32; n];
    let mut finished = 0u32;

    let mut ordered: Vec<Gate> = Vec::with_capacity(ng);
    let mut order: Vec<u32> = Vec::with_capacity(natives.len());
    let mut steps: Vec<Step> = Vec::new();
    let mut emit = |comp: &[u32], ordered: &mut Vec<Gate>, order: &mut Vec<u32>| {
        let v0 = comp[0] as usize;
        let looped = comp.len() > 1 || deps[v0].contains(&(v0 as u32));
        if !looped {
            if v0 < ng {
                ordered.push(gates[v0]);
                let end = ordered.len() as u32;
                match steps.last_mut() {
                    Some(Step::Gates { end: e, .. }) if *e == end - 1 => *e = end,
                    _ => steps.push(Step::Gates {
                        start: end - 1,
                        end,
                    }),
                }
            } else {
                order.push((v0 - ng) as u32);
                steps.push(Step::Native((v0 - ng) as u32));
            }
            return;
        }
        let start = ordered.len() as u32;
        let nstart = order.len() as u32;
        for &v in comp {
            let v = v as usize;
            if v < ng {
                ordered.push(gates[v]);
            } else {
                order.push((v - ng) as u32);
            }
        }
        steps.push(Step::Loop {
            start,
            end: ordered.len() as u32,
            nstart,
            nend: order.len() as u32,
        });
    };

    for root in 0..n as u32 {
        if index[root as usize] != UNVISITED {
            continue;
        }
        call.push((root, 0));
        while let Some(&mut (v, ref mut edge)) = call.last_mut() {
            let vi = v as usize;
            if *edge == 0 && index[vi] == UNVISITED {
                index[vi] = counter;
                low[vi] = counter;
                counter += 1;
                stack.push(v);
                on_stack[vi] = true;
            }
            if (*edge as usize) < deps[vi].len() {
                let w = deps[vi][*edge as usize];
                *edge += 1;
                let wi = w as usize;
                if index[wi] == UNVISITED {
                    call.push((w, 0));
                } else if on_stack[wi] {
                    low[vi] = low[vi].min(index[wi]);
                }
                continue;
            }
            call.pop();
            finish[vi] = finished;
            finished += 1;
            if let Some(&(parent, _)) = call.last() {
                low[parent as usize] = low[parent as usize].min(low[vi]);
            }
            if low[vi] == index[vi] {
                let mut comp = Vec::new();
                loop {
                    let w = stack.pop().unwrap();
                    on_stack[w as usize] = false;
                    comp.push(w);
                    if w == v {
                        break;
                    }
                }
                comp.sort_unstable_by_key(|&w| finish[w as usize]);
                emit(&comp, &mut ordered, &mut order);
            }
        }
    }
    (ordered, order, steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chip::{into, out, ChipBuilder, Part, Wire};

    fn not_chip() -> ChipDef {
        let mut b = ChipBuilder::new("Not", &["in"], &["out"]);
        let x = b.input("in");
        let n = b.nand(x, x);
        b.output("out", n);
        b.build()
    }

    #[test]
    fn nand_lanes() {
        let mut b = ChipBuilder::new("N", &["a", "b"], &["out"]);
        let (x, y) = (b.input("a"), b.input("b"));
        let n = b.nand(x, y);
        b.output("out", n);
        let mut c = Circuit::compile_def(&Library::new(), &b.build()).unwrap();
        // lanes: a = 0011, b = 0101
        c.set_input_lanes(0, 0, 0b1100);
        c.set_input_lanes(1, 0, 0b1010);
        c.eval().unwrap();
        assert_eq!(c.output_lanes(0, 0) & 0xF, 0b0111);
    }

    #[test]
    fn hierarchy_flattens() {
        let mut lib = Library::new();
        lib.insert(not_chip());
        let mut b = ChipBuilder::new("NotNot", &["in"], &["out"]);
        let x = b.input("in");
        let y = b.chip1("Not", &[x]);
        let z = b.chip1("Not", &[y]);
        b.output("out", z);
        lib.insert(b.build());
        let mut c = Circuit::compile(&lib, "NotNot").unwrap();
        assert_eq!(c.gate_count(), 2);
        assert!(!c.has_feedback());
        assert_eq!(c.eval_values(&[1]).unwrap(), vec![1]);
        assert_eq!(c.eval_values(&[0]).unwrap(), vec![0]);
    }

    #[test]
    fn repeat_split_join() {
        let mut lib = Library::new();
        lib.insert(not_chip());
        // out = !in with the two nibbles swapped.
        let mut b = ChipBuilder::new("T", &["in[8]"], &["out[8]"]);
        let x = b.input("in");
        let n = b.chip_n("Not", 8, &[x]);
        let s = b.split(&[4, 4], out(n, 0));
        let j = b.join(&[4, 4], &[out(s, 1), out(s, 0)]);
        b.output("out", j);
        let mut c = Circuit::compile_def(&lib, &b.build()).unwrap();
        assert_eq!(c.gate_count(), 8);
        assert_eq!(c.eval_values(&[0x1E]).unwrap(), vec![0x1E]);
        assert_eq!(c.eval_values(&[0x0F]).unwrap(), vec![0x0F]);
        assert_eq!(c.eval_values(&[0xA3]).unwrap(), vec![0xC5]);
    }

    #[test]
    fn one_bit_broadcasts() {
        let mut b = ChipBuilder::new("T", &["a"], &["out[4]"]);
        let a = b.input("a");
        b.output("out", a);
        let mut c = Circuit::compile_def(&Library::new(), &b.build()).unwrap();
        assert_eq!(c.eval_values(&[1]).unwrap(), vec![0xF]);
    }

    #[test]
    fn constants() {
        let mut b = ChipBuilder::new("T", &[], &["out[8]"]);
        let k = b.constant(8, 0xA5);
        b.output("out", k);
        let mut c = Circuit::compile_def(&Library::new(), &b.build()).unwrap();
        assert_eq!(c.eval_values(&[]).unwrap(), vec![0xA5]);
        c.reset();
        assert_eq!(c.eval_values(&[]).unwrap(), vec![0xA5]);
    }

    #[test]
    fn unconnected_inputs_read_zero() {
        let mut d = ChipDef::new("Loose", &[], &["out"]);
        d.parts.push(Part::new(PartKind::Nand));
        d.wires.push(Wire {
            from: out(0, 0),
            to: Sink::Output(0),
        });
        let mut c = Circuit::compile_def(&Library::new(), &d).unwrap();
        assert_eq!(c.eval_values(&[]).unwrap(), vec![1]);
    }

    #[test]
    fn errors() {
        let lib = Library::new();
        let mut d = ChipDef::new("X", &["a"], &["out"]);
        d.wires.push(Wire {
            from: Source::Input(0),
            to: Sink::Output(0),
        });
        d.wires.push(Wire {
            from: Source::Input(0),
            to: Sink::Output(0),
        });
        assert!(matches!(
            Circuit::compile_def(&lib, &d),
            Err(CompileError::MultipleDrivers { .. })
        ));

        let mut d = ChipDef::new("X", &["a[2]"], &["out[3]"]);
        d.wires.push(Wire {
            from: Source::Input(0),
            to: Sink::Output(0),
        });
        assert!(matches!(
            Circuit::compile_def(&lib, &d),
            Err(CompileError::WidthMismatch { .. })
        ));

        let mut d = ChipDef::new("X", &[], &[]);
        d.parts.push(Part::new(PartKind::Chip("Missing".into())));
        assert!(matches!(
            Circuit::compile_def(&lib, &d),
            Err(CompileError::UnknownPart { .. })
        ));

        let mut lib = Library::new();
        let mut d = ChipDef::new("Self", &[], &[]);
        d.parts.push(Part::new(PartKind::Chip("Self".into())));
        lib.insert(d);
        assert!(matches!(
            Circuit::compile(&lib, "Self"),
            Err(CompileError::Recursive { .. })
        ));
    }

    #[test]
    fn ring_oscillator_detected() {
        // A NAND whose output feeds both its inputs never settles.
        let mut d = ChipDef::new("Ring", &[], &["out"]);
        d.parts.push(Part::new(PartKind::Nand));
        d.wires.push(Wire {
            from: out(0, 0),
            to: into(0, 0),
        });
        d.wires.push(Wire {
            from: out(0, 0),
            to: into(0, 1),
        });
        d.wires.push(Wire {
            from: out(0, 0),
            to: Sink::Output(0),
        });
        let mut c = Circuit::compile_def(&Library::new(), &d).unwrap();
        assert!(c.has_feedback());
        assert_eq!(c.eval(), Err(Oscillation));
    }

    #[test]
    fn only_loops_are_looped() {
        // NOT -> SR latch -> NOT: the latch gates loop, the others don't.
        let mut b = ChipBuilder::new("T", &["s", "r"], &["q"]);
        let (s, r) = (b.input("s"), b.input("r"));
        let ns = b.nand(s, s);
        let nr = b.nand(r, r);
        let q = b.part(Part::new(PartKind::Nand), &[ns]);
        let qn = b.nand(nr, out(q, 0));
        b.wire(qn, into(q, 1));
        let nq = b.nand(out(q, 0), out(q, 0));
        let o = b.nand(nq, nq);
        b.output("q", o);
        let mut c = Circuit::compile_def(&Library::new(), &b.build()).unwrap();
        let looped: Vec<u32> = c
            .steps
            .iter()
            .filter_map(|s| match s {
                Step::Loop { start, end, .. } => Some(end - start),
                _ => None,
            })
            .collect();
        assert_eq!(looped, vec![2]);
        assert_eq!(c.eval_values(&[1, 0]).unwrap(), vec![1]);
        assert_eq!(c.eval_values(&[0, 0]).unwrap(), vec![1]);
        assert_eq!(c.eval_values(&[0, 1]).unwrap(), vec![0]);
        assert_eq!(c.eval_values(&[0, 0]).unwrap(), vec![0]);
    }
}

/// How deep the logic is: the most NAND gates a change at an input (or a
/// latch's output) passes through before it reaches an output or a latch.
/// This decides how fast a chip can be clocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Depth {
    /// Gates on the longest path.
    pub gates: usize,
}

impl Circuit {
    /// The longest chain of gates in the circuit. Gates in feedback loops
    /// (latches) start and end paths: a loop is storage, not a path. Native
    /// parts count as no gates.
    pub fn depth(&self) -> Depth {
        // Arrival depth of each net: the most gates before it.
        let mut at = vec![0usize; self.values.len()];
        let mut longest = 0;
        for step in &self.steps {
            match *step {
                Step::Gates { start, end } => {
                    for g in &self.gates[start as usize..end as usize] {
                        let d = at[g.a as usize].max(at[g.b as usize]) + 1;
                        at[g.out as usize] = d;
                        longest = longest.max(d);
                    }
                }
                Step::Loop { start, end, .. } => {
                    // Paths end at the loop's inputs; its outputs start anew.
                    for g in &self.gates[start as usize..end as usize] {
                        longest = longest.max(at[g.a as usize]).max(at[g.b as usize]);
                    }
                    for g in &self.gates[start as usize..end as usize] {
                        at[g.out as usize] = 1;
                    }
                }
                Step::Native(i) => {
                    let n = &self.natives[i as usize];
                    let d = n
                        .inputs
                        .iter()
                        .flatten()
                        .map(|&x| at[x as usize])
                        .max()
                        .unwrap_or(0);
                    for &o in n.outputs.iter().flatten() {
                        at[o as usize] = d;
                    }
                }
            }
        }
        Depth { gates: longest }
    }
}

#[cfg(test)]
mod depth_tests {
    use super::*;
    use crate::chip::{ChipBuilder, Library};

    #[test]
    fn depth_counts_the_longest_chain() {
        let mut b = ChipBuilder::new("T", &["a", "b"], &["x", "y"]);
        let (a, bb) = (b.input("a"), b.input("b"));
        let n1 = b.nand(a, bb);
        let n2 = b.nand(n1, n1);
        let n3 = b.nand(n2, a);
        b.output("x", n3);
        b.output("y", n1);
        let c = Circuit::compile_def(&Library::new(), &b.build()).unwrap();
        assert_eq!(c.depth().gates, 3);
    }

    #[test]
    fn latches_cut_paths() {
        // An SR latch fed by one gate on each side, read through one gate.
        let mut b = ChipBuilder::new("T", &["s", "r"], &["q"]);
        let (s, r) = (b.input("s"), b.input("r"));
        let ns = b.nand(s, s);
        let nr = b.nand(r, r);
        let q = b.part(crate::chip::Part::new(crate::chip::PartKind::Nand), &[ns]);
        let nq = b.part(crate::chip::Part::new(crate::chip::PartKind::Nand), &[nr]);
        b.wire(crate::chip::out(q, 0), crate::chip::into(nq, 1));
        b.wire(crate::chip::out(nq, 0), crate::chip::into(q, 1));
        let o = b.nand(crate::chip::out(q, 0), crate::chip::out(q, 0));
        b.output("q", o);
        let c = Circuit::compile_def(&Library::new(), &b.build()).unwrap();
        assert!(c.has_feedback());
        // In: NOT then the loop (1); out: loop gate (1) then NOT (2).
        assert_eq!(c.depth().gates, 2);
    }
}
