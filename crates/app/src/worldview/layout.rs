//! Where world parts are drawn and where their pins are, like the
//! canvas's layout: each part is drawn as a gate symbol, a box with named
//! pins, or (switches, lamps, clocks, inputs, outputs, screens, displays)
//! its own face.

use egui::{pos2, vec2, Pos2, Rect, Vec2};
use makine_sim::{Library, Part, PartKind, PartPins, Pin};
use makine_world::{Device, End, Gate, Id, Kind, World};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::design::{geometry, Geometry, Symbol, GATE_H, HEADER, PIN_GAP};

/// Size of the faces drawn by the world view itself.
pub const SWITCH: Vec2 = vec2(56.0, 40.0);
pub const LAMP: Vec2 = vec2(44.0, 44.0);
pub const PORT: Vec2 = vec2(84.0, 40.0);
/// Pixels of a screen device, at zoom 1.
pub const SCREEN_PX: f32 = 6.0;

/// How a world part is drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Face {
    /// A gate symbol or a box, exactly as on the canvas.
    Canvas(Part),
    Switch,
    Lamp,
    Clock,
    Input,
    Output,
    Screen,
    Display,
}

/// A part as placed on screen.
pub struct Shape {
    pub face: Face,
    pub geometry: Geometry,
    pub pins: PartPins,
}

/// The canvas part a gate or library part is drawn as.
fn canvas_part(kind: &Kind) -> Option<Part> {
    Some(match kind {
        Kind::Gate(Gate::Nand) => Part::new(PartKind::Nand),
        Kind::Gate(g) => {
            let name = match g {
                Gate::Not => "Not",
                Gate::And => "And",
                Gate::Or => "Or",
                Gate::Xor => "Xor",
                Gate::Nor => "Nor",
                Gate::Nand => unreachable!(),
            };
            Part::new(PartKind::Chip(name.into()))
        }
        Kind::Part(p) => p.clone(),
        // Memories and the keyboard are boxes named after the device.
        Kind::Device(
            d @ (Device::Ram { .. }
            | Device::Rom { .. }
            | Device::Keyboard
            | Device::Register { .. }
            | Device::ProgramRom { .. }),
        ) => Part::new(PartKind::Chip(d.name())),
        _ => return None,
    })
}

/// A part's shape: what it looks like and where its pins are.
pub fn shape(kind: &Kind, lib: &Library, symbols: bool) -> Option<Shape> {
    let pins = kind.pins(lib)?;
    let left = vec2(-1.0, 0.0);
    let col = |size: Vec2, pins: &PartPins| Geometry {
        symbol: Symbol::Box,
        size,
        inputs: (0..pins.inputs.len())
            .map(|i| {
                let y = size.y * (i as f32 + 1.0) / (pins.inputs.len() as f32 + 1.0);
                (vec2(0.0, y), left)
            })
            .collect(),
        outputs: (0..pins.outputs.len())
            .map(|i| {
                let y = size.y * (i as f32 + 1.0) / (pins.outputs.len() as f32 + 1.0);
                vec2(size.x, y)
            })
            .collect(),
    };
    let (face, geometry) = match kind {
        Kind::Switch { .. } => (Face::Switch, col(SWITCH, &pins)),
        Kind::Clock { .. } => (Face::Clock, col(SWITCH, &pins)),
        Kind::Lamp => (Face::Lamp, col(LAMP, &pins)),
        Kind::Input { .. } => (Face::Input, col(PORT, &pins)),
        Kind::Output { .. } => (Face::Output, col(PORT, &pins)),
        Kind::Device(Device::Screen { bits }) => {
            // Pin names on the left, the pixels on the right.
            let side = (1u32 << bits) as f32 * SCREEN_PX + 16.0;
            let h = side.max(HEADER + pins.inputs.len() as f32 * PIN_GAP);
            (Face::Screen, col(vec2(side + 56.0, h), &pins))
        }
        Kind::Device(Device::Display { .. }) => {
            (Face::Display, col(vec2(100.0, GATE_H + 16.0), &pins))
        }
        _ => {
            let part = canvas_part(kind)?;
            let g = geometry(&part.kind, &pins, &label(kind), symbols);
            (Face::Canvas(part), g)
        }
    };
    Some(Shape {
        face,
        geometry,
        pins,
    })
}

/// The name drawn on a part.
pub fn label(kind: &Kind) -> String {
    match kind {
        Kind::Part(p) => crate::canvas::part_label(p),
        Kind::Input { name, .. } | Kind::Output { name, .. } => name.clone(),
        k => k.label(),
    }
}

/// Every part's shape and the camera, for one frame.
pub struct Layout {
    pub origin: Pos2,
    pub zoom: f32,
    pub parts: Vec<(Id, Pos2, Option<Shape>)>,
    index: HashMap<Id, usize>,
    pub modules: Vec<ModuleShape>,
    pub owners: HashMap<Id, Id>,
    source_proxy: HashMap<End, Pos2>,
    sink_proxy: HashMap<End, Pos2>,
    /// Parts outside the module being looked into: not drawn at all.
    outside: HashSet<Id>,
    /// Connections to the outside of the module being looked into.
    pub ports: Vec<Port>,
}

/// Where a net crosses the edge of the module being looked into: on the
/// left for signals coming in, on the right for signals going out.
pub struct Port {
    /// The pin, in world units.
    pub at: Pos2,
    pub name: String,
    pub width: u8,
    pub incoming: bool,
    /// The output driving the net (for its value).
    pub source: End,
}

/// Size of a port tab, in world units.
pub const PORT_TAB: Vec2 = vec2(150.0, 24.0);

impl Port {
    pub fn rect(&self) -> Rect {
        let min = if self.incoming {
            self.at - vec2(PORT_TAB.x, PORT_TAB.y / 2.0)
        } else {
            self.at - vec2(0.0, PORT_TAB.y / 2.0)
        };
        Rect::from_min_size(min, PORT_TAB)
    }
}

pub struct ModuleShape {
    pub id: Id,
    pub name: String,
    pub rect: Rect,
    pub closed: bool,
    pub inputs: Vec<(End, String, u8)>,
    pub outputs: Vec<(End, String, u8)>,
}

impl Layout {
    pub fn new(world: &World, lib: &Library, origin: Pos2, zoom: f32, symbols: bool) -> Self {
        let parts: Vec<_> = world
            .parts()
            .map(|(id, p)| (id, pos2(p.pos.x, p.pos.y), shape(&p.kind, lib, symbols)))
            .collect();
        let index = parts
            .iter()
            .enumerate()
            .map(|(i, (id, _, _))| (*id, i))
            .collect();
        Self {
            origin,
            zoom,
            parts,
            index,
            modules: Vec::new(),
            owners: HashMap::new(),
            source_proxy: HashMap::new(),
            sink_proxy: HashMap::new(),
            outside: HashSet::new(),
            ports: Vec::new(),
        }
    }

    /// Project collapsed modules without changing the circuit. A boundary
    /// pin still addresses the actual inner pin, so reconnecting it edits
    /// the same wire whether its module is open or closed.
    ///
    /// With a `scope`, only that module's insides are shown, and the nets
    /// crossing its edge become ports.
    pub fn project(mut self, world: &World, collapsed: &BTreeSet<Id>, scope: Option<Id>) -> Self {
        let inside: Option<BTreeSet<Id>> = scope
            .and_then(|s| world.module(s))
            .map(|m| m.members.iter().copied().collect());
        if let Some(inside) = &inside {
            self.outside = self
                .parts
                .iter()
                .map(|(id, _, _)| *id)
                .filter(|id| !inside.contains(id))
                .collect();
        }
        let mut groups: Vec<_> = world
            .modules()
            .filter(|(id, m)| {
                Some(*id) != scope
                    && inside.as_ref().is_none_or(|inside| {
                        m.members
                            .iter()
                            .all(|p| inside.contains(p) || world.part(*p).is_none())
                    })
            })
            .collect();
        groups.sort_by_key(|(id, m)| (std::cmp::Reverse(m.members.len()), *id));
        for (id, m) in groups {
            let members: BTreeSet<_> = m
                .members
                .iter()
                .copied()
                .filter(|p| world.part(*p).is_some())
                .collect();
            if members.is_empty() || members.iter().any(|p| self.owners.contains_key(p)) {
                continue;
            }
            let Some(bounds) = members
                .iter()
                .filter_map(|p| self.part_rect(*p))
                .reduce(|a, b| a.union(b))
            else {
                continue;
            };
            let closed = collapsed.contains(&id);
            let mut ins = std::collections::BTreeMap::<End, Vec<End>>::new();
            let mut outs = BTreeSet::new();
            for (from, to) in world.wires() {
                let f = members.contains(&from.part);
                let t = members.contains(&to.part);
                if f && !t {
                    outs.insert(from);
                }
                if t && !f {
                    ins.entry(from).or_default().push(to);
                }
            }
            let pin_label = |e: End, output: bool| {
                let part = world.part(e.part)?;
                let pin = if output {
                    self.output_pin(e)
                } else {
                    self.input_pin(e)
                }?;
                let named = if output {
                    world.wires().find_map(|(from, to)| {
                        (from == e)
                            .then(|| world.wire_style(to))
                            .flatten()
                            .filter(|s| !s.name.is_empty())
                            .map(|s| s.name.clone())
                    })
                } else {
                    None
                };
                Some((
                    e,
                    named.unwrap_or_else(|| format!("{} / {}", label(&part.kind), pin.name)),
                    pin.width,
                ))
            };
            let inputs: Vec<_> = ins
                .iter()
                .filter_map(|(from, ends)| {
                    let (_, name, width) = pin_label(*from, true)?;
                    Some((ends[0], name, width))
                })
                .collect();
            let outputs: Vec<_> = outs
                .into_iter()
                .filter_map(|e| pin_label(e, true))
                .collect();
            let rect = if closed {
                let rows = inputs.len().max(outputs.len()).max(1);
                Rect::from_min_size(bounds.min, vec2(280.0, 38.0 + rows as f32 * 24.0))
            } else {
                let mut r = bounds.expand(26.0);
                r.min.y -= 30.0;
                r
            };
            if closed {
                for p in &members {
                    self.owners.insert(*p, id);
                }
                for (i, (e, _, _)) in inputs.iter().enumerate() {
                    let p = pos2(rect.left(), rect.top() + 42.0 + i as f32 * 24.0);
                    for ends in ins.values().filter(|ends| ends.contains(e)) {
                        for end in ends {
                            self.sink_proxy.insert(*end, p);
                        }
                    }
                }
                for (i, (e, _, _)) in outputs.iter().enumerate() {
                    self.source_proxy
                        .insert(*e, pos2(rect.right(), rect.top() + 42.0 + i as f32 * 24.0));
                }
            }
            self.modules.push(ModuleShape {
                id,
                name: m.name.clone(),
                rect,
                closed,
                inputs,
                outputs,
            });
        }
        // Enclose projected children, not the coordinates of their hidden
        // thousands of gates. Work inside-out for nested modules.
        for i in (0..self.modules.len()).rev() {
            if self.modules[i].closed {
                continue;
            }
            let Some(m) = world.module(self.modules[i].id) else {
                continue;
            };
            let member: BTreeSet<_> = m.members.iter().copied().collect();
            let raw = member
                .iter()
                .filter(|id| self.visible(**id))
                .filter_map(|id| self.part_rect(*id));
            let children = self
                .modules
                .iter()
                .skip(i + 1)
                .filter(|c| {
                    world
                        .module(c.id)
                        .is_some_and(|c| c.members.iter().all(|id| member.contains(id)))
                })
                .map(|c| c.rect);
            if let Some(bounds) = raw.chain(children).reduce(|a, b| a.union(b)) {
                let mut r = bounds.expand(26.0);
                r.min.y -= 30.0;
                self.modules[i].rect = r;
            }
        }
        if inside.is_some() {
            self.place_ports(world);
        }
        self
    }

    /// The name of the net driven by output `e`: its wire's name, or the
    /// part and pin.
    fn net_name(&self, world: &World, e: End) -> Option<(String, u8)> {
        let part = world.part(e.part)?;
        let pin = self.output_pin(e)?;
        let named = world.wires().find_map(|(from, to)| {
            (from == e)
                .then(|| world.wire_style(to))
                .flatten()
                .filter(|s| !s.name.is_empty())
                .map(|s| s.name.clone())
        });
        Some((
            named.unwrap_or_else(|| format!("{} / {}", label(&part.kind), pin.name)),
            pin.width,
        ))
    }

    /// World position of input `e` as shown (a closed module's pin if it
    /// is inside one).
    fn world_sink(&self, e: End) -> Option<Pos2> {
        if let Some(p) = self.sink_proxy.get(&e) {
            return Some(*p);
        }
        let (at, s) = self.find(e.part)?;
        Some(at + s.geometry.inputs.get(e.pin)?.0)
    }

    fn world_source(&self, e: End) -> Option<Pos2> {
        if let Some(p) = self.source_proxy.get(&e) {
            return Some(*p);
        }
        let (at, s) = self.find(e.part)?;
        Some(at + *s.geometry.outputs.get(e.pin)?)
    }

    /// Ports for the nets crossing the edge of the module looked into,
    /// each near the pins it connects, without overlapping.
    fn place_ports(&mut self, world: &World) {
        let Some(bounds) = self.bounds() else { return };
        let mut incoming = BTreeMap::<End, Vec<End>>::new();
        let mut outgoing = BTreeMap::<End, Vec<End>>::new();
        for (from, to) in world.wires() {
            match (
                self.outside.contains(&from.part),
                self.outside.contains(&to.part),
            ) {
                (true, false) => incoming.entry(from).or_default().push(to),
                (false, true) => outgoing.entry(from).or_default().push(to),
                _ => {}
            }
        }
        const GAP: f32 = 34.0;
        const MARGIN: f32 = 120.0;
        let mean = |ys: Vec<f32>| {
            if ys.is_empty() {
                bounds.center().y
            } else {
                ys.iter().sum::<f32>() / ys.len() as f32
            }
        };
        let column = |mut v: Vec<(f32, End, Vec<End>)>| {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut last = f32::NEG_INFINITY;
            for p in &mut v {
                p.0 = p.0.max(last + GAP);
                last = p.0;
            }
            v
        };
        let ins = column(
            incoming
                .into_iter()
                .map(|(from, to)| {
                    let y = mean(
                        to.iter()
                            .filter_map(|e| self.world_sink(*e))
                            .map(|p| p.y)
                            .collect(),
                    );
                    (y, from, to)
                })
                .collect(),
        );
        let outs = column(
            outgoing
                .into_iter()
                .map(|(from, to)| {
                    (
                        self.world_source(from).map_or(bounds.center().y, |p| p.y),
                        from,
                        to,
                    )
                })
                .collect(),
        );
        for (y, from, _) in ins {
            let at = pos2(bounds.left() - MARGIN, y);
            self.source_proxy.insert(from, at);
            if let Some((name, width)) = self.net_name(world, from) {
                self.ports.push(Port {
                    at,
                    name,
                    width,
                    incoming: true,
                    source: from,
                });
            }
        }
        for (y, from, to) in outs {
            let at = pos2(bounds.right() + MARGIN, y);
            for e in to {
                self.sink_proxy.insert(e, at);
            }
            if let Some((name, width)) = self.net_name(world, from) {
                self.ports.push(Port {
                    at,
                    name,
                    width,
                    incoming: false,
                    source: from,
                });
            }
        }
    }

    /// Whether part `id` is outside the module being looked into.
    pub fn outside(&self, id: Id) -> bool {
        self.outside.contains(&id)
    }

    pub fn sink_aliases(&self, end: End) -> Vec<End> {
        match self.sink_proxy.get(&end) {
            Some(at) => self
                .sink_proxy
                .iter()
                .filter(|(e, p)| *p == at && self.owners.get(&e.part) == self.owners.get(&end.part))
                .map(|(e, _)| *e)
                .collect(),
            None => vec![end],
        }
    }

    pub fn visible(&self, id: Id) -> bool {
        !self.owners.contains_key(&id) && !self.outside.contains(&id)
    }

    pub fn wire_visible(&self, from: End, to: End) -> bool {
        if self.outside(from.part) && self.outside(to.part) {
            return false;
        }
        !matches!((self.owners.get(&from.part), self.owners.get(&to.part)), (Some(a), Some(b)) if a == b)
    }

    pub fn module_at(&self, p: Pos2) -> Option<Id> {
        self.modules
            .iter()
            .rev()
            .find(|m| {
                let r = self.rect(m.rect);
                (m.closed && r.contains(p))
                    || Rect::from_min_size(r.min, vec2(r.width(), 28.0 * self.zoom)).contains(p)
            })
            .map(|m| m.id)
    }

    pub fn to_screen(&self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.zoom
    }

    pub fn to_world(&self, p: Pos2) -> Pos2 {
        ((p - self.origin) / self.zoom).to_pos2()
    }

    pub fn rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(r.min), self.to_screen(r.max))
    }

    fn find(&self, id: Id) -> Option<(Pos2, &Shape)> {
        let (_, at, s) = self.parts.get(*self.index.get(&id)?)?;
        Some((*at, s.as_ref()?))
    }

    /// The world rect of part `id` (a small box for a missing chip).
    pub fn part_rect(&self, id: Id) -> Option<Rect> {
        let (_, at, s) = self.parts.get(*self.index.get(&id)?)?;
        Some(match s {
            Some(s) => s.geometry.rect(*at),
            None => Rect::from_min_size(*at, vec2(120.0, 40.0)),
        })
    }

    /// Screen position of output pin `end`.
    pub fn source_pos(&self, end: End) -> Option<Pos2> {
        if !self.visible(end.part) {
            return self.source_proxy.get(&end).map(|p| self.to_screen(*p));
        }
        let (at, s) = self.find(end.part)?;
        Some(self.to_screen(at + *s.geometry.outputs.get(end.pin)?))
    }

    /// Screen position of input pin `end`.
    pub fn sink_pos(&self, end: End) -> Option<Pos2> {
        if !self.visible(end.part) {
            return self.sink_proxy.get(&end).map(|p| self.to_screen(*p));
        }
        let (at, s) = self.find(end.part)?;
        Some(self.to_screen(at + s.geometry.inputs.get(end.pin)?.0))
    }

    /// Direction a wire leaves input `end` in (towards its source).
    pub fn sink_dir(&self, end: End) -> Vec2 {
        if self.sink_proxy.contains_key(&end) {
            return vec2(-1.0, 0.0);
        }
        self.find(end.part)
            .and_then(|(_, s)| s.geometry.inputs.get(end.pin))
            .map_or(vec2(-1.0, 0.0), |(_, d)| *d)
    }

    pub fn output_pin(&self, end: End) -> Option<&Pin> {
        self.find(end.part)?.1.pins.outputs.get(end.pin)
    }

    pub fn input_pin(&self, end: End) -> Option<&Pin> {
        self.find(end.part)?.1.pins.inputs.get(end.pin)
    }

    pub fn sources(&self) -> impl Iterator<Item = (End, Pos2)> + '_ {
        self.parts
            .iter()
            .filter(|(id, _, _)| self.visible(*id))
            .flat_map(move |(id, at, s)| {
                s.iter().flat_map(move |s| {
                    s.geometry
                        .outputs
                        .iter()
                        .enumerate()
                        .map(move |(pin, off)| (End::new(*id, pin), self.to_screen(*at + *off)))
                })
            })
            .chain(
                self.source_proxy
                    .iter()
                    .map(|(e, p)| (*e, self.to_screen(*p))),
            )
    }

    pub fn sinks(&self) -> impl Iterator<Item = (End, Pos2)> + '_ {
        self.parts
            .iter()
            .filter(|(id, _, _)| self.visible(*id))
            .flat_map(move |(id, at, s)| {
                s.iter().flat_map(move |s| {
                    s.geometry
                        .inputs
                        .iter()
                        .enumerate()
                        .map(move |(pin, (off, _))| {
                            (End::new(*id, pin), self.to_screen(*at + *off))
                        })
                })
            })
            .chain(
                self.sink_proxy
                    .iter()
                    .map(|(e, p)| (*e, self.to_screen(*p))),
            )
    }

    /// The topmost part under screen point `p`.
    pub fn part_at(&self, p: Pos2) -> Option<Id> {
        self.parts
            .iter()
            .rev()
            .map(|(id, _, _)| *id)
            .filter(|id| self.visible(*id))
            .find(|id| {
                self.part_rect(*id)
                    .is_some_and(|r| self.rect(r).contains(p))
            })
    }

    /// Bounding rect of every part, in world units.
    pub fn bounds(&self) -> Option<Rect> {
        self.parts
            .iter()
            .filter(|(id, _, _)| self.visible(*id))
            .filter_map(|(id, _, _)| self.part_rect(*id))
            .chain(self.modules.iter().map(|m| m.rect))
            .chain(self.ports.iter().map(|p| p.rect()))
            .reduce(|a, b| a.union(b))
    }
}
