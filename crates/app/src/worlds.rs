//! World files (`.makinedunya`): a world plus the free chips it uses, so a
//! friend can open it with nothing but the levels it needs solved.

use std::collections::{BTreeMap, BTreeSet};

use makine_pack::Package;
use makine_sim::Library;
use makine_world::{Block, Device, World};
use serde::{Deserialize, Serialize};

use crate::sandbox::{self, Sandbox};

pub const EXT: &str = "makinedunya";
const FORMAT: &str = "makine-world";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct WorldFile {
    format: String,
    version: u32,
    world: World,
    /// One package per free chip used directly by a chip block (each with
    /// its own dependencies).
    #[serde(default)]
    chips: Vec<Package>,
}

/// Chip names used by chip blocks in the world.
fn chip_names(world: &World) -> BTreeSet<String> {
    world
        .blocks()
        .filter_map(|(_, b)| match b {
            Block::Device {
                device: Device::Chip { name, .. },
                ..
            } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

pub fn export(world: &World, lib: &Library, sandbox: &Sandbox) -> Result<String, String> {
    let mut chips = Vec::new();
    for name in chip_names(world) {
        if sandbox.chips.contains_key(&name) {
            chips.push(sandbox::export(lib, sandbox, &name)?);
        }
    }
    let f = WorldFile {
        format: FORMAT.into(),
        version: VERSION,
        world: world.clone(),
        chips,
    };
    Ok(serde_json::to_string(&f).expect("worlds serialize"))
}

/// Read a world file: install the free chips it carries into `sandbox`
/// (skipping ones already there) and return the world.
pub fn import(text: &str, lib: &Library, sandbox: &mut Sandbox) -> Result<World, String> {
    let f: WorldFile =
        serde_json::from_str(text).map_err(|e| format!("geçersiz dünya dosyası: {e}"))?;
    if f.format != FORMAT {
        return Err("bu bir makine dünyası değil".into());
    }
    if f.version > VERSION {
        return Err("dosya daha yeni bir sürümden; makine'yi güncelle".into());
    }
    // Check every package first so a failure adds nothing.
    let mut trial = sandbox.clone();
    for p in &f.chips {
        let mut l = lib.clone();
        for d in trial.chips.values() {
            l.insert(d.def.clone());
        }
        sandbox::import(&l, &mut trial, &p.to_json())?;
    }
    let mut l = lib.clone();
    for d in trial.chips.values() {
        l.insert(d.def.clone());
    }
    let missing: Vec<String> = chip_names(&f.world)
        .into_iter()
        .filter(|n| !l.contains(n))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "bu dünya için önce şunlar gerekli: {}",
            missing.join(", ")
        ));
    }
    *sandbox = trial;
    Ok(f.world)
}

/// A name not in `taken`: `base`, then `base 2`, `base 3`…
pub fn unique_name(base: &str, taken: &BTreeMap<String, World>) -> String {
    if !taken.contains_key(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} {i}"))
        .find(|n| !taken.contains_key(n))
        .unwrap()
}

/// Valid world names: not empty, not too long, no line breaks.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("dünyanın bir adı olmalı".into());
    }
    if name.chars().count() > 40 || name.contains('\n') {
        return Err("ad çok uzun".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::Design;
    use makine_levels::reference::reference;
    use makine_sim::ChipBuilder;
    use makine_world::{Dir, Pos};

    fn lib() -> Library {
        let mut l = Library::new();
        l.insert(reference("Not"));
        l
    }

    #[test]
    fn worlds_carry_their_chips() {
        let mut sb = Sandbox::default();
        let mut b = ChipBuilder::new("Tampon", &["a"], &["q"]);
        let x = b.input("a");
        let y = b.chip1("Not", &[x]);
        let z = b.chip1("Not", &[y]);
        b.output("q", z);
        sb.chips.insert("Tampon".into(), Design::new(b.build()));
        let mut l = lib();
        l.insert(sb.chips["Tampon"].def.clone());

        let mut w = World::new("paylaş");
        let def = &sb.chips["Tampon"].def;
        let d = Device::Chip {
            name: "Tampon".into(),
            inputs: def.inputs.clone(),
            outputs: def.outputs.clone(),
        };
        w.apply(w.place_device(d, Pos::new(0, 0, 0), Dir::East).unwrap());
        let not = Device::Chip {
            name: "Not".into(),
            inputs: def.inputs.clone(),
            outputs: def.outputs.clone(),
        };
        w.apply(w.place_device(not, Pos::new(0, 5, 0), Dir::East).unwrap());
        let text = export(&w, &l, &sb).unwrap();

        // A friend with the Not level solved and no free chips.
        let mut friend = Sandbox::default();
        let got = import(&text, &lib(), &mut friend).unwrap();
        assert_eq!(got.blocks().count(), w.blocks().count());
        assert!(friend.chips.contains_key("Tampon"));
        // Without the Not level, nothing is added.
        let mut empty = Sandbox::default();
        let e = import(&text, &Library::new(), &mut empty).unwrap_err();
        assert!(e.contains("Not"), "{e}");
        assert!(empty.chips.is_empty());
        assert!(import("{}", &lib(), &mut empty).is_err());
    }

    #[test]
    fn names() {
        let mut m = BTreeMap::new();
        assert_eq!(unique_name("dünya", &m), "dünya");
        m.insert("dünya".into(), World::new("dünya"));
        m.insert("dünya 2".into(), World::new("dünya 2"));
        assert_eq!(unique_name("dünya", &m), "dünya 3");
        assert!(check_name(" ").is_err());
        assert!(check_name("Benim şehrim").is_ok());
    }
}
