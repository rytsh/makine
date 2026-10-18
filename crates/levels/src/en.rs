//! English titles and descriptions of the levels, by chip name. Turkish
//! is in the level definitions themselves.

/// (title, description) of a level in English.
pub(crate) fn level(chip: &str) -> Option<(&'static str, &'static str)> {
    let _ = chip;
    None
}

/// English name of a section, by its Turkish name.
pub(crate) fn section(tr: &str) -> Option<&'static str> {
    let _ = tr;
    None
}
