//! What can be placed in a world, and the pins each part has.

use makine_lang::{tr, trf};
use makine_sim::{mask, Library, Part, PartKind, PartPins, Pin, MAX_WIDTH};
use serde::{Deserialize, Serialize};

use crate::device::Device;

/// Built-in gates, so a world can be built before any level is solved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gate {
    Not,
    And,
    Or,
    Xor,
    Nand,
    Nor,
}

impl Gate {
    pub const ALL: [Gate; 6] = [
        Gate::Not,
        Gate::And,
        Gate::Or,
        Gate::Xor,
        Gate::Nand,
        Gate::Nor,
    ];

    pub fn inputs(self) -> usize {
        match self {
            Gate::Not => 1,
            _ => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Gate::Not => "NOT",
            Gate::And => "AND",
            Gate::Or => "OR",
            Gate::Xor => "XOR",
            Gate::Nand => "NAND",
            Gate::Nor => "NOR",
        }
    }
}

/// Longest port name.
pub const MAX_NAME: usize = 32;
pub const MAX_PERIOD: u32 = 1_000_000;

/// Port names: letters, digits and `_`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= MAX_NAME
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// A part in a world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Gate(Gate),
    /// A one-bit switch the player flips.
    Switch {
        on: bool,
    },
    /// Lit while its input is not 0. Its pin takes a wire of any width.
    Lamp,
    /// Toggles every `period` steps.
    Clock {
        period: u32,
    },
    /// A named input of the world (a pin when it becomes a chip). Its value
    /// is set by clicking it.
    Input {
        name: String,
        width: u8,
        value: u64,
    },
    /// A named output of the world.
    Output {
        name: String,
        width: u8,
    },
    Device(Device),
    /// A NAND, a chip from the player's library, a splitter, a joiner or a
    /// constant: the same parts as in the level editor.
    Part(Part),
}

impl Kind {
    /// The part's pins. `None` for a chip missing from `lib`.
    pub fn pins(&self, lib: &Library) -> Option<PartPins> {
        let one = |n: &str| Pin::new(n, 1);
        let (inputs, outputs) = match self {
            Kind::Gate(g) if g.inputs() == 1 => (vec![one("in")], vec![one("out")]),
            Kind::Gate(_) => (vec![one("a"), one("b")], vec![one("out")]),
            Kind::Switch { .. } | Kind::Clock { .. } => (vec![], vec![one("out")]),
            // Any width: lit while the wire is not 0.
            Kind::Lamp => (vec![Pin::new("in", 0)], vec![]),
            Kind::Input { name, width, .. } => (vec![], vec![Pin::new(name.clone(), *width)]),
            Kind::Output { name, width } => (vec![Pin::new(name.clone(), *width)], vec![]),
            Kind::Device(d) => (d.inputs(), d.outputs()),
            Kind::Part(p) => return lib.part_pins(p),
        };
        Some(PartPins { inputs, outputs })
    }

    pub fn label(&self) -> String {
        match self {
            Kind::Gate(g) => g.name().into(),
            Kind::Switch { .. } => tr!("Anahtar", "Switch").into(),
            Kind::Lamp => tr!("Lamba", "Lamp").into(),
            Kind::Clock { .. } => tr!("Saat", "Clock").into(),
            Kind::Input { name, .. } => trf!("Giriş {name}", "Input {name}"),
            Kind::Output { name, .. } => trf!("Çıkış {name}", "Output {name}"),
            Kind::Device(d) => d.name(),
            Kind::Part(p) => match &p.kind {
                PartKind::Nand => "NAND".into(),
                PartKind::Chip(n) => n.clone(),
                PartKind::Split(_) => tr!("Böl", "Split").into(),
                PartKind::Join(_) => tr!("Birleştir", "Join").into(),
                PartKind::Const { .. } => tr!("Sabit", "Constant").into(),
            },
        }
    }

    /// The library chip this part is, if it is one.
    pub fn chip_name(&self) -> Option<&str> {
        match self {
            Kind::Part(Part {
                kind: PartKind::Chip(n),
                ..
            }) => Some(n),
            _ => None,
        }
    }

    /// The port name, for inputs and outputs.
    pub fn port_name(&self) -> Option<&str> {
        match self {
            Kind::Input { name, .. } | Kind::Output { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Whether `self` and `other` build the same circuit: they differ at
    /// most in what the player sets while it runs (a switch, an input's
    /// value).
    pub fn same_circuit(&self, other: &Kind) -> bool {
        match (self, other) {
            (Kind::Switch { .. }, Kind::Switch { .. }) => true,
            (
                Kind::Input { name, width, .. },
                Kind::Input {
                    name: n2,
                    width: w2,
                    ..
                },
            ) => name == n2 && width == w2,
            (a, b) => a == b,
        }
    }

    /// The part as it may be stored: sizes clamped, or `None` if it makes
    /// no sense (e.g. from a broken file or another player).
    pub fn sanitized(self) -> Option<Kind> {
        let width_ok = |w: u8| (1..=MAX_WIDTH).contains(&w);
        Some(match self {
            Kind::Clock { period } => Kind::Clock {
                period: period.clamp(1, MAX_PERIOD),
            },
            Kind::Input { name, width, value } => {
                if !valid_name(&name) || !width_ok(width) {
                    return None;
                }
                Kind::Input {
                    name,
                    width,
                    value: value & mask(width),
                }
            }
            Kind::Output { name, width } => {
                if !valid_name(&name) || !width_ok(width) {
                    return None;
                }
                Kind::Output { name, width }
            }
            Kind::Device(d) => Kind::Device(d.clamped()),
            Kind::Part(p) => {
                let groups_ok = |w: &[u8]| {
                    !w.is_empty()
                        && w.len() <= MAX_WIDTH as usize
                        && w.iter().all(|&x| x >= 1)
                        && w.iter().map(|&x| x as u32).sum::<u32>() <= MAX_WIDTH as u32
                };
                let ok = (1..=MAX_WIDTH).contains(&p.repeat)
                    && match &p.kind {
                        PartKind::Nand => true,
                        PartKind::Chip(n) => !n.is_empty() && n.len() <= 64,
                        PartKind::Split(w) | PartKind::Join(w) => groups_ok(w),
                        PartKind::Const { width, .. } => width_ok(*width),
                    };
                if !ok {
                    return None;
                }
                let mut p = p;
                if let PartKind::Const { width, value } = &mut p.kind {
                    *value &= mask(*width);
                }
                Kind::Part(p)
            }
            k => k,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("x0"));
        assert!(valid_name("çık_1"));
        assert!(!valid_name(""));
        assert!(!valid_name("a b"));
        assert!(!valid_name(&"a".repeat(MAX_NAME + 1)));
    }

    #[test]
    fn sanitizing() {
        let i = Kind::Input {
            name: "a".into(),
            width: 4,
            value: 0xff,
        };
        assert_eq!(
            i.sanitized(),
            Some(Kind::Input {
                name: "a".into(),
                width: 4,
                value: 0xf
            })
        );
        let bad = Kind::Output {
            name: "q".into(),
            width: 0,
        };
        assert_eq!(bad.sanitized(), None);
        let split = Kind::Part(Part::new(PartKind::Split(vec![40, 40])));
        assert_eq!(split.sanitized(), None);
        assert_eq!(
            Kind::Clock { period: 0 }.sanitized(),
            Some(Kind::Clock { period: 1 })
        );
    }

    #[test]
    fn switches_and_inputs_keep_the_circuit() {
        let a = Kind::Switch { on: false };
        assert!(a.same_circuit(&Kind::Switch { on: true }));
        let i = |name: &str, value| Kind::Input {
            name: name.into(),
            width: 1,
            value,
        };
        assert!(i("a", 0).same_circuit(&i("a", 1)));
        assert!(!i("a", 0).same_circuit(&i("b", 0)));
        assert!(!Kind::Gate(Gate::And).same_circuit(&Kind::Gate(Gate::Or)));
    }
}
