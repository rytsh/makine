//! Saved program drafts. A failed build never overwrites the running image
//! or loses the learner's source. IDs, not translated names, select drafts.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    #[default]
    RiscV,
    Mini,
    Mcs4,
    Custom,
}

impl Architecture {
    pub fn name(self) -> &'static str {
        match self {
            Self::RiscV => "RISC-V RV32IM",
            Self::Mini => "Mini CPU",
            Self::Mcs4 => "Intel 4004",
            Self::Custom => makine_lang::tr!("Kendi komut setim", "My instruction set"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub id: u64,
    pub name: String,
    pub architecture: Architecture,
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notebook {
    pub drafts: Vec<Draft>,
    pub selected: u64,
    pub solved: std::collections::BTreeSet<String>,
    pub custom: makine_rv::smallasm::Definition,
    pub custom_chip: String,
}

impl Notebook {
    pub fn add(&mut self, name: &str, architecture: Architecture, source: &str) -> u64 {
        let id = self
            .drafts
            .iter()
            .map(|p| p.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.drafts.push(Draft {
            id,
            name: name.into(),
            architecture,
            source: source.into(),
        });
        self.selected = id;
        id
    }

    pub fn ensure(&mut self, legacy: Option<&str>) {
        if self.drafts.is_empty() {
            self.add(
                makine_lang::tr!("İlk programım", "My first program"),
                Architecture::RiscV,
                legacy.unwrap_or("li a0, 2\nli a1, 3\nadd a0, a0, a1\nebreak\n"),
            );
        }
        if !self.drafts.iter().any(|d| d.id == self.selected) {
            self.selected = self.drafts[0].id;
        }
    }

    pub fn current(&self) -> Option<&Draft> {
        self.drafts.iter().find(|d| d.id == self.selected)
    }

    pub fn current_mut(&mut self) -> Option<&mut Draft> {
        self.drafts.iter_mut().find(|d| d.id == self.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drafts_survive_invalid_source_and_legacy_migrates_once() {
        let mut n = Notebook::default();
        n.ensure(Some("bad instruction"));
        n.add("other", Architecture::Mini, "LDI 4\nHLT");
        let text = serde_json::to_string(&n).unwrap();
        let mut loaded: Notebook = serde_json::from_str(&text).unwrap();
        loaded.ensure(Some("do not overwrite"));
        assert_eq!(n, loaded);
        assert_eq!(loaded.drafts[0].source, "bad instruction");
    }
}
