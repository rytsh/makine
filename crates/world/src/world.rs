//! The world: parts placed freely on a plane and wires from pin to pin,
//! like the level editor, changed only through edits.
//!
//! Every part has an id that stays the same while it exists, and every
//! input pin takes at most one wire, so a wire is known by the pin it goes
//! into. An edit sets one part or one wire (or several, in a batch); the
//! last write wins. That keeps players editing the same world in step:
//! applying everyone's edits in the host's order gives the same world.

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};

use serde::{Deserialize, Serialize};

use crate::part::Kind;

/// A part's id.
pub type Id = u64;

/// A point on the plane, in canvas units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pos {
    pub x: f32,
    pub y: f32,
}

impl Pos {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Far enough from the origin is refused, and so is NaN.
    pub fn in_bounds(self) -> bool {
        self.x.abs() <= MAX_COORD && self.y.abs() <= MAX_COORD
    }
}

/// Parts may be placed within ±MAX_COORD on both axes.
pub const MAX_COORD: f32 = 1.0e7;

/// A part and where its top-left corner is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    pub kind: Kind,
    pub pos: Pos,
}

/// A pin of a part: its id and the pin's index among its inputs or
/// outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct End {
    pub part: Id,
    pub pin: usize,
}

/// A live schematic module. Members keep their ids, positions and wires;
/// collapsing is a local view preference, never a simulation change.
/// Nested modules are represented by nested member sets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub members: Vec<Id>,
    /// Where the module is drawn when closed. It stays put while its
    /// insides are rearranged (none: at its members' top-left corner).
    #[serde(default)]
    pub pos: Option<Pos>,
}

/// Presentation of a wire, independent of its electrical connection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WireStyle {
    pub name: String,
    /// Manual waypoints in world coordinates (empty: a curved wire).
    pub points: Vec<Pos>,
}

impl End {
    pub const fn new(part: Id, pin: usize) -> Self {
        Self { part, pin }
    }
}

/// A change to the world.
///
/// Edits may come from files or other players, so the world checks them:
/// parts that make no sense are ignored. A wire whose part was removed (by
/// someone else, at the same time) stays stored but connects nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    /// Put a part with this id (replacing it), or remove it with `None`.
    Part {
        id: Id,
        part: Option<Placed>,
    },
    /// Wire output `from` into input `to`, or cut the wire into `to`.
    Wire {
        to: End,
        from: Option<End>,
    },
    Module {
        id: Id,
        module: Option<Module>,
    },
    Style {
        to: End,
        style: Option<WireStyle>,
    },
    /// Several edits applied together, undone together.
    Batch(Vec<Edit>),
}

/// One thing an edit changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key {
    Part(Id),
    Wire(End),
    Module(Id),
    Style(End),
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Part(Placed),
    Wire(End),
    Module(Module),
    Style(WireStyle),
}

/// What an edit changed: (thing, before, after). Undoing it restores only
/// things that still are what the edit left, so it never overwrites what
/// someone else did since (see [`World::revert`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Change {
    items: Vec<(Key, Option<Value>, Option<Value>)>,
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub name: String,
    parts: BTreeMap<Id, Placed>,
    /// Wires by the input they go into: `to -> from`. Stored as a list (JSON
    /// map keys must be strings).
    #[serde(with = "wire_list")]
    wires: BTreeMap<End, End>,
    #[serde(default)]
    modules: BTreeMap<Id, Module>,
    #[serde(default, with = "style_list")]
    styles: BTreeMap<End, WireStyle>,
    /// Bumped on every change, so views and compiled circuits know when to
    /// refresh.
    #[serde(skip)]
    revision: u64,
    /// Bumped only on changes that change the circuit (not on moving a
    /// part, or flipping a switch).
    #[serde(skip)]
    circuit_revision: u64,
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

    /// Changes only when the circuit does (see [`World::revision`]).
    pub fn circuit_revision(&self) -> u64 {
        self.circuit_revision
    }

    pub fn part(&self, id: Id) -> Option<&Placed> {
        self.parts.get(&id)
    }

    pub fn parts(&self) -> impl Iterator<Item = (Id, &Placed)> {
        self.parts.iter().map(|(id, p)| (*id, p))
    }

    /// Every wire as (from, to), including wires whose parts are gone.
    pub fn wires(&self) -> impl Iterator<Item = (End, End)> + '_ {
        self.wires.iter().map(|(to, from)| (*from, *to))
    }

    /// The output wired into input `to`.
    pub fn wire_into(&self, to: End) -> Option<End> {
        self.wires.get(&to).copied()
    }

    pub fn modules(&self) -> impl Iterator<Item = (Id, &Module)> {
        self.modules.iter().map(|(id, m)| (*id, m))
    }

    pub fn module(&self, id: Id) -> Option<&Module> {
        self.modules.get(&id)
    }

    /// Top-left corner of parts `ids` (where a new module over them goes).
    pub fn corner(&self, ids: &[Id]) -> Option<Pos> {
        ids.iter()
            .filter_map(|id| self.part(*id).map(|p| p.pos))
            .reduce(|a, b| Pos::new(a.x.min(b.x), a.y.min(b.y)))
    }

    pub fn wire_style(&self, to: End) -> Option<&WireStyle> {
        self.styles.get(&to)
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// An id no part has, different on every call and on every machine
    /// (players pick ids without asking each other).
    pub fn new_id(&self) -> Id {
        loop {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(self.revision);
            h.write_usize(self.parts.len());
            // Small enough to be exact in JSON numbers everywhere.
            let id = h.finish() >> 11;
            if id != 0 && !self.parts.contains_key(&id) && !self.modules.contains_key(&id) {
                return id;
            }
        }
    }

    /// The edit removing part `id` and every wire into or out of it.
    pub fn remove(&self, id: Id) -> Edit {
        let mut v = vec![Edit::Part { id, part: None }];
        v.extend(
            self.wires
                .iter()
                .filter(|(to, from)| to.part == id || from.part == id)
                .map(|(to, _)| Edit::Wire {
                    to: *to,
                    from: None,
                }),
        );
        v.extend(
            self.wires
                .iter()
                .filter(|(to, from)| to.part == id || from.part == id)
                .map(|(to, _)| Edit::Style {
                    to: *to,
                    style: None,
                }),
        );
        Edit::Batch(v)
    }

    /// Apply an edit and record what it changed, for undo.
    pub fn apply_change(&mut self, edit: Edit) -> Change {
        let mut items: Vec<(Key, Option<Value>, Option<Value>)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        self.record(edit, &mut |key, before| {
            // A thing set twice keeps its first `before`.
            if seen.insert(key) {
                items.push((key, before, None));
            }
        });
        for i in &mut items {
            i.2 = self.get(i.0);
        }
        items.retain(|(_, before, after)| before != after);
        Change { items }
    }

    fn get(&self, key: Key) -> Option<Value> {
        match key {
            Key::Part(id) => self.parts.get(&id).cloned().map(Value::Part),
            Key::Wire(to) => self.wires.get(&to).copied().map(Value::Wire),
            Key::Module(id) => self.modules.get(&id).cloned().map(Value::Module),
            Key::Style(to) => self.styles.get(&to).cloned().map(Value::Style),
        }
    }

    fn record(&mut self, edit: Edit, seen: &mut impl FnMut(Key, Option<Value>)) {
        match edit {
            Edit::Part { id, part } => {
                seen(Key::Part(id), self.get(Key::Part(id)));
                self.put_part(id, part);
            }
            Edit::Wire { to, from } => {
                seen(Key::Wire(to), self.get(Key::Wire(to)));
                self.put_wire(to, from);
            }
            Edit::Module { id, module } => {
                seen(Key::Module(id), self.get(Key::Module(id)));
                self.put_module(id, module);
            }
            Edit::Style { to, style } => {
                seen(Key::Style(to), self.get(Key::Style(to)));
                self.put_style(to, style);
            }
            Edit::Batch(edits) => {
                for e in edits {
                    self.record(e, seen);
                }
            }
        }
    }

    /// The edit undoing `change` (`back`) or doing it again (`!back`),
    /// limited to things nobody has changed since. `None` if there is
    /// nothing left to do.
    pub fn revert(&self, change: &Change, back: bool) -> Option<Edit> {
        let edits: Vec<Edit> = change
            .items
            .iter()
            .filter_map(|(key, before, after)| {
                let (expect, to) = if back {
                    (after, before)
                } else {
                    (before, after)
                };
                (self.get(*key) == *expect).then(|| match (key, to.clone()) {
                    (Key::Part(id), v) => Edit::Part {
                        id: *id,
                        part: match v {
                            Some(Value::Part(p)) => Some(p),
                            _ => None,
                        },
                    },
                    (Key::Wire(end), v) => Edit::Wire {
                        to: *end,
                        from: match v {
                            Some(Value::Wire(f)) => Some(f),
                            _ => None,
                        },
                    },
                    (Key::Module(id), v) => Edit::Module {
                        id: *id,
                        module: match v {
                            Some(Value::Module(m)) => Some(m),
                            _ => None,
                        },
                    },
                    (Key::Style(to), v) => Edit::Style {
                        to: *to,
                        style: match v {
                            Some(Value::Style(s)) => Some(s),
                            _ => None,
                        },
                    },
                })
            })
            .collect();
        (!edits.is_empty()).then_some(Edit::Batch(edits))
    }

    /// Apply an edit.
    pub fn apply(&mut self, edit: Edit) {
        match edit {
            Edit::Part { id, part } => self.put_part(id, part),
            Edit::Wire { to, from } => self.put_wire(to, from),
            Edit::Module { id, module } => self.put_module(id, module),
            Edit::Style { to, style } => self.put_style(to, style),
            Edit::Batch(edits) => edits.into_iter().for_each(|e| self.apply(e)),
        }
    }

    fn put_part(&mut self, id: Id, part: Option<Placed>) {
        let part = match part {
            Some(p) if p.pos.in_bounds() => match p.kind.sanitized() {
                Some(kind) => Some(Placed { kind, pos: p.pos }),
                None => return,
            },
            Some(_) => return,
            None => None,
        };
        let old = match &part {
            Some(p) => self.parts.insert(id, p.clone()),
            None => self.parts.remove(&id),
        };
        if old == part {
            return;
        }
        self.revision += 1;
        let same = match (&old, &part) {
            (Some(a), Some(b)) => a.kind.same_circuit(&b.kind),
            _ => false,
        };
        if !same {
            self.circuit_revision += 1;
        }
    }

    fn put_wire(&mut self, to: End, from: Option<End>) {
        let old = match from {
            Some(f) => self.wires.insert(to, f),
            None => self.wires.remove(&to),
        };
        if old != from {
            self.revision += 1;
            self.circuit_revision += 1;
        }
    }

    fn put_module(&mut self, id: Id, module: Option<Module>) {
        let module = module.map(|mut m| {
            m.name = m
                .name
                .chars()
                .filter(|c| !c.is_control())
                .take(48)
                .collect();
            m.members.sort_unstable();
            m.members.dedup();
            m
        });
        if module.as_ref().is_some_and(|m| {
            m.name.trim().is_empty()
                || m.members.len() > 100_000
                || m.pos.is_some_and(|p| !p.in_bounds())
        }) {
            return;
        }
        let old = match &module {
            Some(m) => self.modules.insert(id, m.clone()),
            None => self.modules.remove(&id),
        };
        if old != module {
            self.revision += 1;
        }
    }

    fn put_style(&mut self, to: End, style: Option<WireStyle>) {
        if style
            .as_ref()
            .is_some_and(|s| s.points.len() > 128 || s.points.iter().any(|p| !p.in_bounds()))
        {
            return;
        }
        let style = style.map(|mut s| {
            s.name = s
                .name
                .chars()
                .filter(|c| !c.is_control())
                .take(48)
                .collect();
            s
        });
        let old = match &style {
            Some(s) => self.styles.insert(to, s.clone()),
            None => self.styles.remove(&to),
        };
        if old != style {
            self.revision += 1;
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("worlds serialize")
    }

    /// A world from JSON, checked the way edits are.
    pub fn from_json(text: &str) -> Result<World, String> {
        let raw: World = serde_json::from_str(text).map_err(|e| e.to_string())?;
        Ok(raw.checked())
    }

    /// The same world with anything that makes no sense left out.
    pub fn checked(self) -> World {
        let mut w = World::new(self.name);
        for (id, p) in self.parts {
            w.put_part(id, Some(p));
        }
        for (to, from) in self.wires {
            w.put_wire(to, Some(from));
        }
        for (id, m) in self.modules {
            w.put_module(id, Some(m));
        }
        for (to, s) in self.styles {
            w.put_style(to, Some(s));
        }
        w
    }
}

mod style_list {
    use super::{End, WireStyle};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;
    pub fn serialize<S: Serializer>(m: &BTreeMap<End, WireStyle>, s: S) -> Result<S::Ok, S::Error> {
        m.iter().collect::<Vec<_>>().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<End, WireStyle>, D::Error> {
        Ok(Vec::<(End, WireStyle)>::deserialize(d)?
            .into_iter()
            .collect())
    }
}

mod wire_list {
    use super::End;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(m: &BTreeMap<End, End>, s: S) -> Result<S::Ok, S::Error> {
        let v: Vec<(End, End)> = m.iter().map(|(to, from)| (*from, *to)).collect();
        v.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<End, End>, D::Error> {
        let v: Vec<(End, End)> = Vec::deserialize(d)?;
        Ok(v.into_iter().map(|(from, to)| (to, from)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::Gate;

    fn put(w: &mut World, kind: Kind, x: f32) -> Id {
        let id = w.new_id();
        w.apply(Edit::Part {
            id,
            part: Some(Placed {
                kind,
                pos: Pos::new(x, 0.0),
            }),
        });
        id
    }

    #[test]
    fn parts_wires_and_undo() {
        let mut w = World::new("w");
        let s = put(&mut w, Kind::Switch { on: true }, 0.0);
        let l = put(&mut w, Kind::Lamp, 100.0);
        assert_ne!(s, l);
        let c = w.apply_change(Edit::Wire {
            to: End::new(l, 0),
            from: Some(End::new(s, 0)),
        });
        assert_eq!(w.wire_into(End::new(l, 0)), Some(End::new(s, 0)));
        let undo = w.revert(&c, true).unwrap();
        w.apply(undo);
        assert_eq!(w.wire_into(End::new(l, 0)), None);
        let redo = w.revert(&c, false).unwrap();
        w.apply(redo);
        assert_eq!(w.wires().count(), 1);

        // Removing a part takes its wires with it.
        let e = w.remove(s);
        w.apply(e);
        assert_eq!(w.len(), 1);
        assert_eq!(w.wires().count(), 0);
    }

    #[test]
    fn undo_keeps_other_players_work() {
        let mut w = World::new("w");
        let a = put(&mut w, Kind::Gate(Gate::And), 0.0);
        let mine = w.apply_change(Edit::Part {
            id: a,
            part: Some(Placed {
                kind: Kind::Gate(Gate::And),
                pos: Pos::new(50.0, 0.0),
            }),
        });
        // Someone else turns it into an OR: my move cannot be undone over it.
        w.apply(Edit::Part {
            id: a,
            part: Some(Placed {
                kind: Kind::Gate(Gate::Or),
                pos: Pos::new(50.0, 0.0),
            }),
        });
        assert_eq!(w.revert(&mine, true), None);
    }

    #[test]
    fn moving_and_switching_keep_the_circuit() {
        let mut w = World::new("w");
        let s = put(&mut w, Kind::Switch { on: false }, 0.0);
        let r = w.circuit_revision();
        w.apply(Edit::Part {
            id: s,
            part: Some(Placed {
                kind: Kind::Switch { on: true },
                pos: Pos::new(10.0, 10.0),
            }),
        });
        assert_eq!(w.circuit_revision(), r);
        put(&mut w, Kind::Lamp, 0.0);
        assert_ne!(w.circuit_revision(), r);
    }

    #[test]
    fn bad_edits_are_ignored() {
        let mut w = World::new("w");
        w.apply(Edit::Part {
            id: 1,
            part: Some(Placed {
                kind: Kind::Lamp,
                pos: Pos::new(f32::NAN, 0.0),
            }),
        });
        w.apply(Edit::Part {
            id: 2,
            part: Some(Placed {
                kind: Kind::Input {
                    name: "bad name".into(),
                    width: 1,
                    value: 0,
                },
                pos: Pos::new(0.0, 0.0),
            }),
        });
        assert!(w.is_empty());
        assert_eq!(w.revision(), 0);
    }

    #[test]
    fn json_round_trip() {
        let mut w = World::new("w");
        let s = put(&mut w, Kind::Switch { on: true }, 0.0);
        let l = put(&mut w, Kind::Lamp, 100.0);
        w.apply(Edit::Wire {
            to: End::new(l, 0),
            from: Some(End::new(s, 0)),
        });
        let back = World::from_json(&w.to_json()).unwrap();
        assert_eq!(back.parts().count(), 2);
        assert_eq!(back.wire_into(End::new(l, 0)), Some(End::new(s, 0)));
        // Worlds from before (a grid of cells) are not read.
        assert!(World::from_json(r#"{"name":"eski","chunks":[]}"#).is_err());
    }

    #[test]
    fn presentation_is_serialized_undoable_and_not_electrical() {
        let mut w = World::new("w");
        let a = put(&mut w, Kind::Lamp, 0.0);
        let r = w.circuit_revision();
        let change = w.apply_change(Edit::Batch(vec![
            Edit::Module {
                id: 99,
                module: Some(Module {
                    name: "ALU".into(),
                    members: vec![a],
                    pos: None,
                }),
            },
            Edit::Style {
                to: End::new(a, 0),
                style: Some(WireStyle {
                    name: "data".into(),
                    points: vec![Pos::new(10.0, 20.0)],
                }),
            },
        ]));
        assert_eq!(w.circuit_revision(), r);
        let back = World::from_json(&w.to_json()).unwrap();
        assert_eq!(back.module(99).unwrap().name, "ALU");
        assert_eq!(back.wire_style(End::new(a, 0)).unwrap().points.len(), 1);
        w.apply(w.revert(&change, true).unwrap());
        assert!(w.module(99).is_none());
        w.apply(w.revert(&change, false).unwrap());
        assert_eq!(w.module(99).unwrap().name, "ALU");
    }
}
