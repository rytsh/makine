//! The look of the app: a circuit board on a workbench.
//!
//! Dark green solder mask for the boards (canvas, world), a near-black
//! green for the panels around them, copper for what the player acts on,
//! off-white silkscreen for text and outlines, and amber for a signal that
//! is 1. Type is IBM Plex Sans, with Plex Mono for values.

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText, Shadow,
    Stroke, Style, TextStyle, Ui, Visuals,
};

/// Panels around the boards.
pub const PANEL: Color32 = Color32::from_rgb(17, 22, 20);
/// Inputs, code and other recessed fields.
pub const WELL: Color32 = Color32::from_rgb(10, 14, 12);
/// The board (solder mask) under circuits.
pub const BOARD: Color32 = Color32::from_rgb(14, 30, 24);
/// Grid lines and dots on the board.
pub const BOARD_GRID: Color32 = Color32::from_rgb(28, 50, 41);
/// Raised surfaces: buttons, part bodies.
pub const RAISED: Color32 = Color32::from_rgb(30, 40, 36);
pub const RAISED_HOVER: Color32 = Color32::from_rgb(40, 54, 48);
pub const LINE: Color32 = Color32::from_rgb(44, 58, 52);

/// Silkscreen: text and outlines.
pub const INK: Color32 = Color32::from_rgb(230, 232, 222);
pub const INK_DIM: Color32 = Color32::from_rgb(146, 162, 152);

/// What the player acts on: selection, focus, the current tool.
pub const COPPER: Color32 = Color32::from_rgb(214, 140, 74);
pub const COPPER_BRIGHT: Color32 = Color32::from_rgb(240, 170, 104);
/// Text on copper.
pub const ON_COPPER: Color32 = Color32::from_rgb(28, 18, 10);

/// A 1-bit signal that is 1.
pub const SIGNAL: Color32 = Color32::from_rgb(255, 190, 84);
/// A wire carrying 0: unlit copper.
pub const SIGNAL_OFF: Color32 = Color32::from_rgb(112, 80, 56);
/// Buses (several bits).
pub const BUS: Color32 = Color32::from_rgb(110, 182, 230);

pub const GOOD: Color32 = Color32::from_rgb(120, 214, 140);
pub const WARN: Color32 = Color32::from_rgb(240, 200, 110);
pub const BAD: Color32 = Color32::from_rgb(242, 112, 96);

/// The font family of headings and strong labels.
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Plex for text, then DejaVu (and egui's own) for symbols Plex lacks.
fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let mut add = |name: &str, data: &'static [u8]| {
        f.font_data
            .insert(name.into(), FontData::from_static(data).into());
    };
    add(
        "PlexSans",
        include_bytes!("../fonts/IBMPlexSans-Regular.ttf"),
    );
    add(
        "PlexSansSemiBold",
        include_bytes!("../fonts/IBMPlexSans-SemiBold.ttf"),
    );
    add(
        "PlexMono",
        include_bytes!("../fonts/IBMPlexMono-Regular.ttf"),
    );
    add("DejaVuSans", include_bytes!("../fonts/DejaVuSans.ttf"));
    add(
        "DejaVuSansMono",
        include_bytes!("../fonts/DejaVuSansMono.ttf"),
    );
    let fallbacks = f.families[&FontFamily::Proportional].clone();
    let with = |first: &[&str], rest: &[String]| -> Vec<String> {
        first
            .iter()
            .map(|s| s.to_string())
            .chain(rest.iter().cloned())
            .chain(["DejaVuSans".to_string()])
            .collect()
    };
    let mono_rest = f.families[&FontFamily::Monospace].clone();
    f.families
        .insert(FontFamily::Proportional, with(&["PlexSans"], &fallbacks));
    f.families
        .insert(bold(), with(&["PlexSansSemiBold", "PlexSans"], &fallbacks));
    let mut mono = with(&["PlexMono"], &mono_rest);
    mono.push("DejaVuSansMono".into());
    f.families.insert(FontFamily::Monospace, mono);
    f
}

fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    let r = CornerRadius::same(3);
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(5);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };
    v.extreme_bg_color = WELL;
    v.text_edit_bg_color = Some(WELL);
    v.faint_bg_color = Color32::from_rgb(22, 29, 26);
    v.code_bg_color = WELL;
    v.hyperlink_color = COPPER_BRIGHT;
    v.warn_fg_color = WARN;
    v.error_fg_color = BAD;
    v.override_text_color = None;
    v.weak_text_color = Some(INK_DIM);
    v.selection.bg_fill = COPPER;
    v.selection.stroke = Stroke::new(1.0, ON_COPPER);
    v.text_cursor.stroke = Stroke::new(2.0, COPPER_BRIGHT);
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = true;
    v.striped = true;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    w.noninteractive.corner_radius = r;

    w.inactive.bg_fill = RAISED;
    w.inactive.weak_bg_fill = RAISED;
    w.inactive.bg_stroke = Stroke::new(1.0, LINE);
    w.inactive.fg_stroke = Stroke::new(1.0, INK);
    w.inactive.corner_radius = r;

    w.hovered.bg_fill = RAISED_HOVER;
    w.hovered.weak_bg_fill = RAISED_HOVER;
    w.hovered.bg_stroke = Stroke::new(1.0, COPPER);
    w.hovered.fg_stroke = Stroke::new(1.5, INK);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = COPPER;
    w.active.weak_bg_fill = COPPER;
    w.active.bg_stroke = Stroke::new(1.0, COPPER_BRIGHT);
    // Also the colour of strong text.
    w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;

    w.open.bg_fill = RAISED_HOVER;
    w.open.weak_bg_fill = RAISED_HOVER;
    w.open.bg_stroke = Stroke::new(1.0, COPPER);
    w.open.fg_stroke = Stroke::new(1.0, INK);
    w.open.corner_radius = r;
    v
}

fn style(s: &mut Style) {
    s.visuals = visuals();
    s.text_styles = [
        (TextStyle::Heading, FontId::new(21.0, bold())),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.5, FontFamily::Monospace),
        ),
    ]
    .into();
    let sp = &mut s.spacing;
    sp.item_spacing = egui::vec2(8.0, 6.0);
    sp.button_padding = egui::vec2(9.0, 4.0);
    sp.interact_size.y = 24.0;
    sp.indent = 16.0;
    sp.window_margin = Margin::same(12);
    sp.menu_margin = Margin::same(6);
}

/// Fonts and style; once, on the first frame.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.all_styles_mut(style);
}

/// A section title in a side panel: small, spaced, in copper, with room
/// above it.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(
        RichText::new(title.to_uppercase())
            .font(FontId::new(11.0, bold()))
            .color(COPPER)
            .extra_letter_spacing(1.2),
    );
    ui.add_space(2.0);
}

/// Text in the heading family, at `size`.
pub fn strong(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, bold()))
}
