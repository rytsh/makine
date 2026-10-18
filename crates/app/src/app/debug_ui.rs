//! Looking inside the computer while it runs: registers, the instructions
//! around pc (click one to stop there), memory, and a small assembly
//! editor to write a program and run it at once.

use egui::{RichText, Ui};
use makine_lang::{tr, trf};
use makine_rv::isa::REG_NAMES;

use super::App;
use crate::computer::Computer;
use crate::theme;

/// What the debugger shows; not saved.
#[derive(Default)]
pub(super) struct Debug {
    /// Memory viewer address, as typed.
    pub mem_addr: String,
    /// The assembly being edited, and its last error.
    pub source: Option<String>,
    pub asm_error: Option<(usize, String)>,
    /// Registers before the last step, to light up what changed.
    pub before: Option<[u32; 32]>,
}

impl App {
    /// The tools under the screen: registers, code, memory, editor.
    pub(super) fn debugger(&mut self, ui: &mut Ui) {
        let source_default = self
            .progress
            .program
            .clone()
            .unwrap_or_else(|| crate::computer::EXAMPLES[0].1.to_string());
        let Some(c) = self.computer.as_mut() else {
            return;
        };
        ui.add_space(8.0);
        ui.columns(2, |cols| {
            registers(&mut cols[0], c, &self.debug);
            code(&mut cols[1], c);
        });
        memory(ui, c, &mut self.debug.mem_addr);
        // The editor: write, then assemble and run.
        let mut run = None;
        egui::CollapsingHeader::new(
            RichText::new(tr!("✎ Assembly yaz", "✎ Write assembly")).strong(),
        )
        .id_salt("asm_editor")
        .show(ui, |ui| {
            let src = self.debug.source.get_or_insert(source_default);
            ui.label(
                RichText::new(tr!(
                    "RISC-V assembly yaz; ▶ Derle ve çalıştır programı senin işlemcine yükler. \
                     Program ebreak ile durur. Bellek haritası sağ panelde.",
                    "Write RISC-V assembly; ▶ Assemble and run loads it into your processor. \
                     The program stops at ebreak. The memory map is in the right panel."
                ))
                .small()
                .weak(),
            );
            let error_line = self.debug.asm_error.as_ref().map(|(l, _)| *l);
            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                let mut job = egui::text::LayoutJob::default();
                for (i, line) in text.as_str().split_inclusive('\n').enumerate() {
                    let color = if Some(i + 1) == error_line {
                        theme::BAD
                    } else if line.trim_start().starts_with('#') {
                        theme::INK_DIM
                    } else {
                        theme::INK
                    };
                    job.append(
                        line,
                        0.0,
                        egui::TextFormat::simple(egui::FontId::monospace(13.0), color),
                    );
                }
                job.wrap.max_width = wrap;
                ui.fonts_mut(|f| f.layout_job(job))
            };
            ui.add(
                egui::TextEdit::multiline(src)
                    .code_editor()
                    .desired_rows(14)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut layouter),
            );
            if let Some((line, e)) = &self.debug.asm_error {
                ui.colored_label(theme::BAD, trf!("Satır {line}: {e}", "Line {line}: {e}"));
            }
            ui.horizontal(|ui| {
                if ui
                    .button(tr!("▶ Derle ve çalıştır", "▶ Assemble and run"))
                    .clicked()
                {
                    run = Some(src.clone());
                }
                if ui.button(tr!("Örneği yükle", "Load the example")).clicked() {
                    *src = crate::computer::EXAMPLES[0].1.to_string();
                }
            });
        });
        if let Some(src) = run {
            match makine_rv::asm::assemble(&src) {
                Ok(_) => {
                    self.debug.asm_error = None;
                    self.load_program("program", src.as_bytes());
                }
                Err(e) => self.debug.asm_error = Some((e.line, e.message)),
            }
        }
    }
}

/// The 32 registers in two columns; those changed by the last step lit.
fn registers(ui: &mut Ui, c: &Computer, debug: &Debug) {
    ui.label(RichText::new(tr!("Register'lar", "Registers")).strong());
    let Some(x) = c.registers() else {
        ui.label(
            RichText::new(tr!(
                "Senin işlemcinin register'ları devrenin içinde. Referans işlemciyle yan yana \
                 çalışırken gösterilir; ikisi ayrıldığı için artık gösterilemiyor.",
                "Your processor's registers are inside the circuit. They are shown while it \
                 runs side by side with the reference; the two went apart, so they cannot be \
                 shown any more."
            ))
            .small()
            .weak(),
        );
        if let Some(l) = &c.shadow_lost {
            ui.colored_label(theme::WARN, l);
        }
        return;
    };
    egui::Grid::new("regs")
        .num_columns(4)
        .spacing(egui::vec2(8.0, 1.0))
        .show(ui, |ui| {
            for row in 0..16 {
                for r in [row, row + 16] {
                    let changed = debug.before.is_some_and(|b| b[r] != x[r]);
                    ui.label(
                        RichText::new(format!("{:>4}", REG_NAMES[r]))
                            .monospace()
                            .size(12.0)
                            .color(theme::INK_DIM),
                    )
                    .on_hover_text(format!("x{r}"));
                    let v = RichText::new(format!("{:08x}", x[r]))
                        .monospace()
                        .size(12.0);
                    let v = if changed {
                        v.color(theme::SIGNAL)
                    } else if x[r] == 0 {
                        v.color(theme::INK_DIM)
                    } else {
                        v
                    };
                    ui.label(v)
                        .on_hover_text(format!("{} = {}", x[r], x[r] as i32));
                }
                ui.end_row();
            }
        });
}

/// Instructions around pc. Click one to stop there.
fn code(ui: &mut Ui, c: &mut Computer) {
    ui.label(RichText::new(tr!("Komutlar", "Instructions")).strong());
    ui.label(
        RichText::new(tr!(
            "Bir satıra tıkla: program oraya gelince durur (●).",
            "Click a line: the program stops when it gets there (●)."
        ))
        .small()
        .weak(),
    );
    let pc = c.pc();
    let start = pc.saturating_sub(4 * 6) & !3;
    let mut toggle = None;
    for k in 0..16u32 {
        let a = start + 4 * k;
        let instr = c.machine.peek_ram(a);
        let here = a == pc;
        let bp = c.breakpoints.contains(&a);
        let text = format!(
            "{} {} {a:08x}  {}",
            if bp { "●" } else { " " },
            if here { "▶" } else { " " },
            makine_rv::disasm::disasm(instr, a)
        );
        let t = RichText::new(text).monospace().size(12.0);
        let t = if here {
            t.color(theme::SIGNAL).strong()
        } else if bp {
            t.color(theme::BAD)
        } else {
            t
        };
        if ui
            .add(egui::Label::new(t).sense(egui::Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            toggle = Some(a);
        }
    }
    if let Some(a) = toggle {
        if !c.breakpoints.remove(&a) {
            c.breakpoints.insert(a);
        }
    }
    if !c.breakpoints.is_empty()
        && ui
            .small_button(tr!("Durma noktalarını temizle", "Clear stops"))
            .clicked()
    {
        c.breakpoints.clear();
    }
}

/// Eight rows of memory from an address the player types.
fn memory(ui: &mut Ui, c: &Computer, addr: &mut String) {
    egui::CollapsingHeader::new(RichText::new(tr!("Bellek", "Memory")).strong())
        .id_salt("memory_view")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let l = ui.label(tr!("Adres", "Address")).id;
                ui.add(
                    egui::TextEdit::singleline(addr)
                        .hint_text("0x0")
                        .desired_width(100.0)
                        .font(egui::TextStyle::Monospace),
                )
                .labelled_by(l);
                for (name, a) in [("sp", 2usize), ("gp", 3)] {
                    if let Some(x) = c.registers() {
                        if ui.small_button(name).clicked() {
                            *addr = format!("0x{:x}", x[a]);
                        }
                    }
                }
            });
            let t = addr.trim().trim_start_matches("0x");
            let base =
                u32::from_str_radix(if t.is_empty() { "0" } else { t }, 16).unwrap_or(0) & !15;
            for row in 0..8u32 {
                let a = base.wrapping_add(row * 16);
                let words: Vec<u32> = (0..4).map(|w| c.machine.peek_ram(a + w * 4)).collect();
                let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                let ascii: String = bytes
                    .iter()
                    .map(|&b| {
                        if (32..127).contains(&b) {
                            b as char
                        } else {
                            '·'
                        }
                    })
                    .collect();
                ui.label(
                    RichText::new(format!(
                        "{a:08x}  {:08x} {:08x} {:08x} {:08x}  {ascii}",
                        words[0], words[1], words[2], words[3]
                    ))
                    .monospace()
                    .size(12.0),
                );
            }
        });
}
