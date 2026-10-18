//! The in-app wiki: Markdown pages from `docs/wiki`, compiled into the
//! executable so it stays a single file.
//!
//! Each page starts with an HTML comment naming the levels it explains:
//!
//! ```text
//! <!--
//! seviyeler: Latch, DLatch
//! -->
//! ```
//!
//! Links between pages (`[x](hafiza.md)`) open the page in the wiki; other
//! links open in the browser. Images (`img/x.png`) are served from memory.
//!
//! The English version of each page is in `docs/wiki/en/`, under the same
//! name; the page shown follows the interface language.

use egui::{RichText, Ui};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

pub struct Page {
    /// File name without `.md`, e.g. `hafiza`.
    pub slug: &'static str,
    /// The Turkish page, which also names the levels.
    pub source: &'static str,
    pub en: &'static str,
}

impl Page {
    /// The page in the interface language.
    pub fn text(&self) -> &'static str {
        makine_lang::pick(self.source, self.en)
    }

    /// The page title: its first `# ` heading.
    pub fn title(&self) -> &'static str {
        self.text()
            .lines()
            .find_map(|l| l.strip_prefix("# "))
            .unwrap_or(self.slug)
            .trim()
    }

    /// Levels (chip names) this page explains, from its header comment.
    pub fn levels(&self) -> impl Iterator<Item = &'static str> {
        let header = self
            .source
            .strip_prefix("<!--")
            .and_then(|s| s.split_once("-->"))
            .map_or("", |(h, _)| h);
        header
            .lines()
            .filter_map(|l| l.trim().strip_prefix("seviyeler:"))
            .flat_map(|l| l.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// The Markdown without the header comment.
    fn body(&self) -> &'static str {
        body_of(self.text())
    }
}

fn body_of(source: &'static str) -> &'static str {
    match source
        .strip_prefix("<!--")
        .and_then(|s| s.split_once("-->"))
    {
        Some((_, rest)) => rest.trim_start(),
        None => source,
    }
}

macro_rules! pages {
    ($($slug:literal),* $(,)?) => {
        &[$(Page {
            slug: $slug,
            source: include_str!(concat!("../../../docs/wiki/", $slug, ".md")),
            en: include_str!(concat!("../../../docs/wiki/en/", $slug, ".md")),
        }),*]
    };
}

/// Pages in reading order.
pub static PAGES: &[Page] = pages![
    "baslarken",
    "ikilik",
    "transistor",
    "nand",
    "kapilar",
    "mux",
    "toplama",
    "cikarma",
    "hafiza",
    "register",
    "mini",
    "programlama",
    "alu",
    "verimlilik",
    "riscv",
    "islemci",
    "bilgisayar",
    "dunya",
    "moduller",
    "sinyaller",
    "atolye",
    "4004",
];

macro_rules! images {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../../../docs/wiki/img/", $name)) as &[u8])),*]
    };
}

static IMAGES: &[(&str, &[u8])] = images![
    "kapilar.png",
    "transistor.png",
    "cmos-nand.png",
    "nand-her-sey.png",
    "toplayici.png",
    "sr-mandal.png",
    "saat.png",
    "islemci.png",
    "ikilik.png",
    "foto-wafer.jpg",
    "foto-die.jpg",
    "foto-breadboard.jpg",
];

const IMAGE_SCHEME: &str = "bytes://wiki/";

pub fn page(slug: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.slug == slug)
}

/// The page explaining a level, if any.
pub fn page_for_level(chip: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.levels().any(|l| l == chip))
}

/// Make the wiki's images available to egui, and add fallback fonts for
/// the symbols the pages use (arrows, box drawing). Call once per context.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
    for (name, bytes) in IMAGES {
        ctx.include_bytes(format!("{IMAGE_SCHEME}img/{name}"), *bytes);
    }
    crate::theme::install(ctx);
}

pub struct Wiki {
    pub current: &'static Page,
    cache: CommonMarkCache,
    /// Pages visited before the current one, for "back".
    history: Vec<&'static Page>,
    scroll_to_top: bool,
}

impl Default for Wiki {
    fn default() -> Self {
        Self {
            current: &PAGES[0],
            cache: CommonMarkCache::default(),
            history: Vec::new(),
            scroll_to_top: false,
        }
    }
}

impl Wiki {
    pub fn open(&mut self, page: &'static Page) {
        if page.slug != self.current.slug {
            self.history.push(self.current);
            self.current = page;
            self.scroll_to_top = true;
        }
    }

    /// The page list, for the side panel.
    pub fn contents(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(makine_lang::tr!("İçindekiler", "Contents")).strong());
        let mut open = None;
        for (i, p) in PAGES.iter().enumerate() {
            let selected = p.slug == self.current.slug;
            let label = format!("{}. {}", i + 1, p.title());
            if ui.selectable_label(selected, label).clicked() && !selected {
                open = Some(p);
            }
        }
        if let Some(p) = open {
            self.open(p);
        }
    }

    /// The current page. Returns a level to open, if the reader clicked one.
    pub fn show(&mut self, ui: &mut Ui) -> Option<&'static str> {
        let page = self.current;
        let mut go_level = None;
        centered(ui, |ui| self.header(ui, page, &mut go_level));
        ui.separator();

        // Links to other pages are hooks; everything else opens normally.
        for p in PAGES {
            self.cache.add_link_hook(format!("{}.md", p.slug));
        }
        // The scroll area fills the panel, so its bar sits at the right edge;
        // the page is a centred column inside it.
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(("wiki", page.slug, makine_lang::get().code()))
            .auto_shrink([false, false]);
        if std::mem::take(&mut self.scroll_to_top) {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        scroll.show(ui, |ui| {
            centered(ui, |ui| {
                CommonMarkViewer::new()
                    .default_implicit_uri_scheme(IMAGE_SCHEME)
                    .max_image_width(Some(PAGE_WIDTH as usize))
                    .show(ui, &mut self.cache, page.body());
                ui.add_space(40.0);
            });
        });
        let clicked = PAGES
            .iter()
            .find(|p| self.cache.get_link_hook(&format!("{}.md", p.slug)) == Some(true));
        if let Some(p) = clicked {
            self.open(p);
        }
        go_level
    }

    /// Back and next buttons, and links to the page's levels.
    fn header(&mut self, ui: &mut Ui, page: &'static Page, go_level: &mut Option<&'static str>) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.history.is_empty(),
                    egui::Button::new(makine_lang::tr!("◀ Geri", "◀ Back")),
                )
                .clicked()
            {
                if let Some(p) = self.history.pop() {
                    self.current = p;
                    self.scroll_to_top = true;
                }
            }
            let idx = PAGES.iter().position(|p| p.slug == page.slug).unwrap_or(0);
            if let Some(next) = PAGES.get(idx + 1) {
                if ui
                    .button(makine_lang::trf!(
                        "Sonraki: {} ▶",
                        "Next: {} ▶",
                        next.title()
                    ))
                    .clicked()
                {
                    self.open(next);
                }
            }
        });
        let levels: Vec<&str> = page.levels().collect();
        if !levels.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(makine_lang::tr!("İlgili seviyeler:", "Related levels:")).weak(),
                );
                for l in levels {
                    let title = makine_levels::find(l).map_or(l, |lv| lv.title);
                    if ui.link(title).clicked() {
                        *go_level = Some(l);
                    }
                }
            });
        }
    }
}

/// Widest a wiki page gets; wider windows centre it.
const PAGE_WIDTH: f32 = 820.0;

/// Lay `add` out in a column at most [`PAGE_WIDTH`] wide, centred in `ui`.
fn centered(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    let column = width.min(PAGE_WIDTH);
    ui.horizontal_top(|ui| {
        ui.add_space((width - column) / 2.0);
        ui.vertical(|ui| {
            ui.set_width(column);
            add(ui);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_has_a_title() {
        for p in PAGES {
            assert!(p.source.contains("\n# "), "{} has no title", p.slug);
            assert!(!p.title().is_empty());
        }
    }

    /// Links between pages and to images must point at something that exists.
    /// Both languages: same header, same links and images.
    #[test]
    fn english_pages_match() {
        let links = |s: &str| {
            let mut v: Vec<String> = s
                .split("](")
                .skip(1)
                .map(|r| r[..r.find(')').unwrap()].to_string())
                .collect();
            v.sort();
            v
        };
        for p in PAGES {
            let head = |s: &'static str| s.len() - body_of(s).len();
            assert_eq!(
                p.source[..head(p.source)].trim(),
                p.en[..head(p.en)].trim(),
                "{}: header differs",
                p.slug
            );
            assert_eq!(links(p.source), links(p.en), "{}: links differ", p.slug);
            assert!(p.en.contains("\n# "), "{}: no English title", p.slug);
        }
    }

    #[test]
    fn links_resolve() {
        for p in PAGES {
            let body = p.body();
            let mut rest = body;
            while let Some(i) = rest.find("](") {
                rest = &rest[i + 2..];
                let target = &rest[..rest.find(')').unwrap()];
                if target.starts_with("http") || target.starts_with('#') || target.starts_with('<')
                {
                    continue;
                }
                if let Some(slug) = target.strip_suffix(".md") {
                    assert!(page(slug).is_some(), "{}: broken link {target}", p.slug);
                } else if let Some(img) = target.strip_prefix("img/") {
                    assert!(
                        IMAGES.iter().any(|(n, _)| *n == img),
                        "{}: image {target} not embedded",
                        p.slug
                    );
                } else {
                    panic!("{}: unexpected link {target}", p.slug);
                }
            }
        }
    }

    /// Every image file in docs/wiki/img is embedded, so none is forgotten.
    #[test]
    fn all_images_embedded() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/wiki/img");
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                IMAGES.iter().any(|(n, _)| *n == name),
                "{name} is not in IMAGES"
            );
        }
    }

    /// Level names in page headers are real levels, and every level has a page.
    #[test]
    fn levels_have_pages() {
        for p in PAGES {
            for l in p.levels() {
                assert!(
                    makine_levels::find(l).is_some(),
                    "{}: unknown level {l}",
                    p.slug
                );
            }
        }
        for l in makine_levels::levels() {
            assert!(
                page_for_level(l.chip).is_some(),
                "level {} has no wiki page",
                l.chip
            );
        }
    }

    /// Every character in the pages can be drawn: a missing glyph panics in
    /// tests and shows as a box in the app.
    #[test]
    fn pages_have_glyphs() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut missing = Vec::new();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            ui.ctx().fonts_mut(|f| {
                for p in PAGES {
                    for c in p.source.chars().filter(|c| !c.is_control()) {
                        for id in [
                            egui::FontId::proportional(14.0),
                            egui::FontId::monospace(14.0),
                        ] {
                            if !f.has_glyph(&id, c) && !missing.contains(&c) {
                                missing.push(c);
                            }
                        }
                    }
                }
            });
        });
        out.textures_delta.clear();
        assert!(missing.is_empty(), "no glyph for {missing:?}");
    }

    /// Every page is listed: no orphan Markdown files.
    #[test]
    fn all_pages_listed() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/wiki");
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            if let Some(slug) = name.strip_suffix(".md") {
                assert!(page(slug).is_some(), "{name} is not in PAGES");
            }
        }
    }
}
