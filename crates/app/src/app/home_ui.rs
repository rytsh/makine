//! The home page: what makine is, where the player got to, and their
//! profiles (switch, new, rename, copy, paste, start over, delete).

use egui::{vec2, RichText, Ui};
use makine_lang::{tr, trf};

use super::{App, View};
use crate::profiles::{self, Profiles};
use crate::store::Progress;
use crate::theme;

/// Home page state that does not need saving.
#[derive(Default)]
pub(super) struct Home {
    new_name: String,
    rename: Option<(usize, String)>,
    pub paste: String,
    /// An action waiting for "yes".
    confirm: Option<Confirm>,
    message: Option<(bool, String)>,
    /// Summaries of the other profiles, by file: reading them every frame
    /// would load whole progress files.
    summaries: std::collections::HashMap<String, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Confirm {
    Reset,
    Delete(usize),
    Replace,
}

pub(super) fn logo() -> egui::ImageSource<'static> {
    egui::include_image!("../../../../assets/logo.png")
}

impl App {
    /// The side panel on the home page: the parts of the game.
    pub(super) fn home_panel(&mut self, ui: &mut Ui) {
        theme::section(ui, tr!("Bölümler", "Sections"));
        let all = makine_levels::levels();
        let mut start = 0;
        while start < all.len() {
            let section = all[start].section;
            let section_name = all[start].section_name();
            let end = start
                + all[start..]
                    .iter()
                    .take_while(|l| l.section == section)
                    .count();
            let done = all[start..end]
                .iter()
                .filter(|l| self.progress.solved(l.chip))
                .count();
            ui.horizontal(|ui| {
                ui.label(section_name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let col = if done == end - start {
                        theme::GOOD
                    } else {
                        theme::INK_DIM
                    };
                    ui.colored_label(col, format!("{done}/{}", end - start));
                });
            });
            start = end;
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(tr!(
                "Her seviye bir çip kurdurur; testi geçince çip kütüphanene eklenir ve \
                 sonraki seviyelerde parça olarak kullanılır. Son durak, Doom çalıştıran \
                 RISC-V işlemci.",
                "Each level has you build a chip; once it passes the test, the chip joins \
                 your library and becomes a part in later levels. The last stop is a \
                 RISC-V CPU that runs Doom."
            ))
            .small()
            .color(theme::INK_DIM),
        );
    }

    pub(super) fn go_home(&mut self) {
        self.store_world();
        self.save();
        self.view = View::Home;
    }

    /// Switch to profile `i`: save this one and load that one, as if the
    /// app had just been started with it.
    pub(super) fn switch_profile(&mut self, i: usize) {
        self.store_world();
        self.save();
        let Some(mut list) = self.profiles.take() else {
            return;
        };
        list.current = i;
        let _ = list.save();
        let progress = list.progress(i);
        self.restart(progress, Some(list));
    }

    /// Start over with `progress`, keeping the window and the profiles.
    fn restart(&mut self, progress: Progress, profiles: Option<Profiles>) {
        let mut home = std::mem::take(&mut self.home);
        // The profile left was just saved; its summary may have changed.
        home.summaries.clear();
        let mut fresh = App::with_profiles(progress, profiles);
        fresh.wiki_installed = self.wiki_installed;
        fresh.home = home;
        fresh.view = View::Home;
        *self = fresh;
    }

    pub(super) fn home_view(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(640.0);
                ui.add(egui::Image::new(logo()).fit_to_exact_size(vec2(112.0, 112.0)));
                ui.add_space(6.0);
                ui.label(RichText::new("makine").size(34.0).strong());
                ui.label(
                    RichText::new(tr!(
                        "NAND kapısından çalışan bir bilgisayara",
                        "From a NAND gate to a working computer"
                    ))
                    .size(16.0)
                    .color(theme::INK_DIM),
                );
            });
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(640.0);
                self.continue_card(ui);
                ui.add_space(14.0);
                self.profiles_card(ui);
            });
        });
    }

    fn continue_card(&mut self, ui: &mut Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    let all = makine_levels::levels();
                    let done = all.iter().filter(|l| self.progress.solved(l.chip)).count();
                    ui.label(
                        RichText::new(trf!(
                            "{} · {}/{} seviye",
                            "{} · {}/{} levels",
                            self.profiles
                                .as_ref()
                                .map_or(tr!("Profil", "Profile"), |p| p.current().name.as_str()),
                            done,
                            all.len()
                        ))
                        .strong(),
                    );
                    ui.add(
                        egui::ProgressBar::new(done as f32 / all.len() as f32)
                            .desired_height(8.0)
                            .fill(theme::COPPER),
                    );
                    ui.add_space(6.0);
                    let next = all
                        .iter()
                        .enumerate()
                        .find(|(i, l)| self.progress.unlocked(*i) && !self.progress.solved(l.chip))
                        .map(|(_, l)| *l);
                    ui.horizontal_wrapped(|ui| {
                        let label = match next {
                            Some(l) if done == 0 => trf!("▶  Başla: {}", "▶  Start: {}", l.name()),
                            Some(l) => trf!("▶  Devam et: {}", "▶  Continue: {}", l.name()),
                            None => tr!("▶  Seviyeler", "▶  Levels").into(),
                        };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(label).size(15.0))
                                    .min_size(vec2(0.0, 30.0)),
                            )
                            .clicked()
                        {
                            self.view = View::Editor;
                            if let Some(l) = next {
                                self.open_level(l);
                            }
                        }
                        if ui.button("📖  Wiki").clicked() {
                            self.view = View::Wiki;
                        }
                        if ui.button(tr!("🌍  Dünya", "🌍  World")).clicked() {
                            self.open_world();
                        }
                        if self.progress.solved("Cpu")
                            && ui.button(tr!("🖥  Bilgisayar", "🖥  Computer")).clicked()
                        {
                            self.view = View::Computer;
                        }
                    });
                });
            });
    }

    fn profiles_card(&mut self, ui: &mut Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    theme::section(ui, tr!("Profiller", "Profiles"));
                    if self.profiles.is_none() {
                        ui.colored_label(
                            theme::WARN,
                            tr!("Kayıt klasörü bulunamadı: ilerleme kaydedilmiyor.", "No folder to save in: progress is not being saved."),
                        );
                        return;
                    }
                    if self.session.is_some() {
                        ui.colored_label(
                            theme::WARN,
                            tr!("Birlikte kurarken profil değiştirilemez; önce oturumdan çık.", "Profiles cannot change while building together; leave the session first."),
                        );
                        return;
                    }
                    self.profile_list(ui);
                    ui.add_space(8.0);
                    self.profile_new(ui);
                    ui.add_space(8.0);
                    self.profile_copy(ui);
                    ui.add_space(8.0);
                    self.profile_danger(ui);
                    if let Some((ok, m)) = &self.home.message {
                        ui.add_space(6.0);
                        ui.colored_label(if *ok { theme::GOOD } else { theme::BAD }, m);
                    }
                });
            });
    }

    fn profile_list(&mut self, ui: &mut Ui) {
        let Some(list) = &self.profiles else { return };
        let rows: Vec<(String, bool)> = list
            .list
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.clone(), i == list.current))
            .collect();
        let mut switch = None;
        let mut rename = None;
        egui::Grid::new("profiles")
            .num_columns(3)
            .spacing(vec2(12.0, 6.0))
            .show(ui, |ui| {
                for (i, (name, current)) in rows.into_iter().enumerate() {
                    if let Some((at, text)) = &mut self.home.rename {
                        if *at == i {
                            let r = ui.add(egui::TextEdit::singleline(text).desired_width(160.0));
                            if r.lost_focus() || ui.button(tr!("Tamam", "OK")).clicked() {
                                rename = Some((i, text.clone()));
                            }
                            ui.label("");
                            ui.end_row();
                            continue;
                        }
                    }
                    let label = if current {
                        RichText::new(format!("● {name}"))
                            .strong()
                            .color(theme::COPPER_BRIGHT)
                    } else {
                        RichText::new(format!("○ {name}"))
                    };
                    ui.label(label);
                    let summary = if current {
                        profiles::summary(&self.progress)
                    } else {
                        let Some(l) = &self.profiles else { continue };
                        let file = l.list[i].file.clone();
                        self.home
                            .summaries
                            .entry(file)
                            .or_insert_with(|| profiles::summary(&l.progress(i)))
                            .clone()
                    };
                    ui.label(RichText::new(summary).color(theme::INK_DIM).small());
                    ui.horizontal(|ui| {
                        if !current && ui.button(tr!("Geç", "Switch")).clicked() {
                            switch = Some(i);
                        }
                        if ui
                            .small_button("✏")
                            .on_hover_text(tr!("Adını değiştir", "Rename"))
                            .clicked()
                        {
                            self.home.rename = Some((i, name.clone()));
                        }
                        if self.profiles.as_ref().is_some_and(|l| l.list.len() > 1)
                            && ui
                                .small_button("🗑")
                                .on_hover_text(tr!("Profili sil", "Delete profile"))
                                .clicked()
                        {
                            self.home.confirm = Some(Confirm::Delete(i));
                        }
                    });
                    ui.end_row();
                }
            });
        if let Some((i, name)) = rename {
            self.home.rename = None;
            if let Some(l) = &mut self.profiles {
                self.home.message = l.rename(i, &name).err().map(|e| (false, e));
            }
        }
        if let Some(i) = switch {
            self.switch_profile(i);
            self.home.message = Some((
                true,
                trf!(
                    "'{}' profiline geçildi.",
                    "Switched to profile '{}'.",
                    self.profiles
                        .as_ref()
                        .map_or("", |p| p.current().name.as_str())
                ),
            ));
        }
    }

    fn profile_new(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let l = ui.label(tr!("Yeni profil", "New profile")).id;
            ui.add(egui::TextEdit::singleline(&mut self.home.new_name).desired_width(160.0))
                .labelled_by(l);
            let name = self.home.new_name.clone();
            if ui.button(tr!("Boş başla", "Start empty")).clicked() {
                self.add_profile(&name, Progress::default());
            }
            if ui
                .button(tr!("Bunun kopyası", "Copy of this one"))
                .on_hover_text(tr!(
                    "Şu anki profilin bütün ilerlemesiyle yeni bir profil",
                    "A new profile with all of this profile's progress"
                ))
                .clicked()
            {
                self.store_world();
                let p = self.progress.clone();
                self.add_profile(&name, p);
            }
        });
    }

    pub(super) fn add_profile(&mut self, name: &str, progress: Progress) {
        let Some(list) = &mut self.profiles else {
            return;
        };
        match list.add(name, &progress) {
            Ok(i) => {
                self.home.new_name.clear();
                self.switch_profile(i);
                self.home.message = Some((
                    true,
                    trf!("'{}' profili açıldı.", "Profile '{}' created.", name.trim()),
                ));
            }
            Err(e) => self.home.message = Some((false, e)),
        }
    }

    fn profile_copy(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(tr!(
                "Profili başka bir bilgisayara taşımak için kopyala; orada yapıştır. \
                 Metin bütün seviyeleri, çipleri ve dünyaları içerir.",
                "To move a profile to another computer, copy it and paste it there. \
                 The text holds every level, chip and world."
            ))
            .small()
            .color(theme::INK_DIM),
        );
        ui.horizontal(|ui| {
            if ui
                .button(tr!("📋 Profili kopyala", "📋 Copy profile"))
                .clicked()
            {
                self.store_world();
                ui.ctx().copy_text(profiles::export(&self.progress));
                self.home.message = Some((
                    true,
                    tr!(
                        "Profil panoya kopyalandı.",
                        "Profile copied to the clipboard."
                    )
                    .into(),
                ));
            }
            if ui
                .button(tr!("💾 Dosyaya kaydet", "💾 Save to file"))
                .clicked()
            {
                self.store_world();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(tr!("makine profili", "makine profile"), &["json"])
                    .set_file_name("makine-profil.json")
                    .save_file()
                {
                    self.home.message = Some(match self.progress.save(&path) {
                        Ok(()) => (true, tr!("Profil kaydedildi.", "Profile saved.").into()),
                        Err(e) => (false, trf!("Kaydedilemedi: {e}", "Could not save: {e}")),
                    });
                }
            }
            if ui.button(tr!("📂 Dosyadan aç", "📂 Open file")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(tr!("makine profili", "makine profile"), &["json"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(t) => {
                            self.home.paste = t;
                            self.home.confirm = Some(Confirm::Replace);
                        }
                        Err(e) => {
                            self.home.message =
                                Some((false, trf!("Okunamadı: {e}", "Could not read: {e}")))
                        }
                    }
                }
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.home.paste)
                .hint_text(tr!(
                    "Kopyalanan profili buraya yapıştır",
                    "Paste a copied profile here"
                ))
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        if !self.home.paste.trim().is_empty() {
            ui.horizontal(|ui| {
                if ui
                    .button(tr!("Yeni profil olarak ekle", "Add as a new profile"))
                    .clicked()
                {
                    match profiles::import(&self.home.paste) {
                        Ok(p) => {
                            let name = if self.home.new_name.trim().is_empty() {
                                self.free_profile_name()
                            } else {
                                self.home.new_name.clone()
                            };
                            self.home.paste.clear();
                            self.add_profile(&name, p);
                        }
                        Err(e) => self.home.message = Some((false, e)),
                    }
                }
                if ui
                    .button(tr!("Bu profilin yerine koy", "Replace this profile"))
                    .clicked()
                {
                    self.home.confirm = Some(Confirm::Replace);
                }
            });
        }
    }

    fn free_profile_name(&self) -> String {
        (1..)
            .map(|n| trf!("Profil {n}", "Profile {n}"))
            .find(|n| {
                self.profiles
                    .as_ref()
                    .is_none_or(|p| p.check_name(n, None).is_ok())
            })
            .unwrap()
    }

    fn profile_danger(&mut self, ui: &mut Ui) {
        if ui
            .button(RichText::new(tr!("İlerlemeyi sıfırla", "Reset progress")).color(theme::BAD))
            .on_hover_text(tr!(
                "Bu profildeki bütün seviyeleri, çipleri ve dünyaları sil",
                "Delete every level, chip and world in this profile"
            ))
            .clicked()
        {
            self.home.confirm = Some(Confirm::Reset);
        }
        let Some(c) = self.home.confirm else { return };
        let name = self
            .profiles
            .as_ref()
            .map_or(String::new(), |p| p.current().name.clone());
        let text = match c {
            Confirm::Reset => trf!(
                "'{name}' profilindeki bütün ilerleme silinecek. Ayarlar kalır. Emin misin?",
                "All progress in profile '{name}' will be deleted. Settings stay. Are you sure?"
            ),
            Confirm::Delete(i) => trf!(
                "'{}' profili silinecek. Emin misin?",
                "Profile '{}' will be deleted. Are you sure?",
                self.profiles
                    .as_ref()
                    .and_then(|p| p.list.get(i))
                    .map_or("", |p| p.name.as_str())
            ),
            Confirm::Replace => trf!(
                "'{name}' profilinin ilerlemesi yapıştırılan profille değiştirilecek. Emin misin?",
                "The progress of profile '{name}' will be replaced with the pasted one. Are you sure?"
            ),
        };
        egui::Frame::new()
            .fill(theme::WELL)
            .inner_margin(10.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.colored_label(theme::WARN, text);
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new(tr!("Evet", "Yes")).color(theme::BAD))
                        .clicked()
                    {
                        self.home.confirm = None;
                        self.confirmed(c);
                    }
                    if ui.button(tr!("Vazgeç", "Cancel")).clicked() {
                        self.home.confirm = None;
                    }
                });
            });
    }

    pub(super) fn confirmed(&mut self, c: Confirm) {
        match c {
            Confirm::Reset => {
                let old = &self.progress;
                let fresh = Progress {
                    fast: old.fast,
                    symbols: old.symbols,
                    fullscreen: old.fullscreen,
                    custom_frame: old.custom_frame,
                    ..Progress::default()
                };
                let list = self.profiles.take();
                self.restart(fresh, list);
                self.save();
                self.home.message =
                    Some((true, tr!("İlerleme sıfırlandı.", "Progress reset.").into()));
            }
            Confirm::Delete(i) => {
                let Some(list) = &mut self.profiles else {
                    return;
                };
                let was = list.current;
                match list.remove(i) {
                    Ok(()) if i == was => {
                        // Load the one now current without saving the
                        // deleted one again.
                        let list = self.profiles.take().unwrap();
                        let progress = list.progress(list.current);
                        self.restart(progress, Some(list));
                        self.home.message =
                            Some((true, tr!("Profil silindi.", "Profile deleted.").into()));
                    }
                    Ok(()) => {
                        self.home.message =
                            Some((true, tr!("Profil silindi.", "Profile deleted.").into()))
                    }
                    Err(e) => self.home.message = Some((false, e)),
                }
            }
            Confirm::Replace => match profiles::import(&self.home.paste) {
                Ok(p) => {
                    self.home.paste.clear();
                    let list = self.profiles.take();
                    self.restart(p, list);
                    self.save();
                    self.home.message =
                        Some((true, tr!("Profil yüklendi.", "Profile loaded.").into()));
                }
                Err(e) => self.home.message = Some((false, e)),
            },
        }
    }
}
