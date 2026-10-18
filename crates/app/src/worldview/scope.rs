//! Live probes and a bounded, per-simulation-step logic analyser.
use super::WorldView;
use crate::theme;
use egui::{pos2, vec2, Align2, FontId, Rect, RichText, Stroke, Ui};
use makine_lang::{tr, trf};
use makine_world::End;

impl WorldView {
    pub fn scope_panel(&mut self, ui: &mut Ui) {
        if let Some((id, _)) = self.world.parts().find(|(_, p)| {
            matches!(
                p.kind,
                makine_world::Kind::Device(makine_world::Device::ProgramRom { .. })
            )
        }) {
            if let Some(op) = self.sim.value(End::new(id, 0)) {
                let next = self.sim.value(End::new(id, 1)).unwrap_or(0);
                ui.label(
                    RichText::new(trf!(
                        "4004 yorumu: {}",
                        "4004 reading: {}",
                        makine_world::projects::mcs4::disassemble(op as u8, next as u8)
                    ))
                    .monospace(),
                );
                ui.label(
                    RichText::new(tr!(
                        "Bir sonraki yükselen kenarda yürütülecek komut.",
                        "The instruction run on the next rising edge."
                    ))
                    .weak()
                    .small(),
                );
            }
        }
        if let Some((id, p)) = self.single() {
            if let Some(pins) = p.kind.pins(self.sim.library()) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(tr!("Çıkışları izle:", "Watch outputs:"))
                            .weak()
                            .small(),
                    );
                    for (pin, p) in pins.outputs.iter().enumerate() {
                        if ui.small_button(p.to_string()).clicked() {
                            self.sim.watch(End::new(id, pin));
                        }
                    }
                });
            }
        }
        if self.sim.traces.is_empty() {
            return;
        }
        crate::theme::section(ui, tr!("Sinyal geçmişi", "Signal history"));
        ui.checkbox(
            &mut self.scope_binary,
            tr!("Değerleri ikilik göster", "Show values in binary"),
        );
        ui.label(
            RichText::new(tr!(
                "Son 256 adım · ? = okunamıyor · telde sağ tık: izle",
                "Last 256 steps · ? = unreadable · right click a wire: watch"
            ))
            .weak()
            .small(),
        );
        let mut remove = None;
        let mut trigger = self.sim.break_on_change;
        for (end, trace) in &self.sim.traces {
            let part = self.world.part(end.part);
            let pin = part
                .and_then(|p| p.kind.pins(self.sim.library()))
                .and_then(|p| p.outputs.get(end.pin).cloned());
            let name = part
                .map(|p| p.kind.label())
                .unwrap_or(tr!("Silinmiş parça", "Deleted part").into());
            let width = pin.as_ref().map_or(1, |p| p.width);
            let name = format!(
                "{} / {}",
                name,
                pin.as_ref().map_or("?".into(), |p| p.to_string())
            );
            let v = self.sim.value(*end);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&name).small());
                ui.monospace(v.map_or("?".into(), |v| {
                    if self.scope_binary {
                        format!("{v:0w$b}", w = width as usize)
                    } else {
                        format!("0x{v:X}")
                    }
                }));
                if ui.small_button("✖").clicked() {
                    remove = Some(*end);
                }
            });
            let mut on = trigger == Some(*end);
            if ui
                .checkbox(&mut on, tr!("Değişince dur", "Stop on change"))
                .changed()
            {
                trigger = on.then_some(*end);
            }
            let (r, _) = ui.allocate_exact_size(
                vec2(ui.available_width().min(380.0), 35.0),
                egui::Sense::hover(),
            );
            let p = ui.painter();
            p.rect_filled(r, 3.0, theme::WELL);
            let n = trace.len().max(2);
            let dx = r.width() / (n - 1) as f32;
            if width == 1 {
                let y = |v: u64| {
                    if v == 0 {
                        r.bottom() - 6.0
                    } else {
                        r.top() + 6.0
                    }
                };
                for (i, (a, b)) in trace.iter().zip(trace.iter().skip(1)).enumerate() {
                    if let (Some(av), Some(bv)) = (a.1, b.1) {
                        let x0 = r.left() + i as f32 * dx;
                        let x1 = x0 + dx;
                        p.line_segment(
                            [pos2(x0, y(av)), pos2(x1, y(av))],
                            Stroke::new(1.5, theme::SIGNAL),
                        );
                        p.line_segment(
                            [pos2(x1, y(av)), pos2(x1, y(bv))],
                            Stroke::new(1.5, theme::SIGNAL),
                        );
                    }
                }
            } else {
                // Runs of a bus value, not a misleading analogue waveform.
                let samples: Vec<_> = trace.iter().collect();
                let mut start = 0;
                while start < samples.len() {
                    let val = samples[start].1;
                    let mut stop = start + 1;
                    while stop < samples.len() && samples[stop].1 == val {
                        stop += 1;
                    }
                    let x0 = r.left() + start as f32 * dx;
                    let x1 = (r.left() + stop as f32 * dx).min(r.right());
                    let run =
                        Rect::from_min_max(pos2(x0, r.top() + 6.0), pos2(x1, r.bottom() - 6.0));
                    p.rect_stroke(
                        run,
                        1.0,
                        Stroke::new(1.0, theme::BUS),
                        egui::StrokeKind::Inside,
                    );
                    if run.width() > 24.0 {
                        p.text(
                            run.center(),
                            Align2::CENTER_CENTER,
                            val.map_or("?".into(), |v| format!("{v:X}")),
                            FontId::monospace(10.0),
                            theme::BUS,
                        );
                    }
                    start = stop;
                }
            }
            if let Some((tick, _)) = trace.back() {
                ui.label(
                    RichText::new(trf!("adım {tick}", "step {tick}"))
                        .weak()
                        .small(),
                );
            }
        }
        if let Some(e) = remove {
            self.sim.traces.remove(&e);
            if trigger == Some(e) {
                trigger = None;
            }
        }
        self.sim.break_on_change = trigger;
        ui.horizontal(|ui| {
            if ui.button(tr!("Geçmişi temizle", "Clear history")).clicked() {
                for t in self.sim.traces.values_mut() {
                    t.clear();
                }
            }
            if ui.button(tr!("Başlangıca dön", "Back to start")).clicked() {
                self.sim.reset();
            }
        });
    }
}
