//! A chip definition plus where its parts sit on the canvas, and the shape
//! each part is drawn with.

use egui::{pos2, vec2, Pos2, Rect, Vec2};
use makine_sim::{ChipDef, Library, Part, PartId, PartKind, PartPins, Sink, Source};
use serde::{Deserialize, Serialize};

pub const HEADER: f32 = 22.0;
pub const PIN_GAP: f32 = 22.0;
pub const PIN_RADIUS: f32 = 6.0;

/// Gate symbols: length of the pin leads, body size, inversion bubble.
pub const LEAD: f32 = 12.0;
pub const GATE_W: f32 = 50.0;
pub const GATE_H: f32 = 40.0;
pub const BUBBLE: f32 = 5.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Design {
    pub def: ChipDef,
    /// Top-left corner of each part, in canvas (world) units. Parallel to
    /// `def.parts`.
    pub pos: Vec<[f32; 2]>,
    /// Top-left corner of the chip's own input and output boxes, once the
    /// player has moved them. Missing entries use the default column.
    #[serde(default)]
    pub input_pos: Vec<Option<[f32; 2]>>,
    #[serde(default)]
    pub output_pos: Vec<Option<[f32; 2]>>,
}

pub const PORT_X: f32 = 20.0;
pub const PORT_W: f32 = 84.0;
pub const PORT_H: f32 = 40.0;

/// Which of the chip's own pins a port box is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    Input(usize),
    Output(usize),
}

impl Design {
    pub fn new(def: ChipDef) -> Self {
        let pos = (0..def.parts.len())
            .map(|i| [200.0 + 40.0 * i as f32, 80.0])
            .collect();
        Self {
            def,
            pos,
            input_pos: Vec::new(),
            output_pos: Vec::new(),
        }
    }

    /// World rect of a port box. Unmoved inputs stack on the left at
    /// `PORT_X`, unmoved outputs on the right at `output_x`.
    pub fn port_rect(&self, port: Port, output_x: f32) -> Rect {
        let (list, i, x) = match port {
            Port::Input(i) => (&self.input_pos, i, PORT_X),
            Port::Output(i) => (&self.output_pos, i, output_x),
        };
        let min = match list.get(i).copied().flatten() {
            Some([x, y]) => pos2(x, y),
            None => default_port_pos(i, x),
        };
        Rect::from_min_size(min, vec2(PORT_W, PORT_H))
    }

    /// Move a port box by `delta`, pinning it at its current place first.
    pub fn move_port(&mut self, port: Port, output_x: f32, delta: Vec2) {
        let at = self.port_rect(port, output_x).min + delta;
        let (list, i) = match port {
            Port::Input(i) => (&mut self.input_pos, i),
            Port::Output(i) => (&mut self.output_pos, i),
        };
        if list.len() <= i {
            list.resize(i + 1, None);
        }
        list[i] = Some([at.x, at.y]);
    }

    /// Put every port box back in its default column.
    pub fn reset_ports(&mut self) {
        self.input_pos.clear();
        self.output_pos.clear();
    }

    pub fn add_part(&mut self, part: Part, at: Pos2) -> PartId {
        self.def.parts.push(part);
        self.pos.push([at.x, at.y]);
        self.def.parts.len() - 1
    }

    pub fn remove_part(&mut self, id: PartId) {
        self.def.remove_part(id);
        self.pos.remove(id);
    }

    pub fn part_pos(&self, id: PartId) -> Pos2 {
        let [x, y] = self.pos[id];
        pos2(x, y)
    }

    /// Remove several parts at once.
    pub fn remove_parts(&mut self, ids: &[PartId]) {
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        for &id in ids.iter().rev() {
            if id < self.def.parts.len() {
                self.remove_part(id);
            }
        }
    }

    /// The parts `ids` with the wires between them, as text for the
    /// clipboard (`None` if nothing is selected).
    pub fn copy(&self, ids: &[PartId]) -> Option<String> {
        let mut ids: Vec<PartId> = ids
            .iter()
            .copied()
            .filter(|&i| i < self.def.parts.len())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return None;
        }
        let index = |p: PartId| ids.iter().position(|&i| i == p);
        let wires = self
            .def
            .wires
            .iter()
            .filter_map(|w| match (w.from, w.to) {
                (Source::Part { part: a, pin: pa }, Sink::Part { part: b, pin: pb }) => {
                    Some(((index(a)?, pa), (index(b)?, pb)))
                }
                _ => None,
            })
            .collect();
        let clip = Clip {
            makine_parts: ids.iter().map(|&i| self.def.parts[i].clone()).collect(),
            pos: ids.iter().map(|&i| self.pos[i]).collect(),
            wires,
        };
        serde_json::to_string(&clip).ok()
    }

    /// Paste parts copied with [`Design::copy`], their top-left corner at
    /// `at`. Returns the new parts (none if `text` is not copied parts).
    pub fn paste(&mut self, text: &str, at: Pos2) -> Vec<PartId> {
        let Ok(clip) = serde_json::from_str::<Clip>(text.trim()) else {
            return Vec::new();
        };
        if clip.makine_parts.len() != clip.pos.len() {
            return Vec::new();
        }
        let min = clip
            .pos
            .iter()
            .fold([f32::MAX; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
        let first = self.def.parts.len();
        for (part, p) in clip.makine_parts.into_iter().zip(clip.pos) {
            self.add_part(part, pos2(at.x + p[0] - min[0], at.y + p[1] - min[1]));
        }
        let n = self.def.parts.len() - first;
        for ((a, pa), (b, pb)) in clip.wires {
            if a < n && b < n {
                self.def.connect(
                    Source::Part {
                        part: first + a,
                        pin: pa,
                    },
                    Sink::Part {
                        part: first + b,
                        pin: pb,
                    },
                );
            }
        }
        (first..self.def.parts.len()).collect()
    }

    /// Drop wires to pins that no longer exist or whose widths no longer
    /// fit, e.g. after a part's pins changed.
    pub fn prune(&mut self, lib: &Library) {
        let pins: Vec<Option<PartPins>> = self.def.parts.iter().map(|p| lib.part_pins(p)).collect();
        let def = &self.def;
        let width_of_source = |s: Source| match s {
            Source::Input(i) => def.inputs.get(i).map(|p| p.width),
            Source::Part { part, pin } => {
                pins.get(part)?.as_ref()?.outputs.get(pin).map(|p| p.width)
            }
        };
        let width_of_sink = |s: Sink| match s {
            Sink::Output(i) => def.outputs.get(i).map(|p| p.width),
            Sink::Part { part, pin } => pins.get(part)?.as_ref()?.inputs.get(pin).map(|p| p.width),
        };
        let keep: Vec<bool> = def
            .wires
            .iter()
            .map(|w| match (width_of_source(w.from), width_of_sink(w.to)) {
                (Some(f), Some(t)) => f == t || f == 1,
                _ => false,
            })
            .collect();
        let mut it = keep.iter();
        self.def.wires.retain(|_| *it.next().unwrap());
    }
}

/// Parts on the clipboard. The odd field name keeps other JSON (such as
/// copied world parts) from being read as parts.
#[derive(Serialize, Deserialize)]
struct Clip {
    makine_parts: Vec<Part>,
    pos: Vec<[f32; 2]>,
    /// (part, output pin) → (part, input pin), indexes into `makine_parts`.
    wires: Vec<((usize, usize), (usize, usize))>,
}

/// Undo and redo for a design: whole copies, since designs are small.
#[derive(Default)]
pub struct History {
    undo: Vec<Design>,
    redo: Vec<Design>,
    /// Whether the last change can take in the next one (dragging a number
    /// in a menu changes the design every frame; that is one step).
    merging: bool,
}

impl History {
    const MAX: usize = 100;

    /// Remember `before`, the design as it was before a change. `merge`
    /// changes in a row make one step.
    pub fn push(&mut self, before: Design, merge: bool) {
        let merged = merge && std::mem::replace(&mut self.merging, merge);
        self.merging = merge;
        if self.undo.last() == Some(&before) || (merged && !self.undo.is_empty()) {
            self.redo.clear();
            return;
        }
        self.undo.push(before);
        if self.undo.len() > Self::MAX {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Go back one change from `now`; `None` if there is nothing to undo.
    pub fn undo(&mut self, now: &Design) -> Option<Design> {
        self.merging = false;
        let d = self.undo.pop()?;
        self.redo.push(now.clone());
        Some(d)
    }

    pub fn redo(&mut self, now: &Design) -> Option<Design> {
        self.merging = false;
        let d = self.redo.pop()?;
        self.undo.push(now.clone());
        Some(d)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

/// Default top-left corner of the `i`-th port in a column at `x`.
pub fn default_port_pos(i: usize, x: f32) -> Pos2 {
    pos2(x, 40.0 + i as f32 * 56.0)
}

/// How a part is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// A labelled box with named pins.
    Box,
    Not,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Mux,
    Demux,
}

impl Symbol {
    /// The symbol for a part, if gate symbols are on. Symbols are only used
    /// when the part has the pins the symbol expects.
    pub fn of(kind: &PartKind, pins: &PartPins, symbols: bool) -> Symbol {
        if !symbols {
            return Symbol::Box;
        }
        let s = match kind {
            PartKind::Nand => Symbol::Nand,
            PartKind::Chip(name) => match name.as_str() {
                "Not" => Symbol::Not,
                "And" => Symbol::And,
                "Or" => Symbol::Or,
                "Nor" => Symbol::Nor,
                "Xor" => Symbol::Xor,
                "Mux" | "Mux32" => Symbol::Mux,
                "Demux" => Symbol::Demux,
                _ => Symbol::Box,
            },
            _ => Symbol::Box,
        };
        let (i, o) = (pins.inputs.len(), pins.outputs.len());
        let fits = match s {
            Symbol::Box => true,
            Symbol::Not => (i, o) == (1, 1),
            Symbol::And | Symbol::Nand | Symbol::Or | Symbol::Nor | Symbol::Xor => (i, o) == (2, 1),
            Symbol::Mux => (i, o) == (3, 1),
            Symbol::Demux => (i, o) == (2, 2),
        };
        if fits {
            s
        } else {
            Symbol::Box
        }
    }

    pub fn bubble(self) -> bool {
        matches!(self, Symbol::Not | Symbol::Nand | Symbol::Nor)
    }
}

/// Where a part's pins are, relative to its top-left corner. `dir` is the
/// direction a wire leaves the pin in: inputs face left, except a select
/// input at the bottom, which faces down.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub symbol: Symbol,
    pub size: Vec2,
    pub inputs: Vec<(Vec2, Vec2)>,
    pub outputs: Vec<Vec2>,
}

impl Geometry {
    pub fn rect(&self, at: Pos2) -> Rect {
        Rect::from_min_size(at, self.size)
    }
}

pub fn geometry(kind: &PartKind, pins: &PartPins, label: &str, symbols: bool) -> Geometry {
    let symbol = Symbol::of(kind, pins, symbols);
    let left = vec2(-1.0, 0.0);
    let down = vec2(0.0, 1.0);
    let w = LEAD + GATE_W + if symbol.bubble() { 2.0 * BUBBLE } else { 0.0 } + LEAD;
    let h = GATE_H;
    match symbol {
        Symbol::Box => {
            let size = box_size(label, pins);
            let row = |i: usize| HEADER + (i as f32 + 0.5) * PIN_GAP;
            Geometry {
                symbol,
                size,
                inputs: (0..pins.inputs.len())
                    .map(|i| (vec2(0.0, row(i)), left))
                    .collect(),
                outputs: (0..pins.outputs.len())
                    .map(|i| vec2(size.x, row(i)))
                    .collect(),
            }
        }
        Symbol::Not => Geometry {
            symbol,
            size: vec2(w, h),
            inputs: vec![(vec2(0.0, h / 2.0), left)],
            outputs: vec![vec2(w, h / 2.0)],
        },
        Symbol::And | Symbol::Nand | Symbol::Or | Symbol::Nor | Symbol::Xor => Geometry {
            symbol,
            size: vec2(w, h),
            inputs: vec![(vec2(0.0, h * 0.25), left), (vec2(0.0, h * 0.75), left)],
            outputs: vec![vec2(w, h / 2.0)],
        },
        Symbol::Mux => {
            // a, b on the left; sel from below.
            let h = GATE_H + 20.0;
            Geometry {
                symbol,
                size: vec2(w, h + LEAD),
                inputs: vec![
                    (vec2(0.0, h * 0.3), left),
                    (vec2(0.0, h * 0.7), left),
                    (vec2(w / 2.0, h + LEAD), down),
                ],
                outputs: vec![vec2(w, h / 2.0)],
            }
        }
        Symbol::Demux => {
            let h = GATE_H + 20.0;
            Geometry {
                symbol,
                size: vec2(w, h + LEAD),
                inputs: vec![(vec2(0.0, h / 2.0), left), (vec2(w / 2.0, h + LEAD), down)],
                outputs: vec![vec2(w, h * 0.3), vec2(w, h * 0.7)],
            }
        }
    }
}

/// Size of a part drawn as a box.
fn box_size(label: &str, pins: &PartPins) -> Vec2 {
    let rows = pins.inputs.len().max(pins.outputs.len()).max(1);
    let longest = |ps: &[makine_sim::Pin]| {
        ps.iter()
            .map(|p| p.to_string().chars().count())
            .max()
            .unwrap_or(0)
    };
    let pin_text = (longest(&pins.inputs) + longest(&pins.outputs)) as f32 * 6.5;
    let w = (label.chars().count() as f32 * 8.0 + 24.0)
        .max(pin_text + 36.0)
        .max(70.0);
    vec2(w, HEADER + rows as f32 * PIN_GAP + 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_sim::{out, ChipBuilder};

    #[test]
    fn prune_drops_stale_wires() {
        let mut b = ChipBuilder::new("T", &["a[4]"], &["o"]);
        let a = b.input("a");
        let s = b.split(&[1, 3], a);
        b.output("o", out(s, 0));
        let mut d = Design::new(b.build());
        let lib = Library::new();
        d.prune(&lib);
        assert_eq!(d.def.wires.len(), 2);

        // Now the split only has one 4-bit group: input still fits, output
        // pin 0 is 4 bits wide and no longer fits the 1-bit chip output.
        d.def.parts[0].kind = PartKind::Split(vec![4]);
        d.prune(&lib);
        assert_eq!(d.def.wires.len(), 1);
    }

    #[test]
    fn ports_move_and_reset() {
        let mut d = Design::new(ChipDef::new("T", &["a", "b"], &["o"]));
        let r = d.port_rect(Port::Input(1), 500.0);
        assert_eq!(r.min, default_port_pos(1, PORT_X));
        d.move_port(Port::Input(1), 500.0, vec2(30.0, -10.0));
        assert_eq!(
            d.port_rect(Port::Input(1), 500.0).min,
            r.min + vec2(30.0, -10.0)
        );
        // Input 0 and the output are untouched.
        assert_eq!(
            d.port_rect(Port::Input(0), 500.0).min,
            default_port_pos(0, PORT_X)
        );
        assert_eq!(
            d.port_rect(Port::Output(0), 500.0).min,
            default_port_pos(0, 500.0)
        );
        // A moved output stays put when the window (output_x) changes.
        d.move_port(Port::Output(0), 500.0, vec2(-100.0, 0.0));
        assert_eq!(d.port_rect(Port::Output(0), 900.0).min.x, 400.0);
        d.reset_ports();
        assert_eq!(
            d.port_rect(Port::Input(1), 500.0).min,
            default_port_pos(1, PORT_X)
        );
    }

    #[test]
    fn copy_paste_keeps_inner_wires() {
        let mut b = ChipBuilder::new("T", &["a"], &["o"]);
        let a = b.input("a");
        let x = b.nand(a, a);
        let y = b.nand(x, x);
        b.output("o", y);
        let mut d = Design::new(b.build());
        let text = d.copy(&[0, 1]).unwrap();
        let new = d.paste(&text, pos2(500.0, 500.0));
        assert_eq!(new, vec![2, 3]);
        // Only the wires between the copied parts come along (two, from
        // part 2 into both pins of part 3).
        assert_eq!(d.def.wires.len(), 5 + 2);
        assert_eq!(d.part_pos(2), pos2(500.0, 500.0));
        assert!(d.paste("{\"parts\":[]}", pos2(0.0, 0.0)).is_empty());
        d.remove_parts(&[3, 0, 2]);
        assert_eq!(d.def.parts.len(), 1);
    }

    #[test]
    fn history_undoes_and_redoes() {
        let mut h = History::default();
        let a = Design::new(ChipDef::new("T", &["a"], &["o"]));
        let mut b = a.clone();
        b.add_part(Part::new(PartKind::Nand), pos2(0.0, 0.0));
        h.push(a.clone(), true);
        h.push(b.clone(), true);
        assert_eq!(h.undo(&b), Some(a.clone()), "menu changes make one step");
        assert!(!h.can_undo());
        assert_eq!(h.redo(&a), Some(b.clone()));
        assert!(h.undo(&b).is_some());
        h.push(a.clone(), false);
        assert!(!h.can_redo(), "a new change drops the redo list");
    }

    #[test]
    fn old_saves_load() {
        // Saves from before movable ports have no port positions.
        let json =
            r#"{"def":{"name":"T","inputs":[],"outputs":[],"parts":[],"wires":[]},"pos":[]}"#;
        let d: Design = serde_json::from_str(json).unwrap();
        assert!(d.input_pos.is_empty());
    }

    #[test]
    fn symbols_need_matching_pins() {
        let lib = Library::new();
        let nand = lib.kind_pins(&PartKind::Nand).unwrap();
        assert_eq!(Symbol::of(&PartKind::Nand, &nand, true), Symbol::Nand);
        assert_eq!(Symbol::of(&PartKind::Nand, &nand, false), Symbol::Box);
        // A chip named "And" with the wrong pins stays a box.
        let three = PartPins {
            inputs: makine_sim::parse_pins(&["a", "b", "c"]),
            outputs: makine_sim::parse_pins(&["out"]),
        };
        assert_eq!(
            Symbol::of(&PartKind::Chip("And".into()), &three, true),
            Symbol::Box
        );
        assert_eq!(
            Symbol::of(&PartKind::Chip("Mux".into()), &three, true),
            Symbol::Mux
        );
    }

    #[test]
    fn pins_are_on_the_outline() {
        let lib = Library::new();
        let pins = lib.kind_pins(&PartKind::Nand).unwrap();
        let g = geometry(&PartKind::Nand, &pins, "NAND", true);
        for (p, _) in &g.inputs {
            assert_eq!(p.x, 0.0);
        }
        assert_eq!(g.outputs[0].x, g.size.x);
        assert!(g.inputs.iter().all(|(p, _)| p.y > 0.0 && p.y < g.size.y));
    }
}
