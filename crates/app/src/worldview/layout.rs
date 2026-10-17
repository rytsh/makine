//! Where world parts are drawn and where their pins are, like the
//! canvas's layout: each part is drawn as a gate symbol, a box with named
//! pins, or (switches, lamps, clocks, inputs, outputs, screens, displays)
//! its own face.

use egui::{pos2, vec2, Pos2, Rect, Vec2};
use makine_sim::{Library, Part, PartKind, PartPins, Pin};
use makine_world::{Device, End, Gate, Id, Kind, World};

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
        Kind::Device(d @ (Device::Ram { .. } | Device::Rom { .. } | Device::Keyboard)) => {
            Part::new(PartKind::Chip(d.name()))
        }
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
}

impl Layout {
    pub fn new(world: &World, lib: &Library, origin: Pos2, zoom: f32, symbols: bool) -> Self {
        Self {
            origin,
            zoom,
            parts: world
                .parts()
                .map(|(id, p)| (id, pos2(p.pos.x, p.pos.y), shape(&p.kind, lib, symbols)))
                .collect(),
        }
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
        self.parts
            .iter()
            .find(|(i, _, _)| *i == id)
            .and_then(|(_, at, s)| Some((*at, s.as_ref()?)))
    }

    /// The world rect of part `id` (a small box for a missing chip).
    pub fn part_rect(&self, id: Id) -> Option<Rect> {
        let (_, at, s) = self.parts.iter().find(|(i, _, _)| *i == id)?;
        Some(match s {
            Some(s) => s.geometry.rect(*at),
            None => Rect::from_min_size(*at, vec2(120.0, 40.0)),
        })
    }

    /// Screen position of output pin `end`.
    pub fn source_pos(&self, end: End) -> Option<Pos2> {
        let (at, s) = self.find(end.part)?;
        Some(self.to_screen(at + *s.geometry.outputs.get(end.pin)?))
    }

    /// Screen position of input pin `end`.
    pub fn sink_pos(&self, end: End) -> Option<Pos2> {
        let (at, s) = self.find(end.part)?;
        Some(self.to_screen(at + s.geometry.inputs.get(end.pin)?.0))
    }

    /// Direction a wire leaves input `end` in (towards its source).
    pub fn sink_dir(&self, end: End) -> Vec2 {
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
        self.parts.iter().flat_map(move |(id, at, s)| {
            s.iter().flat_map(move |s| {
                s.geometry
                    .outputs
                    .iter()
                    .enumerate()
                    .map(move |(pin, off)| (End::new(*id, pin), self.to_screen(*at + *off)))
            })
        })
    }

    pub fn sinks(&self) -> impl Iterator<Item = (End, Pos2)> + '_ {
        self.parts.iter().flat_map(move |(id, at, s)| {
            s.iter().flat_map(move |s| {
                s.geometry
                    .inputs
                    .iter()
                    .enumerate()
                    .map(move |(pin, (off, _))| (End::new(*id, pin), self.to_screen(*at + *off)))
            })
        })
    }

    /// The topmost part under screen point `p`.
    pub fn part_at(&self, p: Pos2) -> Option<Id> {
        self.parts.iter().rev().map(|(id, _, _)| *id).find(|id| {
            self.part_rect(*id)
                .is_some_and(|r| self.rect(r).contains(p))
        })
    }

    /// Bounding rect of every part, in world units.
    pub fn bounds(&self) -> Option<Rect> {
        self.parts
            .iter()
            .filter_map(|(id, _, _)| self.part_rect(*id))
            .reduce(|a, b| a.union(b))
    }
}
