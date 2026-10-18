//! The world view's side of the app: the list of worlds, world files,
//! building together on the network, and the world panel.

use egui::{RichText, Ui};
use makine_lang::{tr, trf};
use makine_levels::levels;
use makine_sim::PartKind;

use super::{App, View};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum WorldTab {
    #[default]
    Build,
    Parts,
    Signals,
    Worlds,
    Learn,
}
use crate::design::Design;
use crate::sandbox;

impl App {
    pub(super) fn open_mcs4_project(&mut self) {
        self.open_project(makine_world::projects::mcs4::project(
            makine_world::projects::mcs4::DEMO,
        ));
    }

    /// Open a ready-made project as a new world, keeping the current one.
    pub(super) fn open_project(&mut self, mut p: makine_world::projects::Project) {
        if self.session.is_some() {
            return;
        }
        self.store_world();
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
        self.world_code = false;
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
        let msg = reason.unwrap_or_else(|| tr!("Paylaşım bitti.", "Sharing ended.").into());
        if s.is_host() {
            self.status = Some(msg);
            return;
        }
        // A guest goes back to their own last world.
        self.status = Some(trf!(
            "{msg} Kendi dünyana döndün.",
            "{msg} You are back in your own world."
        ));
        self.world = None;
        self.open_world();
    }

    /// Hosting and joining.
    pub(super) fn network_section(&mut self, ui: &mut Ui) {
        let title = match &self.session {
            Some(s) => trf!(
                "🔗 Birlikte kur ({} kişi)",
                "🔗 Build together ({} people)",
                s.players().len().max(1)
            ),
            None => tr!("🔗 Birlikte kur", "🔗 Build together").into(),
        };
        egui::CollapsingHeader::new(RichText::new(title).strong())
            .id_salt("network")
            .show(ui, |ui| {
                let Some(s) = &self.session else {
                    ui.label(
                        RichText::new(
                            tr!(
                                "Aynı ağdaki (aynı Wi-Fi) arkadaşlarınla aynı dünyada birlikte kur.",
                                "Build in the same world with friends on the same network (same Wi-Fi)."
                            ),
                        )
                        .weak()
                        .small(),
                    );
                    ui.horizontal(|ui| {
                        let l = ui.label(tr!("Adın", "Your name")).id;
                        ui.add(egui::TextEdit::singleline(&mut self.net_name).desired_width(120.0))
                            .labelled_by(l);
                    });
                    if ui
                        .button(tr!("📡 Bu dünyayı paylaşıma aç", "📡 Share this world"))
                        .on_hover_text(tr!(
                            "Arkadaşların bu dünyaya bağlanabilir",
                            "Your friends can connect to this world"
                        ))
                        .clicked()
                    {
                        match crate::session::Session::host(&self.net_name, makine_net::PORT) {
                            Ok(s) => self.session = Some(s),
                            Err(e) => {
                                self.status = Some(trf!(
                                    "Paylaşım açılamadı ({e}). Başka bir makine zaten açık olabilir.",
                                    "Could not start sharing ({e}). Another makine may already be open."
                                ))
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        let l = ui.label(tr!("Adres", "Address")).id;
                        ui.add(
                            egui::TextEdit::singleline(&mut self.net_addr)
                                .hint_text("192.168.1.20")
                                .desired_width(120.0),
                        )
                        .labelled_by(l);
                        if ui.button(tr!("Bağlan", "Connect")).clicked() {
                            // Save our own world before showing theirs.
                            self.store_world();
                            match crate::session::Session::join(&self.net_addr, &self.net_name) {
                                Ok(s) => self.session = Some(s),
                                Err(e) => {
                                    self.status = Some(trf!(
                                        "Bağlanılamadı: {e}. Adresi ve aynı ağda olduğunuzu kontrol et.",
                                        "Could not connect: {e}. Check the address and that you are on the same network."
                                    ))
                                }
                            }
                        }
                    });
                    ui.separator();
                    ui.label(
                        RichText::new(tr!(
                            "İnternet üzerinden: ikiniz de aynı sunucuyu yazın. Paylaşımı açan bir \
                             oda kodu alır, öteki o kodla katılır. Sunucu için: makine-relay.",
                            "Over the internet: both of you enter the same server. The one who \
                             shares gets a room code, the other joins with it. For a server: \
                             makine-relay."
                        ))
                        .weak()
                        .small(),
                    );
                    ui.horizontal(|ui| {
                        let l = ui.label(tr!("Sunucu", "Server")).id;
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.progress.relay)
                                    .hint_text("relay.ornek.com")
                                    .desired_width(150.0),
                            )
                            .labelled_by(l)
                            .changed()
                        {
                            self.save_soon();
                        }
                    });
                    let relay = self.progress.relay.trim().to_string();
                    ui.horizontal(|ui| {
                        let l = ui.label(tr!("Oda kodu", "Room code")).id;
                        ui.add(
                            egui::TextEdit::singleline(&mut self.net_room)
                                .hint_text("k7qx2m")
                                .desired_width(80.0),
                        )
                        .labelled_by(l);
                        if ui
                            .add_enabled(
                                !relay.is_empty() && !self.net_room.trim().is_empty(),
                                egui::Button::new(tr!("Katıl", "Join")),
                            )
                            .clicked()
                        {
                            self.store_world();
                            match crate::session::Session::join_room(
                                &relay,
                                &self.net_room,
                                &self.net_name,
                            ) {
                                Ok(s) => self.session = Some(s),
                                Err(e) => {
                                    self.status = Some(trf!(
                                        "Katılınamadı: {e}",
                                        "Could not join: {e}"
                                    ))
                                }
                            }
                        }
                    });
                    if ui
                        .add_enabled(
                            !relay.is_empty(),
                            egui::Button::new(tr!(
                                "🌐 İnternette paylaş",
                                "🌐 Share on the internet"
                            )),
                        )
                        .clicked()
                    {
                        let started = crate::session::Session::host(&self.net_name, 0)
                            .and_then(|mut s| s.open_room(&relay).map(|_| s));
                        match started {
                            Ok(s) => self.session = Some(s),
                            Err(e) => {
                                self.status = Some(trf!(
                                    "Sunucuya bağlanılamadı: {e}",
                                    "Could not reach the server: {e}"
                                ))
                            }
                        }
                    }
                    return;
                };
                ui.label(RichText::new(&s.status).small());
                if let Some(code) = s.room() {
                    let code = code.to_string();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&code).monospace().size(20.0).strong());
                        if ui.small_button(tr!("📋 Kopyala", "📋 Copy")).clicked() {
                            ui.ctx().copy_text(code.clone());
                        }
                    });
                }
                let me = s.my_id();
                for p in s.players() {
                    let tag = match (p.id == makine_net::HOST_ID, Some(p.id) == me) {
                        (true, true) => tr!(" (sen, ev sahibi)", " (you, host)"),
                        (true, false) => tr!(" (ev sahibi)", " (host)"),
                        (false, true) => tr!(" (sen)", " (you)"),
                        _ => "",
                    };
                    ui.colored_label(crate::session::color(p.id), format!("● {}{tag}", p.name));
                }
                let label = if s.is_host() {
                    tr!("Paylaşımı kapat", "Stop sharing")
                } else {
                    tr!("Ayrıl", "Leave")
                };
                if ui.button(label).clicked() {
                    self.leave_session(None);
                }
            });
    }

    pub(super) fn worlds_section(&mut self, ui: &mut Ui) {
        let current = self.world.as_ref().map(|w| w.world.name.clone());
        let n = self.progress.worlds.len();
        let mut open = None;
        let title = trf!("Dünyalar ({n})", "Worlds ({n})");
        egui::CollapsingHeader::new(RichText::new(title).strong())
            .id_salt("worlds")
            .show(ui, |ui| {
                for name in self.progress.worlds.keys() {
                    let me = current.as_deref() == Some(name.as_str());
                    if ui.selectable_label(me, format!("🌍 {name}")).clicked() && !me {
                        open = Some(name.clone());
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr!("+ Yeni", "+ New")).clicked() {
                        let name = crate::worlds::unique_name("dünya", &self.progress.worlds);
                        self.switch_to_new(makine_world::World::new(name));
                    }
                    if ui.button(tr!("Kopyasını al", "Make a copy")).clicked() {
                        if let Some(w) = &self.world {
                            let mut copy = w.world.clone();
                            copy.name =
                                crate::worlds::unique_name(&w.world.name, &self.progress.worlds);
                            self.switch_to_new(copy);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let l = ui.label(tr!("Ad", "Name")).id;
                    ui.add(egui::TextEdit::singleline(&mut self.world_rename).desired_width(120.0))
                        .labelled_by(l);
                    if ui.button(tr!("Değiştir", "Rename")).clicked() {
                        self.rename_world();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(tr!("📤 Dosyaya kaydet…", "📤 Save to file…"))
                        .clicked()
                    {
                        self.export_world();
                    }
                    if ui
                        .button(tr!("📥 Dosyadan aç…", "📥 Open from file…"))
                        .clicked()
                    {
                        self.import_world();
                    }
                });
                let can_delete = n > 1;
                let r = ui.add_enabled(
                    can_delete,
                    egui::Button::new(tr!("🗑 Bu dünyayı sil", "🗑 Delete this world")),
                );
                let r = if can_delete {
                    r
                } else {
                    r.on_disabled_hover_text(tr!(
                        "Son dünya silinemez",
                        "The last world can't be deleted"
                    ))
                };
                if r.clicked() {
                    self.confirm_delete_world = true;
                }
                if self.confirm_delete_world {
                    ui.horizontal(|ui| {
                        ui.colored_label(crate::theme::BAD, tr!("Emin misin?", "Are you sure?"));
                        if ui.button(tr!("Evet, sil", "Yes, delete")).clicked() {
                            self.delete_world();
                        }
                        if ui.button(tr!("Vazgeç", "Cancel")).clicked() {
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
                .then(|| trf!("'{to}' adında bir dünya var", "a world named '{to}' exists"))
        });
        if let Some(e) = err {
            w.status = Some(e);
            return;
        }
        self.progress.worlds.remove(&from);
        if let Some(lab) = self.progress.notebook.labs.remove(&from) {
            self.progress.notebook.labs.insert(to.clone(), lab);
        }
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
        self.progress.notebook.labs.remove(&name);
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
            .add_filter(tr!("makine dünyası", "makine world"), &[crate::worlds::EXT])
            .set_file_name(format!("{}.{}", w.world.name, crate::worlds::EXT))
            .save_file()
        else {
            return;
        };
        self.status = Some(match std::fs::write(&path, text) {
            Ok(()) => trf!("Kaydedildi: {}", "Saved: {}", path.display()),
            Err(e) => format!("{}: {e}", path.display()),
        });
    }

    pub(super) fn import_world(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr!("makine dünyası", "makine world"), &[crate::worlds::EXT])
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
                self.status = Some(trf!("'{}' açıldı.", "'{}' opened.", world.name));
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
    /// The World inspector: a fixed header, tabs for what used to share one
    /// long column, and the simulation controls pinned to the bottom.
    pub(super) fn world_panel(&mut self, ui: &mut Ui) {
        let Some(w) = &mut self.world else { return };
        ui.label(crate::theme::strong(w.world.name.clone(), 20.0));
        w.breadcrumbs(ui);
        w.summary(ui);
        ui.add_space(6.0);
        // One row of equal tabs, so the panel never grows a second tab line.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding.x = 2.0;
            let tabs = [
                (WorldTab::Build, tr!("Kur", "Build")),
                (WorldTab::Parts, tr!("Parçalar", "Parts")),
                (WorldTab::Signals, tr!("Sinyal", "Signals")),
                (WorldTab::Worlds, tr!("Dünyalar", "Worlds")),
                (WorldTab::Learn, tr!("Öğren", "Learn")),
            ];
            let width = (ui.available_width() - 2.0 * (tabs.len() - 1) as f32) / tabs.len() as f32;
            for (tab, label) in tabs {
                let on = self.world_tab == tab;
                if ui
                    .add_sized(
                        [width, 26.0],
                        egui::Button::selectable(on, label).truncate(),
                    )
                    .clicked()
                {
                    self.world_tab = tab;
                }
            }
        });
        ui.separator();
        egui::Panel::bottom("world-sim-controls")
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(0, 8)),
            )
            .show(ui, |ui| self.world_sim_controls(ui));
        egui::ScrollArea::vertical()
            .id_salt("world-tab-scroll")
            .show(ui, |ui| match self.world_tab {
                WorldTab::Build => self.world_build_tab(ui),
                WorldTab::Parts => self.world_parts_tab(ui),
                WorldTab::Signals => {
                    if let Some(w) = &mut self.world {
                        w.scope_panel(ui);
                    }
                }
                WorldTab::Worlds => self.world_worlds_tab(ui),
                WorldTab::Learn => self.world_learning_panel(ui),
            });
        if let Some(req) = self.world.as_mut().and_then(|w| w.request.take()) {
            self.handle_world_request(req);
        }
    }

    /// The selection, modules and wire tools; what the player works on.
    fn world_build_tab(&mut self, ui: &mut Ui) {
        let Some(w) = &mut self.world else { return };
        w.symbols = self.progress.symbols;
        if w.selected.is_empty() && w.selected_module.is_none() {
            ui.label(
                RichText::new(tr!(
                    "Bir parça ya da modül seç; ayarları burada görünür.",
                    "Select a part or module; its settings show here."
                ))
                .weak(),
            );
        }
        w.module_panel(ui);
        w.selection_panel(ui);
        egui::CollapsingHeader::new(RichText::new(tr!("⌨ Kısayollar", "⌨ Shortcuts")).strong())
            .id_salt("world-shortcuts")
            .show(ui, |ui| {
                egui::Grid::new("world-shortcut-grid")
                    .num_columns(2)
                    .spacing([10.0, 2.0])
                    .show(ui, |ui| {
                        for (keys, what) in crate::worldview::shortcuts() {
                            ui.label(RichText::new(keys).monospace().small().strong());
                            ui.label(RichText::new(what).small());
                            ui.end_row();
                        }
                    });
            });
    }

    fn world_parts_tab(&mut self, ui: &mut Ui) {
        let Some(w) = &mut self.world else { return };
        crate::theme::section(ui, tr!("Parçalar", "Parts"));
        w.palette_ui(ui, crate::worldview::palette());
        crate::theme::section(ui, tr!("Cihazlar", "Devices"));
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
                .map(|n| (n.clone(), tr!("Senin çipin", "Your chip").to_string())),
        );
        if !chips.is_empty() {
            egui::CollapsingHeader::new(
                RichText::new(trf!("Çiplerin ({})", "Your chips ({})", chips.len())).strong(),
            )
            .id_salt("world-chips")
            .show(ui, |ui| {
                ui.label(
                    RichText::new(tr!(
                        "Çözdüğün seviyeler ve serbest çiplerin. Sürükle ya da tıkla.",
                        "Levels you solved and your free chips. Drag or click."
                    ))
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
                RichText::new(tr!(
                    "Koymak için tahtaya tıkla. Bırakmak için Esc.",
                    "Click the board to place. Esc to drop."
                ))
                .color(crate::theme::COPPER_BRIGHT)
                .small(),
            );
        }

    }

    /// Worlds, building together and example projects.
    fn world_worlds_tab(&mut self, ui: &mut Ui) {
        // Switching worlds mid-session would leave the others behind.
        if self.session.is_none() {
            self.worlds_section(ui);
        }
        self.network_section(ui);
        if self.session.is_none() {
            egui::CollapsingHeader::new(tr!("Örnek projeler", "Example projects"))
                .id_salt("workshop-projects")
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(tr!(
                            "Mevcut dünyan korunur; örnek ayrı bir dünya olarak açılır.",
                            "Your current world is kept; the example opens as a separate world."
                        ))
                        .weak()
                        .small(),
                    );
                    use makine_world::projects::examples::{self, Size};
                    let mut open = None;
                    for size in [Size::Small, Size::Medium, Size::Large] {
                        crate::theme::section(
                            ui,
                            match size {
                                Size::Small => tr!("Küçük", "Small"),
                                Size::Medium => tr!("Orta", "Medium"),
                                Size::Large => tr!("Büyük", "Large"),
                            },
                        );
                        for e in examples::all().into_iter().filter(|e| e.size == size) {
                            if ui.button(e.title()).on_hover_text(e.about()).clicked() {
                                open = Some(e.build);
                            }
                        }
                    }
                    if let Some(build) = open {
                        self.open_project(build());
                    }
                    crate::theme::section(ui, tr!("Çok büyük", "Very large"));
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(tr!("4004 atölyesi aç", "Open 4004 workshop"))
                            .on_hover_text(tr!(
                                "Sayma programıyla: R0 sayar, A'ya eklenir.",
                                "With the counting program: R0 counts, added to A."
                            ))
                            .clicked()
                        {
                            self.open_mcs4_project();
                        }
                        if ui
                            .button(tr!("4004 + RAM", "4004 + RAM"))
                            .on_hover_text(tr!(
                                "4002 RAM programıyla: RAM'de sayar, ROM girişini ekler, RAM \
                                 portuna yazar.",
                                "With the 4002 RAM program: counts in RAM, adds the ROM input, \
                                 writes to the RAM port."
                            ))
                            .clicked()
                        {
                            self.open_project(makine_world::projects::mcs4::project(
                                makine_world::projects::mcs4::RAM_DEMO,
                            ));
                        }
                    });
                    ui.label(
                        RichText::new(tr!(
                            "Kapılardan kurulmuş 4004 çekirdeği, 4002 RAM'i ve G/Ç portlarıyla. \
                             Yalnızca WPM (program RAM'i) yok.",
                            "A 4004 core made of gates, with 4002 RAM and I/O ports. Only WPM \
                             (program RAM) is missing."
                        ))
                        .weak()
                        .small(),
                    );
                });
        }
    }

    fn world_sim_controls(&mut self, ui: &mut Ui) {
        let Some(w) = &mut self.world else { return };
        crate::theme::section(ui, tr!("Simülasyon", "Simulation"));
        ui.horizontal_wrapped(|ui| {
            let label = if w.sim.running {
                tr!("⏸ Durdur", "⏸ Pause")
            } else {
                tr!("▶ Çalıştır", "▶ Run")
            };
            if ui
                .button(label)
                .on_hover_text(tr!(
                    "Saatleri durdur/başlat (P)",
                    "Pause/run the clocks (P)"
                ))
                .clicked()
            {
                w.sim.running = !w.sim.running;
            }
            if ui
                .add_enabled(w.can_undo(), egui::Button::new(tr!("↶ Geri al", "↶ Undo")))
                .clicked()
            {
                w.undo();
            }
            if ui
                .add_enabled(w.can_redo(), egui::Button::new(tr!("↷ Yinele", "↷ Redo")))
                .clicked()
            {
                w.redo();
            }
            if ui
                .button(tr!("⌖ Ortala", "⌖ Center"))
                .on_hover_text(tr!(
                    "Bütün dünyayı göster (Home)",
                    "Show the whole world (Home)"
                ))
                .clicked()
            {
                w.refit();
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(tr!("Saat çevrimi", "Clock cycle"))
                .on_hover_text(tr!(
                    "Dur ve ilk saatin bir sonraki yükselen kenarına kadar ilerle",
                    "Pause and run until the first clock's next rising edge"
                ))
                .clicked()
                && !w.sim.clock_cycle(&w.world)
            {
                w.status = Some(tr!("Önce bir Saat koy.", "Place a Clock first.").into());
            }
            if ui.button(tr!("Baştan", "Restart")).clicked() {
                w.sim.reset();
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !w.sim.running,
                    egui::Button::new(tr!("⏭ Bir adım", "⏭ One step")),
                )
                .on_hover_text(tr!(
                    "Durmuşken saati bir adım ilerlet (N)",
                    "While paused, move the clock one step (N)"
                ))
                .clicked()
            {
                w.sim.single_step();
            }
            let l = ui
                .label(tr!("Hız", "Speed"))
                .on_hover_text(tr!(
                    "Saat her 'periyot' adımda bir değişir; hız, karedeki adım sayısı",
                    "A clock changes every 'period' steps; speed is steps per frame"
                ))
                .id;
            let name = |v: u32| match v {
                0 => tr!("En hızlı", "Fastest").to_string(),
                v => trf!("{v} adım/kare", "{v} steps/frame"),
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
            RichText::new(trf!(
                "{} parça · adım {} · {}",
                "{} parts · step {} · {}",
                w.world.len(),
                w.sim.tick,
                if w.sim.running {
                    trf!("{:.0} adım/sn", "{:.0} steps/s", w.sim.rate)
                } else {
                    tr!("durdu", "paused").into()
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
                    w.status = Some(trf!(
                        "'{name}' çipi oluşturuldu ({n} parça). Serbest çiplerde ve 'Çiplerin' \
                         listesinde.",
                        "Chip '{name}' made ({n} parts). It is in free chips and in the 'Your chips' \
                         list."
                    ));
                }
                self.save();
            }
            Err(e) => w.status = Some(e),
        }
    }
}
