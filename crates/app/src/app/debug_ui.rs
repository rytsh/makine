//! Looking inside the computer while it runs: registers, the instructions
//! around pc (click one to stop there) and memory. Editing lives in the
//! source-first workbench so every draft is saved in the notebook.

use egui::{RichText, Ui};
use makine_lang::tr;
use makine_rv::isa::REG_NAMES;

use super::App;
use crate::computer::Computer;
use crate::theme;

/// What the debugger shows; not saved.
#[derive(Default)]
pub(super) struct Debug {
    pub isa_lesson: usize,
    pub isa_answer: String,
    pub isa_feedback: Option<(bool, String)>,
    pub isa_hint: bool,
    pub lesson_trace: Option<(String, Vec<crate::lessons::ExampleStep>)>,
    pub built: Option<(u64, String, makine_rv::asm::Program)>,
    pub selected_line: usize,
    pub slow: bool,
    pub last_tick: f64,
    pub lesson: Option<String>,
    pub prediction: String,
    pub show_answer: bool,
    pub show_hint: bool,
    pub report: Option<crate::lessons::Report>,
    pub report_source: Option<(u64, String)>,
    pub show_solution: bool,
    pub beginner: bool,
    pub number_format: u8,
    pub small_built: Option<(u64, String, makine_rv::smallasm::Program)>,
    pub mini_execution: Option<super::small_ui::MiniExecution>,
    pub mini_source: Option<String>,
    pub last_case: Option<(u64, String, crate::lessons::Case)>,
    pub help_query: String,
    /// Memory viewer address, as typed.
    pub mem_addr: String,
    /// The last build error; invalid source stays in the saved notebook.
    pub asm_error: Option<(usize, String)>,
    /// Registers before the last step, to light up what changed.
    pub before: Option<[u32; 32]>,
}

impl App {
    /// The tools under the screen: registers, code, memory, editor.
    pub(super) fn debugger(&mut self, ui: &mut Ui) {
        let Some(c) = self.computer.as_mut() else {
            return;
        };
        ui.add_space(8.0);
        ui.columns(2, |cols| {
            registers(&mut cols[0], c, &self.debug);
            code(&mut cols[1], c);
        });
        memory(ui, c, &mut self.debug.mem_addr);
        if ui
            .button(tr!(
                "Kaynağı Programlama'da aç",
                "Open source in Programming"
            ))
            .clicked()
        {
            if let Some((source, _)) = c.source.clone() {
                if let Some(id) = self
                    .progress
                    .notebook
                    .drafts
                    .iter()
                    .find(|d| d.source == source)
                    .map(|d| d.id)
                {
                    self.progress.notebook.select(id);
                } else {
                    self.progress.notebook.add(
                        tr!("Yüklenen program", "Loaded program"),
                        crate::programming::Architecture::RiscV,
                        &source,
                    );
                }
                self.save_soon();
            }
            self.view = super::View::Editor;
            self.editor_code = true;
        }
    }
}

/// The 32 registers in two columns; those changed by the last step lit.
pub(super) fn registers(ui: &mut Ui, c: &Computer, debug: &Debug) {
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
        .num_columns(if debug.number_format == 2 { 2 } else { 4 })
        .spacing(egui::vec2(8.0, 1.0))
        .show(ui, |ui| {
            for row in 0..16 {
                for r in [row, row + 16] {
                    if debug.beginner
                        && c.source.is_some()
                        && !c.source.as_ref().is_some_and(|(s, _)| {
                            s.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                                .filter_map(makine_rv::isa::parse_reg)
                                .any(|n| n as usize == r)
                        })
                    {
                        continue;
                    }
                    let changed = debug.before.is_some_and(|b| b[r] != x[r]);
                    ui.label(
                        RichText::new(format!("{:>4}", REG_NAMES[r]))
                            .monospace()
                            .size(12.0)
                            .color(theme::INK_DIM),
                    )
                    .on_hover_text(format!("x{r}"));
                    let value = match debug.number_format {
                        1 => (x[r] as i32).to_string(),
                        2 => format!("{:032b}", x[r]),
                        _ => format!("{:08x}", x[r]),
                    };
                    let v = RichText::new(value).monospace().size(12.0);
                    let v = if changed {
                        v.color(theme::SIGNAL)
                    } else if x[r] == 0 {
                        v.color(theme::INK_DIM)
                    } else {
                        v
                    };
                    ui.label(v)
                        .on_hover_text(format!("{} = {}", x[r], x[r] as i32));
                    if debug.number_format == 2 {
                        ui.end_row();
                    }
                }
                if debug.number_format != 2 {
                    ui.end_row();
                }
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
pub(super) fn memory(ui: &mut Ui, c: &Computer, addr: &mut String) {
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
