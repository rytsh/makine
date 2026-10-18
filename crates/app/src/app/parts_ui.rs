//! Searchable editor palette. Eligibility is resolved before filtering, so
//! search never grants access to future chips or recursive sandbox parts.
use super::{App, Target};
use crate::{canvas::NewPart, theme};
use egui::{RichText, Ui};
use makine_lang::{tr, trf};
use makine_levels::{find, levels, Level};
use makine_sim::PartKind;

#[derive(Default)]
pub(super) struct PartsBrowser {
    pub query: String,
    focused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Group {
    Basic,
    Bus,
    Free,
    Section(&'static str),
}

struct Entry {
    id: String,
    label: String,
    tip: String,
    search: String,
    group: Group,
    group_name: &'static str,
    kind: PartKind,
}

fn normalized(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| {
            Some(match c {
                'ı' => 'i',
                'ş' => 's',
                'ğ' => 'g',
                'ü' => 'u',
                'ö' => 'o',
                'ç' => 'c',
                '\u{307}' => return None,
                c if c.is_alphanumeric() => c,
                _ => ' ',
            })
        })
        .collect()
}

impl Entry {
    fn matches(&self, query: &str) -> bool {
        normalized(query)
            .split_whitespace()
            .all(|word| self.search.contains(word))
    }
}

impl App {
    fn palette_entries(&self) -> Vec<Entry> {
        let mut entries = vec![Entry {
            id: "builtin:nand".into(),
            label: "NAND".into(),
            tip: tr!(
                "Her şeyin temeli: iki giriş de 1 ise 0, değilse 1.",
                "The base of everything: 0 if both inputs are 1, otherwise 1."
            )
            .into(),
            search: normalized("NAND kapı gate temel basic"),
            group: Group::Basic,
            group_name: tr!("Temel", "Basic"),
            kind: PartKind::Nand,
        }];
        let (allowed, buses): (Vec<&Level>, bool) = match &self.target {
            Target::Level(level) => (
                level.allowed_chips().collect(),
                level.index() >= find("Mux32").unwrap().index(),
            ),
            Target::Free(_) => (levels().to_vec(), true),
        };
        if buses {
            for (id, label, aliases, kind, tip) in [
                (
                    "split",
                    tr!("Böl", "Split"),
                    "bol böl split bus ayir ayır",
                    PartKind::Split(vec![1; 8]),
                    tr!(
                        "Bus'ı parçalara ayır. Sağ tıkla: grupları ayarla.",
                        "Split a bus into parts. Right-click: set the groups."
                    ),
                ),
                (
                    "join",
                    tr!("Birleştir", "Join"),
                    "birlestir birleştir join bus",
                    PartKind::Join(vec![1; 8]),
                    tr!(
                        "Parçaları tek bus yap. Sağ tıkla: grupları ayarla.",
                        "Join parts into one bus. Right-click: set the groups."
                    ),
                ),
                (
                    "constant",
                    tr!("Sabit", "Constant"),
                    "sabit constant const bus",
                    PartKind::Const { width: 1, value: 1 },
                    tr!(
                        "Hep aynı değeri verir. Sağ tıkla: değeri ayarla.",
                        "Always gives the same value. Right-click: set the value."
                    ),
                ),
            ] {
                entries.push(Entry {
                    id: format!("builtin:{id}"),
                    label: label.into(),
                    tip: tip.into(),
                    search: normalized(aliases),
                    group: Group::Bus,
                    group_name: tr!("Bus araçları", "Bus tools"),
                    kind,
                });
            }
        }
        if let Target::Free(me) = &self.target {
            for name in self
                .progress
                .sandbox
                .chips
                .keys()
                .filter(|name| !self.progress.sandbox.uses(name, me))
            {
                entries.push(Entry {
                    id: format!("chip:{name}"),
                    label: trf!("Çip: {name}", "Chip: {name}"),
                    tip: tr!("Senin çipin", "Your chip").into(),
                    search: normalized(&format!("{name} serbest free custom çip chip")),
                    group: Group::Free,
                    group_name: tr!("Serbest çipler", "Free chips"),
                    kind: PartKind::Chip(name.clone()),
                });
            }
        }
        for l in allowed.iter().filter(|l| self.progress.solved(l.chip)) {
            let names = l.names();
            let group_name = l
                .section_name()
                .split_once(". ")
                .map_or(l.section_name(), |(_, title)| title);
            let aliases = match l.chip {
                "Bit" | "Register4" | "Register32" | "MiniAcc8" => "yazmaç kayit register storage",
                "Counter32" | "Counter12" | "MiniPc4" => "sayac sayaç counter program pc",
                _ => "",
            };
            entries.push(Entry {
                id: format!("chip:{}", l.chip),
                label: l.chip.into(),
                tip: l.name().into(),
                search: normalized(&format!("{} {} {} {aliases}", l.chip, names[0], names[1])),
                group: Group::Section(l.section),
                group_name,
                kind: PartKind::Chip(l.chip.into()),
            });
        }
        entries
    }

    pub(super) fn parts_panel(&mut self, ui: &mut Ui) {
        ui.strong(tr!("Parçalar", "Parts"));
        ui.label(
            RichText::new(tr!("Tuvale sürükle", "Drag onto the board"))
                .small()
                .weak(),
        );
        let label = ui
            .label(RichText::new(tr!("Parça ara", "Find a part")).small())
            .id;
        let search_id = ui.scope_id().with("parts-search");
        let escape = self.parts_browser.focused
            && ui.input(|input| {
                input.raw.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Escape,
                            pressed: true,
                            ..
                        }
                    )
                })
            });
        if escape {
            self.parts_browser.query.clear();
        }
        let search = ui.add(egui::TextEdit::singleline(&mut self.parts_browser.query).id(search_id)
            .hint_text(tr!("ad veya çip", "name or chip")).char_limit(128).desired_width(ui.available_width()))
            .labelled_by(label)
            .on_hover_text(tr!("Çip adı, Türkçe veya İngilizce başlık. Yalnızca bu devrede kullanabileceğin parçalar aranır.", "Chip id, Turkish or English title. Searches only parts allowed in this circuit."));
        if escape {
            search.surrender_focus();
        }
        self.parts_browser.focused = search.has_focus() && !escape;
        let entries = self.palette_entries();
        let shown: Vec<_> = entries
            .iter()
            .filter(|entry| entry.matches(&self.parts_browser.query))
            .collect();
        let filtering = !self.parts_browser.query.trim().is_empty();
        ui.label(
            RichText::new(if filtering {
                trf!("{} / {} parça", "{} / {} parts", shown.len(), entries.len())
            } else {
                trf!("{} parça", "{} parts", entries.len())
            })
            .small()
            .weak(),
        );
        if filtering
            && ui
                .small_button(tr!("Aramayı temizle", "Clear search"))
                .clicked()
        {
            self.parts_browser.query.clear();
        }
        ui.separator();
        if shown.is_empty() {
            ui.label(tr!("Eşleşme yok", "No matches"));
            ui.label(
                RichText::new(tr!(
                    "Başka bir ad dene veya aramayı temizle.",
                    "Try another name or clear the search."
                ))
                .small()
                .weak(),
            );
            return;
        }
        let latest = entries
            .iter()
            .rev()
            .find(|e| matches!(e.group, Group::Section(_)))
            .map(|e| e.group);
        egui::ScrollArea::vertical()
            .id_salt(("parts_scroll", filtering))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut start = 0;
                while start < shown.len() {
                    let entry = shown[start];
                    let count = shown[start..]
                        .iter()
                        .take_while(|e| e.group == entry.group)
                        .count();
                    let default_open = entries.len() <= 12
                        || matches!(entry.group, Group::Basic | Group::Bus | Group::Free)
                        || latest == Some(entry.group);
                    egui::CollapsingHeader::new(
                        RichText::new(format!("{} ({count})", entry.group_name))
                            .small()
                            .strong(),
                    )
                    .id_salt(("editor-parts-group", entry.group, filtering))
                    .default_open(default_open)
                    .open(filtering.then_some(true))
                    .show(ui, |ui| {
                        for entry in &shown[start..start + count] {
                            palette_row(ui, entry);
                        }
                    });
                    start += count;
                }
            });
    }
}

fn palette_row(ui: &mut Ui, entry: &Entry) {
    let id = ui.scope_id().with(("editor-palette", &entry.id));
    ui.dnd_drag_source(id, NewPart(entry.kind.clone()), |ui| {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(6, 5))
            .fill(theme::WELL)
            .stroke(egui::Stroke::new(1., theme::LINE))
            .corner_radius(3.)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(&entry.label).wrap());
            });
    })
    .response
    .on_hover_text(&entry.tip);
}

#[cfg(test)]
mod tests;
