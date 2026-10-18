//! Circuit, code and live world are three tools for the same activity.
//! Draft/world links are saved, and edits always go through world history.
use super::{App, View};
use crate::{
    curriculum::{self, Activity},
    programming::Architecture,
    theme,
};
use egui::{RichText, Ui};
use makine_lang::{tr, trf};

impl App {
    pub(super) fn showing_code(&self) -> bool {
        (self.view == View::Editor && self.editor_code)
            || (self.view == View::World && self.world_code)
    }

    pub(super) fn open_activity(&mut self, a: Activity) {
        let chip = a.chip();
        let level = makine_levels::find(chip).unwrap();
        if self.view != View::World {
            if !self.progress.unlocked(level.index()) {
                self.status = Some(
                    tr!(
                        "Önce bu devrenin önceki seviyelerini tamamla.",
                        "Complete this circuit's earlier levels first."
                    )
                    .into(),
                );
                return;
            }
            self.open_level(level);
            self.view = View::Editor;
            self.editor_code = true;
        } else {
            self.world_code = true;
        }
        let (key, title, architecture, source) = match a {
            Activity::Program(id) => {
                self.debug.lesson = Some(id.into());
                self.debug.show_hint = false;
                self.debug.show_solution = false;
                self.debug.show_answer = false;
                self.debug.prediction = self
                    .progress
                    .notebook
                    .lesson_notes
                    .get(&format!("predict:{id}"))
                    .cloned()
                    .unwrap_or_default();
                let l = crate::lessons::all()
                    .into_iter()
                    .find(|l| l.id == id)
                    .unwrap();
                (
                    format!("program:{id}"),
                    l.title,
                    Architecture::RiscV,
                    l.starter,
                )
            }
            Activity::Isa(i) => {
                self.debug.isa_lesson = i;
                self.debug.isa_feedback = None;
                self.debug.isa_answer.clear();
                self.debug.isa_hint = false;
                (
                    "isa-course".into(),
                    tr!("Kendi ISA çalışmam", "My ISA project"),
                    Architecture::Custom,
                    "LDI 3\nST 0\nADD 0\nST 1\nHLT\n",
                )
            }
        };
        let id = self
            .progress
            .notebook
            .drafts
            .iter()
            .find(|d| d.activity.as_deref() == Some(&key))
            .map(|d| d.id)
            .unwrap_or_else(|| {
                let id = self.progress.notebook.add(title, architecture, source);
                if architecture == Architecture::Custom {
                    self.progress.notebook.custom = makine_rv::smallasm::Definition {
                        bits: 8,
                        opcode_bits: 4,
                        instructions: vec![],
                    };
                    self.progress.notebook.custom_chip = String::new();
                    self.progress.notebook.sync_custom();
                }
                self.progress.notebook.current_mut().unwrap().activity = Some(key);
                id
            });
        self.progress.notebook.select(id);
        self.debug.report = None;
        self.debug.report_source = None;
        self.debug.last_case = None;
        self.debug.asm_error = None;
        self.save_soon();
    }

    pub(super) fn level_activities(&mut self, ui: &mut Ui) {
        let chip = self.level.chip;
        let activities = curriculum::activities(chip);
        theme::section(
            ui,
            tr!("Kur → Kodla → Dünya'da dene", "Build → Code → Try in World"),
        );
        ui.label(tr!(
            "Aynı çalışmanın devre, program ve deney adımları; ilerlemen ortaktır.",
            "Circuit, program and experiment steps of the same project share progress."
        ));
        for a in activities {
            let label = format!(
                "{} {}",
                if a.done(&self.progress.notebook) {
                    "✔"
                } else {
                    "○"
                },
                a.title()
            );
            if ui.button(label).clicked() {
                self.open_activity(a);
            }
        }
        if let Some(example) = curriculum::example(chip) {
            if ui
                .add_enabled(
                    self.session.is_none(),
                    egui::Button::new(tr!("Bu kavramı Dünya'da dene", "Try this concept in World")),
                )
                .clicked()
            {
                if let Some(e) = makine_world::projects::examples::all()
                    .into_iter()
                    .find(|e| e.id == example)
                {
                    self.open_project((e.build)());
                    self.attach_lab(chip, None, None);
                }
            }
        }
        if chip == "MiniCpu"
            && ui
                .button(tr!(
                    "Kendi Mini CPU'ma program yaz",
                    "Write code for my Mini CPU"
                ))
                .clicked()
        {
            self.open_mini_activity();
        }
        if chip == "Counter12"
            && ui
                .add_enabled(
                    self.session.is_none(),
                    egui::Button::new(tr!(
                        "4004 programı ve Dünya deneyi",
                        "4004 code and World experiment"
                    )),
                )
                .clicked()
        {
            self.progress.notebook.add(
                "4004",
                Architecture::Mcs4,
                "FIM P0,0\nloop: SRC P0\nRDM\nIAC\nWRM\nJUN loop\n",
            );
            self.editor_code = true;
        }
    }

    pub(super) fn open_mini_activity(&mut self) {
        const KEY: &str = "mini-program";
        if let Some(id) = self
            .progress
            .notebook
            .drafts
            .iter()
            .find(|d| d.activity.as_deref() == Some(KEY))
            .map(|d| d.id)
        {
            self.progress.notebook.select(id);
        } else {
            self.progress.notebook.add(
                tr!("Mini CPU programım", "My Mini CPU program"),
                Architecture::Mini,
                "LDI 5\nloop: ST 14\nSUB 15\nJZ done\nJMP loop\ndone: HLT\n.ram 15,1\n",
            );
            self.progress.notebook.current_mut().unwrap().activity = Some(KEY.into());
        }
        if self.view == View::World {
            self.world_code = true;
        } else {
            self.editor_code = true;
        }
        self.save_soon();
    }

    pub(super) fn attach_lab(&mut self, chip: &str, draft: Option<u64>, rom: Option<u64>) {
        if let Some(w) = &self.world {
            let lab = self
                .progress
                .notebook
                .labs
                .entry(w.world.name.clone())
                .or_default();
            lab.chip = Some(chip.into());
            lab.draft = draft;
            lab.rom = rom;
        }
        self.save_soon();
    }

    pub(super) fn workspace_header(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if self.view == View::World {
                let name = self
                    .world
                    .as_ref()
                    .map(|w| w.world.name.as_str())
                    .unwrap_or("");
                ui.label(RichText::new(trf!("Dünya: {name}", "World: {name}")).strong());
                if ui.button(tr!("Devreye dön", "Back to circuit")).clicked() {
                    self.world_code = false;
                }
            } else {
                ui.label(
                    RichText::new(trf!("Seviye: {}", "Level: {}", self.level.name())).strong(),
                );
                if ui
                    .button(tr!("Devreyi kur / incele", "Build / inspect circuit"))
                    .clicked()
                {
                    self.editor_code = false;
                }
            }
        });
        if self.view == View::World {
            self.world_code_actions(ui);
        } else {
            self.level_activities(ui);
        }
        ui.separator();
    }

    pub(super) fn world_learning_panel(&mut self, ui: &mut Ui) {
        let Some(w) = &self.world else { return };
        let name = w.world.name.clone();
        let lab = self
            .progress
            .notebook
            .labs
            .get(&name)
            .cloned()
            .unwrap_or_default();
        egui::CollapsingHeader::new(tr!("Bu dünyada öğren ve kodla", "Learn and code in this world")).id_salt("world_learning").default_open(lab.chip.is_some()||lab.draft.is_some()).show(ui,|ui|{
            if let Some(chip)=&lab.chip{
                if let Some(l)=makine_levels::find(chip){
                    ui.label(trf!("Bağlı seviye: {}", "Linked level: {}",l.name()));
                    if ui.button(tr!("Seviyedeki devreme dön", "Return to my level circuit")).clicked(){self.view=View::Editor;self.open_level(l);}
                }
                for a in curriculum::activities(chip){if ui.button(format!("{} {}",if a.done(&self.progress.notebook){"✔"}else{"○"},a.title())).clicked(){self.open_activity(a);}}
            }else{
                ui.label(tr!("Bir seviyenin deneyini burada geliştirebilir veya seçili ROM'a kendi programını bağlayabilirsin.","Develop a level's experiment here, or connect your source to a selected ROM."));
            }
            if let Some(id)=lab.draft{
                if let Some(d)=self.progress.notebook.drafts.iter().find(|d|d.id==id){
                    if ui.button(trf!("Kodu düzenle: {}", "Edit code: {}",d.name)).clicked(){self.progress.notebook.select(id);self.world_code=true;}
                }
            }
            if ui.button(tr!("Dünya programları / kod araçları", "World programs / code tools")).clicked(){self.world_code=true;}
            self.world_code_actions(ui);
        });
    }

    pub(super) fn world_code_actions(&mut self, ui: &mut Ui) {
        let Some(w) = &self.world else { return };
        let name = w.world.name.clone();
        let lab = self
            .progress
            .notebook
            .labs
            .get(&name)
            .cloned()
            .unwrap_or_default();
        if let Some(id) = lab.rom {
            let present = w.world.part(id).is_some();
            ui.label(trf!("Program ROM'u: #{id}", "Program ROM: #{id}"));
            if !present {
                ui.colored_label(
                    theme::WARN,
                    tr!(
                        "Bağlı ROM silinmiş; hedefi yeniden seç.",
                        "Linked ROM was deleted; select a target again."
                    ),
                );
            }
        }
        ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(self.session.is_none(),egui::Button::new(tr!("Seçili ROM'u bu taslağa bağla", "Link selected ROM to this draft"))).clicked(){
                if let Err(e)=self.bind_selected_rom(){self.status=Some(e);}
            }
            if ui.add_enabled(self.session.is_none()&&lab.rom.is_some(),egui::Button::new(tr!("ROM'a yükle ve baştan başlat", "Upload ROM and restart"))).on_hover_text(tr!("Derlenmiş taslağı aynı devreye yazar. Geçici kayıt/RAM durumu sıfırlanır. ROM değişikliği geri alınabilir.","Writes the compiled draft into this circuit. Transient registers/RAM are reset. The ROM edit can be undone.")).clicked(){
                if let Err(e)=self.upload_world_rom(){self.status=Some(e);}
            }
            if ui.button(tr!("Saat çevrimi", "Clock cycle")).clicked(){if let Some(w)=&mut self.world{w.sim.clock_cycle(&w.world);}}
        });
        if let Some(w) = &self.world {
            if let Some(rom) = lab.rom.and_then(|id| w.world.part(id)) {
                let words = match &rom.kind {
                    makine_world::Kind::Device(
                        makine_world::Device::Rom { words, .. }
                        | makine_world::Device::ProgramRom { words },
                    ) => Some(words),
                    _ => None,
                };
                let draft = self.progress.notebook.current();
                let fresh = words.is_some_and(|w| *w == lab.loaded_words)
                    && draft.is_some_and(|d| {
                        Some(d.id) == lab.draft && Some(&d.source) == lab.loaded_source.as_ref()
                    });
                if !fresh {
                    ui.colored_label(theme::WARN,tr!("ROM ve taslak henüz eşleşmiyor; yüklemeden önce derle. Devreyi değiştirmek ilerlemeyi tamamlamaz.","ROM and draft are not in sync; build before uploading. Editing the circuit does not complete a lesson."));
                }
            }
        }
    }

    fn bind_selected_rom(&mut self) -> Result<(), String> {
        if self.session.is_some() {
            return Err(tr!(
                "ROM bağlamak için ağ oturumundan ayrıl.",
                "Leave the network session before linking ROM."
            )
            .into());
        }
        let w = self
            .world
            .as_ref()
            .ok_or_else(|| "world missing".to_string())?;
        let selected: Vec<_> = w
            .selected
            .iter()
            .filter(|&&id| {
                w.world.part(id).is_some_and(|p| {
                    matches!(
                        p.kind,
                        makine_world::Kind::Device(
                            makine_world::Device::Rom { .. }
                                | makine_world::Device::ProgramRom { .. }
                        )
                    )
                })
            })
            .copied()
            .collect();
        if selected.len() != 1 {
            return Err(tr!(
                "Dünya'da tam bir ROM veya Program ROM seç; sonra kod araçlarına dön.",
                "Select exactly one ROM or Program ROM in World, then return to code tools."
            )
            .into());
        }
        let draft = self
            .progress
            .notebook
            .current()
            .cloned()
            .ok_or_else(|| "draft missing".to_string())?;
        if draft.architecture == Architecture::RiscV {
            return Err(tr!("Bu ROM bağlantısı Mini/4004/özel ISA içindir. RV32 kodu bilgisayar simülatöründe çalışır, Dünya tellerine bağlı değildir.","This ROM link is for Mini/4004/custom ISAs. RV32 runs in the computer simulator, not World wires.").into());
        }
        let lab = self
            .progress
            .notebook
            .labs
            .entry(w.world.name.clone())
            .or_default();
        lab.rom = Some(selected[0]);
        lab.draft = Some(draft.id);
        lab.loaded_source = None;
        lab.loaded_words.clear();
        self.save_soon();
        Ok(())
    }

    pub(super) fn upload_world_rom(&mut self) -> Result<(), String> {
        use makine_world::{Device, Edit, Kind};
        if self.session.is_some() {
            return Err(tr!(
                "ROM yüklemek için ağ oturumundan ayrıl.",
                "Leave the network session before uploading ROM."
            )
            .into());
        }
        let draft = self
            .progress
            .notebook
            .current()
            .cloned()
            .ok_or_else(|| "draft missing".to_string())?;
        let (_, source, p) = self
            .debug
            .small_built
            .as_ref()
            .filter(|(id, s, _)| *id == draft.id && *s == draft.source)
            .ok_or_else(|| {
                tr!(
                    "Önce güncel taslağı başarıyla derle.",
                    "Build the current draft successfully first."
                )
                .to_string()
            })?;
        let w = self
            .world
            .as_mut()
            .ok_or_else(|| "world missing".to_string())?;
        let lab = self
            .progress
            .notebook
            .labs
            .get(&w.world.name)
            .cloned()
            .ok_or_else(|| tr!("Önce bir ROM bağla.", "Link a ROM first.").to_string())?;
        if lab.draft != Some(draft.id) {
            return Err(tr!(
                "Bu dünya başka taslağa bağlı; seçili ROM'u bu taslağa açıkça bağla.",
                "This world is linked to another draft; explicitly link its ROM to this draft."
            )
            .into());
        }
        let id = lab
            .rom
            .ok_or_else(|| tr!("Önce bir ROM bağla.", "Link a ROM first.").to_string())?;
        let mut part = w
            .world
            .part(id)
            .cloned()
            .ok_or_else(|| tr!("ROM artık yok.", "ROM no longer exists.").to_string())?;
        let words: Vec<u64> = p.words.iter().map(|&v| v as u64).collect();
        part.kind = match &part.kind {
            Kind::Device(Device::Rom { addr, data, .. })
                if draft.architecture != Architecture::Mcs4
                    && words.len() <= 1usize << *addr
                    && words.iter().all(|&v| v <= makine_sim::mask(*data)) =>
            {
                Kind::Device(Device::Rom {
                    addr: *addr,
                    data: *data,
                    words: words.clone(),
                })
            }
            Kind::Device(Device::ProgramRom { .. }) if draft.architecture == Architecture::Mcs4 => {
                Kind::Device(Device::ProgramRom {
                    words: words.clone(),
                })
            }
            _ => {
                return Err(tr!(
                    "ROM türü, kapasitesi veya kelime genişliği bu programla eşleşmiyor.",
                    "ROM type, capacity or word width does not match this program."
                )
                .into())
            }
        };
        if !p.ram.is_empty() {
            return Err(tr!("Dünya'ya yükleme .ram başlangıcını henüz taşımıyor; RAM'i devrede kur veya .ram kullanmadan bir program yaz. Hiçbir değişiklik yapılmadı.","World upload does not transfer .ram initialization; initialize RAM in the circuit or use a program without .ram. No changes were made.").into());
        }
        w.edit(Edit::Part {
            id,
            part: Some(part),
        });
        w.sim.running = false;
        w.sim.reset();
        let lab = self.progress.notebook.labs.get_mut(&w.world.name).unwrap();
        lab.loaded_words = words;
        lab.loaded_source = Some(source.clone());
        self.store_world();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Progress;
    use makine_world::{Device, Edit, Kind, Placed, Pos, World};

    fn solved_app() -> App {
        let mut p = Progress::default();
        for l in makine_levels::levels() {
            p.solutions
                .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
        }
        App::new(p, None)
    }

    #[test]
    fn activities_open_inside_their_level_and_reuse_one_draft() {
        let mut app = solved_app();
        let a = Activity::Program("address");
        app.open_activity(a);
        assert_eq!(app.level.chip, "Ram8");
        assert!(app.editor_code && app.view == View::Editor);
        let first = app.progress.notebook.selected;
        app.progress.notebook.current_mut().unwrap().source = "my work".into();
        app.editor_code = false;
        app.open_activity(a);
        assert_eq!(app.progress.notebook.selected, first);
        assert_eq!(app.progress.notebook.current().unwrap().source, "my work");
        // Locked levels refuse their activities.
        let mut fresh = App::new(Progress::default(), None);
        fresh.open_activity(Activity::Program("project"));
        assert!(!fresh.editor_code);
    }

    #[test]
    fn code_uploads_into_the_same_world_rom_undoably_and_rejects_bad_targets() {
        let mut app = solved_app();
        app.open_world();
        let mut w = World::new("lab");
        w.apply(Edit::Part {
            id: 7,
            part: Some(Placed {
                kind: Kind::Device(Device::Rom {
                    addr: 4,
                    data: 8,
                    words: vec![],
                }),
                pos: Pos::new(0.0, 0.0),
            }),
        });
        app.switch_to_new(w);
        app.open_mini_activity();
        let id = app.progress.notebook.selected;
        app.progress.notebook.current_mut().unwrap().source = "LDI 5\nHLT".into();
        // Nothing selected: refused.
        assert!(app.bind_selected_rom().is_err());
        app.world.as_mut().unwrap().selected.insert(7);
        app.bind_selected_rom().unwrap();
        // Not built yet: refused.
        assert!(app.upload_world_rom().is_err());
        let p = makine_rv::smallasm::assemble("LDI 5\nHLT", makine_rv::smallasm::Dialect::Mini)
            .unwrap();
        app.debug.small_built = Some((id, "LDI 5\nHLT".into(), p));
        app.upload_world_rom().unwrap();
        let rom = |a: &App| match &a.world.as_ref().unwrap().world.part(7).unwrap().kind {
            Kind::Device(Device::Rom { words, .. }) => words.clone(),
            _ => panic!(),
        };
        assert_eq!(rom(&app), vec![0x15, 0xF0]);
        assert!(app.progress.worlds["lab"].part(7).is_some());
        app.world.as_mut().unwrap().undo();
        assert!(rom(&app).is_empty(), "ROM upload is undoable");
        // A program too large for the ROM is refused without changes.
        let big = vec!["NOP"; 17].join("\n");
        let p = makine_rv::smallasm::assemble(
            &big,
            makine_rv::smallasm::Dialect::Custom(&crate::isa_lessons::definition()),
        );
        let mut d = crate::isa_lessons::definition();
        d.bits = 8;
        if let Ok(p) = p {
            app.progress.notebook.current_mut().unwrap().source = big.clone();
            app.debug.small_built = Some((id, big, p));
            assert!(app.upload_world_rom().is_err());
            assert!(rom(&app).is_empty());
        }
        let _ = d;
    }
}

#[cfg(test)]
mod shots {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    #[ignore]
    fn journey_screenshots() {
        let dir = std::env::var("MAKINE_SHOT_DIR").unwrap_or("/tmp/opencode".into());
        std::fs::create_dir_all(&dir).unwrap();
        for lang in makine_lang::Lang::ALL {
            makine_lang::set_for_thread(Some(lang));
            let mut p = crate::store::Progress::default();
            for l in makine_levels::levels()
                .iter()
                .take_while(|l| l.chip != "MiniCpu")
            {
                p.solutions
                    .insert(l.chip.into(), makine_levels::reference::reference(l.chip));
            }
            let mut app = App::new(p, None);
            app.open_level(makine_levels::find("Ram8").unwrap());
            let app = Rc::new(RefCell::new(app));
            let a = app.clone();
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1440., 900.))
                .wgpu()
                .build_ui(move |ui| a.borrow_mut().ui(ui));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/journey-level-{}.png", lang.code()))
                .unwrap();
            app.borrow_mut().open_activity(Activity::Program("address"));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/journey-code-{}.png", lang.code()))
                .unwrap();
            {
                let mut a = app.borrow_mut();
                let e = makine_world::projects::examples::all()
                    .into_iter()
                    .find(|e| e.id == "ram_bench")
                    .unwrap();
                a.open_project((e.build)());
                a.attach_lab("Ram8", None, None);
                a.open_mini_activity();
            }
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(format!("{dir}/journey-world-{}.png", lang.code()))
                .unwrap();
            makine_lang::set_for_thread(None);
        }
    }
}
