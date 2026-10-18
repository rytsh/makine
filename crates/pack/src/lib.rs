//! Chip packages: sharing chips between players and worlds.
//!
//! A package is a JSON file (`.makine`) holding one chip plus every custom
//! chip it uses, so it can be loaded into an empty library. Built-in parts
//! (NAND, split, join, constants) and level chips (`Not`, `Adder32`, …) are
//! referred to by name; the receiver must have solved those levels, or the
//! package lists them as missing.
//!
//! The format is versioned and identified by a content hash, so a future
//! multiplayer server or world can tell whether two players have the same
//! chip without trusting names.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use makine_lang::trf;
use makine_sim::{ChipDef, Library, PartKind};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "makine-chip";
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub format: String,
    pub version: u32,
    /// Name of the main chip.
    pub chip: String,
    pub author: Option<String>,
    pub description: Option<String>,
    /// The main chip and every custom chip it uses, dependencies first.
    pub chips: Vec<ChipDef>,
    /// Chips used but not included: expected to exist in the receiver's
    /// library (level chips).
    pub requires: Vec<String>,
    /// Optional editor layout per chip: the position of each part. Purely
    /// cosmetic; not part of the fingerprint.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layout: BTreeMap<String, Vec<[f32; 2]>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PackError {
    UnknownChip(String),
    Recursive(String),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::UnknownChip(c) => {
                f.write_str(&trf!("bilinmeyen çip: {c}", "unknown chip: {c}"))
            }
            PackError::Recursive(c) => {
                f.write_str(&trf!("{c} kendi kendini içeriyor", "{c} contains itself"))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UnpackError {
    Format(String),
    /// Chips the package needs that the receiver does not have.
    Missing(Vec<String>),
    /// A chip in the package has the same name as a different chip the
    /// receiver already has.
    Conflict(String),
    Invalid(String),
}

impl fmt::Display for UnpackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnpackError::Format(e) => {
                f.write_str(&trf!("geçersiz çip dosyası: {e}", "invalid chip file: {e}"))
            }
            UnpackError::Missing(m) => f.write_str(&trf!(
                "bu çip için önce şunlar gerekli: {}",
                "this chip needs these first: {}",
                m.join(", ")
            )),
            UnpackError::Conflict(c) => f.write_str(&trf!(
                "'{c}' adında farklı bir çipin zaten var; birini yeniden adlandır",
                "you already have a different chip named '{c}'; rename one of them"
            )),
            UnpackError::Invalid(e) => f.write_str(&trf!(
                "çip derlenemiyor: {e}",
                "the chip can't be built: {e}"
            )),
        }
    }
}

/// Bundle `chip` with the custom chips it uses. `custom` is the set of
/// chips that belong to the player (and so must travel with the package);
/// every other chip name is listed in `requires`.
pub fn pack(
    lib: &Library,
    chip: &str,
    custom: &BTreeMap<String, ChipDef>,
) -> Result<Package, PackError> {
    let mut order = Vec::new();
    let mut requires = BTreeSet::new();
    let mut visiting = Vec::new();
    collect(lib, chip, custom, &mut order, &mut requires, &mut visiting)?;
    Ok(Package {
        format: FORMAT.into(),
        version: VERSION,
        chip: chip.into(),
        author: None,
        description: None,
        chips: order,
        requires: requires.into_iter().collect(),
        layout: BTreeMap::new(),
    })
}

fn collect(
    lib: &Library,
    name: &str,
    custom: &BTreeMap<String, ChipDef>,
    order: &mut Vec<ChipDef>,
    requires: &mut BTreeSet<String>,
    visiting: &mut Vec<String>,
) -> Result<(), PackError> {
    if order.iter().any(|c| c.name == name) {
        return Ok(());
    }
    if visiting.iter().any(|v| v == name) {
        return Err(PackError::Recursive(name.into()));
    }
    let Some(def) = custom.get(name) else {
        if lib.contains(name) {
            requires.insert(name.to_string());
            return Ok(());
        }
        return Err(PackError::UnknownChip(name.into()));
    };
    visiting.push(name.to_string());
    for part in &def.parts {
        if let PartKind::Chip(sub) = &part.kind {
            collect(lib, sub, custom, order, requires, visiting)?;
        }
    }
    visiting.pop();
    order.push(def.clone());
    Ok(())
}

impl Package {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("packages serialize")
    }

    pub fn from_json(text: &str) -> Result<Self, UnpackError> {
        let p: Package =
            serde_json::from_str(text).map_err(|e| UnpackError::Format(e.to_string()))?;
        if p.format != FORMAT {
            return Err(UnpackError::Format(trf!(
                "'{}' bir makine çipi değil",
                "'{}' is not a makine chip",
                p.format
            )));
        }
        if p.version > VERSION {
            return Err(UnpackError::Format(trf!(
                "dosya daha yeni bir sürümden (v{}); makine'yi güncelle",
                "the file is from a newer version (v{}); update makine",
                p.version
            )));
        }
        if !p.chips.iter().any(|c| c.name == p.chip) {
            return Err(UnpackError::Format(trf!(
                "ana çip '{}' dosyada yok",
                "the main chip '{}' is not in the file",
                p.chip
            )));
        }
        Ok(p)
    }

    pub fn main(&self) -> &ChipDef {
        self.chips
            .iter()
            .find(|c| c.name == self.chip)
            .expect("checked on load")
    }

    /// Check the package against the receiver's library and custom chips.
    /// Returns the chips to add (those the receiver does not already have
    /// in identical form).
    pub fn plan_install(
        &self,
        lib: &Library,
        custom: &BTreeMap<String, ChipDef>,
    ) -> Result<Vec<ChipDef>, UnpackError> {
        let included: BTreeSet<&str> = self.chips.iter().map(|c| c.name.as_str()).collect();
        let missing: Vec<String> = self
            .requires
            .iter()
            .filter(|r| !lib.contains(r) && !included.contains(r.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(UnpackError::Missing(missing));
        }
        let mut add = Vec::new();
        for c in &self.chips {
            match custom.get(&c.name) {
                Some(existing) if existing == c => {}
                Some(_) => return Err(UnpackError::Conflict(c.name.clone())),
                None if lib.contains(&c.name) => return Err(UnpackError::Conflict(c.name.clone())),
                None => add.push(c.clone()),
            }
        }
        // Everything must compile with the receiver's library.
        let mut trial = lib.clone();
        for c in &add {
            trial.insert(c.clone());
        }
        for c in &self.chips {
            makine_sim::Circuit::compile_def(&trial, c)
                .map_err(|e| UnpackError::Invalid(e.to_string()))?;
        }
        Ok(add)
    }

    /// A short, stable fingerprint of the chips' contents (not names of
    /// authors or descriptions). Two packages with the same fingerprint
    /// behave the same.
    pub fn fingerprint(&self) -> String {
        let canonical = serde_json::to_string(&self.chips).expect("chips serialize");
        format!("{:016x}", fnv1a(canonical.as_bytes()))
    }
}

/// FNV-1a, 64 bit: tiny and stable across platforms and versions.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_sim::{out, ChipBuilder};

    fn not_lib() -> Library {
        let mut lib = Library::new();
        let mut b = ChipBuilder::new("Not", &["in"], &["out"]);
        let x = b.input("in");
        let n = b.nand(x, x);
        b.output("out", n);
        lib.insert(b.build());
        lib
    }

    /// Custom chips: Buf = Not(Not(x)); Buf4 = Buf ×4.
    fn customs() -> BTreeMap<String, ChipDef> {
        let mut b = ChipBuilder::new("Buf", &["in"], &["out"]);
        let x = b.input("in");
        let y = b.chip1("Not", &[x]);
        let z = b.chip1("Not", &[y]);
        b.output("out", z);
        let buf = b.build();
        let mut b = ChipBuilder::new("Buf4", &["in[4]"], &["out[4]"]);
        let x = b.input("in");
        let p = b.chip_n("Buf", 4, &[x]);
        b.output("out", out(p, 0));
        let buf4 = b.build();
        [("Buf".into(), buf), ("Buf4".into(), buf4)]
            .into_iter()
            .collect()
    }

    fn lib_with(custom: &BTreeMap<String, ChipDef>) -> Library {
        let mut lib = not_lib();
        for c in custom.values() {
            lib.insert(c.clone());
        }
        lib
    }

    #[test]
    fn pack_collects_dependencies_in_order() {
        let custom = customs();
        let p = pack(&lib_with(&custom), "Buf4", &custom).unwrap();
        let names: Vec<&str> = p.chips.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Buf", "Buf4"]);
        assert_eq!(p.requires, ["Not"]);
    }

    #[test]
    fn round_trip_and_install() {
        let custom = customs();
        let p = pack(&lib_with(&custom), "Buf4", &custom).unwrap();
        let back = Package::from_json(&p.to_json()).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.fingerprint(), p.fingerprint());

        // A player with the Not level solved and no custom chips.
        let add = back.plan_install(&not_lib(), &BTreeMap::new()).unwrap();
        assert_eq!(add.len(), 2);
        // Installing twice is a no-op.
        let add = back.plan_install(&lib_with(&custom), &custom).unwrap();
        assert!(add.is_empty());
    }

    #[test]
    fn missing_and_conflicts() {
        let custom = customs();
        let p = pack(&lib_with(&custom), "Buf4", &custom).unwrap();
        assert_eq!(
            p.plan_install(&Library::new(), &BTreeMap::new()),
            Err(UnpackError::Missing(vec!["Not".into()]))
        );
        let mut other = BTreeMap::new();
        let mut d = ChipDef::new("Buf", &["in"], &["out"]);
        d.wires.push(makine_sim::Wire {
            from: makine_sim::Source::Input(0),
            to: makine_sim::Sink::Output(0),
        });
        other.insert("Buf".to_string(), d);
        assert_eq!(
            p.plan_install(&lib_with(&other), &other),
            Err(UnpackError::Conflict("Buf".into()))
        );
    }

    #[test]
    fn bad_files() {
        assert!(matches!(
            Package::from_json("{"),
            Err(UnpackError::Format(_))
        ));
        let custom = customs();
        let mut p = pack(&lib_with(&custom), "Buf4", &custom).unwrap();
        p.version = VERSION + 1;
        assert!(matches!(
            Package::from_json(&p.to_json()),
            Err(UnpackError::Format(_))
        ));
        p.version = VERSION;
        p.format = "other".into();
        assert!(matches!(
            Package::from_json(&p.to_json()),
            Err(UnpackError::Format(_))
        ));
    }

    #[test]
    fn fingerprint_ignores_metadata() {
        let custom = customs();
        let mut p = pack(&lib_with(&custom), "Buf4", &custom).unwrap();
        let f = p.fingerprint();
        p.author = Some("ray".into());
        p.description = Some("dört tampon".into());
        assert_eq!(p.fingerprint(), f);
        p.chips[0].name.push('x');
        assert_ne!(p.fingerprint(), f);
    }
}
