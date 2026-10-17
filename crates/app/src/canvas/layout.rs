//! Where everything is on screen: parts, pins, ports and wires' ends.

use egui::{vec2, Pos2, Rect, Vec2};
use makine_sim::{Library, PartId, PartKind, PartPins, Pin, Sink, Source};

use super::Hover;
use crate::design::{geometry, Design, Geometry, Port};

pub(super) struct Layout {
    pub origin: Pos2,
    pub zoom: f32,
    /// World position, geometry and pins of each part (`None`: the part's
    /// chip is unknown).
    pub parts: Vec<Option<(Pos2, Geometry, PartPins)>>,
    /// World rects of the chip's input and output boxes.
    pub inputs: Vec<Rect>,
    pub outputs: Vec<Rect>,
    /// Where unmoved output boxes go.
    pub output_x: f32,
}

impl Layout {
    pub fn new(
        design: &Design,
        lib: &Library,
        origin: Pos2,
        zoom: f32,
        symbols: bool,
        output_x: f32,
    ) -> Layout {
        let port_rects = |n: usize, port: fn(usize) -> Port| {
            (0..n)
                .map(|i| design.port_rect(port(i), output_x))
                .collect()
        };
        Layout {
            origin,
            zoom,
            parts: design
                .def
                .parts
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let pins = lib.part_pins(p)?;
                    let g = geometry(&p.kind, &pins, &part_label(p), symbols);
                    Some((design.part_pos(i), g, pins))
                })
                .collect(),
            inputs: port_rects(design.def.inputs.len(), Port::Input),
            outputs: port_rects(design.def.outputs.len(), Port::Output),
            output_x,
        }
    }

    /// What is under screen point `p`; pins within `pin_hit` count.
    pub fn hover(&self, p: Pos2, pin_hit: f32) -> Hover {
        let in_rect = |rs: &[Rect]| rs.iter().position(|r| self.rect(*r).contains(p));
        let input = in_rect(&self.inputs);
        Hover {
            source: self.sources().find(|(_, sp)| sp.distance(p) < pin_hit),
            sink: self.sinks().find(|(_, sp)| sp.distance(p) < pin_hit),
            part: self.part_at(p),
            input,
            port: input
                .map(Port::Input)
                .or_else(|| in_rect(&self.outputs).map(Port::Output)),
        }
    }

    /// Whether a wire from `from` may go into `to`: same width, or a
    /// single bit fanned out to every bit.
    pub fn fits(&self, design: &Design, from: Source, to: Sink) -> bool {
        let fw = self.source_width(design, from);
        fw == 1 || fw == self.sink_width(design, to)
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

    pub fn source_pos(&self, s: Source) -> Option<Pos2> {
        Some(match s {
            Source::Input(i) => self.to_screen(self.inputs.get(i)?.right_center()),
            Source::Part { part, pin } => {
                let (at, g, _) = self.parts.get(part)?.as_ref()?;
                self.to_screen(*at + *g.outputs.get(pin)?)
            }
        })
    }

    pub fn sink_pos(&self, s: Sink) -> Option<Pos2> {
        Some(match s {
            Sink::Output(i) => self.to_screen(self.outputs.get(i)?.left_center()),
            Sink::Part { part, pin } => {
                let (at, g, _) = self.parts.get(part)?.as_ref()?;
                self.to_screen(*at + g.inputs.get(pin)?.0)
            }
        })
    }

    pub fn source_width(&self, design: &Design, s: Source) -> u8 {
        match s {
            Source::Input(i) => design.def.inputs[i].width,
            Source::Part { part, pin } => self.parts[part]
                .as_ref()
                .map_or(1, |(_, _, p)| p.outputs[pin].width),
        }
    }

    pub fn sink_width(&self, design: &Design, s: Sink) -> u8 {
        match s {
            Sink::Output(i) => design.def.outputs[i].width,
            Sink::Part { part, pin } => self.parts[part]
                .as_ref()
                .map_or(1, |(_, _, p)| p.inputs[pin].width),
        }
    }

    pub fn sources(&self) -> impl Iterator<Item = (Source, Pos2)> + '_ {
        let inputs = (0..self.inputs.len()).map(Source::Input);
        let parts = self.parts.iter().enumerate().flat_map(|(part, p)| {
            let n = p.as_ref().map_or(0, |(_, _, pins)| pins.outputs.len());
            (0..n).map(move |pin| Source::Part { part, pin })
        });
        inputs
            .chain(parts)
            .filter_map(|s| Some((s, self.source_pos(s)?)))
    }

    pub fn sinks(&self) -> impl Iterator<Item = (Sink, Pos2)> + '_ {
        let outputs = (0..self.outputs.len()).map(Sink::Output);
        let parts = self.parts.iter().enumerate().flat_map(|(part, p)| {
            let n = p.as_ref().map_or(0, |(_, _, pins)| pins.inputs.len());
            (0..n).map(move |pin| Sink::Part { part, pin })
        });
        outputs
            .chain(parts)
            .filter_map(|s| Some((s, self.sink_pos(s)?)))
    }

    /// Direction a wire leaves a sink in (towards its source).
    pub fn sink_dir(&self, s: Sink) -> Vec2 {
        match s {
            Sink::Part { part, pin } => self.parts[part]
                .as_ref()
                .and_then(|(_, g, _)| g.inputs.get(pin))
                .map_or(vec2(-1.0, 0.0), |(_, d)| *d),
            Sink::Output(_) => vec2(-1.0, 0.0),
        }
    }

    pub fn part_at(&self, p: Pos2) -> Option<PartId> {
        self.parts.iter().enumerate().rev().find_map(|(i, r)| {
            r.as_ref()
                .filter(|(at, g, _)| self.rect(g.rect(*at)).contains(p))
                .map(|_| i)
        })
    }
}

/// Text shown for a value of the given width: decimal up to a byte,
/// hexadecimal for wider buses.
pub fn format_value(width: u8, v: u64) -> String {
    if width <= 8 {
        v.to_string()
    } else {
        format!("0x{v:X}")
    }
}

pub fn part_label(p: &makine_sim::Part) -> String {
    let name = match &p.kind {
        PartKind::Nand => "NAND".to_string(),
        PartKind::Chip(n) => n.clone(),
        PartKind::Split(w) => format!("Böl {}", join_widths(w)),
        PartKind::Join(w) => format!("Birleştir {}", join_widths(w)),
        PartKind::Const { width, value } => format!("Sabit {}", format_value(*width, *value)),
    };
    if p.repeat > 1 {
        format!("{name} ×{}", p.repeat)
    } else {
        name
    }
}

fn join_widths(w: &[u8]) -> String {
    if w.len() > 1 && w.iter().all(|&x| x == w[0]) {
        format!("{}×{}", w.len(), w[0])
    } else {
        w.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub(super) fn pin_label(p: &Pin) -> String {
    p.to_string()
}
