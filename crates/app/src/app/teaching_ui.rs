//! Explanations stay separate from build controls. A learner can inspect
//! future material, but completing a course requires its prerequisites.

use super::{App, View};
use crate::{isa_lessons, programming::Architecture, theme};
use egui::{RichText, Ui};
use makine_lang::{tr, trf};

impl App {
    pub(super) fn circuit_teaching(&mut self, ui: &mut Ui) {
        let chip = self.level.chip;
        let Some(g) = makine_levels::teaching::guide(chip) else {
            return;
        };
        egui::CollapsingHeader::new(tr!(
            "Ayrıntılı anlatım ve deney",
            "Detailed explanation and experiment"
        ))
        .id_salt(("circuit_teaching", chip))
        .show(ui, |ui| {
            ui.label(RichText::new(tr!("Neden bu devre?", "Why this circuit?")).strong());
            ui.label(g.concept);
            ui.add_space(6.0);
            ui.label(RichText::new(tr!("Somut örnek", "Worked example")).strong());
            ui.label(g.example);
            ui.add_space(6.0);
            ui.label(RichText::new(tr!("Sık hata", "Common mistake")).strong());
            ui.label(g.trap);
            ui.add_space(6.0);
            ui.label(RichText::new(tr!("Kendin dene ve açıkla", "Try it and explain")).strong());
            ui.label(g.experiment);
            let key = format!("circuit:{chip}");
            let label = ui
                .label(tr!(
                    "Kendi notum / beklediğim sonuç",
                    "My note / expected result"
                ))
                .id;
            let notes = self.progress.notebook.lesson_notes.entry(key).or_default();
            if ui
                .add(
                    egui::TextEdit::multiline(notes)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .labelled_by(label)
                .changed()
            {
                self.save_soon();
            }
        });
    }

    pub(super) fn isa_panel(&mut self, ui: &mut Ui) {
        ui.separator();
        ui.heading(tr!(
            "Komut setini adım adım öğren",
            "Learn ISA design step by step"
        ));
        let lessons = isa_lessons::all();
        self.debug.isa_lesson = self.debug.isa_lesson.min(lessons.len() - 1);
        let label = ui.label(tr!("Tasarım dersi", "Design lesson")).id;
        egui::ComboBox::from_id_salt("isa_course_picker")
            .width(ui.available_width() - 16.)
            .selected_text(lessons[self.debug.isa_lesson].title)
            .show_ui(ui, |ui| {
                for (i, l) in lessons.iter().enumerate() {
                    let done = self.progress.notebook.isa_solved.contains(l.id);
                    if ui
                        .selectable_label(
                            self.debug.isa_lesson == i,
                            format!("{}{}", if done { "✔ " } else { "" }, l.title),
                        )
                        .clicked()
                    {
                        self.debug.isa_lesson = i;
                        self.debug.isa_answer.clear();
                        self.debug.isa_hint = false;
                        self.debug.isa_feedback = None;
                    }
                }
            })
            .response
            .labelled_by(label);
        let l = &lessons[self.debug.isa_lesson];
        ui.label(l.objective);
        let ready = lessons[..self.debug.isa_lesson]
            .iter()
            .all(|l| self.progress.notebook.isa_solved.contains(l.id));
        if !ready {
            ui.colored_label(
                theme::WARN,
                tr!(
                    "Okuyabilirsin; tamamlamak için önceki derslerin kontrollerini geç.",
                    "You may read ahead; pass earlier checks before completing this lesson."
                ),
            );
        }
        ui.label(l.task);
        if !l.answer.is_empty() {
            let label = ui.label(tr!("Yanıtın", "Your answer")).id;
            ui.add(egui::TextEdit::singleline(&mut self.debug.isa_answer))
                .labelled_by(label);
        }
        if ui
            .add_enabled(
                ready,
                egui::Button::new(tr!("Ders kontrolünü çalıştır", "Run lesson check")),
            )
            .clicked()
        {
            self.check_isa_lesson();
        }
        if let Some((passed, message)) = &self.debug.isa_feedback {
            ui.colored_label(if *passed { theme::GOOD } else { theme::BAD }, message);
        }
        if ui.button(tr!("İpucu göster", "Show hint")).clicked() {
            self.debug.isa_hint = true;
        }
        if self.debug.isa_hint {
            ui.label(l.hint);
        }
        if let Some(chip) = l.chip {
            if ui
                .button(trf!("{chip} seviyesine git", "Go to {chip} level"))
                .clicked()
            {
                if let Some(level) = makine_levels::find(chip) {
                    if self.progress.unlocked(level.index()) {
                        self.view = View::Editor;
                        self.open_level(level);
                    } else {
                        self.status=Some(tr!("Önce açık ara devre seviyelerini tamamla; sonraki seviyeler sırayla açılır.","Finish the unlocked intermediate circuit levels first; later levels unlock in order.").into());
                    }
                }
            }
        }
        if self.debug.isa_lesson == 2
            && ui
                .button(tr!("Ayrı ders taslağı aç", "Open separate lesson draft"))
                .clicked()
        {
            self.progress.notebook.add(
                tr!("ISA eğitim taslağım", "My ISA course draft"),
                Architecture::Custom,
                "LDI 3\nST 0\nADD 0\nST 1\nHLT\n",
            );
            self.progress.notebook.custom = makine_rv::smallasm::Definition {
                bits: 8,
                opcode_bits: 4,
                instructions: vec![],
            };
            self.progress.notebook.custom_chip = String::new();
            self.progress.notebook.sync_custom();
            self.debug.small_built = None;
            self.save_soon();
        }
        if self.debug.isa_lesson == 9 {
            let label = ui
                .label(tr!("Kendi CPU çipimin adı", "My CPU chip name"))
                .id;
            if ui
                .text_edit_singleline(&mut self.progress.notebook.custom_chip)
                .labelled_by(label)
                .changed()
            {
                self.progress.notebook.sync_custom();
                self.save_soon();
            }
            if ui
                .add_enabled(
                    self.progress.solved("MiniCpu"),
                    egui::Button::new(tr!(
                        "Kendi MiniCpu çözümümden serbest CPU oluştur",
                        "Make a free CPU from my MiniCpu solution"
                    )),
                )
                .clicked()
            {
                self.make_course_cpu();
            }
        }
        let completed = self.progress.notebook.isa_solved.contains(l.id);
        if ui
            .add_enabled(
                completed && self.debug.isa_lesson + 1 < lessons.len(),
                egui::Button::new(tr!("Sonraki tasarım dersi", "Next design lesson")),
            )
            .clicked()
        {
            self.debug.isa_lesson += 1;
            self.debug.isa_answer.clear();
            self.debug.isa_feedback = None;
            self.debug.isa_hint = false;
        }
    }

    pub(super) fn isa_material(&mut self, ui: &mut Ui) {
        let all = isa_lessons::all();
        let l = &all[self.debug.isa_lesson.min(all.len() - 1)];
        egui::CollapsingHeader::new(l.title)
            .id_salt(("isa_material", l.id))
            .default_open(true)
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(760.0));
                ui.label(l.explanation);
                ui.add_space(8.);
                ui.label(RichText::new(tr!("Adım adım örnek", "Worked example")).strong());
                ui.code(l.example);
                if matches!(l.id, "isa-control" | "isa-control-gates" | "isa-wiring") {
                    egui::Grid::new(("isa_control_table", l.id))
                        .striped(true)
                        .show(ui, |ui| {
                            for name in ["op", "load", "imm", "alu", "sub", "we", "jump", "hold"] {
                                ui.label(RichText::new(name).strong());
                            }
                            ui.end_row();
                            for (name, signals) in isa_lessons::control_rows() {
                                ui.monospace(name);
                                for signal in signals {
                                    ui.monospace(signal.to_string());
                                }
                                ui.end_row();
                            }
                        });
                }
                let label = ui.label(tr!("Tasarımla ilgili notum", "My design note")).id;
                let note = self
                    .progress
                    .notebook
                    .lesson_notes
                    .entry(l.id.into())
                    .or_default();
                if ui
                    .add(
                        egui::TextEdit::multiline(note)
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

    pub(super) fn check_isa_lesson(&mut self) {
        let lessons = isa_lessons::all();
        let Some(l) = lessons.get(self.debug.isa_lesson) else {
            return;
        };
        if !lessons[..self.debug.isa_lesson]
            .iter()
            .all(|l| self.progress.notebook.isa_solved.contains(l.id))
        {
            self.debug.isa_feedback = Some((
                false,
                tr!(
                    "Önceki dersleri tamamla.",
                    "Complete earlier lessons first."
                )
                .into(),
            ));
            return;
        }
        let result = if !l.answer.is_empty() {
            if isa_lessons::normalize(&self.debug.isa_answer) == isa_lessons::normalize(l.answer) {
                Ok(tr!(
                    "Doğru. Örnekteki nedenini de kendi notuna yaz.",
                    "Correct. Explain why in your own note too."
                )
                .into())
            } else {
                Err(tr!("Henüz doğru değil. Bit alanlarını, önceliği veya hangi komutun en son yazdığını tekrar izle.","Not yet correct. Recheck the fields, priorities, or the last instruction that wrote the value.").into())
            }
        } else if l.id == "isa-encoding" {
            isa_lessons::check_definition(&self.progress.notebook.custom).map(|_| {
                tr!(
                    "Dokuz komutun kodlaması sözleşmeyle eşleşiyor.",
                    "All nine instruction encodings match the contract."
                )
                .into()
            })
        } else if l.id == "isa-run" {
            isa_lessons::check_definition(&self.progress.notebook.custom).and_then(|_| {
                let name = self.progress.notebook.custom_chip.trim();
                let def = self.progress.sandbox.chips.get(name).ok_or_else(|| {
                    tr!(
                        "Önce kendi serbest CPU çipinin adını gir.",
                        "Enter the name of your own free CPU chip first."
                    )
                    .to_string()
                })?;
                isa_lessons::check_cpu(&self.lib, &def.def).map(|n| {
                    trf!(
                        "{n} çevrim doğrulandı: kendi ISA'n kendi devrende çalışıyor.",
                        "Verified {n} cycles: your ISA runs on your circuit."
                    )
                })
            })
        } else if let Some(chip) = l.chip {
            self.progress
                .solutions
                .get(chip)
                .ok_or_else(|| {
                    trf!(
                        "Önce {chip} seviyesini kendi devrenle geç.",
                        "First pass {chip} with your circuit."
                    )
                })
                .and_then(|def| {
                    let level = makine_levels::find(chip).unwrap();
                    match makine_levels::check(level, &self.lib, def).map_err(|e| e.to_string())? {
                        makine_sim::Verdict::Pass => Ok(trf!(
                            "{chip} kayıtlı çözümün tekrar doğrulandı.",
                            "Your saved {chip} solution passed again."
                        )),
                        _ => Err(tr!(
                            "Kayıtlı devre kontrolü geçmedi; seviyeye dönüp düzelt.",
                            "The saved circuit failed; return to the level and fix it."
                        )
                        .into()),
                    }
                })
        } else {
            Err(tr!(
                "Bu dersin kontrolü bulunamadı.",
                "No check was found for this lesson."
            )
            .into())
        };
        match result {
            Ok(s) => {
                self.progress.notebook.isa_solved.insert(l.id.into());
                self.debug.isa_feedback = Some((true, s));
                self.save_soon();
            }
            Err(s) => self.debug.isa_feedback = Some((false, s)),
        }
    }

    pub(super) fn make_course_cpu(&mut self) {
        let name = self.progress.notebook.custom_chip.trim().to_string();
        let Some(solution) = self.progress.solutions.get("MiniCpu").cloned() else {
            self.debug.isa_feedback = Some((
                false,
                tr!(
                    "Önce MiniCpu'yu kendi devrenle çöz.",
                    "First solve MiniCpu with your circuit."
                )
                .into(),
            ));
            return;
        };
        let result=crate::sandbox::check_name(&name,&self.lib,&self.progress.sandbox,None).and_then(|_|{
            let level=makine_levels::find("MiniCpu").unwrap();
            if !matches!(makine_levels::check(level,&self.lib,&solution).map_err(|e|e.to_string())?,makine_sim::Verdict::Pass){return Err(tr!("Kendi MiniCpu çözümün yeniden doğrulanamadı.","Your MiniCpu solution did not pass again.").into());}
            let mut def=solution;def.name=name.clone();def.outputs[2].name="wdata".into();
            self.progress.sandbox.chips.insert(name.clone(),crate::design::Design::new(def));
            self.refresh_library();self.progress.notebook.sync_custom();self.save_soon();
            Ok(tr!("Kendi çözümün kopyalandı: yalnızca acc çıkışı wdata adını aldı. Artık son kontrolü çalıştırabilirsin.","Your solution was copied: only output acc was renamed wdata. You can now run the final check.").to_string())
        });
        self.debug.isa_feedback = Some(match result {
            Ok(s) => (true, s),
            Err(s) => (false, s),
        });
    }

    pub(super) fn programming_teaching(&mut self, ui: &mut Ui, draft: &crate::programming::Draft) {
        let Some(l) = crate::lessons::all()
            .into_iter()
            .find(|l| Some(l.id) == self.debug.lesson.as_deref())
        else {
            return;
        };
        egui::CollapsingHeader::new(tr!("Dersi ayrıntılı incele", "Study this lesson in detail")).id_salt(("lesson_reading",l.id)).show(ui,|ui|{
            ui.heading(l.title);ui.label(l.about);
            ui.label(RichText::new(tr!("Örnek program: izle ve açıkla", "Example program: trace and explain")).strong());ui.code(l.example);
            ui.label(tr!("Bu örnek kendi başlangıç değerlerini kurar. Görev taslağı ayrıdır: testler görev girişlerini register ve belleğe kendisi koyar. Örnekteki sabitleri göreve kopyalamak farklı girişlerde başarısız olur.","This example sets its own starting values. The task draft is separate: tests supply task registers and memory. Copying example constants into the task will fail on other inputs."));
            if ui.button(tr!("Örneğin açıklamalı izini çıkar", "Trace the example with changes")).clicked(){
                self.debug.lesson_trace=Some((l.id.into(),crate::lessons::trace(l.example)));
            }
            if let Some((id,steps))=&self.debug.lesson_trace{if id==l.id{
                ui.label(tr!("Temiz makine; konsol örneğinde başlangıç girdisi Enter. İlk 32 komut gösterilir, kullanıcı programın değiştirilmez.","Fresh machine; console example input is Enter. Showing the first 32 instructions without changing your program."));
                for step in steps {ui.monospace(format!("L{}  {:04x}: {}",step.line,step.pc,step.instruction));ui.label(&step.changes);}
            }}
            ui.label(l.change);ui.label(RichText::new(tr!("Sınanacak girişler", "Inputs that will be tested")).strong());
            for c in crate::lessons::cases(l.id).iter().take(3){
                ui.monospace(c.registers.iter().map(|(r,v)|trf!("x{r}={v} (işaretli {})","x{r}={v} (signed {})",*v as i32)).collect::<Vec<_>>().join(", "));
            }
            let key=format!("program:{}",l.id);
            let label=ui.label(tr!("Kendi açıklamam / denemem", "My explanation / experiment")).id;
            let note=self.progress.notebook.lesson_notes.entry(key).or_default();
            if ui.add(egui::TextEdit::multiline(note).desired_rows(3).desired_width(f32::INFINITY)).labelled_by(label).changed(){self.save_soon();}
            ui.label(trf!("Çalışma taslağın: {} (değiştirilmedi)","Your draft: {} (unchanged)",draft.name));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn course_cannot_be_completed_by_skipping_or_using_reference_fallback() {
        let mut app = App::new(crate::store::Progress::default(), None);
        app.debug.isa_lesson = 9;
        app.check_isa_lesson();
        assert!(!app.debug.isa_feedback.as_ref().unwrap().0);
        assert!(app.progress.notebook.isa_solved.is_empty());
        let lessons = isa_lessons::all();
        app.progress
            .notebook
            .isa_solved
            .extend(lessons[..9].iter().map(|l| l.id.into()));
        app.progress.notebook.custom = isa_lessons::definition();
        app.check_isa_lesson();
        assert!(
            !app.debug.isa_feedback.as_ref().unwrap().0,
            "no own chip must fail"
        );
        assert!(!app.progress.notebook.isa_solved.contains("isa-run"));
    }
    #[test]
    fn saved_learner_solution_can_become_a_free_cpu_and_pass_the_capstone() {
        let mut p = crate::store::Progress::default();
        for l in makine_levels::levels() {
            p.solutions
                .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
        }
        let mut app = App::new(p, None);
        app.debug.isa_lesson = 9;
        app.progress
            .notebook
            .isa_solved
            .extend(isa_lessons::all()[..9].iter().map(|l| l.id.into()));
        app.progress.notebook.custom = isa_lessons::definition();
        app.progress.notebook.custom_chip = "MyCpu".into();
        app.make_course_cpu();
        assert!(
            app.debug.isa_feedback.as_ref().unwrap().0,
            "{:?}",
            app.debug.isa_feedback
        );
        app.check_isa_lesson();
        assert!(
            app.progress.notebook.isa_solved.contains("isa-run"),
            "{:?}",
            app.debug.isa_feedback
        );
        let saved = serde_json::to_string(&app.progress).unwrap();
        let loaded: crate::store::Progress = serde_json::from_str(&saved).unwrap();
        assert!(loaded.notebook.isa_solved.contains("isa-run"));
        // Creating again must refuse to overwrite the student's saved chip.
        app.make_course_cpu();
        assert!(!app.debug.isa_feedback.as_ref().unwrap().0);
    }

    #[test]
    fn all_ten_course_checks_pass_in_sequence_and_wrong_answers_do_not_advance() {
        let mut p = crate::store::Progress::default();
        for l in makine_levels::levels() {
            p.solutions
                .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
        }
        let mut app = App::new(p, None);
        app.progress
            .notebook
            .add("Course", Architecture::Custom, "LDI 3\nHLT");
        app.progress.notebook.custom = isa_lessons::definition();
        app.progress.notebook.custom_chip = "CourseCpu".into();
        app.make_course_cpu();
        for (i, l) in isa_lessons::all().iter().enumerate() {
            app.debug.isa_lesson = i;
            if !l.answer.is_empty() {
                app.debug.isa_answer = "wrong".into();
                app.check_isa_lesson();
                assert!(!app.progress.notebook.isa_solved.contains(l.id));
            }
            app.debug.isa_answer = l.answer.into();
            app.check_isa_lesson();
            assert!(
                app.progress.notebook.isa_solved.contains(l.id),
                "{}: {:?}",
                l.id,
                app.debug.isa_feedback
            );
        }
    }
}
