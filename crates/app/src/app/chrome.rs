//! The top bar: the logo, the parts of the game as tabs, settings, and —
//! when the window has no frame from the system — our own title bar with
//! its buttons, dragging and resizing.

use egui::{pos2, vec2, Color32, CursorIcon, FontId, Rect, RichText, Sense, Stroke, Ui};
use makine_lang::{tr, Lang};

use super::{home_ui, App, View};
use crate::theme;

pub const BAR_HEIGHT: f32 = 40.0;
/// How far in from the window's edge a drag resizes it.
const GRIP: f32 = 6.0;

impl App {
    /// The bar at the top of the window.
    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        let framed = !self.progress.custom_frame;
        egui::Panel::top("top_bar")
            .exact_size(BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(theme::WELL)
                    .inner_margin(egui::Margin::symmetric(10, 0))
                    .stroke(Stroke::new(1.0, theme::LINE)),
            )
            .show(ui, |ui| {
                let bar = ui.max_rect();
                // The empty part of the bar moves the window; added first so
                // the buttons on top of it get the clicks.
                let drag = ui.interact(bar, ui.scope_id().with("drag"), Sense::click_and_drag());
                if !framed {
                    if drag.drag_started() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if drag.double_clicked() {
                        toggle_maximized(ui.ctx());
                    }
                }
                ui.horizontal_centered(|ui| {
                    self.brand(ui);
                    ui.add_space(14.0);
                    self.tabs(ui);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !framed {
                            window_buttons(ui);
                            ui.add_space(6.0);
                        }
                        self.settings_menu(ui);
                        if let Some(p) = &self.profiles {
                            ui.label(
                                RichText::new(&p.current().name)
                                    .color(theme::INK_DIM)
                                    .small(),
                            )
                            .on_hover_text(tr!("Profil", "Profile"));
                        }
                    });
                });
            });
    }

    /// Logo and name; a click goes home.
    fn brand(&mut self, ui: &mut Ui) {
        let r = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add(egui::Image::new(home_ui::logo()).fit_to_exact_size(vec2(24.0, 24.0)));
                ui.label(theme::strong("makine", 17.0).color(theme::INK));
            })
            .response
            .interact(Sense::click())
            .on_hover_text(tr!(
                "Ana sayfa: ilerleme ve profiller",
                "Home: progress and profiles"
            ));
        if r.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if r.clicked() {
            self.go_home();
        }
    }

    fn tabs(&mut self, ui: &mut Ui) {
        let cpu_done = self.progress.solved("Cpu");
        let tabs: [(View, &str, &str, bool); 6] = [
            (View::Home, "⌂", tr!("Ana sayfa", "Home"), true),
            (View::Editor, "◧", tr!("Seviyeler", "Levels"), true),
            (View::World, "🌍", tr!("Dünya", "World"), true),
            (
                View::Programming,
                "",
                tr!("Programlama", "Programming"),
                true,
            ),
            (View::Wiki, "📖", "Wiki", true),
            (View::Computer, "🖥", tr!("Bilgisayar", "Computer"), cpu_done),
        ];
        for (view, icon, name, enabled) in tabs {
            let on = self.view == view || (view == View::Computer && self.view == View::Play);
            let r = tab(ui, on, enabled, &format!("{icon}  {name}"));
            let r = if enabled {
                r
            } else {
                r.on_disabled_hover_text(tr!(
                    "İşlemciyi bitirince açılır",
                    "Opens once the CPU is done"
                ))
            };
            if r.clicked() && !on {
                match view {
                    View::Home => self.go_home(),
                    View::World => self.open_world(),
                    v => self.view = v,
                }
            }
        }
    }

    fn settings_menu(&mut self, ui: &mut Ui) {
        let r = ui.menu_button(RichText::new("⚙").size(16.0), |ui| {
            ui.set_min_width(240.0);
            theme::section(ui, tr!("Dil", "Language"));
            for lang in Lang::ALL {
                if ui.radio(makine_lang::get() == lang, lang.name()).clicked() {
                    self.set_lang(lang);
                }
            }
            theme::section(ui, tr!("Simülasyon", "Simulation"));
            let fast = ui
                .checkbox(
                    &mut self.progress.fast,
                    tr!("⚡ Hızlı simülasyon", "⚡ Fast simulation"),
                )
                .on_hover_text(tr!(
                    "Geçtiğin çipleri kapı kapı simüle etmek yerine aynı davranışı gösteren \
                     hızlı bir modelle çalıştır. Çipin testi geçtiği için sonuç aynıdır, \
                     sadece çok daha hızlıdır. Kapalıyken her şey NAND seviyesinde simüle edilir.",
                    "Run solved chips with a fast model that behaves the same instead of \
                     gate by gate. The chip passed its test, so the result is the same, \
                     only much faster. When off, everything is simulated as NAND gates."
                ));
            if fast.changed() {
                self.refresh_library();
                self.rebuild();
                self.save();
            }
            theme::section(ui, tr!("Görünüm", "Look"));
            if ui
                .checkbox(
                    &mut self.progress.symbols,
                    tr!("Kapı sembolleri", "Gate symbols"),
                )
                .on_hover_text(tr!(
                    "NOT, AND, OR, XOR, NAND, MUX'u standart mantık sembolleriyle çiz",
                    "Draw NOT, AND, OR, XOR, NAND and MUX with standard logic symbols"
                ))
                .changed()
            {
                self.canvas.symbols = self.progress.symbols;
                self.save();
            }
            if ui
                .checkbox(
                    &mut self.progress.fullscreen,
                    tr!("Tam ekran (F11)", "Full screen (F11)"),
                )
                .changed()
            {
                let on = self.progress.fullscreen;
                self.set_fullscreen(ui.ctx(), on);
            }
            let mut system = !self.progress.custom_frame;
            if ui
                .checkbox(
                    &mut system,
                    tr!("Sistemin pencere çerçevesi", "System window frame"),
                )
                .on_hover_text(tr!(
                    "Kapalıyken makine kendi başlık çubuğunu çizer. Pencere taşınamıyor ya da \
                     boyutlanamıyorsa bunu aç.",
                    "When off, makine draws its own title bar. Turn this on if the window \
                     cannot be moved or resized."
                ))
                .changed()
            {
                self.progress.custom_frame = !system;
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Decorations(system));
                self.save();
            }
        });
        r.response.on_hover_text(tr!("Ayarlar", "Settings"));
    }

    pub(super) fn set_lang(&mut self, lang: Lang) {
        makine_lang::set(lang);
        if let Some(p) = &mut self.profiles {
            p.lang = Some(lang.code().into());
            let _ = p.save();
        }
        // Texts computed earlier (test results, notes) are in the old one.
        self.result = None;
        self.update_cost();
    }

    /// Without the system's frame, the window's edges resize it.
    pub(super) fn resize_edges(&self, ctx: &egui::Context) {
        if !self.progress.custom_frame {
            return;
        }
        let (full, max) = ctx.input(|i| {
            let v = i.viewport();
            (v.fullscreen.unwrap_or(false), v.maximized.unwrap_or(false))
        });
        if full || max {
            return;
        }
        let screen = ctx.content_rect();
        let Some(p) = ctx.pointer_hover_pos() else {
            return;
        };
        use egui::ResizeDirection as D;
        let (l, r, t, b) = (
            p.x - screen.left() < GRIP,
            screen.right() - p.x < GRIP,
            p.y - screen.top() < GRIP,
            screen.bottom() - p.y < GRIP,
        );
        let dir = match (l, r, t, b) {
            (true, _, true, _) => D::NorthWest,
            (_, true, true, _) => D::NorthEast,
            (true, _, _, true) => D::SouthWest,
            (_, true, _, true) => D::SouthEast,
            (true, ..) => D::West,
            (_, true, ..) => D::East,
            (_, _, true, _) => D::North,
            (.., true) => D::South,
            _ => return,
        };
        ctx.set_cursor_icon(match dir {
            D::NorthWest | D::SouthEast => CursorIcon::ResizeNwSe,
            D::NorthEast | D::SouthWest => CursorIcon::ResizeNeSw,
            D::West | D::East => CursorIcon::ResizeHorizontal,
            D::North | D::South => CursorIcon::ResizeVertical,
        });
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }

    /// A thin copper line around a frameless window, so it stands apart
    /// from what is behind it.
    pub(super) fn window_border(&self, ctx: &egui::Context) {
        if !self.progress.custom_frame
            || ctx.input(|i| {
                i.viewport().fullscreen.unwrap_or(false) || i.viewport().maximized.unwrap_or(false)
            })
        {
            return;
        }
        let r = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique("window_border"),
        ))
        .rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, theme::COPPER.gamma_multiply(0.6)),
            egui::StrokeKind::Inside,
        );
    }
}

/// A tab in the bar: text with a copper line under it when selected.
fn tab(ui: &mut Ui, on: bool, enabled: bool, text: &str) -> egui::Response {
    let font = FontId::proportional(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let size = vec2(galley.size().x + 22.0, BAR_HEIGHT - 8.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, r) = ui.allocate_exact_size(size, sense);
    let r = r.on_hover_cursor(if enabled {
        CursorIcon::PointingHand
    } else {
        CursorIcon::NotAllowed
    });
    let color = if !enabled {
        theme::INK_DIM.gamma_multiply(0.5)
    } else if on {
        theme::INK
    } else if r.hovered() {
        theme::COPPER_BRIGHT
    } else {
        theme::INK_DIM
    };
    let p = ui.painter();
    if r.hovered() && enabled && !on {
        p.rect_filled(rect.shrink(2.0), 4.0, theme::RAISED.gamma_multiply(0.6));
    }
    p.galley(
        pos2(rect.left() + 11.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    if on {
        let y = rect.bottom() + 3.0;
        p.line_segment(
            [pos2(rect.left() + 6.0, y), pos2(rect.right() - 6.0, y)],
            Stroke::new(2.5, theme::COPPER),
        );
    }
    // Text for screen readers and tests.
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, text));
    r
}

fn toggle_maximized(ctx: &egui::Context) {
    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
}

/// Minimize, maximize and close, right to left.
fn window_buttons(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let close = window_button(ui, Glyph::Close, tr!("Kapat", "Close"), true);
    if close.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let size = window_button(
        ui,
        if max { Glyph::Restore } else { Glyph::Maximize },
        if max {
            tr!("Küçült", "Restore")
        } else {
            tr!("Büyüt", "Maximize")
        },
        false,
    );
    if size.clicked() {
        toggle_maximized(&ctx);
    }
    let min = window_button(
        ui,
        Glyph::Minimize,
        tr!("Simge durumuna", "Minimize"),
        false,
    );
    if min.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

#[derive(Clone, Copy)]
enum Glyph {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// A window button drawn as a little chip pad: lines, not font glyphs.
fn window_button(ui: &mut Ui, glyph: Glyph, name: &str, danger: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(36.0, 26.0), Sense::click());
    let r = r.on_hover_text(name);
    let hover = r.hovered();
    let p = ui.painter();
    if hover {
        let fill = if danger {
            theme::BAD
        } else {
            theme::RAISED_HOVER
        };
        p.rect_filled(rect, 4.0, fill);
    }
    let ink = if hover && danger {
        Color32::WHITE
    } else if hover {
        theme::COPPER_BRIGHT
    } else {
        theme::INK_DIM
    };
    let s = Stroke::new(1.4, ink);
    let c = rect.center();
    let h = 5.0;
    match glyph {
        Glyph::Minimize => {
            p.line_segment([pos2(c.x - h, c.y + 3.0), pos2(c.x + h, c.y + 3.0)], s);
        }
        Glyph::Maximize => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(2.0 * h, 2.0 * h)),
                1.0,
                s,
                egui::StrokeKind::Middle,
            );
        }
        Glyph::Restore => {
            let back = Rect::from_center_size(c + vec2(2.0, -2.0), vec2(8.0, 8.0));
            let front = Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(8.0, 8.0));
            p.rect_stroke(back, 1.0, s, egui::StrokeKind::Middle);
            p.rect_filled(
                front,
                1.0,
                if hover {
                    theme::RAISED_HOVER
                } else {
                    theme::WELL
                },
            );
            p.rect_stroke(front, 1.0, s, egui::StrokeKind::Middle);
        }
        Glyph::Close => {
            p.line_segment([c + vec2(-h, -h), c + vec2(h, h)], s);
            p.line_segment([c + vec2(-h, h), c + vec2(h, -h)], s);
        }
    }
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    r
}
