//! What a cell can hold.

use serde::{Deserialize, Serialize};

use crate::device::Device;

/// The four horizontal directions. Vertical connections between layers go
/// through [`Block::Via`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Dir {
    /// +x
    East,
    /// +y
    South,
    /// -x
    West,
    /// -y
    North,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::East, Dir::South, Dir::West, Dir::North];

    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::East => (1, 0),
            Dir::South => (0, 1),
            Dir::West => (-1, 0),
            Dir::North => (0, -1),
        }
    }

    /// Turn clockwise (seen from above).
    pub fn cw(self) -> Dir {
        match self {
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
            Dir::North => Dir::East,
        }
    }

    pub fn ccw(self) -> Dir {
        self.cw().cw().cw()
    }

    pub fn opposite(self) -> Dir {
        self.cw().cw()
    }

    pub fn is_horizontal(self) -> bool {
        matches!(self, Dir::East | Dir::West)
    }

    /// This direction's bit in a [`Block::Trace`]'s `sides`.
    pub fn bit(self) -> u8 {
        match self {
            Dir::East => 1,
            Dir::South => 2,
            Dir::West => 4,
            Dir::North => 8,
        }
    }
}

/// All four sides, as [`Block::Trace`] bits.
pub const ALL_SIDES: u8 = 0b1111;

/// Built-in gates. Two-input gates read from the two sides of their back
/// (left and right of the facing direction); NOT and buffers read from the
/// back. All drive the cell in front.
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

    pub fn eval(self, a: bool, b: bool) -> bool {
        match self {
            Gate::Not => !a,
            Gate::And => a & b,
            Gate::Or => a | b,
            Gate::Xor => a ^ b,
            Gate::Nand => !(a & b),
            Gate::Nor => !(a | b),
        }
    }
}

/// One cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    /// A wire that connects on all four sides (worlds made before
    /// [`Block::Trace`]; tests and text drawings).
    Wire,
    /// A wire that connects only on the sides in `sides` (bits of
    /// [`Dir::bit`]). Two traces touch only if both have the side between
    /// them, so wires drawn side by side stay apart; drawing over a wire
    /// joins it.
    Trace { sides: u8 },
    /// A wire that connects only along its axis (front and back), so
    /// straight wires laid side by side form a bus without touching.
    Straight { facing: Dir },
    /// Two wires crossing without touching: east-west and north-south are
    /// separate signals.
    Cross,
    /// Connects to the cells directly above and below (other layers), and
    /// to neighbours on its own layer, like a wire.
    Via,
    /// A gate facing `facing`: output in front, inputs behind.
    Gate { gate: Gate, facing: Dir },
    /// A one-bit switch the player toggles. Drives the cell in front.
    Switch { on: bool, facing: Dir },
    /// A lamp: lit when any neighbouring wire is 1. It only reads: wires on
    /// different sides stay separate signals.
    Lamp,
    /// A clock: drives the cell in front, toggling every `period` ticks.
    Clock { period: u32, facing: Dir },
    /// A named pin of the whole world. Inputs drive the cell in front;
    /// outputs read the cell behind. When the world is packaged as a chip,
    /// these become the chip's pins (1 bit each).
    Port {
        name: String,
        output: bool,
        facing: Dir,
    },
    /// A device covering several cells; this is its anchor cell (back-left
    /// corner). See [`crate::device::layout`].
    Device { device: Device, facing: Dir },
    /// Another cell of a device: the anchor is at this cell plus (dx, dy).
    Cover { dx: i32, dy: i32 },
}

impl Block {
    pub fn facing(&self) -> Option<Dir> {
        match self {
            Block::Gate { facing, .. }
            | Block::Switch { facing, .. }
            | Block::Clock { facing, .. }
            | Block::Port { facing, .. }
            | Block::Straight { facing }
            | Block::Device { facing, .. } => Some(*facing),
            _ => None,
        }
    }

    /// The same block turned clockwise.
    pub fn rotated(&self) -> Block {
        let mut b = self.clone();
        match &mut b {
            Block::Gate { facing, .. }
            | Block::Switch { facing, .. }
            | Block::Clock { facing, .. }
            | Block::Port { facing, .. }
            | Block::Straight { facing }
            | Block::Device { facing, .. } => *facing = facing.cw(),
            Block::Trace { sides } => {
                *sides = Dir::ALL
                    .iter()
                    .filter(|d| *sides & d.bit() != 0)
                    .fold(0, |s, d| s | d.cw().bit());
            }
            _ => {}
        }
        b
    }

    /// Whether a wire in the neighbouring cell, in direction `d`, joins this
    /// block's net(s). Gates and devices connect only on their pin sides.
    /// Multi-cell devices answer `false` here: their pins depend on the
    /// cell, see [`crate::World::accepts_wire`].
    pub fn accepts_wire_from(&self, d: Dir) -> bool {
        match self {
            Block::Wire | Block::Cross | Block::Via | Block::Lamp => true,
            Block::Trace { sides } => sides & d.bit() != 0,
            Block::Straight { facing } => d == *facing || d == facing.opposite(),
            Block::Gate { gate, facing } => {
                d == *facing
                    || match gate.inputs() {
                        1 => d == facing.opposite(),
                        _ => d == facing.cw() || d == facing.ccw(),
                    }
            }
            Block::Switch { facing, .. } | Block::Clock { facing, .. } => d == *facing,
            Block::Port { output, facing, .. } => {
                if *output {
                    d == facing.opposite()
                } else {
                    d == *facing
                }
            }
            Block::Device { .. } | Block::Cover { .. } => false,
        }
    }

    /// Whether this block drives a wire on side `d` (its output side).
    /// Only meaningful where [`Block::accepts_wire_from`] is true; devices
    /// are answered by [`crate::World::pin`].
    pub fn drives(&self, d: Dir) -> bool {
        match self {
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

    pub fn label(&self) -> String {
        match self {
            Block::Wire | Block::Trace { .. } => "Tel".into(),
            Block::Straight { .. } => "Düz tel".into(),
            Block::Cross => "Köprü".into(),
            Block::Via => "Geçiş".into(),
            Block::Gate { gate, .. } => gate.name().into(),
            Block::Switch { .. } => "Anahtar".into(),
            Block::Lamp => "Lamba".into(),
            Block::Clock { .. } => "Saat".into(),
            Block::Port { name, output, .. } => {
                format!("{} {name}", if *output { "Çıkış" } else { "Giriş" })
            }
            Block::Device { device, .. } => device.name(),
            Block::Cover { .. } => "Cihaz".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions() {
        for d in Dir::ALL {
            assert_eq!(d.cw().ccw(), d);
            assert_eq!(d.opposite().opposite(), d);
            let (x, y) = d.delta();
            let (ox, oy) = d.opposite().delta();
            assert_eq!((x + ox, y + oy), (0, 0));
        }
        assert_eq!(Dir::East.cw(), Dir::South);
    }

    #[test]
    fn gate_pins() {
        let and = Block::Gate {
            gate: Gate::And,
            facing: Dir::East,
        };
        // Output east, inputs north and south.
        assert!(and.accepts_wire_from(Dir::East));
        assert!(and.accepts_wire_from(Dir::North));
        assert!(and.accepts_wire_from(Dir::South));
        assert!(!and.accepts_wire_from(Dir::West));
        let not = Block::Gate {
            gate: Gate::Not,
            facing: Dir::East,
        };
        assert!(not.accepts_wire_from(Dir::West));
        assert!(!not.accepts_wire_from(Dir::North));
        // Rotating four times is the identity.
        assert_eq!(and.rotated().rotated().rotated().rotated(), and);
    }
}
