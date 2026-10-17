//! Saving and loading the player's progress.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use makine_levels::levels;
use makine_sim::{ChipDef, Library};

use serde::{Deserialize, Serialize};

use crate::design::Design;
use crate::sandbox::Sandbox;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Work in progress, per level.
    #[serde(default)]
    pub designs: BTreeMap<String, Design>,
    /// The last version of each chip that passed its level's test. These make
    /// up the library the player builds with.
    #[serde(default)]
    pub solutions: BTreeMap<String, ChipDef>,
    /// Use fast models of solved chips.
    #[serde(default = "yes")]
    pub fast: bool,
    /// Last program loaded into the computer.
    #[serde(default)]
    pub program: Option<String>,
    /// Draw basic gates as logic symbols.
    #[serde(default = "yes")]
    pub symbols: bool,
    /// Start in full screen.
    #[serde(default)]
    pub fullscreen: bool,
    /// Free-form chips.
    #[serde(default)]
    pub sandbox: Sandbox,
}

fn yes() -> bool {
    true
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            designs: BTreeMap::new(),
            solutions: BTreeMap::new(),
            fast: true,
            program: None,
            symbols: true,
            fullscreen: false,
            sandbox: Sandbox::default(),
        }
    }
}

impl Progress {
    /// The chips the player has built: solved levels and sandbox chips.
    /// With `fast`, solved level chips are simulated by their native models
    /// instead of their gates.
    pub fn library(&self, fast: bool) -> Library {
        let mut lib = makine_levels::fast::library(self.solutions.values(), fast);
        for d in self.sandbox.chips.values() {
            lib.insert(d.def.clone());
        }
        lib
    }

    pub fn solved(&self, chip: &str) -> bool {
        self.solutions.contains_key(chip)
    }

    /// Levels are unlocked one at a time: a level is open once every level
    /// before it is solved.
    pub fn unlocked(&self, index: usize) -> bool {
        levels()[..index].iter().all(|l| self.solved(l.chip))
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Write atomically: a crash mid-save must not lose the old file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

pub fn default_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "makine").map(|d| d.data_dir().join("progress.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_levels::reference::reference;

    #[test]
    fn round_trip() {
        let mut p = Progress::default();
        let def = reference("Counter32");
        p.designs
            .insert("Counter32".into(), Design::new(def.clone()));
        p.solutions.insert("Not".into(), reference("Not"));
        let dir = std::env::temp_dir().join(format!("makine-test-{}", std::process::id()));
        let path = dir.join("p.json");
        p.save(&path).unwrap();
        assert_eq!(Progress::load(&path).unwrap(), p);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unlocking() {
        let mut p = Progress::default();
        assert!(p.unlocked(0));
        assert!(!p.unlocked(1));
        p.solutions.insert("Not".into(), reference("Not"));
        assert!(p.unlocked(1));
        assert!(!p.unlocked(2));
    }
}
