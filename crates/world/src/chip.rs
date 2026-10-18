//! Worlds as chips: a selection of parts becomes a [`ChipDef`] the player
//! can use in the editor, in other chips, or back in a world.
//!
//! The chip's pins are the selection's input and output parts, in order
//! from top to bottom. Switches become constants at their current setting.
//! Clocks and devices with memory cannot go inside a chip yet.

use std::collections::BTreeSet;

use makine_lang::{tr, trf};
use makine_sim::{ChipDef, Library, Part, PartKind, Pin, Sink, Source, Wire};

use crate::compile::{Builder, Mode};
use crate::device::Device;
use crate::part::Kind;
use crate::world::{Edit, End, Id, Placed, World};

/// The parts `ids` of `world` as a chip named `name`. `lib` holds the chips
/// the world uses. Wires from outside the selection are cut.
pub fn to_chip(world: &World, ids: &[Id], name: &str, lib: &Library) -> Result<ChipDef, String> {
    let set: BTreeSet<Id> = ids.iter().copied().collect();
    let mut parts: Vec<(Id, &Placed)> = world.parts().filter(|(id, _)| set.contains(id)).collect();
    // Pins top to bottom, then left to right.
    parts.sort_by(|a, b| {
        (a.1.pos.y, a.1.pos.x)
            .partial_cmp(&(b.1.pos.y, b.1.pos.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (_, p) in &parts {
        match &p.kind {
            Kind::Clock { .. } => {
                return Err(
                    tr!("saat çipin içine giremez", "a clock can't go inside a chip").into(),
                )
            }
            Kind::Device(d) if !matches!(d, Device::Display { .. }) => {
                return Err(trf!(
                    "{} henüz çipin içine giremez",
                    "{} can't go inside a chip yet",
                    d.name()
                ));
            }
            _ => {}
        }
    }
    let mut names = BTreeSet::new();
    for (_, p) in &parts {
        if let Some(n) = p.kind.port_name() {
            if !names.insert(n.to_string()) {
                return Err(trf!("'{n}' adında iki uç var", "two pins are named '{n}'"));
            }
        }
    }
    if !parts
        .iter()
        .any(|(_, p)| matches!(p.kind, Kind::Output { .. }))
    {
        return Err(tr!(
            "seçimde hiç çıkış yok: çipin ne üreteceğini bir Çıkış ile göster",
            "the selection has no output: use an Output to show what the chip makes"
        )
        .into());
    }

    // The selection alone, wires to the outside cut.
    let mut inside = World::new(name);
    for (id, p) in &parts {
        inside.apply(Edit::Part {
            id: *id,
            part: Some((*p).clone()),
        });
    }
    for (from, to) in world.wires() {
        if set.contains(&from.part) && set.contains(&to.part) {
            inside.apply(Edit::Wire {
                to,
                from: Some(from),
            });
        }
    }

    let mut b = Builder::new(name, lib, Mode::Chip);
    for (id, p) in &parts {
        b.part(*id, &p.kind);
    }
    b.wires(&inside);
    if let Some(p) = b.problems.first() {
        return Err(p.to_string());
    }
    // Output parts become the chip's outputs; nothing wired reads 0.
    let mut outs = Vec::new();
    for (id, p) in &parts {
        if let Kind::Output { name, width } = &p.kind {
            let src = match inside.wire_into(End::new(*id, 0)) {
                Some(from) => {
                    let (src, w) = b.source(from).ok_or(tr!(
                        "bir çıkışın teli kopuk",
                        "an output's wire is not connected"
                    ))?;
                    if w != *width && w != 1 {
                        return Err(trf!(
                            "'{name}' {width} bit, ona bağlı tel {w} bit",
                            "'{name}' is {width} bits, its wire is {w} bits"
                        ));
                    }
                    Some(src)
                }
                None => None,
            };
            outs.push((Pin::new(name.clone(), *width), src));
        }
    }
    let mut def: ChipDef = b.def;
    for (pin, src) in outs {
        let o = def.outputs.len();
        let src = src.unwrap_or_else(|| {
            def.parts.push(Part::new(PartKind::Const {
                width: pin.width,
                value: 0,
            }));
            Source::Part {
                part: def.parts.len() - 1,
                pin: 0,
            }
        });
        def.outputs.push(pin);
        def.wires.push(Wire {
            from: src,
            to: Sink::Output(o),
        });
    }
    makine_sim::Circuit::compile_def(lib, &def).map_err(|e| e.to_string())?;
    Ok(def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::Gate;
    use crate::world::Pos;
    use makine_sim::Circuit;

    fn put(w: &mut World, kind: Kind, y: f32) -> Id {
        let id = w.new_id();
        w.apply(Edit::Part {
            id,
            part: Some(Placed {
                kind,
                pos: Pos::new(0.0, y),
            }),
        });
        id
    }

    fn wire(w: &mut World, from: Id, to: (Id, usize)) {
        w.apply(Edit::Wire {
            to: End::new(to.0, to.1),
            from: Some(End::new(from, 0)),
        });
    }

    fn input(name: &str, width: u8) -> Kind {
        Kind::Input {
            name: name.into(),
            width,
            value: 0,
        }
    }

    fn output(name: &str, width: u8) -> Kind {
        Kind::Output {
            name: name.into(),
            width,
        }
    }

    #[test]
    fn a_selection_becomes_a_chip() {
        let mut w = World::new("w");
        let a = put(&mut w, input("a", 1), 0.0);
        let b = put(&mut w, input("b", 1), 50.0);
        let x = put(&mut w, Kind::Gate(Gate::Xor), 20.0);
        let q = put(&mut w, output("q", 1), 30.0);
        let outside = put(&mut w, Kind::Lamp, 90.0);
        wire(&mut w, a, (x, 0));
        wire(&mut w, b, (x, 1));
        wire(&mut w, x, (q, 0));
        wire(&mut w, x, (outside, 0));
        let lib = Library::new();
        let def = to_chip(&w, &[a, b, x, q], "Ya", &lib).unwrap();
        let names = |p: &[Pin]| p.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        assert_eq!(names(&def.inputs), ["a", "b"], "top to bottom");
        assert_eq!(names(&def.outputs), ["q"]);
        let mut sim = Circuit::compile_def(&lib, &def).unwrap();
        for v in 0..4u64 {
            let out = sim.eval_values(&[v & 1, v >> 1]).unwrap();
            assert_eq!(out, vec![(v & 1) ^ (v >> 1)]);
        }
    }

    #[test]
    fn buses_and_refusals() {
        let mut w = World::new("w");
        let a = put(&mut w, input("a", 8), 0.0);
        let q = put(&mut w, output("q", 8), 10.0);
        let unwired = put(&mut w, output("z", 4), 20.0);
        wire(&mut w, a, (q, 0));
        let lib = Library::new();
        let def = to_chip(&w, &[a, q, unwired], "Gec", &lib).unwrap();
        let mut sim = Circuit::compile_def(&lib, &def).unwrap();
        assert_eq!(sim.eval_values(&[0xA5]).unwrap(), vec![0xA5, 0]);

        assert!(to_chip(&w, &[a], "X", &lib).unwrap_err().contains("çıkış"));
        let clk = put(&mut w, Kind::Clock { period: 3 }, 0.0);
        assert!(to_chip(&w, &[a, q, clk], "X", &lib).is_err());
        let dup = put(&mut w, output("q", 1), 40.0);
        assert!(to_chip(&w, &[a, q, dup], "X", &lib)
            .unwrap_err()
            .contains("iki"));
    }
}
