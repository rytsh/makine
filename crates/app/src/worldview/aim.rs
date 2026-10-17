//! What the pointer aims at: the block under it and the cell a new block
//! would go into, found by walking the pointer's ray through the grid.
//!
//! Like in Minecraft, clicking a face of a block builds next to that face:
//! on top of it, or beside it. Wires are flat, like paint on the floor:
//! aiming at one aims at its own cell, so a wire can be painted over (with
//! a gate, say). Blocks above the edited layer are hidden and cannot be hit;
//! the edited layer's floor stops the ray, so empty cells there are built in
//! directly.

use glam::DVec3;
use makine_world::block::Block;
use makine_world::{Pos, World};

use super::camera::Ray;

/// How many cells the ray may cross before giving up.
const MAX_STEPS: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aim {
    /// The block under the pointer, if any (else the pointer is on the
    /// floor).
    pub target: Option<Pos>,
    /// Where a block placed now would go.
    pub place: Pos,
}

/// Height of a block's box, as drawn, for hitting it.
fn height(world: &World, p: Pos, b: &Block) -> f64 {
    match b {
        Block::Wire | Block::Straight { .. } => 0.12,
        Block::Cross => 0.26,
        Block::Via => {
            if matches!(world.get(p.up()), Some(Block::Via)) {
                1.0
            } else {
                0.35
            }
        }
        Block::Gate { .. } => 0.6,
        Block::Switch { .. } => 0.3,
        Block::Lamp => 0.75,
        Block::Clock { .. } => 0.5,
        Block::Port { .. } => 0.4,
        Block::Device { .. } | Block::Cover { .. } => 0.5,
    }
}

/// Wires lie on the floor: building on one replaces it.
pub fn flat(b: &Block) -> bool {
    matches!(b, Block::Wire | Block::Straight { .. } | Block::Cross)
}

/// Blocks a click uses (toggles or selects) instead of building on.
pub fn interactive(b: &Block) -> bool {
    matches!(
        b,
        Block::Switch { .. }
            | Block::Clock { .. }
            | Block::Port { .. }
            | Block::Device { .. }
            | Block::Cover { .. }
    )
}

/// Where the ray enters the box `lo..hi`: the distance and the axis of the
/// face it enters through (0 x, 1 y, 2 z).
fn enter_box(ray: &Ray, lo: DVec3, hi: DVec3) -> Option<(f64, usize)> {
    let (mut t0, mut t1, mut axis) = (f64::NEG_INFINITY, f64::INFINITY, 2);
    for i in 0..3 {
        let (o, d) = (ray.from[i], ray.dir[i]);
        if d.abs() < 1e-12 {
            if o < lo[i] || o > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - o) / d, (hi[i] - o) / d);
        if a.min(b) > t0 {
            t0 = a.min(b);
            axis = i;
        }
        t1 = t1.min(a.max(b));
    }
    (t0 <= t1 && t1 >= 0.0).then_some((t0.max(0.0), axis))
}

/// What `ray` aims at, building on layer `layer` (layers above it are
/// hidden).
pub fn aim(world: &World, ray: &Ray, layer: i32) -> Option<Aim> {
    let lim = makine_world::world::MAX_COORD as f64;
    if ray.from.abs().max_element() > lim {
        return None;
    }
    let mut cell = ray.from.floor();
    let step = ray.dir.signum();
    let delta = DVec3::new(
        (1.0 / ray.dir.x).abs(),
        (1.0 / ray.dir.y).abs(),
        (1.0 / ray.dir.z).abs(),
    );
    // Distance along the ray to the next cell border on each axis.
    let mut next = DVec3::ZERO;
    for i in 0..3 {
        next[i] = if ray.dir[i] > 0.0 {
            (cell[i] + 1.0 - ray.from[i]) * delta[i]
        } else if ray.dir[i] < 0.0 {
            (ray.from[i] - cell[i]) * delta[i]
        } else {
            f64::INFINITY
        };
    }
    for _ in 0..MAX_STEPS {
        let p = Pos::new(cell.x as i32, cell.y as i32, cell.z as i32);
        if p.z <= layer {
            if let Some(b) = world.get(p) {
                let lo = cell;
                let hi = cell + DVec3::new(1.0, 1.0, height(world, p, b));
                if let Some((_, axis)) = enter_box(ray, lo, hi) {
                    if flat(b) {
                        return Some(Aim {
                            target: Some(p),
                            place: p,
                        });
                    }
                    // The neighbour on the side the ray came from.
                    let mut n = [0; 3];
                    n[axis] = -step[axis] as i32;
                    let place = Pos::new(p.x + n[0], p.y + n[1], p.z + n[2]);
                    return Some(Aim {
                        target: Some(p),
                        place,
                    });
                }
            }
        }
        let axis = if next.x < next.y && next.x < next.z {
            0
        } else if next.y < next.z {
            1
        } else {
            2
        };
        // Going down through the edited layer's floor: an empty cell there.
        if axis == 2 && step.z < 0.0 && p.z == layer {
            return Some(Aim {
                target: None,
                place: p,
            });
        }
        cell[axis] += step[axis];
        next[axis] += delta[axis];
        if cell.abs().max_element() > lim {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_world::block::{Dir, Gate};
    use makine_world::Edit;

    fn down_at(x: f64, y: f64) -> Ray {
        // Slanting down from the south-east, like the camera.
        Ray {
            from: DVec3::new(x + 3.0, y + 3.0, 6.0),
            dir: DVec3::new(-3.0, -3.0, -6.0).normalize(),
        }
    }

    fn set(w: &mut World, p: Pos, b: Block) {
        w.apply(Edit::Set {
            pos: p,
            block: Some(b),
        });
    }

    #[test]
    fn empty_floor_is_built_in() {
        let w = World::new("a");
        // The ray reaches height 0 at (2.5, 2.5).
        let a = aim(&w, &down_at(2.5, 2.5), 0).unwrap();
        assert_eq!(
            a,
            Aim {
                target: None,
                place: Pos::new(2, 2, 0)
            }
        );
        let a = aim(&w, &down_at(2.5, 2.5), 3).unwrap();
        assert_eq!(a.place.z, 3, "the edited layer's floor");
    }

    #[test]
    fn top_of_a_block_builds_above_it() {
        let mut w = World::new("a");
        let g = Pos::new(2, 2, 0);
        let gate = Block::Gate {
            gate: Gate::And,
            facing: Dir::East,
        };
        set(&mut w, g, gate);
        // Aim at the centre of its top (height 0.6).
        let r = Ray {
            from: DVec3::new(2.5 + 2.0, 2.5 + 2.0, 0.6 + 4.0),
            dir: DVec3::new(-2.0, -2.0, -4.0).normalize(),
        };
        let a = aim(&w, &r, 0).unwrap();
        assert_eq!(a.target, Some(g));
        assert_eq!(a.place, Pos::new(2, 2, 1));
    }

    #[test]
    fn side_of_a_block_builds_beside_it() {
        let mut w = World::new("a");
        let g = Pos::new(0, 0, 0);
        set(&mut w, g, Block::Lamp);
        // Level with the lamp, coming from the east.
        let r = Ray {
            from: DVec3::new(5.5, 0.5, 0.3),
            dir: DVec3::new(-1.0, 0.0, -0.01).normalize(),
        };
        let a = aim(&w, &r, 0).unwrap();
        assert_eq!(a.target, Some(g));
        assert_eq!(a.place, Pos::new(1, 0, 0));
    }

    #[test]
    fn wires_are_painted_over() {
        let mut w = World::new("a");
        let p = Pos::new(2, 2, 0);
        set(&mut w, p, Block::Wire);
        let a = aim(&w, &down_at(2.5, 2.5), 0).unwrap();
        assert_eq!(
            a,
            Aim {
                target: Some(p),
                place: p
            }
        );
    }

    #[test]
    fn hidden_layers_are_not_hit() {
        let mut w = World::new("a");
        set(&mut w, Pos::new(2, 2, 1), Block::Lamp);
        let a = aim(&w, &down_at(2.5, 2.5), 0).unwrap();
        assert_eq!(a.target, None, "layer 1 is hidden while editing 0");
    }

    #[test]
    fn stacking_climbs() {
        // Each block placed on top of the last.
        let mut w = World::new("a");
        let mut layer = 0;
        for _ in 0..4 {
            let r = Ray {
                from: DVec3::new(0.5, 0.5, layer as f64 + 10.0),
                dir: DVec3::new(0.0, 0.0, -1.0),
            };
            let a = aim(&w, &r, layer).unwrap();
            set(&mut w, a.place, Block::Lamp);
            layer = a.place.z;
        }
        assert_eq!(w.len(), 4);
        assert!(w.get(Pos::new(0, 0, 3)).is_some());
    }
}
