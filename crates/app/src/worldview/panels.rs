//! The world view's side panels: the parts palette and the settings of
//! what is selected.

use egui::{RichText, Ui};
use makine_lang::{tr, trf};
use makine_sim::{mask, Part, PartKind};
use makine_world::{device, Device, Gate, Id, Kind, Placed};

use super::{Request, WorldView};
use crate::theme;

/// What can be dragged from the palette onto the board.
#[derive(Clone, Debug)]
pub struct PaletteItem(pub Kind);

/// A palette entry.
pub struct Entry {
    pub name: &'static str,
    pub kind: Kind,
    pub tip: &'static str,
}

/// The parts offered, in order (1–9 pick the first nine).
pub fn palette() -> Vec<Entry> {
    let e = |name, kind, tip| Entry { name, kind, tip };
    vec![
        e(
            tr!("Anahtar", "Switch"),
            Kind::Switch { on: false },
            tr!("Tıkla: aç / kapa", "Click: on / off"),
        ),
        e(
            tr!("Lamba", "Lamp"),
            Kind::Lamp,
            tr!("Girişi 0 değilse yanar", "Lights when its input is not 0"),
        ),
        e(
            "NOT",
            Kind::Gate(Gate::Not),
            tr!("Girişin tersi", "The opposite of the input"),
        ),
        e(
            "AND",
            Kind::Gate(Gate::And),
            tr!("İkisi de 1 ise 1", "1 if both are 1"),
        ),
        e(
            "OR",
            Kind::Gate(Gate::Or),
            tr!("En az biri 1 ise 1", "1 if at least one is 1"),
        ),
        e(
            "XOR",
            Kind::Gate(Gate::Xor),
            tr!("Farklıysa 1", "1 if they differ"),
        ),
        e(
            "NAND",
            Kind::Gate(Gate::Nand),
            tr!("İkisi de 1 ise 0", "0 if both are 1"),
        ),
        e(
            "NOR",
            Kind::Gate(Gate::Nor),
            tr!("İkisi de 0 ise 1", "1 if both are 0"),
        ),
        e(
            tr!("Saat", "Clock"),
            Kind::Clock { period: 30 },
            tr!(
                "Kendi kendine yanıp söner. Seçip periyodunu ayarla.",
                "Blinks on its own. Select it to set its period."
            ),
        ),
        e(
            tr!("Giriş", "Input"),
            Kind::Input {
                name: "a".into(),
                width: 1,
                value: 0,
            },
            tr!(
                "Tıkla: değeri değiştir. Çip yapınca çipin girişi olur.",
                "Click: change the value. Becomes a chip input when you make a chip."
            ),
        ),
        e(
            tr!("Çıkış", "Output"),
            Kind::Output {
                name: "q".into(),
                width: 1,
            },
            tr!(
                "Bağlı telin değerini gösterir. Çip yapınca çipin çıkışı olur.",
                "Shows the value of its wire. Becomes a chip output when you make a chip."
            ),
        ),
        e(
            tr!("Sabit", "Constant"),
            Kind::Part(Part::new(PartKind::Const { width: 1, value: 1 })),
            tr!("Hep aynı değeri verir", "Always gives the same value"),
        ),
        e(
            tr!("Böl", "Split"),
            Kind::Part(Part::new(PartKind::Split(vec![1; 8]))),
            tr!("Bus'ı parçalara ayırır", "Splits a bus into pieces"),
        ),
        e(
            tr!("Birleştir", "Join"),
            Kind::Part(Part::new(PartKind::Join(vec![1; 8]))),
            tr!("Parçaları tek bus yapar", "Joins pieces into one bus"),
        ),
    ]
}

/// Devices offered.
pub fn device_palette() -> Vec<Entry> {
    let e = |name, d, tip| Entry {
        name,
        kind: Kind::Device(d),
        tip,
    };
    vec![
        e(
            tr!("Yazmaç", "Register"),
            Device::Register { bits: 4 },
            tr!(
                "Yükselen saat kenarında veri saklar; sıfırlama girişli",
                "Stores data on a rising clock edge; has a reset input"
            ),
        ),
        e(
            "Program ROM",
            Device::ProgramRom { words: vec![0] },
            tr!("4096 bayt program belleği", "4096 bytes of program memory"),
        ),
        e(
            "RAM",
            Device::Ram { addr: 4, data: 8 },
            tr!("Okunup yazılabilen bellek", "Memory you can read and write"),
        ),
        e(
            "ROM",
            Device::Rom {
                addr: 4,
                data: 8,
                words: Vec::new(),
            },
            tr!(
                "İçeriği senin yazdığın sabit bellek",
                "Fixed memory with contents you write"
            ),
        ),
        e(
            tr!("Ekran", "Screen"),
            Device::Screen { bits: 3 },
            tr!("Siyah beyaz piksel ekran", "Black and white pixel screen"),
        ),
        e(
            tr!("Klavye", "Keyboard"),
            Device::Keyboard,
            tr!(
                "Yazdığın tuşları devreye verir",
                "Sends the keys you type to the circuit"
            ),
        ),
        e(
            tr!("Gösterge", "Display"),
            Device::Display { bits: 8 },
            tr!("Teldeki sayıyı gösterir", "Shows the number on the wire"),
        ),
    ]
}

/// Width of a palette tile.
pub const TILE: f32 = 58.0;

/// A palette tile: a drawing of the part over its name. Selected tiles are
/// copper. Dragging it onto the board places the part there.
pub fn tile(ui: &mut Ui, kind: &Kind, name: &str, selected: bool) -> egui::Response {
    let size = egui::vec2(TILE, 52.0);
    let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    paint_tile(ui, rect, kind, name, selected, r.hovered());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, name));
    draggable(ui, r, kind)
}

/// Let `r` be dragged onto the board to drop `kind` there; a ghost of its
/// name follows the pointer.
pub fn draggable(ui: &Ui, r: egui::Response, kind: &Kind) -> egui::Response {
    r.dnd_set_drag_payload(PaletteItem(kind.clone()));
    if r.dragged() {
        if let Some(p) = ui.ctx().pointer_interact_pos() {
            let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("world-drag"));
            ui.ctx().layer_painter(layer).text(
                p + egui::vec2(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                super::label(kind),
                egui::FontId::proportional(13.0),
                theme::COPPER_BRIGHT,
            );
        }
    }
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_tile(ui: &Ui, rect: egui::Rect, kind: &Kind, name: &str, selected: bool, hovered: bool) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let p = ui.painter();
    let (fill, stroke) = if selected {
        (theme::COPPER.gamma_multiply(0.22), theme::COPPER)
    } else if hovered {
        (theme::RAISED_HOVER, theme::INK_DIM)
    } else {
        (theme::RAISED, theme::LINE)
    };
    p.rect(
        rect,
        4.0,
        fill,
        egui::Stroke::new(if selected { 1.5 } else { 1.0 }, stroke),
        egui::StrokeKind::Inside,
    );
    let icon = egui::Rect::from_center_size(
        rect.center_top() + egui::vec2(0.0, 19.0),
        egui::vec2(28.0, 24.0),
    );
    icon_for(p, icon, kind, selected);
    let text = if selected {
        theme::COPPER_BRIGHT
    } else {
        theme::INK
    };
    p.text(
        rect.center_bottom() - egui::vec2(0.0, 6.0),
        egui::Align2::CENTER_BOTTOM,
        name,
        egui::FontId::proportional(11.0),
        text,
    );
}

/// A small drawing of a part, for its tile.
fn icon_for(painter: &egui::Painter, r: egui::Rect, kind: &Kind, lit: bool) {
    use crate::canvas::draw::{GATE_LINE, ON};
    use egui::{pos2, vec2, Color32, Pos2, Stroke};
    let c = r.center();
    let s = r.height().min(r.width());
    let line = Stroke::new(1.6, if lit { Color32::WHITE } else { GATE_LINE });
    let accent = if lit { ON } else { theme::COPPER };
    match kind {
        Kind::Gate(g) => {
            use crate::design::Symbol;
            let symbol = match g {
                Gate::Not => Symbol::Not,
                Gate::And => Symbol::And,
                Gate::Nand => Symbol::Nand,
                Gate::Or => Symbol::Or,
                Gate::Nor => Symbol::Nor,
                Gate::Xor => Symbol::Xor,
            };
            let (x0, x1) = (c.x - s * 0.34, c.x + s * 0.26);
            let (y0, y1) = (c.y - s * 0.3, c.y + s * 0.3);
            let outline = crate::canvas::gate_outline(symbol, x0, x1, y0, y1);
            if *g == Gate::Xor {
                let pts: Vec<Pos2> = (0..=10)
                    .map(|k| {
                        let t = k as f32 / 10.0;
                        pos2(
                            x0 - s * 0.08 + 4.0 * t * (1.0 - t) * s * 0.1,
                            y0 + (y1 - y0) * t,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, line));
            }
            painter.add(egui::Shape::closed_line(outline, line));
            if symbol.bubble() {
                painter.circle_stroke(pos2(x1 + s * 0.06, c.y), s * 0.06, line);
            }
        }
        Kind::Switch { .. } => {
            let body = egui::Rect::from_center_size(c, vec2(s * 0.8, s * 0.42));
            painter.rect_filled(body, s * 0.21, theme::RAISED_HOVER);
            painter.circle_filled(body.right_center() - vec2(s * 0.21, 0.0), s * 0.15, accent);
        }
        Kind::Lamp => {
            painter.circle_filled(c, s * 0.42, ON.gamma_multiply(0.15));
            painter.circle_filled(c, s * 0.26, Color32::from_rgb(255, 238, 170));
        }
        Kind::Clock { .. } => {
            let w = s * 0.4;
            let (y0, y1) = (c.y + s * 0.2, c.y - s * 0.2);
            let pts = vec![
                pos2(c.x - w, y0),
                pos2(c.x - w / 2.0, y0),
                pos2(c.x - w / 2.0, y1),
                pos2(c.x + w / 2.0, y1),
                pos2(c.x + w / 2.0, y0),
                pos2(c.x + w, y0),
            ];
            painter.add(egui::Shape::line(pts, Stroke::new(2.0, accent)));
        }
        Kind::Input { .. } | Kind::Output { .. } => {
            let output = matches!(kind, Kind::Output { .. });
            let body = egui::Rect::from_center_size(c, vec2(s * 0.62, s * 0.5));
            let fill = Color32::from_rgb(208, 220, 210);
            painter.rect_filled(body, 2.0, fill);
            let (x, d) = if output {
                (body.left(), -1.0)
            } else {
                (body.right(), 1.0)
            };
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(x + d * s * 0.2, c.y),
                    pos2(x, c.y - s * 0.12),
                    pos2(x, c.y + s * 0.12),
                ],
                fill,
                Stroke::NONE,
            ));
        }
        Kind::Part(p) => match &p.kind {
            PartKind::Const { .. } => {
                painter.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    "1",
                    egui::FontId::monospace(s * 0.7),
                    accent,
                );
            }
            PartKind::Split(_) | PartKind::Join(_) => {
                let split = matches!(p.kind, PartKind::Split(_));
                let (a, b) = if split { (-1.0, 1.0) } else { (1.0, -1.0) };
                let hub = c + vec2(a * s * 0.1, 0.0);
                painter.line_segment(
                    [c + vec2(a * s * 0.45, 0.0), hub],
                    Stroke::new(s * 0.14, theme::BUS),
                );
                for dy in [-0.3, -0.1, 0.1, 0.3] {
                    painter.line_segment(
                        [hub, c + vec2(b * s * 0.45, dy * s)],
                        Stroke::new(1.5, accent),
                    );
                }
            }
            _ => chip_icon(painter, c, s, line),
        },
        Kind::Device(Device::Screen { .. }) => {
            let body = egui::Rect::from_center_size(c, vec2(s * 0.86, s * 0.62));
            painter.rect_filled(body, 2.0, Color32::from_rgb(8, 12, 10));
            painter.rect_stroke(body, 2.0, line, egui::StrokeKind::Inside);
            for (x, y) in [
                (-0.2, -0.1),
                (0.2, -0.1),
                (-0.2, 0.12),
                (0.0, 0.16),
                (0.2, 0.12),
            ] {
                painter.circle_filled(
                    c + vec2(x * s, y * s),
                    s * 0.04,
                    Color32::from_rgb(140, 255, 160),
                );
            }
        }
        Kind::Device(Device::Display { .. }) => {
            let body = egui::Rect::from_center_size(c, vec2(s * 0.9, s * 0.56));
            painter.rect_filled(body, 2.0, Color32::from_rgb(12, 24, 18));
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "42",
                egui::FontId::monospace(s * 0.4),
                Color32::from_rgb(140, 255, 160),
            );
        }
        Kind::Device(_) => chip_icon(painter, c, s, line),
    }
}

/// A chip package with pins along both long edges.
fn chip_icon(painter: &egui::Painter, c: egui::Pos2, s: f32, line: egui::Stroke) {
    use egui::{pos2, vec2};
    let body = egui::Rect::from_center_size(c, vec2(s * 0.86, s * 0.62));
    painter.rect_filled(body, 2.0, crate::canvas::draw::PART_FILL);
    painter.rect_stroke(body, 2.0, line, egui::StrokeKind::Inside);
    for k in 0..4 {
        let x = body.left() + body.width() * (0.2 + 0.2 * k as f32);
        for y in [body.top(), body.bottom()] {
            painter.line_segment(
                [pos2(x, y - s * 0.07), pos2(x, y + s * 0.07)],
                egui::Stroke::new(1.6, egui::Color32::from_rgb(196, 204, 198)),
            );
        }
    }
}

/// Tiles in rows that fill the width.
pub fn tiles(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        ui.horizontal_wrapped(add);
    });
}

impl WorldView {
    /// Tiles for `entries`; clicking one puts it in hand (click the board to
    /// place it), dragging one drops it where it is let go.
    pub fn palette_ui(&mut self, ui: &mut Ui, entries: Vec<Entry>) {
        tiles(ui, |ui| {
            for e in entries {
                let selected = self.placing.as_ref().is_some_and(|k| same_tool(k, &e.kind));
                if tile(ui, &e.kind, e.name, selected)
                    .on_hover_text(e.tip)
                    .clicked()
                {
                    self.placing = if selected { None } else { Some(e.kind) };
                }
            }
        });
    }

    /// Tools for the selection: copy, delete, make a chip.
    pub fn selection_panel(&mut self, ui: &mut Ui) {
        let n = self.selected.len();
        if n == 0 {
            return;
        }
        if n == 1 {
            self.part_panel(ui);
        } else {
            ui.horizontal(|ui| {
                ui.label(theme::strong(
                    trf!("{n} parça seçili", "{n} parts selected"),
                    15.0,
                ));
                if ui
                    .small_button("✖")
                    .on_hover_text(tr!("Seçimi bırak (Esc)", "Clear selection (Esc)"))
                    .clicked()
                {
                    self.selected.clear();
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(tr!("Çoğalt", "Duplicate"))
                .on_hover_text("Ctrl+D")
                .clicked()
            {
                self.duplicate();
            }
            if ui
                .button(tr!("Kopyala", "Copy"))
                .on_hover_text("Ctrl+C")
                .clicked()
            {
                if let Some(t) = self.copy() {
                    ui.ctx().copy_text(t);
                }
            }
            if ui
                .button(tr!("Sil", "Delete"))
                .on_hover_text("Delete")
                .clicked()
            {
                self.delete_selected();
            }
        });
        if n < 2 {
            return;
        }
        theme::section(ui, tr!("Çipe çevir", "Turn into a chip"));
        ui.label(
            RichText::new(tr!(
                "Seçimdeki Girişler ve Çıkışlar çipin uçları olur (yukarıdan aşağı). \
                 Anahtarlar o anki değerleriyle sabit olur.",
                "The Inputs and Outputs in the selection become the chip's pins (top to bottom). \
                 Switches become constants with their current values."
            ))
            .weak()
            .small(),
        );
        ui.horizontal(|ui| {
            let l = ui.label(tr!("Ad", "Name")).id;
            ui.add(
                egui::TextEdit::singleline(&mut self.chip_name)
                    .hint_text("Toplayici")
                    .desired_width(110.0),
            )
            .labelled_by(l);
            if ui.button(tr!("Çip yap", "Make chip")).clicked() {
                self.request = Some(Request::MakeChip {
                    name: self.chip_name.trim().to_string(),
                    parts: self.selected.iter().copied().collect(),
                });
            }
        });
    }

    /// Settings of the one selected part.
    fn part_panel(&mut self, ui: &mut Ui) {
        let Some((id, Placed { kind, .. })) = self.single().map(|(i, p)| (i, p.clone())) else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(theme::strong(kind.label(), 15.0));
            if ui
                .small_button("✖")
                .on_hover_text(tr!("Seçimi bırak (Esc)", "Clear selection (Esc)"))
                .clicked()
            {
                self.selected.clear();
            }
        });
        let mut new: Option<Kind> = None;
        let num = |ui: &mut Ui, label: &str, v: u32, lo: u32, hi: u32| -> Option<u32> {
            let mut x = v;
            let r = ui
                .horizontal(|ui| {
                    let l = ui.label(label).id;
                    ui.add(egui::DragValue::new(&mut x).range(lo..=hi))
                        .labelled_by(l)
                })
                .inner;
            (r.changed() && x != v).then_some(x)
        };
        match &kind {
            Kind::Clock { period } => {
                if let Some(p) = num(
                    ui,
                    tr!("Periyot (adım)", "Period (steps)"),
                    *period,
                    1,
                    100_000,
                ) {
                    new = Some(Kind::Clock { period: p });
                }
            }
            Kind::Input { .. } | Kind::Output { .. } => new = port_settings(ui, &kind),
            Kind::Part(p) => {
                if let Some(k) = part_settings(ui, p) {
                    new = Some(Kind::Part(k));
                }
            }
            Kind::Device(d) => {
                if let Some(d) = self.device_settings(ui, id, d) {
                    new = Some(Kind::Device(d));
                }
            }
            _ => {}
        }
        if let Some(k) = new {
            self.set_kind(id, k);
        }
    }

    /// Settings of a device; returns the changed device.
    fn device_settings(&mut self, ui: &mut Ui, id: Id, d: &Device) -> Option<Device> {
        ui.label(RichText::new(d.help()).weak().small());
        let size = |ui: &mut Ui, label: &str, v: u8, lo: u8, hi: u8| -> Option<u8> {
            let mut x = v;
            let r = ui
                .horizontal(|ui| {
                    let l = ui.label(label).id;
                    ui.add(egui::DragValue::new(&mut x).range(lo..=hi))
                        .labelled_by(l)
                })
                .inner;
            (r.changed() && x != v).then_some(x)
        };
        let mut changed = None;
        match d {
            Device::Ram { addr, data } => {
                if let Some(a) = size(
                    ui,
                    tr!("Adres biti", "Address bits"),
                    *addr,
                    1,
                    device::MAX_ADDR,
                ) {
                    changed = Some(Device::Ram {
                        addr: a,
                        data: *data,
                    });
                }
                if let Some(b) = size(
                    ui,
                    tr!("Veri biti", "Data bits"),
                    *data,
                    1,
                    device::MAX_DATA,
                ) {
                    changed = Some(Device::Ram {
                        addr: *addr,
                        data: b,
                    });
                }
                if let Some(words) = self.ram_words(id) {
                    ui.label(RichText::new(tr!("İçerik", "Contents")).small());
                    ui.label(
                        RichText::new(device::words_text(&words, *data))
                            .monospace()
                            .small(),
                    );
                }
            }
            Device::Rom { addr, data, words } => {
                if let Some(a) = size(
                    ui,
                    tr!("Adres biti", "Address bits"),
                    *addr,
                    1,
                    device::MAX_ADDR,
                ) {
                    changed = Some(Device::Rom {
                        addr: a,
                        data: *data,
                        words: words.clone(),
                    });
                }
                if let Some(b) = size(
                    ui,
                    tr!("Veri biti", "Data bits"),
                    *data,
                    1,
                    device::MAX_DATA,
                ) {
                    changed = Some(Device::Rom {
                        addr: *addr,
                        data: b,
                        words: words.clone(),
                    });
                }
                changed = changed.or_else(|| {
                    self.rom_editor(ui, id, *addr, *data, words)
                        .map(|words| Device::Rom {
                            addr: *addr,
                            data: *data,
                            words,
                        })
                });
            }
            Device::Screen { bits } => {
                if let Some(b) = size(
                    ui,
                    tr!("Boyut biti", "Size bits"),
                    *bits,
                    2,
                    device::MAX_SCREEN_BITS,
                ) {
                    changed = Some(Device::Screen { bits: b });
                }
            }
            Device::Display { bits } => {
                if let Some(b) = size(ui, tr!("Bit sayısı", "Bits"), *bits, 1, device::MAX_DATA) {
                    changed = Some(Device::Display { bits: b });
                }
            }
            Device::Register { bits } => {
                if let Some(b) = size(ui, tr!("Bit sayısı", "Bits"), *bits, 1, 64) {
                    changed = Some(Device::Register { bits: b });
                }
                let value = self.sim.value(makine_world::End::new(id, 0));
                ui.monospace(value.map_or("?".into(), |v| format!("0x{v:X} ({v})")));
            }
            Device::ProgramRom { words } => {
                changed = self
                    .rom_editor(ui, id, 12, 8, words)
                    .map(|words| Device::ProgramRom { words });
            }
            Device::Keyboard => {
                ui.colored_label(
                    theme::GOOD,
                    tr!(
                        "⌨ Yazdıkların bu klavyeye gidiyor. Bitirince Esc.",
                        "⌨ What you type goes to this keyboard. Esc when done."
                    ),
                );
            }
        }
        changed
    }

    /// The text box for a ROM's contents; returns the ROM with new words.
    fn rom_editor(
        &mut self,
        ui: &mut Ui,
        id: Id,
        addr: u8,
        data: u8,
        words: &[u64],
    ) -> Option<Vec<u64>> {
        // Refresh the text when another ROM is selected or its contents
        // changed (undo, another player).
        let fresh = device::words_text(words, data);
        let stale = self
            .rom_text
            .as_ref()
            .is_none_or(|(i, _, shown, _)| *i != id || shown.as_deref() != Some(fresh.as_str()));
        if stale {
            self.rom_text = Some((id, fresh.clone(), Some(fresh), None));
        }
        let l = ui
            .label(
                RichText::new(trf!(
                    "İçerik: {} kelime, onaltılık (ya da #onluk), ; yorum",
                    "Contents: {} words, hexadecimal (or #decimal), ; comment",
                    1u32 << addr
                ))
                .small(),
            )
            .id;
        let (_, text, _, err) = self.rom_text.as_mut().unwrap();
        ui.add(
            egui::TextEdit::multiline(text)
                .code_editor()
                .desired_rows(6)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(l);
        let mut out = None;
        if ui.button(tr!("İçeriği yükle", "Load contents")).clicked() {
            match device::parse_words(text) {
                Ok(w) if w.len() > 1 << addr => {
                    *err = Some(trf!(
                        "{} kelime fazla",
                        "{} words too many",
                        w.len() - (1 << addr)
                    ))
                }
                Ok(w) => {
                    *err = None;
                    out = Some(w);
                }
                Err(e) => *err = Some(e),
            }
        }
        if let Some(e) = err {
            ui.colored_label(theme::BAD, e.as_str());
        }
        out
    }
}

/// Name, width and (for inputs) value of an input or output part; returns
/// the changed part.
fn port_settings(ui: &mut Ui, kind: &Kind) -> Option<Kind> {
    let (name, width, value) = match kind {
        Kind::Input { name, width, value } => (name, *width, Some(*value)),
        Kind::Output { name, width } => (name, *width, None),
        _ => return None,
    };
    let mut n = name.clone();
    let mut w = width;
    let mut v = value.unwrap_or(0);
    ui.horizontal(|ui| {
        let l = ui.label(tr!("Ad", "Name")).id;
        ui.add(egui::TextEdit::singleline(&mut n).desired_width(80.0))
            .labelled_by(l);
    });
    ui.horizontal(|ui| {
        let l = ui.label(tr!("Bit sayısı", "Bits")).id;
        ui.add(egui::DragValue::new(&mut w).range(1..=64))
            .labelled_by(l);
    });
    if value.is_some() {
        ui.horizontal(|ui| {
            let l = ui.label(tr!("Değer", "Value")).id;
            ui.add(egui::DragValue::new(&mut v).range(0..=mask(w)))
                .labelled_by(l);
        });
    }
    let n = n.trim().to_string();
    let name = if makine_world::part::valid_name(&n) {
        n
    } else {
        name.clone()
    };
    let new = match value {
        Some(_) => Kind::Input {
            name,
            width: w,
            value: v & mask(w),
        },
        None => Kind::Output { name, width: w },
    };
    (new != *kind).then_some(new)
}

/// Whether two palette kinds are the same tool (a switch is a switch,
/// whatever its setting).
fn same_tool(a: &Kind, b: &Kind) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
        && match (a, b) {
            (Kind::Gate(x), Kind::Gate(y)) => x == y,
            (Kind::Device(x), Kind::Device(y)) => {
                std::mem::discriminant(x) == std::mem::discriminant(y)
            }
            (Kind::Part(x), Kind::Part(y)) => match (&x.kind, &y.kind) {
                (PartKind::Chip(m), PartKind::Chip(n)) => m == n,
                (m, n) => std::mem::discriminant(m) == std::mem::discriminant(n),
            },
            _ => true,
        }
}

/// Settings of a library part, splitter, joiner or constant, like the
/// canvas's right-click menu. Returns the changed part.
fn part_settings(ui: &mut Ui, p: &Part) -> Option<Part> {
    let mut part = p.clone();
    let mut changed = false;
    match &mut part.kind {
        PartKind::Split(widths) | PartKind::Join(widths) => {
            let label = ui
                .label(tr!(
                    "Gruplar (düşük bitten başlayarak)",
                    "Groups (from the low bit)"
                ))
                .id;
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
            ui.label(
                RichText::new(tr!(
                    "örn. 8,8,8,8 ya da 1,1,1,1,1,1,1,1",
                    "e.g. 8,8,8,8 or 1,1,1,1,1,1,1,1"
                ))
                .weak()
                .small(),
            );
        }
        PartKind::Const { width, value } => {
            ui.horizontal(|ui| {
                let label = ui.label(tr!("Bit sayısı", "Bits")).id;
                changed |= ui
                    .add(egui::DragValue::new(width).range(1..=64))
                    .labelled_by(label)
                    .changed();
            });
            ui.horizontal(|ui| {
                let label = ui.label(tr!("Değer", "Value")).id;
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
            let label = ui.label(tr!("Tekrar", "Repeat")).id;
            changed |= ui
                .add(egui::DragValue::new(&mut part.repeat).range(1..=64))
                .labelled_by(label)
                .changed();
        })
        .response
        .on_hover_text(tr!(
            "Parçanın kaç kopyası yan yana çalışsın. Her uç bu kadar genişler.",
            "How many copies of the part work side by side. Each pin gets this many times wider."
        ));
    }
    changed.then_some(part)
}

fn parse_widths(text: &str) -> Option<Vec<u8>> {
    let ws: Vec<u8> = text
        .split(',')
        .map(|s| s.trim().parse::<u8>().ok().filter(|&w| w >= 1))
        .collect::<Option<_>>()?;
    let total: u32 = ws.iter().map(|&w| w as u32).sum();
    (!ws.is_empty() && total <= 64).then_some(ws)
}
