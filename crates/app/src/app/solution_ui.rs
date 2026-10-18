//! A level's example solution and how the player's solution compares:
//! NAND gates in each, the known minimum, and where extra gates come from.

use egui::{vec2, Pos2, RichText, Ui};
use makine_lang::{tr, trf};
use makine_levels::cost::{self, Cost};
use makine_levels::Level;
use makine_sim::{ChipDef, Circuit, Library, Sink, Source};

use super::App;
use crate::canvas::Canvas;
use crate::design::{geometry, Design};

/// An independent, temporary workspace. Switching tabs or docking never
/// replaces the player's design, simulation, camera or undo history.
pub(super) struct Solution {
    pub chip: &'static str,
    pub design: Design,
    pub sim: Result<Circuit, String>,
    pub inputs: Vec<u64>,
    pub canvas: Canvas,
    pub active: bool,
    pub detached: bool,
    pub return_drag: bool,
    window_position: Option<Pos2>,
    lib: Library,
    fit: bool,
    status: Option<String>,
}

/// What the player has seen of a level's solution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Reveal {
    #[default]
    Hidden,
    /// Asked once: solving it yourself teaches more.
    Asked,
}

impl Solution {
    pub fn new(level: &'static Level, lib: &Library, symbols: bool) -> Self {
        let def = cost::best(level.chip);
        let design = arrange(def, lib, symbols);
        let mut canvas = Canvas::new();
        canvas.symbols = symbols;
        let mut s = Self {
            chip: level.chip,
            sim: Circuit::compile_def(lib, &design.def).map_err(|e| e.to_string()),
            inputs: vec![0; design.def.inputs.len()],
            design,
            canvas,
            active: true,
            detached: false,
            return_drag: false,
            window_position: None,
            lib: lib.clone(),
            fit: true,
            status: None,
        };
        s.run();
        s
    }

    pub fn run(&mut self) {
        if let Ok(sim) = &mut self.sim {
            for (pin, &v) in self.inputs.iter().enumerate() {
                sim.set_input(pin, v);
            }
            let _ = sim.eval();
        }
    }

    fn level(&self) -> &'static Level {
        makine_levels::find(self.chip).expect("example level exists")
    }

    pub(super) fn viewport_id(&self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("example-solution", self.chip))
    }

    fn window_builder(&self, custom_frame: bool) -> egui::ViewportBuilder {
        let mut builder = egui::ViewportBuilder::default()
            .with_title(trf!(
                "Örnek çözüm: {} — makine",
                "Example solution: {} — makine",
                self.level().name()
            ))
            .with_inner_size([1000., 700.])
            .with_min_inner_size([540., 360.])
            .with_decorations(!custom_frame)
            .with_resizable(true);
        if let Some(icon) = super::chrome::window_icon() {
            builder = builder.with_icon(icon);
        }
        builder
    }
}

#[cfg(test)]
mod tests;

/// Lay a chip out left to right: each part one column after the parts it
/// reads from (feedback wires are ignored), stacked within its column.
fn arrange(def: ChipDef, lib: &Library, symbols: bool) -> Design {
    let n = def.parts.len();
    let mut column = vec![0usize; n];
    // Longest path from the inputs, at most n rounds (loops stop growing).
    for _ in 0..n.min(64) {
        let mut changed = false;
        for w in &def.wires {
            if let (Source::Part { part: a, .. }, Sink::Part { part: b, .. }) = (w.from, w.to) {
                if a != b && column[b] <= column[a] && column[a] < n {
                    column[b] = column[a] + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut d = Design::new(def);
    let mut y = vec![40.0f32; column.iter().max().map_or(1, |m| m + 1)];
    for (i, p) in d.def.parts.iter().enumerate() {
        let h = lib.part_pins(p).map_or(60.0, |pins| {
            geometry(&p.kind, &pins, &crate::canvas::part_label(p), symbols)
                .size
                .y
        });
        let c = column[i];
        d.pos[i] = [200.0 + c as f32 * 150.0, y[c]];
        y[c] += h + 36.0;
    }
    d
}

impl App {
    /// Work out how the player's design compares (counted without fast
    /// models, so every chip is its NAND gates).
    pub(super) fn update_cost(&mut self) {
        let super::Target::Level(level) = self.target else {
            self.cost = None;
            return;
        };
        let lib = self.progress.library(false);
        let def = self.design().def.clone();
        self.cost = Cost::of(level.chip, &lib, &def).ok();
        self.delay = makine_levels::depth::Delay::of(level.chip, &lib, &def).ok();
    }

    /// Hints, opened one at a time; how many is remembered.
    pub(super) fn hints_view(&mut self, ui: &mut Ui) {
        let level = self.level;
        let all = makine_levels::hints::hints(level.chip);
        if all.is_empty() {
            return;
        }
        let shown = self.progress.hints.get(level.chip).copied().unwrap_or(0);
        crate::theme::section(ui, tr!("İpuçları", "Hints"));
        let titles = [
            tr!("Fikir", "The idea"),
            tr!("Parçalar", "Parts"),
            tr!("Bağlantılar", "Wiring"),
        ];
        for (i, h) in all.iter().enumerate().take(shown) {
            egui::Frame::new()
                .fill(crate::theme::WELL)
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(format!("{}. {}", i + 1, titles.get(i).unwrap_or(&"")))
                            .small()
                            .color(crate::theme::COPPER),
                    );
                    ui.label(*h);
                });
            ui.add_space(2.0);
        }
        if shown < all.len() {
            let label = if shown == 0 {
                tr!("💡 İpucu al", "💡 Get a hint")
            } else {
                tr!("💡 Bir ipucu daha", "💡 One more hint")
            };
            if ui
                .button(label)
                .on_hover_text(trf!(
                    "{} ipucundan {} açık",
                    "{1} of {0} hints open",
                    all.len(),
                    shown
                ))
                .clicked()
            {
                self.progress
                    .hints
                    .insert(level.chip.to_string(), shown + 1);
                self.save_soon();
            }
        }
    }

    pub(super) fn cost_view(&mut self, ui: &mut Ui) {
        let level = self.level;
        let Some(c) = self.cost.clone() else {
            return;
        };
        crate::theme::section(ui, tr!("Verimlilik", "Efficiency"));
        ui.label(
            RichText::new(tr!(
                "Seviye açılınca ve her testte güncellenir.",
                "Updated when the level opens and on every test."
            ))
            .weak()
            .small(),
        );
        egui::Grid::new("cost").num_columns(2).show(ui, |ui| {
            ui.label(tr!("Senin devren", "Your circuit"));
            ui.monospace(format!("{} NAND", c.mine));
            ui.end_row();
            if let Some(e) = c.example {
                ui.label(tr!("Örnek çözüm", "Example solution"));
                ui.monospace(format!("{e} NAND"));
                ui.end_row();
            }
            if let Some(m) = c.minimum {
                ui.label(tr!("Bilinen en az", "Known minimum"));
                ui.monospace(format!("{m} NAND"));
                ui.end_row();
            }
        });
        if let Some(d) = self.delay.filter(|d| d.mine > 0) {
            egui::Grid::new("delay").num_columns(2).show(ui, |ui| {
                ui.label(tr!("En uzun yol", "Longest path"))
                    .on_hover_text(tr!(
                        "Bir sinyalin girişten çıkışa (ya da bir mandala) giderken geçtiği en çok \
                         kapı. Her kapı biraz gecikir; bu yol ne kadar kısaysa saat o kadar hızlı \
                         olabilir.",
                        "The most gates a signal passes through from an input to an output (or a \
                         latch). Each gate adds a little delay; the shorter this path, the faster \
                         the clock can be."
                    ));
                ui.monospace(trf!("{} kapı", "{} gates", d.mine));
                ui.end_row();
                if let Some(e) = d.example {
                    ui.label(tr!("Örnekte", "In the example"));
                    ui.monospace(trf!("{} kapı", "{} gates", e));
                    ui.end_row();
                }
            });
            if let Some(e) = d.example.filter(|e| d.mine < *e) {
                ui.colored_label(
                    crate::theme::GOOD,
                    trf!(
                        "⚡ Senin devren örnekten {} kapı daha sığ: daha hızlı!",
                        "⚡ Your circuit is {} gates shallower than the example: faster!",
                        e - d.mine
                    ),
                );
            }
        }
        let here = c.here.unwrap_or(c.mine);
        match c.target() {
            _ if c.mine == 0 => {}
            Some(t) if c.mine < t => {
                ui.colored_label(
                    crate::theme::GOOD,
                    trf!(
                        "Örnekten {} kapı daha küçük. Harika!",
                        "{} gates smaller than the example. Great!",
                        t - c.mine
                    ),
                );
            }
            Some(_) if c.optimal() => {
                let what = if c.minimum.is_some() {
                    tr!(
                        "✔ En az kapıyla çözülmüş: bundan küçüğü yok.",
                        "✔ Solved with the fewest gates: nothing smaller exists."
                    )
                } else {
                    tr!(
                        "✔ Örnek çözüm kadar küçük.",
                        "✔ As small as the example solution."
                    )
                };
                ui.colored_label(crate::theme::GOOD, what);
            }
            Some(t) => {
                ui.colored_label(
                    crate::theme::WARN,
                    trf!(
                        "Çalışıyor ama {} kapı fazla.",
                        "It works, but uses {} gates too many.",
                        c.mine - t
                    ),
                );
                let earlier = c.from_earlier();
                if earlier > 0 {
                    ui.label(
                        RichText::new(trf!(
                            "Bunun {earlier} kapısı içinde kullandığın önceki çiplerden geliyor: \
                             onların seviyelerine dönüp küçültürsen burası da küçülür.",
                            "{earlier} of them come from earlier chips used inside: make those \
                             levels smaller and this one shrinks too."
                        ))
                        .small(),
                    );
                }
                if let Some(e) = c.example.filter(|e| here > *e) {
                    ui.label(
                        RichText::new(trf!(
                            "Bu seviyenin kendi tasarımı {} kapı fazla. Bazı ara sonuçları \
                             paylaşabilir misin? Örneğin bir NAND'ın çıkışını iki yerde kullanmak.",
                            "This level's own design has {} gates too many. Can some results \
                             be shared? For example, using one NAND's output in two places.",
                            here - e
                        ))
                        .small(),
                    );
                }
            }
            None => {}
        }
        ui.label(
            RichText::new(tr!(
                "Kapı sayısı çipin boyutunu ve maliyetini belirler. Az kapı her zaman \
                 hızlı demek değildir: hızı en uzun yoldaki kapı sayısı belirler.",
                "The gate count sets a chip's size and cost. Fewer gates is not always \
                 faster: speed depends on the number of gates on the longest path."
            ))
            .weak()
            .small(),
        );
        if let Some(page) = crate::wiki::page("verimlilik") {
            if ui
                .link(tr!("📖 Kapı sayısını azaltmak", "📖 Using fewer gates"))
                .clicked()
            {
                self.wiki.open(page);
                self.view = super::View::Wiki;
            }
        }
        self.solution_buttons(ui, level);
    }

    fn solution_buttons(&mut self, ui: &mut Ui, level: &'static Level) {
        let solved = self.progress.solved(level.chip);
        let open = self.solution.as_ref().is_some_and(|s| s.chip == level.chip);
        ui.add_space(4.0);
        if open {
            if ui
                .button(tr!("Örnek çözüm sekmesine git", "Go to example tab"))
                .clicked()
            {
                self.activate_solution(ui.ctx());
            }
            return;
        }
        if solved || self.reveal == Reveal::Asked {
            if ui
                .button(tr!(
                    "💡 Örnek çözümü göster",
                    "💡 Show the example solution"
                ))
                .on_hover_text(tr!(
                    "Örnek ayrı bir sekmede açılır; kendi devrene tek tıkla dönebilirsin",
                    "The example opens in a separate tab; return to your own circuit with one click"
                ))
                .clicked()
            {
                self.open_solution(level);
            }
            if !solved {
                ui.label(
                    RichText::new(tr!(
                        "Önce kendin denersen çok daha iyi öğrenirsin.",
                        "You learn much more if you try it yourself first."
                    ))
                    .weak()
                    .small(),
                );
            }
        } else if ui
            .button(tr!("💡 Çözümü gör", "💡 See the solution"))
            .on_hover_text(tr!(
                "Testi geçince örnek çözüm her zaman açık",
                "Once you pass the test, the example is always available"
            ))
            .clicked()
        {
            self.reveal = Reveal::Asked;
        }
    }

    pub(super) fn open_solution(&mut self, level: &'static Level) {
        self.workspace.scroll_to_active = true;
        if self.solution.as_ref().is_none_or(|s| s.chip != level.chip) {
            self.solution = Some(Solution::new(level, &self.lib, self.progress.symbols));
        }
        if let Some(s) = &mut self.solution {
            s.active = !s.detached;
        }
        self.editor_code = false;
        self.reveal = Reveal::Hidden;
    }

    pub(super) fn showing_solution(&self) -> bool {
        self.view == super::View::Editor
            && !self.editor_code
            && self
                .solution
                .as_ref()
                .is_some_and(|s| s.active && !s.detached)
    }

    pub(super) fn activate_solution(&mut self, ctx: &egui::Context) {
        self.workspace.scroll_to_active = true;
        if let Some(s) = &mut self.solution {
            if s.detached {
                ctx.send_viewport_cmd_to(s.viewport_id(), egui::ViewportCommand::Focus);
            } else {
                s.active = true;
                self.editor_code = false;
                self.view = super::View::Editor;
            }
        }
    }

    pub(super) fn detach_solution(&mut self, position: Option<Pos2>) {
        if let Some(s) = &mut self.solution {
            s.detached = true;
            s.active = false;
            s.window_position = position;
            s.fit = true;
        }
    }

    pub(super) fn dock_solution(&mut self) {
        if let Some(s) = &mut self.solution {
            s.detached = false;
            s.return_drag = false;
            // Closing a reference window must not interrupt the current work.
            s.active = false;
            s.fit = true;
        }
    }

    /// The example shares the browser document strip with the user's chips.
    pub(super) fn solution_tabs(&mut self, ui: &mut Ui, reveal_active: bool) {
        let Some(solution) = &self.solution else {
            return;
        };
        let example_name = solution.level().name();
        let detached = solution.detached;
        let example_active = self.showing_solution();
        let label = if detached {
            trf!(
                "Örnek: {example_name} (ayrı pencere)",
                "Example: {example_name} (separate window)"
            )
        } else {
            trf!("Örnek: {example_name}", "Example: {example_name}")
        };
        let (response, close) = super::workspace_ui::document_tab(
            ui,
            "example-document",
            &label,
            example_active,
            Some(Some(tr!("Örnek sekmesini kapat", "Close example tab"))),
        );
        if reveal_active && example_active {
            ui.scroll_to_rect(
                response.rect.expand2(egui::vec2(32., 0.)),
                Some(egui::Align::Center),
            );
        }
        if response.clicked() {
            self.activate_solution(ui.ctx());
        }
        if !detached
            && response.drag_stopped()
            && response
                .interact_pointer_pos()
                .is_some_and(|p| p.y > response.rect.bottom() + 24.)
        {
            self.detach_solution(super::workspace_ui::drag_window_position(
                ui.ctx(),
                response.interact_pointer_pos(),
            ));
            ui.ctx().request_repaint();
        }
        if close || response.clicked_by(egui::PointerButton::Middle) {
            self.solution = None;
        }
    }

    /// Immediate viewports share UI-thread state safely but have independent
    /// native frames, input and DPI, so the OS can move them between monitors.
    pub(super) fn solution_window(&mut self, ctx: &egui::Context) {
        let custom_frame = self.progress.custom_frame;
        let Some(solution) = self.solution.as_mut().filter(|s| s.detached) else {
            return;
        };
        let mut builder = solution.window_builder(custom_frame);
        if let Some(position) = solution.window_position.take() {
            builder = builder.with_position(position);
        }
        let mut dock = false;
        ctx.show_viewport_immediate(solution.viewport_id(), builder, |ui, class| {
            if class != egui::ViewportClass::EmbeddedWindow {
                dock |= super::chrome::child_window_frame(
                    ui,
                    &trf!("Örnek: {}", "Example: {}", solution.level().name()),
                    custom_frame,
                );
            }
            if class != egui::ViewportClass::EmbeddedWindow
                && ui.ctx().input(|i| i.viewport().close_requested())
            {
                dock = true;
            }
            egui::Panel::top("example_window_toolbar").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(trf!("Örnek: {}", "Example: {}", solution.level().name()));
                    dock |= ui.button(tr!("Sekmeye geri al", "Dock as tab")).clicked();
                    solution.return_drag |= super::workspace_ui::dock_drag_handle(ui, 0);
                });
                ui.label(tr!(
                    "Geçici örnek çalışma alanı; kendi devren ve ilerlemen değişmez.",
                    "Temporary example workspace; your own circuit and progress stay unchanged."
                ));
            });
            if ui.available_width() < 760. {
                egui::Panel::bottom("example_window_details_compact")
                    .default_size(120.)
                    .resizable(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| solution.controls(ui));
                    });
            } else {
                egui::Panel::right("example_window_details")
                    .default_size(240.)
                    .resizable(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| solution.controls(ui));
                    });
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| solution.show_canvas(ui));
        });
        if dock {
            self.dock_solution();
            ctx.request_repaint();
        }
    }

    /// The example solution's side panel: what it shows and its numbers.
    pub(super) fn solution_panel(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.solution else { return };
        s.controls(ui);
        if ui
            .button(tr!("Kendi devreme dön", "Back to my circuit"))
            .clicked()
        {
            s.active = false;
        }
    }

    pub(super) fn solution_canvas(&mut self, ui: &mut Ui) {
        if let Some(s) = &mut self.solution {
            s.show_canvas(ui);
        }
    }
}

impl Solution {
    fn controls(&mut self, ui: &mut Ui) {
        let level = self.level();
        ui.heading(trf!(
            "Örnek çözüm: {}",
            "Example solution: {}",
            level.name()
        ));
        ui.label(
            RichText::new(tr!(
                "Bu, oyunun örnek çözümü. Değiştirebilir ve girişlerini deneyebilirsin; \
                 değişiklikler kaydedilmez. Kendi devren olduğu gibi duruyor.",
                "This is the game's example solution. You can change it and try its inputs; \
                 changes are not saved. Your own circuit stays as it is."
            ))
            .weak(),
        );
        if let Some(n) = cost::example_count(level.chip) {
            ui.label(trf!("{n} NAND kapısı", "{n} NAND gates"));
        }
        if let Some(m) = cost::known_minimum(level.chip) {
            ui.label(
                RichText::new(trf!(
                    "Bu işi {m} NAND'dan azıyla yapmanın yolu yok.",
                    "There is no way to do this with fewer than {m} NANDs."
                ))
                .small(),
            );
        }
        if let Some(note) = cost::note(level.chip) {
            ui.add_space(4.0);
            ui.label(note);
        }
        if ui
            .button(tr!("Örnek devreyi sığdır", "Fit example circuit"))
            .clicked()
        {
            self.fit = true;
        }
        if let Err(error) = &self.sim {
            ui.colored_label(crate::theme::BAD, error);
        }
        if let Some(message) = &self.status {
            ui.label(message);
        }
        let mut changed = false;
        for (i, p) in self
            .design
            .def
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, p)| p.width > 1)
        {
            ui.horizontal(|ui| {
                let label = ui.label(&p.name).id;
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.inputs[i])
                            .hexadecimal(1, false, true)
                            .prefix("0x")
                            .range(0..=makine_sim::mask(p.width)),
                    )
                    .labelled_by(label)
                    .changed();
            });
        }
        if changed {
            self.run();
        }
    }

    /// The example solution on the canvas.
    fn show_canvas(&mut self, ui: &mut Ui) {
        let mut bounds = egui::Rect::NOTHING;
        for (id, part) in self.design.def.parts.iter().enumerate() {
            let at = self.design.part_pos(id);
            let size = self.lib.part_pins(part).map_or(vec2(120., 40.), |pins| {
                geometry(
                    &part.kind,
                    &pins,
                    &crate::canvas::part_label(part),
                    self.canvas.symbols,
                )
                .size
            });
            bounds = bounds.union(egui::Rect::from_min_size(at, size));
        }
        let output_x = (bounds.max.x + 100.).max(400.);
        for input in 0..self.design.def.inputs.len() {
            bounds = bounds.union(
                self.design
                    .port_rect(crate::design::Port::Input(input), output_x),
            );
        }
        for output in 0..self.design.def.outputs.len() {
            bounds = bounds.union(
                self.design
                    .port_rect(crate::design::Port::Output(output), output_x),
            );
        }
        if std::mem::take(&mut self.fit) && bounds.is_finite() {
            let viewport = ui.max_rect().shrink(24.);
            self.canvas.zoom = (viewport.width() / bounds.width().max(1.))
                .min(viewport.height() / bounds.height().max(1.))
                .clamp(0.03, 1.);
            self.canvas.offset =
                ui.max_rect().size() / 2. - bounds.center().to_vec2() * self.canvas.zoom;
        }
        let sim = self.sim.as_ref().ok().cloned();
        let out = self.canvas.show(
            ui,
            &mut self.design,
            &self.lib,
            sim.as_ref(),
            &self.inputs,
            output_x,
        );
        if let Some((pin, v)) = out.toggled_input {
            self.inputs[pin] = v;
            self.run();
        }
        if out.changed {
            self.sim = Circuit::compile_def(&self.lib, &self.design.def).map_err(|e| e.to_string());
            self.run();
        }
        if let Some(message) = out.status {
            self.status = Some(message);
        }
    }
}
