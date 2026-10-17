//! Rectangular regions of a world: copying, cutting, pasting and moving
//! groups of blocks.
//!
//! A region spans every layer of a rectangle, so circuits built over
//! several layers (with vias) are copied whole. Copied blocks form a
//! [`Clip`], plain data that can be rotated, pasted elsewhere, put on the
//! system clipboard as text, or sent to another world.

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::device;
use crate::world::{Edit, Pos, World, MAX_LAYER};

pub const CLIP_FORMAT: &str = "makine-blocks";

/// A rectangle of cells (inclusive corners) through all layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Region {
    /// The region between two corner cells, in any order.
    pub fn between(a: Pos, b: Pos) -> Region {
        Region {
            x0: a.x.min(b.x),
            y0: a.y.min(b.y),
            x1: a.x.max(b.x),
            y1: a.y.max(b.y),
        }
    }

    pub fn contains(&self, p: Pos) -> bool {
        (self.x0..=self.x1).contains(&p.x) && (self.y0..=self.y1).contains(&p.y)
    }

    pub fn width(&self) -> i32 {
        self.x1 - self.x0 + 1
    }

    pub fn height(&self) -> i32 {
        self.y1 - self.y0 + 1
    }

    /// Blocks in the region. A device is included only when all of its
    /// cells are inside; otherwise none of them are.
    pub fn blocks<'a>(&self, world: &'a World) -> Vec<(Pos, &'a Block)> {
        world
            .blocks()
            .filter(|(p, _)| self.contains(*p))
            .filter(|(p, _)| match world.device_at(*p) {
                Some((anchor, d, f)) => device::layout(d, anchor, f)
                    .cells
                    .iter()
                    .all(|c| self.contains(*c)),
                None => true,
            })
            .collect()
    }

    /// The edit clearing the region (devices only partly inside are left
    /// alone).
    pub fn clear(&self, world: &World) -> Edit {
        Edit::Batch(
            self.blocks(world)
                .into_iter()
                .map(|(pos, _)| Edit::Set { pos, block: None })
                .collect(),
        )
    }
}

/// Copied blocks, at positions relative to the clip's origin (its lowest
/// corner: smallest x, y and layer).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    pub format: String,
    pub blocks: Vec<(Pos, Block)>,
}

impl Clip {
    pub fn copy(world: &World, region: Region) -> Option<Clip> {
        let blocks = region.blocks(world);
        let z0 = blocks.iter().map(|(p, _)| p.z).min()?;
        let x0 = blocks.iter().map(|(p, _)| p.x).min()?;
        let y0 = blocks.iter().map(|(p, _)| p.y).min()?;
        Some(Clip {
            format: CLIP_FORMAT.into(),
            blocks: blocks
                .into_iter()
                .map(|(p, b)| (Pos::new(p.x - x0, p.y - y0, p.z - z0), b.clone()))
                .collect(),
        })
    }

    /// Size on the floor: (width, height).
    pub fn size(&self) -> (i32, i32) {
        let w = self.blocks.iter().map(|(p, _)| p.x).max().unwrap_or(0) + 1;
        let h = self.blocks.iter().map(|(p, _)| p.y).max().unwrap_or(0) + 1;
        (w, h)
    }

    /// The clip turned clockwise (seen from above), still starting at 0,0.
    pub fn rotated(&self) -> Clip {
        // Clockwise: east becomes south, (x, y) -> (-y, x).
        let turn = |x: i32, y: i32| (-y, x);
        let blocks: Vec<(Pos, Block)> = self
            .blocks
            .iter()
            .map(|(p, b)| {
                let (x, y) = turn(p.x, p.y);
                let b = match b {
                    Block::Cover { dx, dy } => {
                        let (dx, dy) = turn(*dx, *dy);
                        Block::Cover { dx, dy }
                    }
                    b => b.rotated(),
                };
                (Pos::new(x, y, p.z), b)
            })
            .collect();
        let x0 = blocks.iter().map(|(p, _)| p.x).min().unwrap_or(0);
        let y0 = blocks.iter().map(|(p, _)| p.y).min().unwrap_or(0);
        Clip {
            format: self.format.clone(),
            blocks: blocks
                .into_iter()
                .map(|(p, b)| (Pos::new(p.x - x0, p.y - y0, p.z), b))
                .collect(),
        }
    }

    /// The edit pasting the clip with its origin at `at`. Fails if it would
    /// overlap a device that is not being replaced, or leave the layers.
    pub fn paste(&self, world: &World, at: Pos) -> Result<Edit, String> {
        let target = |p: &Pos| Pos::new(at.x + p.x, at.y + p.y, at.z + p.z);
        let top = self.blocks.iter().map(|(p, _)| p.z).max().unwrap_or(0);
        if at.z < 0 || at.z + top > MAX_LAYER {
            return Err(format!("katman {}'ın üstüne çıkıyor", MAX_LAYER));
        }
        // Devices under the paste are removed whole, so they must lie
        // entirely under it.
        let mut edits = Vec::new();
        let cells: std::collections::BTreeSet<Pos> =
            self.blocks.iter().map(|(p, _)| target(p)).collect();
        let mut removed = std::collections::BTreeSet::new();
        for c in &cells {
            if let Some((anchor, d, f)) = world.device_at(*c) {
                if removed.insert(anchor) {
                    let l = device::layout(d, anchor, f);
                    if !l.cells.iter().all(|c| cells.contains(c)) {
                        return Err(format!("{} cihazının üstüne yapıştırılamaz", d.name()));
                    }
                    edits.extend(world.remove(anchor));
                }
            }
        }
        for (p, b) in &self.blocks {
            edits.push(Edit::Set {
                pos: target(p),
                block: Some(b.clone()),
            });
        }
        Ok(Edit::Batch(edits))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("clips serialize")
    }

    /// A clip from text, if it is one (e.g. from the system clipboard).
    pub fn from_json(text: &str) -> Option<Clip> {
        let c: Clip = serde_json::from_str(text.trim()).ok()?;
        (c.format == CLIP_FORMAT).then_some(c)
    }
}

/// Move the blocks of `region` by (dx, dy): cut and paste in one edit.
pub fn move_region(world: &World, region: Region, dx: i32, dy: i32) -> Result<Edit, String> {
    let clip = Clip::copy(world, region).ok_or("seçim boş")?;
    let origin = clip_origin(world, region).ok_or("seçim boş")?;
    let cut = region.clear(world);
    let mut after = world.clone();
    after.apply(cut.clone());
    let paste = clip.paste(&after, Pos::new(origin.x + dx, origin.y + dy, origin.z))?;
    Ok(Edit::Batch(vec![cut, paste]))
}

/// Where a region's clip came from: its lowest corner in the world.
pub fn clip_origin(world: &World, region: Region) -> Option<Pos> {
    let b = region.blocks(world);
    Some(Pos::new(
        b.iter().map(|(p, _)| p.x).min()?,
        b.iter().map(|(p, _)| p.y).min()?,
        b.iter().map(|(p, _)| p.z).min()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{Dir, Gate};
    use crate::device::Device;

    fn sample() -> World {
        let mut w = World::new("t");
        w.apply(Edit::Set {
            pos: Pos::new(1, 1, 0),
            block: Some(Block::Wire),
        });
        w.apply(Edit::Set {
            pos: Pos::new(2, 1, 0),
            block: Some(Block::Gate {
                gate: Gate::Not,
                facing: Dir::East,
            }),
        });
        w.apply(Edit::Set {
            pos: Pos::new(2, 1, 1),
            block: Some(Block::Via),
        });
        let e = w.place_device(
            Device::Rom {
                addr: 1,
                data: 2,
                words: vec![1, 2],
            },
            Pos::new(1, 3, 0),
            Dir::East,
        );
        w.apply(e.unwrap());
        w
    }

    #[test]
    fn copy_paste_round_trip() {
        let w = sample();
        let r = Region::between(Pos::new(0, 0, 0), Pos::new(10, 10, 0));
        let clip = Clip::copy(&w, r).unwrap();
        assert_eq!(clip.blocks.len(), w.len());
        let mut other = World::new("o");
        other.apply(clip.paste(&other, Pos::new(100, -5, 2)).unwrap());
        assert_eq!(other.len(), w.len());
        // Same circuit, shifted: the ROM is a working device there too.
        assert!(other.device_at(Pos::new(101, -3, 2)).is_some());
        assert_eq!(other.get(Pos::new(101, -5, 3)), Some(&Block::Via));
        // Through text, too.
        assert_eq!(Clip::from_json(&clip.to_json()), Some(clip));
        assert_eq!(Clip::from_json("merhaba"), None);
    }

    #[test]
    fn partial_devices_are_left_out() {
        let w = sample();
        // The ROM is 3 wide from (1,3); this cuts through it.
        let r = Region::between(Pos::new(0, 0, 0), Pos::new(2, 3, 0));
        let clip = Clip::copy(&w, r).unwrap();
        assert!(clip
            .blocks
            .iter()
            .all(|(_, b)| !matches!(b, Block::Device { .. } | Block::Cover { .. })));
        let mut w2 = w.clone();
        w2.apply(r.clear(&w));
        assert!(w2.device_at(Pos::new(1, 3, 0)).is_some(), "ROM untouched");
    }

    #[test]
    fn rotation_keeps_devices_whole() {
        let w = sample();
        let clip = Clip::copy(&w, Region::between(Pos::new(0, 0, 0), Pos::new(10, 10, 0))).unwrap();
        let mut c = clip.clone();
        for turns in 1..=4 {
            c = c.rotated();
            let mut o = World::new("o");
            o.apply(c.paste(&o, Pos::new(0, 0, 0)).unwrap());
            // Every cover points at a device anchor whose layout includes it.
            for (p, b) in o.blocks() {
                if matches!(b, Block::Cover { .. }) {
                    let (anchor, d, f) = o.device_at(p).expect("cover without device");
                    assert!(
                        device::layout(d, anchor, f).cells.contains(&p),
                        "turn {turns}"
                    );
                }
            }
            crate::compile::compile(&o).unwrap();
        }
        assert_eq!(c, clip, "four turns is the identity");
    }

    #[test]
    fn paste_over_device_and_move() {
        let w = sample();
        // A single wire pasted onto the ROM is refused.
        let wire = Clip {
            format: CLIP_FORMAT.into(),
            blocks: vec![(Pos::new(0, 0, 0), Block::Wire)],
        };
        assert!(wire.paste(&w, Pos::new(2, 4, 0)).is_err());
        // Moving everything right by 5.
        let r = Region::between(Pos::new(0, 0, 0), Pos::new(10, 10, 0));
        let mut m = w.clone();
        m.apply(move_region(&w, r, 5, 0).unwrap());
        assert_eq!(m.len(), w.len());
        assert_eq!(m.get(Pos::new(6, 1, 0)), Some(&Block::Wire));
        assert!(m.device_at(Pos::new(6, 3, 0)).is_some());
        assert!(m.get(Pos::new(1, 1, 0)).is_none());
        // Moving onto itself (overlapping) works too.
        let mut m2 = w.clone();
        m2.apply(move_region(&w, r, 1, 1).unwrap());
        assert_eq!(m2.len(), w.len());
    }
}
