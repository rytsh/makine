//! The truth table of a small combinational level: every input row, what
//! the level expects and what the player's circuit gives, live.

use egui::{RichText, Ui};
use makine_lang::tr;
use makine_levels::{Level, Test};
use makine_sim::Circuit;

use super::App;
use crate::canvas::{format_value, ON};

/// Levels with at most this many input bits get a table.
const MAX_BITS: u32 = 4;

/// One row: inputs, expected outputs and the circuit's.
pub(super) struct Row {
    pub inputs: Vec<u64>,
    pub expected: Vec<Option<u64>>,
    pub actual: Option<Vec<u64>>,
}

/// The rows of `level`'s table, run through `sim` (a copy, so the live
/// circuit keeps its inputs). `None` for levels without a table.
pub(super) fn table(level: &Level, sim: Option<&Circuit>) -> Option<Vec<Row>> {
    if !matches!(level.test, Test::Function(_) | Test::Partial(_)) {
        return None;
    }
    let bits: u32 = level.input_pins().iter().map(|p| p.width as u32).sum();
    if bits > MAX_BITS {
        return None;
    }
    let mut sim = sim.cloned();
    Some(
        level
            .steps()
            .into_iter()
            .map(|s| {
                let actual = sim.as_mut().and_then(|c| {
                    c.reset();
                    c.eval_values(&s.inputs).ok()
                });
                Row {
                    inputs: s.inputs,
                    expected: s.expect,
                    actual,
                }
            })
            .collect(),
    )
}

impl App {
    /// The table under the level's description. Clicking a row loads its
    /// inputs into the circuit.
    pub(super) fn truth_table(&mut self, ui: &mut Ui) {
        let level = self.level;
        let sim = self.sim.as_ref().ok().filter(|_| !self.oscillating);
        let Some(rows) = table(level, sim) else {
            return;
        };
        let ins = level.input_pins();
        let outs = level.output_pins();
        let right = rows
            .iter()
            .filter(|r| {
                r.actual.as_ref().is_some_and(|a| {
                    r.expected
                        .iter()
                        .zip(a)
                        .all(|(e, a)| e.is_none_or(|e| e == *a))
                })
            })
            .count();
        let header = format!(
            "{}  {right}/{}",
            tr!("Doğruluk tablosu", "Truth table"),
            rows.len()
        );
        let mut load = None;
        egui::CollapsingHeader::new(RichText::new(header).strong())
            .id_salt(("truth", level.chip))
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(tr!(
                        "Satıra tıkla: o girişler devreye yüklenir. Sağdaki sütun senin devrenin \
                         sonucu.",
                        "Click a row to load its inputs into the circuit. The right column is \
                         your circuit's result."
                    ))
                    .weak()
                    .small(),
                );
                egui::Grid::new(("truth_grid", level.chip))
                    .striped(true)
                    .spacing(egui::vec2(10.0, 2.0))
                    .show(ui, |ui| {
                        for p in &ins {
                            ui.label(RichText::new(&p.name).small().strong());
                        }
                        ui.label("│");
                        for p in &outs {
                            ui.label(RichText::new(&p.name).small().strong());
                        }
                        ui.label(RichText::new(tr!("senin", "yours")).small().weak());
                        ui.end_row();
                        for (ri, r) in rows.iter().enumerate() {
                            let current = r.inputs == self.inputs;
                            for (p, v) in ins.iter().zip(&r.inputs) {
                                let t = RichText::new(format_value(p.width, *v)).monospace();
                                let t = if current { t.color(ON) } else { t };
                                if ui
                                    .add(egui::Label::new(t).sense(egui::Sense::click()))
                                    .clicked()
                                {
                                    load = Some(ri);
                                }
                            }
                            ui.label("│");
                            for (p, e) in outs.iter().zip(&r.expected) {
                                ui.monospace(e.map_or("-".into(), |e| format_value(p.width, e)));
                            }
                            let (text, ok) = match &r.actual {
                                Some(a) => {
                                    let ok = r
                                        .expected
                                        .iter()
                                        .zip(a)
                                        .all(|(e, a)| e.is_none_or(|e| e == *a));
                                    let vals: Vec<String> = outs
                                        .iter()
                                        .zip(a)
                                        .map(|(p, v)| format_value(p.width, *v))
                                        .collect();
                                    (
                                        format!(
                                            "{} {}",
                                            vals.join(" "),
                                            if ok { "✔" } else { "✖" }
                                        ),
                                        ok,
                                    )
                                }
                                None => ("?".into(), false),
                            };
                            ui.colored_label(
                                if ok {
                                    crate::theme::GOOD
                                } else {
                                    crate::theme::BAD
                                },
                                RichText::new(text).monospace(),
                            );
                            ui.end_row();
                        }
                    });
            });
        if let Some(ri) = load {
            self.inputs = rows[ri].inputs.clone();
            self.run();
        }
    }
}

impl App {
    /// The first time: a few steps over the canvas, telling what to do on
    /// the first level. Each step is done by doing it.
    pub(super) fn guide(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if self.progress.guide_done || self.level.chip != "Not" {
            return;
        }
        if self.progress.solved("Not") {
            self.progress.guide_done = true;
            return;
        }
        let d = self.design().def.clone();
        let step = if d.parts.is_empty() {
            0
        } else if d.wires.len() < 3 {
            1
        } else {
            2
        };
        let text = [
            tr!(
                "1/3  Soldaki Parçalar listesinden NAND'ı tutup tuvale sürükle.",
                "1/3  Drag the NAND from the Parts list on the left onto the board."
            ),
            tr!(
                "2/3  Tel çek: bir uçtan (yuvarlak nokta) ötekine sürükle. in'i NAND'ın iki \
                 girişine, NAND'ın çıkışını out'a bağla.",
                "2/3  Draw wires: drag from one pin (a round dot) to another. Wire in to both \
                 inputs of the NAND, and the NAND's output to out."
            ),
            tr!(
                "3/3  in'e tıklayıp değiştir ve out'u izle. Doğru görünüyorsa sağdaki \
                 ▶ Test et'e bas.",
                "3/3  Click in to change it and watch out. If it looks right, press \
                 ▶ Test on the right."
            ),
        ][step];
        egui::Area::new(egui::Id::unique("guide"))
            .order(egui::Order::Foreground)
            .fixed_pos(canvas.center_bottom() + egui::vec2(-230.0, -110.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .stroke(egui::Stroke::new(1.5, crate::theme::COPPER))
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_width(460.0);
                        ui.label(
                            RichText::new(tr!("İlk adımlar", "First steps"))
                                .strong()
                                .color(crate::theme::COPPER_BRIGHT),
                        );
                        ui.label(text);
                        if ui
                            .small_button(tr!("Rehberi kapat", "Close the guide"))
                            .clicked()
                        {
                            self.progress.guide_done = true;
                            self.save_soon();
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_levels::find;
    use makine_levels::reference::reference;
    use makine_sim::Library;

    #[test]
    fn the_table_shows_right_and_wrong_rows() {
        let level = find("Xor").unwrap();
        let mut lib = Library::new();
        for c in ["Not", "And", "Or"] {
            lib.insert(reference(c));
        }
        let good = Circuit::compile_def(&lib, &reference("Xor")).unwrap();
        let rows = table(level, Some(&good)).unwrap();
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| r.actual.as_deref()
            == Some(&r.expected.iter().map(|e| e.unwrap()).collect::<Vec<_>>()[..])));
        // An OR in place of the XOR is wrong on the last row only.
        let mut or = reference("Or");
        or.name = "Xor".into();
        let bad = Circuit::compile_def(&lib, &or).unwrap();
        let rows = table(level, Some(&bad)).unwrap();
        let wrong: Vec<_> = rows
            .iter()
            .filter(|r| r.actual.as_ref().unwrap()[0] != r.expected[0].unwrap())
            .map(|r| r.inputs.clone())
            .collect();
        assert_eq!(wrong, vec![vec![1, 1]]);
        assert!(table(find("Adder8").unwrap(), None).is_none());
        assert!(table(find("Latch").unwrap(), None).is_none());
    }
}
