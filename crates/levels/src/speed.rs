//! An optional side path after the processor: what makes processors fast.
//! A shorter carry path, a pipeline register, forwarding between pipeline
//! stages and a cache.
//!
//! Speed is measured, not just function: the adder levels also fail when
//! their longest gate path is not clearly shorter than the player's own
//! Adder32's (see [`check_depth`]).

use makine_lang::trf;
use makine_rv::emu::mul_div;
use makine_sim::{
    into, mask, out, ChipBuilder, ChipDef, Circuit, Library, Part, PartKind, Rng, Source, TestStep,
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
    Level {
        chip: "SeqMulDiv",
        section: SECTION,
        title: "Adım Adım Çarpma ve Bölme",
        description:
            "MulDiv'in yaptığını yap, ama her saatte bir adım. funct3 MulDiv'deki gibidir.\n\
            go = 1 iken her yükselen clk kenarı işi bir adım ilerletir. Sonuç hazır olunca \
            ready = 1 olur ve out sonucu gösterir. ready = 1 iken go = 1 olan kenar işi bitirir: \
            ready 0'a döner, sonraki kenar yeni bir işlem başlatır. go = 0 olan kenar işi \
            bırakır. go = 1 olduğundan ready'ye kadar a, b ve funct3 değişmez. En fazla 64 kenar \
            sürebilir; en uzun kapı yolu kendi Adder32'ninkinin 1,5 katını geçmesin.\n\n\
            MulDiv 32 toplamayı ve 32 çıkarmayı art arda yapar: işlemcinin en uzun yolu odur. \
            Burada her kenarda tek bir toplayıcı çalışır. Çarpma: kalan {A, Q} çiftini her adımda \
            sağa kaydır, Q'nun en düşük biti 1 ise önce A'ya çarpılanı ekle. Bölme: {A, Q}'yu sola \
            kaydır, bölen sığıyorsa A'dan çıkar ve bölüm bitini 1 yap. İşaretleri başta ve sonda \
            birer adımda düzelt.",
        inputs: &["a[32]", "b[32]", "funct3[3]", "go", "clk"],
        outputs: &["out[32]", "ready"],
        test: Test::Custom(seq_muldiv_test),
    },
    Level {
        chip: "CpuSeq",
        section: SECTION,
        title: "Hızlı Saatli İşlemci",
        description: "RISC-V işlemcini yeniden kur, bu kez MulDiv yerine SeqMulDiv ile. Uçlar \
            İşlemci seviyesindekilerin aynısı. Çarpma ya da bölme komutunda go = 1 olsun; \
            ready gelene kadar pc ilerlemesin ve register yazılmasın (işlemci bekler). ready \
            gelince sonucu rd'ye yaz ve devam et. Öteki komutlar yine bir saatte biter. En uzun \
            kapı yolu kendi Adder32'ninkinin 3 katını geçmesin.\n\n\
            Saat ancak en uzun yol durulduktan sonra vurabilir. Tek döngülü işlemcide bu yol \
            bölücüden geçer ve öteki bütün komutları da yavaşlatır. Bölme artık 35 saat sürer \
            ama saat 20 kat kadar hızlı vurabilir; programlarda bölme seyrek olduğu için \
            hepsi hızlanır. FPGA bilgisayarı bu işlemciyle çok daha hızlı çalışır.",
        inputs: &["instr[32]", "rdata[32]", "reset", "clk"],
        outputs: &["pc[32]", "addr[32]", "wdata[32]", "wmask[4]", "we"],
        test: Test::Custom(crate::cpu::cpu_test),
    },
];

/// Edges the multi-step multiplier/divider may take for one operation.
pub const SEQ_MAX_EDGES: usize = 64;

/// The longest path allowed, as a share of the player's own Adder32's:
/// the levels about speed must beat the adder they started from.
pub fn depth_share(chip: &str) -> Option<f64> {
    Some(match chip {
        "FastAdd32" | "PipeAdd32" => 0.6,
        "SeqMulDiv" => 1.5,
        "CpuSeq" => 3.0,
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
            "Devre doğru çalışıyor ama yavaş: en uzun yol {depth} NAND. Senin Adder32'ninki {adder}; bu seviyede en fazla {limit} olabilir. Saat ancak en uzun yol durulduktan sonra vurabilir.",
            "The circuit works but is slow: the longest path is {depth} NANDs. Your Adder32's is {adder}; this level allows at most {limit}. The clock can only tick once the longest path has settled."
        ),
    })
}

/// Operations the multi-step multiplier/divider test runs: the edge cases
/// of every funct3, then random ones.
fn seq_cases() -> Vec<(u32, u32, u32)> {
    let edges = [
        (123456, (-789i32) as u32),
        (0x8000_0000, u32::MAX),
        (7, (-2i32) as u32),
        ((-7i32) as u32, 2),
        (u32::MAX, u32::MAX),
        (0x8000_0000, 0x8000_0000),
        (12345, 0),
        ((-12345i32) as u32, 0),
        (0, 5),
    ];
    let mut v = Vec::new();
    for f3 in 0..8 {
        for &(a, b) in &edges {
            v.push((a, b, f3));
        }
    }
    let mut rng = Rng(72);
    for _ in 0..48 {
        let a = rng.next_u64() as u32;
        let b = rng.next_u64() as u32 >> (rng.next_u64() % 32);
        v.push((a, b, (rng.next_u64() % 8) as u32));
    }
    v
}

fn op_name(f3: u32) -> &'static str {
    [
        "mul", "mulh", "mulhsu", "mulhu", "div", "divu", "rem", "remu",
    ][f3 as usize & 7]
}

/// Drives a multi-step multiplier/divider: starts each operation with go,
/// waits for ready, checks the result and that finishing clears ready.
fn seq_muldiv_test(c: &mut Circuit) -> Verdict {
    const A: usize = 0;
    const B: usize = 1;
    const F3: usize = 2;
    const GO: usize = 3;
    const CLK: usize = 4;
    c.set_native_lanes(1);
    c.reset();
    let mut step = 0;
    let mut eval = |c: &mut Circuit, clk: u64| -> Result<(), Verdict> {
        step += 1;
        c.set_input(CLK, clk);
        c.eval().map_err(|_| Verdict::Oscillates { step })
    };
    let wrong = |step: usize, message: String| Verdict::Wrong { step, message };
    let run = |c: &mut Circuit, eval: &mut dyn FnMut(&mut Circuit, u64) -> Result<(), Verdict>| {
        // An edge with go = 0 leaves any operation: the circuit is idle.
        c.set_input(GO, 0);
        eval(c, 0)?;
        eval(c, 1)?;
        eval(c, 0)?;
        for (k, (a, b, f3)) in seq_cases().into_iter().enumerate() {
            let what = format!("{} 0x{a:08X}, 0x{b:08X}", op_name(f3));
            c.set_input(A, a as u64);
            c.set_input(B, b as u64);
            c.set_input(F3, f3 as u64);
            c.set_input(GO, 1);
            eval(c, 0)?;
            if c.output(1) & 1 == 1 {
                return Err(wrong(
                    k,
                    trf!(
                        "{what}: ready, işlem başlamadan 1 oldu.",
                        "{what}: ready was 1 before the operation started."
                    ),
                ));
            }
            // Now and then the operation is left half way (go = 0): the
            // next one must start from scratch.
            if k % 7 == 3 {
                for _ in 0..5 {
                    eval(c, 1)?;
                    eval(c, 0)?;
                }
                c.set_input(GO, 0);
                eval(c, 1)?;
                eval(c, 0)?;
                c.set_input(GO, 1);
                eval(c, 0)?;
            }
            let mut edges = 0;
            while c.output(1) & 1 == 0 {
                if edges == SEQ_MAX_EDGES {
                    return Err(wrong(
                        k,
                        trf!(
                            "{what}: {SEQ_MAX_EDGES} saat kenarından sonra ready hâlâ 0.",
                            "{what}: ready is still 0 after {SEQ_MAX_EDGES} clock edges."
                        ),
                    ));
                }
                eval(c, 1)?;
                eval(c, 0)?;
                edges += 1;
            }
            let want = mul_div(f3, a, b);
            let got = c.output(0) as u32;
            if got != want {
                return Err(wrong(
                    k,
                    trf!(
                        "{what} = 0x{want:08X} olmalıydı, devren {edges} kenardan sonra 0x{got:08X} verdi.",
                        "{what} should be 0x{want:08X}; your circuit gave 0x{got:08X} after {edges} edges."
                    ),
                ));
            }
            // The edge that takes the result ends the operation.
            eval(c, 1)?;
            if k % 5 == 1 {
                c.set_input(GO, 0);
            }
            eval(c, 0)?;
            if c.output(1) & 1 == 1 {
                return Err(wrong(
                    k,
                    trf!(
                        "{what}: sonuç alındıktan sonraki kenarda ready 0'a dönmeli.",
                        "{what}: ready must go back to 0 at the edge that takes the result."
                    ),
                ));
            }
        }
        Ok(())
    };
    match run(c, &mut eval) {
        Ok(()) => Verdict::Pass,
        Err(v) => v,
    }
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
        "SeqMulDiv" => seq_muldiv(&mut b),
        "CpuSeq" => crate::reference_cpu::cpu_seq(&mut b),
        _ => return None,
    }
    Some(b.build())
}

/// Multiply and divide one step per clock: a load, 32 shift-and-add (or
/// shift-and-subtract) steps on unsigned magnitudes, and two steps that fix
/// the signs. Each edge goes through one 32-bit adder.
///
/// State: A (high word / remainder, then the result), Q (low word /
/// quotient), M (multiplicand / divisor), a 6-bit step count and two flags,
/// busy and done.
fn seq_muldiv(b: &mut ChipBuilder) {
    let (x, y, f3) = (b.input("a"), b.input("b"), b.input("funct3"));
    let (go, clk) = (b.input("go"), b.input("clk"));
    let zero1 = b.constant(1, 0);
    let one1 = b.constant(1, 1);
    let zero32 = b.constant(32, 0);
    let fs = b.split(&[1, 1, 1], f3);
    let (f0, f1, f2) = (out(fs, 0), out(fs, 1), out(fs, 2));
    let xs = b.split(&[31, 1], x);
    let ys = b.split(&[31, 1], y);
    let (sa, sb) = (out(xs, 1), out(ys, 1));

    // Signed division works on magnitudes.
    let nf0 = b.chip1("Not", &[f0]);
    let signed_div = b.chip1("And", &[f2, nf0]);
    let neg_a = b.chip1("And", &[sa, signed_div]);
    let neg_b = b.chip1("And", &[sb, signed_div]);
    let na = b.chip1("Sub32", &[zero32, x]);
    let nb = b.chip1("Sub32", &[zero32, y]);
    let ma = b.chip1("Mux32", &[x, na, neg_a]);
    let mb = b.chip1("Mux32", &[y, nb, neg_b]);

    // State; inputs wired at the end.
    let reg = |b: &mut ChipBuilder| b.part(Part::new(PartKind::Chip("Register32".into())), &[]);
    let (ra, rq, rm) = (reg(b), reg(b), reg(b));
    let rc = b.part(
        Part {
            kind: PartKind::Chip("Bit".into()),
            repeat: 8,
        },
        &[],
    );
    let (a_q, q_q, m_q) = (out(ra, 0), out(rq, 0), out(rm, 0));
    let ctl = b.split(&[1, 4, 1, 1, 1], out(rc, 0));
    let (c0, c5, busy, done) = (out(ctl, 0), out(ctl, 2), out(ctl, 3), out(ctl, 4));
    let cnt = b.split(&[6, 2], out(rc, 0));

    let active = b.chip1("Or", &[busy, done]);
    let idle = b.chip1("Not", &[active]);
    let nc5 = b.chip1("Not", &[c5]);
    let stepping = b.chip1("And", &[busy, nc5]);
    let fixing = b.chip1("And", &[busy, c5]);
    let fix2 = b.chip1("And", &[fixing, c0]);

    // One multiply step: {A, Q} >>= 1, after A += M when Q[0] is 1.
    let qs = b.split(&[1, 31], q_q);
    let addend = b.chip1("Mux32", &[zero32, m_q, out(qs, 0)]);
    let s = b.chip("Adder32", &[a_q, addend, zero1]);
    let ss = b.split(&[1, 31], out(s, 0));
    let a_mul = b.join(&[31, 1], &[out(ss, 1), out(s, 1)]);
    let q_mul = b.join(&[31, 1], &[out(qs, 1), out(ss, 0)]);

    // One divide step: {A, Q} <<= 1, then A -= M if it fits.
    let as_ = b.split(&[31, 1], a_q);
    let qd = b.split(&[31, 1], q_q);
    let shifted = b.join(&[1, 31], &[out(qd, 1), out(as_, 0)]);
    let nm = b.chip_n("Not", 32, &[m_q]);
    let diff = b.chip("Adder32", &[shifted, out(nm, 0), one1]);
    let ge = b.chip1("Or", &[out(diff, 1), out(as_, 1)]);
    let a_div = b.chip1("Mux32", &[shifted, out(diff, 0), ge]);
    let q_div = b.join(&[1, 31], &[ge, out(qd, 0)]);

    let a_step = b.chip1("Mux32", &[a_mul, a_div, f2]);
    let q_step = b.chip1("Mux32", &[q_mul, q_div, f2]);

    // The fixing steps compute A = X - Y.
    // First: the low word, a high word less a's correction, or a quotient
    // or remainder with its sign.
    let any01 = b.chip1("Or", &[f0, f1]);
    let lo = b.chip1("Not", &[any01]);
    let mul_x1 = b.chip1("Mux32", &[a_q, q_q, lo]);
    let one_of = b.chip1("Xor", &[f0, f1]);
    let c1 = b.chip1("And", &[sa, one_of]);
    let mul_y1 = b.chip1("Mux32", &[zero32, y, c1]);
    let picked = b.chip1("Mux32", &[q_q, a_q, f1]);
    let qneg = b.chip1("Xor", &[neg_a, neg_b]);
    let dneg = b.chip1("Mux", &[qneg, neg_a, f1]);
    let div_x1 = b.chip1("Mux32", &[picked, zero32, dneg]);
    let div_y1 = b.chip1("Mux32", &[zero32, picked, dneg]);
    let x1 = b.chip1("Mux32", &[mul_x1, div_x1, f2]);
    let y1 = b.chip1("Mux32", &[mul_y1, div_y1, f2]);
    // Second: mulh also takes away b's correction.
    let nf1 = b.chip1("Not", &[f1]);
    let nf2 = b.chip1("Not", &[f2]);
    let is_mulh = b.chip1("And", &[f0, nf1]);
    let is_mulh = b.chip1("And", &[is_mulh, nf2]);
    let c2 = b.chip1("And", &[sb, is_mulh]);
    let y2 = b.chip1("Mux32", &[zero32, x, c2]);
    let fx = b.chip1("Mux32", &[x1, a_q, fix2]);
    let fy = b.chip1("Mux32", &[y1, y2, fix2]);
    let fixed = b.chip1("Sub32", &[fx, fy]);

    let work_a = b.chip1("Mux32", &[fixed, a_step, stepping]);
    let a_next = b.chip1("Mux32", &[work_a, zero32, idle]);
    let q0 = b.chip1("Mux32", &[y, ma, f2]);
    let q_next = b.chip1("Mux32", &[q_step, q0, idle]);
    let m0 = b.chip1("Mux32", &[x, mb, f2]);
    let m_next = b.chip1("Mux32", &[m_q, m0, idle]);
    let working = b.chip1("Or", &[idle, busy]);
    let load = b.chip1("And", &[go, working]);
    for (r, d) in [(ra, a_next), (rq, q_next), (rm, m_next)] {
        b.wire(d, into(r, 0));
        b.wire(load, into(r, 1));
        b.wire(clk, into(r, 2));
    }

    // Control: count while busy; done after the second fixing step; any
    // edge with go = 0, or the one after done, goes back to idle.
    let pad = b.constant(2, 0);
    let c8 = b.join(&[6, 2], &[out(cnt, 0), pad]);
    let one8 = b.constant(8, 1);
    let inc = b.chip("Adder8", &[c8, one8, zero1]);
    let incs = b.split(&[6, 2], out(inc, 0));
    let zero6 = b.constant(6, 0);
    let cnt_next = b.chip_n("Mux", 6, &[zero6, out(incs, 0), busy]);
    let nfix2 = b.chip1("Not", &[fix2]);
    let keep = b.chip1("And", &[busy, nfix2]);
    let busy_next = b.chip1("Or", &[idle, keep]);
    let busy_next = b.chip1("And", &[go, busy_next]);
    let done_next = b.chip1("And", &[go, fix2]);
    let ctl_next = b.join(&[6, 1, 1], &[out(cnt_next, 0), busy_next, done_next]);
    b.wire(ctl_next, into(rc, 0));
    b.wire(one1, into(rc, 1));
    b.wire(clk, into(rc, 2));

    // Division by zero: the quotient is all ones, the remainder is a.
    let bz = b.chip1("Zero32", &[y]);
    let div0 = b.chip1("And", &[f2, bz]);
    let ones = b.constant(32, 0xFFFF_FFFF);
    let by0 = b.chip1("Mux32", &[ones, x, f1]);
    let r = b.chip1("Mux32", &[a_q, by0, div0]);
    b.output("out", r);
    b.output("ready", done);
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

    /// The multi-step unit is short: one adder per edge, not 64.
    #[test]
    fn seq_muldiv_is_short() {
        let lib = library();
        let seq = path(&lib, &reference("SeqMulDiv").unwrap()).unwrap();
        let adder = path(&lib, lib.get("Adder32").unwrap()).unwrap();
        let comb = path(&lib, lib.get("MulDiv").unwrap()).unwrap();
        assert!(
            seq * 2 <= adder * 3 && comb > 20 * seq,
            "{seq} {adder} {comb}"
        );
    }

    /// MulDiv renamed passes the function but has no ready: it fails.
    #[test]
    fn combinational_muldiv_is_not_sequential() {
        let lib = library();
        let mut b = ChipBuilder::new(
            "SeqMulDiv",
            &["a[32]", "b[32]", "funct3[3]", "go", "clk"],
            &["out[32]", "ready"],
        );
        let (x, y, f) = (b.input("a"), b.input("b"), b.input("funct3"));
        let m = b.chip1("MulDiv", &[x, y, f]);
        b.output("out", m);
        let one = b.constant(1, 1);
        b.output("ready", one);
        assert!(matches!(
            check(find("SeqMulDiv").unwrap(), &lib, &b.build()).unwrap(),
            Verdict::Wrong { .. }
        ));
    }

    /// The fast model passes the level, and the processor built on the
    /// multi-step unit is many times shorter than the single-cycle one.
    #[test]
    fn cpu_seq_passes_and_is_short() {
        let mut refs: Vec<ChipDef> = levels()
            .iter()
            .take_while(|l| l.chip != "CpuSeq")
            .map(|l| crate::reference::reference(l.chip))
            .collect();
        let fast = crate::fast::library(refs.iter(), true);
        let cpu = reference("CpuSeq").unwrap();
        assert_eq!(
            check(find("CpuSeq").unwrap(), &fast, &cpu).unwrap(),
            Verdict::Pass
        );
        refs.push(cpu.clone());
        let mut lib = Library::new();
        for d in refs {
            lib.insert(d);
        }
        let seq = path(&lib, &cpu).unwrap();
        let single = path(&lib, lib.get("Cpu").unwrap()).unwrap();
        assert!(seq * 15 < single, "{seq} vs {single}");
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
