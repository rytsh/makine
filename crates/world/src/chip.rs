//! Worlds as chips: a region of blocks becomes a [`ChipDef`] the player can
//! use in the editor, in other chips, or place back into a world as a block.
//!
//! The chip's pins are the world's ports. Ports named with a common prefix
//! and the numbers `0..n` (`x0`, `x1`, … `x7`) become one bus `x[n]`, bit
//! `i` from port `xi`. Switches become constants at their current setting.
//! Clocks and devices with memory cannot go inside a chip yet.

use std::collections::BTreeMap;

use makine_sim::{ChipDef, Library, Part, PartKind, Pin, Sink, Source, Wire};

use crate::block::Block;
use crate::compile::{compile, InputSource};
use crate::device::Device;
use crate::world::World;

/// Group 1-bit port names into pins: (pin, port name of each bit).
fn group(names: &[String]) -> Vec<(Pin, Vec<String>)> {
    let mut buses: BTreeMap<String, BTreeMap<u32, String>> = BTreeMap::new();
    let mut singles = Vec::new();
    for n in names {
        let digits = n.len() - n.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let (base, num) = n.split_at(n.len() - digits);
        match (base.is_empty(), num.parse::<u32>()) {
            (false, Ok(i)) if digits > 0 && (num == "0" || !num.starts_with('0')) => {
                buses
                    .entry(base.to_string())
                    .or_default()
                    .insert(i, n.clone());
            }
            _ => singles.push(n.clone()),
        }
    }
    let mut out: Vec<(Pin, Vec<String>)> = Vec::new();
    for (base, bits) in buses {
        let n = bits.len() as u32;
        let complete = n > 1 && n <= 64 && bits.keys().copied().eq(0..n) && !names.contains(&base);
        if complete {
            out.push((Pin::new(base, n as u8), bits.into_values().collect()));
        } else {
            singles.extend(bits.into_values());
        }
    }
    out.extend(
        singles
            .into_iter()
            .map(|n| (Pin::new(n.clone(), 1), vec![n])),
    );
    out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    out
}

/// The world as a chip named `name`. `lib` holds the chips the world uses.
pub fn to_chip(world: &World, name: &str, lib: &Library) -> Result<ChipDef, String> {
    for (_, b) in world.blocks() {
        match b {
            Block::Clock { .. } => return Err("saat bloğu çipin içine giremez".into()),
            Block::Device { device, .. }
                if !matches!(device, Device::Chip { .. } | Device::Display { .. }) =>
            {
                return Err(format!("{} henüz çipin içine giremez", device.name()));
            }
            _ => {}
        }
    }
    crate::compile::check_chips(world, lib)?;
    let c = compile(world).map_err(|e| e.to_string())?;
    let switch_on = |p| matches!(world.get(p), Some(Block::Switch { on: true, .. }));

    let in_names: Vec<String> = c
        .inputs
        .iter()
        .filter_map(|s| match s {
            InputSource::Port(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    let out_names: Vec<String> = c.ports.iter().map(|(n, _)| n.clone()).collect();
    if out_names.is_empty() {
        return Err(
            "seçimde hiç çıkış portu yok: çipin ne üreteceğini bir çıkış portuyla göster".into(),
        );
    }
    let ins = group(&in_names);
    let outs = group(&out_names);
    if let Some(p) = ins
        .iter()
        .find(|i| outs.iter().any(|o| o.0.name == i.0.name))
    {
        return Err(format!("'{}' hem giriş hem çıkış portu", p.0.name));
    }

    let mut def = ChipDef::new(name, &[], &[]);
    def.inputs = ins.iter().map(|(p, _)| p.clone()).collect();
    def.outputs = outs.iter().map(|(p, _)| p.clone()).collect();
    def.parts = c.def.parts.clone();

    // New source for each old chip input.
    let mut src_of: Vec<Source> = Vec::new();
    for s in &c.inputs {
        let src = match s {
            InputSource::Port(n) => {
                let (j, (pin, bits)) = ins
                    .iter()
                    .enumerate()
                    .find(|(_, (_, bits))| bits.contains(n))
                    .unwrap();
                if pin.width == 1 {
                    Source::Input(j)
                } else {
                    let bit = bits.iter().position(|b| b == n).unwrap();
                    split_bit(&mut def, j, pin.width, bit)
                }
            }
            InputSource::Switch(p) => {
                def.parts.push(Part::new(PartKind::Const {
                    width: 1,
                    value: switch_on(*p) as u64,
                }));
                Source::Part {
                    part: def.parts.len() - 1,
                    pin: 0,
                }
            }
            InputSource::Clock { .. } => unreachable!("rejected above"),
        };
        src_of.push(src);
    }
    let map = |s: Source| match s {
        Source::Input(i) => src_of[i],
        s => s,
    };

    // Old output -> new sink, for port outputs only.
    let mut sink_of: BTreeMap<usize, Sink> = BTreeMap::new();
    let mut joins: BTreeMap<usize, usize> = BTreeMap::new();
    for (port, o) in &c.ports {
        let (j, (pin, bits)) = outs
            .iter()
            .enumerate()
            .find(|(_, (_, b))| b.contains(port))
            .unwrap();
        let sink = if pin.width == 1 {
            Sink::Output(j)
        } else {
            let join = *joins.entry(j).or_insert_with(|| {
                def.parts
                    .push(Part::new(PartKind::Join(vec![1; pin.width as usize])));
                let join = def.parts.len() - 1;
                def.wires.push(Wire {
                    from: Source::Part { part: join, pin: 0 },
                    to: Sink::Output(j),
                });
                join
            });
            Sink::Part {
                part: join,
                pin: bits.iter().position(|b| b == port).unwrap(),
            }
        };
        sink_of.insert(*o, sink);
    }
    for w in &c.def.wires {
        let to = match w.to {
            Sink::Output(o) => match sink_of.get(&o) {
                Some(s) => *s,
                None => continue,
            },
            s => s,
        };
        def.wires.push(Wire {
            from: map(w.from),
            to,
        });
    }
    // Check it builds.
    makine_sim::Circuit::compile_def(lib, &def).map_err(|e| e.to_string())?;
    Ok(def)
}

/// Bit `bit` of input bus `pin`, through one Split per bus (reused).
fn split_bit(def: &mut ChipDef, pin: usize, width: u8, bit: usize) -> Source {
    let existing = def.wires.iter().find_map(|w| match (w.from, w.to) {
        (Source::Input(i), Sink::Part { part, .. })
            if i == pin && matches!(def.parts[part].kind, PartKind::Split(_)) =>
        {
            Some(part)
        }
        _ => None,
    });
    let split = existing.unwrap_or_else(|| {
        def.parts
            .push(Part::new(PartKind::Split(vec![1; width as usize])));
        let s = def.parts.len() - 1;
        def.wires.push(Wire {
            from: Source::Input(pin),
            to: Sink::Part { part: s, pin: 0 },
        });
        s
    });
    Source::Part {
        part: split,
        pin: bit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping() {
        let names: Vec<String> = ["x1", "x0", "sel", "y0", "a10", "q2", "q0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let g = group(&names);
        let pins: Vec<String> = g.iter().map(|(p, _)| p.to_string()).collect();
        // x0,x1 -> x[2]; y0 alone stays y0; q0,q2 have a gap: singles.
        assert_eq!(pins, ["a10", "q0", "q2", "sel", "x[2]", "y0"]);
        assert_eq!(g[4].1, ["x0", "x1"]);
    }
}
