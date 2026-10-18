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
    #[serde(default)]
    pub isa: Option<makine_rv::smallasm::Definition>,
    #[serde(default)]
    pub cpu_chip: String,
    /// Stable association; navigation reuses the draft without overwriting it.
    #[serde(default)]
    pub activity: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lab {
    pub chip: Option<String>,
    pub draft: Option<u64>,
    pub rom: Option<u64>,
    pub loaded_words: Vec<u64>,
    pub loaded_source: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notebook {
    pub drafts: Vec<Draft>,
    pub selected: u64,
    pub solved: std::collections::BTreeSet<String>,
    pub custom: makine_rv::smallasm::Definition,
    pub custom_chip: String,
    pub isa_solved: std::collections::BTreeSet<String>,
    pub lesson_notes: std::collections::BTreeMap<String, String>,
    pub labs: std::collections::BTreeMap<String, Lab>,
}

impl Notebook {
    pub fn add(&mut self, name: &str, architecture: Architecture, source: &str) -> u64 {
        self.sync_custom();
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
            activity: None,
            isa: (architecture == Architecture::Custom).then(|| self.custom.clone()),
            cpu_chip: if architecture == Architecture::Custom {
                self.custom_chip.clone()
            } else {
                String::new()
            },
        });
        self.selected = id;
        id
    }

    pub fn ensure(&mut self, legacy: Option<&str>) {
        for d in &mut self.drafts {
            if d.architecture == Architecture::Custom && d.isa.is_none() {
                d.isa = Some(self.custom.clone());
                d.cpu_chip = self.custom_chip.clone();
            }
        }
        if self.drafts.is_empty() {
            self.add(
                makine_lang::tr!("İlk programım", "My first program"),
                Architecture::RiscV,
                legacy.unwrap_or("li a0, 42\nebreak\n"),
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

    pub fn sync_custom(&mut self) {
        let (definition, chip) = (self.custom.clone(), self.custom_chip.clone());
        if let Some(d) = self
            .current_mut()
            .filter(|d| d.architecture == Architecture::Custom)
        {
            d.isa = Some(definition);
            d.cpu_chip = chip;
        }
    }

    pub fn select(&mut self, id: u64) {
        self.sync_custom();
        self.selected = id;
        if let Some(d) = self
            .current()
            .filter(|d| d.architecture == Architecture::Custom)
            .cloned()
        {
            if let Some(def) = d.isa {
                self.custom = def;
            }
            self.custom_chip = d.cpu_chip;
        }
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

    #[test]
    fn instruction_sets_are_independent_per_draft() {
        let mut n = Notebook::default();
        n.ensure(None);
        let one = n.add("one", Architecture::Custom, "YUKLE 1");
        n.custom.instructions[1].opcode = 2;
        n.custom_chip = "FirstCpu".into();
        let two = n.add("two", Architecture::Custom, "YUKLE 3");
        n.custom.instructions[1].opcode = 7;
        n.custom_chip = "SecondCpu".into();
        n.select(one);
        assert_eq!(n.custom.instructions[1].opcode, 2);
        assert_eq!(n.custom_chip, "FirstCpu");
        n.select(two);
        assert_eq!(n.custom.instructions[1].opcode, 7);
        assert_eq!(n.custom_chip, "SecondCpu");
        n.sync_custom();
        let text = serde_json::to_string(&n).unwrap();
        let mut saved: Notebook = serde_json::from_str(&text).unwrap();
        saved.ensure(None);
        saved.select(one);
        assert_eq!(saved.custom_chip, "FirstCpu");
    }
}
