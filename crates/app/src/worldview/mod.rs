//! The world view: parts on a plane with wires between their pins, edited
//! the way the level editor is, and running live.
//!
//! - Drag a part from the side panel onto the board (or click it in the
//!   panel, then click the board) to place it.
//! - Drag a part to move it; drag empty board to pan (or select, with
//!   Shift); the wheel zooms.
//! - Drag from an output pin to an input pin to wire them. Pulling a
//!   wire's end off an input moves or cuts it.
//! - Click a switch or an input to change it; click any other part to
//!   select it (its settings show in the side panel).
//! - Right click a part or a wire for its menu; Delete removes what is
//!   selected (or the wire under the pointer).
//!
//! Every change goes through [`WorldView::edit`], so it can be undone and
//! shared with other players.

use std::collections::BTreeSet;

use egui::{pos2, vec2, Color32, Id as EguiId, Pos2, Rect, Sense, Ui, Vec2};
use makine_lang::{tr, trf};
use makine_sim::{mask, Library};
use makine_world::{device, Change, Device, Edit, End, Id, Kind, Placed, Pos, World};

use crate::worldsim::WorldSim;

mod draw;
mod layout;
mod panels;
mod routing;
mod scope;
#[cfg(test)]
mod tests;
mod workbench;

pub use layout::{label, shape, Layout};
pub use panels::{device_palette, draggable, palette, PaletteItem};

/// Every key and gesture of the world view: (keys, what they do), for the
/// side panel.
pub fn shortcuts() -> Vec<(&'static str, &'static str)> {
    vec![
        (tr!("Sürükle (parça)", "Drag (part)"), tr!("taşı", "move")),
        (
            tr!("Sürükle (uçtan uca)", "Drag (pin to pin)"),
            tr!("tel çek", "draw a wire"),
        ),
        (
            tr!("Sürükle (boşluk)", "Drag (empty)"),
            tr!("kaydır", "pan"),
        ),
        (
            tr!("Shift+sürükle", "Shift+drag"),
            tr!("alan seç", "select an area"),
        ),
        (
            tr!("Tık", "Click"),
            tr!(
                "anahtarı, girişi değiştir; parçayı seç",
                "toggle a switch or input; select a part"
            ),
        ),
        (
            tr!("Sağ tık", "Right click"),
            tr!("parçanın ya da telin menüsü", "part or wire menu"),
        ),
        (
            "Delete",
            tr!(
                "seçileni ya da üstündeki teli sil",
                "delete the selection or the wire under it"
            ),
        ),
        (
            "Ctrl+C / X / V",
            tr!("kopyala / kes / yapıştır", "copy / cut / paste"),
        ),
        ("Ctrl+D", tr!("seçileni çoğalt", "duplicate the selection")),
        ("Ctrl+A", tr!("hepsini seç", "select all")),
        ("Ctrl+Z / Ctrl+Y", tr!("geri al / yinele", "undo / redo")),
        (
            "1–9",
            tr!(
                "parça seç (tıklayınca koyulur)",
                "pick a part (click to place)"
            ),
        ),
        (
            "Esc",
            tr!(
                "elindekini ya da seçimi bırak",
                "drop what you hold or the selection"
            ),
        ),
        (tr!("Tekerlek", "Wheel"), tr!("yakınlaştır", "zoom")),
        (
            "P",
            tr!("saatleri durdur / başlat", "pause / run the clocks"),
        ),
        (
            "N",
            tr!("durmuşken bir adım ilerlet", "step once while paused"),
        ),
        ("Home", tr!("bütün dünyayı göster", "show the whole world")),
    ]
}

/// Parts copied: positions relative to the first part, and the wires
/// between them by index.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Clip {
    format: String,
    pub parts: Vec<Placed>,
    /// (from part, from pin, to part, to pin), as indices into `parts`.
    pub wires: Vec<(usize, usize, usize, usize)>,
    #[serde(default)]
    modules: Vec<(String, Vec<usize>)>,
    #[serde(default)]
    styles: Vec<(usize, usize, makine_world::WireStyle)>,
}

const CLIP_FORMAT: &str = "makine-world-clip-2";

impl Clip {
    fn from_text(text: &str) -> Option<Clip> {
        let c: Clip = serde_json::from_str(text.trim()).ok()?;
        (c.format == CLIP_FORMAT).then_some(c)
    }
}

pub struct WorldView {
    pub world: World,
    /// A part kind in hand: the next click on the board places it.
    pub placing: Option<Kind>,
    /// Camera: screen offset of the world origin from the board's corner,
    /// and zoom.
    pub offset: Vec2,
    pub zoom: f32,
    /// Draw basic gates as logic symbols.
    pub symbols: bool,
    /// This player's changes, newest last. Undo only takes back what
    /// nobody else has changed since.
    undo: Vec<Change>,
    redo: Vec<Change>,
    /// The running circuit of the world.
    pub sim: WorldSim,
    /// Selected parts.
    pub selected: BTreeSet<Id>,
    pub selected_module: Option<Id>,
    pub collapsed: BTreeSet<Id>,
    /// The modules looked into, outermost first; only the last one's
    /// insides are shown.
    pub scope: Vec<Id>,
    module_name: String,
    selected_wire: Option<End>,
    routing: Option<End>,
    pub minimap: bool,
    scope_binary: bool,
    /// What is being dragged.
    drag: Drag,
    /// The world as it was when a move started, so the whole move is one
    /// edit (one undo, one network message).
    moving: Option<Vec<(Id, Pos)>>,
    /// Modules moved with them (all their parts are), as they were.
    moving_modules: Vec<(Id, makine_world::Module)>,
    /// Centre the camera on the world at the next frame.
    fit: bool,
    /// The board's rect and world origin on screen in the last frame.
    pub origin: Pos2,
    board: Rect,
    /// Pointer position in world units, if it is over the board.
    hover_pos: Option<Pos2>,
    /// A message for the player.
    pub status: Option<String>,
    /// ROM contents being edited: part, text in the box, the contents the
    /// text was made from, parse error.
    rom_text: Option<(Id, String, Option<String>, Option<String>)>,
    /// The last copied parts.
    pub clipboard: Option<Clip>,
    /// Name typed for a new chip made from the selection.
    pub chip_name: String,
    /// Something the app must do (it owns the chip library).
    pub request: Option<Request>,
    /// Edits made here since the session last took them, while
    /// `sharing`.
    pub outbox: Vec<Edit>,
    /// Whether this world is shared (edits are collected in `outbox`).
    pub sharing: bool,
    /// Other players' pointers: name, colour, world position.
    pub others: Vec<(String, Color32, Pos)>,
    /// Screens' pixels, by part, read once per frame.
    screens: std::collections::HashMap<Id, Vec<bool>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Drag {
    #[default]
    None,
    /// The view itself.
    Pan,
    /// Selected parts (the one grabbed among them).
    Parts,
    /// A wire from this output.
    Wire(End),
    /// A selection rectangle from this world point.
    Select(Pos2),
}

/// Requests from the world view to the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Turn the selected parts into a free chip named `name`.
    MakeChip { name: String, parts: Vec<Id> },
}

impl Default for WorldView {
    fn default() -> Self {
        Self::new(World::new("dünya"))
    }
}

/// What is under the pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Hover {
    source: Option<(End, Pos2)>,
    sink: Option<(End, Pos2)>,
    part: Option<Id>,
    /// A wire (by the input it goes into), when nothing else is.
    wire: Option<End>,
}

impl WorldView {
    pub fn new(world: World) -> Self {
        let collapsed = world.modules().map(|(id, _)| id).collect();
        Self {
            world,
            placing: None,
            offset: Vec2::ZERO,
            zoom: 1.0,
            symbols: true,
            undo: Vec::new(),
            redo: Vec::new(),
            sim: WorldSim::default(),
            selected: BTreeSet::new(),
            selected_module: None,
            collapsed,
            scope: Vec::new(),
            module_name: tr!("Modül", "Module").into(),
            selected_wire: None,
            routing: None,
            minimap: true,
            scope_binary: false,
            drag: Drag::None,
            moving: None,
            moving_modules: Vec::new(),
            fit: true,
            origin: Pos2::ZERO,
            board: Rect::NOTHING,
            hover_pos: None,
            status: None,
            rom_text: None,
            clipboard: None,
            chip_name: String::new(),
            request: None,
            outbox: Vec::new(),
            sharing: false,
            others: Vec::new(),
            screens: Default::default(),
        }
    }

    /// Apply another player's edit. It is not undoable here (undo only
    /// takes back this player's own edits).
    pub fn apply_remote(&mut self, e: Edit) {
        self.world.apply(e);
        self.after_remote_edits();
    }

    /// Tidy up after other players changed the world.
    pub fn after_remote_edits(&mut self) {
        let w = &self.world;
        self.selected.retain(|id| w.part(*id).is_some());
        self.collapsed.retain(|id| w.module(*id).is_some());
        if let Some(gone) = self.scope.iter().position(|id| w.module(*id).is_none()) {
            self.scope.truncate(gone);
            self.fit = true;
        }
        if self
            .selected_module
            .is_some_and(|id| w.module(id).is_none())
        {
            self.selected_module = None;
        }
    }

    /// The pointer's world position, for other players.
    pub fn hover(&self) -> Option<Pos> {
        self.hover_pos.map(|p| Pos::new(p.x, p.y))
    }

    /// Set the player's chip library (used by library parts).
    pub fn set_library(&mut self, lib: Library) {
        self.sim.set_library(lib);
    }

    /// Apply an edit made by the player (undoable).
    pub fn edit(&mut self, e: Edit) {
        if self.sharing {
            self.outbox.push(e.clone());
        }
        let change = self.world.apply_change(e);
        if change.is_empty() {
            return;
        }
        self.undo.push(change);
        self.redo.clear();
        if self.undo.len() > 500 {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self) {
        self.step_history(true);
    }

    pub fn redo(&mut self) {
        self.step_history(false);
    }

    fn step_history(&mut self, back: bool) {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        while let Some(change) = from.pop() {
            let Some(e) = self.world.revert(&change, back) else {
                continue;
            };
            if self.sharing {
                self.outbox.push(e.clone());
            }
            self.world.apply(e);
            to.push(change);
            self.after_remote_edits();
            return;
        }
    }

    pub fn can_undo(&self) -> bool {
        self.undo
            .iter()
            .any(|c| self.world.revert(c, true).is_some())
    }

    pub fn can_redo(&self) -> bool {
        self.redo
            .iter()
            .any(|c| self.world.revert(c, false).is_some())
    }

    /// Put a new part of `kind` with its top-left corner at `at`. Returns
    /// its id. Inputs and outputs get the next free name.
    pub fn add(&mut self, kind: Kind, at: Pos2) -> Id {
        let kind = free_name(&self.world, kind);
        let id = self.world.new_id();
        let mut edits = vec![Edit::Part {
            id,
            part: Some(Placed {
                kind,
                pos: Pos::new(at.x.round(), at.y.round()),
            }),
        }];
        edits.extend(self.adopt(&[id]));
        self.edit(Edit::Batch(edits));
        id
    }

    /// Edits putting new parts `ids` into the modules being looked into,
    /// so they stay in view.
    fn adopt(&self, ids: &[Id]) -> Vec<Edit> {
        self.scope
            .iter()
            .filter_map(|m| {
                let mut module = self.world.module(*m)?.clone();
                module.members.extend_from_slice(ids);
                Some(Edit::Module {
                    id: *m,
                    module: Some(module),
                })
            })
            .collect()
    }

    /// Look into module `id` (one of those shown): only its insides are
    /// drawn, with its connections to the outside as ports.
    pub fn enter_module(&mut self, id: Id) {
        if self.world.module(id).is_none() || self.scope.last() == Some(&id) {
            return;
        }
        // A module inside the one looked into, maybe several levels down:
        // go through the ones between.
        let members = |m: Id| -> BTreeSet<Id> {
            self.world
                .module(m)
                .map(|m| m.members.iter().copied().collect())
                .unwrap_or_default()
        };
        let target = members(id);
        let mut path: Vec<(Id, usize)> = self
            .world
            .modules()
            .filter(|(m, module)| {
                *m != id
                    && !self.scope.contains(m)
                    && module.members.len() > target.len()
                    && target.iter().all(|p| module.members.contains(p))
                    && self
                        .scope
                        .last()
                        .is_none_or(|s| module.members.iter().all(|p| members(*s).contains(p)))
            })
            .map(|(m, module)| (m, module.members.len()))
            .collect();
        path.sort_by_key(|(m, n)| (std::cmp::Reverse(*n), *m));
        self.scope.extend(path.into_iter().map(|(m, _)| m));
        self.scope.push(id);
        self.selected.clear();
        self.selected_module = None;
        self.refit();
    }

    /// Go back out to the `depth` outermost levels (0: the whole world).
    pub fn leave_to(&mut self, depth: usize) {
        if depth >= self.scope.len() {
            return;
        }
        self.scope.truncate(depth);
        self.selected.clear();
        self.selected_module = None;
        self.refit();
    }

    /// Change part `id` into `kind`, keeping where it is. Wires to pins it
    /// no longer has (or whose widths no longer fit) are cut.
    pub fn set_kind(&mut self, id: Id, kind: Kind) {
        let Some(p) = self.world.part(id) else { return };
        let mut edits = vec![Edit::Part {
            id,
            part: Some(Placed {
                kind: kind.clone(),
                pos: p.pos,
            }),
        }];
        let lib = self.sim.library().clone();
        if let Some(pins) = kind.pins(&lib) {
            let out_width = |e: End, w: &World| {
                let k = if e.part == id {
                    &kind
                } else {
                    &w.part(e.part)?.kind
                };
                Some(k.pins(&lib)?.outputs.get(e.pin)?.width)
            };
            for (from, to) in self.world.wires() {
                let keep = if to.part == id {
                    match (pins.inputs.get(to.pin), out_width(from, &self.world)) {
                        (Some(p), Some(fw)) => makine_world::compile::fits(fw, p.width),
                        _ => false,
                    }
                } else if from.part == id {
                    let into = self
                        .world
                        .part(to.part)
                        .and_then(|p| p.kind.pins(&lib))
                        .and_then(|p| p.inputs.get(to.pin).map(|p| p.width));
                    match (pins.outputs.get(from.pin), into) {
                        (Some(p), Some(tw)) => makine_world::compile::fits(p.width, tw),
                        _ => false,
                    }
                } else {
                    true
                };
                if !keep {
                    edits.push(Edit::Wire { to, from: None });
                }
            }
        }
        self.edit(Edit::Batch(edits));
    }

    /// Remove the selected parts and their wires.
    pub fn delete_selected(&mut self) {
        let ids: Vec<Id> = std::mem::take(&mut self.selected).into_iter().collect();
        if ids.is_empty() {
            return;
        }
        let mut edits: Vec<_> = ids.iter().map(|id| self.world.remove(*id)).collect();
        let groups: Vec<_> = self
            .world
            .modules()
            .filter(|(_, m)| m.members.iter().all(|id| ids.contains(id)))
            .map(|(id, _)| Edit::Module { id, module: None })
            .collect();
        edits.extend(groups);
        self.edit(Edit::Batch(edits));
        self.selected_module = None;
    }

    /// Copy the selected parts (and the wires between them); also returns
    /// them as text for the system clipboard.
    pub fn copy(&mut self) -> Option<String> {
        let ids: Vec<Id> = self.selected.iter().copied().collect();
        if ids.is_empty() {
            return None;
        }
        let first = self.world.part(ids[0])?.pos;
        let parts: Vec<Placed> = ids
            .iter()
            .filter_map(|id| {
                let mut p = self.world.part(*id)?.clone();
                p.pos = Pos::new(p.pos.x - first.x, p.pos.y - first.y);
                Some(p)
            })
            .collect();
        let index = |id: Id| ids.iter().position(|i| *i == id);
        let wires = self
            .world
            .wires()
            .filter_map(|(f, t)| Some((index(f.part)?, f.pin, index(t.part)?, t.pin)))
            .collect();
        let clip = Clip {
            format: CLIP_FORMAT.into(),
            parts,
            wires,
            modules: self
                .world
                .modules()
                .filter(|(_, m)| m.members.iter().all(|id| index(*id).is_some()))
                .map(|(_, m)| {
                    (
                        m.name.clone(),
                        m.members.iter().filter_map(|id| index(*id)).collect(),
                    )
                })
                .collect(),
            styles: self
                .world
                .wires()
                .filter_map(|(_, to)| {
                    let mut s = self.world.wire_style(to)?.clone();
                    for p in &mut s.points {
                        p.x -= first.x;
                        p.y -= first.y;
                    }
                    Some((index(to.part)?, to.pin, s))
                })
                .collect(),
        };
        let text = serde_json::to_string(&clip).ok();
        self.clipboard = Some(clip);
        text
    }

    pub fn cut(&mut self) -> Option<String> {
        let t = self.copy()?;
        self.delete_selected();
        Some(t)
    }

    /// Paste `text` if it holds parts (else the last copy) at world point
    /// `at`. The pasted parts are selected.
    pub fn paste(&mut self, text: Option<&str>, at: Pos2) {
        let Some(clip) = text
            .and_then(Clip::from_text)
            .or_else(|| self.clipboard.clone())
        else {
            return;
        };
        let mut ids = Vec::new();
        let mut edits = Vec::new();
        let mut scratch = self.world.clone();
        for p in &clip.parts {
            let id = scratch.new_id();
            let kind = free_name(&scratch, p.kind.clone());
            let e = Edit::Part {
                id,
                part: Some(Placed {
                    kind,
                    pos: Pos::new((at.x + p.pos.x).round(), (at.y + p.pos.y).round()),
                }),
            };
            scratch.apply(e.clone());
            edits.push(e);
            ids.push(id);
        }
        for &(fp, fpin, tp, tpin) in &clip.wires {
            if let (Some(f), Some(t)) = (ids.get(fp), ids.get(tp)) {
                edits.push(Edit::Wire {
                    to: End::new(*t, tpin),
                    from: Some(End::new(*f, fpin)),
                });
            }
        }
        for (name, members) in &clip.modules {
            let id = scratch.new_id();
            let members: Vec<Id> = members
                .iter()
                .filter_map(|i| ids.get(*i).copied())
                .collect();
            let e = Edit::Module {
                id,
                module: Some(makine_world::Module {
                    name: name.clone(),
                    pos: scratch.corner(&members),
                    members,
                }),
            };
            scratch.apply(e.clone());
            edits.push(e);
        }
        for (i, pin, style) in &clip.styles {
            if let Some(id) = ids.get(*i) {
                let mut style = style.clone();
                // Coordinates are relative to the copied origin (see copy).
                for p in &mut style.points {
                    p.x += at.x;
                    p.y += at.y;
                }
                edits.push(Edit::Style {
                    to: End::new(*id, *pin),
                    style: Some(style),
                });
            }
        }
        edits.extend(self.adopt(&ids));
        self.edit(Edit::Batch(edits));
        self.selected = ids.into_iter().collect();
        self.selected_module = None;
    }

    /// Copy and paste the selection a little lower and to the right.
    pub fn duplicate(&mut self) {
        let first = self
            .selected
            .first()
            .and_then(|id| self.world.part(*id))
            .map(|p| p.pos);
        if let (Some(_), Some(p)) = (self.copy(), first) {
            let clip = self.clipboard.clone();
            self.paste(None, pos2(p.x + 24.0, p.y + 24.0));
            // The clipboard keeps what the player copied before.
            self.clipboard = clip;
        }
    }

    /// Where a click on the board puts the part in hand: the pointer, or
    /// the middle of the board.
    fn drop_point(&self) -> Pos2 {
        self.hover_pos
            .unwrap_or_else(|| ((self.board.center() - self.origin) / self.zoom).to_pos2())
    }

    /// The selected part, if exactly one is selected.
    pub fn single(&self) -> Option<(Id, &Placed)> {
        if self.selected.len() != 1 {
            return None;
        }
        let id = *self.selected.first()?;
        Some((id, self.world.part(id)?))
    }

    /// Whether typed text goes to a keyboard device.
    pub fn typing(&self) -> bool {
        matches!(
            self.single(),
            Some((
                _,
                Placed {
                    kind: Kind::Device(Device::Keyboard),
                    ..
                }
            ))
        )
    }

    /// Contents of RAM part `id`, if it is running.
    pub fn ram_words(&mut self, id: Id) -> Option<Vec<u64>> {
        let (sim, name) = self.sim.device(id)?;
        device::ram(sim, &name)
    }

    /// Set a screen's pixels directly (for screenshots).
    #[cfg(test)]
    pub fn set_screen_for_test(&mut self, id: Id, f: impl Fn(usize, usize) -> bool) {
        self.sim.run_frame(&self.world);
        let (sim, name) = self.sim.device(id).unwrap();
        let s: &mut makine_sim::Clocked<device::ScreenState> = sim.native_mut(&name).unwrap();
        let n = 1usize << s.state.bits;
        for y in 0..n {
            for x in 0..n {
                s.state.pixels[y * n + x] = f(x, y);
            }
        }
    }

    /// Fit the camera to the world at the next frame.
    pub fn refit(&mut self) {
        self.fit = true;
    }

    fn layout(&self, origin: Pos2) -> Layout {
        Layout::new(
            &self.world,
            self.sim.library(),
            origin,
            self.zoom,
            self.symbols,
        )
        .project(&self.world, &self.collapsed, self.scope.last().copied())
    }

    fn fit_to(&mut self, board: Rect) {
        let l = self.layout(board.min);
        let Some(b) = l.bounds() else {
            self.zoom = 1.0;
            self.offset = board.size() / 2.0 - vec2(60.0, 40.0);
            return;
        };
        let b = b.expand(40.0);
        self.zoom = (board.width() / b.width())
            .min(board.height() / b.height())
            .clamp(0.03, 1.5);
        self.offset = board.size() / 2.0 - b.center().to_vec2() * self.zoom;
    }

    /// Pins grab from this far away, in screen points.
    fn pin_hit(&self) -> f32 {
        10.0 * self.zoom.max(0.8)
    }

    fn hover_at(&self, l: &Layout, p: Pos2, wires: bool) -> Hover {
        let hit = self.pin_hit();
        let closest = |it: Box<dyn Iterator<Item = (End, Pos2)> + '_>| {
            it.filter(|(_, sp)| sp.distance(p) < hit)
                .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
        };
        let mut h = Hover {
            source: closest(Box::new(l.sources())),
            sink: closest(Box::new(l.sinks())),
            part: l.part_at(p),
            wire: None,
        };
        if wires && h.source.is_none() && h.sink.is_none() && h.part.is_none() {
            h.wire = self.wire_at(l, p, 6.0 * self.zoom.max(0.8));
        }
        h
    }

    /// The wire (by its input) passing within `hit` of screen point `p`.
    fn wire_at(&self, l: &Layout, p: Pos2, hit: f32) -> Option<End> {
        self.world
            .wires()
            .filter(|(from, to)| l.wire_visible(*from, *to))
            .filter_map(|(from, to)| {
                let d = routing::wire_distance(&self.world, l, from, to, p)?;
                (d < hit).then_some((to, d))
            })
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|(to, _)| to)
    }

    /// Whether a wire may go from output `from` into input `to`.
    fn fits(&self, l: &Layout, from: End, to: End) -> bool {
        l.sink_aliases(to)
            .into_iter()
            .all(|to| match (l.output_pin(from), l.input_pin(to)) {
                (Some(f), Some(t)) => makine_world::compile::fits(f.width, t.width),
                _ => false,
            })
    }

    pub fn show(&mut self, ui: &mut Ui) {
        self.after_remote_edits();
        self.breadcrumbs(ui);
        self.map_ui(ui);
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let board = response.rect;
        self.board = board;
        if std::mem::take(&mut self.fit) {
            self.fit_to(board);
        }
        self.zoom_at_pointer(ui, &response);
        let origin = board.min + self.offset;
        self.origin = origin;
        let l = self.layout(origin);
        let painter = painter.with_clip_rect(board);
        let pointer = ui
            .ctx()
            .pointer_interact_pos()
            .filter(|p| board.contains(*p));
        self.hover_pos = pointer.map(|p| l.to_world(p));
        let hover = pointer.map_or_else(Hover::default, |p| {
            self.hover_at(&l, p, self.drag == Drag::None)
        });

        self.keys(ui, &response, &hover);
        self.gestures(ui, &response, &l, &hover);
        self.clicks(ui, &response, &hover);
        self.context_menu(ui, &response, &hover);
        // Parts dropped from the side panel.
        if let (Some(kind), Some(p)) = (response.dnd_release_payload::<PaletteItem>(), pointer) {
            let id = self.add(kind.0.clone(), l.to_world(p) - vec2(30.0, 20.0));
            self.selected = [id].into();
            self.selected_module = None;
        }

        self.sim.run_frame(&self.world);
        self.screens = draw::read_screens(self);
        // Layout again: this frame's edits moved or added parts.
        let l = self.layout(origin);
        self.draw(&painter, board, &l, &hover);
        if let (Drag::Wire(from), Some(p)) = (self.drag, pointer) {
            self.draw_new_wire(&painter, &l, from, p, &hover);
        }
        self.set_cursor(ui, &hover);
        let clocks = self
            .world
            .parts()
            .any(|(_, p)| matches!(p.kind, Kind::Clock { .. }));
        if (self.sim.running && clocks) || self.sim.advancing_cycle() || self.typing() {
            ui.ctx().request_repaint();
        }
    }

    /// The wheel (or a pinch) zooms around the pointer.
    fn zoom_at_pointer(&mut self, ui: &Ui, response: &egui::Response) {
        let Some(hp) = response.hover_pos() else {
            return;
        };
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let f = zoom * (scroll * 0.0015).exp();
        if f != 1.0 {
            let new = (self.zoom * f).clamp(0.05, 3.0);
            let at = hp - response.rect.min;
            let world = (at - self.offset) / self.zoom;
            self.offset = at - world * new;
            self.zoom = new;
        }
    }

    fn keys(&mut self, ui: &Ui, response: &egui::Response, hover: &Hover) {
        if self.typing() {
            let keys: Vec<u8> = ui.input(|i| {
                let mut out = Vec::new();
                for e in &i.events {
                    match e {
                        egui::Event::Text(t) => {
                            out.extend(t.chars().filter(char::is_ascii).map(|c| c as u8))
                        }
                        egui::Event::Key {
                            key, pressed: true, ..
                        } => match key {
                            egui::Key::Enter => out.push(b'\n'),
                            egui::Key::Backspace => out.push(8),
                            egui::Key::Escape => out.push(27),
                            _ => {}
                        },
                        _ => {}
                    }
                }
                out
            });
            if keys.contains(&27) {
                self.selected.clear();
            } else if let Some(id) = self.selected.first().copied() {
                for k in keys {
                    self.sim.press(id, k);
                }
            }
            return;
        }
        if !(response.hovered() || response.has_focus()) || ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        use egui::Key;
        if ui.input(|i| i.key_pressed(Key::Escape))
            && self.routing.take().is_none()
            && self.placing.take().is_none()
        {
            // Esc with nothing selected goes up a level.
            if self.selected.is_empty() && self.selected_module.is_none() {
                self.leave_to(self.scope.len().saturating_sub(1));
            }
            self.selected.clear();
            self.selected_module = None;
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            self.routing = None;
        }
        let events = ui.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Copy => {
                    if let Some(t) = self.copy() {
                        ui.ctx().copy_text(t);
                    }
                }
                egui::Event::Cut => {
                    if let Some(t) = self.cut() {
                        ui.ctx().copy_text(t);
                    }
                }
                egui::Event::Paste(t) => {
                    let at = self.drop_point();
                    self.paste(Some(&t), at);
                }
                _ => {}
            }
        }
        if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            if !self.selected.is_empty() {
                self.delete_selected();
            } else if let Some(to) = hover.wire {
                let l = self.layout(self.origin);
                self.edit(Edit::Batch(
                    l.sink_aliases(to)
                        .into_iter()
                        .map(|to| Edit::Wire { to, from: None })
                        .collect(),
                ));
            }
        }
        let (undo, redo, dup, all) = ui.input(|i| {
            let cmd = i.modifiers.command;
            (
                cmd && !i.modifiers.shift && i.key_pressed(Key::Z),
                cmd && (i.key_pressed(Key::Y) || (i.modifiers.shift && i.key_pressed(Key::Z))),
                cmd && i.key_pressed(Key::D),
                cmd && i.key_pressed(Key::A),
            )
        });
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if dup {
            self.duplicate();
        }
        if all {
            let l = self.layout(self.origin);
            self.selected = self
                .world
                .parts()
                .map(|(id, _)| id)
                .filter(|id| !l.outside(*id))
                .collect();
        }
        const DIGITS: [Key; 9] = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        let (digit, pause, step, home) = ui.input(|i| {
            let plain = !i.modifiers.ctrl && !i.modifiers.command && !i.modifiers.alt;
            let k = |key| plain && i.key_pressed(key);
            (
                DIGITS.iter().position(|d| k(*d)),
                k(Key::P),
                k(Key::N),
                k(Key::Home),
            )
        });
        if let Some(item) = digit.and_then(|d| palette().into_iter().nth(d)) {
            self.placing = Some(item.kind);
        }
        if pause {
            self.sim.running = !self.sim.running;
        }
        if step && !self.sim.running {
            self.sim.single_step();
        }
        if home {
            self.refit();
        }
    }

    fn gestures(&mut self, ui: &Ui, response: &egui::Response, l: &Layout, hover: &Hover) {
        let z = self.zoom;
        if response.drag_started_by(egui::PointerButton::Primary) {
            // A drag is recognized only once the pointer has moved; look at
            // what was under it when it was pressed.
            let press = ui
                .input(|i| i.pointer.press_origin())
                .or(ui.ctx().pointer_interact_pos());
            let at = press.map_or_else(Hover::default, |p| self.hover_at(l, p, false));
            let shift = ui.input(|i| i.modifiers.shift);
            let held = at
                .sink
                .and_then(|(to, _)| Some((to, self.world.wire_into(to)?)));
            let module = press.and_then(|p| l.module_at(p));
            self.drag = if let Some((from, _)) = at.source {
                Drag::Wire(from)
            } else if let Some((to, from)) = held {
                // Pulling a wire off an input: drop it on another input to
                // move it, or on empty board to cut it.
                self.edit(Edit::Batch(
                    l.sink_aliases(to)
                        .into_iter()
                        .map(|to| Edit::Wire { to, from: None })
                        .collect(),
                ));
                Drag::Wire(from)
            } else if let Some(id) = module.filter(|_| !shift) {
                self.selected_module = Some(id);
                self.selected = self
                    .world
                    .module(id)
                    .map(|m| m.members.iter().copied().collect())
                    .unwrap_or_default();
                self.start_move();
                Drag::Parts
            } else if let Some(id) = at.part.filter(|_| !shift) {
                self.selected_module = None;
                if !self.selected.contains(&id) {
                    self.selected = [id].into();
                }
                self.start_move();
                Drag::Parts
            } else if shift {
                Drag::Select(l.to_world(press.unwrap_or_default()))
            } else {
                Drag::Pan
            };
        } else if response.drag_started_by(egui::PointerButton::Middle) {
            self.drag = Drag::Pan;
        }
        if response.dragged() {
            let d = response.drag_delta();
            match self.drag {
                Drag::Pan => self.offset += d,
                Drag::Parts => {
                    // Moved locally every frame; one edit when released.
                    for id in self.selected.clone() {
                        if let Some(mut p) = self.world.part(id).cloned() {
                            p.pos = Pos::new(p.pos.x + d.x / z, p.pos.y + d.y / z);
                            self.world.apply(Edit::Part { id, part: Some(p) });
                        }
                    }
                    for (id, _) in self.moving_modules.clone() {
                        if let Some(mut m) = self.world.module(id).cloned() {
                            m.pos = m.pos.map(|p| Pos::new(p.x + d.x / z, p.y + d.y / z));
                            self.world.apply(Edit::Module {
                                id,
                                module: Some(m),
                            });
                        }
                    }
                }
                Drag::Select(a) => {
                    if let Some(b) = self.hover_pos {
                        let r = Rect::from_two_pos(a, b);
                        self.selected = l
                            .parts
                            .iter()
                            .filter(|(id, _, _)| {
                                l.visible(*id)
                                    && l.part_rect(*id).is_some_and(|pr| r.intersects(pr))
                            })
                            .map(|(id, _, _)| *id)
                            .collect();
                        for m in l
                            .modules
                            .iter()
                            .filter(|m| m.closed && r.intersects(m.rect))
                        {
                            if let Some(m) = self.world.module(m.id) {
                                self.selected.extend(m.members.iter().copied());
                            }
                        }
                        self.selected_module = None;
                    }
                }
                _ => {}
            }
        }
        if response.drag_stopped() {
            match self.drag {
                Drag::Wire(from) => {
                    if let Some((to, _)) = hover.sink {
                        if self.fits(l, from, to) {
                            self.edit(Edit::Batch(
                                l.sink_aliases(to)
                                    .into_iter()
                                    .map(|to| Edit::Wire {
                                        to,
                                        from: Some(from),
                                    })
                                    .collect(),
                            ));
                        } else {
                            self.status = Some(
                                tr!(
                                    "Bu iki ucun genişliği uymuyor.",
                                    "These two pins have different widths."
                                )
                                .into(),
                            );
                        }
                    }
                }
                Drag::Parts => self.finish_move(),
                _ => {}
            }
            self.drag = Drag::None;
        }
    }

    /// Start moving the selected parts, and the modules all of whose
    /// parts are selected.
    fn start_move(&mut self) {
        self.moving = Some(
            self.selected
                .iter()
                .filter_map(|id| Some((*id, self.world.part(*id)?.pos)))
                .collect(),
        );
        self.moving_modules = self
            .world
            .modules()
            .filter(|(_, m)| {
                m.pos.is_some() && m.members.iter().all(|id| self.selected.contains(id))
            })
            .map(|(id, m)| (id, m.clone()))
            .collect();
    }

    /// End a move: put the parts back where they were and move them in one
    /// edit, snapped to whole units.
    fn finish_move(&mut self) {
        let Some(start) = self.moving.take() else {
            return;
        };
        let delta = start
            .first()
            .and_then(|(id, old)| {
                let now = self.world.part(*id)?.pos;
                Some(vec2(now.x.round() - old.x, now.y.round() - old.y))
            })
            .unwrap_or_default();
        let moved: BTreeSet<_> = start.iter().map(|(id, _)| *id).collect();
        let mut edits = Vec::new();
        // A module without a place of its own (from an older save) sits at
        // its parts' corner: pin it where it was, so moving some of its
        // insides does not move it.
        let before: std::collections::HashMap<Id, Pos> = start.iter().copied().collect();
        for (id, m) in self.world.modules() {
            let inside = m.members.iter().filter(|p| moved.contains(p)).count();
            if m.pos.is_some() || inside == 0 || inside == m.members.len() {
                continue;
            }
            let corner = m
                .members
                .iter()
                .filter_map(|p| {
                    before
                        .get(p)
                        .copied()
                        .or(self.world.part(*p).map(|p| p.pos))
                })
                .reduce(|a, b| Pos::new(a.x.min(b.x), a.y.min(b.y)));
            let mut m = m.clone();
            m.pos = corner;
            edits.push(Edit::Module {
                id,
                module: Some(m),
            });
        }
        for (id, from) in start {
            let Some(mut p) = self.world.part(id).cloned() else {
                continue;
            };
            let to = Pos::new(p.pos.x.round(), p.pos.y.round());
            p.pos = from;
            self.world.apply(Edit::Part {
                id,
                part: Some(p.clone()),
            });
            if to != from {
                p.pos = to;
                edits.push(Edit::Part { id, part: Some(p) });
            }
        }
        for (id, from) in std::mem::take(&mut self.moving_modules) {
            let Some(mut m) = self.world.module(id).cloned() else {
                continue;
            };
            let to = m.pos.map(|p| Pos::new(p.x.round(), p.y.round()));
            self.world.apply(Edit::Module {
                id,
                module: Some(from.clone()),
            });
            if to != from.pos {
                m.pos = to;
                edits.push(Edit::Module {
                    id,
                    module: Some(m),
                });
            }
        }
        // Routes entirely inside the moved group move with their parts.
        // Boundary wires retain their external waypoints.
        for (from, to) in self.world.wires() {
            if moved.contains(&from.part) && moved.contains(&to.part) {
                if let Some(mut style) = self
                    .world
                    .wire_style(to)
                    .cloned()
                    .filter(|s| !s.points.is_empty())
                {
                    for p in &mut style.points {
                        p.x += delta.x;
                        p.y += delta.y;
                    }
                    edits.push(Edit::Style {
                        to,
                        style: Some(style),
                    });
                }
            }
        }
        if !edits.is_empty() {
            self.edit(Edit::Batch(edits));
        }
    }

    fn clicks(&mut self, ui: &Ui, response: &egui::Response, hover: &Hover) {
        if response.double_clicked() {
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                if let Some(id) = self.layout(self.origin).module_at(p) {
                    self.enter_module(id);
                    return;
                }
            }
        }
        if !response.clicked() {
            return;
        }
        if let Some(to) = self.routing {
            if let Some(p) = self.hover_pos {
                let mut s = self.world.wire_style(to).cloned().unwrap_or_default();
                s.points.push(Pos::new(p.x.round(), p.y.round()));
                self.edit(Edit::Style { to, style: Some(s) });
            }
            return;
        }
        if let Some(p) = ui.ctx().pointer_interact_pos() {
            if let Some(id) = self.layout(self.origin).module_at(p) {
                self.selected_module = Some(id);
                self.selected = self
                    .world
                    .module(id)
                    .map(|m| m.members.iter().copied().collect())
                    .unwrap_or_default();
                return;
            }
        }
        if let Some(kind) = self.placing.clone() {
            let at = self.drop_point() - vec2(30.0, 20.0);
            let id = self.add(kind, at);
            self.selected = [id].into();
            self.selected_module = None;
            // Keep the part in hand to place more; Esc lets go.
            return;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        self.selected_module = None;
        let Some(id) = hover.part else {
            if !shift {
                self.selected.clear();
            }
            return;
        };
        if shift {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
            return;
        }
        // Clicking a switch flips it, clicking an input changes its value.
        let p = self.world.part(id).cloned();
        let new = match p.as_ref().map(|p| &p.kind) {
            Some(Kind::Switch { on }) => Some(Kind::Switch { on: !on }),
            Some(Kind::Input { name, width, value }) => {
                let v = if *width == 1 {
                    value ^ 1
                } else {
                    value.wrapping_add(1) & mask(*width)
                };
                Some(Kind::Input {
                    name: name.clone(),
                    width: *width,
                    value: v,
                })
            }
            _ => None,
        };
        if let (Some(kind), Some(p)) = (new, p) {
            self.edit(Edit::Part {
                id,
                part: Some(Placed { kind, pos: p.pos }),
            });
        }
        self.selected = [id].into();
    }

    /// Right click: a part's or wire's menu.
    fn context_menu(&mut self, ui: &Ui, response: &egui::Response, hover: &Hover) {
        let menu_id = EguiId::unique("world-menu-target");
        if response.secondary_clicked() {
            if let Some(id) = hover.part {
                if !self.selected.contains(&id) {
                    self.selected = [id].into();
                }
            }
            let target = match (hover.sink, hover.part, hover.wire) {
                (Some((to, _)), _, _) if self.world.wire_into(to).is_some() => Menu::Wire(to),
                (_, Some(_), _) => Menu::Part,
                (_, _, Some(to)) => Menu::Wire(to),
                _ => Menu::Board,
            };
            ui.ctx().data_mut(|d| d.insert_temp(menu_id, target));
        }
        response.context_menu(|ui| {
            let target = ui
                .ctx()
                .data(|d| d.get_temp::<Menu>(menu_id))
                .unwrap_or(Menu::Board);
            match target {
                Menu::Wire(to) => {
                    if ui
                        .button(tr!("Tel adı / yolunu düzenle", "Edit wire name / path"))
                        .clicked()
                    {
                        self.selected_wire = Some(to);
                        ui.close();
                    }
                    if ui.button(tr!("Sinyali izle", "Watch signal")).clicked() {
                        if let Some(from) = self.world.wire_into(to) {
                            self.sim.watch(from);
                        }
                        ui.close();
                    }
                    if ui.button(tr!("✂ Kabloyu kes", "✂ Cut wire")).clicked() {
                        let l = self.layout(self.origin);
                        self.edit(Edit::Batch(
                            l.sink_aliases(to)
                                .into_iter()
                                .map(|to| Edit::Wire { to, from: None })
                                .collect(),
                        ));
                        ui.close();
                    }
                }
                Menu::Part => {
                    let n = self.selected.len();
                    if ui
                        .button(tr!("Çoğalt (Ctrl+D)", "Duplicate (Ctrl+D)"))
                        .clicked()
                    {
                        self.duplicate();
                        ui.close();
                    }
                    if ui
                        .button(tr!("Kopyala (Ctrl+C)", "Copy (Ctrl+C)"))
                        .clicked()
                    {
                        if let Some(t) = self.copy() {
                            ui.ctx().copy_text(t);
                        }
                        ui.close();
                    }
                    let label = if n > 1 {
                        trf!("{n} parçayı sil", "Delete {n} parts")
                    } else {
                        tr!("Sil", "Delete").into()
                    };
                    if ui.button(label).clicked() {
                        self.delete_selected();
                        ui.close();
                    }
                }
                Menu::Board => {
                    if ui
                        .add_enabled(
                            self.clipboard.is_some(),
                            egui::Button::new(tr!("Yapıştır", "Paste")),
                        )
                        .clicked()
                    {
                        let at = self.drop_point();
                        self.paste(None, at);
                        ui.close();
                    }
                    ui.label(
                        egui::RichText::new(tr!(
                            "Parça eklemek için sağdaki listeden sürükle ya da tıkla.",
                            "To add a part, drag or click it in the list on the right."
                        ))
                        .weak()
                        .small(),
                    );
                }
            }
        });
    }

    fn set_cursor(&self, ui: &Ui, hover: &Hover) {
        let icon = match self.drag {
            Drag::Parts | Drag::Pan => egui::CursorIcon::Grabbing,
            Drag::Wire(_) | Drag::Select(_) => egui::CursorIcon::Crosshair,
            Drag::None if self.placing.is_some() => egui::CursorIcon::Copy,
            Drag::None if hover.source.is_some() || hover.sink.is_some() => {
                egui::CursorIcon::Crosshair
            }
            Drag::None if hover.wire.is_some() => egui::CursorIcon::PointingHand,
            Drag::None => match hover.part.and_then(|id| self.world.part(id)) {
                Some(Placed {
                    kind: Kind::Switch { .. } | Kind::Input { .. },
                    ..
                }) => egui::CursorIcon::PointingHand,
                Some(_) => egui::CursorIcon::Grab,
                None => return,
            },
        };
        ui.ctx().set_cursor_icon(icon);
    }
}

#[derive(Clone, Copy, Debug)]
enum Menu {
    Board,
    Part,
    Wire(End),
}

/// `kind` with a port name no part of `world` uses yet (counting up: a, b,
/// c …; x0, x1 …).
pub fn free_name(world: &World, mut kind: Kind) -> Kind {
    let taken: BTreeSet<String> = world
        .parts()
        .filter_map(|(_, p)| p.kind.port_name().map(str::to_string))
        .collect();
    if let Kind::Input { name, .. } | Kind::Output { name, .. } = &mut kind {
        for _ in 0..10_000 {
            if !taken.contains(name.as_str()) {
                break;
            }
            *name = next_name(name);
        }
    }
    kind
}

/// The name after `name`: the number at its end counts up (`x0` → `x1`,
/// `q9` → `q10`), else its last letter does (`a` → `b`, `z` → `aa`).
pub fn next_name(name: &str) -> String {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        let (base, num) = name.split_at(name.len() - digits);
        if let Ok(n) = num.parse::<u64>() {
            return format!("{base}{}", n + 1);
        }
    }
    let mut chars: Vec<char> = name.chars().collect();
    // Count like a spreadsheet's columns over a..z (A..Z keeps its case).
    let mut i = chars.len();
    loop {
        if i == 0 {
            let first = if chars.first().is_some_and(|c| c.is_ascii_uppercase()) {
                'A'
            } else {
                'a'
            };
            chars.insert(0, first);
            break;
        }
        i -= 1;
        match chars[i] {
            'z' => chars[i] = 'a',
            'Z' => chars[i] = 'A',
            c if c.is_ascii_alphabetic() => {
                chars[i] = (c as u8 + 1) as char;
                break;
            }
            // Something else at the end (`_`): start numbering.
            _ => return format!("{name}1"),
        }
    }
    chars.into_iter().collect()
}
