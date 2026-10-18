//! A level's example solution and how the player's solution compares:
//! NAND gates in each, the known minimum, and where extra gates come from.

use egui::{vec2, RichText, Ui};
use makine_levels::cost::{self, Cost};
use makine_levels::Level;
use makine_sim::{ChipDef, Circuit, Library, Sink, Source};

use super::App;
use crate::canvas::Canvas;
use crate::design::{geometry, Design};

/// The example solution open on the canvas instead of the player's own.
pub(super) struct Solution {
    pub chip: &'static str,
    pub design: Design,
    pub sim: Result<Circuit, String>,
    pub inputs: Vec<u64>,
    pub canvas: Canvas,
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
}

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
    }

    pub(super) fn cost_view(&mut self, ui: &mut Ui) {
        let level = self.level;
        let Some(c) = self.cost.clone() else {
            return;
        };
        crate::theme::section(ui, "Verimlilik");
        ui.label(
            RichText::new("Seviye açılınca ve her testte güncellenir.")
                .weak()
                .small(),
        );
        egui::Grid::new("cost").num_columns(2).show(ui, |ui| {
            ui.label("Senin devren");
            ui.monospace(format!("{} NAND", c.mine));
            ui.end_row();
            if let Some(e) = c.example {
                ui.label("Örnek çözüm");
                ui.monospace(format!("{e} NAND"));
                ui.end_row();
            }
            if let Some(m) = c.minimum {
                ui.label("Bilinen en az");
                ui.monospace(format!("{m} NAND"));
                ui.end_row();
            }
        });
        let here = c.here.unwrap_or(c.mine);
        match c.target() {
            _ if c.mine == 0 => {}
            Some(t) if c.mine < t => {
                ui.colored_label(
                    crate::theme::GOOD,
                    format!("★ Örnekten {} kapı daha küçük. Harika!", t - c.mine),
                );
            }
            Some(_) if c.optimal() => {
                let what = if c.minimum.is_some() {
                    "✔ En az kapıyla çözülmüş: bundan küçüğü yok."
                } else {
                    "✔ Örnek çözüm kadar küçük."
                };
                ui.colored_label(crate::theme::GOOD, what);
            }
            Some(t) => {
                ui.colored_label(
                    crate::theme::WARN,
                    format!("Çalışıyor ama {} kapı fazla.", c.mine - t),
                );
                let earlier = c.from_earlier();
                if earlier > 0 {
                    ui.label(
                        RichText::new(format!(
                            "Bunun {earlier} kapısı içinde kullandığın önceki çiplerden geliyor: \
                             onların seviyelerine dönüp küçültürsen burası da küçülür."
                        ))
                        .small(),
                    );
                }
                if let Some(e) = c.example.filter(|e| here > *e) {
                    ui.label(
                        RichText::new(format!(
                            "Bu seviyenin kendi tasarımı {} kapı fazla. Bazı ara sonuçları \
                             paylaşabilir misin? Örneğin bir NAND'ın çıkışını iki yerde kullanmak.",
                            here - e
                        ))
                        .small(),
                    );
                }
            }
            None => {}
        }
        ui.label(
            RichText::new(
                "Kapı sayısı işlemcinin boyutunu ve maliyetini belirler. Az kapı her zaman \
                 hızlı demek değildir: önemli olan en uzun yoldaki kapı sayısıdır.",
            )
            .weak()
            .small(),
        );
        if let Some(page) = crate::wiki::page("verimlilik") {
            if ui.link("📖 Kapı sayısını azaltmak").clicked() {
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
            if ui.button("Kendi devreme dön").clicked() {
                self.solution = None;
            }
            return;
        }
        if solved || self.reveal == Reveal::Asked {
            if ui
                .button("💡 Örnek çözümü göster")
                .on_hover_text("Örnek devre kendi devrenin yerine açılır; seninki silinmez")
                .clicked()
            {
                self.solution = Some(Solution::new(level, &self.lib, self.progress.symbols));
                self.reveal = Reveal::Hidden;
            }
            if !solved {
                ui.label(
                    RichText::new("Önce kendin denersen çok daha iyi öğrenirsin.")
                        .weak()
                        .small(),
                );
            }
        } else if ui
            .button("💡 Çözümü gör")
            .on_hover_text("Testi geçince örnek çözüm her zaman açık")
            .clicked()
        {
            self.reveal = Reveal::Asked;
        }
    }

    /// The example solution's side panel: what it shows and its numbers.
    pub(super) fn solution_panel(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.solution else { return };
        let level = self.level;
        ui.heading(format!("Örnek çözüm: {}", level.title));
        ui.label(
            RichText::new(
                "Bu, oyunun örnek çözümü. Değiştirebilir ve girişlerini deneyebilirsin; \
                 değişiklikler kaydedilmez. Kendi devren olduğu gibi duruyor.",
            )
            .weak(),
        );
        if let Some(n) = cost::example_count(level.chip) {
            ui.label(format!("{n} NAND kapısı"));
        }
        if let Some(m) = cost::known_minimum(level.chip) {
            ui.label(
                RichText::new(format!("Bu işi {m} NAND'dan azıyla yapmanın yolu yok.")).small(),
            );
        }
        if let Some(note) = cost::note(level.chip) {
            ui.add_space(4.0);
            ui.label(note);
        }
        let mut changed = false;
        for (i, p) in s
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
                        egui::DragValue::new(&mut s.inputs[i])
                            .hexadecimal(1, false, true)
                            .prefix("0x")
                            .range(0..=makine_sim::mask(p.width)),
                    )
                    .labelled_by(label)
                    .changed();
            });
        }
        if changed {
            s.run();
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Kendi devreme dön").clicked() {
                self.solution = None;
            }
        });
    }

    /// The example solution on the canvas.
    pub(super) fn solution_canvas(&mut self, ui: &mut Ui) {
        let lib = self.lib.clone();
        let Some(s) = &mut self.solution else { return };
        let output_x = (ui.available_width() / s.canvas.zoom.max(0.3) - 110.0).max(400.0);
        let banner = egui::Rect::from_min_size(ui.max_rect().min, vec2(ui.available_width(), 28.0));
        let sim = s.sim.as_ref().ok().cloned();
        let out = s
            .canvas
            .show(ui, &mut s.design, &lib, sim.as_ref(), &s.inputs, output_x);
        if let Some((pin, v)) = out.toggled_input {
            s.inputs[pin] = v;
            s.run();
        }
        if out.changed {
            s.sim = Circuit::compile_def(&lib, &s.design.def).map_err(|e| e.to_string());
            s.run();
        }
        ui.painter().rect_filled(banner, 0.0, crate::theme::PANEL);
        ui.painter().text(
            banner.left_center() + vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "💡 ÖRNEK ÇÖZÜM · kaydedilmez · sağ panelden kendi devrene dön",
            egui::FontId::proportional(13.0),
            crate::theme::COPPER_BRIGHT,
        );
    }
}
