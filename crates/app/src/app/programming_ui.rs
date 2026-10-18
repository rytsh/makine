//! The source-first programming workbench, using the same computer as the
//! Computer tab. Drafts and the loaded image deliberately have separate lives.

use super::{debug_ui, App};
use crate::{programming::Architecture, theme};
use egui::{RichText, Ui};
use makine_lang::{tr, trf};

impl App {
    pub(super) fn programming_panel(&mut self, ui: &mut Ui) {
        if self.debug.report_source.as_ref().is_some_and(|(id, s)| {
            !self
                .progress
                .notebook
                .current()
                .is_some_and(|d| d.id == *id && d.source == *s)
        }) {
            self.debug.report = None;
            self.debug.report_source = None;
        }
        ui.heading(tr!("Programlama atölyesi", "Programming workshop"));
        ui.label(tr!(
            "Taslaklar otomatik kaydedilir. Derlemek programı çalıştırmaz.",
            "Drafts save automatically. Building does not run your program."
        ));
        let risc_v = self
            .progress
            .notebook
            .current()
            .is_some_and(|d| d.architecture == Architecture::RiscV);
        if risc_v && self.progress.solved("Cpu") {
            if ui
                .checkbox(
                    &mut self.turbo,
                    tr!("Referans işlemciyi kullan", "Use reference processor"),
                )
                .changed()
            {
                self.computer = None;
                self.debug.slow = false;
            }
        } else if risc_v {
            ui.label(
                RichText::new(tr!(
                    "CPU'yu bitirmeden referans işlemcide öğrenebilirsin.",
                    "Learn on the reference processor before completing your CPU."
                ))
                .small()
                .weak(),
            );
        }
        theme::section(ui, tr!("Programlarım", "My programs"));
        let drafts = self.progress.notebook.drafts.clone();
        for d in drafts {
            if ui
                .selectable_label(self.progress.notebook.selected == d.id, &d.name)
                .on_hover_text(d.architecture.name())
                .clicked()
            {
                self.progress.notebook.select(d.id);
                self.debug.asm_error = None;
                self.debug.report = None;
                self.debug.last_case = None;
                self.debug.isa_feedback = None;
                self.save_soon();
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr!("Yeni", "New")).clicked() {
                self.progress.notebook.add(
                    tr!("Yeni program", "New program"),
                    Architecture::RiscV,
                    "ebreak\n",
                );
                self.save_soon();
            }
            if ui.button(tr!("Çoğalt", "Duplicate")).clicked() {
                if let Some(d) = self.progress.notebook.current().cloned() {
                    self.progress.notebook.add(
                        &format!("{} (2)", d.name),
                        d.architecture,
                        &d.source,
                    );
                    self.save_soon();
                }
            }
            if ui.button(tr!("Dosya aç", "Open file")).clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Assembly", &["s", "asm"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&p) {
                        Ok(s) if s.len() <= 1024 * 1024 => {
                            self.progress.notebook.add(
                                p.file_stem().and_then(|s| s.to_str()).unwrap_or("program"),
                                Architecture::RiscV,
                                &s,
                            );
                            self.save_soon();
                        }
                        Ok(_) => {
                            self.status = Some(
                                tr!(
                                    "Kaynak en çok 1 MB olabilir.",
                                    "Source must not exceed 1 MB."
                                )
                                .into(),
                            )
                        }
                        Err(e) => self.status = Some(e.to_string()),
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (a, s) in [
                (
                    Architecture::Mini,
                    "LDI 5\nloop: ST 14\nSUB 15\nJZ done\nJMP loop\ndone: HLT\n.ram 15, 1\n",
                ),
                (
                    Architecture::Mcs4,
                    "FIM P0, 0\nloop: SRC P0\nRDM\nIAC\nWRM\nRDR\nCLC\nADM\nWMP\nJUN loop\n",
                ),
                (
                    Architecture::Custom,
                    "YUKLE 5\nTOPLA 2\nATLA end\nend: DUR\n",
                ),
            ] {
                if ui.button(format!("+ {}", a.name())).clicked() {
                    self.progress.notebook.add(a.name(), a, s);
                    self.save_soon();
                }
            }
        });
        if let Some(d) = self.progress.notebook.current().cloned() {
            if ui
                .button(tr!(".s dosyasına kaydet", "Save .s file"))
                .clicked()
            {
                if let Some(p) = rfd::FileDialog::new()
                    .set_file_name("program.s")
                    .save_file()
                {
                    if let Err(e) = std::fs::write(p, d.source) {
                        self.status = Some(e.to_string());
                    }
                }
            }
            if d.architecture != Architecture::RiscV {
                theme::section(
                    ui,
                    tr!("Bu makineyi programlamak", "Programming this machine"),
                );
                ui.label(match d.architecture{
                    Architecture::Mini=>tr!("Önce Derle, sonra Devreye yükle. Adım ile acc, pc ve RAM'i izle. MiniCpu seviyesini bitirdiysen kendi devren kullanılır; yoksa örnek devre açıkça etiketlenir.","Build, then Load into circuit. Step to watch acc, pc and RAM. If you solved MiniCpu, your circuit is used; otherwise the example circuit is explicitly labelled."),
                    Architecture::Mcs4=>tr!("Derlenen kod Dünya'da 4004 atölyesine yüklenir; mevcut dünyan korunur. Saat çevrimi ile komut yürüt. WPM bu donanımda desteklenmez.","Compiled code opens in the 4004 World workshop, keeping your current world. Clock cycle executes an instruction. This hardware does not implement WPM."),
                    _=>tr!("Bir komutun adını, opcode'unu ve argümanını tanımla. CPU çipini Serbest çipler'de kendin kur; Derle ve Devreye yükle ROM'u o devreye bağlar. Hazır davranış emülatörü eklenmez.","Define instruction names, opcodes and operands. Build the CPU in Free chips; Build and Load into circuit connect ROM to that circuit. No behaviour emulator is added."),
                });
                if d.architecture == Architecture::Custom {
                    self.isa_panel(ui);
                }
                return;
            }
        }
        theme::section(ui, tr!("Öğrenme yolu", "Learning path"));
        egui::CollapsingHeader::new(tr!("Daha büyük projeler", "Larger projects"))
            .id_salt("program_projects")
            .show(ui, |ui| {
                for (name, source) in crate::lessons::projects() {
                    if ui.button(name).clicked() {
                        self.progress
                            .notebook
                            .add(name, Architecture::RiscV, source);
                        self.save_soon();
                    }
                }
            });
        let current = crate::lessons::all()
            .into_iter()
            .find(|l| Some(l.id) == self.debug.lesson.as_deref())
            .map_or(tr!("Ders seç", "Choose lesson"), |l| l.title);
        let lesson_label = ui.label(tr!("Ders", "Lesson")).id;
        egui::ComboBox::from_id_salt("lesson_picker")
            .width(ui.available_width() - 16.0)
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (i, l) in crate::lessons::all().into_iter().enumerate() {
                    let solved = self.progress.notebook.solved.contains(l.id);
                    let title = format!("{}{}. {}", if solved { "✔ " } else { "" }, i + 1, l.title);
                    if ui
                        .selectable_label(self.debug.lesson.as_deref() == Some(l.id), title)
                        .clicked()
                    {
                        self.debug.lesson = Some(l.id.into());
                        self.debug.show_answer = false;
                        self.debug.show_hint = false;
                        self.debug.show_solution = false;
                        self.debug.prediction = self
                            .progress
                            .notebook
                            .lesson_notes
                            .get(&format!("predict:{}", l.id))
                            .cloned()
                            .unwrap_or_default();
                        self.debug.report = None;
                    }
                }
            })
            .response
            .labelled_by(lesson_label);
        if let Some(l) = crate::lessons::all()
            .into_iter()
            .find(|l| Some(l.id) == self.debug.lesson.as_deref())
        {
            ui.separator();
            ui.heading(l.title);
            let ready = crate::lessons::unlocked(l.id, &self.progress.notebook.solved);
            if !ready {
                let previous = crate::lessons::all()
                    .into_iter()
                    .take_while(|p| p.id != l.id)
                    .find(|p| !self.progress.notebook.solved.contains(p.id))
                    .map_or("", |p| p.title);
                ui.colored_label(
                    theme::WARN,
                    trf!(
                        "Bu dersi okuyabilirsin. Görev için önce tamamla: {previous}",
                        "You may read this lesson. Before its task, complete: {previous}"
                    ),
                );
            }
            ui.label(l.about);
            theme::section(ui, tr!("Tahmin et", "Predict"));
            ui.label(l.predict);
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.debug.prediction)
                        .hint_text(tr!("Tahminin", "Your prediction")),
                )
                .changed()
            {
                self.progress
                    .notebook
                    .lesson_notes
                    .insert(format!("predict:{}", l.id), self.debug.prediction.clone());
                self.save_soon();
            }
            if ui.button(tr!("Yanıtı açıkla", "Reveal answer")).clicked() {
                self.debug.show_answer = true;
            }
            if self.debug.show_answer {
                ui.label(trf!("Yanıt: {}", "Answer: {}", l.answer));
            }
            ui.label(l.change);
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr!("Örneği aç", "Open example")).clicked() {
                    self.progress
                        .notebook
                        .add(l.title, Architecture::RiscV, l.example);
                    self.save_soon();
                }
                if ui
                    .add_enabled(ready, egui::Button::new(tr!("Görevi başlat", "Start task")))
                    .clicked()
                {
                    self.progress
                        .notebook
                        .add(l.title, Architecture::RiscV, l.starter);
                    self.save_soon();
                }
            });
            theme::section(ui, tr!("Kendin yaz", "Write it yourself"));
            ui.label(l.task);
            if ui
                .add_enabled(ready, egui::Button::new(tr!("Görevi test et", "Test task")))
                .clicked()
            {
                self.test_program(l.id);
            }
            if ui.button(tr!("İpucu", "Hint")).clicked() {
                self.debug.show_hint = true;
            }
            if self.debug.show_hint {
                ui.label(l.hint);
            }
            ui.checkbox(
                &mut self.debug.show_solution,
                tr!(
                    "Örnek çözümü göster (taslağım değişmesin)",
                    "Reveal solution (keep my draft)"
                ),
            );
            if self.debug.show_solution {
                ui.code(l.solution);
            }
        }
        if let Some(r) = &self.debug.report {
            let passed = r.failure.is_none() && r.total > 0;
            ui.separator();
            ui.label(trf!(
                "{} / {} test geçti",
                "{} / {} tests passed",
                r.passed,
                r.total
            ));
            if let Some((case, e)) = &r.failure {
                ui.colored_label(theme::BAD, e);
                for (reg, v) in &case.registers {
                    ui.monospace(format!("x{reg} = {v} ({})", *v as i32));
                }
                for (addr, bytes) in &case.data {
                    ui.monospace(format!(
                        "RAM {addr:08x}: {}",
                        bytes
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ));
                }
                if ui
                    .button(tr!("Bu girişlerle adımla", "Step with these inputs"))
                    .clicked()
                {
                    self.load_failed_case();
                }
            }
            if passed {
                let lessons = crate::lessons::all();
                if let Some(next) = lessons
                    .iter()
                    .position(|l| Some(l.id) == self.debug.lesson.as_deref())
                    .and_then(|i| lessons.get(i + 1))
                {
                    if ui
                        .button(trf!("Sonraki ders: {}", "Next lesson: {}", next.title))
                        .clicked()
                    {
                        self.debug.lesson = Some(next.id.into());
                        self.debug.report = None;
                        self.debug.show_answer = false;
                        self.debug.show_hint = false;
                        self.debug.show_solution = false;
                        self.debug.prediction = self
                            .progress
                            .notebook
                            .lesson_notes
                            .get(&format!("predict:{}", next.id))
                            .cloned()
                            .unwrap_or_default();
                    }
                }
            }
        }
        if let Some(c) = &self.computer {
            ui.separator();
            ui.label(
                if matches!(c.engine, crate::computer::Engine::Reference(_)) {
                    tr!("Motor: referans işlemci", "Engine: reference processor")
                } else {
                    tr!("Motor: kendi CPU devren", "Engine: your CPU circuit")
                },
            );
            if matches!(c.engine, crate::computer::Engine::Circuit(_)) {
                ui.label(RichText::new(tr!("Register değerleri referans eş-çalıştırmadan; devreden doğrudan ölçüm değildir.","Registers come from the reference shadow, not direct circuit measurements.")).small().color(theme::WARN));
            }
            ui.checkbox(
                &mut self.debug.beginner,
                tr!(
                    "Yalnızca kullanılan register'lar",
                    "Only registers used in source"
                ),
            );
            let radix_label = ui.label(tr!("Sayı gösterimi", "Number format")).id;
            egui::ComboBox::from_id_salt("register_radix")
                .selected_text(match self.debug.number_format {
                    1 => tr!("Onluk (işaretli)", "Decimal (signed)"),
                    2 => tr!("İkilik", "Binary"),
                    _ => tr!("Onaltılık", "Hexadecimal"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.debug.number_format,
                        0,
                        tr!("Onaltılık", "Hexadecimal"),
                    );
                    ui.selectable_value(
                        &mut self.debug.number_format,
                        1,
                        tr!("Onluk (işaretli)", "Decimal (signed)"),
                    );
                    ui.selectable_value(&mut self.debug.number_format, 2, tr!("İkilik", "Binary"));
                })
                .response
                .labelled_by(radix_label);
            debug_ui::registers(ui, c, &self.debug);
        }
    }

    fn test_program(&mut self, id: &str) {
        if !crate::lessons::unlocked(id, &self.progress.notebook.solved) {
            self.status = Some(
                tr!(
                    "Görevden önce eksik temel dersleri tamamla.",
                    "Complete the missing foundation lessons before this task."
                )
                .into(),
            );
            return;
        }
        let Some(d) = self.progress.notebook.current().cloned() else {
            return;
        };
        if d.architecture != Architecture::RiscV {
            return;
        }
        match makine_rv::asm::assemble(&d.source) {
            Ok(p) => {
                let r = crate::lessons::check(id, &p);
                if r.failure.is_none() && r.total > 0 {
                    self.progress.notebook.solved.insert(id.into());
                    self.save_soon();
                }
                self.debug.report = Some(r);
                self.debug.report_source = Some((d.id, d.source));
                self.debug.asm_error = None;
            }
            Err(e) => self.debug.asm_error = Some((e.line, e.message)),
        }
    }

    fn load_failed_case(&mut self) {
        let Some(case) = self
            .debug
            .report
            .as_ref()
            .and_then(|r| r.failure.as_ref())
            .map(|(c, _)| c.clone())
        else {
            return;
        };
        let Some(d) = self.progress.notebook.current().cloned() else {
            return;
        };
        if let Ok(p) = makine_rv::asm::assemble(&d.source) {
            self.debug.built = Some((d.id, d.source.clone(), p.clone()));
            let (cpu, machine) = crate::lessons::initialize(&p, &case);
            let mut c = crate::computer::Computer::new(crate::computer::Engine::Reference(
                makine_rv::emu::Cpu::new(),
            ));
            if let Err(e) = c.load_program(vec![(0, p.bytes.clone())], 0) {
                self.status = Some(e);
                return;
            }
            c.engine = crate::computer::Engine::Reference(cpu);
            c.machine = machine;
            self.debug.last_case = Some((d.id, d.source.clone(), case));
            c.source = Some((d.source, p));
            c.trace_enabled = true;
            self.computer = Some(c);
            self.turbo = true;
            self.debug.slow = false;
        }
    }

    pub(super) fn programming_view(&mut self, ui: &mut Ui) {
        let Some(draft) = self.progress.notebook.current().cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.heading(tr!("Programlama", "Programming"));
            ui.label(draft.architecture.name());
        });
        let mut dirty = false;
        let mut architecture_changed = false;
        if let Some(d) = self.progress.notebook.current_mut() {
            ui.horizontal(|ui| {
                let label = ui.label(tr!("Ad", "Name")).id;
                dirty |= ui
                    .add(egui::TextEdit::singleline(&mut d.name).desired_width(200.0))
                    .labelled_by(label)
                    .changed();
                let arch_label = ui.label(tr!("Mimari", "Architecture")).id;
                egui::ComboBox::from_id_salt("draft_architecture")
                    .selected_text(d.architecture.name())
                    .show_ui(ui, |ui| {
                        for a in [
                            Architecture::RiscV,
                            Architecture::Mini,
                            Architecture::Mcs4,
                            Architecture::Custom,
                        ] {
                            architecture_changed |= ui
                                .selectable_value(&mut d.architecture, a, a.name())
                                .changed();
                        }
                    })
                    .response
                    .labelled_by(arch_label);
            });
        }
        if architecture_changed {
            self.debug.built = None;
            self.debug.small_built = None;
            self.debug.asm_error = None;
            self.debug.report = None;
            self.save_soon();
            return;
        }
        if dirty {
            self.debug.report = None;
            self.debug.last_case = None;
            self.save_soon();
        }
        if draft.architecture != Architecture::RiscV {
            self.small_program_view(ui);
            return;
        }
        let mut build = false;
        let mut load = false;
        let mut step = false;
        let mut source_step = false;
        let mut until = false;
        ui.horizontal_wrapped(|ui| {
            build = ui.button(tr!("Derle", "Build")).clicked();
            let fresh = self
                .debug
                .built
                .as_ref()
                .is_some_and(|(id, s, _)| *id == draft.id && *s == draft.source);
            load = ui
                .add_enabled(
                    fresh,
                    egui::Button::new(tr!("Baştan yükle", "Load from start")),
                )
                .clicked();
            step = ui.button(tr!("Bir komut", "One instruction")).clicked();
            source_step = ui
                .button(tr!("Bir kaynak satırı", "One source line"))
                .clicked();
            until = ui.button(tr!("İmlece kadar", "Run to cursor")).clicked();
            ui.checkbox(&mut self.debug.slow, tr!("Yavaş çalıştır", "Run slowly"));
            if let Some(c) = &mut self.computer {
                if ui
                    .button(if c.running {
                        tr!("Durdur", "Pause")
                    } else {
                        tr!("Çalıştır", "Run")
                    })
                    .clicked()
                {
                    c.running = !c.running;
                    self.debug.slow = false;
                    c.pause_reason = None;
                    c.instruction_limit = Some(c.instructions + 100_000);
                }
            }
        });
        if build {
            match makine_rv::asm::assemble(&draft.source) {
                Ok(p) => {
                    self.debug.built = Some((draft.id, draft.source.clone(), p));
                    self.debug.asm_error = None;
                }
                Err(e) => {
                    self.debug.asm_error = Some((e.line, e.message));
                }
            }
        }
        if load {
            self.load_program(&draft.name, draft.source.as_bytes());
            if let Some(c) = &mut self.computer {
                c.running = false;
                c.trace_enabled = true;
            }
            self.debug.before = None;
            if let Some((_, _, case)) = self
                .debug
                .last_case
                .clone()
                .filter(|(id, source, _)| *id == draft.id && *source == draft.source)
            {
                if let Some(c) = &mut self.computer {
                    if let crate::computer::Engine::Reference(cpu) = &mut c.engine {
                        for (r, v) in case.registers {
                            cpu.x[r] = v;
                        }
                        for (a, bytes) in case.data {
                            c.machine.load(a, &bytes);
                        }
                    }
                }
            }
        }
        if until {
            if let Some(c) = &mut self.computer {
                if let Some((s, p)) = &c.source {
                    if *s == draft.source {
                        c.run_until = p
                            .lines
                            .iter()
                            .filter(|(_, line)| **line == self.debug.selected_line)
                            .map(|(a, _)| *a)
                            .min();
                        c.running = c.run_until.is_some();
                        c.instruction_limit = Some(c.instructions + 100_000);
                        self.debug.slow = false;
                    }
                }
            }
        }
        if step || source_step {
            self.debug.slow = false;
            if let Some(c) = &mut self.computer {
                c.running = false;
                self.debug.before = c.registers().copied();
                let result = if source_step {
                    c.step_source()
                } else {
                    c.step_n(1)
                };
                if let Err(e) = result {
                    c.halted = Some(e);
                }
            }
        }
        if self.debug.slow {
            let now = ui.input(|i| i.time);
            if now - self.debug.last_tick >= 0.4 {
                self.debug.last_tick = now;
                if let Some(c) = &mut self.computer {
                    c.running = false;
                    self.debug.before = c.registers().copied();
                    if let Err(e) = c.step_n(1) {
                        c.halted = Some(e);
                        self.debug.slow = false;
                    }
                }
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        let active_line = self.computer.as_ref().and_then(|c| {
            c.source
                .as_ref()
                .filter(|(s, _)| *s == draft.source)
                .and_then(|_| c.source_line())
        });
        if self
            .computer
            .as_ref()
            .and_then(|c| c.source.as_ref())
            .is_some_and(|(s, _)| *s != draft.source)
        {
            ui.colored_label(
                theme::WARN,
                tr!(
                    "Çalışan görüntü bu taslaktan farklı. Derle ve baştan yükle.",
                    "The loaded image differs from this draft. Build and load it again."
                ),
            );
        }
        if let Some((line, e)) = &self.debug.asm_error {
            ui.colored_label(theme::BAD, trf!("Satır {line}: {e}", "Line {line}: {e}"));
        }
        egui::ScrollArea::vertical()
            .id_salt("program_workbench")
            .show(ui, |ui| {
                self.programming_teaching(ui, &draft);
                ui.columns(2, |cols| {
                    let source_label = cols[0]
                        .label(RichText::new(tr!("Kaynak kod", "Source code")).strong())
                        .id;
                    if let Some(d) = self.progress.notebook.current_mut() {
                        let error = self.debug.asm_error.as_ref().map(|e| e.0);
                        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, width: f32| {
                            let mut job = egui::text::LayoutJob::default();
                            for (i, line) in text.as_str().split_inclusive('\n').enumerate() {
                                let color = if error == Some(i + 1) {
                                    theme::BAD
                                } else if active_line == Some(i + 1) {
                                    theme::SIGNAL
                                } else if line.trim_start().starts_with('#') {
                                    theme::INK_DIM
                                } else {
                                    theme::INK
                                };
                                job.append(
                                    line,
                                    0.0,
                                    egui::TextFormat::simple(egui::FontId::monospace(14.0), color),
                                );
                            }
                            job.wrap.max_width = width;
                            ui.fonts_mut(|f| f.layout_job(job))
                        };
                        let output = egui::TextEdit::multiline(&mut d.source)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(23)
                            .layouter(&mut layouter)
                            .show(&mut cols[0]);
                        egui::Response::clone(&output.response).labelled_by(source_label);
                        dirty |= output.response.changed();
                        if output.response.has_focus() {
                            if let Some(c) = output.cursor_range {
                                self.debug.selected_line = d
                                    .source
                                    .chars()
                                    .take(c.primary.index.into())
                                    .filter(|c| *c == '\n')
                                    .count()
                                    + 1;
                            }
                        }
                    }
                    cols[1].label(
                        RichText::new(tr!("Kaynak → makine kodu", "Source → machine code"))
                            .strong(),
                    );
                    if let Some((id, source, p)) = &self.debug.built {
                        if *id == draft.id {
                            for (&addr, &line) in
                                p.lines.iter().collect::<std::collections::BTreeMap<_, _>>()
                            {
                                let word = p
                                    .bytes
                                    .get(addr as usize..addr as usize + 4)
                                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                                    .unwrap_or(0);
                                let breakpoint = self.computer.as_ref().is_some_and(|c| {
                                    c.breakpoints.contains(&addr)
                                        && c.source.as_ref().is_some_and(|(s, _)| s == source)
                                });
                                let text = format!(
                                    "{} {line:>3}  {addr:08x}  {word:08x}  {}",
                                    if breakpoint { "●" } else { " " },
                                    makine_rv::disasm::disasm(word, addr)
                                );
                                let t = RichText::new(text).monospace().size(12.0);
                                let r = cols[1].add(
                                    egui::Label::new(if active_line == Some(line) {
                                        t.color(theme::SIGNAL)
                                    } else if breakpoint {
                                        t.color(theme::BAD)
                                    } else {
                                        t
                                    })
                                    .sense(egui::Sense::click()),
                                );
                                if r.clicked() {
                                    self.debug.selected_line = line;
                                }
                                if r.double_clicked() {
                                    if let Some(c) = &mut self.computer {
                                        if c.source.as_ref().is_some_and(|(s, _)| s == source)
                                            && !c.breakpoints.remove(&addr)
                                        {
                                            c.breakpoints.insert(addr);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        cols[1].label(tr!(
                            "Derle: her satırın gerçek komutlarını burada gör.",
                            "Build to see the real instructions for each line here."
                        ));
                    }
                    cols[1].label(tr!(
                        "Çift tık: adreste durma noktası.",
                        "Double-click: breakpoint at this address."
                    ));
                });
                self.instruction_explanation(ui, &draft);
                self.measured_datapath(ui);
                if let Some(c) = &mut self.computer {
                    c.run_for(std::time::Duration::from_millis(8));
                    if c.running {
                        ui.ctx().request_repaint();
                    }
                    if let Some(e) = &c.halted {
                        ui.colored_label(theme::WARN, e);
                    }
                    if let Some(e) = &c.pause_reason {
                        ui.colored_label(theme::WARN, e);
                    }
                    if let Some(t) = &c.trace {
                        theme::section(
                            ui,
                            tr!("Son adımda ne oldu?", "What happened in the last step?"),
                        );
                        ui.monospace(format!(
                            "PC {:08x} → {:08x}   {}",
                            t.pc,
                            t.next_pc,
                            makine_rv::disasm::disasm(t.instr, t.pc)
                        ));
                        if let (Some(a), Some(b)) = (t.before, t.after) {
                            for r in 1..32 {
                                if a[r] != b[r] {
                                    ui.monospace(format!(
                                        "{}: {} → {}",
                                        makine_rv::isa::REG_NAMES[r],
                                        a[r] as i32,
                                        b[r] as i32
                                    ));
                                }
                            }
                        }
                        if let Some((a, v, mask)) = t.store {
                            ui.monospace(format!("RAM / I/O [{a:08x}] ← {v:08x}  mask={mask:04b}"));
                        }
                    }
                    debug_ui::memory(ui, c, &mut self.debug.mem_addr);
                    egui::CollapsingHeader::new(tr!("Ekran ve konsol", "Screen and console"))
                        .id_salt("workbench_output")
                        .show(ui, |ui| c.ui(ui));
                }
                ui.separator();
                egui::CollapsingHeader::new(tr!("Komut rehberi", "Instruction reference"))
                    .id_salt("instruction_reference")
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.debug.help_query)
                                .hint_text(tr!("Komut ara (ör. lw)", "Find instruction (e.g. lw)")),
                        );
                        let query = self.debug.help_query.to_ascii_lowercase();
                        for (name, syntax, about) in crate::instruction_help::entries()
                            .into_iter()
                            .filter(|(n, _, _)| n.to_ascii_lowercase().contains(&query))
                        {
                            ui.label(RichText::new(name).strong());
                            ui.code(syntax);
                            ui.label(about);
                            ui.add_space(5.0);
                        }
                    });
            });
        if dirty {
            self.debug.report = None;
            self.debug.last_case = None;
            self.save_soon();
        }
    }

    fn measured_datapath(&self, ui: &mut Ui) {
        let Some(crate::computer::Computer {
            engine: crate::computer::Engine::Circuit(cpu),
            ..
        }) = &self.computer
        else {
            return;
        };
        let Some(def) = self.lib.get("Cpu") else {
            return;
        };
        egui::CollapsingHeader::new(tr!("Kendi CPU devremden ölçülen teller", "Wires measured in my CPU circuit")).id_salt("measured_cpu").show(ui,|ui|{
            ui.label(tr!("Bunlar referans değerler değil, çalışan devrenin son yerleşmiş tel değerleridir. Saat kenarından sonraki durumu gösterir.","These are the running circuit's settled wire values, not reference values. They show the state after the clock edge."));
            for (wi,w) in def.wires.iter().enumerate(){
                if let makine_sim::Source::Part{part,pin}=w.from {
                    if let Some(makine_sim::Part{kind:makine_sim::PartKind::Chip(name),..})=def.parts.get(part){
                        if matches!(name.as_str(),"Decoder"|"Control"|"RegFile"|"Alu"|"BranchUnit"|"LoadUnit"|"StoreUnit") {
                            let label=self.lib.get(name).and_then(|c|c.outputs.get(pin)).map_or("?",|p|p.name.as_str());
                            ui.monospace(format!("{name}.{label} = {:08x}",cpu.circuit.wire(wi)));
                        }
                    }
                }
            }
        });
    }

    fn instruction_explanation(&self, ui: &mut Ui, draft: &crate::programming::Draft) {
        let Some((id, _, p)) = &self.debug.built else {
            return;
        };
        if *id != draft.id {
            return;
        }
        let current = self
            .computer
            .as_ref()
            .filter(|c| c.source.as_ref().is_some_and(|(s, _)| *s == draft.source))
            .map(|c| c.pc());
        let addr = p
            .lines
            .iter()
            .filter(|(_, l)| **l == self.debug.selected_line)
            .map(|(a, _)| *a)
            .min()
            .or(current);
        let Some(addr) = addr else { return };
        let Some(bytes) = p.bytes.get(addr as usize..addr as usize + 4) else {
            return;
        };
        let word = u32::from_le_bytes(bytes.try_into().unwrap());
        let d = makine_rv::explain::decode(word);
        egui::CollapsingHeader::new(tr!("Komuttan donanıma", "From instruction to hardware")).id_salt("instruction_fields").default_open(true).show(ui,|ui|{
            ui.monospace(format!("{:032b}",word));
            ui.monospace(format!("{}  opcode={:07b} rd=x{} rs1=x{} rs2=x{} funct3={:03b} funct7={:07b}",d.format,makine_rv::isa::opcode(word),makine_rv::isa::rd(word),makine_rv::isa::rs1(word),makine_rv::isa::rs2(word),makine_rv::isa::funct3(word),word>>25));
            ui.label(tr!("Alan konumları gösterilir; her biçim hepsini kullanmaz (ör. S'de rd yok).", "Field positions are shown; not every format uses all of them (e.g. S has no rd)."));
            if let Some(i)=d.immediate{ui.monospace(format!("imm = {i}"));}
            ui.label(d.path);
            ui.label(d.meaning);
            ui.label(RichText::new(tr!("Bu, bir çevrimin açıklayıcı veri yolu şemasıdır; ek saat aşamaları veya devreden ölçülmüş sinyaller değildir.","This explains the datapath within a cycle, not extra clock stages or signals measured from the circuit.")).small().weak());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Progress;
    use egui_kittest::kittest::Queryable;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn beginner_can_build_load_and_step_without_solving_cpu() {
        let app = Rc::new(RefCell::new(App::new(Progress::default(), None)));
        app.borrow_mut().view = super::super::View::Programming;
        let a = app.clone();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1440., 960.))
            .build_ui(move |ui| a.borrow_mut().ui(ui));
        h.run_steps(3);
        h.get_by_label("Derle").click();
        h.run_steps(3);
        assert!(app.borrow().debug.built.is_some());
        assert!(app.borrow().computer.is_none(), "build does not run");
        h.get_by_label("Baştan yükle").click();
        h.run_steps(3);
        assert!(matches!(
            app.borrow().computer.as_ref().unwrap().engine,
            crate::computer::Engine::Reference(_)
        ));
        assert!(!app.borrow().computer.as_ref().unwrap().running);
        h.get_by_label("Bir kaynak satırı").click();
        h.run_steps(3);
        assert_eq!(
            app.borrow().computer.as_ref().unwrap().registers().unwrap()[10],
            42
        );
        app.borrow_mut()
            .progress
            .notebook
            .current_mut()
            .unwrap()
            .source = "not_an_instruction".into();
        h.run_steps(3);
        h.get_by_label("Derle").click();
        h.run_steps(3);
        assert!(app.borrow().debug.asm_error.is_some());
        assert_eq!(
            app.borrow().computer.as_ref().unwrap().registers().unwrap()[10],
            42,
            "failed build keeps loaded machine"
        );
    }

    #[test]
    fn task_failures_are_reproducible_in_the_debugger() {
        let mut a = App::new(Progress::default(), None);
        a.progress
            .notebook
            .solved
            .extend(["register".into(), "copy".into()]);
        a.progress.notebook.current_mut().unwrap().source = "ebreak".into();
        a.test_program("sum");
        assert_eq!(a.debug.report.as_ref().unwrap().passed, 0);
        a.load_failed_case();
        let c = a.computer.as_ref().unwrap();
        assert_eq!(
            (c.registers().unwrap()[10], c.registers().unwrap()[11]),
            (2, 3)
        );
        assert_eq!(c.source_line(), Some(1));
    }

    #[test]
    fn task_checks_require_foundations_but_do_not_erase_old_completions() {
        let mut a = App::new(Progress::default(), None);
        a.progress.notebook.current_mut().unwrap().source = "add a0,a0,a1\nebreak".into();
        a.test_program("sum");
        assert!(a.debug.report.is_none());
        assert!(!a.progress.notebook.solved.contains("sum"));
        for id in ["register", "copy", "sum"] {
            let source = crate::lessons::all()
                .into_iter()
                .find(|l| l.id == id)
                .unwrap()
                .solution;
            a.progress.notebook.current_mut().unwrap().source = source.into();
            a.test_program(id);
            assert!(a.progress.notebook.solved.contains(id));
        }
        a.progress
            .notebook
            .lesson_notes
            .insert("predict:bits".into(), "1".into());
        let saved = serde_json::to_string(&a.progress).unwrap();
        let fresh = App::new(serde_json::from_str(&saved).unwrap(), None);
        assert_eq!(fresh.debug.lesson.as_deref(), Some("bits"));
        assert_eq!(fresh.debug.prediction, "1");
        assert!(fresh.progress.notebook.solved.contains("sum"));
    }

    #[test]
    #[ignore]
    fn programming_screenshots() {
        let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
        std::fs::create_dir_all(&dir).unwrap();
        for (lang, width) in [
            (makine_lang::Lang::Tr, 1440.),
            (makine_lang::Lang::En, 1000.),
        ] {
            makine_lang::set_for_thread(Some(lang));
            let mut app = App::new(Progress::default(), None);
            app.view = super::super::View::Programming;
            app.debug.lesson = Some("loop".into());
            let src = crate::lessons::all()
                .into_iter()
                .find(|l| l.id == "loop")
                .unwrap()
                .example;
            app.progress.notebook.current_mut().unwrap().source = src.into();
            let id = app.progress.notebook.selected;
            app.debug.built = Some((id, src.into(), makine_rv::asm::assemble(src).unwrap()));
            app.load_program("loop", src.as_bytes());
            let c = app.computer.as_mut().unwrap();
            c.running = false;
            c.trace_enabled = true;
            c.step_n(3).unwrap();
            app.debug.selected_line = 3;
            let app = Rc::new(RefCell::new(app));
            let a = app.clone();
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(width, 960.))
                .wgpu()
                .build_ui(move |ui| a.borrow_mut().ui(ui));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/programming-{}.png", lang.code()))
                .unwrap();
            app.borrow_mut()
                .progress
                .notebook
                .add("Mini", Architecture::Mini, "LDI 5\nHLT\n");
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/mini-{}.png", lang.code()))
                .unwrap();
            app.borrow_mut()
                .progress
                .notebook
                .add("ISA", Architecture::Custom, "YUKLE 5\nDUR\n");
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/isa-{}.png", lang.code()))
                .unwrap();
            {
                let mut a = app.borrow_mut();
                a.debug.isa_lesson = 5;
                a.progress
                    .notebook
                    .isa_solved
                    .extend(crate::isa_lessons::all()[..5].iter().map(|l| l.id.into()));
            }
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/isa-control-{}.png", lang.code()))
                .unwrap();
            {
                let mut a = app.borrow_mut();
                a.view = super::super::View::Editor;
                a.progress.designs.insert(
                    "MiniDecode".into(),
                    crate::design::Design::new(makine_levels::reference::reference("MiniDecode")),
                );
                a.open_level(makine_levels::find("MiniDecode").unwrap());
            }
            h.run_steps(4);
            h.get_by_label(tr!(
                "Ayrıntılı anlatım ve deney",
                "Detailed explanation and experiment"
            ))
            .click();
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/level-guide-{}.png", lang.code()))
                .unwrap();
            makine_lang::set_for_thread(None);
        }
    }
}
