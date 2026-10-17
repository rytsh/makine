//! The world view's side of the app: the list of worlds, world files,
//! building together on the network, and the world panel.

use egui::{RichText, Ui};
use makine_levels::levels;
use makine_sim::PartKind;

use super::{App, View};
use crate::design::Design;
use crate::sandbox;

impl App {
    pub(super) fn open_mcs4_project(&mut self) {
        if self.session.is_some() {
            return;
        }
        self.store_world();
        let mut p = makine_world::projects::mcs4::project(makine_world::projects::mcs4::DEMO);
        p.world.name = crate::worlds::unique_name(&p.world.name, &self.progress.worlds);
        self.switch_world(p.world);
        if let Some(w) = &mut self.world {
            w.collapsed = p.collapsed.into_iter().collect();
            w.sim.running = false;
            for end in p.watches {
                w.sim.watch(end);
            }
        }
        self.view = View::World;
    }

    pub(super) fn open_world(&mut self) {
        if self.world.is_none() {
            let p = &self.progress;
            let world = p
                .last_world
                .as_ref()
                .and_then(|n| p.worlds.get(n))
                .or_else(|| p.worlds.values().next())
                .cloned()
                .unwrap_or_else(|| makine_world::World::new("dünya"));
            self.switch_world(world);
        }
        self.view = View::World;
    }

    /// Show `world` in the world view (the current one is already saved).
    pub(super) fn switch_world(&mut self, world: makine_world::World) {
        self.progress.last_world = Some(world.name.clone());
        self.world_rename = world.name.clone();
        let mut v = crate::worldview::WorldView::new(world);
        v.set_library(self.lib.clone());
        self.world = Some(v);
        self.store_world();
    }

    /// Move edits between the world view and the network. Runs every
    /// frame, whatever view is shown, so other players are never kept
    /// waiting.
    pub(super) fn net_tick(&mut self) {
        let (Some(s), Some(w)) = (&mut self.session, &mut self.world) else {
            return;
        };
        let rev = w.world.revision();
        let (closed, new_chips) = s.tick(w, &self.lib, &mut self.progress.sandbox);
        let changed = w.world.revision() != rev;
        if changed {
            self.store_world();
        }
        if new_chips {
            self.refresh_library();
            self.save();
        }
        if let Some(reason) = closed {
            self.leave_session(Some(reason));
        }
    }

    pub(super) fn leave_session(&mut self, reason: Option<String>) {
        let Some(s) = self.session.take() else { return };
        if let Some(w) = &mut self.world {
            w.others.clear();
            w.sharing = false;
            w.outbox.clear();
        }
        let msg = reason.unwrap_or_else(|| "Paylaşım bitti.".into());
        if s.is_host() {
            self.status = Some(msg);
            return;
        }
        // A guest goes back to their own last world.
        self.status = Some(format!("{msg} Kendi dünyana döndün."));
        self.world = None;
        self.open_world();
    }

    /// Hosting and joining.
    pub(super) fn network_section(&mut self, ui: &mut Ui) {
        let title = match &self.session {
            Some(s) => format!("🔗 Birlikte kur ({} kişi)", s.players().len().max(1)),
            None => "🔗 Birlikte kur".into(),
        };
        egui::CollapsingHeader::new(RichText::new(title).strong())
            .id_salt("network")
            .show(ui, |ui| {
                let Some(s) = &self.session else {
                    ui.label(
                        RichText::new(
                            "Aynı ağdaki (aynı Wi-Fi) arkadaşlarınla aynı dünyada birlikte kur.",
                        )
                        .weak()
                        .small(),
                    );
                    ui.horizontal(|ui| {
                        let l = ui.label("Adın").id;
                        ui.add(egui::TextEdit::singleline(&mut self.net_name).desired_width(120.0))
                            .labelled_by(l);
                    });
                    if ui
                        .button("📡 Bu dünyayı paylaşıma aç")
                        .on_hover_text("Arkadaşların bu dünyaya bağlanabilir")
                        .clicked()
                    {
                        match crate::session::Session::host(&self.net_name, makine_net::PORT) {
                            Ok(s) => self.session = Some(s),
                            Err(e) => {
                                self.status = Some(format!(
                                    "Paylaşım açılamadı ({e}). Başka bir makine zaten açık olabilir."
                                ))
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        let l = ui.label("Adres").id;
                        ui.add(
                            egui::TextEdit::singleline(&mut self.net_addr)
                                .hint_text("192.168.1.20")
                                .desired_width(120.0),
                        )
                        .labelled_by(l);
                        if ui.button("Bağlan").clicked() {
                            // Save our own world before showing theirs.
                            self.store_world();
                            match crate::session::Session::join(&self.net_addr, &self.net_name) {
                                Ok(s) => self.session = Some(s),
                                Err(e) => {
                                    self.status = Some(format!(
                                        "Bağlanılamadı: {e}. Adresi ve aynı ağda olduğunuzu kontrol et."
                                    ))
                                }
                            }
                        }
                    });
                    return;
                };
                ui.label(RichText::new(&s.status).small());
                let me = s.my_id();
                for p in s.players() {
                    let tag = match (p.id == makine_net::HOST_ID, Some(p.id) == me) {
                        (true, true) => " (sen, ev sahibi)",
                        (true, false) => " (ev sahibi)",
                        (false, true) => " (sen)",
                        _ => "",
                    };
                    ui.colored_label(crate::session::color(p.id), format!("● {}{tag}", p.name));
                }
                let label = if s.is_host() { "Paylaşımı kapat" } else { "Ayrıl" };
                if ui.button(label).clicked() {
                    self.leave_session(None);
                }
            });
    }

    pub(super) fn worlds_section(&mut self, ui: &mut Ui) {
        let current = self.world.as_ref().map(|w| w.world.name.clone());
        let n = self.progress.worlds.len();
        let mut open = None;
        egui::CollapsingHeader::new(RichText::new(format!("Dünyalar ({n})")).strong())
            .id_salt("worlds")
            .show(ui, |ui| {
                for name in self.progress.worlds.keys() {
                    let me = current.as_deref() == Some(name.as_str());
                    if ui.selectable_label(me, format!("🌍 {name}")).clicked() && !me {
                        open = Some(name.clone());
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("+ Yeni").clicked() {
                        let name = crate::worlds::unique_name("dünya", &self.progress.worlds);
                        self.switch_to_new(makine_world::World::new(name));
                    }
                    if ui.button("Kopyasını al").clicked() {
                        if let Some(w) = &self.world {
                            let mut copy = w.world.clone();
                            copy.name =
                                crate::worlds::unique_name(&w.world.name, &self.progress.worlds);
                            self.switch_to_new(copy);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let l = ui.label("Ad").id;
                    ui.add(egui::TextEdit::singleline(&mut self.world_rename).desired_width(120.0))
                        .labelled_by(l);
                    if ui.button("Değiştir").clicked() {
                        self.rename_world();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("📤 Dosyaya kaydet…").clicked() {
                        self.export_world();
                    }
                    if ui.button("📥 Dosyadan aç…").clicked() {
                        self.import_world();
                    }
                });
                let can_delete = n > 1;
                let r = ui.add_enabled(can_delete, egui::Button::new("🗑 Bu dünyayı sil"));
                let r = if can_delete {
                    r
                } else {
                    r.on_disabled_hover_text("Son dünya silinemez")
                };
                if r.clicked() {
                    self.confirm_delete_world = true;
                }
                if self.confirm_delete_world {
                    ui.horizontal(|ui| {
                        ui.colored_label(crate::theme::BAD, "Emin misin?");
                        if ui.button("Evet, sil").clicked() {
                            self.delete_world();
                        }
                        if ui.button("Vazgeç").clicked() {
                            self.confirm_delete_world = false;
                        }
                    });
                }
            });
        if let Some(name) = open {
            if let Some(w) = self.progress.worlds.get(&name).cloned() {
                self.switch_world(w);
            }
        }
    }

    pub(super) fn switch_to_new(&mut self, world: makine_world::World) {
        self.progress
            .worlds
            .insert(world.name.clone(), world.clone());
        self.switch_world(world);
    }

    pub(super) fn rename_world(&mut self) {
        let to = self.world_rename.trim().to_string();
        let Some(w) = &mut self.world else { return };
        let from = w.world.name.clone();
        if to == from {
            return;
        }
        let err = crate::worlds::check_name(&to).err().or_else(|| {
            self.progress
                .worlds
                .contains_key(&to)
                .then(|| format!("'{to}' adında bir dünya var"))
        });
        if let Some(e) = err {
            w.status = Some(e);
            return;
        }
        self.progress.worlds.remove(&from);
        w.world.name = to.clone();
        self.progress.last_world = Some(to);
        self.store_world();
    }

    pub(super) fn delete_world(&mut self) {
        self.confirm_delete_world = false;
        let Some(w) = &self.world else { return };
        let name = w.world.name.clone();
        if self.progress.worlds.len() < 2 {
            return;
        }
        self.progress.worlds.remove(&name);
        let next = self.progress.worlds.values().next().cloned().unwrap();
        self.world = None;
        self.switch_world(next);
    }

    pub(super) fn export_world(&mut self) {
        let Some(w) = &self.world else { return };
        let text = match crate::worlds::export(&w.world, &self.lib, &self.progress.sandbox) {
            Ok(t) => t,
            Err(e) => {
                self.status = Some(e);
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("makine dünyası", &[crate::worlds::EXT])
            .set_file_name(format!("{}.{}", w.world.name, crate::worlds::EXT))
            .save_file()
        else {
            return;
        };
        self.status = Some(match std::fs::write(&path, text) {
            Ok(()) => format!("Kaydedildi: {}", path.display()),
            Err(e) => format!("{}: {e}", path.display()),
        });
    }

    pub(super) fn import_world(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("makine dünyası", &[crate::worlds::EXT])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|t| crate::worlds::import(&t, &self.lib, &mut self.progress.sandbox));
        match result {
            Ok(mut world) => {
                world.name = crate::worlds::unique_name(&world.name, &self.progress.worlds);
                self.refresh_library();
                self.status = Some(format!("'{}' açıldı.", world.name));
                self.switch_to_new(world);
            }
            Err(e) => self.status = Some(e),
        }
    }

    /// Copy the open world into the progress, to be written to disk soon
    /// (edits come many per second while painting).
    pub(super) fn store_world(&mut self) {
        // A world joined over the network belongs to its host. (Until it
        // arrives, the view still shows the player's own world.)
        if self
            .session
            .as_ref()
            .is_some_and(|s| !s.is_host() && s.ready)
        {
            return;
        }
        if let Some(w) = &self.world {
            self.progress
                .worlds
                .insert(w.world.name.clone(), w.world.clone());
            self.save_soon();
        }
    }

    /// Right panel in the world view: parts, simulation, the selection.
    pub(super) fn world_panel(&mut self, ui: &mut Ui) {
        if self.world.is_none() {
            return;
        }
        let title = self.world.as_ref().unwrap().world.name.clone();
        ui.label(crate::theme::strong(title, 22.0));
        ui.add_space(4.0);
        // Switching worlds mid-session would leave the others behind.
        if self.session.is_none() {
            self.worlds_section(ui);
        }
        self.network_section(ui);
        if self.session.is_none() {
            egui::CollapsingHeader::new("Örnek projeler")
                .id_salt("workshop-projects")
                .show(ui, |ui| {
                    ui.label(RichText::new("Mevcut dünyan korunur; örnek ayrı bir dünya olarak açılır.").weak().small());
                    if ui.button("4004 atölyesi aç").clicked() { self.open_mcs4_project(); }
                    ui.label(RichText::new("Kapılardan aritmetik ve kontrol çekirdeği; özgün MCS-4 bellek/GÇ sistemi henüz yok.").weak().small());
                });
        }
        let Some(w) = &mut self.world else { return };
        egui::CollapsingHeader::new(RichText::new("⌨ Kısayollar").strong())
            .id_salt("world-shortcuts")
            .show(ui, |ui| {
                egui::Grid::new("world-shortcut-grid")
                    .num_columns(2)
                    .spacing([10.0, 2.0])
                    .show(ui, |ui| {
                        for (keys, what) in crate::worldview::SHORTCUTS {
                            ui.label(RichText::new(*keys).monospace().small().strong());
                            ui.label(RichText::new(*what).small());
                            ui.end_row();
                        }
                    });
            });
        ui.separator();

        // What is selected comes first: it is what the player works on.
        w.symbols = self.progress.symbols;
        w.module_panel(ui);
        w.selection_panel(ui);
        let watched = w.sim.traces.len();
        egui::CollapsingHeader::new(format!("Sinyal izleme ({watched})"))
            .id_salt("world-scope")
            .show(ui, |ui| w.scope_panel(ui));

        crate::theme::section(ui, "Parçalar");
        w.palette_ui(ui, crate::worldview::palette());
        crate::theme::section(ui, "Cihazlar");
        w.palette_ui(ui, crate::worldview::device_palette());
        // Chips from the library, as parts.
        let mut chips: Vec<(String, String)> = levels()
            .iter()
            .filter(|l| self.progress.solved(l.chip))
            .map(|l| (l.chip.to_string(), l.title.to_string()))
            .collect();
        chips.extend(
            self.progress
                .sandbox
                .chips
                .keys()
                .map(|n| (n.clone(), "Senin çipin".to_string())),
        );
        if !chips.is_empty() {
            egui::CollapsingHeader::new(
                RichText::new(format!("Çiplerin ({})", chips.len())).strong(),
            )
            .id_salt("world-chips")
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Çözdüğün seviyeler ve serbest çiplerin. Sürükle ya da tıkla.")
                        .weak()
                        .small(),
                );
                ui.horizontal_wrapped(|ui| {
                    for (name, tip) in &chips {
                        let kind = makine_world::Kind::Part(makine_sim::Part::new(PartKind::Chip(
                            name.clone(),
                        )));
                        let selected = w.placing.as_ref() == Some(&kind);
                        let b = egui::Button::new(name.as_str())
                            .selected(selected)
                            .sense(egui::Sense::click_and_drag());
                        let r = ui.add(b);
                        let r = crate::worldview::draggable(ui, r, &kind).on_hover_text(tip);
                        if r.clicked() {
                            w.placing = if selected { None } else { Some(kind) };
                        }
                    }
                });
            });
        }
        if w.placing.is_some() {
            ui.label(
                RichText::new("Koymak için tahtaya tıkla. Bırakmak için Esc.")
                    .color(crate::theme::COPPER_BRIGHT)
                    .small(),
            );
        }

        crate::theme::section(ui, "Simülasyon");
        ui.horizontal(|ui| {
            let label = if w.sim.running {
                "⏸ Durdur"
            } else {
                "▶ Çalıştır"
            };
            if ui
                .button(label)
                .on_hover_text("Saatleri durdur/başlat (P)")
                .clicked()
            {
                w.sim.running = !w.sim.running;
            }
            if ui
                .add_enabled(w.can_undo(), egui::Button::new("↶ Geri al"))
                .clicked()
            {
                w.undo();
            }
            if ui
                .add_enabled(w.can_redo(), egui::Button::new("↷ Yinele"))
                .clicked()
            {
                w.redo();
            }
            if ui
                .button("⌖ Ortala")
                .on_hover_text("Bütün dünyayı göster (Home)")
                .clicked()
            {
                w.refit();
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button("Saat çevrimi")
                .on_hover_text("Dur ve ilk saatin bir sonraki yükselen kenarına kadar ilerle")
                .clicked()
                && !w.sim.clock_cycle(&w.world)
            {
                w.status = Some("Önce bir Saat koy.".into());
            }
            if ui.button("Baştan").clicked() {
                w.sim.reset();
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!w.sim.running, egui::Button::new("⏭ Bir adım"))
                .on_hover_text("Durmuşken saati bir adım ilerlet (N)")
                .clicked()
            {
                w.sim.single_step();
            }
            let l = ui
                .label("Hız")
                .on_hover_text("Saat her 'periyot' adımda bir değişir; hız, karedeki adım sayısı")
                .id;
            let name = |v: u32| match v {
                0 => "En hızlı".to_string(),
                v => format!("{v} adım/kare"),
            };
            egui::ComboBox::from_id_salt("world-speed")
                .selected_text(name(w.sim.speed))
                .show_ui(ui, |ui| {
                    for v in [1, 10, 100, 1000, 0] {
                        ui.selectable_value(&mut w.sim.speed, v, name(v));
                    }
                })
                .response
                .labelled_by(l);
        });
        ui.label(
            RichText::new(format!(
                "{} parça · adım {} · {}",
                w.world.len(),
                w.sim.tick,
                if w.sim.running {
                    format!("{:.0} adım/sn", w.sim.rate)
                } else {
                    "durdu".into()
                }
            ))
            .weak(),
        );
        if let Some(e) = w.sim.error() {
            ui.colored_label(crate::theme::BAD, e.to_string());
        }
        if let Some(c) = w.sim.compiled() {
            for p in c.problems.iter().take(3) {
                ui.colored_label(crate::theme::WARN, p.to_string());
            }
        }
        if let Some(e) = &w.status {
            ui.colored_label(crate::theme::WARN, e);
        }
        if let Some(req) = w.request.take() {
            self.handle_world_request(req);
        }
    }

    pub(super) fn handle_world_request(&mut self, req: crate::worldview::Request) {
        let crate::worldview::Request::MakeChip { name, parts } = req;
        let Some(w) = &mut self.world else { return };
        if let Err(e) = sandbox::check_name(&name, &self.lib, &self.progress.sandbox, None) {
            w.status = Some(e);
            return;
        }
        match makine_world::chip::to_chip(&w.world, &parts, &name, &self.lib) {
            Ok(def) => {
                let n = def.parts.len();
                let mut d = Design::new(def);
                // Lay the parts out in a grid so the editor is usable.
                for (i, p) in d.pos.iter_mut().enumerate() {
                    *p = [200.0 + 90.0 * (i % 8) as f32, 60.0 + 70.0 * (i / 8) as f32];
                }
                self.progress.sandbox.chips.insert(name.clone(), d);
                self.refresh_library();
                if let Some(w) = &mut self.world {
                    w.set_library(self.lib.clone());
                    w.chip_name.clear();
                    w.status = Some(format!(
                        "'{name}' çipi oluşturuldu ({n} parça). Serbest çiplerde ve 'Çiplerin' \
                         listesinde."
                    ));
                }
                self.save();
            }
            Err(e) => w.status = Some(e),
        }
    }
}
