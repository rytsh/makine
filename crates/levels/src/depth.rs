//! How deep a solution's logic is: the most gates a signal passes through,
//! next to the example solution's.

use makine_sim::{ChipDef, Circuit, CompileError, Library};

use crate::cost::{best, best_library};

/// Gates on the longest path of `def`, with the chips in `lib`.
pub fn depth(lib: &Library, def: &ChipDef) -> Result<usize, CompileError> {
    Circuit::compile_def(lib, def).map(|c| c.depth().gates)
}

/// The longest path in a level's example solution (worked out once).
pub fn example_depth(chip: &str) -> Option<usize> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static DEPTHS: OnceLock<Mutex<HashMap<String, Option<usize>>>> = OnceLock::new();
    let depths = DEPTHS.get_or_init(Default::default);
    if let Some(d) = depths.lock().unwrap().get(chip) {
        return *d;
    }
    let d = depth(best_library(), &best(chip)).ok();
    depths.lock().unwrap().insert(chip.to_string(), d);
    d
}

/// The player's depth against the example's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delay {
    pub mine: usize,
    pub example: Option<usize>,
}

impl Delay {
    pub fn of(chip: &str, lib: &Library, def: &ChipDef) -> Result<Delay, CompileError> {
        Ok(Delay {
            mine: depth(lib, def)?,
            example: example_depth(chip),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adders_are_as_deep_as_their_carry_chain() {
        let fa = example_depth("FullAdder").unwrap();
        let a8 = example_depth("Adder8").unwrap();
        let a32 = example_depth("Adder32").unwrap();
        assert!(fa >= 3, "{fa}");
        // Ripple carry: each bit adds the carry's path through one adder.
        assert!(a8 > fa && a32 > 3 * a8, "{fa} {a8} {a32}");
        assert_eq!(example_depth("Not"), Some(1));
    }
}
