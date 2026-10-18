//! The language of the user interface: Turkish or English.
//!
//! Text is written in both languages where it is used:
//!
//! ```
//! use makine_lang::{tr, trf};
//! let n = 3;
//! let a: &str = tr!("Test et", "Test");
//! let b: String = trf!("{n} kapı", "{n} gates");
//! ```
//!
//! The language is one setting for the whole program, read on every use,
//! so switching it changes the next frame.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    #[default]
    Tr,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Tr, Lang::En];

    /// Short code, as saved: `tr`, `en`.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Tr => "tr",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let code = code.to_ascii_lowercase();
        if code.starts_with("tr") {
            Some(Lang::Tr)
        } else if code.starts_with("en") {
            Some(Lang::En)
        } else {
            None
        }
    }

    /// The language's own name.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Tr => "Türkçe",
            Lang::En => "English",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

thread_local! {
    /// A language for this thread only, over the program's: lets tests
    /// running side by side use different languages.
    static THREAD: std::cell::Cell<Option<Lang>> = const { std::cell::Cell::new(None) };
}

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// Use `lang` on this thread only (`None`: the program's language again).
pub fn set_for_thread(lang: Option<Lang>) {
    THREAD.with(|t| t.set(lang));
}

pub fn get() -> Lang {
    if let Some(l) = THREAD.with(|t| t.get()) {
        return l;
    }
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::En,
        _ => Lang::Tr,
    }
}

/// The language of the computer's settings, if it is one we have.
pub fn from_system(locale: Option<&str>) -> Lang {
    locale.and_then(Lang::from_code).unwrap_or(Lang::Tr)
}

pub fn is_en() -> bool {
    get() == Lang::En
}

/// `tr` or `en`, in the current language.
pub fn pick<T>(tr: T, en: T) -> T {
    match get() {
        Lang::Tr => tr,
        Lang::En => en,
    }
}

/// A text in both languages: `tr!("Türkçe", "English")` is a `&'static str`.
#[macro_export]
macro_rules! tr {
    ($tr:expr, $en:expr $(,)?) => {
        $crate::pick($tr, $en)
    };
}

/// A formatted text in both languages, like `format!`:
/// `trf!("{n} kapı", "{n} gates")`. Both strings may use the same names.
#[macro_export]
macro_rules! trf {
    ($tr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::is_en() {
            format!($en $(, $arg)*)
        } else {
            format!($tr $(, $arg)*)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_and_formats() {
        let n = 3;
        assert_eq!(tr!("evet", "yes"), "evet");
        assert_eq!(trf!("{n} kapı", "{n} gates"), "3 kapı");
        set_for_thread(Some(Lang::En));
        assert_eq!(tr!("evet", "yes"), "yes");
        assert_eq!(trf!("{} kapı", "{} gates", n + 1), "4 gates");
        set_for_thread(None);
        assert_eq!(get(), Lang::Tr);
        assert_eq!(Lang::from_code("tr-TR"), Some(Lang::Tr));
        assert_eq!(Lang::from_code("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::from_code("de"), None);
    }
}
