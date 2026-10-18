//! Turning a world into a circuit.
//!
//! Gates become NAND parts, devices native parts, library parts stay what
//! they are. Switches, clocks and inputs become the chip's inputs. Every
//! output pin that has a wire gets a chip output too (a *probe*), so the
//! view can show the value on any wire.
//!
//! [`compile`] builds the circuit the world view runs; [`crate::chip`]
//! builds a chip the player keeps, from the same parts.

use std::collections::HashMap;
use std::fmt;

use makine_sim::{ChipDef, Library, NativeDef, Part, PartKind, PartPins, Pin, Sink, Source, Wire};

use crate::part::{Gate, Kind};
use crate::world::{End, Id, World};

/// Where a chip input's value comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputSource {
    Switch,
    Clock { period: u32 },
    Port,
}

/// Why a part or wire was left out of the circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// A chip that is not in the library.
    MissingChip { part: Id, name: String },
    /// A wire between pins of different widths (other than one bit fanned
    /// out to every bit).
    Width { to: End, from: u8, into: u8 },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::MissingChip { name, .. } => f.write_str(&makine_lang::trf!(
                "'{name}' çipi kütüphanende yok",
                "chip '{name}' is not in your library"
            )),
            Problem::Width { from, into, .. } => f.write_str(&makine_lang::trf!(
                "{from} bitlik bir tel {into} bitlik bir uca bağlı",
                "a {from}-bit wire is connected to a {into}-bit pin"
            )),
        }
    }
}

/// The circuit of a world, plus how to find each wire's value in it.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub def: ChipDef,
    /// Chip inputs in order: the part setting each one.
    pub inputs: Vec<(Id, InputSource)>,
    /// The chip output showing each wired output pin.
    pub probes: HashMap<End, usize>,
    /// Native device parts: part id and native name.
    pub devices: Vec<(Id, String)>,
    /// Native parts the circuit uses, see [`Compiled::library_with`].
    pub natives: Vec<(String, NativeDef)>,
    /// Parts and wires left out.
    pub problems: Vec<Problem>,
}

impl Compiled {
    /// `base` (the player's chips) plus the devices' native parts.
    pub fn library_with(&self, base: &Library) -> Library {
        let mut lib = base.clone();
        for (name, n) in &self.natives {
            lib.insert_native(name.clone(), n.clone());
        }
        lib
    }

    /// The chip output carrying output pin `end`, if it is wired.
    pub fn probe(&self, end: End) -> Option<usize> {
        self.probes.get(&end).copied()
    }

    /// The native name of device part `id`.
    pub fn native(&self, id: Id) -> Option<&str> {
        self.devices
            .iter()
            .find(|(d, _)| *d == id)
            .map(|(_, n)| n.as_str())
    }
}

/// How the parts of a world are built.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// To run in the world view: switches, clocks and inputs are chip
    /// inputs, every wire is probed.
    Run,
    /// To keep as a chip: input parts are its inputs, output parts its
    /// outputs, switches constants.
    Chip,
}

/// What a part became: the sinks each input pin feeds and the source of
/// each output pin.
#[derive(Default)]
struct Built {
    inputs: Vec<Vec<Sink>>,
    outputs: Vec<Source>,
    pins: Option<PartPins>,
}

pub(crate) struct Builder<'a> {
    pub def: ChipDef,
    lib: &'a Library,
    mode: Mode,
    built: HashMap<Id, Built>,
    pub inputs: Vec<(Id, InputSource)>,
    pub devices: Vec<(Id, String)>,
    pub natives: Vec<(String, NativeDef)>,
    pub problems: Vec<Problem>,
}

impl<'a> Builder<'a> {
    pub fn new(name: &str, lib: &'a Library, mode: Mode) -> Self {
        Self {
            def: ChipDef::new(name, &[], &[]),
            lib,
            mode,
            built: HashMap::new(),
            inputs: Vec::new(),
            devices: Vec::new(),
            natives: Vec::new(),
            problems: Vec::new(),
        }
    }

    fn add_part(&mut self, part: Part) -> usize {
        self.def.parts.push(part);
        self.def.parts.len() - 1
    }

    fn chip_input(&mut self, id: Id, pin: Pin, src: InputSource) -> Source {
        self.def.inputs.push(pin);
        self.inputs.push((id, src));
        Source::Input(self.def.inputs.len() - 1)
    }

    fn constant(&mut self, width: u8, value: u64) -> Source {
        let part = self.add_part(Part::new(PartKind::Const { width, value }));
        Source::Part { part, pin: 0 }
    }

    /// Add part `id` to the circuit.
    pub fn part(&mut self, id: Id, kind: &Kind) {
        let pins = kind.pins(self.lib);
        if pins.is_none() {
            if let Some(name) = kind.chip_name() {
                self.problems.push(Problem::MissingChip {
                    part: id,
                    name: name.into(),
                });
            }
            return;
        }
        let one = |name: &str| Pin::new(name, 1);
        let mut b = Built {
            pins,
            ..Default::default()
        };
        match kind {
            Kind::Gate(g) => {
                let mut ins = Vec::new();
                let out = gate_parts(&mut self.def, *g, &mut ins);
                b.inputs = vec![Vec::new(); g.inputs()];
                for (input, sink) in ins {
                    b.inputs[input].push(sink);
                }
                b.outputs = vec![out];
            }
            Kind::Switch { on } => {
                let src = match self.mode {
                    Mode::Run => self.chip_input(id, one(&format!("sw{id}")), InputSource::Switch),
                    Mode::Chip => self.constant(1, *on as u64),
                };
                b.outputs = vec![src];
            }
            Kind::Clock { period } => {
                // Rejected before building a chip.
                let src = self.chip_input(
                    id,
                    one(&format!("clk{id}")),
                    InputSource::Clock { period: *period },
                );
                b.outputs = vec![src];
            }
            Kind::Input { name, width, .. } => {
                let pin_name = match self.mode {
                    Mode::Run => format!("in{id}"),
                    Mode::Chip => name.clone(),
                };
                let src = self.chip_input(id, Pin::new(pin_name, *width), InputSource::Port);
                b.outputs = vec![src];
            }
            Kind::Lamp | Kind::Output { .. } => {
                // They read the wire into them; nothing to build.
                b.inputs = vec![Vec::new()];
            }
            Kind::Device(d) => match d.native() {
                Some(n) => {
                    let name = d.native_name(id);
                    let part = self.add_part(Part::new(PartKind::Chip(name.clone())));
                    b.inputs = (0..n.inputs.len())
                        .map(|pin| vec![Sink::Part { part, pin }])
                        .collect();
                    b.outputs = (0..n.outputs.len())
                        .map(|pin| Source::Part { part, pin })
                        .collect();
                    self.natives.push((name.clone(), n));
                    self.devices.push((id, name));
                }
                // A display only reads its input.
                None => b.inputs = vec![Vec::new()],
            },
            Kind::Part(p) => {
                let part = self.add_part(p.clone());
                let pins = b.pins.as_ref().unwrap();
                b.inputs = (0..pins.inputs.len())
                    .map(|pin| vec![Sink::Part { part, pin }])
                    .collect();
                b.outputs = (0..pins.outputs.len())
                    .map(|pin| Source::Part { part, pin })
                    .collect();
            }
        }
        self.built.insert(id, b);
    }

    /// The source of output pin `end`, and its width.
    pub fn source(&self, end: End) -> Option<(Source, u8)> {
        let b = self.built.get(&end.part)?;
        let src = *b.outputs.get(end.pin)?;
        let width = b.pins.as_ref()?.outputs.get(end.pin)?.width;
        Some((src, width))
    }

    fn input_width(&self, end: End) -> Option<u8> {
        let b = self.built.get(&end.part)?;
        Some(b.pins.as_ref()?.inputs.get(end.pin)?.width)
    }

    /// Wire every input from its wire's source. Returns the wires that
    /// connect (from, to).
    pub fn wires(&mut self, world: &World) -> Vec<(End, End)> {
        let mut ok = Vec::new();
        for (from, to) in world.wires() {
            let (Some((src, fw)), Some(tw)) = (self.source(from), self.input_width(to)) else {
                continue;
            };
            if !fits(fw, tw) {
                self.problems.push(Problem::Width {
                    to,
                    from: fw,
                    into: tw,
                });
                continue;
            }
            for &sink in &self.built[&to.part].inputs[to.pin] {
                self.def.wires.push(Wire {
                    from: src,
                    to: sink,
                });
            }
            ok.push((from, to));
        }
        ok
    }
}

/// Whether an output `from` bits wide may drive an input `into` bits wide:
/// the same width, one bit fanned out to every bit, or any width into a
/// lamp (stored as 0 bits wide: it reads any wire).
pub fn fits(from: u8, into: u8) -> bool {
    from == into || from == 1 || into == 0
}

/// The circuit the world view runs. `lib` holds the player's chips.
pub fn compile(world: &World, lib: &Library) -> Compiled {
    let mut b = Builder::new(&world.name, lib, Mode::Run);
    for (id, p) in world.parts() {
        b.part(id, &p.kind);
    }
    b.wires(world);
    // Probe every output, including one not wired yet. Inspection must not
    // require an artificial lamp or dummy wire.
    let ends: Vec<_> = b
        .built
        .iter()
        .flat_map(|(id, p)| (0..p.outputs.len()).map(|pin| End::new(*id, pin)))
        .collect();
    let mut probes = HashMap::new();
    for from in ends {
        if probes.contains_key(&from) {
            continue;
        }
        let (src, width) = b.source(from).expect("wired above");
        let o = b.def.outputs.len();
        b.def
            .outputs
            .push(Pin::new(format!("p{}_{}", from.part, from.pin), width));
        b.def.wires.push(Wire {
            from: src,
            to: Sink::Output(o),
        });
        probes.insert(from, o);
    }
    Compiled {
        def: b.def,
        inputs: b.inputs,
        probes,
        devices: b.devices,
        natives: b.natives,
        problems: b.problems,
    }
}

/// Add NAND parts computing `gate`. Returns the output; the parts' external
/// inputs are recorded in `ins` as (gate input, sink).
fn gate_parts(def: &mut ChipDef, gate: Gate, ins: &mut Vec<(usize, Sink)>) -> Source {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Device;
    use crate::world::{Edit, Placed, Pos};
    use makine_sim::Circuit;

    pub(crate) fn put(w: &mut World, kind: Kind) -> Id {
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

    pub(crate) fn wire(w: &mut World, from: (Id, usize), to: (Id, usize)) {
        w.apply(Edit::Wire {
            to: End::new(to.0, to.1),
            from: Some(End::new(from.0, from.1)),
        });
    }

    fn run(w: &World, lib: &Library, inputs: &[u64]) -> (Compiled, Circuit) {
        let c = compile(w, lib);
        let mut sim = Circuit::compile_def(&c.library_with(lib), &c.def).unwrap();
        for (i, v) in inputs.iter().enumerate() {
            sim.set_input(i, *v);
        }
        sim.eval().unwrap();
        (c, sim)
    }

    #[test]
    fn gates_follow_their_truth_tables() {
        let lib = Library::new();
        for g in Gate::ALL {
            let mut w = World::new("g");
            let a = put(&mut w, Kind::Switch { on: false });
            let b = put(&mut w, Kind::Switch { on: false });
            let gate = put(&mut w, Kind::Gate(g));
            let lamp = put(&mut w, Kind::Lamp);
            wire(&mut w, (a, 0), (gate, 0));
            if g.inputs() == 2 {
                wire(&mut w, (b, 0), (gate, 1));
            }
            wire(&mut w, (gate, 0), (lamp, 0));
            for v in 0..4u64 {
                let (x, y) = (v & 1, v >> 1);
                // Inputs are in id order; find which is which.
                let c = compile(&w, &lib);
                let ins: Vec<u64> = c
                    .inputs
                    .iter()
                    .map(|(id, _)| if *id == a { x } else { y })
                    .collect();
                let (c, sim) = run(&w, &lib, &ins);
                let got = sim.output(c.probe(End::new(gate, 0)).unwrap());
                let (x, y) = (x == 1, y == 1);
                let want = match g {
                    Gate::Not => !x,
                    Gate::And => x & y,
                    Gate::Or => x | y,
                    Gate::Xor => x ^ y,
                    Gate::Nand => !(x & y),
                    Gate::Nor => !(x | y),
                };
                assert_eq!(got, want as u64, "{g:?} {x} {y}");
            }
        }
    }

    #[test]
    fn buses_and_problems() {
        let lib = Library::new();
        let mut w = World::new("b");
        let i = put(
            &mut w,
            Kind::Input {
                name: "a".into(),
                width: 4,
                value: 0,
            },
        );
        let not = put(&mut w, Kind::Gate(Gate::Not));
        let disp = put(&mut w, Kind::Device(Device::Display { bits: 4 }));
        wire(&mut w, (i, 0), (disp, 0));
        // A 4-bit wire into a 1-bit gate is left out and reported.
        wire(&mut w, (i, 0), (not, 0));
        let missing = put(&mut w, Kind::Part(Part::new(PartKind::Chip("Yok".into()))));
        let (c, mut sim) = run(&w, &lib, &[9]);
        assert_eq!(sim.output(c.probe(End::new(i, 0)).unwrap()), 9);
        assert!(c.problems.contains(&Problem::Width {
            to: End::new(not, 0),
            from: 4,
            into: 1
        }));
        assert!(c
            .problems
            .iter()
            .any(|p| matches!(p, Problem::MissingChip { part, .. } if *part == missing)));
        sim.set_input(0, 3);
        sim.eval().unwrap();
        assert_eq!(sim.output(c.probe(End::new(i, 0)).unwrap()), 3);
    }

    #[test]
    fn devices_and_library_parts() {
        let lib = Library::new();
        let mut w = World::new("d");
        let rom = put(
            &mut w,
            Kind::Device(Device::Rom {
                addr: 2,
                data: 8,
                words: vec![10, 20, 30, 40],
            }),
        );
        let a = put(
            &mut w,
            Kind::Input {
                name: "a".into(),
                width: 2,
                value: 0,
            },
        );
        let split = put(&mut w, Kind::Part(Part::new(PartKind::Split(vec![4, 4]))));
        wire(&mut w, (a, 0), (rom, 0));
        wire(&mut w, (rom, 0), (split, 0));
        let lamp = put(&mut w, Kind::Lamp);
        wire(&mut w, (split, 1), (lamp, 0));
        let (c, sim) = run(&w, &lib, &[2]);
        assert_eq!(c.problems, vec![]);
        assert_eq!(sim.output(c.probe(End::new(rom, 0)).unwrap()), 30);
        assert_eq!(sim.output(c.probe(End::new(split, 0)).unwrap()), 30 & 15);
        assert_eq!(sim.output(c.probe(End::new(split, 1)).unwrap()), 30 >> 4);
        assert!(c.native(rom).is_some());
    }
}
