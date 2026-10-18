//! Profiles: several players (or several attempts) on one computer, each
//! with its own progress file.
//!
//! `profiles.json` in the data directory names the profiles and the one
//! in use; profile `n` is saved in `profile-<n>.json`. The progress file
//! from before profiles (`progress.json`) becomes the first profile.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::Progress;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// File name in the data directory.
    pub file: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profiles {
    pub list: Vec<Profile>,
    /// Index into `list`.
    pub current: usize,
    /// Language of the interface (`tr`, `en`), for every profile; none: the
    /// computer's language.
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(skip)]
    dir: PathBuf,
}

pub const MAX_NAME: usize = 32;

impl Profiles {
    /// Read the profile list in `dir`, making the first profile from an old
    /// progress file (or empty) if there is none.
    pub fn load(dir: &Path) -> Profiles {
        let index = dir.join("profiles.json");
        if let Some(mut p) = std::fs::read_to_string(&index)
            .ok()
            .and_then(|t| serde_json::from_str::<Profiles>(&t).ok())
            .filter(|p| !p.list.is_empty())
        {
            p.current = p.current.min(p.list.len() - 1);
            p.dir = dir.to_path_buf();
            return p;
        }
        let mut p = Profiles {
            list: Vec::new(),
            current: 0,
            lang: None,
            dir: dir.to_path_buf(),
        };
        let old = dir.join("progress.json");
        let file = p.free_file();
        if old.exists() {
            // Keep the old file too, in case an older version runs again.
            let _ = std::fs::copy(&old, dir.join(&file));
        }
        p.list.push(Profile {
            name: "Ben".into(),
            file,
        });
        let _ = p.save();
        p
    }

    pub fn save(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let tmp = self.dir.join("profiles.json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, self.dir.join("profiles.json"))
    }

    pub fn current(&self) -> &Profile {
        &self.list[self.current]
    }

    pub fn path(&self, i: usize) -> PathBuf {
        self.dir.join(&self.list[i].file)
    }

    pub fn current_path(&self) -> PathBuf {
        self.path(self.current)
    }

    /// The progress of profile `i` (empty if it has none yet; an unreadable
    /// file is kept aside, as when starting).
    pub fn progress(&self, i: usize) -> Progress {
        let path = self.path(i);
        if !path.exists() {
            return Progress::default();
        }
        Progress::load(&path).unwrap_or_else(|e| {
            eprintln!("could not read {}: {e}; starting fresh", path.display());
            let _ = std::fs::rename(&path, path.with_extension("json.bad"));
            Progress::default()
        })
    }

    fn free_file(&self) -> String {
        (1..)
            .map(|n| format!("profile-{n}.json"))
            .find(|f| self.list.iter().all(|p| &p.file != f) && !self.dir.join(f).exists())
            .unwrap()
    }

    /// Check a new name for profile `except` (none: a new profile).
    pub fn check_name(&self, name: &str, except: Option<usize>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(makine_lang::tr!("Profile bir ad ver.", "Give the profile a name.").into());
        }
        if name.chars().count() > MAX_NAME {
            return Err(makine_lang::trf!(
                "Ad en çok {MAX_NAME} harf olabilir.",
                "A name can be at most {MAX_NAME} letters."
            ));
        }
        if self
            .list
            .iter()
            .enumerate()
            .any(|(i, p)| Some(i) != except && p.name.to_lowercase() == name.to_lowercase())
        {
            return Err(makine_lang::tr!(
                "Bu adda bir profil zaten var.",
                "There is already a profile with this name."
            )
            .into());
        }
        Ok(name.to_string())
    }

    /// Add a profile with `progress`. Returns its index.
    pub fn add(&mut self, name: &str, progress: &Progress) -> Result<usize, String> {
        let name = self.check_name(name, None)?;
        let file = self.free_file();
        let profile = Profile { name, file };
        self.list.push(profile);
        let i = self.list.len() - 1;
        if let Err(e) = progress.save(&self.path(i)).and_then(|_| self.save()) {
            self.list.pop();
            return Err(format!("Kaydedilemedi: {e}"));
        }
        Ok(i)
    }

    pub fn rename(&mut self, i: usize, name: &str) -> Result<(), String> {
        let name = self.check_name(name, Some(i))?;
        self.list[i].name = name;
        self.save().map_err(|e| format!("Kaydedilemedi: {e}"))
    }

    /// Remove profile `i` (not the last one). Its file is kept as `.deleted`
    /// so a mistake can be undone by hand.
    pub fn remove(&mut self, i: usize) -> Result<(), String> {
        if self.list.len() <= 1 {
            return Err(makine_lang::tr!(
                "Son profil silinemez.",
                "The last profile cannot be deleted."
            )
            .into());
        }
        let path = self.path(i);
        if path.exists() {
            let _ = std::fs::rename(&path, path.with_extension("json.deleted"));
        }
        self.list.remove(i);
        if self.current > i || self.current >= self.list.len() {
            self.current = self.current.saturating_sub(1);
        }
        self.save().map_err(|e| format!("Kaydedilemedi: {e}"))
    }
}

/// Progress as text to paste somewhere else, and back.
pub fn export(progress: &Progress) -> String {
    serde_json::to_string(progress).unwrap_or_default()
}

pub fn import(text: &str) -> Result<Progress, String> {
    serde_json::from_str(text.trim()).map_err(|_| {
        makine_lang::tr!(
            "Bu bir makine profili değil (ya da eksik kopyalanmış).",
            "This is not a makine profile (or it was not copied whole)."
        )
        .to_string()
    })
}

/// How far a profile has got, for the list.
pub fn summary(p: &Progress) -> String {
    let all = makine_levels::levels();
    let done = all.iter().filter(|l| p.solved(l.chip)).count();
    let mut s = makine_lang::trf!("{done}/{} seviye", "{done}/{} levels", all.len());
    if !p.sandbox.chips.is_empty() {
        s += &makine_lang::trf!(" · {} çip", " · {} chips", p.sandbox.chips.len());
    }
    if !p.worlds.is_empty() {
        s += &makine_lang::trf!(" · {} dünya", " · {} worlds", p.worlds.len());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_levels::reference::reference;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("makine-prof-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn old_progress_becomes_first_profile() {
        let d = dir("old");
        let mut p = Progress::default();
        p.solutions.insert("Not".into(), reference("Not"));
        p.save(&d.join("progress.json")).unwrap();
        let profiles = Profiles::load(&d);
        assert_eq!(profiles.list.len(), 1);
        assert!(profiles.progress(0).solved("Not"));
        // Loading again reads the index, not the old file.
        assert_eq!(Profiles::load(&d), profiles);
    }

    #[test]
    fn add_rename_remove_and_copy() {
        let d = dir("ops");
        let mut profiles = Profiles::load(&d);
        let mut p = Progress::default();
        p.solutions.insert("Not".into(), reference("Not"));
        let text = export(&p);
        let i = profiles.add("Ayşe", &import(&text).unwrap()).unwrap();
        assert!(profiles.progress(i).solved("Not"));
        assert!(profiles.add("ayşe", &p).is_err());
        assert!(profiles.add("  ", &p).is_err());
        profiles.rename(i, "Ali").unwrap();
        assert!(profiles.rename(0, "ali").is_err());
        profiles.current = i;
        profiles.remove(0).unwrap();
        assert_eq!(profiles.current().name, "Ali");
        assert!(profiles.remove(0).is_err());
        let again = Profiles::load(&d);
        assert_eq!(again.list.len(), 1);
        assert_eq!(again.current().name, "Ali");
        assert!(import("merhaba").is_err());
    }
}
