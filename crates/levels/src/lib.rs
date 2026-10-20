//! The learning path: each level asks the user to build one chip, using only
//! the built-in parts and the chips from earlier levels. Passing a level adds
//! the user's chip to their library.

use makine_sim::{
    check_combinational, check_sequence, function_table, mask, parse_pins, partial_table, ChipDef,
    Circuit, CompileError, Library, PartKind, Pin, Rng, TestStep, Verdict,
};

pub enum Test {
    /// Combinational chip: outputs are a pure function of inputs.
    Function(fn(&[u64]) -> Vec<u64>),
    /// Combinational, but some outputs are "don't care" for some inputs.
    Partial(fn(&[u64]) -> Vec<Option<u64>>),
    /// Sequential chip: steps run in order and state carries over.
    Sequence(fn() -> Vec<TestStep>),
    /// Anything else: drives the compiled circuit itself.
    Custom(fn(&mut Circuit) -> Verdict),
}

pub struct Level {
    /// Name of the chip being built; also the level id.
    pub chip: &'static str,
    pub section: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// Pin specs, see [`Pin::parse`].
    pub inputs: &'static [&'static str],
    pub outputs: &'static [&'static str],
    pub test: Test,
}

impl Level {
    pub fn input_pins(&self) -> Vec<Pin> {
        parse_pins(self.inputs)
    }

    pub fn output_pins(&self) -> Vec<Pin> {
        parse_pins(self.outputs)
    }

    /// Test vectors, for table-driven tests (empty for custom tests).
    pub fn steps(&self) -> Vec<TestStep> {
        match self.test {
            Test::Function(f) => function_table(&self.input_pins(), f),
            Test::Partial(f) => partial_table(&self.input_pins(), f),
            Test::Sequence(f) => f(),
            Test::Custom(_) => Vec::new(),
        }
    }

    /// An empty chip with the right pins, for the user to start from.
    pub fn blank(&self) -> ChipDef {
        ChipDef::new(self.chip, self.inputs, self.outputs)
    }

    /// Chips from earlier levels, which may be used in this one.
    pub fn allowed_chips(&self) -> impl Iterator<Item = &'static Level> {
        let me = self.chip;
        let section = self.section;
        levels()
            .iter()
            .copied()
            .take_while(move |l| l.chip != me)
            // Chips from an optional section only inside that section.
            .filter(move |l| !l.optional() || l.section == section)
    }

    /// Whether the level is a side path: it unlocks like the others, but
    /// later levels do not wait for it.
    pub fn optional(&self) -> bool {
        self.section == mini::SECTION
            || self.section == memory::SECTION
            || self.section == speed::SECTION
            || self.section == gpu::SECTION
    }

    /// The title in the interface language.
    pub fn name(&self) -> &'static str {
        match en::level(self.chip) {
            Some((t, _)) if makine_lang::is_en() => t,
            _ => self.title,
        }
    }

    /// Both localized titles without changing the interface language. Useful
    /// for finding a chip by either the lesson title or its stable chip id.
    pub fn names(&self) -> [&'static str; 2] {
        [
            self.title,
            en::level(self.chip).map_or(self.title, |(title, _)| title),
        ]
    }

    /// The description in the interface language.
    pub fn about(&self) -> &'static str {
        match en::level(self.chip) {
            Some((_, d)) if makine_lang::is_en() => d,
            _ => self.description,
        }
    }

    /// The section's name in the interface language (`section` itself is
    /// the Turkish name, used to group levels).
    pub fn section_name(&self) -> &'static str {
        if makine_lang::is_en() {
            en::section(self.section).unwrap_or(self.section)
        } else {
            self.section
        }
    }

    pub fn index(&self) -> usize {
        levels().iter().position(|l| l.chip == self.chip).unwrap()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CheckError {
    WrongPins,
    ForbiddenPart(String),
    Compile(CompileError),
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckError::WrongPins => f.write_str(makine_lang::tr!(
                "çipin giriş/çıkışları seviyeyle uyuşmuyor",
                "the chip's inputs/outputs do not match the level"
            )),
            CheckError::ForbiddenPart(p) => f.write_str(&makine_lang::trf!(
                "{p} bu seviyede henüz kullanılamaz",
                "{p} cannot be used in this level yet"
            )),
            CheckError::Compile(e) => write!(f, "{e}"),
        }
    }
}

/// Check a user's solution. `lib` holds the chips the user has built so far.
pub fn check(level: &Level, lib: &Library, def: &ChipDef) -> Result<Verdict, CheckError> {
    if def.inputs != level.input_pins() || def.outputs != level.output_pins() {
        return Err(CheckError::WrongPins);
    }
    for p in &def.parts {
        if let PartKind::Chip(name) = &p.kind {
            if !level.allowed_chips().any(|l| l.chip == name) {
                return Err(CheckError::ForbiddenPart(name.clone()));
            }
        }
    }
    let mut circuit = Circuit::compile_def(lib, def).map_err(CheckError::Compile)?;
    let steps = level.steps();
    let verdict = match level.test {
        Test::Function(_) | Test::Partial(_) => {
            check_combinational(&mut circuit, &level.input_pins(), &steps)
        }
        Test::Sequence(_) => check_sequence(&mut circuit, &steps),
        Test::Custom(f) => f(&mut circuit),
    };
    // Levels about speed also fail a working circuit that is too slow.
    if verdict.passed() {
        if let Some(slow) = speed::check_depth(level.chip, lib, def) {
            return Ok(slow);
        }
    }
    Ok(verdict)
}

pub fn find(chip: &str) -> Option<&'static Level> {
    levels().iter().copied().find(|l| l.chip == chip)
}

/// Every level, in order.
pub fn levels() -> &'static [&'static Level] {
    static ALL: std::sync::OnceLock<Vec<&'static Level>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        BASIC
            .iter()
            .chain(memory::LEVELS)
            .chain(mini::LEVELS)
            .chain(cpu::LEVELS)
            .chain(workshop::LEVELS)
            .chain(speed::LEVELS)
            .chain(gpu::LEVELS)
            .collect()
    })
}

pub mod cost;
pub mod cpu;
pub mod depth;
mod en;
pub mod fast;
pub mod gpu;
pub mod hints;
pub mod memory;
pub mod mini;
pub mod reference;
mod reference_cpu;
pub mod speed;
pub mod teaching;
pub mod workshop;

const LOGIC: &str = "1. Mantık";
const ARITH: &str = "2. Aritmetik";
const MEMORY: &str = "3. Hafıza";

static BASIC: &[Level] = &[
    Level {
        chip: "Not",
        section: LOGIC,
        title: "DEĞİL (NOT)",
        description: "Girişin tersini ver: 0 ise 1, 1 ise 0.\n\n\
            Elinde sadece NAND var. NAND'ın iki girişine aynı sinyali verirsen ne olur?",
        inputs: &["in"],
        outputs: &["out"],
        test: Test::Function(|i| vec![!i[0] & 1]),
    },
    Level {
        chip: "And",
        section: LOGIC,
        title: "VE (AND)",
        description: "İki giriş de 1 ise çıkış 1 olsun.\n\n\
            NAND zaten 'VE DEĞİL' demek. Tersini almak için az önce yaptığın NOT'u kullan.",
        inputs: &["a", "b"],
        outputs: &["out"],
        test: Test::Function(|i| vec![i[0] & i[1]]),
    },
    Level {
        chip: "Or",
        section: LOGIC,
        title: "VEYA (OR)",
        description: "Girişlerden en az biri 1 ise çıkış 1 olsun.\n\n\
            İpucu: De Morgan kuralı. a VEYA b = DEĞİL(DEĞİL a VE DEĞİL b).",
        inputs: &["a", "b"],
        outputs: &["out"],
        test: Test::Function(|i| vec![i[0] | i[1]]),
    },
    Level {
        chip: "Xor",
        section: LOGIC,
        title: "ÖZEL VEYA (XOR)",
        description: "Girişler farklıysa çıkış 1 olsun.\n\n\
            XOR, toplama devresinin kalbidir: 1 + 1 = 10, alt basamak 0.",
        inputs: &["a", "b"],
        outputs: &["out"],
        test: Test::Function(|i| vec![i[0] ^ i[1]]),
    },
    Level {
        chip: "Mux",
        section: LOGIC,
        title: "Seçici (MUX)",
        description: "sel = 0 ise a'yı, sel = 1 ise b'yi çıkışa ver.\n\n\
            Bilgisayarın her yerinde 'hangi veri gitsin?' sorusunu MUX cevaplar.",
        inputs: &["a", "b", "sel"],
        outputs: &["out"],
        test: Test::Function(|i| vec![if i[2] == 1 { i[1] } else { i[0] }]),
    },
    Level {
        chip: "Demux",
        section: LOGIC,
        title: "Dağıtıcı (DEMUX)",
        description: "MUX'un tersi: girişi sel = 0 ise a'ya, sel = 1 ise b'ye gönder.\n\
            Diğer çıkış 0 kalsın.",
        inputs: &["in", "sel"],
        outputs: &["a", "b"],
        test: Test::Function(|i| vec![i[0] & !i[1] & 1, i[0] & i[1]]),
    },
    Level {
        chip: "Mux32",
        section: LOGIC,
        title: "32-bit Seçici",
        description: "Artık tek tek bitlerle değil, 32 bitlik sayılarla (bus) çalışıyoruz.\n\n\
            sel = 0 ise a'yı, sel = 1 ise b'yi ver. 32 tane MUX koymana gerek yok: \
            bir parçayı seçip 'Tekrar' sayısını 32 yaparsan 32 kopyası yan yana çalışır. \
            1 bitlik bir teli geniş bir girişe bağlarsan o bit bütün kopyalara dağıtılır.",
        inputs: &["a[32]", "b[32]", "sel"],
        outputs: &["out[32]"],
        test: Test::Function(|i| vec![if i[2] == 1 { i[1] } else { i[0] }]),
    },
    Level {
        chip: "HalfAdder",
        section: ARITH,
        title: "Yarım Toplayıcı",
        description: "İki biti topla. sum alt basamak, carry elde.\n\n\
            0+0=00, 0+1=01, 1+0=01, 1+1=10",
        inputs: &["a", "b"],
        outputs: &["sum", "carry"],
        test: Test::Function(|i| vec![i[0] ^ i[1], i[0] & i[1]]),
    },
    Level {
        chip: "FullAdder",
        section: ARITH,
        title: "Tam Toplayıcı",
        description: "Üç biti topla: a, b ve önceki basamaktan gelen elde (c).\n\n\
            Bunları yan yana dizince istediğin genişlikte sayı toplayabilirsin.",
        inputs: &["a", "b", "c"],
        outputs: &["sum", "carry"],
        test: Test::Function(|i| {
            let n = i[0] + i[1] + i[2];
            vec![n & 1, n >> 1]
        }),
    },
    Level {
        chip: "Adder8",
        section: ARITH,
        title: "8-bit Toplayıcı",
        description: "İki 8 bitlik sayıyı ve gelen eldeyi (c) topla.\n\n\
            Bus'ı bitlerine ayırmak için Böl (Split), bitleri tekrar bus yapmak için \
            Birleştir (Join) parçalarını kullan. 8 tam toplayıcıyı zincirle: \
            her birinin eldesi bir sonrakine gider. Bit 0 en düşük basamaktır.",
        inputs: &["a[8]", "b[8]", "c"],
        outputs: &["sum[8]", "carry"],
        test: Test::Function(|i| {
            let n = i[0] + i[1] + i[2];
            vec![n & 0xFF, n >> 8]
        }),
    },
    Level {
        chip: "Adder32",
        section: ARITH,
        title: "32-bit Toplayıcı",
        description: "Dört tane 8-bit toplayıcıyı zincirle.\n\n\
            RISC-V işlemcin tam olarak bu genişlikte sayılarla çalışacak.",
        inputs: &["a[32]", "b[32]", "c"],
        outputs: &["sum[32]", "carry"],
        test: Test::Function(|i| {
            let n = i[0] + i[1] + i[2];
            vec![n & 0xFFFF_FFFF, n >> 32]
        }),
    },
    Level {
        chip: "Sub32",
        section: ARITH,
        title: "32-bit Çıkarıcı",
        description: "a - b hesapla.\n\n\
            İkiye tümleyen sayılarda -b = DEĞİL(b) + 1. Yani a - b = a + DEĞİL(b) + 1. \
            '+1' için toplayıcının elde girişini kullanabilirsin: Sabit (Const) parçası \
            her zaman aynı değeri verir.",
        inputs: &["a[32]", "b[32]"],
        outputs: &["out[32]"],
        test: Test::Function(|i| vec![i[0].wrapping_sub(i[1]) & 0xFFFF_FFFF]),
    },
    Level {
        chip: "Latch",
        section: MEMORY,
        title: "Mandal (SR Latch)",
        description: "Hafızanın doğduğu yer! set = 1 olunca q 1 olsun, reset = 1 olunca 0.\n\
            İkisi de 0 iken q eski değerini KORUSUN.\n\n\
            İpucu: iki NAND'ın çıkışını birbirinin girişine bağla (geri besleme).",
        inputs: &["set", "reset"],
        outputs: &["q"],
        test: Test::Sequence(latch_steps),
    },
    Level {
        chip: "DLatch",
        section: MEMORY,
        title: "D Mandalı",
        description: "en = 1 iken q, d'yi takip etsin. en = 0 olunca son değeri tutsun.\n\n\
            SR mandalının set ve reset girişlerini d ve en'den üret.",
        inputs: &["d", "en"],
        outputs: &["q"],
        test: Test::Sequence(dlatch_steps),
    },
    Level {
        chip: "DFF",
        section: MEMORY,
        title: "D Flip-Flop",
        description:
            "Saat (clk) 0'dan 1'e geçtiği AN d'yi kaydet; geri kalan her zaman q sabit kalsın.\n\n\
            Bilgisayardaki her şey bu saat vuruşlarıyla adım adım ilerler. \
            İpucu: iki D mandalını arka arkaya koy (usta-çırak). clk = 0 iken birincisi açık, \
            clk = 1 iken ikincisi açık olsun.",
        inputs: &["d", "clk"],
        outputs: &["q"],
        test: Test::Sequence(dff_steps),
    },
    Level {
        chip: "Bit",
        section: MEMORY,
        title: "1-bit Kayıt",
        description:
            "load = 1 iken saat vuruşunda d'yi kaydet, load = 0 iken eski değeri koru.\n\n\
            İpucu: DFF'in girişine bir MUX koy: ya yeni değer ya kendi çıkışı.",
        inputs: &["d", "load", "clk"],
        outputs: &["q"],
        test: Test::Sequence(bit_steps),
    },
    Level {
        chip: "Register32",
        section: MEMORY,
        title: "32-bit Register",
        description: "32 tane Bit'i yan yana koy: işlemcinin her register'ı böyle bir şey.\n\n\
            'Tekrar' ayarını hatırla.",
        inputs: &["d[32]", "load", "clk"],
        outputs: &["q[32]"],
        test: Test::Sequence(register32_steps),
    },
    Level {
        chip: "Counter32",
        section: MEMORY,
        title: "Program Sayacı",
        description: "Her saat vuruşunda: reset = 1 ise 0'a dön, değilse load = 1 ise d'yi yükle, \
            değilse inc = 1 ise 4 artır (bir RISC-V komutu 4 bayttır), değilse aynı kal.\n\n\
            İşlemci 'sıradaki komut nerede?' sorusunu bu sayaçla cevaplar.",
        inputs: &["d[32]", "load", "inc", "reset", "clk"],
        outputs: &["out[32]"],
        test: Test::Sequence(counter_steps),
    },
    Level {
        chip: "Ram8",
        section: MEMORY,
        title: "RAM (8 kelime)",
        description: "Bellek: 8 tane 32-bit kelime. addr hangi kelime olduğunu seçer. \
            q her zaman addr'deki kelimeyi gösterir; saat vuruşunda we = 1 ise d oraya yazılır.\n\n\
            8 Register32, yazılacak olanı seçen bir dağıtıcı (Demux ağacı) ve okunacak olanı \
            seçen bir seçici (MUX ağacı). Bilgisayarının ana belleği aynı fikrin milyonlarca \
            kelimeye büyütülmüş hali; onu Rust ile simüle ediyoruz.",
        inputs: &["addr[3]", "d[32]", "we", "clk"],
        outputs: &["q[32]"],
        test: Test::Sequence(ram8_steps),
    },
];

fn ram8_steps() -> Vec<TestStep> {
    let mut rng = Rng(8);
    let mut mem = [None::<u64>; 8];
    let mut steps = Vec::new();
    for cycle in 0..200 {
        let addr = if cycle < 8 { cycle } else { rng.next_u64() % 8 };
        let d = rng.next_u64() & mask(32);
        let we = if cycle < 8 { 1 } else { rng.bit() };
        let peek = rng.next_u64() % 8;
        let at = |a: u64, mem: &[Option<u64>; 8]| vec![mem[a as usize]];
        steps.push(TestStep {
            inputs: vec![addr, d, we, 0],
            expect: at(addr, &mem),
        });
        if we == 1 {
            mem[addr as usize] = Some(d);
        }
        steps.push(TestStep {
            inputs: vec![addr, d, we, 1],
            expect: at(addr, &mem),
        });
        // Reading another address needs no clock.
        steps.push(TestStep {
            inputs: vec![peek, d, we, 1],
            expect: at(peek, &mem),
        });
    }
    steps
}

fn latch_steps() -> Vec<TestStep> {
    // (set, reset, expected q)
    let rows = [
        (1, 0, 1),
        (0, 0, 1),
        (0, 0, 1),
        (0, 1, 0),
        (0, 0, 0),
        (1, 0, 1),
        (0, 0, 1),
        (0, 1, 0),
        (0, 0, 0),
    ];
    rows.iter()
        .map(|&(s, r, q)| TestStep {
            inputs: vec![s, r],
            expect: vec![Some(q)],
        })
        .collect()
}

fn dlatch_steps() -> Vec<TestStep> {
    let mut rng = Rng(1);
    let mut q = None;
    let mut steps = Vec::new();
    for i in 0..64 {
        let d = rng.bit();
        // Start with en = 1 so q gets a known value.
        let en = if i == 0 { 1 } else { rng.bit() };
        if en == 1 {
            q = Some(d);
        }
        steps.push(TestStep {
            inputs: vec![d, en],
            expect: vec![q],
        });
    }
    steps
}

/// Run a clocked model for `cycles` cycles. `inputs(rng)` gives the inputs
/// for a cycle without the clock; `model(state, inputs)` returns the state
/// after the rising edge. Each cycle becomes three steps: clock low (output
/// still the old state), clock high (output now the new state), and clock
/// high with fresh inputs (output must not change: proves edge triggering).
fn clocked(
    cycles: usize,
    seed: u64,
    mut inputs: impl FnMut(&mut Rng, usize) -> Vec<u64>,
    mut model: impl FnMut(Option<u64>, &[u64]) -> Option<u64>,
) -> Vec<TestStep> {
    let mut rng = Rng(seed);
    let mut state: Option<u64> = None;
    let mut steps = Vec::new();
    for cycle in 0..cycles {
        let ins = inputs(&mut rng, cycle);
        let with_clk = |clk| ins.iter().copied().chain([clk]).collect::<Vec<_>>();
        steps.push(TestStep {
            inputs: with_clk(0),
            expect: vec![state],
        });
        state = model(state, &ins);
        steps.push(TestStep {
            inputs: with_clk(1),
            expect: vec![state],
        });
        let noise: Vec<u64> = inputs(&mut rng, cycle).into_iter().chain([1]).collect();
        steps.push(TestStep {
            inputs: noise,
            expect: vec![state],
        });
    }
    steps
}

fn dff_steps() -> Vec<TestStep> {
    clocked(48, 2, |rng, _| vec![rng.bit()], |_, i| Some(i[0]))
}

fn bit_steps() -> Vec<TestStep> {
    clocked(
        64,
        3,
        |rng, cycle| vec![rng.bit(), if cycle == 0 { 1 } else { rng.bit() }],
        |q, i| if i[1] == 1 { Some(i[0]) } else { q },
    )
}

fn register32_steps() -> Vec<TestStep> {
    clocked(
        64,
        4,
        |rng, cycle| {
            vec![
                rng.next_u64() & mask(32),
                if cycle == 0 { 1 } else { rng.bit() },
            ]
        },
        |q, i| if i[1] == 1 { Some(i[0]) } else { q },
    )
}

fn counter_steps() -> Vec<TestStep> {
    clocked(
        96,
        5,
        |rng, cycle| {
            let d = rng.next_u64() & mask(32);
            let r = rng.next_u64() % 16;
            // Mostly count, sometimes load or reset; start with a reset.
            let (load, inc, reset) = match (cycle, r) {
                (0, _) => (0, 0, 1),
                (_, 0) => (0, 1, 1),
                (_, 1 | 2) => (1, 1, 0),
                (_, 3) => (0, 0, 0),
                _ => (0, 1, 0),
            };
            vec![d, load, inc, reset]
        },
        |q, i| {
            let (d, load, inc, reset) = (i[0], i[1], i[2], i[3]);
            if reset == 1 {
                Some(0)
            } else if load == 1 {
                Some(d)
            } else if inc == 1 {
                q.map(|q| q.wrapping_add(4) & mask(32))
            } else {
                q
            }
        },
    )
}
