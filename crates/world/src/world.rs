//! The world: a sparse, layered grid of blocks, changed only through edits.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::block::{Block, Dir};
use crate::device::{self, Device};

/// A cell position: `x`, `y` on a layer, `z` the layer (0 = ground).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn step(self, d: Dir) -> Pos {
        let (dx, dy) = d.delta();
        Pos::new(self.x + dx, self.y + dy, self.z)
    }

    pub fn up(self) -> Pos {
        Pos::new(self.x, self.y, self.z + 1)
    }

    pub fn down(self) -> Pos {
        Pos::new(self.x, self.y, self.z - 1)
    }

    fn chunk(self) -> ChunkKey {
        ChunkKey {
            cx: self.x.div_euclid(CHUNK),
            cy: self.y.div_euclid(CHUNK),
            z: self.z,
        }
    }

    fn index(self) -> usize {
        (self.y.rem_euclid(CHUNK) * CHUNK + self.x.rem_euclid(CHUNK)) as usize
    }
}

pub const CHUNK: i32 = 16;
/// Layers allowed: 0 (ground) to MAX_LAYER.
pub const MAX_LAYER: i32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChunkKey {
    pub cx: i32,
    pub cy: i32,
    pub z: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    /// CHUNK×CHUNK cells, row by row.
    cells: Vec<Option<Block>>,
    /// Number of non-empty cells, so empty chunks can be dropped.
    count: u32,
}

impl Chunk {
    fn new() -> Self {
        Self {
            cells: vec![None; (CHUNK * CHUNK) as usize],
            count: 0,
        }
    }
}

/// A change to the world. Applying an edit returns its inverse, so edits can
/// be undone and replayed; a server would order and broadcast them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edit {
    /// Put `block` at `pos` (replacing whatever was there), or clear the cell
    /// with `None`.
    Set { pos: Pos, block: Option<Block> },
    /// Several edits applied together, undone together.
    Batch(Vec<Edit>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub name: String,
    /// Stored as a list of (key, chunk) pairs: JSON maps need string keys,
    /// and a list is also how chunks would be streamed to other players.
    #[serde(with = "chunk_list")]
    chunks: BTreeMap<ChunkKey, Chunk>,
    /// Bumped on every change, so views and compiled circuits know when to
    /// refresh.
    #[serde(skip)]
    revision: u64,
}

impl World {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn get(&self, pos: Pos) -> Option<&Block> {
        self.chunks.get(&pos.chunk())?.cells[pos.index()].as_ref()
    }

    /// Apply an edit; returns the edit that undoes it. Cells outside the
    /// allowed layers are ignored.
    pub fn apply(&mut self, edit: Edit) -> Edit {
        match edit {
            Edit::Set { pos, block } => {
                let old = self.put(pos, block);
                Edit::Set { pos, block: old }
            }
            Edit::Batch(edits) => {
                let mut undo: Vec<Edit> = edits.into_iter().map(|e| self.apply(e)).collect();
                undo.reverse();
                Edit::Batch(undo)
            }
        }
    }

    fn put(&mut self, pos: Pos, block: Option<Block>) -> Option<Block> {
        if !(0..=MAX_LAYER).contains(&pos.z) {
            return block;
        }
        let key = pos.chunk();
        let chunk = self.chunks.entry(key).or_insert_with(Chunk::new);
        let cell = &mut chunk.cells[pos.index()];
        let old = std::mem::replace(cell, block);
        match (old.is_some(), cell.is_some()) {
            (false, true) => chunk.count += 1,
            (true, false) => chunk.count -= 1,
            _ => {}
        }
        if chunk.count == 0 {
            self.chunks.remove(&key);
        }
        self.revision += 1;
        old
    }

    /// Every non-empty cell, in no particular order.
    pub fn blocks(&self) -> impl Iterator<Item = (Pos, &Block)> {
        self.chunks.iter().flat_map(|(k, c)| {
            c.cells.iter().enumerate().filter_map(move |(i, b)| {
                let b = b.as_ref()?;
                let i = i as i32;
                Some((
                    Pos::new(k.cx * CHUNK + i % CHUNK, k.cy * CHUNK + i / CHUNK, k.z),
                    b,
                ))
            })
        })
    }

    pub fn len(&self) -> usize {
        self.chunks.values().map(|c| c.count as usize).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Bounding box of all blocks: (min, max), inclusive.
    pub fn bounds(&self) -> Option<(Pos, Pos)> {
        let mut it = self.blocks().map(|(p, _)| p);
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| {
            (
                Pos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)),
                Pos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)),
            )
        }))
    }

    /// The anchor of the device covering `pos`, with the device and its
    /// facing.
    pub fn device_at(&self, pos: Pos) -> Option<(Pos, &Device, Dir)> {
        let anchor = match self.get(pos)? {
            Block::Device { .. } => pos,
            Block::Cover { dx, dy } => Pos::new(pos.x + dx, pos.y + dy, pos.z),
            _ => return None,
        };
        match self.get(anchor)? {
            Block::Device { device, facing } => Some((anchor, device, *facing)),
            _ => None,
        }
    }

    /// Whether the block at `pos` connects to a wire on its side `side`.
    /// Unlike [`Block::accepts_wire_from`] this knows device pins.
    pub fn accepts_wire(&self, pos: Pos, side: Dir) -> bool {
        match self.get(pos) {
            None => false,
            Some(Block::Device { .. } | Block::Cover { .. }) => {
                let Some((anchor, device, facing)) = self.device_at(pos) else {
                    return false;
                };
                let l = device::layout(device, anchor, facing);
                l.inputs
                    .iter()
                    .chain(&l.outputs)
                    .any(|&(p, d)| p == pos && d == side)
            }
            Some(b) => b.accepts_wire_from(side),
        }
    }

    /// The edit placing `device` with its anchor at `anchor`, or `None` if
    /// a cell it needs is taken or outside the allowed layers.
    pub fn place_device(&self, device: Device, anchor: Pos, facing: Dir) -> Option<Edit> {
        let device = device.clamped();
        let l = device::layout(&device, anchor, facing);
        if !(0..=MAX_LAYER).contains(&anchor.z) || l.cells.iter().any(|&c| self.get(c).is_some()) {
            return None;
        }
        let mut edits: Vec<Edit> = l
            .cells
            .iter()
            .filter(|&&c| c != anchor)
            .map(|&c| Edit::Set {
                pos: c,
                block: Some(Block::Cover {
                    dx: anchor.x - c.x,
                    dy: anchor.y - c.y,
                }),
            })
            .collect();
        edits.push(Edit::Set {
            pos: anchor,
            block: Some(Block::Device { device, facing }),
        });
        Some(Edit::Batch(edits))
    }

    /// The edit clearing the cell at `pos`: the whole device if it is part
    /// of one.
    pub fn remove(&self, pos: Pos) -> Option<Edit> {
        self.get(pos)?;
        let Some((anchor, device, facing)) = self.device_at(pos) else {
            return Some(Edit::Set { pos, block: None });
        };
        let cells = device::layout(device, anchor, facing).cells;
        Some(Edit::Batch(
            cells
                .into_iter()
                .filter(|&c| match self.get(c) {
                    Some(Block::Cover { dx, dy }) => Pos::new(c.x + dx, c.y + dy, c.z) == anchor,
                    Some(Block::Device { .. }) => c == anchor,
                    _ => false,
                })
                .map(|c| Edit::Set {
                    pos: c,
                    block: None,
                })
                .collect(),
        ))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("worlds serialize")
    }

    pub fn from_json(text: &str) -> Result<World, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
}

mod chunk_list {
    use super::{Chunk, ChunkKey};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<ChunkKey, Chunk>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        map.iter().collect::<Vec<_>>().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<ChunkKey, Chunk>, D::Error> {
        Ok(Vec::<(ChunkKey, Chunk)>::deserialize(d)?
            .into_iter()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Gate;

    #[test]
    fn set_get_and_undo() {
        let mut w = World::new("t");
        let p = Pos::new(-3, 20, 1);
        let undo = w.apply(Edit::Set {
            pos: p,
            block: Some(Block::Wire),
        });
        assert_eq!(w.get(p), Some(&Block::Wire));
        assert_eq!(w.len(), 1);
        w.apply(undo);
        assert_eq!(w.get(p), None);
        assert!(w.is_empty(), "empty chunks are dropped");
    }

    #[test]
    fn batch_undo_restores_everything() {
        let mut w = World::new("t");
        let a = Pos::new(0, 0, 0);
        w.apply(Edit::Set {
            pos: a,
            block: Some(Block::Lamp),
        });
        let before = w.clone();
        let undo = w.apply(Edit::Batch(vec![
            Edit::Set {
                pos: a,
                block: Some(Block::Wire),
            },
            Edit::Set {
                pos: a.step(Dir::East),
                block: Some(Block::Wire),
            },
            Edit::Set {
                pos: a,
                block: None,
            },
        ]));
        w.apply(undo);
        assert_eq!(
            w.blocks().collect::<Vec<_>>(),
            before.blocks().collect::<Vec<_>>()
        );
    }

    #[test]
    fn blocks_and_bounds() {
        let mut w = World::new("t");
        let cells = [Pos::new(-17, 5, 0), Pos::new(40, -2, 3), Pos::new(0, 0, 0)];
        for p in cells {
            w.apply(Edit::Set {
                pos: p,
                block: Some(Block::Via),
            });
        }
        let mut got: Vec<Pos> = w.blocks().map(|(p, _)| p).collect();
        got.sort();
        let mut want = cells.to_vec();
        want.sort();
        assert_eq!(got, want);
        assert_eq!(w.bounds(), Some((Pos::new(-17, -2, 0), Pos::new(40, 5, 3))));
    }

    #[test]
    fn layers_are_limited() {
        let mut w = World::new("t");
        w.apply(Edit::Set {
            pos: Pos::new(0, 0, -1),
            block: Some(Block::Wire),
        });
        w.apply(Edit::Set {
            pos: Pos::new(0, 0, MAX_LAYER + 1),
            block: Some(Block::Wire),
        });
        assert!(w.is_empty());
    }

    #[test]
    fn devices_place_and_remove_whole() {
        let mut w = World::new("t");
        let ram = Device::Ram { addr: 2, data: 4 };
        let anchor = Pos::new(5, 5, 0);
        let e = w.place_device(ram.clone(), anchor, Dir::East).unwrap();
        w.apply(e);
        let (len, wide) = ram.size();
        assert_eq!(w.len(), (len * wide) as usize);
        // Every cell knows its device; pins accept wires on the right side.
        let far = Pos::new(5 + len - 1, 5 + wide - 1, 0);
        assert_eq!(w.device_at(far).map(|d| d.0), Some(anchor));
        assert!(w.accepts_wire(anchor, Dir::West));
        assert!(!w.accepts_wire(anchor, Dir::North));
        assert!(w.accepts_wire(Pos::new(5 + len - 1, 5, 0), Dir::East));
        // Overlapping placement is refused.
        assert!(w.place_device(ram, Pos::new(6, 6, 0), Dir::East).is_none());
        // Removing any cell removes the device.
        let e = w.remove(far).unwrap();
        w.apply(e);
        assert!(w.is_empty());
    }

    #[test]
    fn json_round_trip() {
        let mut w = World::new("dünya");
        w.apply(Edit::Set {
            pos: Pos::new(1, 2, 0),
            block: Some(Block::Gate {
                gate: Gate::Xor,
                facing: Dir::North,
            }),
        });
        w.apply(Edit::Set {
            pos: Pos::new(5, 2, 0),
            block: Some(Block::Port {
                name: "a".into(),
                output: false,
                facing: Dir::East,
            }),
        });
        let back = World::from_json(&w.to_json()).unwrap();
        assert_eq!(
            back.blocks().collect::<Vec<_>>(),
            w.blocks().collect::<Vec<_>>()
        );
        assert_eq!(back.name, "dünya");
    }
}
