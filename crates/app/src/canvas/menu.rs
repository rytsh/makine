//! Right-click menus on the canvas.

use egui::Ui;
use makine_sim::{mask, Library, PartId, PartKind, Sink};

use super::layout::part_label;
use crate::design::Design;

#[derive(Clone, Copy, Debug)]
pub(super) enum MenuTarget {
    None,
    Sink(Sink),
    Wire(usize),
    Part(PartId),
    Ports,
}

/// Context menu for a part. Returns whether the design changed.
pub(super) fn part_menu(ui: &mut Ui, design: &mut Design, i: PartId, lib: &Library) -> bool {
    let mut changed = false;
    let part = &mut design.def.parts[i];
    ui.label(egui::RichText::new(part_label(part)).strong());
    ui.separator();

    match &mut part.kind {
        PartKind::Split(widths) | PartKind::Join(widths) => {
            let label = ui.label("Gruplar (düşük bitten başlayarak):").id;
            let mut text = widths
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join(",");
            if ui
                .text_edit_singleline(&mut text)
                .labelled_by(label)
                .changed()
            {
                if let Some(ws) = parse_widths(&text) {
                    *widths = ws;
                    changed = true;
                }
            }
            ui.label(egui::RichText::new("örn. 8,8,8,8 ya da 1,1,1,1,1,1,1,1").small());
        }
        PartKind::Const { width, value } => {
            ui.horizontal(|ui| {
                let label = ui.label("Genişlik:").id;
                changed |= ui
                    .add(egui::DragValue::new(width).range(1..=64))
                    .labelled_by(label)
                    .changed();
            });
            ui.horizontal(|ui| {
                let label = ui.label("Değer:").id;
                changed |= ui
                    .add(egui::DragValue::new(value).hexadecimal(1, false, true))
                    .labelled_by(label)
                    .changed();
            });
            *value &= mask(*width);
        }
        _ => {}
    }

    if !matches!(part.kind, PartKind::Const { .. }) {
        ui.horizontal(|ui| {
            let label = ui.label("Tekrar:").id;
            changed |= ui
                .add(egui::DragValue::new(&mut part.repeat).range(1..=64))
                .labelled_by(label)
                .changed();
        })
        .response
        .on_hover_text("Parçanın kaç kopyası yan yana çalışsın. Her uç bu kadar genişler.");
    }

    if changed {
        design.prune(lib);
    }
    ui.separator();
    if ui.button("Sil").clicked() {
        design.remove_part(i);
        changed = true;
        ui.close();
    }
    changed
}

fn parse_widths(text: &str) -> Option<Vec<u8>> {
    let ws: Vec<u8> = text
        .split(',')
        .map(|s| s.trim().parse::<u8>().ok().filter(|&w| w >= 1))
        .collect::<Option<_>>()?;
    let total: u32 = ws.iter().map(|&w| w as u32).sum();
    (!ws.is_empty() && total <= 64).then_some(ws)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_parsing() {
        assert_eq!(parse_widths("8, 8,8,8"), Some(vec![8; 4]));
        assert_eq!(parse_widths("1"), Some(vec![1]));
        assert_eq!(parse_widths("0,1"), None);
        assert_eq!(parse_widths("32,33"), None);
        assert_eq!(parse_widths("x"), None);
    }
}
