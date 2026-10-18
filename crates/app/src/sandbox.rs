//! Sandbox: free-form chips the player designs without a level, and
//! sharing them as packages.
//!
//! A sandbox chip may use every solved level chip, every other sandbox chip
//! (except itself or chips that use it) and all built-in parts. Its pins are
//! whatever the player defines.

use std::collections::BTreeMap;

use egui::{RichText, Ui};
use makine_pack::Package;
use makine_sim::{ChipDef, Library, PartKind, Pin, MAX_WIDTH};
use serde::{Deserialize, Serialize};

use crate::design::Design;

/// File extension for chip packages.
pub const EXT: &str = "makine";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sandbox {
    /// Chips by name, with their canvas layout.
    #[serde(default)]
    pub chips: BTreeMap<String, Design>,
}

impl Sandbox {
    /// The sandbox chip definitions.
    pub fn defs(&self) -> BTreeMap<String, ChipDef> {
        self.chips
            .iter()
            .map(|(n, d)| (n.clone(), d.def.clone()))
            .collect()
    }

    /// Whether chip `user` uses `used`, directly or through other sandbox
    /// chips. Placing `used` inside `user` would then be a cycle.
    pub fn uses(&self, user: &str, used: &str) -> bool {
        let mut stack = vec![user.to_string()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(c) = stack.pop() {
            if c == used {
                return true;
            }
            if !seen.insert(c.clone()) {
                continue;
            }
            if let Some(d) = self.chips.get(&c) {
                for p in &d.def.parts {
                    if let PartKind::Chip(n) = &p.kind {
                        stack.push(n.clone());
                    }
                }
            }
        }
        false
    }

    /// Chips that use `name`; they would break if it were deleted or renamed.
    pub fn users_of(&self, name: &str) -> Vec<String> {
        self.chips
            .keys()
            .filter(|c| c.as_str() != name && self.uses(c, name))
            .cloned()
            .collect()
    }

    /// Rename a chip and every reference to it.
    pub fn rename(&mut self, from: &str, to: &str) {
        if let Some(mut d) = self.chips.remove(from) {
            d.def.name = to.to_string();
            self.chips.insert(to.to_string(), d);
        }
        for d in self.chips.values_mut() {
            for p in &mut d.def.parts {
                if let PartKind::Chip(n) = &mut p.kind {
                    if n == from {
                        *n = to.to_string();
                    }
                }
            }
        }
    }

    /// Change a chip's pins, keeping wires to pins that still exist (by
    /// name) and dropping the rest. Chips that use it lose wires to removed
    /// pins too.
    pub fn set_pins(&mut self, name: &str, inputs: Vec<Pin>, outputs: Vec<Pin>) {
        let Some(d) = self.chips.get_mut(name) else {
            return;
        };
        let old_in = std::mem::replace(&mut d.def.inputs, inputs);
        let old_out = std::mem::replace(&mut d.def.outputs, outputs);
        let new_in = d.def.inputs.clone();
        let new_out = d.def.outputs.clone();
        let remap = |old: &[Pin], new: &[Pin], i: usize| -> Option<usize> {
            let n = &old.get(i)?.name;
            new.iter().position(|p| &p.name == n)
        };
        // Inside the chip: wires from its inputs and to its outputs.
        d.def.wires.retain_mut(|w| {
            if let makine_sim::Source::Input(i) = &mut w.from {
                match remap(&old_in, &new_in, *i) {
                    Some(j) => *i = j,
                    None => return false,
                }
            }
            if let makine_sim::Sink::Output(i) = &mut w.to {
                match remap(&old_out, &new_out, *i) {
                    Some(j) => *i = j,
                    None => return false,
                }
            }
            true
        });
        d.input_pos = remap_positions(&d.input_pos, &old_in, &new_in);
        d.output_pos = remap_positions(&d.output_pos, &old_out, &new_out);
        // Users: wires to/from this chip's parts.
        for user in self.users_of(name) {
            let ud = self.chips.get_mut(&user).unwrap();
            let parts: Vec<bool> = ud
                .def
                .parts
                .iter()
                .map(|p| p.kind == PartKind::Chip(name.into()))
                .collect();
            ud.def.wires.retain_mut(|w| {
                if let makine_sim::Source::Part { part, pin } = &mut w.from {
                    if parts[*part] {
                        match remap(&old_out, &new_out, *pin) {
                            Some(j) => *pin = j,
                            None => return false,
                        }
                    }
                }
                if let makine_sim::Sink::Part { part, pin } = &mut w.to {
                    if parts[*part] {
                        match remap(&old_in, &new_in, *pin) {
                            Some(j) => *pin = j,
                            None => return false,
                        }
                    }
                }
                true
            });
        }
    }
}

fn remap_positions(pos: &[Option<[f32; 2]>], old: &[Pin], new: &[Pin]) -> Vec<Option<[f32; 2]>> {
    new.iter()
        .map(|p| {
            old.iter()
                .position(|o| o.name == p.name)
                .and_then(|i| pos.get(i).copied().flatten())
        })
        .collect()
}

/// Valid chip names: a letter, then letters, digits or `_`. They must not
/// clash with built-in or level chips.
pub fn check_name(
    name: &str,
    lib: &Library,
    sandbox: &Sandbox,
    current: Option<&str>,
) -> Result<(), String> {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() => {}
        _ => {
            return Err(makine_lang::tr!(
                "ad bir harfle başlamalı",
                "a name must start with a letter"
            )
            .into())
        }
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_') {
        return Err(makine_lang::tr!(
            "adda sadece harf, rakam ve _ olabilir",
            "a name can only have letters, digits and _"
        )
        .into());
    }
    if name.len() > 40 {
        return Err(makine_lang::tr!("ad çok uzun", "the name is too long").into());
    }
    if Some(name) == current {
        return Ok(());
    }
    if makine_levels::find(name).is_some() || name == makine_sim::NAND || lib.native(name).is_some()
    {
        return Err(makine_lang::trf!(
            "'{name}' bir seviye çipinin adı",
            "'{name}' is the name of a level chip"
        ));
    }
    if sandbox.chips.contains_key(name) {
        return Err(makine_lang::trf!(
            "'{name}' adında bir çipin zaten var",
            "you already have a chip named '{name}'"
        ));
    }
    Ok(())
}

/// Parse a comma-separated pin list like `a, b[8], sel`.
pub fn parse_pin_list(text: &str) -> Result<Vec<Pin>, String> {
    let mut pins: Vec<Pin> = Vec::new();
    for spec in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, width) = match spec.split_once('[') {
            None => (spec, 1u32),
            Some((n, rest)) => {
                let w = rest
                    .strip_suffix(']')
                    .and_then(|w| w.trim().parse::<u32>().ok())
                    .ok_or_else(|| {
                        makine_lang::trf!(
                            "'{spec}': genişlik [1..64] olmalı",
                            "'{spec}': the width must be [1..64]"
                        )
                    })?;
                (n.trim(), w)
            }
        };
        if !(1..=MAX_WIDTH as u32).contains(&width) {
            return Err(makine_lang::trf!(
                "'{spec}': genişlik 1 ile 64 arasında olmalı",
                "'{spec}': the width must be between 1 and 64"
            ));
        }
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(makine_lang::trf!(
                "'{spec}': geçersiz uç adı",
                "'{spec}': not a valid pin name"
            ));
        }
        if pins.iter().any(|p| p.name == name) {
            return Err(makine_lang::trf!(
                "'{name}' iki kez yazılmış",
                "'{name}' is written twice"
            ));
        }
        pins.push(Pin::new(name, width as u8));
    }
    Ok(pins)
}

pub fn pins_text(pins: &[Pin]) -> String {
    pins.iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A new, empty sandbox chip with two inputs and one output.
pub fn new_chip(name: &str) -> Design {
    Design::new(ChipDef::new(name, &["a", "b"], &["out"]))
}

/// Build a package for `name`, with the editor layout of each included chip.
pub fn export(lib: &Library, sandbox: &Sandbox, name: &str) -> Result<Package, String> {
    let mut p = makine_pack::pack(lib, name, &sandbox.defs()).map_err(|e| e.to_string())?;
    for c in &p.chips {
        if let Some(d) = sandbox.chips.get(&c.name) {
            p.layout.insert(c.name.clone(), d.pos.clone());
        }
    }
    Ok(p)
}

/// Add the chips of a package file to the sandbox. Returns the main chip's
/// name.
pub fn import(lib: &Library, sandbox: &mut Sandbox, text: &str) -> Result<String, String> {
    let p = Package::from_json(text).map_err(|e| e.to_string())?;
    install(lib, sandbox, &p)
}

/// Add a package's chips to the sandbox. Returns the main chip's name.
pub fn install(lib: &Library, sandbox: &mut Sandbox, p: &Package) -> Result<String, String> {
    let add = p
        .plan_install(lib, &sandbox.defs())
        .map_err(|e| e.to_string())?;
    for def in add {
        let mut d = Design::new(def.clone());
        if let Some(pos) = p.layout.get(&def.name) {
            if pos.len() == d.pos.len() {
                d.pos = pos.clone();
            }
        }
        sandbox.chips.insert(def.name.clone(), d);
    }
    Ok(p.chip.clone())
}

/// Pin editor for a sandbox chip. Returns new pins when the player applies
/// a valid change.
pub struct PinEditor {
    pub inputs: String,
    pub outputs: String,
    pub error: Option<String>,
}

impl PinEditor {
    pub fn new(def: &ChipDef) -> Self {
        Self {
            inputs: pins_text(&def.inputs),
            outputs: pins_text(&def.outputs),
            error: None,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<(Vec<Pin>, Vec<Pin>)> {
        let mut apply = false;
        egui::Grid::new("pins").num_columns(2).show(ui, |ui| {
            let l = ui.label(makine_lang::tr!("Girişler", "Inputs")).id;
            ui.add(egui::TextEdit::singleline(&mut self.inputs).hint_text("a, b[8]"))
                .labelled_by(l);
            ui.end_row();
            let l = ui.label(makine_lang::tr!("Çıkışlar", "Outputs")).id;
            ui.add(egui::TextEdit::singleline(&mut self.outputs).hint_text("out"))
                .labelled_by(l);
            ui.end_row();
        });
        ui.label(
            RichText::new(makine_lang::tr!(
                "Virgülle ayır; bus için genişlik: sayi[32]",
                "Separate with commas; width for a bus: num[32]"
            ))
            .weak()
            .small(),
        );
        if ui
            .button(makine_lang::tr!("Uçları uygula", "Apply pins"))
            .clicked()
        {
            apply = true;
        }
        if let Some(e) = &self.error {
            ui.colored_label(crate::theme::BAD, e);
        }
        if !apply {
            return None;
        }
        match (parse_pin_list(&self.inputs), parse_pin_list(&self.outputs)) {
            (Ok(i), Ok(o)) => {
                self.error = None;
                Some((i, o))
            }
            (Err(e), _) | (_, Err(e)) => {
                self.error = Some(e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_levels::reference::reference;
    use makine_sim::{into, out, ChipBuilder, Sink, Source};

    fn lib() -> Library {
        let mut lib = Library::new();
        lib.insert(reference("Not"));
        lib
    }

    fn sandbox() -> Sandbox {
        let mut s = Sandbox::default();
        let mut b = ChipBuilder::new("Buf", &["in"], &["out"]);
        let x = b.input("in");
        let y = b.chip1("Not", &[x]);
        let z = b.chip1("Not", &[y]);
        b.output("out", z);
        s.chips.insert("Buf".into(), Design::new(b.build()));
        let mut b = ChipBuilder::new("Two", &["a", "b"], &["x", "y"]);
        let (a, bb) = (b.input("a"), b.input("b"));
        let p = b.chip1("Buf", &[a]);
        let q = b.chip1("Buf", &[bb]);
        b.output("x", p);
        b.output("y", q);
        s.chips.insert("Two".into(), Design::new(b.build()));
        s
    }

    #[test]
    fn pin_lists() {
        let p = parse_pin_list("a, b[8], sel").unwrap();
        assert_eq!(p, makine_sim::parse_pins(&["a", "b[8]", "sel"]));
        assert!(parse_pin_list("a, a").is_err());
        assert!(parse_pin_list("x[65]").is_err());
        assert!(parse_pin_list("x[0]").is_err());
        assert!(parse_pin_list("bad name").is_err());
        assert_eq!(parse_pin_list("").unwrap(), vec![]);
        assert_eq!(pins_text(&p), "a, b[8], sel");
    }

    #[test]
    fn names() {
        let s = sandbox();
        assert!(check_name("Yeni", &lib(), &s, None).is_ok());
        assert!(check_name("Not", &lib(), &s, None).is_err());
        assert!(check_name("Adder32", &lib(), &s, None).is_err());
        assert!(check_name("Buf", &lib(), &s, None).is_err());
        assert!(check_name("Buf", &lib(), &s, Some("Buf")).is_ok());
        assert!(check_name("1x", &lib(), &s, None).is_err());
        assert!(check_name("a b", &lib(), &s, None).is_err());
        assert!(check_name("Çarpıcı", &lib(), &s, None).is_ok());
    }

    #[test]
    fn cycles_and_users() {
        let s = sandbox();
        assert!(s.uses("Two", "Buf"));
        assert!(!s.uses("Buf", "Two"));
        assert!(s.uses("Buf", "Buf"));
        assert_eq!(s.users_of("Buf"), vec!["Two".to_string()]);
    }

    #[test]
    fn rename_updates_users() {
        let mut s = sandbox();
        s.rename("Buf", "Tampon");
        assert!(s.chips.contains_key("Tampon"));
        assert_eq!(s.chips["Tampon"].def.name, "Tampon");
        assert!(s.chips["Two"]
            .def
            .parts
            .iter()
            .all(|p| p.kind == PartKind::Chip("Tampon".into())));
        let mut l = lib();
        for d in s.chips.values() {
            l.insert(d.def.clone());
        }
        makine_sim::Circuit::compile(&l, "Two").unwrap();
    }

    #[test]
    fn set_pins_keeps_wires_by_name() {
        let mut s = sandbox();
        // Two: swap output order and drop input b.
        let ins = makine_sim::parse_pins(&["a"]);
        let outs = makine_sim::parse_pins(&["y", "x"]);
        s.set_pins("Two", ins, outs);
        let d = &s.chips["Two"].def;
        // Wire a -> part 0 stays (input 0); wire b -> part 1 is gone.
        assert!(d
            .wires
            .iter()
            .any(|w| w.from == Source::Input(0) && w.to == into(0, 0)));
        assert!(!d.wires.iter().any(|w| w.to == into(1, 0)));
        // Outputs follow their names.
        assert!(d
            .wires
            .iter()
            .any(|w| w.from == out(0, 0) && w.to == Sink::Output(1)));
        assert!(d
            .wires
            .iter()
            .any(|w| w.from == out(1, 0) && w.to == Sink::Output(0)));
    }

    #[test]
    fn set_pins_updates_users() {
        let mut s = sandbox();
        // Buf's input renamed: Two's wires into Buf parts are dropped.
        s.set_pins(
            "Buf",
            makine_sim::parse_pins(&["girdi"]),
            makine_sim::parse_pins(&["out"]),
        );
        let two = &s.chips["Two"].def;
        assert!(!two.wires.iter().any(|w| matches!(w.to, Sink::Part { .. })));
        assert_eq!(two.wires.len(), 2); // the two outputs remain
    }

    #[test]
    fn export_import_round_trip() {
        let mut s = sandbox();
        s.chips.get_mut("Two").unwrap().pos[1] = [123.0, 45.0];
        let p = export(&lib(), &s, "Two").unwrap();
        let text = p.to_json();
        let mut other = Sandbox::default();
        assert_eq!(import(&lib(), &mut other, &text).unwrap(), "Two");
        assert_eq!(other.defs(), s.defs());
        assert_eq!(other.chips["Two"].pos[1], [123.0, 45.0]);
        // Without the Not level, import fails and nothing is added.
        let mut empty = Sandbox::default();
        let e = import(&Library::new(), &mut empty, &text).unwrap_err();
        assert!(e.contains("Not"), "{e}");
        assert!(empty.chips.is_empty());
    }
}
