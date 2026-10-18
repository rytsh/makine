//! Browser-style editor documents. Designs stay in Progress; only local editor
//! state is parked/swapped, so tabs and detached windows edit the same saved work.
use super::{App, Target, TestResult, View};
use crate::{
    canvas::Canvas,
    design::{Design, History},
    sandbox::PinEditor,
    theme,
};
use egui::{Pos2, RichText, Sense, Ui};
use makine_lang::{tr, trf};
use makine_sim::Circuit;

type TabId = u64;

#[derive(Default)]
pub(super) struct Workspace {
    tabs: Vec<EditorTab>,
    active: Option<TabId>,
    next: TabId,
    pub scroll_to_active: bool,
}

impl Workspace {
    pub(super) fn has_detached(&self) -> bool {
        self.tabs.iter().any(|tab| tab.detached)
    }
}

struct EditorTab {
    id: TabId,
    target: Target,
    state: Option<EditorState>,
    detached: bool,
    position: Option<Pos2>,
    restore_drag: bool,
    /// The editor's first document stays open; only tabs opened later close.
    closable: bool,
}

struct EditorState {
    target: Target,
    level: &'static makine_levels::Level,
    canvas: Canvas,
    sim: Result<Circuit, String>,
    inputs: Vec<u64>,
    oscillating: bool,
    result: Option<TestResult>,
    history: History,
    snapshot: Option<Design>,
    cost: Option<makine_levels::cost::Cost>,
    delay: Option<makine_levels::depth::Delay>,
    mini: Option<super::mini_ui::MiniRun>,
    parts: super::parts_ui::PartsBrowser,
    inspector: super::inspector_ui::Inspector,
    rename: String,
    pins: Option<PinEditor>,
    reveal: super::solution_ui::Reveal,
    editor_code: bool,
    revision: u64,
    dependencies: std::collections::BTreeMap<String, Option<makine_sim::ChipDef>>,
    fast: bool,
}

fn target_name(target: &Target) -> String {
    match target {
        Target::Level(level) => level.name().into(),
        Target::Free(name) => name.clone(),
    }
}

fn target_exists(app: &App, target: &Target) -> bool {
    match target {
        Target::Level(_) => true,
        Target::Free(name) => app.progress.sandbox.chips.contains_key(name),
    }
}

impl App {
    fn take_editor(&mut self) -> EditorState {
        let def = self.design().def.clone();
        let dependencies = chip_dependencies(&self.lib, &def);
        EditorState {
            target: self.target.clone(),
            level: self.level,
            canvas: std::mem::replace(&mut self.canvas, Canvas::new()),
            sim: std::mem::replace(&mut self.sim, Err(String::new())),
            inputs: std::mem::take(&mut self.inputs),
            oscillating: self.oscillating,
            result: self.result.take(),
            history: std::mem::take(&mut self.history),
            snapshot: self.snapshot.take(),
            cost: self.cost.take(),
            delay: self.delay.take(),
            mini: self.mini.take(),
            parts: std::mem::take(&mut self.parts_browser),
            inspector: std::mem::take(&mut self.inspector),
            rename: std::mem::take(&mut self.rename_to),
            pins: self.pin_editor.take(),
            reveal: self.reveal,
            editor_code: self.editor_code,
            revision: self.library_revision,
            dependencies,
            fast: self.progress.fast,
        }
    }

    fn restore_editor(&mut self, state: EditorState) {
        let revision = state.revision;
        let changed = state.fast != self.progress.fast
            || state
                .dependencies
                .iter()
                .any(|(name, def)| self.lib.get(name) != def.as_ref());
        self.target = state.target;
        self.level = state.level;
        self.canvas = state.canvas;
        self.canvas.symbols = self.progress.symbols;
        self.sim = state.sim;
        self.inputs = state.inputs;
        self.oscillating = state.oscillating;
        self.result = state.result;
        self.history = state.history;
        self.snapshot = state.snapshot;
        self.cost = state.cost;
        self.delay = state.delay;
        self.mini = state.mini;
        self.parts_browser = state.parts;
        self.inspector = state.inspector;
        self.rename_to = state.rename;
        self.pin_editor = state.pins;
        self.reveal = state.reveal;
        self.editor_code = state.editor_code;
        if revision != self.library_revision && changed {
            self.rebuild();
            self.update_cost();
        }
    }

    fn park_active_editor(&mut self) {
        if let Some(id) = self.workspace.active {
            let state = self.take_editor();
            if let Some(tab) = self.workspace.tabs.iter_mut().find(|t| t.id == id) {
                tab.target = state.target.clone();
                tab.state = Some(state);
            }
        }
    }

    pub(super) fn open_editor_tab(&mut self, target: Target) {
        if let Some(id) = self.workspace.active {
            if let Some(tab) = self.workspace.tabs.iter_mut().find(|t| t.id == id) {
                tab.target = self.target.clone();
            }
        }
        if let Some(id) = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.target == target)
            .map(|t| t.id)
        {
            self.activate_editor_tab(id);
            return;
        }
        self.park_active_editor();
        self.initialize_target(target.clone());
        self.workspace.next += 1;
        let id = self.workspace.next;
        let closable = !self.workspace.tabs.is_empty();
        self.workspace.tabs.push(EditorTab {
            id,
            target,
            state: None,
            detached: false,
            position: None,
            restore_drag: false,
            closable,
        });
        self.workspace.active = Some(id);
        self.workspace.scroll_to_active = true;
    }

    fn activate_editor_tab(&mut self, id: TabId) {
        self.workspace.scroll_to_active = true;
        if self.workspace.active != Some(id) {
            let Some(index) = self.workspace.tabs.iter().position(|t| t.id == id) else {
                return;
            };
            self.park_active_editor();
            if let Some(state) = self.workspace.tabs[index].state.take() {
                self.restore_editor(state);
            }
            self.workspace.active = Some(id);
        }
        if let Some(tab) = self.workspace.tabs.iter_mut().find(|t| t.id == id) {
            tab.detached = false;
        }
        if let Some(example) = &mut self.solution {
            example.active = false;
        }
        self.editor_code = false;
        self.view = View::Editor;
        self.just_opened = true;
    }

    fn close_editor_tab(&mut self, id: TabId) {
        let Some(index) = self.workspace.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let was_active = self.workspace.active == Some(id);
        self.workspace.tabs.remove(index);
        if was_active {
            self.workspace.active = None;
            if let Some(next) = self
                .workspace
                .tabs
                .get(index.min(self.workspace.tabs.len().saturating_sub(1)))
                .map(|t| t.id)
            {
                self.activate_editor_tab(next);
            } else {
                self.view = View::Home;
            }
        }
        self.save_soon();
    }

    fn detach_editor_tab(&mut self, id: TabId, position: Option<Pos2>) {
        if self.workspace.active == Some(id) {
            self.park_active_editor();
            self.workspace.active = None;
            if let Some(next) = self
                .workspace
                .tabs
                .iter()
                .find(|t| t.id != id && !t.detached)
                .map(|t| t.id)
            {
                self.activate_editor_tab(next);
            } else {
                // Keep a harmless root editor state while its only document is
                // in a child window. The home page offers reopening its tab.
                self.view = View::Home;
            }
        }
        if let Some(tab) = self.workspace.tabs.iter_mut().find(|t| t.id == id) {
            tab.detached = true;
            tab.position = position;
        }
    }

    fn dock_editor_tab(&mut self, id: TabId) {
        if let Some(tab) = self.workspace.tabs.iter_mut().find(|t| t.id == id) {
            tab.detached = false;
            tab.restore_drag = false;
        }
        self.activate_editor_tab(id);
    }

    /// A horizontally scrolling document strip; controls do not wrap into
    /// another row as more chips open. Stable ids survive ordering and rename.
    pub(super) fn workspace_tabs(&mut self, ui: &mut Ui) {
        if self.workspace.tabs.is_empty() && self.view == View::Editor {
            self.open_editor_tab(self.target.clone());
        }
        if self.workspace.active.is_none() && self.view == View::Editor && !self.showing_solution()
        {
            self.view = View::Home;
        }
        if self.view == View::Editor && !ui.ctx().text_edit_focused() {
            let (close, cycle, back) = ui.input_mut(|input| {
                let close = input.consume_key(egui::Modifiers::COMMAND, egui::Key::W);
                // egui accepts additional Shift for a command-only binding;
                // consume the more specific backwards shortcut first.
                let back = input.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Tab,
                );
                let cycle = !back && input.consume_key(egui::Modifiers::COMMAND, egui::Key::Tab);
                (close, cycle, back)
            });
            if close {
                if self.showing_solution() {
                    self.solution = None;
                } else if let Some(id) = self
                    .workspace
                    .active
                    .filter(|id| self.workspace.tabs.iter().any(|t| t.id == *id && t.closable))
                {
                    self.close_editor_tab(id);
                }
            }
            if cycle || back {
                let attached: Vec<_> = self
                    .workspace
                    .tabs
                    .iter()
                    .filter(|t| !t.detached)
                    .map(|t| t.id)
                    .collect();
                if !attached.is_empty() {
                    let at = attached
                        .iter()
                        .position(|id| Some(*id) == self.workspace.active)
                        .unwrap_or(0);
                    let next = if back {
                        (at + attached.len() - 1) % attached.len()
                    } else {
                        (at + 1) % attached.len()
                    };
                    self.activate_editor_tab(attached[next]);
                }
            }
        }
        let active = self.workspace.active;
        let example_active = self.showing_solution();
        let reveal_active = std::mem::take(&mut self.workspace.scroll_to_active);
        let tabs: Vec<_> = self
            .workspace
            .tabs
            .iter()
            .map(|t| (t.id, target_name(&t.target), t.detached, t.closable))
            .collect();
        let mut activate = None;
        let mut close = None;
        let mut detach = None;
        let mut reorder = None;
        egui::Panel::top("document-tabs")
            .frame(egui::Frame::new().fill(theme::WELL).inner_margin(6.))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("document-tab-scroll")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.;
                                for (id, name, detached, closable) in &tabs {
                                    let label = if *detached {
                                        format!("{name} · {}", tr!("pencere", "window"))
                                    } else {
                                        name.clone()
                                    };
                                    let (response, closed) = document_tab(
                                        ui,
                                        *id,
                                        &label,
                                        active == Some(*id) && !example_active,
                                        closable.then_some(None),
                                    );
                                    if reveal_active && active == Some(*id) && !example_active {
                                        ui.scroll_to_rect(
                                            response.rect.expand2(egui::vec2(32., 0.)),
                                            Some(egui::Align::Center),
                                        );
                                    }
                                    if response.clicked() {
                                        activate = Some(*id);
                                    }
                                    if closed
                                        || (*closable
                                            && response.clicked_by(egui::PointerButton::Middle))
                                    {
                                        close = Some(*id);
                                    }
                                    response.dnd_set_drag_payload(EditorDrag(*id));
                                    if let Some(dragged) =
                                        response.dnd_release_payload::<EditorDrag>()
                                    {
                                        if dragged.0 != *id {
                                            reorder = Some((dragged.0, *id));
                                        }
                                    }
                                    if !detached
                                        && response.drag_stopped()
                                        && response
                                            .interact_pointer_pos()
                                            .is_some_and(|p| p.y > response.rect.bottom() + 24.)
                                    {
                                        detach = Some((
                                            *id,
                                            drag_window_position(
                                                ui.ctx(),
                                                response.interact_pointer_pos(),
                                            ),
                                        ));
                                    }
                                }
                                if self.solution.is_some() {
                                    self.solution_tabs(ui, reveal_active);
                                }
                            });
                        });
                });
                if self.workspace.tabs.iter().any(|t| t.restore_drag)
                    || self.solution.as_ref().is_some_and(|s| s.return_drag)
                {
                    ui.label(
                        RichText::new(tr!(
                            "Pencereyi bu çubuğa bırak: sekmeye geri alınır.",
                            "Drop the window on this bar to dock as a tab."
                        ))
                        .small()
                        .color(theme::COPPER_BRIGHT),
                    );
                }
                let dock_drop = ui.interact(
                    ui.max_rect(),
                    ui.scope_id().with("tab-dock-target"),
                    Sense::hover(),
                );
                if let Some(drop) = dock_drop.dnd_release_payload::<DetachedDrag>() {
                    if drop.0 == 0 {
                        self.dock_solution();
                    } else {
                        self.dock_editor_tab(drop.0);
                    }
                }
                if let Some(root_rect) = ui.ctx().input(|i| i.viewport().inner_rect) {
                    let local = ui.max_rect();
                    let global = local.translate(root_rect.min.to_vec2());
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::unique("root-tab-strip"), global)
                    });
                    let ids: Vec<_> = self
                        .workspace
                        .tabs
                        .iter()
                        .filter(|t| t.restore_drag)
                        .map(|t| t.id)
                        .collect();
                    for id in ids {
                        let window = editor_viewport_id(id);
                        let over = ui.ctx().input(|i| {
                            i.raw
                                .viewports
                                .get(&window)
                                .and_then(|v| v.outer_rect)
                                .is_some_and(|rect| {
                                    global.contains(rect.min + egui::vec2(100., 20.))
                                })
                        });
                        if over {
                            self.dock_editor_tab(id);
                        }
                    }
                    if let Some(example) =
                        self.solution.as_ref().filter(|example| example.return_drag)
                    {
                        let window = example.viewport_id();
                        if ui.ctx().input(|input| {
                            input
                                .raw
                                .viewports
                                .get(&window)
                                .and_then(|view| view.outer_rect)
                                .is_some_and(|rect| {
                                    global.contains(rect.min + egui::vec2(100., 20.))
                                })
                        }) {
                            self.dock_solution();
                        }
                    }
                }
            });
        if let Some((from, to)) = reorder {
            if let (Some(from), Some(to)) = (
                self.workspace.tabs.iter().position(|t| t.id == from),
                self.workspace.tabs.iter().position(|t| t.id == to),
            ) {
                let tab = self.workspace.tabs.remove(from);
                self.workspace.tabs.insert(to, tab);
            }
        }
        if let Some(id) = activate {
            if tabs
                .iter()
                .any(|(tab, _, detached, _)| *tab == id && *detached)
            {
                ui.ctx()
                    .send_viewport_cmd_to(editor_viewport_id(id), egui::ViewportCommand::Focus);
            } else {
                self.activate_editor_tab(id);
            }
        }
        if let Some((id, position)) = detach {
            self.detach_editor_tab(id, position);
            ui.ctx().request_repaint();
        }
        if let Some(id) = close {
            self.close_editor_tab(id);
        }
    }

    pub(super) fn editor_windows(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.workspace.active {
            if let Some(tab) = self.workspace.tabs.iter_mut().find(|t| t.id == id) {
                tab.target = self.target.clone();
            }
        }
        self.workspace.tabs.retain(|tab| match &tab.target {
            Target::Level(_) => true,
            Target::Free(name) => self.progress.sandbox.chips.contains_key(name),
        });
        let ids: Vec<_> = self
            .workspace
            .tabs
            .iter()
            .filter(|t| t.detached)
            .map(|t| t.id)
            .collect();
        for id in ids {
            let Some(index) = self.workspace.tabs.iter().position(|t| t.id == id) else {
                continue;
            };
            let Some(state) = self.workspace.tabs[index].state.take() else {
                continue;
            };
            if !target_exists(self, &state.target) {
                continue;
            }
            let root = self.take_editor();
            let root_view = self.view;
            let code = state.editor_code;
            self.restore_editor(state);
            self.editor_code = false;
            let title = target_name(&self.target);
            let mut builder = egui::ViewportBuilder::default()
                .with_title(format!("{title} — makine"))
                .with_inner_size([1100., 760.])
                .with_min_inner_size([600., 420.])
                .with_decorations(!self.progress.custom_frame);
            if let Some(icon) = super::chrome::window_icon() {
                builder = builder.with_icon(icon);
            }
            if let Some(position) = self.workspace.tabs[index].position.take() {
                builder = builder.with_position(position);
            }
            let mut dock = false;
            let mut return_drag = false;
            ctx.show_viewport_immediate(editor_viewport_id(id),builder, |ui,class| {
                let native = class != egui::ViewportClass::EmbeddedWindow;
                ui.push_id(("editor-document",id), |ui| {
                    if native { dock |= super::chrome::child_window_frame(ui,&title,self.progress.custom_frame); }
                    dock |= native && ui.ctx().input(|i|i.viewport().close_requested());
                    egui::Panel::top("editor-window-tools").show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            dock |= ui.button(tr!("Sekmeye geri al", "Dock as tab")).clicked();
                            return_drag |= dock_drag_handle(ui,id);
                            ui.label(RichText::new(tr!("Düzenlemeler profilinde saklanır.", "Edits are saved to your profile.")).small().weak());
                        });
                    });
                    egui::Panel::left("detached-parts").default_size(150.).show(ui, |ui|self.parts_panel(ui));
                    egui::Panel::right("detached-inspector").default_size(300.).resizable(true).show(ui, |ui| {
                        // Only the detached document's circuit inspector is
                        // shown here; code remains in the root workbench.
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.heading(&title);
                            self.bus_inputs(ui);
                            self.undo_buttons(ui);
                            if matches!(self.target,Target::Level(_)) {
                                if ui.button(tr!("▶  Test et", "▶  Test")).clicked() { self.run_test(); }
                                if let Some(result) = &self.result {
                                    ui.label(match result { TestResult::Pass => tr!("Bütün testler geçti.", "Every test passed."),_ => tr!("Test geçmedi; devredeki bağlantıları ve girişleri kontrol et.", "The test failed; check the circuit's connections and inputs.") });
                                }
                            } else { ui.label(tr!("Serbest çip", "Free chip")); }
                        });
                    });
                    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
                        let output_x = (ui.available_width()/self.canvas.zoom.max(0.03)-110.).max(400.);
                        let lib = self.lib.clone(); let inputs = self.inputs.clone(); let sim = self.sim.as_ref().ok().cloned();
                        let out = self.canvas.show(ui,super::design_for(&mut self.progress,&self.target),&lib,sim.as_ref(),&inputs,output_x);
                        if let Some((pin,value)) = out.toggled_input { self.inputs[pin] = value; self.run(); }
                        if out.changed { self.design_changed_merging(out.merge); }
                        if out.undo { self.undo(); }
                        if out.redo { self.redo(); }
                    });
                });
            });
            self.editor_code = code;
            let state = self.take_editor();
            self.restore_editor(root);
            self.view = root_view;
            if let Some(tab) = self.workspace.tabs.iter_mut().find(|t| t.id == id) {
                tab.target = state.target.clone();
                tab.state = Some(state);
                tab.restore_drag |= return_drag;
            }
            if dock {
                self.dock_editor_tab(id);
            }
        }
    }
}

fn chip_dependencies(
    lib: &makine_sim::Library,
    def: &makine_sim::ChipDef,
) -> std::collections::BTreeMap<String, Option<makine_sim::ChipDef>> {
    let mut found = std::collections::BTreeMap::new();
    let mut pending: Vec<_> = def
        .parts
        .iter()
        .filter_map(|part| match &part.kind {
            makine_sim::PartKind::Chip(name) => Some(name.clone()),
            _ => None,
        })
        .collect();
    while let Some(name) = pending.pop() {
        if found.contains_key(&name) {
            continue;
        }
        let definition = lib.get(&name).cloned();
        if let Some(def) = &definition {
            pending.extend(def.parts.iter().filter_map(|part| match &part.kind {
                makine_sim::PartKind::Chip(name) => Some(name.clone()),
                _ => None,
            }));
        }
        found.insert(name, definition);
    }
    found
}

#[derive(Clone)]
struct EditorDrag(TabId);
#[derive(Clone)]
pub(super) struct DetachedDrag(pub TabId);

pub(super) fn editor_viewport_id(id: TabId) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("editor-document", id))
}

pub(super) fn drag_window_position(ctx: &egui::Context, pointer: Option<Pos2>) -> Option<Pos2> {
    ctx.input(|i| i.viewport().inner_rect)
        .zip(pointer)
        .map(|(rect, pointer)| rect.min + pointer.to_vec2() - egui::vec2(80., 16.))
}

pub(super) fn dock_drag_handle(ui: &mut Ui, id: TabId) -> bool {
    let response = ui.add(egui::Button::new(tr!("Çubuğa sürükle", "Drag to tab bar")).sense(Sense::click_and_drag()))
        .on_hover_text(tr!("Bu tutamacı ana pencerenin sekme çubuğuna sürükle. Sistemler arası sürükleme yoksa tıklayıp pencerenin başlığını çubuğa taşı.", "Drag this handle to the main window tab bar. If cross-window drag is unavailable, click it and move the window title over the bar."));
    response.dnd_set_drag_payload(DetachedDrag(id));
    if response.drag_started() && !ui.ctx().embed_viewports() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    response.clicked() || response.drag_started()
}

/// A browser-style tab: one bordered shape holding the title and, for tabs
/// that may close, the × inside its right edge. The title is painted, not a
/// label, so the whole tab stays a drag handle with the normal cursor.
pub(super) fn document_tab(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    selected: bool,
    close: Option<Option<&str>>,
) -> (egui::Response, bool) {
    const HEIGHT: f32 = 28.;
    const PAD: f32 = 12.;
    const CLOSE: f32 = 18.;
    let base = ui.make_persistent_id(&id);
    let close_width = if close.is_some() { CLOSE + 4. } else { 0. };
    let font = egui::TextStyle::Button.resolve(ui.style());
    let natural = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), egui::Color32::PLACEHOLDER)
        .size()
        .x;
    let width = (natural + 2. * PAD + close_width).clamp(96., 220.);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, HEIGHT), Sense::hover());
    let mut response = ui.interact(rect, base, Sense::click_and_drag());
    let text_width = width - 2. * PAD - close_width;
    let mut job = egui::text::LayoutJob::simple_singleline(
        label.to_string(),
        font,
        egui::Color32::PLACEHOLDER,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(text_width);
    let galley = ui.painter().layout_job(job);
    if galley.elided {
        response = response.on_hover_text(label);
    }

    let close_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - PAD / 2. - CLOSE / 2., rect.center().y),
        egui::vec2(CLOSE, CLOSE),
    );
    let closed = close.map(|name| {
        let closed = ui.interact(close_rect, base.with("close"), Sense::click());
        closed.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                true,
                name.map(str::to_string)
                    .unwrap_or_else(|| trf!("Sekmeyi kapat: {label}", "Close tab: {label}")),
            )
        });
        closed
    });
    let hovered = response.hovered() || closed.as_ref().is_some_and(|c| c.hovered());

    let painter = ui.painter();
    let corners = egui::CornerRadius {
        nw: 6,
        ne: 6,
        sw: 0,
        se: 0,
    };
    let fill = if selected {
        theme::PANEL
    } else if hovered {
        theme::RAISED.gamma_multiply(0.7)
    } else {
        theme::PANEL.gamma_multiply(0.45)
    };
    let border = if selected {
        theme::LINE
    } else {
        theme::LINE.gamma_multiply(0.6)
    };
    painter.rect(
        rect,
        corners,
        fill,
        egui::Stroke::new(1., border),
        egui::StrokeKind::Inside,
    );
    if selected {
        painter.line_segment(
            [
                rect.left_top() + egui::vec2(5., 1.),
                rect.right_top() + egui::vec2(-5., 1.),
            ],
            egui::Stroke::new(2., theme::COPPER),
        );
    }
    let ink = if selected || hovered {
        theme::INK
    } else {
        theme::INK_DIM
    };
    painter.galley(
        egui::pos2(rect.left() + PAD, rect.center().y - galley.size().y / 2.),
        galley,
        ink,
    );
    if let Some(closed) = &closed {
        if closed.hovered() {
            painter.circle_filled(close_rect.center(), CLOSE / 2., theme::RAISED_HOVER);
        }
        let mark = if closed.hovered() {
            theme::COPPER_BRIGHT
        } else if selected || hovered {
            theme::INK_DIM
        } else {
            theme::INK_DIM.gamma_multiply(0.6)
        };
        let c = close_rect.center();
        let h = 3.5;
        let stroke = egui::Stroke::new(1.4, mark);
        painter.line_segment([c + egui::vec2(-h, -h), c + egui::vec2(h, h)], stroke);
        painter.line_segment([c + egui::vec2(-h, h), c + egui::vec2(h, -h)], stroke);
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
    (response, closed.is_some_and(|c| c.clicked()))
}

#[cfg(test)]
mod tests;
