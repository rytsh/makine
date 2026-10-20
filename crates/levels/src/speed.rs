//! An optional side path after the processor: what makes processors fast.
//! A shorter carry path, a pipeline register, forwarding between pipeline
//! stages and a cache.
//!
//! Speed is measured, not just function: the adder levels also fail when
//! their longest gate path is not clearly shorter than the player's own
//! Adder32's (see [`check_depth`]).

use makine_lang::trf;
use makine_sim::{
    mask, out, ChipBuilder, ChipDef, Circuit, Library, Part, PartKind, Rng, Source, TestStep,
    Verdict,
};

use crate::{Level, Test};

pub const SECTION: &str = "7. Hızlı işlemci";

/// Lines in the cache level, and bits of its tag.
const LINES: usize = 4;
const TAG_BITS: u8 = 6;

pub static LEVELS: &[Level] = &[
    Level {
        chip: "FastAdd32",
        section: SECTION,
        title: "Hızlı Toplayıcı",
        description: "Adder32 gibi a + b + c topla; ama en uzun kapı yolu, kendi Adder32'ninkinin \
            %60'ını geçmesin.\n\n\
            Zincirleme toplayıcıda elde bir bitten ötekine sırayla yürür: 32 bitte yaklaşık 68 \
            kapı. Saat ancak bu yol durulduktan sonra vurabilir. Elde seçmeli toplayıcı üst \
            baytları iki kez, elde 0 ve elde 1 için önceden toplar; gerçek elde gelince MUX \
            doğru sonucu seçer. Daha çok kapı, ama daha kısa yol.",
        inputs: &["a[32]", "b[32]", "c"],
        outputs: &["sum[32]", "carry"],
        test: Test::Function(|i| {
            let n = i[0] + i[1] + i[2];
            vec![n & 0xFFFF_FFFF, n >> 32]
        }),
    },
    Level {
        chip: "PipeAdd32",
        section: SECTION,
        title: "Boru Hattı: İki Aşamalı Toplayıcı",
        description:
            "sum, bir önceki yükselen clk kenarındaki a + b olsun (32 bit, taşan atılır). \
            Her kenarda yeni bir toplama başlar. En uzun kapı yolu, kendi Adder32'ninkinin %60'ını \
            geçmesin.\n\n\
            İşi ikiye böl: alt 16 biti topla ve sonucu, eldeyi ve a ile b'nin üst yarılarını \
            bir yazmaca koy. Kenardan sonra üst yarıyı topla. Her aşama yarı uzunlukta olduğu \
            için saat iki kat hızlı vurabilir. Bir toplamanın sonucu bir saat gecikmeyle çıkar \
            (gecikme), ama her saatte bir sonuç çıkar (verim). İşlemcilerdeki boru hattı budur.",
        inputs: &["a[32]", "b[32]", "clk"],
        outputs: &["sum[32]"],
        test: Test::Sequence(pipe_steps),
    },
    Level {
        chip: "Forward",
        section: SECTION,
        title: "Veri Yönlendirme",
        description:
            "Boru hattındaki bir komut, önceki komutun henüz register'a yazılmamış sonucuna \
            ihtiyaç duyabilir. fa (rs1 için) ve fb (rs2 için) şunu söylesin:\n\
            1: sonuç bir önceki komutta, EX aşamasında (ex_we = 1, ex_rd = rs, ex_rd ≠ 0)\n\
            2: değilse iki önceki komutta, MEM aşamasında (mem_we = 1, mem_rd = rs, mem_rd ≠ 0)\n\
            0: register dosyasından okunan değer doğru.\n\n\
            EX daha yenidir, ikisi birden eşleşirse 1 kazanır. x0'a yazılan hiçbir şey \
            yönlendirilmez: x0 her zaman 0'dır.",
        inputs: &[
            "rs1[5]",
            "rs2[5]",
            "ex_rd[5]",
            "ex_we",
            "mem_rd[5]",
            "mem_we",
        ],
        outputs: &["fa[2]", "fb[2]"],
        test: Test::Function(|i| vec![forward(i[0], i), forward(i[1], i)]),
    },
    Level {
        chip: "Cache4",
        section: SECTION,
        title: "Önbellek",
        description: "4 satırlık doğrudan eşlemeli önbellek. addr'ın alt 2 biti satırı seçer, üst \
            6 biti etikettir. Satırın geçerli biti 1 ve etiketi eşleşiyorsa hit = 1 ve q satırın \
            verisidir. Yükselen clk kenarında fill = 1 ise satıra d yazılır: geçerli = 1, etiket = \
            addr'ın üst 6 biti. Açılışta bütün satırlar geçersizdir.\n\n\
            Ana bellek yavaştır: işlemci her erişimde onu beklerse durur. Önbellek son kullanılan \
            kelimeleri işlemcinin yanında tutar. Isabet (hit) olursa veri hemen gelir; ıskalarsa \
            (miss) bellekten getirilip satıra yazılır. Gerçek önbellekler aynı fikrin binlerce \
            satırlık halidir.",
        inputs: &["addr[8]", "d[32]", "fill", "clk"],
        outputs: &["hit", "q[32]"],
        test: Test::Sequence(cache_steps),
    },
];

/// The longest path allowed, as a share of the player's own Adder32's:
/// the levels about speed must beat the adder they started from.
pub fn depth_share(chip: &str) -> Option<f64> {
    Some(match chip {
        "FastAdd32" | "PipeAdd32" => 0.6,
        _ => return None,
    })
}

/// The longest path of `def`, in NAND gates. `lib` holds the player's
/// chips; they are all taken as gates (no fast models) except the storage
/// chips: a register's input ends one path and its output starts the next,
/// the way a clock divides the work.
pub fn path(lib: &Library, def: &ChipDef) -> Option<usize> {
    let mut gates = Library::new();
    for d in lib.chips() {
        gates.insert(d.clone());
    }
    for ff in ["DFF", "DLatch", "Latch"] {
        if let Some(m) = crate::fast::model(ff) {
            gates.insert_native(ff, m);
            gates.set_fast(ff, true);
        }
    }
    Some(Circuit::compile_def(&gates, def).ok()?.depth().gates)
}

/// Fail a working circuit that is too slow. `lib` holds the player's chips.
pub fn check_depth(chip: &str, lib: &Library, def: &ChipDef) -> Option<Verdict> {
    let share = depth_share(chip)?;
    let adder = path(lib, lib.get("Adder32")?)?;
    let limit = (adder as f64 * share) as usize;
    let depth = path(lib, def)?;
    (depth > limit).then(|| Verdict::Wrong {
        step: 0,
        message: trf!(
            "Devre doğru çalışıyor ama yavaş: en uzun yol {depth} NAND. Senin Adder32'ninki {adder}; bunun %60'ı, yani en fazla {limit} olmalı. Saat ancak en uzun yol durulduktan sonra vurabilir.",
            "The circuit works but is slow: the longest path is {depth} NANDs. Your Adder32's is {adder}; at most 60% of that, {limit}, is allowed. The clock can only tick once the longest path has settled."
        ),
    })
}

fn forward(rs: u64, i: &[u64]) -> u64 {
    let (ex_rd, ex_we, mem_rd, mem_we) = (i[2], i[3], i[4], i[5]);
    if ex_we == 1 && ex_rd != 0 && ex_rd == rs {
        1
    } else if mem_we == 1 && mem_rd != 0 && mem_rd == rs {
        2
    } else {
        0
    }
}

fn pipe_steps() -> Vec<TestStep> {
    let mut rng = Rng(70);
    let mut steps = Vec::new();
    let mut result: Option<u64> = None;
    for cycle in 0..96 {
        let (a, b) = match cycle {
            0 => (0xFFFF, 1),
            1 => (0xFFFF_FFFF, 1),
            2 => (0x8000_FFFF, 0x7FFF_0001),
            _ => (rng.next_u64() & mask(32), rng.next_u64() & mask(32)),
        };
        steps.push(TestStep {
            inputs: vec![a, b, 0],
            expect: vec![result],
        });
        result = Some((a + b) & mask(32));
        steps.push(TestStep {
            inputs: vec![a, b, 1],
            expect: vec![result],
        });
        // Changing the inputs without a new edge starts nothing.
        steps.push(TestStep {
            inputs: vec![rng.next_u64() & mask(32), rng.next_u64() & mask(32), 1],
            expect: vec![result],
        });
    }
    steps
}

fn cache_steps() -> Vec<TestStep> {
    let mut rng = Rng(71);
    let mut lines: [Option<(u64, u64)>; LINES] = [None; LINES];
    let mut steps = Vec::new();
    let look = |lines: &[Option<(u64, u64)>; LINES], addr: u64| {
        let (idx, tag) = (addr as usize % LINES, addr >> 2);
        match lines[idx] {
            Some((t, d)) if t == tag => vec![Some(1), Some(d)],
            _ => vec![Some(0), None],
        }
    };
    // A few addresses, so lookups hit often; conflicting tags evict.
    let addrs: Vec<u64> = (0..10).map(|_| rng.next_u64() & 0xFF).collect();
    for cycle in 0..150 {
        let addr = if cycle < 4 {
            cycle
        } else {
            addrs[(rng.next_u64() % addrs.len() as u64) as usize]
        };
        let d = rng.next_u64() & mask(32);
        let fill = if cycle < 4 {
            0
        } else {
            rng.next_u64().is_multiple_of(3) as u64
        };
        steps.push(TestStep {
            inputs: vec![addr, d, fill, 0],
            expect: look(&lines, addr),
        });
        if fill == 1 {
            lines[addr as usize % LINES] = Some((addr >> 2, d));
        }
        steps.push(TestStep {
            inputs: vec![addr, d, fill, 1],
            expect: look(&lines, addr),
        });
        // Another address while clk stays high: a lookup, no write.
        let other = addrs[(rng.next_u64() % addrs.len() as u64) as usize];
        steps.push(TestStep {
            inputs: vec![other, rng.next_u64() & mask(32), 1, 1],
            expect: look(&lines, other),
        });
    }
    steps
}

/// Reference solutions.
pub fn reference(chip: &str) -> Option<ChipDef> {
    let level = crate::find(chip)?;
    let mut b = ChipBuilder::new(chip, level.inputs, level.outputs);
    match chip {
        "FastAdd32" => {
            // Carry select: byte 0 ripples; bytes 1–3 are added for both
            // carries at once, and the real carry picks.
            let (x, y, c) = (b.input("a"), b.input("b"), b.input("c"));
            let xs = b.split(&[8; 4], x);
            let ys = b.split(&[8; 4], y);
            let (zero, one) = (b.constant(1, 0), b.constant(1, 1));
            let first = b.chip("Adder8", &[out(xs, 0), out(ys, 0), c]);
            let mut sums = vec![out(first, 0)];
            let mut carry = out(first, 1);
            for k in 1..4 {
                let s0 = b.chip("Adder8", &[out(xs, k), out(ys, k), zero]);
                let s1 = b.chip("Adder8", &[out(xs, k), out(ys, k), one]);
                let sum = b.chip_n("Mux", 8, &[out(s0, 0), out(s1, 0), carry]);
                sums.push(out(sum, 0));
                carry = b.chip1("Mux", &[out(s0, 1), out(s1, 1), carry]);
            }
            let sum = b.join(&[8; 4], &sums);
            b.output("sum", sum);
            b.output("carry", carry);
        }
        "PipeAdd32" => {
            let (x, y, clk) = (b.input("a"), b.input("b"), b.input("clk"));
            let xs = b.split(&[8, 8, 16], x);
            let ys = b.split(&[8, 8, 16], y);
            let zero = b.constant(1, 0);
            // Stage 1: the low 16 bits.
            let l0 = b.chip("Adder8", &[out(xs, 0), out(ys, 0), zero]);
            let l1 = b.chip("Adder8", &[out(xs, 1), out(ys, 1), out(l0, 1)]);
            // The pipeline register: low sum, carry, both high halves.
            let stage = b.join(
                &[8, 8, 1, 16, 16],
                &[out(l0, 0), out(l1, 0), out(l1, 1), out(xs, 2), out(ys, 2)],
            );
            let one = b.constant(1, 1);
            let reg = b.part(
                Part {
                    kind: PartKind::Chip("Bit".into()),
                    repeat: 49,
                },
                &[stage, one, clk],
            );
            let r = b.split(&[16, 1, 8, 8, 8, 8], out(reg, 0));
            // Stage 2: the high 16 bits.
            let h0 = b.chip("Adder8", &[out(r, 2), out(r, 4), out(r, 1)]);
            let h1 = b.chip("Adder8", &[out(r, 3), out(r, 5), out(h0, 1)]);
            let sum = b.join(&[16, 8, 8], &[out(r, 0), out(h0, 0), out(h1, 0)]);
            b.output("sum", sum);
        }
        "Forward" => {
            let ins = ["rs1", "rs2", "ex_rd", "ex_we", "mem_rd", "mem_we"].map(|n| b.input(n));
            let [rs1, rs2, ex_rd, ex_we, mem_rd, mem_we] = ins;
            let ex_nz = nonzero5(&mut b, ex_rd);
            let ex_ok = b.chip1("And", &[ex_we, ex_nz]);
            let mem_nz = nonzero5(&mut b, mem_rd);
            let mem_ok = b.chip1("And", &[mem_we, mem_nz]);
            for (rs, name) in [(rs1, "fa"), (rs2, "fb")] {
                let from_ex = equal5(&mut b, rs, ex_rd);
                let from_ex = b.chip1("And", &[from_ex, ex_ok]);
                let from_mem = equal5(&mut b, rs, mem_rd);
                let from_mem = b.chip1("And", &[from_mem, mem_ok]);
                let not_ex = b.chip1("Not", &[from_ex]);
                let bit1 = b.chip1("And", &[from_mem, not_ex]);
                let f = b.join(&[1, 1], &[from_ex, bit1]);
                b.output(name, f);
            }
        }
        "Cache4" => {
            let (addr, d, fill, clk) = (
                b.input("addr"),
                b.input("d"),
                b.input("fill"),
                b.input("clk"),
            );
            let a = b.split(&[1, 1, TAG_BITS], addr);
            let (a0, a1, tag) = (out(a, 0), out(a, 1), out(a, 2));
            let one = b.constant(1, 1);
            // A line: valid (1) + tag (6) + data (32) = 39 bits.
            let line_in = b.join(&[1, TAG_BITS, 32], &[one, tag, d]);
            let n0 = b.chip1("Not", &[a0]);
            let n1 = b.chip1("Not", &[a1]);
            let mut lines = Vec::new();
            for k in 0..LINES {
                let s0 = if k & 1 == 1 { a0 } else { n0 };
                let s1 = if k & 2 == 2 { a1 } else { n1 };
                let sel = b.chip1("And", &[s0, s1]);
                let we = b.chip1("And", &[sel, fill]);
                let reg = b.part(
                    Part {
                        kind: PartKind::Chip("Bit".into()),
                        repeat: 39,
                    },
                    &[line_in, we, clk],
                );
                lines.push(out(reg, 0));
            }
            let lo = b.chip_n("Mux", 39, &[lines[0], lines[1], a0]);
            let hi = b.chip_n("Mux", 39, &[lines[2], lines[3], a0]);
            let line = b.chip_n("Mux", 39, &[out(lo, 0), out(hi, 0), a1]);
            let f = b.split(&[1, TAG_BITS, 32], out(line, 0));
            let same = equal_n(&mut b, out(f, 1), tag, TAG_BITS);
            let hit = b.chip1("And", &[out(f, 0), same]);
            b.output("hit", hit);
            b.output("q", out(f, 2));
        }
        _ => return None,
    }
    Some(b.build())
}

/// 1 if two `width`-bit buses are equal.
fn equal_n(b: &mut ChipBuilder, x: Source, y: Source, width: u8) -> Source {
    let diff = b.part(
        Part {
            kind: PartKind::Chip("Xor".into()),
            repeat: width,
        },
        &[x, y],
    );
    let any = any_bit(b, out(diff, 0), width);
    b.chip1("Not", &[any])
}

fn equal5(b: &mut ChipBuilder, x: Source, y: Source) -> Source {
    equal_n(b, x, y, 5)
}

fn nonzero5(b: &mut ChipBuilder, x: Source) -> Source {
    any_bit(b, x, 5)
}

/// OR of every bit, as a balanced tree.
fn any_bit(b: &mut ChipBuilder, v: Source, width: u8) -> Source {
    let bits = b.split(&vec![1; width as usize], v);
    let mut level: Vec<Source> = (0..width as usize).map(|i| out(bits, i)).collect();
    while level.len() > 1 {
        level = level
            .chunks(2)
            .map(|p| {
                if p.len() == 2 {
                    b.chip1("Or", &[p[0], p[1]])
                } else {
                    p[0]
                }
            })
            .collect();
    }
    level[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, find, levels};
    use makine_sim::into;

    fn library() -> Library {
        let mut lib = Library::new();
        for l in levels() {
            if l.section == SECTION {
                break;
            }
            lib.insert(crate::reference::reference(l.chip));
        }
        lib
    }

    /// The limit follows the player's adder: with the smallest gates
    /// (shorter paths everywhere) the example solutions still pass.
    #[test]
    fn limit_follows_the_players_adder() {
        let best = crate::cost::best_library();
        let ripple = path(best, best.get("Adder32").unwrap()).unwrap();
        let fast = path(best, &reference("FastAdd32").unwrap()).unwrap();
        assert!(
            ripple < 131 && fast * 10 <= ripple * 6,
            "{fast} vs {ripple}"
        );
    }

    #[test]
    fn references_pass_and_are_fast_enough() {
        let mut lib = library();
        for level in LEVELS {
            assert!(level.optional());
            let def = reference(level.chip).unwrap();
            assert_eq!(
                check(level, &lib, &def).unwrap(),
                Verdict::Pass,
                "{}",
                level.chip
            );
            lib.insert(def);
        }
    }

    /// The app checks against a library with fast models (Adder8 and
    /// Adder32 natives); the depth must still come from the gates.
    #[test]
    fn fast_models_do_not_hide_the_path() {
        let defs: Vec<ChipDef> = levels()
            .iter()
            .take_while(|l| l.section != SECTION)
            .map(|l| crate::reference::reference(l.chip))
            .collect();
        let lib = crate::fast::library(&defs, true);
        assert_eq!(
            check(
                find("FastAdd32").unwrap(),
                &lib,
                &reference("FastAdd32").unwrap()
            )
            .unwrap(),
            Verdict::Pass
        );
        let mut slow = crate::reference::reference("Adder32");
        slow.name = "FastAdd32".into();
        assert!(matches!(
            check(find("FastAdd32").unwrap(), &lib, &slow).unwrap(),
            Verdict::Wrong { .. }
        ));
    }

    /// The plain ripple adder works but is too slow for the fast levels.
    #[test]
    fn ripple_adder_is_too_slow() {
        let lib = library();
        let mut def = crate::reference::reference("Adder32");
        def.name = "FastAdd32".into();
        match check(find("FastAdd32").unwrap(), &lib, &def).unwrap() {
            Verdict::Wrong { message, .. } => assert!(message.contains("131"), "{message}"),
            v => panic!("expected too slow, got {v:?}"),
        }
    }

    /// A pipeline needs its register: a plain adder gives the sum at once,
    /// one cycle too early.
    #[test]
    fn unpipelined_adder_fails() {
        let lib = library();
        let mut b = ChipBuilder::new("PipeAdd32", &["a[32]", "b[32]", "clk"], &["sum[32]"]);
        let (x, y) = (b.input("a"), b.input("b"));
        let zero = b.constant(1, 0);
        let s = b.chip("Adder32", &[x, y, zero]);
        b.output("sum", out(s, 0));
        assert!(matches!(
            check(find("PipeAdd32").unwrap(), &lib, &b.build()).unwrap(),
            Verdict::Fail(_)
        ));
    }

    /// Forwarding from x0 would feed garbage into an instruction.
    #[test]
    fn forwarding_x0_fails() {
        let lib = library();
        let mut def = reference("Forward").unwrap();
        // Replace ex_ok (ex_we AND ex_rd ≠ 0) with ex_we alone.
        let and = def
            .parts
            .iter()
            .position(|p| p.kind == PartKind::Chip("And".into()))
            .unwrap();
        let w = def.wires.iter().position(|w| w.to == into(and, 1)).unwrap();
        def.wires[w].from = def
            .wires
            .iter()
            .find(|w| w.to == into(and, 0))
            .unwrap()
            .from;
        assert!(matches!(
            check(find("Forward").unwrap(), &lib, &def).unwrap(),
            Verdict::Fail(_)
        ));
    }

    /// A cache that ignores the tag hits on the wrong address.
    #[test]
    fn cache_without_tag_fails() {
        let lib = library();
        let mut def = reference("Cache4").unwrap();
        let hit = def
            .wires
            .iter()
            .position(|w| w.to == makine_sim::Sink::Output(0))
            .unwrap();
        // hit = valid alone.
        let and = match def.wires[hit].from {
            Source::Part { part, .. } => part,
            _ => unreachable!(),
        };
        let valid = def
            .wires
            .iter()
            .find(|w| w.to == into(and, 0))
            .unwrap()
            .from;
        def.wires[hit].from = valid;
        assert!(matches!(
            check(find("Cache4").unwrap(), &lib, &def).unwrap(),
            Verdict::Fail(_)
        ));
    }
}
