//! Fast models: a native (Rust) implementation of every level's chip.
//!
//! Once the player's chip passes its level, the simulator may use this model
//! in its place ("hızlı simülasyon"). The chip was verified against the same
//! behaviour, so the larger circuit behaves the same, only much faster.

use makine_sim::{mask, ChipDef, Library, NativeDef};

use crate::cpu::{alu, control};
use crate::{find, levels, Test};

/// Native model of a level's chip.
pub fn model(chip: &str) -> Option<NativeDef> {
    let level = find(chip)?;
    let (ins, outs) = (level.inputs, level.outputs);
    Some(match chip {
        "Latch" => NativeDef::level(
            ins,
            outs,
            0u64,
            |q, i, o| o[0] = latch(*q, i),
            |q, i| *q = latch(*q, i),
        ),
        "DLatch" => NativeDef::level(
            ins,
            outs,
            0u64,
            |q, i, o| o[0] = if i[1] & 1 == 1 { i[0] } else { *q },
            |q, i| {
                if i[1] & 1 == 1 {
                    *q = i[0]
                }
            },
        ),
        "DFF" => NativeDef::clocked(ins, outs, &[], 0u64, |q, i| *q = i[0], |q, _, o| o[0] = *q),
        "Bit" | "Register4" | "Register32" => NativeDef::clocked(
            ins,
            outs,
            &[],
            0u64,
            |q, i| {
                if i[1] & 1 == 1 {
                    *q = i[0]
                }
            },
            |q, _, o| o[0] = *q,
        ),
        "Counter12" => NativeDef::clocked(
            ins,
            outs,
            &[],
            0u64,
            |q, i| {
                *q = if i[3] == 1 {
                    0
                } else if i[1] == 1 {
                    i[0]
                } else {
                    (*q + i[2]) & 4095
                };
            },
            |q, _, o| o[0] = *q,
        ),
        "Counter32" => NativeDef::clocked(
            ins,
            outs,
            &[],
            0u64,
            |q, i| {
                *q = if i[3] & 1 == 1 {
                    0
                } else if i[1] & 1 == 1 {
                    i[0]
                } else if i[2] & 1 == 1 {
                    q.wrapping_add(4) & mask(32)
                } else {
                    *q
                }
            },
            |q, _, o| o[0] = *q,
        ),
        "Ram8" => NativeDef::clocked(
            ins,
            outs,
            &["addr"],
            [0u64; 8],
            |m, i| {
                if i[2] & 1 == 1 {
                    m[i[0] as usize & 7] = i[1];
                }
            },
            |m, i, o| o[0] = m[i[0] as usize & 7],
        ),
        "Ram4x8" => NativeDef::clocked(
            ins,
            outs,
            &["addr"],
            [0u64; 4],
            |m, i| {
                if i[2] == 1 {
                    m[i[0] as usize] = i[1];
                }
            },
            |m, i, o| o[0] = m[i[0] as usize],
        ),
        "RomSequencer" => NativeDef::clocked(
            ins,
            outs,
            &[],
            0u64,
            |pc, i| *pc = crate::memory::next_pc(*pc, i[0], i[1], i[2], i[3]),
            |pc, _, o| {
                o[0] = *pc;
                o[1] = crate::memory::PROGRAM[*pc as usize] as u64;
            },
        ),
        "RegFile" => NativeDef::clocked(
            ins,
            outs,
            &["rs1", "rs2"],
            [0u64; 32],
            |r, i| {
                let (rd, d, we) = (i[2] as usize & 31, i[3], i[4] & 1);
                if we == 1 && rd != 0 {
                    r[rd] = d;
                }
            },
            |r, i, o| {
                o[0] = r[i[0] as usize & 31];
                o[1] = r[i[1] as usize & 31];
            },
        ),
        "Alu" => NativeDef::function(ins, outs, |i| {
            vec![alu(i[0] as u32, i[1] as u32, i[2]) as u64]
        }),
        "Control" => NativeDef::function(ins, outs, |i| {
            let s = control(i[0] as u32, i[1] as u32, i[2] as u32).unwrap_or_default();
            vec![
                s.reg_we,
                s.alu_b_imm,
                s.alu_op,
                s.wb_sel,
                s.mem_read,
                s.mem_write,
                s.branch,
                s.jal,
                s.jalr,
                s.auipc,
                s.mul,
            ]
        }),
        "ScanCounter" => NativeDef::clocked(
            ins,
            outs,
            &[],
            (0u64, 0u64),
            |p, i| *p = crate::gpu::scan_next(p.0, p.1, i[0] & 1 == 1),
            |p, _, o| {
                o[0] = p.0;
                o[1] = p.1;
            },
        ),
        "Fill" => NativeDef::clocked(
            ins,
            outs,
            &["ax", "ay", "bx", "by", "cx", "cy"],
            (0u64, 0u64),
            |p, i| *p = crate::gpu::scan_next(p.0, p.1, i[6] & 1 == 1),
            |p, i, o| {
                let mut v = i[..6].to_vec();
                v.extend([p.0, p.1]);
                o[0] = p.0;
                o[1] = p.1;
                o[2] = crate::gpu::triangle(&v) as u64;
            },
        ),
        "PipeAdd32" => NativeDef::clocked(
            ins,
            outs,
            &[],
            0u64,
            |s, i| *s = (i[0] + i[1]) & mask(32),
            |s, _, o| o[0] = *s,
        ),
        "Cache4" => NativeDef::clocked(
            ins,
            outs,
            &["addr"],
            [None::<(u64, u64)>; 4],
            |lines, i| {
                if i[2] & 1 == 1 {
                    lines[i[0] as usize % 4] = Some((i[0] >> 2, i[1]));
                }
            },
            |lines, i, o| match lines[i[0] as usize % 4] {
                Some((tag, d)) if tag == i[0] >> 2 => {
                    o[0] = 1;
                    o[1] = d;
                }
                // The data of a missed line is "don't care"; show it anyway.
                line => {
                    o[0] = 0;
                    o[1] = line.map_or(0, |l| l.1);
                }
            },
        ),
        "Cpu" => return None,
        _ => match level.test {
            Test::Function(f) => NativeDef::function(ins, outs, f),
            Test::Partial(f) => NativeDef::function(ins, outs, move |i| {
                f(i).into_iter().map(|v| v.unwrap_or(0)).collect()
            }),
            Test::Sequence(_) | Test::Custom(_) => return None,
        },
    })
}

fn latch(q: u64, i: &[u64]) -> u64 {
    if i[0] & 1 == 1 {
        1
    } else if i[1] & 1 == 1 {
        0
    } else {
        q
    }
}

/// The player's library: their solved chips, plus fast models of them.
/// With `fast`, the models are used instead of the gates.
pub fn library<'a>(solutions: impl IntoIterator<Item = &'a ChipDef>, fast: bool) -> Library {
    let mut lib = Library::new();
    for def in solutions {
        if let Some(m) = model(&def.name) {
            lib.insert_native(def.name.clone(), m);
            lib.set_fast(&def.name, fast);
        }
        lib.insert(def.clone());
    }
    lib
}

/// Every level that has a fast model.
pub fn modeled() -> impl Iterator<Item = &'static str> {
    levels()
        .iter()
        .map(|l| l.chip)
        .filter(|c| model(c).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::reference;
    use crate::{check, Level};
    use makine_sim::Verdict;

    fn level_passes_with(level: &Level, model: NativeDef) -> Verdict {
        // Wrap the model in a chip with the level's pins: one part, wired
        // straight through.
        let mut lib = Library::new();
        lib.insert_native("Model", model);
        let mut b = makine_sim::ChipBuilder::new(level.chip, level.inputs, level.outputs);
        let ins: Vec<_> = level
            .input_pins()
            .iter()
            .map(|p| b.input(&p.name))
            .collect();
        let part = b.chip("Model", &ins);
        for (k, p) in level.output_pins().iter().enumerate() {
            b.output(&p.name, makine_sim::out(part, k));
        }
        let def = b.build();
        let mut circuit = makine_sim::Circuit::compile_def(&lib, &def).unwrap();
        let steps = level.steps();
        match level.test {
            Test::Function(_) | Test::Partial(_) => {
                makine_sim::check_combinational(&mut circuit, &level.input_pins(), &steps)
            }
            Test::Sequence(_) => makine_sim::check_sequence(&mut circuit, &steps),
            Test::Custom(f) => f(&mut circuit),
        }
    }

    /// Every model must pass its own level, or swapping it in would change
    /// behaviour.
    #[test]
    fn models_pass_their_levels() {
        for chip in modeled() {
            let level = find(chip).unwrap();
            assert_eq!(
                level_passes_with(level, model(chip).unwrap()),
                Verdict::Pass,
                "{chip}"
            );
        }
    }

    /// The CPU built from the reference parts passes with every sub-chip
    /// replaced by its fast model.
    #[test]
    fn cpu_passes_fast() {
        let refs: Vec<ChipDef> = levels().iter().map(|l| reference(l.chip)).collect();
        let lib = library(refs.iter().filter(|d| d.name != "Cpu"), true);
        let t = std::time::Instant::now();
        assert_eq!(
            check(find("Cpu").unwrap(), &lib, &reference("Cpu")).unwrap(),
            Verdict::Pass
        );
        assert!(t.elapsed().as_secs() < 30, "too slow: {:?}", t.elapsed());
    }

    #[test]
    fn cpu_signals_cover_control_outputs() {
        // Keep `model("Control")` in sync with the level's outputs.
        assert_eq!(find("Control").unwrap().outputs.len(), 11);
    }
}
