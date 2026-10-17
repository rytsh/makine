//! Drawing wires: a wire connects where the player drew it.
//!
//! A drawn wire is a [`Block::Trace`] that knows its connected sides.
//! Dragging from one cell to the next links the two; wires drawn side by
//! side never touch, and drawing over a wire joins it. A wire also links to
//! the pins next to it (gates, switches, lamps, ports, devices), since that
//! is what a wire next to a pin is for.

use crate::block::{Block, Dir};
use crate::world::{Edit, Pos, World};

fn is_wire(b: &Block) -> bool {
    matches!(
        b,
        Block::Wire | Block::Trace { .. } | Block::Straight { .. } | Block::Cross | Block::Via
    )
}

/// The edit drawing a wire into `to`, continuing from `from` (the previous
/// cell of a drag, if any). Cells between them are filled, first along x
/// then along y, so a fast drag leaves no gaps. With `replace`, a block that
/// is not a wire at `to` is replaced (a click); otherwise the drag goes
/// around it, only linking to it.
pub fn draw_wire(world: &World, from: Option<Pos>, to: Pos, replace: bool) -> Edit {
    let mut w = Wires {
        world,
        cells: Vec::new(),
    };
    let path = match from {
        Some(f) if f.z == to.z && f != to => path(f, to),
        _ => vec![to],
    };
    for (i, &cell) in path.iter().enumerate() {
        // The first cell of a path is the previous end, already drawn.
        if i == 0 && from.is_some() && path.len() > 1 {
            continue;
        }
        let made = w.make(cell, replace && cell == to);
        if made {
            w.link_pins(cell);
        }
        if let Some(&prev) = i.checked_sub(1).and_then(|j| path.get(j)) {
            w.link(prev, cell);
        }
    }
    Edit::Batch(
        w.cells
            .into_iter()
            .map(|(pos, b)| Edit::Set {
                pos,
                block: Some(b),
            })
            .collect(),
    )
}

/// The edit opening the wires beside `pos` towards it, for a block just
/// placed there that has pins (a gate, switch, port ...): wires drawn first
/// and blocks placed after still connect. Wires on sides without a pin are
/// left alone.
pub fn link_to_block(world: &World, pos: Pos) -> Edit {
    let mut w = Wires {
        world,
        cells: Vec::new(),
    };
    if world.get(pos).is_some_and(|b| !is_wire(b)) {
        for d in Dir::ALL {
            if world.pin(pos, d).is_some() {
                w.open(pos.step(d), d.opposite());
            }
        }
    }
    Edit::Batch(
        w.cells
            .into_iter()
            .map(|(pos, b)| Edit::Set {
                pos,
                block: Some(b),
            })
            .collect(),
    )
}

/// Cells from `a` to `b`, both included: along x, then along y.
fn path(a: Pos, b: Pos) -> Vec<Pos> {
    let mut out = vec![a];
    let mut p = a;
    while p.x != b.x {
        p.x += (b.x - p.x).signum();
        out.push(p);
    }
    while p.y != b.y {
        p.y += (b.y - p.y).signum();
        out.push(p);
    }
    out
}

/// The wire cells being drawn, over the world.
struct Wires<'a> {
    world: &'a World,
    cells: Vec<(Pos, Block)>,
}

impl Wires<'_> {
    fn get(&self, p: Pos) -> Option<Block> {
        match self.cells.iter().rev().find(|(q, _)| *q == p) {
            Some((_, b)) => Some(b.clone()),
            None => self.world.get(p).cloned(),
        }
    }

    fn set(&mut self, p: Pos, b: Block) {
        self.cells.retain(|(q, _)| *q != p);
        self.cells.push((p, b));
    }

    /// Make `p` a wire if it is empty (or `replace`). Returns whether it
    /// is a wire now.
    fn make(&mut self, p: Pos, replace: bool) -> bool {
        match self.get(p) {
            Some(b) if is_wire(&b) => true,
            Some(Block::Device { .. } | Block::Cover { .. }) => false,
            Some(_) if !replace => false,
            _ => {
                self.set(p, Block::Trace { sides: 0 });
                true
            }
        }
    }

    /// Open side `d` of the wire at `p` (other wires already connect on
    /// every side they can).
    fn open(&mut self, p: Pos, d: Dir) {
        if let Some(Block::Trace { sides }) = self.get(p) {
            if sides & d.bit() == 0 {
                self.set(
                    p,
                    Block::Trace {
                        sides: sides | d.bit(),
                    },
                );
            }
        }
    }

    /// Connect neighbouring cells `a` and `b` where they are wires.
    fn link(&mut self, a: Pos, b: Pos) {
        let Some(d) = Dir::ALL.into_iter().find(|d| a.step(*d) == b) else {
            return;
        };
        self.open(a, d);
        self.open(b, d.opposite());
    }

    /// Open the wire at `p` towards every pin beside it.
    fn link_pins(&mut self, p: Pos) {
        for d in Dir::ALL {
            let n = p.step(d);
            let pin = self
                .get(n)
                .is_some_and(|b| !is_wire(&b) && self.world.pin(n, d.opposite()).is_some());
            if pin {
                self.open(p, d);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Gate;

    fn draw(w: &mut World, cells: &[(i32, i32)]) {
        let mut prev = None;
        for &(x, y) in cells {
            let p = Pos::new(x, y, 0);
            let e = draw_wire(w, prev, p, false);
            w.apply(e);
            prev = Some(p);
        }
    }

    fn sides(w: &World, x: i32, y: i32) -> u8 {
        match w.get(Pos::new(x, y, 0)) {
            Some(Block::Trace { sides }) => *sides,
            b => panic!("{b:?}"),
        }
    }

    const E: u8 = 1;
    const S: u8 = 2;
    const W: u8 = 4;
    const N: u8 = 8;

    #[test]
    fn side_by_side_wires_stay_apart() {
        let mut w = World::new("w");
        draw(&mut w, &[(0, 0), (1, 0), (2, 0)]);
        draw(&mut w, &[(0, 1), (1, 1), (2, 1)]);
        assert_eq!(sides(&w, 1, 0), E | W);
        assert_eq!(sides(&w, 1, 1), E | W);
        assert!(!w.accepts_wire(Pos::new(1, 0, 0), Dir::South));
    }

    #[test]
    fn drawing_over_a_wire_joins_it() {
        let mut w = World::new("w");
        draw(&mut w, &[(0, 1), (1, 1), (2, 1)]);
        // A wire from above, down into the middle one.
        draw(&mut w, &[(1, -1), (1, 0), (1, 1)]);
        assert_eq!(sides(&w, 1, 1), E | W | N);
        assert_eq!(sides(&w, 1, 0), N | S);
    }

    #[test]
    fn fast_drags_fill_the_gap() {
        let mut w = World::new("w");
        draw(&mut w, &[(0, 0), (3, 2)]);
        assert_eq!(w.len(), 6);
        assert_eq!(sides(&w, 3, 0), W | S);
        assert_eq!(sides(&w, 3, 2), N);
    }

    #[test]
    fn wires_link_to_pins_beside_them() {
        let mut w = World::new("w");
        let gate = Block::Gate {
            gate: Gate::And,
            facing: Dir::East,
        };
        w.apply(Edit::Set {
            pos: Pos::new(1, 1, 0),
            block: Some(gate.clone()),
        });
        // A wire passing under the gate's south input links to it, and the
        // drag goes around the gate instead of replacing it.
        draw(&mut w, &[(0, 2), (1, 2), (2, 2)]);
        assert_eq!(sides(&w, 1, 2), E | W | N);
        draw(&mut w, &[(0, 0), (1, 0)]);
        draw(&mut w, &[(1, 0), (1, 1), (1, 2)]);
        assert_eq!(w.get(Pos::new(1, 1, 0)), Some(&gate));
        // A wire ending beside the gate's back (no pin there) stays open.
        draw(&mut w, &[(-1, 1), (0, 1)]);
        assert_eq!(sides(&w, 0, 1), W);
    }

    #[test]
    fn blocks_placed_later_link_to_wires() {
        let mut w = World::new("w");
        draw(&mut w, &[(1, 0), (2, 0)]);
        draw(&mut w, &[(0, 1), (0, 2)]);
        let p = Pos::new(0, 0, 0);
        w.apply(Edit::Set {
            pos: p,
            block: Some(Block::Switch {
                on: true,
                facing: Dir::East,
            }),
        });
        w.apply(link_to_block(&w, p));
        // Its output side (east) links; the wire to its south (no pin
        // there) does not.
        assert_eq!(sides(&w, 1, 0), E | W);
        assert_eq!(sides(&w, 0, 1), S);
    }

    #[test]
    fn a_click_replaces() {
        let mut w = World::new("w");
        w.apply(Edit::Set {
            pos: Pos::new(0, 0, 0),
            block: Some(Block::Lamp),
        });
        let e = draw_wire(&w, None, Pos::new(0, 0, 0), true);
        w.apply(e);
        assert_eq!(sides(&w, 0, 0), 0);
    }
}
