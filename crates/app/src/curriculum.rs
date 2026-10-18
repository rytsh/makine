//! One learning path: each programming/design activity belongs to a chip,
//! and worlds carry local lab context rather than a second progress ledger.
use crate::{isa_lessons, lessons, mini_lessons};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Program(&'static str),
    Mini(&'static str),
    Isa(usize),
}

pub fn program_chip(id: &str) -> Option<&'static str> {
    Some(match id {
        "register" | "copy" => "Bit",
        "sum" | "bits" | "shift" => "Register32",
        "max" | "unsigned" | "loop" => "Counter32",
        "address" | "store" | "memory" | "byte" | "string" => "Ram8",
        "function" | "nested" => "RegFile",
        "console" | "echo" => "LoadUnit",
        "pixel" | "coordinates" => "StoreUnit",
        "project" => "Cpu",
        _ => return None,
    })
}

pub fn isa_chip(index: usize) -> Option<&'static str> {
    Some(match index {
        0 => "AddSub8",
        1..=4 => "MiniDecode",
        5..=6 => "MiniControl",
        7 => "MiniAcc8",
        8..=9 => "MiniCpu",
        _ => return None,
    })
}

pub fn activities(chip: &str) -> Vec<Activity> {
    lessons::all()
        .into_iter()
        .filter(|l| program_chip(l.id) == Some(chip))
        .map(|l| Activity::Program(l.id))
        .chain(
            mini_lessons::all()
                .into_iter()
                .filter(|l| l.chip == chip)
                .map(|l| Activity::Mini(l.id)),
        )
        .chain(
            isa_lessons::all()
                .iter()
                .enumerate()
                .filter(|(i, _)| isa_chip(*i) == Some(chip))
                .map(|(i, _)| Activity::Isa(i)),
        )
        .collect()
}

impl Activity {
    pub fn title(self) -> &'static str {
        match self {
            Self::Program(id) => lessons::all()
                .into_iter()
                .find(|l| l.id == id)
                .map_or(id, |l| l.title),
            Self::Isa(i) => isa_lessons::all().get(i).map_or("ISA", |l| l.title),
            Self::Mini(id) => mini_lessons::find(id).map_or(id, |l| l.title),
        }
    }
    pub fn chip(self) -> &'static str {
        match self {
            Self::Program(id) => program_chip(id),
            Self::Isa(i) => isa_chip(i),
            Self::Mini(id) => mini_lessons::find(id).map(|l| l.chip),
        }
        .unwrap()
    }
    pub fn done(self, n: &crate::programming::Notebook) -> bool {
        match self {
            Self::Program(id) | Self::Mini(id) => n.solved.contains(id),
            Self::Isa(i) => isa_lessons::all()
                .get(i)
                .is_some_and(|l| n.isa_solved.contains(l.id)),
        }
    }
}

pub fn example(chip: &str) -> Option<&'static str> {
    Some(match chip {
        "Not" | "And" | "Or" | "Xor" => "gates",
        "HalfAdder" | "FullAdder" => "half_adder",
        "Adder8" | "Adder32" | "Sub32" => "adder4",
        "Latch" | "DLatch" | "DFF" => "sr_latch",
        "Counter32" | "MiniPc4" => "counter",
        "Ram8" => "ram_bench",
        "Ram4x8" => "byte_ram_bench",
        "Rom4x8" => "rom_bench",
        "ProgramRom" | "RomSequencer" => "program_rom",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_activity_has_exactly_one_real_level() {
        let mut total = 0;
        for l in makine_levels::levels() {
            for a in activities(l.chip) {
                assert_eq!(a.chip(), l.chip);
                total += 1;
            }
        }
        assert_eq!(
            total,
            lessons::all().len() + isa_lessons::all().len() + mini_lessons::all().len()
        );
        for l in lessons::all() {
            assert!(makine_levels::find(program_chip(l.id).unwrap()).is_some());
        }
        for l in makine_levels::levels() {
            if let Some(id) = example(l.chip) {
                assert!(makine_world::projects::examples::all()
                    .iter()
                    .any(|e| e.id == id));
            }
        }
    }
}
