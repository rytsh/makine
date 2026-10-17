//! Chip definitions: the data model a user edits.
//!
//! A chip is a box with named input and output pins. A pin is 1 bit wide or
//! a bus of up to 64 bits. Inside the chip are parts (NAND gates, bus
//! splitters/joiners or other chips) connected by wires. Every wire goes from
//! a [`Source`] (something that produces a value) to a [`Sink`] (something
//! that consumes one). A sink may have at most one wire; a source may fan out.
//!
//! A wire must connect pins of the same width, except that a 1-bit source may
//! drive a wider sink, in which case the bit is copied to every line.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::native::NativeDef;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub const NAND: &str = "Nand";
pub const MAX_WIDTH: u8 = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Pin {
    pub name: String,
    pub width: u8,
}

impl Pin {
    pub fn new(name: impl Into<String>, width: u8) -> Self {
        Self {
            name: name.into(),
            width,
        }
    }

    /// Parse `"a"` (1 bit) or `"a[16]"` (16 bits).
    ///
    /// Panics on malformed specs; intended for chip definitions in code.
    pub fn parse(spec: &str) -> Self {
        match spec.split_once('[') {
            None => Self::new(spec, 1),
            Some((name, rest)) => {
                let width: u8 = rest
                    .strip_suffix(']')
                    .and_then(|w| w.parse().ok())
                    .filter(|w| (1..=MAX_WIDTH).contains(w))
                    .unwrap_or_else(|| panic!("bad pin spec {spec:?}"));
                Self::new(name, width)
            }
        }
    }

    pub fn mask(&self) -> u64 {
        mask(self.width)
    }
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.width == 1 {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}[{}]", self.name, self.width)
        }
    }
}

/// All ones in the low `width` bits.
pub fn mask(width: u8) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1 << width) - 1
    }
}

pub fn parse_pins(specs: &[&str]) -> Vec<Pin> {
    specs.iter().map(|s| Pin::parse(s)).collect()
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PartKind {
    Nand,
    Chip(String),
    /// One bus in, cut into groups of the given widths (lowest bits first).
    Split(Vec<u8>),
    /// Groups of the given widths in (lowest bits first), one bus out.
    Join(Vec<u8>),
    /// A fixed value: wired to ground (0) or power (1) per bit.
    Const {
        width: u8,
        value: u64,
    },
}

impl PartKind {
    pub fn name(&self) -> String {
        match self {
            PartKind::Nand => NAND.to_string(),
            PartKind::Chip(name) => name.clone(),
            PartKind::Split(w) => format!("Split {}", widths_label(w)),
            PartKind::Join(w) => format!("Join {}", widths_label(w)),
            PartKind::Const { width: 1, value } => format!("{value}"),
            PartKind::Const { value, .. } => format!("0x{value:X}"),
        }
    }
}

fn widths_label(widths: &[u8]) -> String {
    widths
        .iter()
        .map(|w| w.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Pin names for the groups of a Split/Join: "0", "1".. for single bits,
/// "0..7", "8..15".. for wider groups.
fn group_pins(widths: &[u8]) -> Vec<Pin> {
    let mut lo = 0u32;
    widths
        .iter()
        .map(|&w| {
            let hi = lo + w as u32 - 1;
            let name = if w == 1 {
                lo.to_string()
            } else {
                format!("{lo}..{hi}")
            };
            lo = hi + 1;
            Pin::new(name, w)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Part {
    pub kind: PartKind,
    /// Number of side-by-side copies. With `repeat = n` every pin becomes `n`
    /// times as wide; copy `k` uses the `k`-th slice of each pin. This turns a
    /// 1-bit `Not` into an `n`-bit one without placing `n` parts.
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub repeat: u8,
}

#[cfg(feature = "serde")]
fn one() -> u8 {
    1
}

impl Part {
    pub fn new(kind: PartKind) -> Self {
        Self { kind, repeat: 1 }
    }
}

pub type PartId = usize;

/// Where a value comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Source {
    /// One of this chip's own input pins.
    Input(usize),
    /// Output pin `pin` of part `part`.
    Part { part: PartId, pin: usize },
}

/// Where a value goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Sink {
    /// One of this chip's own output pins.
    Output(usize),
    /// Input pin `pin` of part `part`.
    Part { part: PartId, pin: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Wire {
    pub from: Source,
    pub to: Sink,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChipDef {
    pub name: String,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    pub parts: Vec<Part>,
    pub wires: Vec<Wire>,
}

impl ChipDef {
    /// Pins are given as specs, see [`Pin::parse`].
    pub fn new(name: impl Into<String>, inputs: &[&str], outputs: &[&str]) -> Self {
        Self {
            name: name.into(),
            inputs: parse_pins(inputs),
            outputs: parse_pins(outputs),
            parts: Vec::new(),
            wires: Vec::new(),
        }
    }

    pub fn input_index(&self, name: &str) -> Option<usize> {
        self.inputs.iter().position(|p| p.name == name)
    }

    pub fn output_index(&self, name: &str) -> Option<usize> {
        self.outputs.iter().position(|p| p.name == name)
    }

    /// Remove a part, its wires, and renumber the remaining parts.
    pub fn remove_part(&mut self, id: PartId) {
        self.parts.remove(id);
        self.wires.retain(|w| {
            !matches!(w.from, Source::Part { part, .. } if part == id)
                && !matches!(w.to, Sink::Part { part, .. } if part == id)
        });
        for w in &mut self.wires {
            if let Source::Part { part, .. } = &mut w.from {
                if *part > id {
                    *part -= 1;
                }
            }
            if let Sink::Part { part, .. } = &mut w.to {
                if *part > id {
                    *part -= 1;
                }
            }
        }
    }

    /// Index of the wire driving `sink`, if any.
    pub fn wire_into(&self, sink: Sink) -> Option<usize> {
        self.wires.iter().position(|w| w.to == sink)
    }

    /// Connect `from` to `to`, replacing any wire already driving `to`.
    pub fn connect(&mut self, from: Source, to: Sink) {
        match self.wire_into(to) {
            Some(i) => self.wires[i].from = from,
            None => self.wires.push(Wire { from, to }),
        }
    }
}

/// The pins of a placed part, with `repeat` applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartPins {
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
}

/// The set of chips available to build with. NAND, Split, Join and Const are
/// always built in.
///
/// A name may have a gate-level definition (built by the player), a native
/// model, or both. When both exist, the native model is used if the name is
/// in the fast set; otherwise the gates are simulated. Natives without a
/// definition are always used (e.g. RAM).
#[derive(Clone, Debug, Default)]
pub struct Library {
    chips: BTreeMap<String, ChipDef>,
    natives: BTreeMap<String, NativeDef>,
    fast: BTreeSet<String>,
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, chip: ChipDef) {
        self.chips.insert(chip.name.clone(), chip);
    }

    pub fn remove(&mut self, name: &str) -> Option<ChipDef> {
        self.chips.remove(name)
    }

    pub fn get(&self, name: &str) -> Option<&ChipDef> {
        self.chips.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        name == NAND || self.chips.contains_key(name) || self.natives.contains_key(name)
    }

    pub fn insert_native(&mut self, name: impl Into<String>, def: NativeDef) {
        self.natives.insert(name.into(), def);
    }

    pub fn native(&self, name: &str) -> Option<&NativeDef> {
        self.natives.get(name)
    }

    /// Simulate `name` with its native model instead of its gates.
    pub fn set_fast(&mut self, name: &str, fast: bool) {
        if fast {
            self.fast.insert(name.to_string());
        } else {
            self.fast.remove(name);
        }
    }

    /// The native model to use for `name`, if it should be used.
    pub fn native_for(&self, name: &str) -> Option<&NativeDef> {
        let n = self.natives.get(name)?;
        (self.fast.contains(name) || !self.chips.contains_key(name)).then_some(n)
    }

    pub fn chips(&self) -> impl Iterator<Item = &ChipDef> {
        self.chips.values()
    }

    /// Pins of a single copy of `kind`, or `None` if it is an unknown chip.
    pub fn kind_pins(&self, kind: &PartKind) -> Option<PartPins> {
        Some(match kind {
            PartKind::Nand => PartPins {
                inputs: vec![Pin::new("a", 1), Pin::new("b", 1)],
                outputs: vec![Pin::new("out", 1)],
            },
            PartKind::Chip(name) => match self.chips.get(name) {
                Some(c) => PartPins {
                    inputs: c.inputs.clone(),
                    outputs: c.outputs.clone(),
                },
                None => {
                    let n = self.natives.get(name)?;
                    PartPins {
                        inputs: n.inputs.clone(),
                        outputs: n.outputs.clone(),
                    }
                }
            },
            PartKind::Split(widths) => PartPins {
                inputs: vec![Pin::new("in", widths.iter().sum())],
                outputs: group_pins(widths),
            },
            PartKind::Join(widths) => PartPins {
                inputs: group_pins(widths),
                outputs: vec![Pin::new("out", widths.iter().sum())],
            },
            PartKind::Const { width, .. } => PartPins {
                inputs: vec![],
                outputs: vec![Pin::new("out", *width)],
            },
        })
    }

    /// Pins of a placed part, with `repeat` applied.
    pub fn part_pins(&self, part: &Part) -> Option<PartPins> {
        let mut pins = self.kind_pins(&part.kind)?;
        let r = part.repeat.max(1);
        for p in pins.inputs.iter_mut().chain(pins.outputs.iter_mut()) {
            // Too-wide parts are rejected when compiling; clamp for display.
            p.width = p.width.saturating_mul(r).min(MAX_WIDTH);
        }
        Some(pins)
    }
}

/// Convenience for constructing chips in code (reference solutions, tests).
///
/// ```
/// use makine_sim::ChipBuilder;
/// let mut b = ChipBuilder::new("Not", &["in"], &["out"]);
/// let x = b.input("in");
/// let n = b.nand(x, x);
/// b.output("out", n);
/// let chip = b.build();
/// assert_eq!(chip.parts.len(), 1);
/// ```
pub struct ChipBuilder {
    def: ChipDef,
}

impl ChipBuilder {
    pub fn new(name: impl Into<String>, inputs: &[&str], outputs: &[&str]) -> Self {
        Self {
            def: ChipDef::new(name, inputs, outputs),
        }
    }

    /// Source for the named input pin. Panics if the pin does not exist.
    pub fn input(&self, name: &str) -> Source {
        let i = self
            .def
            .input_index(name)
            .unwrap_or_else(|| panic!("chip {} has no input {name}", self.def.name));
        Source::Input(i)
    }

    pub fn nand(&mut self, a: Source, b: Source) -> Source {
        let part = self.part(Part::new(PartKind::Nand), &[a, b]);
        Source::Part { part, pin: 0 }
    }

    /// Add a sub-chip, wiring `inputs` to its input pins in order.
    pub fn chip(&mut self, name: &str, inputs: &[Source]) -> PartId {
        self.chip_n(name, 1, inputs)
    }

    /// Add `repeat` side-by-side copies of a sub-chip.
    pub fn chip_n(&mut self, name: &str, repeat: u8, inputs: &[Source]) -> PartId {
        self.part(
            Part {
                kind: PartKind::Chip(name.to_string()),
                repeat,
            },
            inputs,
        )
    }

    /// Add a single-output sub-chip and return its output.
    pub fn chip1(&mut self, name: &str, inputs: &[Source]) -> Source {
        let part = self.chip(name, inputs);
        Source::Part { part, pin: 0 }
    }

    /// Cut a bus into groups. Returns the part; group `i` is `out(part, i)`.
    pub fn split(&mut self, widths: &[u8], bus: Source) -> PartId {
        self.part(Part::new(PartKind::Split(widths.to_vec())), &[bus])
    }

    /// Join groups into one bus.
    pub fn join(&mut self, widths: &[u8], groups: &[Source]) -> Source {
        let part = self.part(Part::new(PartKind::Join(widths.to_vec())), groups);
        Source::Part { part, pin: 0 }
    }

    pub fn constant(&mut self, width: u8, value: u64) -> Source {
        let part = self.part(Part::new(PartKind::Const { width, value }), &[]);
        Source::Part { part, pin: 0 }
    }

    pub fn output(&mut self, name: &str, from: Source) {
        let i = self
            .def
            .output_index(name)
            .unwrap_or_else(|| panic!("chip {} has no output {name}", self.def.name));
        self.def.wires.push(Wire {
            from,
            to: Sink::Output(i),
        });
    }

    /// Add a wire by hand, e.g. to close a feedback loop.
    pub fn wire(&mut self, from: Source, to: Sink) {
        self.def.wires.push(Wire { from, to });
    }

    /// Add a part with its inputs wired in order. Inputs may be left out
    /// (shorter slice) and wired later with [`Self::wire`].
    pub fn part(&mut self, part: Part, inputs: &[Source]) -> PartId {
        let id = self.def.parts.len();
        self.def.parts.push(part);
        for (pin, &from) in inputs.iter().enumerate() {
            self.def.wires.push(Wire {
                from,
                to: Sink::Part { part: id, pin },
            });
        }
        id
    }

    pub fn build(self) -> ChipDef {
        self.def
    }
}

/// Output pin `pin` of `part`.
pub fn out(part: PartId, pin: usize) -> Source {
    Source::Part { part, pin }
}

/// Input pin `pin` of `part`.
pub fn into(part: PartId, pin: usize) -> Sink {
    Sink::Part { part, pin }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_parse() {
        assert_eq!(Pin::parse("a"), Pin::new("a", 1));
        assert_eq!(Pin::parse("bus[32]"), Pin::new("bus", 32));
        assert_eq!(Pin::parse("x[32]").to_string(), "x[32]");
    }

    #[test]
    fn split_and_repeat_pins() {
        let lib = Library::new();
        let p = lib.kind_pins(&PartKind::Split(vec![1, 3])).unwrap();
        assert_eq!(p.inputs, vec![Pin::new("in", 4)]);
        assert_eq!(p.outputs, vec![Pin::new("0", 1), Pin::new("1..3", 3)]);

        let p = lib
            .part_pins(&Part {
                kind: PartKind::Nand,
                repeat: 8,
            })
            .unwrap();
        assert!(p.inputs.iter().chain(&p.outputs).all(|p| p.width == 8));
    }

    #[test]
    fn remove_part_renumbers() {
        let mut b = ChipBuilder::new("T", &["a"], &["out"]);
        let a = b.input("a");
        let x = b.nand(a, a);
        let y = b.nand(x, x);
        b.output("out", y);
        let mut d = b.build();
        d.remove_part(0);
        assert_eq!(d.parts.len(), 1);
        assert_eq!(
            d.wires,
            vec![Wire {
                from: out(0, 0),
                to: Sink::Output(0)
            }]
        );
    }
}
