//! Small-machine programs use the same saved notebook, but their own
//! assemblers. Custom words are only encodings: run them on a real circuit.

use super::App;
use crate::{programming::Architecture, theme};
use egui::{RichText, Ui};
use makine_lang::{tr, trf};
use makine_rv::smallasm::{self, Dialect, Operand};
use makine_sim::Circuit;

pub(super) struct MiniExecution {
    cpu: makine_levels::mini::MiniDriver,
    rom: [u8; 16],
    ram: [u8; 16],
    pub error: Option<String>,
    ticks: u64,
    reference: bool,
}

impl MiniExecution {
    fn new(c: Circuit, p: &smallasm::Program, reference: bool) -> Result<Self, String> {
        let mut m = Self {
            cpu: makine_levels::mini::MiniDriver::new(c),
            rom: [0; 16],
            ram: [0; 16],
            error: None,
            ticks: 0,
            reference,
        };
        for (to, &v) in m.rom.iter_mut().zip(&p.words) {
            *to = v as u8;
        }
        for &(a, v) in &p.ram {
            m.ram[a] = v;
        }
        m.cpu.reset()?;
        Ok(m)
    }
    pub fn step(&mut self) {
        if self.error.is_some() {
            return;
        }
        if self.rom[self.cpu.pc() as usize & 15] >> 4 == 15 {
            self.error = Some(tr!("HLT: program durdu.", "HLT: program stopped.").into());
            return;
        }
        if let Err(e) = self.cpu.step(&self.rom, &mut self.ram) {
            self.error = Some(e);
        }
        self.ticks += 1;
    }
    pub(super) fn inspector(&self, ui: &mut Ui) {
        ui.label(if self.reference {
            tr!(
                "Motor: örnek MiniCpu devresi",
                "Engine: example MiniCpu circuit"
            )
        } else {
            tr!(
                "Motor: kendi MiniCpu devren",
                "Engine: your MiniCpu circuit"
            )
        });
        ui.monospace(format!(
            "PC: {}   acc: {}   ticks: {}",
            self.cpu.pc(),
            self.cpu.acc(),
            self.ticks
        ));
        if let Some(error) = &self.error {
            ui.colored_label(theme::WARN, error);
        }
        ui.separator();
        ui.strong(tr!("Canlı RAM", "Live RAM"));
        egui::Grid::new("mini-inspector-ram")
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr!("Adres", "Address"));
                ui.label("Hex");
                ui.label(tr!("Onluk", "Decimal"));
                ui.end_row();
                for (addr, value) in self.ram.iter().enumerate() {
                    ui.monospace(format!("{addr:02x}"));
                    ui.monospace(format!("{value:02x}"));
                    ui.monospace(value.to_string());
                    ui.end_row();
                }
            });
    }
    fn ui(&self, ui: &mut Ui) {
        ui.label(if self.reference {
            tr!(
                "Motor: örnek MiniCpu devresi",
                "Engine: example MiniCpu circuit"
            )
        } else {
            tr!(
                "Motor: kendi MiniCpu devren",
                "Engine: your MiniCpu circuit"
            )
        });
        ui.monospace(format!(
            "pc={}   acc={}   ticks={}",
            self.cpu.pc(),
            self.cpu.acc(),
            self.ticks
        ));
        if let Some(e) = &self.error {
            ui.colored_label(theme::WARN, e);
        }
        ui.columns(2, |cols| {
            for (a, &i) in self.rom.iter().enumerate() {
                cols[0].label(
                    RichText::new(format!(
                        "{} {a:02x}: {i:02x}  {}",
                        if self.cpu.pc() as usize == a {
                            "▶"
                        } else {
                            " "
                        },
                        makine_levels::mini::disasm(i)
                    ))
                    .monospace(),
                );
            }
            cols[1].label("RAM");
            for (a, v) in self.ram.iter().enumerate() {
                cols[1].monospace(format!("{a:02x}: {v:3} ({v:02x})"));
            }
        });
    }
}

impl App {
    pub(super) fn small_program_view(&mut self, ui: &mut Ui) {
        let Some(d) = self.progress.notebook.current().cloned() else {
            return;
        };
        ui.label(if d.architecture == Architecture::Mini {
            tr!("Derle ile kaynak satırlarını ROM baytlarına çevir. Devreye yükle programı baştan hazırlar; Adım bir komut yürütür.", "Build translates source lines into ROM bytes. Load into circuit restarts the program; Step executes one instruction.")
        } else {
            tr!("Derle: ROM kelimeleri, adresler ve kaynak satırları. Özel komut seti yalnızca kodlamayı tanımlar; davranışı senin devren sağlamalı.", "Build to see ROM words, addresses and source lines. A custom ISA defines only the encoding; your circuit must implement its behaviour.")
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr!("Derle", "Build")).clicked() {
                let dialect = match d.architecture {
                    Architecture::Mini => Dialect::Mini,
                    Architecture::Mcs4 => Dialect::Mcs4,
                    _ => Dialect::Custom(&self.progress.notebook.custom),
                };
                match smallasm::assemble(&d.source, dialect) {
                    Ok(p) => {
                        self.debug.small_built = Some((d.id, d.source.clone(), p));
                        self.debug.asm_error = None;
                    }
                    Err(e) => self.debug.asm_error = Some((e.line, e.message)),
                }
            }
            let valid = self
                .debug
                .small_built
                .as_ref()
                .is_some_and(|(id, s, _)| *id == d.id && *s == d.source);
            if ui
                .add_enabled(
                    valid,
                    egui::Button::new(tr!("ROM hex dışa aktar", "Export ROM hex")),
                )
                .clicked()
            {
                if let Some(p) = rfd::FileDialog::new()
                    .set_file_name("program.hex")
                    .save_file()
                {
                    let words = &self.debug.small_built.as_ref().unwrap().2.words;
                    let text = words
                        .iter()
                        .map(|v| format!("{v:04x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    if let Err(e) = std::fs::write(p, text) {
                        self.status = Some(e.to_string());
                    }
                }
            }
            if ui
                .add_enabled(
                    valid,
                    egui::Button::new(tr!("Devreye yükle", "Load into circuit")),
                )
                .clicked()
            {
                let p = self.debug.small_built.as_ref().unwrap().2.clone();
                match d.architecture {
                    Architecture::Mini => {
                        let own = self.progress.solved("MiniCpu");
                        let lib = if own {
                            &self.lib
                        } else {
                            makine_levels::cost::best_library()
                        };
                        match Circuit::compile(lib, "MiniCpu")
                            .map_err(|e| e.to_string())
                            .and_then(|c| MiniExecution::new(c, &p, !own))
                        {
                            Ok(m) => {
                                self.debug.mini_execution = Some(m);
                                self.debug.mini_source = Some(d.source.clone());
                            }
                            Err(e) => self.status = Some(e),
                        }
                    }
                    Architecture::Mcs4 => {
                        if self.session.is_some() {
                            self.status = Some(
                                tr!(
                                    "Önce ağ oturumundan ayrıl.",
                                    "Leave the network session first."
                                )
                                .into(),
                            );
                        } else {
                            self.open_project(makine_world::projects::mcs4::project(
                                &p.words.iter().map(|&v| v as u64).collect::<Vec<_>>(),
                            ));
                        }
                    }
                    Architecture::Custom => match self.custom_project(&p) {
                        Ok(p) => self.open_project(p),
                        Err(e) => self.status = Some(e),
                    },
                    _ => {}
                }
            }
            if d.architecture == Architecture::Mini
                && ui
                    .add_enabled(
                        self.debug.mini_execution.is_some(),
                        egui::Button::new(tr!("Adım", "Step")),
                    )
                    .on_hover_text(tr!(
                        "Önce Derle ve Devreye yükle.",
                        "Build and load into circuit first."
                    ))
                    .clicked()
            {
                if let Some(m) = &mut self.debug.mini_execution {
                    m.step();
                }
            }
        });
        if let Some((line, e)) = &self.debug.asm_error {
            ui.colored_label(theme::BAD, trf!("Satır {line}: {e}", "Line {line}: {e}"));
        }
        let mut changed = false;
        egui::ScrollArea::vertical().id_salt("small_assembly").show(ui,|ui|{
            if d.architecture==Architecture::Custom{self.isa_material(ui);self.custom_definition(ui);}
            if ui.available_width() < 560. {
                if let Some(d) = self.progress.notebook.current_mut() { changed = small_source(ui, d); }
                self.small_listing(ui, &d);
            } else {
                ui.columns(2, |cols| {
                    if let Some(d) = self.progress.notebook.current_mut() { changed = small_source(&mut cols[0], d); }
                    self.small_listing(&mut cols[1], &d);
                });
            }
            if d.architecture==Architecture::Mini{
                ui.label(tr!("Komutlar: NOP, LDI n, LD a, ST a, ADD a, SUB a, JMP etiket, JZ etiket, HLT. .ram adres,bayt başlangıç verisini tanımlar. ROM/RAM 16 adres.","Instructions: NOP, LDI n, LD a, ST a, ADD a, SUB a, JMP label, JZ label, HLT. .ram address,byte initializes data. ROM/RAM have 16 addresses."));
                if let Some(m)=&self.debug.mini_execution{
                    if self.debug.mini_source.as_ref()!=Some(&d.source){ui.colored_label(theme::WARN,tr!("Çalışan Mini programı bu taslaktan farklı; derle ve devreye yeniden yükle.","The running Mini program differs from this draft; build and load it again."));}
                    m.ui(ui);
                }
            }else if d.architecture==Architecture::Mcs4{
                ui.label(makine_world::projects::mcs4::supported());
                ui.label(tr!("4004'te HLT yok. JUN etiketiyle kendine dönerek beklet. Devre Dünya'da duraklatılmış açılır; Saat çevrimi bir komut yürütür.","4004 has no HLT. Use a self-jump to wait. The circuit opens paused in World; Clock cycle executes one instruction."));
            }
        });
        if changed {
            self.progress.notebook.sync_custom();
            self.save_soon();
        }
    }

    fn small_listing(&self, ui: &mut Ui, draft: &crate::programming::Draft) {
        let Some((_, source, program)) = self
            .debug
            .small_built
            .as_ref()
            .filter(|(id, _, _)| *id == draft.id)
        else {
            return;
        };
        if source != &draft.source {
            ui.colored_label(
                theme::WARN,
                tr!(
                    "Eski derleme baytları; güncel kaynak için yeniden derle.",
                    "Previous build bytes; rebuild for the current source."
                ),
            );
        }
        ui.label(tr!(
            "Adrese göre komut baytları",
            "Instruction bytes by address"
        ));
        for (address, &word) in program.words.iter().enumerate() {
            let description = if !program.lengths.contains_key(&(address as u32)) {
                tr!("veri / komutun devamı", "data / instruction continuation").into()
            } else {
                match draft.architecture {
                    Architecture::Mini => makine_levels::mini::disasm(word as u8),
                    Architecture::Mcs4 => makine_world::projects::mcs4::disassemble(
                        word as u8,
                        program.words.get(address + 1).copied().unwrap_or(0) as u8,
                    ),
                    _ => self.progress.notebook.custom.disasm(word),
                }
            };
            ui.monospace(format!(
                "{:>3}  {address:03x}  {word:04x}  {description}",
                program.lines.get(&(address as u32)).copied().unwrap_or(0)
            ));
        }
    }

    pub(super) fn mini_task_panel(&mut self, ui: &mut Ui, draft: &crate::programming::Draft) {
        let Some(lesson) = draft
            .activity
            .as_deref()
            .and_then(|key| key.strip_prefix("mini-lesson:"))
            .and_then(crate::mini_lessons::find)
            .filter(|_| draft.architecture == Architecture::Mini)
        else {
            return;
        };
        ui.heading(lesson.title);
        ui.label(RichText::new(lesson.task).strong());
        let ready = crate::mini_lessons::unlocked(lesson.id, &self.progress.notebook.solved);
        if !ready {
            ui.colored_label(theme::WARN, tr!("Okuyup deneyebilirsin; sınamadan önce önceki Mini kod görevini tamamla.", "You may read and experiment; finish the preceding Mini code task before checking."));
        }
        if let Some((_, _, result)) = self
            .debug
            .mini_feedback
            .as_ref()
            .filter(|(id, source, _)| *id == draft.id && source == &draft.source)
        {
            match result {
                Ok(message) => {
                    ui.colored_label(theme::GOOD, message);
                }
                Err(message) => {
                    ui.colored_label(theme::BAD, message);
                }
            }
        }
        egui::CollapsingHeader::new(tr!("Nasıl çalışır?", "How does it work?"))
            .id_salt(("mini-task-explanation", lesson.id))
            .show(ui, |ui| {
                ui.label(lesson.about);
                ui.label(tr!(
                    "Görev sınaması örnek MiniCpu devresini kullanır; kendi donanım seviyeni tamamlamaz. Derle → Devreye yükle → Adım ile sonucu ayrıca izle.",
                    "Task checks use the example MiniCpu circuit; they do not complete your hardware level. Also inspect the result with Build → Load into circuit → Step."
                ));
            });
        egui::CollapsingHeader::new(tr!("Mini görev ipucu", "Mini task hint"))
            .id_salt(("mini-task-hint", lesson.id))
            .show(ui, |ui| {
                ui.label(lesson.hint);
            });
        egui::CollapsingHeader::new(tr!("Mini görev örnek çözümü", "Mini task example solution"))
            .id_salt(("mini-task-solution", lesson.id))
            .show(ui, |ui| {
                ui.label(tr!(
                    "Bu örneği açmak taslağını değiştirmez.",
                    "Opening this example does not change your draft."
                ));
                ui.monospace(lesson.solution);
            });
        egui::CollapsingHeader::new(tr!("Deney notları", "Experiment notes"))
            .id_salt(("mini-task-notes", lesson.id))
            .show(ui, |ui| {
                let label = ui
                    .label(tr!(
                        "Tahminim ve deney notum",
                        "My prediction and experiment notes"
                    ))
                    .id;
                let notes = self
                    .progress
                    .notebook
                    .lesson_notes
                    .entry(format!("mini:{}", lesson.id))
                    .or_default();
                if ui
                    .add(
                        egui::TextEdit::multiline(notes)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(label)
                    .changed()
                {
                    self.save_soon();
                }
            });
    }

    pub(super) fn check_mini_task(&mut self) {
        let Some(draft) = self.progress.notebook.current().cloned() else {
            return;
        };
        let Some(id) = draft
            .activity
            .as_deref()
            .and_then(|key| key.strip_prefix("mini-lesson:"))
            .and_then(crate::mini_lessons::find)
            .map(|l| l.id)
        else {
            return;
        };
        if draft.architecture != Architecture::Mini {
            return;
        }
        let result = if crate::mini_lessons::unlocked(id, &self.progress.notebook.solved) {
            crate::mini_lessons::check(id, &draft.source)
        } else {
            Err(tr!(
                "Önceki Mini kod görevini tamamla.",
                "Finish the preceding Mini code task."
            )
            .into())
        };
        if result.is_ok() {
            self.progress.notebook.solved.insert(id.into());
            self.save_soon();
        }
        self.debug.mini_feedback = Some((draft.id, draft.source, result));
    }

    fn custom_definition(&mut self, ui: &mut Ui) {
        let mut changed = false;
        let mut template = false;
        egui::CollapsingHeader::new(tr!("Komut seti tasarımı", "Instruction set design")).id_salt("isa_design").default_open(true).show(ui,|ui|{
            let d=&mut self.progress.notebook.custom;
            ui.horizontal(|ui|{
                ui.label(tr!("Kelime bitleri", "Word bits"));changed|=ui.add(egui::DragValue::new(&mut d.bits).range(4..=16)).changed();
                ui.label(tr!("Opcode bitleri", "Opcode bits"));changed|=ui.add(egui::DragValue::new(&mut d.opcode_bits).range(1..=15)).changed();
            });
            let mut remove=None;
            for (k,i) in d.instructions.iter_mut().enumerate(){
                ui.horizontal(|ui|{
                    let label=ui.label(tr!("Komut", "Instruction")).id;
                    changed|=ui.add(egui::TextEdit::singleline(&mut i.name).desired_width(100.0)).labelled_by(label).changed();
                    changed|=ui.add(egui::DragValue::new(&mut i.opcode).range(0..=255)).changed();
                    let operand_label=ui.label(tr!("Argüman", "Operand")).id;
                    egui::ComboBox::from_id_salt(("operand",k)).selected_text(match i.operand{Operand::None=>tr!("Argüman yok", "No operand"),Operand::Value=>tr!("Sayı", "Value"),Operand::Address=>tr!("Adres", "Address")}).show_ui(ui,|ui|{
                        for (v,s) in [(Operand::None,tr!("Argüman yok", "No operand")),(Operand::Value,tr!("Sayı", "Value")),(Operand::Address,tr!("Adres", "Address"))]{changed|=ui.selectable_value(&mut i.operand,v,s).changed();}
                    }).response.labelled_by(operand_label);
                    if ui.small_button("×").clicked(){remove=Some(k);}
                });
            }
            if let Some(k)=remove{d.instructions.remove(k);changed=true;}
            if ui.button(tr!("Komut ekle", "Add instruction")).clicked(){d.instructions.push(smallasm::Instruction{name:format!("OP{}",d.instructions.len()),opcode:d.instructions.len() as u16,operand:Operand::None});changed=true;}
            if let Err(e)=d.validate(){ui.colored_label(theme::BAD,e);}
            ui.label(tr!("Devre sözleşmesi: instr[kelime], rdata[kelime], reset, clk → pc[adres], addr[adres], wdata[kelime], we. Adres 1–12 bit. İşlemleri bu serbest çipin içinde kapılarla sen kur.","Circuit contract: instr[word], rdata[word], reset, clk → pc[address], addr[address], wdata[word], we. Address width 1–12 bits. Build the operations inside this free chip yourself."));
            ui.horizontal(|ui|{let l=ui.label(tr!("CPU çip adı", "CPU chip name")).id;changed|=ui.text_edit_singleline(&mut self.progress.notebook.custom_chip).labelled_by(l).changed();});
            ui.horizontal_wrapped(|ui|{
                template=ui.button(tr!("Boş CPU şablonu oluştur", "Create blank CPU template")).clicked();
                if ui.button(tr!("ISA kaydet", "Save ISA")).clicked(){
                    if let Some(p)=rfd::FileDialog::new().set_file_name("instruction-set.json").save_file(){
                        match serde_json::to_string_pretty(&self.progress.notebook.custom).map_err(|e|e.to_string()).and_then(|s|std::fs::write(p,s).map_err(|e|e.to_string())){Ok(())=>{},Err(e)=>self.status=Some(e)}
                    }
                }
                if ui.button(tr!("ISA aç", "Open ISA")).clicked(){
                    if let Some(p)=rfd::FileDialog::new().add_filter("ISA", &["json"]).pick_file(){
                        let loaded=std::fs::read_to_string(p).map_err(|e|e.to_string()).and_then(|s|serde_json::from_str::<smallasm::Definition>(&s).map_err(|e|e.to_string()));
                        match loaded.and_then(|d|d.validate().map(|_|d)){Ok(d)=>{self.progress.notebook.custom=d;changed=true;},Err(e)=>self.status=Some(e)}
                    }
                }
            });
        });
        if template {
            let n = self.progress.notebook.custom_chip.trim().to_string();
            let d = &self.progress.notebook.custom;
            let result = d.validate().and_then(|_| {
                crate::sandbox::check_name(&n, &self.lib, &self.progress.sandbox, None)
            });
            match result {
                Ok(()) => {
                    let addr = (d.bits - d.opcode_bits).clamp(1, 12);
                    let ins = [
                        format!("instr[{}]", d.bits),
                        format!("rdata[{}]", d.bits),
                        "reset".into(),
                        "clk".into(),
                    ];
                    let outs = [
                        format!("pc[{addr}]"),
                        format!("addr[{addr}]"),
                        format!("wdata[{}]", d.bits),
                        "we".into(),
                    ];
                    let def = makine_sim::ChipDef::new(
                        &n,
                        &ins.iter().map(String::as_str).collect::<Vec<_>>(),
                        &outs.iter().map(String::as_str).collect::<Vec<_>>(),
                    );
                    self.progress
                        .sandbox
                        .chips
                        .insert(n.clone(), crate::design::Design::new(def));
                    self.refresh_library();
                    self.open_free(n);
                    self.save_soon();
                }
                Err(e) => self.status = Some(e),
            }
        }
        if changed {
            self.debug.small_built = None;
            self.debug.isa_feedback = None;
            self.progress.notebook.sync_custom();
            self.save_soon();
        }
    }

    fn custom_project(
        &self,
        p: &smallasm::Program,
    ) -> Result<makine_world::projects::Project, String> {
        use makine_sim::{Part, PartKind};
        use makine_world::{Device, Edit, End, Kind, Placed, Pos, World};
        self.progress.notebook.custom.validate()?;
        if self.session.is_some() {
            return Err(tr!(
                "Önce ağ oturumundan ayrıl.",
                "Leave the network session first."
            )
            .into());
        }
        let name = self.progress.notebook.custom_chip.trim();
        let chip = self.progress.sandbox.chips.get(name).ok_or_else(|| {
            tr!(
                "Önce Serbest çipler'de CPU çipini kur, adını burada yaz.",
                "First build a CPU in Free chips, then enter its name here."
            )
            .to_string()
        })?;
        let bits = self.progress.notebook.custom.bits;
        let input = |n: &str, w: u8| {
            chip.def
                .inputs
                .iter()
                .position(|x| x.name == n && x.width == w)
        };
        let output = |n: &str, w: u8| {
            chip.def
                .outputs
                .iter()
                .position(|x| x.name == n && x.width == w)
        };
        let addr = chip
            .def
            .outputs
            .iter()
            .find(|x| x.name == "pc")
            .map_or(0, |x| x.width);
        let bad = || {
            tr!(
                "CPU uçları sözleşmeyle eşleşmiyor veya ROM adres alanına sığmıyor.",
                "CPU pins do not match the contract, or the ROM does not fit its address space."
            )
            .to_string()
        };
        if !(1..=12).contains(&addr) || p.words.len() > 1usize << addr || chip.def.inputs.len() != 4
        {
            return Err(bad());
        }
        let (instr, rd, reset, clk) = (
            input("instr", bits).ok_or_else(bad)?,
            input("rdata", bits).ok_or_else(bad)?,
            input("reset", 1).ok_or_else(bad)?,
            input("clk", 1).ok_or_else(bad)?,
        );
        let (pc, ad, wd, we) = (
            output("pc", addr).ok_or_else(bad)?,
            output("addr", addr).ok_or_else(bad)?,
            output("wdata", bits).ok_or_else(bad)?,
            output("we", 1).ok_or_else(bad)?,
        );
        Circuit::compile_def(&self.lib, &chip.def).map_err(|e| e.to_string())?;
        let mut w = World::new(tr!("Kendi bilgisayarım", "My computer"));
        for (id, kind, x, y) in [
            (
                1,
                Kind::Part(Part::new(PartKind::Chip(name.into()))),
                300.,
                100.,
            ),
            (
                2,
                Kind::Device(Device::Rom {
                    addr,
                    data: bits,
                    words: p.words.iter().map(|&x| x as u64).collect(),
                }),
                40.,
                100.,
            ),
            (
                3,
                Kind::Device(Device::Ram { addr, data: bits }),
                600.,
                100.,
            ),
            (4, Kind::Clock { period: 8 }, 40., 400.),
            (
                5,
                Kind::Input {
                    name: "reset".into(),
                    width: 1,
                    value: 0,
                },
                300.,
                400.,
            ),
        ] {
            w.apply(Edit::Part {
                id,
                part: Some(Placed {
                    kind,
                    pos: Pos::new(x, y),
                }),
            });
        }
        for ((from, fp), (to, tp)) in [
            ((1, pc), (2, 0)),
            ((2, 0), (1, instr)),
            ((1, ad), (3, 0)),
            ((1, wd), (3, 1)),
            ((1, we), (3, 2)),
            ((3, 0), (1, rd)),
            ((4, 0), (1, clk)),
            ((4, 0), (3, 3)),
            ((5, 0), (1, reset)),
        ] {
            w.apply(Edit::Wire {
                from: Some(End::new(from, fp)),
                to: End::new(to, tp),
            });
        }
        Ok(makine_world::projects::Project {
            world: w,
            chips: vec![],
            watches: vec![End::new(1, pc), End::new(1, wd), End::new(1, we)],
            collapsed: vec![],
        })
    }
}

fn small_source(ui: &mut Ui, draft: &mut crate::programming::Draft) -> bool {
    let label = ui.label(tr!("Kaynak kod", "Source code")).id;
    ui.add(
        egui::TextEdit::multiline(&mut draft.source)
            .code_editor()
            .desired_rows(20)
            .desired_width(f32::INFINITY),
    )
    .labelled_by(label)
    .changed()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assembled_mini_program_runs_on_a_real_circuit() {
        let p = smallasm::assemble(
            "LDI 5\nloop: ST 14\nSUB 15\nJZ done\nJMP loop\ndone: HLT\n.ram 15,1",
            Dialect::Mini,
        )
        .unwrap();
        let c = Circuit::compile(makine_levels::cost::best_library(), "MiniCpu").unwrap();
        let mut m = MiniExecution::new(c, &p, true).unwrap();
        for _ in 0..30 {
            m.step();
        }
        assert_eq!(m.cpu.acc(), 0);
        assert_eq!(m.ram[14], 1);
        assert_eq!(m.cpu.pc(), 5);
    }

    #[test]
    fn custom_words_are_loaded_into_a_matching_gate_built_cpu() {
        let mut progress = crate::store::Progress::default();
        for l in makine_levels::levels() {
            progress
                .solutions
                .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
        }
        let mut cpu = makine_levels::reference::reference("MiniCpu");
        cpu.name = "CustomCpu".into();
        cpu.outputs[2].name = "wdata".into();
        progress
            .sandbox
            .chips
            .insert("CustomCpu".into(), crate::design::Design::new(cpu));
        progress.notebook.custom_chip = "CustomCpu".into();
        let a = App::new(progress, None);
        let p = smallasm::assemble("YUKLE 7\nDUR", Dialect::Custom(&a.progress.notebook.custom))
            .unwrap();
        let world = a.custom_project(&p).unwrap().world;
        let compiled = makine_world::compile(&world, &a.lib);
        assert!(compiled.problems.is_empty(), "{:?}", compiled.problems);
        let mut sim = crate::worldsim::WorldSim::default();
        sim.set_library(a.lib.clone());
        sim.speed = 50;
        sim.run_frame(&world);
        assert_eq!(sim.value(makine_world::End::new(1, 2)), Some(7));
        assert_eq!(sim.value(makine_world::End::new(1, 0)), Some(1));
    }
}
