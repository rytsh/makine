//! Building worlds from text, for tests and examples.
//!
//! Each character is one cell on layer 0 (or the layer given):
//!
//! | char | block |
//! |---|---|
//! | `-` `\|` `+` | wire (all the same; shapes are for readability) |
//! | `#` | crossing |
//! | `o` | via |
//! | `L` | lamp |
//! | `S` / `s` | switch facing east, on / off |
//! | `>` `<` `^` `v` | NOT gate facing that way |
//! | `A` `O` `X` `N` `R` | AND, OR, XOR, NAND, NOR facing east |
//! | `a`..`z` (other) | input port named by the letter, facing east |
//! | `0`..`9` | output port named `q0`..`q9`, reading from the west |
//! | space or `.` | empty |

use crate::block::{Block, Dir, Gate};
use crate::world::{Edit, Pos, World};

pub fn block_for(c: char) -> Option<Block> {
    let gate = |gate| {
        Some(Block::Gate {
            gate,
            facing: Dir::East,
        })
    };
    match c {
        '-' | '|' | '+' => Some(Block::Wire),
        '#' => Some(Block::Cross),
        'o' => Some(Block::Via),
        'L' => Some(Block::Lamp),
        'S' => Some(Block::Switch {
            on: true,
            facing: Dir::East,
        }),
        's' => Some(Block::Switch {
            on: false,
            facing: Dir::East,
        }),
        '>' => Some(Block::Gate {
            gate: Gate::Not,
            facing: Dir::East,
        }),
        '<' => Some(Block::Gate {
            gate: Gate::Not,
            facing: Dir::West,
        }),
        '^' => Some(Block::Gate {
            gate: Gate::Not,
            facing: Dir::North,
        }),
        'v' => Some(Block::Gate {
            gate: Gate::Not,
            facing: Dir::South,
        }),
        'A' => gate(Gate::And),
        'O' => gate(Gate::Or),
        'X' => gate(Gate::Xor),
        'N' => gate(Gate::Nand),
        'R' => gate(Gate::Nor),
        'a'..='z' => Some(Block::Port {
            name: c.to_string(),
            output: false,
            facing: Dir::East,
        }),
        '0'..='9' => Some(Block::Port {
            name: format!("q{c}"),
            output: true,
            facing: Dir::East,
        }),
        _ => None,
    }
}

/// Edits placing `text` with its top-left at (x, y) on layer z.
pub fn edits(text: &str, x: i32, y: i32, z: i32) -> Edit {
    let mut out = Vec::new();
    for (dy, line) in text.lines().enumerate() {
        for (dx, c) in line.chars().enumerate() {
            if let Some(b) = block_for(c) {
                out.push(Edit::Set {
                    pos: Pos::new(x + dx as i32, y + dy as i32, z),
                    block: Some(b),
                });
            }
        }
    }
    Edit::Batch(out)
}

/// A world drawn from `layers`: one text block per layer, from layer 0 up.
pub fn world(name: &str, layers: &[&str]) -> World {
    let mut w = World::new(name);
    for (z, text) in layers.iter().enumerate() {
        w.apply(edits(text, 0, 0, z as i32));
    }
    w
}
