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

    /// Whether a block may be stored here: on a layer, and far enough from
    /// the edge of `i32` that neighbour and device arithmetic never
    /// overflows.
    pub fn in_bounds(self) -> bool {
        (0..=MAX_LAYER).contains(&self.z)
            && (-MAX_COORD..=MAX_COORD).contains(&self.x)
            && (-MAX_COORD..=MAX_COORD).contains(&self.y)
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
/// Cells allowed: x and y within ±MAX_COORD.
pub const MAX_COORD: i32 = 1 << 24;

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

/// A block as it may be stored: devices normalized, covers pointing nearby.
/// `None` for blocks that cannot be stored.
fn sanitize(block: Block) -> Option<Block> {
    match block {
        Block::Device { device, facing } => Some(Block::Device {
            device: device.normalized()?,
            facing,
        }),
        Block::Cover { dx, dy }
            if dx.unsigned_abs() > device::MAX_SPAN as u32
                || dy.unsigned_abs() > device::MAX_SPAN as u32 =>
        {
            None
        }
        b => Some(b),
    }
}

/// A change to the world. Applying an edit returns its inverse, so edits can
/// be undone and replayed; a server would order and broadcast them.
///
/// Edits may come from files or other players, so the world checks them:
/// cells out of bounds and blocks that cannot be stored are ignored. A
/// device whose cells were overwritten keeps working with the pins it has
/// left; a cover without its device is inert.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edit {
    /// Put `block` at `pos` (replacing whatever was there), or clear the cell
    /// with `None`.
    Set { pos: Pos, block: Option<Block> },
    /// Several edits applied together, undone together.
    Batch(Vec<Edit>),
}

/// What an edit changed, cell by cell: (cell, before, after). Undoing it
/// restores only cells that still hold what the edit left there, so it
/// never overwrites what someone else did since (see [`World::revert`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    cells: Vec<(Pos, Option<Block>, Option<Block>)>,
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
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

    /// Apply an edit and record what it changed, for undo.
    pub fn apply_change(&mut self, edit: Edit) -> Change {
        let mut cells: Vec<(Pos, Option<Block>, Option<Block>)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        self.record(edit, &mut |pos, before| {
            // A cell set twice keeps its first `before`.
            if seen.insert(pos) {
                cells.push((pos, before, None));
            }
        });
        for c in &mut cells {
            c.2 = self.get(c.0).cloned();
        }
        cells.retain(|(_, before, after)| before != after);
        Change { cells }
    }

    fn record(&mut self, edit: Edit, seen: &mut impl FnMut(Pos, Option<Block>)) {
        match edit {
            Edit::Set { pos, block } => {
                seen(pos, self.get(pos).cloned());
                self.put(pos, block);
            }
            Edit::Batch(edits) => {
                for e in edits {
                    self.record(e, seen);
                }
            }
        }
    }

    /// The edit undoing `change` (`back`) or doing it again (`!back`),
    /// limited to cells nobody has changed since. `None` if there is
    /// nothing left to do.
    pub fn revert(&self, change: &Change, back: bool) -> Option<Edit> {
        let edits: Vec<Edit> = change
            .cells
            .iter()
            .filter_map(|(pos, before, after)| {
                let (expect, to) = if back {
                    (after, before)
                } else {
                    (before, after)
                };
                (self.get(*pos) == expect.as_ref()).then(|| Edit::Set {
                    pos: *pos,
                    block: to.clone(),
                })
            })
            .collect();
        (!edits.is_empty()).then_some(Edit::Batch(edits))
    }

    /// Apply an edit; returns the edit that undoes it.
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

    /// Write one cell; returns what was there. Refused writes change
    /// nothing and return the cell as it is, so their inverse is a no-op.
    fn put(&mut self, pos: Pos, block: Option<Block>) -> Option<Block> {
        let block = match block {
            Some(b) => match sanitize(b) {
                Some(b) => Some(b),
                None => return self.get(pos).cloned(),
            },
            None => None,
        };
        if !pos.in_bounds() {
            return None;
        }
        if self.get(pos) == block.as_ref() {
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
    /// facing. A cover only counts if the device there really covers it.
    pub fn device_at(&self, pos: Pos) -> Option<(Pos, &Device, Dir)> {
        let anchor = match self.get(pos)? {
            Block::Device { .. } => pos,
            Block::Cover { dx, dy } => Pos::new(pos.x + dx, pos.y + dy, pos.z),
            _ => return None,
        };
        match self.get(anchor)? {
            Block::Device { device, facing }
                if device::local(device, anchor, *facing, pos).is_some() =>
            {
                Some((anchor, device, *facing))
            }
            _ => None,
        }
    }

    /// Whether the block at `pos` connects to a wire on its side `side`.
    /// Unlike [`Block::accepts_wire_from`] this knows device pins.
    pub fn accepts_wire(&self, pos: Pos, side: Dir) -> bool {
        self.pin(pos, side).is_some()
    }

    /// The connection of the block at `pos` on side `side`: `Some(true)`
    /// for an output that drives the wire there, `Some(false)` for an input
    /// or a conductor, `None` if nothing connects on that side.
    pub fn pin(&self, pos: Pos, side: Dir) -> Option<bool> {
        match self.get(pos)? {
            Block::Device { .. } | Block::Cover { .. } => {
                let (anchor, device, facing) = self.device_at(pos)?;
                let uv = device::local(device, anchor, facing, pos)?;
                device::pin_at(device, facing, uv, side)
            }
            b => b.accepts_wire_from(side).then(|| b.drives(side)),
        }
    }

    /// The edit placing `device` with its anchor at `anchor`, or `None` if
    /// a cell it needs is taken or outside the allowed layers.
    pub fn place_device(&self, device: Device, anchor: Pos, facing: Dir) -> Option<Edit> {
        let device = device.normalized()?;
        let l = device::layout(&device, anchor, facing);
        if l.cells
            .iter()
            .any(|&c| !c.in_bounds() || self.get(c).is_some())
        {
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
                .filter(|&c| self.device_at(c).is_some_and(|d| d.0 == anchor))
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
    use super::{sanitize, Block, Chunk, ChunkKey, Pos, CHUNK};
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<ChunkKey, Chunk>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        map.iter().collect::<Vec<_>>().serialize(s)
    }

    /// Chunks come from files and other players: check their shape and
    /// store every block as [`World::put`] would.
    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<ChunkKey, Chunk>, D::Error> {
        let mut out = BTreeMap::new();
        for (key, chunk) in Vec::<(ChunkKey, Chunk)>::deserialize(d)? {
            if chunk.cells.len() != (CHUNK * CHUNK) as usize {
                return Err(Error::custom("bozuk dünya parçası"));
            }
            let origin = Pos::new(
                key.cx.saturating_mul(CHUNK),
                key.cy.saturating_mul(CHUNK),
                key.z,
            );
            if !origin.in_bounds() {
                continue;
            }
            let cells: Vec<Option<Block>> = chunk
                .cells
                .into_iter()
                .map(|b| b.and_then(sanitize))
                .collect();
            let count = cells.iter().filter(|c| c.is_some()).count() as u32;
            if count > 0 {
                out.insert(key, Chunk { cells, count });
            }
        }
        Ok(out)
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
    fn bad_input_is_ignored() {
        let mut w = World::new("t");
        let far = Pos::new(MAX_COORD + 1, 0, 0);
        let undo = w.apply(Edit::Set {
            pos: far,
            block: Some(Block::Wire),
        });
        assert!(w.is_empty());
        w.apply(undo);
        assert!(w.is_empty());
        // Oversized devices are clamped; impossible ones refused.
        w.apply(Edit::Set {
            pos: Pos::new(0, 0, 0),
            block: Some(Block::Device {
                device: Device::Screen { bits: 200 },
                facing: Dir::East,
            }),
        });
        assert!(matches!(
            w.get(Pos::new(0, 0, 0)),
            Some(Block::Device {
                device: Device::Screen { bits: 5 },
                ..
            })
        ));
        let rev = w.revision();
        w.apply(Edit::Set {
            pos: Pos::new(1, 0, 0),
            block: Some(Block::Cover {
                dx: i32::MIN,
                dy: 0,
            }),
        });
        assert!(w.get(Pos::new(1, 0, 0)).is_none());
        // Writing what is already there is not a change.
        w.apply(Edit::Set {
            pos: Pos::new(0, 0, 0),
            block: w.get(Pos::new(0, 0, 0)).cloned(),
        });
        assert_eq!(w.revision(), rev);
    }

    #[test]
    fn broken_devices_are_inert() {
        // A device whose cover was overwritten by a wire, and a stray cover.
        let mut w = World::new("t");
        let e = w.place_device(
            Device::Ram { addr: 2, data: 2 },
            Pos::new(0, 0, 0),
            Dir::East,
        );
        w.apply(e.unwrap());
        w.apply(Edit::Set {
            pos: Pos::new(0, 1, 0),
            block: Some(Block::Wire),
        });
        w.apply(Edit::Set {
            pos: Pos::new(9, 9, 0),
            block: Some(Block::Cover { dx: 1, dy: 0 }),
        });
        assert!(w.device_at(Pos::new(9, 9, 0)).is_none());
        assert!(!w.accepts_wire(Pos::new(9, 9, 0), Dir::West));
        crate::compile::compile(&w).unwrap();
        // Removing the device leaves the wire.
        w.apply(w.remove(Pos::new(0, 0, 0)).unwrap());
        assert_eq!(w.blocks().count(), 2);
    }

    #[test]
    fn corrupt_files() {
        assert!(World::from_json(
            r#"{"name":"x","chunks":[[{"cx":0,"cy":0,"z":0},{"cells":[],"count":5}]]}"#
        )
        .is_err());
        let mut w = World::new("t");
        w.apply(Edit::Set {
            pos: Pos::new(1, 1, 0),
            block: Some(Block::Wire),
        });
        // A wrong count is recomputed.
        let text = w.to_json().replace("\"count\":1", "\"count\":0");
        assert_eq!(World::from_json(&text).unwrap().len(), 1);
    }

    #[test]
    fn undo_keeps_other_players_work() {
        let mut w = World::new("t");
        let (a, b) = (Pos::new(0, 0, 0), Pos::new(1, 0, 0));
        // I paint two wires.
        let mine = w.apply_change(Edit::Batch(vec![
            Edit::Set {
                pos: a,
                block: Some(Block::Wire),
            },
            Edit::Set {
                pos: b,
                block: Some(Block::Wire),
            },
        ]));
        // Someone else turns one into a lamp.
        w.apply(Edit::Set {
            pos: b,
            block: Some(Block::Lamp),
        });
        // Undoing mine removes my wire, not their lamp.
        let undo = w.revert(&mine, true).unwrap();
        w.apply(undo);
        assert_eq!(w.get(a), None);
        assert_eq!(w.get(b), Some(&Block::Lamp));
        // Redo puts my wire back where it is still free.
        let redo = w.revert(&mine, false).unwrap();
        w.apply(redo);
        assert_eq!(w.get(a), Some(&Block::Wire));
        assert_eq!(w.get(b), Some(&Block::Lamp));
        // Nothing left to undo if everything was overwritten.
        w.apply(Edit::Set {
            pos: a,
            block: Some(Block::Via),
        });
        assert_eq!(w.revert(&mine, true), None);
    }

    #[test]
    fn change_records_first_before_and_last_after() {
        let mut w = World::new("t");
        let a = Pos::new(0, 0, 0);
        w.apply(Edit::Set {
            pos: a,
            block: Some(Block::Lamp),
        });
        let c = w.apply_change(Edit::Batch(vec![
            Edit::Set {
                pos: a,
                block: None,
            },
            Edit::Set {
                pos: a,
                block: Some(Block::Wire),
            },
            // Out of bounds: not a change.
            Edit::Set {
                pos: Pos::new(0, 0, 99),
                block: Some(Block::Wire),
            },
        ]));
        assert_eq!(c.cells, vec![(a, Some(Block::Lamp), Some(Block::Wire))]);
        w.apply(w.revert(&c, true).unwrap());
        assert_eq!(w.get(a), Some(&Block::Lamp));
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
