//! Once the mini processor passes, the player can watch it run programs:
//! the program with pc marked, acc, and the 16 bytes of RAM, one tick at a
//! time or running.

use egui::{RichText, Ui};
use makine_lang::{tr, trf};
use makine_levels::mini::{self, MiniDriver, Program};
use makine_sim::Circuit;

use super::App;
use crate::theme;

pub(super) struct MiniRun {
    cpu: MiniDriver,
    program: usize,
    rom: [u8; 16],
    ram: [u8; 16],
    ticks: u64,
    running: bool,
    error: Option<String>,
    /// RAM address written last, lit for a moment.
    written: Option<u8>,
}

impl MiniRun {
    fn new(circuit: Circuit, program: usize) -> Self {
        let p = &mini::programs()[program];
        let mut r = Self {
            cpu: MiniDriver::new(circuit),
            program,
            rom: p.rom,
            ram: p.ram,
            ticks: 0,
            running: false,
            error: None,
            written: None,
        };
        r.error = r.cpu.reset().err();
        r
    }

    fn tick(&mut self) {
        if self.error.is_some() {
            return;
        }
        match self.cpu.step(&self.rom, &mut self.ram) {
            Ok(c) => {
                self.ticks += 1;
                self.written = c.write.map(|(a, _)| a);
                if c.instr >> 4 == mini::HLT {
                    self.running = false;
                }
            }
            Err(e) => {
                self.error = Some(e);
                self.running = false;
            }
        }
    }
}

impl App {
    /// Under a solved mini processor: run it.
    pub(super) fn mini_view(&mut self, ui: &mut Ui) {
        if self.level.chip != "MiniCpu" || !self.progress.solved("MiniCpu") {
            return;
        }
        theme::section(ui, tr!("İşlemcin çalışıyor", "Your processor at work"));
        let programs = mini::programs();
        let mut pick = None;
        ui.horizontal_wrapped(|ui| {
            for (i, p) in programs.iter().enumerate() {
                let on = self.mini.as_ref().is_some_and(|m| m.program == i);
                if ui.selectable_label(on, name(p)).clicked() {
                    pick = Some(i);
                }
            }
        });
        if let Some(i) = pick {
            match Circuit::compile(&self.lib, "MiniCpu") {
                Ok(c) => self.mini = Some(MiniRun::new(c, i)),
                Err(e) => self.status = Some(e.to_string()),
            }
        }
        let Some(m) = &mut self.mini else {
            ui.label(
                RichText::new(tr!(
                    "Bir program seç: senin devren onu kapı kapı çalıştırır.",
                    "Pick a program: your circuit runs it gate by gate."
                ))
                .small()
                .weak(),
            );
            return;
        };
        ui.horizontal(|ui| {
            let label = if m.running {
                tr!("⏸ Durdur", "⏸ Pause")
            } else {
                tr!("▶ Çalıştır", "▶ Run")
            };
            if ui.button(label).clicked() {
                m.running = !m.running;
            }
            if ui
                .add_enabled(!m.running, egui::Button::new(tr!("Adım", "Step")))
                .clicked()
            {
                m.tick();
            }
            if ui.button(tr!("⟲ Baştan", "⟲ Restart")).clicked() {
                let c = m.cpu.circuit.clone();
                *m = MiniRun::new(c, m.program);
            }
        });
        if m.running {
            // A few ticks a second, so each one can be seen.
            let now = ui.input(|i| i.time);
            let every = 0.25;
            let id = egui::Id::unique("mini_last_tick");
            let last: f64 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(0.0);
            if now - last >= every {
                m.tick();
                ui.ctx().data_mut(|d| d.insert_temp(id, now));
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(every));
        }
        if let Some(e) = &m.error {
            ui.colored_label(theme::BAD, e);
        }
        let (pc, acc) = (m.cpu.pc(), m.cpu.acc());
        ui.label(
            RichText::new(trf!(
                "pc = {pc}   acc = {acc}   ({} vuruş)",
                "pc = {pc}   acc = {acc}   ({} ticks)",
                m.ticks
            ))
            .monospace(),
        );
        ui.columns(2, |cols| {
            cols[0].label(RichText::new(tr!("Program", "Program")).strong());
            for (a, &i) in m.rom.iter().enumerate() {
                if i == 0 && a as u8 > pc && m.rom[a..].iter().all(|&x| x == 0) {
                    break;
                }
                let here = a as u8 == pc;
                let t = RichText::new(format!(
                    "{} {a:>2}  {}",
                    if here { "▶" } else { " " },
                    mini::disasm(i)
                ))
                .monospace()
                .size(12.0);
                cols[0].label(if here { t.color(theme::SIGNAL) } else { t });
            }
            cols[1].label(RichText::new("RAM").strong());
            for row in 0..4 {
                let cells: Vec<String> = (0..4)
                    .map(|k| format!("{:>3}", m.ram[row * 4 + k]))
                    .collect();
                let lit = m.written.is_some_and(|w| w as usize / 4 == row);
                let t = RichText::new(format!("{:>2}: {}", row * 4, cells.join(" ")))
                    .monospace()
                    .size(12.0);
                cols[1].label(if lit { t.color(theme::SIGNAL) } else { t });
            }
        });
        ui.label(RichText::new(about(&programs[m.program])).small().weak());
    }
}

fn name(p: &Program) -> &'static str {
    match p.name {
        "geri sayım" => tr!("Geri sayım", "Countdown"),
        "çarpma" => tr!("Çarpma", "Multiply"),
        _ => "Fibonacci",
    }
}

fn about(p: &Program) -> &'static str {
    match p.name {
        "geri sayım" => tr!(
            "5'ten geri sayar, her sayıyı RAM[14]'e yazar; 0'da durur.",
            "Counts down from 5, writing each number to RAM[14]; stops at 0."
        ),
        "çarpma" => tr!(
            "RAM[0] × RAM[1]'i toplayarak hesaplar, sonucu RAM[2]'ye yazar (13 × 7 = 91).",
            "Works out RAM[0] × RAM[1] by adding, writing the result to RAM[2] (13 × 7 = 91)."
        ),
        _ => tr!(
            "RAM[0] ve RAM[1]'de Fibonacci sayıları: her tur bir sonrakini hesaplar (256'da başa sarar).",
            "Fibonacci numbers in RAM[0] and RAM[1]: each round works out the next (wraps at 256)."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Progress;
    use makine_levels::reference::reference;

    /// The solved mini processor multiplies, on the player's own chips.
    #[test]
    fn the_solved_mini_cpu_runs_a_program() {
        let mut p = Progress::default();
        for l in makine_levels::levels() {
            p.solutions.insert(l.chip.into(), reference(l.chip));
        }
        let app = App::new(p, None);
        let c = Circuit::compile(&app.lib, "MiniCpu").unwrap();
        let mut m = MiniRun::new(c, 1);
        for _ in 0..200 {
            m.tick();
        }
        assert!(m.error.is_none(), "{:?}", m.error);
        assert_eq!(m.ram[2], 91);
        assert_eq!(m.cpu.acc(), 91);
    }
}
