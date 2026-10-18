//! How many NAND gates a solution takes, and how that compares.
//!
//! Every level has an example solution ([`best`]): the reference solution,
//! or a smaller one where the reference was written for clarity. Counts are
//! taken with every chip expanded to NAND gates (no fast models), and the
//! chips inside use the example solutions of their own levels, so a
//! level's count does not depend on how the earlier levels were solved.

use std::sync::OnceLock;

use makine_sim::{ChipBuilder, ChipDef, Circuit, CompileError, Library, Part, PartKind};

use crate::{find, levels, reference::reference};

/// The fewest NAND gates known to do a level's job, where the level is
/// small enough for that to be well known.
pub fn known_minimum(chip: &str) -> Option<usize> {
    Some(match chip {
        "Not" => 1,
        "And" => 2,
        "Or" => 3,
        "Xor" => 4,
        "Mux" => 4,
        "Demux" => 4,
        "HalfAdder" => 5,
        "FullAdder" => 9,
        "DLatch" => 4,
        _ => return None,
    })
}

/// What the example solution does, for the levels where it is not obvious
/// from the circuit.
pub fn note(chip: &str) -> Option<&'static str> {
    Some(match chip {
        "Not" => "NAND'ın iki girişi aynı sinyali alınca 1 NAND 1 = 0, 0 NAND 0 = 1: tersleyici.",
        "And" => "NAND'ın çıkışını tersle. NOT da bir NAND olduğu için toplam 2 kapı.",
        "Or" => "De Morgan: a VEYA b = NAND(DEĞİL a, DEĞİL b). İki NOT ve bir NAND, 3 kapı.",
        "Xor" => {
            "Ortadaki NAND(a, b) iki kez kullanılıyor. XOR'u AND/OR/NOT ile yaparsan daha fazla kapı gerekir; ara sonucu paylaşmak işin sırrı."
        }
        "Mux" => {
            "AND ve OR çipleriyle yazılan MUX 8 kapı tutar. Hepsi NAND olunca AND'lerin sonundaki NOT'lar OR'un başındaki NOT'larla birbirini götürür: NOT sel, NAND(a, NOT sel), NAND(b, sel) ve bunların NAND'ı. 4 kapı."
        }
        "Demux" => {
            "NAND(in, sel) iki işe yarar: tersi b çıkışıdır, ve in VE (DEĞİL sel) = in VE NAND(in, sel) olduğu için a çıkışını da ondan üretiriz. 4 kapı."
        }
        "HalfAdder" => {
            "XOR'un ilk NAND'ı zaten NAND(a, b). Onu terslemek eldeyi (a VE b) verir: ayrı bir AND koymaya gerek yok. 4 + 1 = 5 kapı."
        }
        "FullAdder" => {
            "İki XOR (8 kapı) ve eldede tek NAND: carry = NAND(NAND(a,b), NAND(a⊕b, c)). Her iki NAND da XOR'ların içinden zaten geliyor. İki yarım toplayıcı ve bir VEYA ile 15 kapı tutar; bu 9."
        }
        "Latch" => "Çapraz bağlı iki NAND belleğin kendisi; girişlerdeki iki NOT ise set/reset'i 1'de etkin yapıyor.",
        "DLatch" => {
            "s' = NAND(d, en), r' = NAND(s', en): d'nin tersi için ayrı bir NOT gerekmiyor, çünkü en = 1 iken s' zaten DEĞİL d. Ardından çapraz bağlı iki NAND. 4 kapı."
        }
        "DFF" => "İki D mandalı (4+4) ve saatin tersi için bir NOT: 9 kapı.",
        "Adder8" | "Adder32" | "Adder4" => {
            "Tam toplayıcıları zincirle. Kapı sayısı tam toplayıcının sayısıyla çarpılır: tam toplayıcıdan kazandığın her kapı burada katlanır."
        }
        _ => return None,
    })
}

/// The example solution of a level: the smallest one written down.
pub fn best(chip: &str) -> ChipDef {
    let Some(level) = find(chip) else {
        return reference(chip);
    };
    let mut b = ChipBuilder::new(chip, level.inputs, level.outputs);
    match chip {
        "Mux" => {
            let (x, y, s) = (b.input("a"), b.input("b"), b.input("sel"));
            let ns = b.nand(s, s);
            let l = b.nand(x, ns);
            let r = b.nand(y, s);
            let o = b.nand(l, r);
            b.output("out", o);
        }
        "HalfAdder" => {
            let (x, y) = (b.input("a"), b.input("b"));
            let n = b.nand(x, y);
            let l = b.nand(x, n);
            let r = b.nand(y, n);
            let s = b.nand(l, r);
            let k = b.nand(n, n);
            b.output("sum", s);
            b.output("carry", k);
        }
        "FullAdder" => {
            // XOR twice, sharing its first NAND for the carry:
            // carry = NAND(NAND(a, b), NAND(a xor b, c)).
            let (x, y, c) = (b.input("a"), b.input("b"), b.input("c"));
            let n1 = b.nand(x, y);
            let n2 = b.nand(x, n1);
            let n3 = b.nand(y, n1);
            let h = b.nand(n2, n3);
            let n5 = b.nand(h, c);
            let n6 = b.nand(h, n5);
            let n7 = b.nand(c, n5);
            let s = b.nand(n6, n7);
            let k = b.nand(n1, n5);
            b.output("sum", s);
            b.output("carry", k);
        }
        "Demux" => {
            // in AND NOT sel = in AND NAND(in, sel): the NAND serves both.
            let (i, s) = (b.input("in"), b.input("sel"));
            let n = b.nand(i, s);
            let y = b.nand(n, n);
            let m = b.nand(i, n);
            let x = b.nand(m, m);
            b.output("a", x);
            b.output("b", y);
        }
        "DLatch" => {
            // The gated latch: s' = NAND(d, en), r' = NAND(s', en), and two
            // cross-coupled NANDs.
            let (d, en) = (b.input("d"), b.input("en"));
            let s = b.nand(d, en);
            let r = b.nand(s, en);
            let q = b.part(Part::new(PartKind::Nand), &[s]);
            let qn = b.nand(r, makine_sim::out(q, 0));
            b.wire(qn, makine_sim::into(q, 1));
            b.output("q", makine_sim::out(q, 0));
        }
        _ => return reference(chip),
    }
    b.build()
}

/// Every level's example solution, all as NAND gates.
pub fn best_library() -> &'static Library {
    static LIB: OnceLock<Library> = OnceLock::new();
    LIB.get_or_init(|| {
        let mut lib = Library::new();
        for l in levels() {
            lib.insert(best(l.chip));
        }
        lib
    })
}

/// NAND gates in `def` with the chips in `lib` (which should have no fast
/// models).
pub fn nand_count(lib: &Library, def: &ChipDef) -> Result<usize, CompileError> {
    Circuit::compile_def(lib, def).map(|c| c.gate_count())
}

/// NAND gates in a level's example solution (worked out once).
pub fn example_count(chip: &str) -> Option<usize> {
    static COUNTS: OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<usize>>>> =
        OnceLock::new();
    let counts = COUNTS.get_or_init(Default::default);
    if let Some(n) = counts.lock().unwrap().get(chip) {
        return *n;
    }
    let n = nand_count(best_library(), &best(chip)).ok();
    counts.lock().unwrap().insert(chip.to_string(), n);
    n
}

/// How a solution compares, for the player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cost {
    /// The player's chip with their own chips inside.
    pub mine: usize,
    /// The player's chip with the example chips inside: what this level's
    /// own design costs.
    pub here: Option<usize>,
    pub example: Option<usize>,
    pub minimum: Option<usize>,
}

impl Cost {
    /// `lib` holds the player's chips, without fast models.
    pub fn of(chip: &str, lib: &Library, def: &ChipDef) -> Result<Cost, CompileError> {
        Ok(Cost {
            mine: nand_count(lib, def)?,
            here: nand_count(best_library(), def).ok(),
            example: example_count(chip),
            minimum: known_minimum(chip),
        })
    }

    /// What to aim for: the known minimum, else the example.
    pub fn target(&self) -> Option<usize> {
        self.minimum.or(self.example)
    }

    /// Gates the earlier chips add over their example solutions.
    pub fn from_earlier(&self) -> usize {
        self.here.map_or(0, |h| self.mine.saturating_sub(h))
    }

    /// Whether nothing is known to be smaller.
    pub fn optimal(&self) -> bool {
        self.target().is_some_and(|t| self.mine <= t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check;
    use makine_sim::Verdict;

    #[test]
    fn example_solutions_pass_and_reach_the_minimum() {
        for level in levels() {
            let def = best(level.chip);
            assert_eq!(
                check(level, best_library(), &def).unwrap(),
                Verdict::Pass,
                "{}",
                level.chip
            );
            if let Some(min) = known_minimum(level.chip) {
                assert_eq!(example_count(level.chip), Some(min), "{}", level.chip);
            }
        }
    }

    #[test]
    fn counts_compare_mine_here_and_earlier() {
        // The clear-but-big reference MUX with the example AND/OR/NOT.
        let mut lib = Library::new();
        for l in levels().iter().take_while(|l| l.chip != "Mux") {
            lib.insert(reference(l.chip));
        }
        let c = Cost::of("Mux", &lib, &reference("Mux")).unwrap();
        assert_eq!(c.mine, 8);
        assert_eq!(c.here, Some(8));
        assert_eq!(c.target(), Some(4));
        assert!(!c.optimal());
        let c = Cost::of("Mux", &lib, &best("Mux")).unwrap();
        assert!(c.optimal());

        // A wide adder of the reference (15-gate) full adders: the extra
        // gates come from the earlier level, not from this one.
        let mut lib = Library::new();
        for l in levels().iter().take_while(|l| l.chip != "Adder8") {
            lib.insert(reference(l.chip));
        }
        let c = Cost::of("Adder8", &lib, &reference("Adder8")).unwrap();
        assert_eq!(c.example, Some(8 * 9));
        assert_eq!(c.here, Some(8 * 9));
        assert_eq!(c.from_earlier(), c.mine - 72);
        assert!(c.from_earlier() > 0);
    }
}
