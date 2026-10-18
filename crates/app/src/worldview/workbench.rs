//! Module operations, alignment and a navigable overview map.
use super::WorldView;
use crate::theme;
use egui::{vec2, Align2, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use makine_lang::{tr, trf};
use makine_world::{Edit, Id, Module};

impl WorldView {
    pub fn group_selected(&mut self, name: &str) -> Result<Id, String> {
        let members: Vec<_> = self
            .selected
            .iter()
            .copied()
            .filter(|id| self.world.part(*id).is_some())
            .collect();
        if members.len() < 2 {
            return Err(tr!("En az iki parça seç.", "Select at least two parts.").into());
        }
        if name.trim().is_empty() {
            return Err(tr!("Modüle bir ad ver.", "Give the module a name.").into());
        }
        for (_, m) in self.world.modules() {
            let n = m
                .members
                .iter()
                .filter(|id| self.selected.contains(id))
                .count();
            if n == m.members.len() && n == members.len() {
                return Err(tr!(
                    "Bu parçalar zaten bir modül.",
                    "These parts are already a module."
                )
                .into());
            }
            if n != 0 && n != m.members.len() && n != members.len() {
                return Err(tr!(
                    "Modüller iç içe olabilir ama kısmen örtüşemez. Mevcut modülün tamamını seç.",
                    "Modules can be nested but can't partly overlap. Select the whole existing module."
                )
                .into());
            }
        }
        let id = self.world.new_id();
        self.edit(Edit::Module {
            id,
            module: Some(Module {
                name: name.trim().into(),
                pos: self.world.corner(&members),
                members,
            }),
        });
        self.selected_module = Some(id);
        self.collapsed.insert(id);
        Ok(id)
    }

    pub fn toggle_module(&mut self, id: Id) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.selected_module = Some(id);
        self.refit();
    }

    /// Align top edges or left edges. One edit, layout only.
    pub fn align_selected(&mut self, horizontal: bool) {
        let parts: Vec<_> = self
            .selected
            .iter()
            .filter_map(|id| Some((*id, self.world.part(*id)?.clone())))
            .collect();
        let target = parts
            .iter()
            .map(|(_, p)| if horizontal { p.pos.y } else { p.pos.x })
            .fold(f32::INFINITY, f32::min);
        if !target.is_finite() {
            return;
        }
        self.edit(Edit::Batch(
            parts
                .into_iter()
                .map(|(id, mut p)| {
                    if horizontal {
                        p.pos.y = target;
                    } else {
                        p.pos.x = target;
                    }
                    Edit::Part { id, part: Some(p) }
                })
                .collect(),
        ));
    }

    pub fn module_panel(&mut self, ui: &mut Ui) {
        if let Some((id, mut m)) = self
            .selected_module
            .and_then(|id| Some((id, self.world.module(id)?.clone())))
        {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(trf!("Modül: {}", "Module: {}", m.name)).strong());
                let label = if self.collapsed.contains(&id) {
                    tr!("İçini aç", "Open")
                } else {
                    tr!("Kapat", "Close")
                };
                if ui
                    .button(tr!("İçine gir", "Go inside"))
                    .on_hover_text(tr!(
                        "Yalnızca bu modülün içini göster (çift tık)",
                        "Show only the inside of this module (double click)"
                    ))
                    .clicked()
                {
                    self.enter_module(id);
                }
                if ui
                    .button(label)
                    .on_hover_text(tr!(
                        "Modülü burada, çevresiyle birlikte aç ya da kapat",
                        "Open or close the module here, with its surroundings"
                    ))
                    .clicked()
                {
                    self.toggle_module(id);
                }
                if ui
                    .button(tr!("Grubu çöz", "Ungroup"))
                    .on_hover_text(tr!(
                        "Parçalar ve teller kalır; yalnızca modül sınırı kaldırılır",
                        "Parts and wires stay; only the module border is removed"
                    ))
                    .clicked()
                {
                    self.edit(Edit::Module { id, module: None });
                    self.selected_module = None;
                    self.collapsed.remove(&id);
                }
            });
            let l = ui.label(tr!("Modül adı", "Module name")).id;
            if ui
                .text_edit_singleline(&mut m.name)
                .labelled_by(l)
                .changed()
            {
                self.edit(Edit::Module {
                    id,
                    module: Some(m),
                });
            }
            ui.label(
                RichText::new(tr!(
                    "Başlığa çift tıkla: içine gir. Başlığı sürükle: hepsini taşı.",
                    "Double click the title: go inside. Drag the title: move it all."
                ))
                .weak()
                .small(),
            );
        } else if self.selected.len() >= 2 {
            ui.horizontal(|ui| {
                let l = ui.label(tr!("Modül adı", "Module name")).id;
                ui.add(egui::TextEdit::singleline(&mut self.module_name).desired_width(110.0))
                    .labelled_by(l);
                if ui.button(tr!("Modül yap", "Make module")).clicked() {
                    let name = self.module_name.clone();
                    if let Err(e) = self.group_selected(&name) {
                        self.status = Some(e);
                    }
                }
            });
        }
        if self.selected.len() >= 2 {
            ui.horizontal(|ui| {
                if ui.button(tr!("Üstten hizala", "Align top")).clicked() {
                    self.align_selected(true);
                }
                if ui.button(tr!("Soldan hizala", "Align left")).clicked() {
                    self.align_selected(false);
                }
            });
        }
        ui.checkbox(&mut self.minimap, tr!("Küçük harita", "Minimap"));
        if let Some(to) = self.selected_wire {
            if self.world.wire_into(to).is_none() {
                self.selected_wire = None;
                return;
            }
            let mut style = self.world.wire_style(to).cloned().unwrap_or_default();
            ui.separator();
            let l = ui.label(tr!("Tel adı", "Wire name")).id;
            if ui
                .text_edit_singleline(&mut style.name)
                .labelled_by(l)
                .changed()
            {
                self.edit(Edit::Style {
                    to,
                    style: Some(style.clone()),
                });
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr!("Dirsek ekle", "Add bend")).clicked() {
                    self.routing = Some(to);
                }
                if ui.button(tr!("Otomatik yol", "Auto route")).clicked() {
                    style.points.clear();
                    self.edit(Edit::Style {
                        to,
                        style: Some(style),
                    });
                    self.routing = None;
                }
                if ui.small_button("✖").clicked() {
                    self.selected_wire = None;
                    self.routing = None;
                }
            });
        }
        if self.routing.is_some() {
            ui.colored_label(
                theme::COPPER_BRIGHT,
                tr!(
                    "Tahtaya tıkla: dirsek ekle. Enter / Esc: bitir.",
                    "Click the board: add a bend. Enter / Esc: finish."
                ),
            );
        }
    }

    /// Where the player is: the world, then each module looked into. A
    /// click goes back out to that level.
    pub fn breadcrumbs(&mut self, ui: &mut Ui) {
        if self.scope.is_empty() {
            return;
        }
        let mut go = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(tr!("⬅ Dışarı çık", "⬅ Go out"))
                .on_hover_text(tr!("Bir üst seviyeye dön (Esc)", "Back up one level (Esc)"))
                .clicked()
            {
                go = Some(self.scope.len() - 1);
            }
            ui.separator();
            if ui
                .link(RichText::new(&self.world.name).color(theme::INK_DIM))
                .clicked()
            {
                go = Some(0);
            }
            for (i, id) in self.scope.iter().enumerate() {
                ui.label(RichText::new("›").color(theme::INK_DIM));
                let name = self.world.module(*id).map_or("?", |m| m.name.as_str());
                if i + 1 == self.scope.len() {
                    ui.label(RichText::new(name).strong().color(theme::COPPER_BRIGHT));
                } else if ui.link(RichText::new(name).color(theme::INK_DIM)).clicked() {
                    go = Some(i + 1);
                }
            }
        });
        if let Some(depth) = go {
            self.leave_to(depth);
        }
    }

    /// The overview map's place: the board's bottom-left corner.
    pub(super) fn map_rect(&self, board: Rect) -> Option<Rect> {
        if !self.minimap || self.world.is_empty() {
            return None;
        }
        let size = vec2(180.0, 110.0);
        if board.width() < size.x * 2.0 || board.height() < size.y * 2.0 {
            return None;
        }
        Some(Rect::from_min_size(
            board.left_bottom() + vec2(12.0, -12.0 - size.y),
            size,
        ))
    }

    /// Drawn over the board; clicking or dragging on it moves the camera.
    pub(super) fn map_ui(&mut self, ui: &mut Ui, r: Rect) {
        let l = self.layout(self.origin);
        let Some(bounds) = l.bounds() else { return };
        let bounds = bounds.expand(30.0);
        let response = ui
            .interact(r, ui.scope_id().with("world-map"), Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Crosshair);
        let p = ui.painter();
        p.rect(
            r,
            6.0,
            theme::WELL.gamma_multiply(0.92),
            Stroke::new(1.0, theme::LINE),
            StrokeKind::Inside,
        );
        let inner = r.shrink(6.0);
        let scale = (inner.width() / bounds.width()).min(inner.height() / bounds.height());
        let at = inner.center() - bounds.center().to_vec2() * scale;
        let map = |q: egui::Pos2| at + q.to_vec2() * scale;
        let p = p.with_clip_rect(inner);
        for (id, _, _) in &l.parts {
            if !l.visible(*id) {
                continue;
            }
            if let Some(pr) = l.part_rect(*id) {
                p.rect_filled(
                    Rect::from_min_max(map(pr.min), map(pr.max)),
                    1.0,
                    theme::INK_DIM,
                );
            }
        }
        for m in &l.modules {
            p.rect_stroke(
                Rect::from_min_max(map(m.rect.min), map(m.rect.max)),
                1.0,
                Stroke::new(1.0, theme::COPPER),
                StrokeKind::Inside,
            );
        }
        if self.board.is_positive() {
            let view = Rect::from_min_max(
                map(l.to_world(self.board.min)),
                map(l.to_world(self.board.max)),
            )
            .intersect(inner);
            p.rect_stroke(view, 0.0, Stroke::new(1.0, theme::BUS), StrokeKind::Inside);
        }
        if let Some(point) = response
            .interact_pointer_pos()
            .filter(|_| response.clicked() || response.dragged())
        {
            let world = (point - at) / scale;
            self.offset = self.board.size() * 0.5 - world * self.zoom;
            self.fit = false;
        }
        p.text(
            r.right_bottom() - vec2(5.0, 4.0),
            Align2::RIGHT_BOTTOM,
            tr!("HARİTA", "MAP"),
            FontId::monospace(8.0),
            theme::INK_DIM,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Image, true, tr!("Harita", "Map"))
        });
    }

    /// Part and module counts with the main board gestures, for a side panel.
    pub fn summary(&self, ui: &mut Ui) {
        ui.label(
            RichText::new(trf!(
                "{} parça · {} modül",
                "{} parts · {} modules",
                self.world.len(),
                self.world.modules().count()
            ))
            .weak()
            .small(),
        );
        ui.label(
            RichText::new(tr!(
                "Shift+sürükle: seç · Modüle çift tık: içine gir · Esc: dışarı çık",
                "Shift+drag: select · Double click a module: go inside · Esc: go out"
            ))
            .weak()
            .small(),
        );
    }
}
