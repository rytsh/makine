//! Optional, small memories between the basic memory path and the Mini CPU.
//! ROMs are built from constants and selectors, not a prebuilt memory device.
use crate::{Level, Test};
use makine_sim::{into, out, ChipBuilder, ChipDef, Part, PartKind, Source, TestStep};

pub const SECTION: &str = "Bellek ve ROM";
pub const DATA: [u8; 4] = [0x12, 0xA5, 0x3C, 0x80];
/// LDI 3; ST 0; ADD 0; ST 1; HLT. Remaining addresses are NOP.
pub const PROGRAM: [u8; 16] = [
    0x13, 0x30, 0x40, 0x31, 0xF0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub static LEVELS: &[Level] = &[
    Level {
        chip: "AddressDecode2",
        section: SECTION,
        title: "Adres Çözücü: Bir Hücreyi Seç",
        description: "en=1 iken 2-bit addr'ın seçtiği çıkışı 1, diğerlerini 0 yap. en=0 iken bütün çıkışlar 0 olsun.\n\nAdres 0,1,2,3 sırasıyla s0,s1,s2,s3'ü seçer. Bu devre RAM'in yazma iznini yalnızca bir hücreye gönderecek. Adres verinin kendisi değildir.",
        inputs: &["addr[2]", "en"],
        outputs: &["s0", "s1", "s2", "s3"],
        test: Test::Function(|i| (0..4).map(|a| ((i[0] == a) && i[1] == 1) as u64).collect()),
    },
    Level {
        chip: "Rom4x8",
        section: SECTION,
        title: "ROM Kur: 4 Adres × 8 Bit",
        description: "addr=0,1,2,3 için sırasıyla 0x12, 0xA5, 0x3C, 0x80 ver. q sekiz bitlik veridir.\n\nDört Sabit ve bir MUX ağacıyla kendi ROM'unu kur. Saat ve yazma girişi yok: adres değişince veri değişir. ROM'u programlamak burada sabitleri ve bağlantıları belirlemektir; çalışırken RAM gibi yazılmaz.",
        inputs: &["addr[2]"],
        outputs: &["q[8]"],
        test: Test::Function(|i| vec![DATA[i[0] as usize] as u64]),
    },
    Level {
        chip: "Ram4x8",
        section: SECTION,
        title: "Bayt Belleği: 4 Adres × 8 Bit RAM",
        description: "q her zaman addr'ın seçtiği baytı göstersin. Yalnızca clk'nin yükselen kenarında we=1 ise d o adrese yazılsın; diğer hücreler korunsun.\n\nDört Bit×8 kaydı, Adres Çözücü ve MUX×8 seçicileri kullanabilirsin. ROM'un sabitlerinden farklı olarak RAM çalışırken değişir. Saat yüksek kalırken adres veya veri değiştirmek yeni bir yazma değildir.",
        inputs: &["addr[2]", "d[8]", "we", "clk"],
        outputs: &["q[8]"],
        test: Test::Sequence(ram_steps),
    },
    Level {
        chip: "ProgramRom",
        section: SECTION,
        title: "Programı ROM'a Kodla",
        description: "16 adreslik, 8-bit komut ROM'u kur: 0→0x13, 1→0x30, 2→0x40, 3→0x31, 4→0xF0; 5–15 adresleri 0x00.\n\nBunlar Mini CPU için LDI 3; ST 0; ADD 0; ST 1; HLT programıdır. Üst dört bit opcode, alt dört bit argümandır. Program RAM[0]'a 3, RAM[1]'e 6 yazar. Bu seviyede komutları saklıyorsun; henüz yürütmüyorsun.",
        inputs: &["addr[4]"],
        outputs: &["instr[8]"],
        test: Test::Function(|i| vec![PROGRAM[i[0] as usize] as u64]),
    },
    Level {
        chip: "RomSequencer",
        section: SECTION,
        title: "ROM'dan Komut Getir",
        description: "4-bit pc kaydını Program ROM'una bağla; instr her zaman ROM[pc] olsun. Her yükselen kenarda öncelik: reset→pc=0; load→pc=d; run=1 ve instr≠0xF0 ise pc=pc+1; yoksa koru. 15+1, 0'a sarar.\n\nProgramı adres ve komut baytlarıyla adımla; HLT'de dur, load ile başka adrese git, reset ile başa dön. Bu bir komut getirme devresidir: ADD veya ST yürütmez. Programı gerçekten çalıştırmak için sonraki Mini işlemci yolunda ALU, kontrol ve RAM'i bağlayacaksın.",
        inputs: &["d[4]", "load", "run", "reset", "clk"],
        outputs: &["pc[4]", "instr[8]"],
        test: Test::Sequence(sequencer_steps),
    },
];

fn ram_steps() -> Vec<TestStep> {
    let mut steps = Vec::new();
    let mut cells = [None; 4];
    for k in 0..96u64 {
        let addr = (k % 4) as usize;
        let d = (k * 73 + 0x80) & 255;
        let we = (k < 4 || k % 3 != 0) as u64;
        steps.push(TestStep {
            inputs: vec![addr as u64, d, we, 0],
            expect: vec![cells[addr]],
        });
        if we == 1 {
            cells[addr] = Some(d);
        }
        steps.push(TestStep {
            inputs: vec![addr as u64, d, we, 1],
            expect: vec![cells[addr]],
        });
        // Changing address, data and write enable without another edge cannot write.
        for (a, &q) in cells.iter().enumerate() {
            steps.push(TestStep {
                inputs: vec![a as u64, d ^ 255, 1, 1],
                expect: vec![q],
            });
        }
    }
    steps
}

fn sequencer_steps() -> Vec<TestStep> {
    let mut steps = vec![TestStep {
        inputs: vec![0, 0, 0, 1, 0],
        expect: vec![None, None],
    }];
    let mut pc = 0;
    steps.push(fetch_step(&[0, 0, 0, 1, 1], pc));
    // Walk the program, pause between instructions, stay halted, reset while paused.
    let mut controls = vec![[0, 0, 1, 0]; 12];
    controls.insert(2, [0, 0, 0, 0]);
    controls.extend([[0, 0, 0, 1], [0, 0, 1, 0]]);
    // Visit every address, including NOP space and 15→0; load overrides HLT/run.
    for a in 0..16 {
        controls.extend([[a, 1, 0, 0], [0, 0, 0, 0], [0, 0, 1, 0]]);
    }
    controls.extend([[15, 1, 1, 1], [4, 1, 1, 0], [2, 1, 1, 0]]);
    for [d, load, run, reset] in controls {
        steps.push(fetch_step(&[d, load, run, reset, 0], pc));
        pc = next_pc(pc, d, load, run, reset);
        steps.push(fetch_step(&[d, load, run, reset, 1], pc));
        steps.push(fetch_step(&[15 - d, 1 - load, 1 - run, 1 - reset, 1], pc));
    }
    steps
}

fn fetch_step(inputs: &[u64], pc: u64) -> TestStep {
    TestStep {
        inputs: inputs.to_vec(),
        expect: vec![Some(pc), Some(PROGRAM[pc as usize] as u64)],
    }
}

pub(crate) fn next_pc(pc: u64, d: u64, load: u64, run: u64, reset: u64) -> u64 {
    if reset == 1 {
        0
    } else if load == 1 {
        d
    } else if run == 1 && PROGRAM[pc as usize] != 0xF0 {
        (pc + 1) & 15
    } else {
        pc
    }
}

pub(crate) fn reference(chip: &str) -> Option<ChipDef> {
    let level = LEVELS.iter().find(|l| l.chip == chip)?;
    let mut b = ChipBuilder::new(chip, level.inputs, level.outputs);
    match chip {
        "AddressDecode2" => {
            let addr = b.input("addr");
            let bits = b.split(&[1, 1], addr);
            let lo = out(bits, 0);
            let hi = out(bits, 1);
            let nlo = b.chip1("Not", &[lo]);
            let nhi = b.chip1("Not", &[hi]);
            let en = b.input("en");
            for (name, x, y) in [
                ("s0", nlo, nhi),
                ("s1", lo, nhi),
                ("s2", nlo, hi),
                ("s3", lo, hi),
            ] {
                let selected = b.chip1("And", &[x, y]);
                let enabled = b.chip1("And", &[selected, en]);
                b.output(name, enabled);
            }
        }
        "Rom4x8" | "ProgramRom" => {
            let addr = b.input("addr");
            let words = if chip == "Rom4x8" {
                DATA.as_slice()
            } else {
                PROGRAM.as_slice()
            };
            let q = rom(&mut b, addr, words);
            b.output(if chip == "Rom4x8" { "q" } else { "instr" }, q);
        }
        "Ram4x8" => {
            let (addr, d, we, clk) = (b.input("addr"), b.input("d"), b.input("we"), b.input("clk"));
            let select = b.chip("AddressDecode2", &[addr, we]);
            let mut cells = Vec::new();
            for a in 0..4 {
                let reg = b.chip_n("Bit", 8, &[d, out(select, a), clk]);
                cells.push(out(reg, 0));
            }
            let q = select_words(&mut b, addr, cells);
            b.output("q", q);
        }
        "RomSequencer" => {
            let (d, load, run, reset, clk) = (
                b.input("d"),
                b.input("load"),
                b.input("run"),
                b.input("reset"),
                b.input("clk"),
            );
            let reg = b.part(
                Part {
                    kind: PartKind::Chip("Bit".into()),
                    repeat: 4,
                },
                &[],
            );
            let pc = out(reg, 0);
            let instr = b.chip1("ProgramRom", &[pc]);
            // Detect HLT's upper nibble. ProgramRom emits only 0xF0 for opcode 15.
            let fields = b.split(&[4, 1, 1, 1, 1], instr);
            let h1 = b.chip1("And", &[out(fields, 1), out(fields, 2)]);
            let h2 = b.chip1("And", &[out(fields, 3), out(fields, 4)]);
            let halt = b.chip1("And", &[h1, h2]);
            let not_halt = b.chip1("Not", &[halt]);
            let advance = b.chip1("And", &[run, not_halt]);
            let zero4 = b.constant(4, 0);
            let wide = b.join(&[4, 4], &[pc, zero4]);
            let one8 = b.constant(8, 1);
            let zero = b.constant(1, 0);
            let sum = b.chip("Adder8", &[wide, one8, zero]);
            let halves = b.split(&[4, 4], out(sum, 0));
            let loaded = b.chip_n("Mux", 4, &[out(halves, 0), d, load]);
            let cleared = b.chip_n("Mux", 4, &[out(loaded, 0), zero4, reset]);
            let write = b.chip1("Or", &[load, reset]);
            let write = b.chip1("Or", &[write, advance]);
            b.wire(out(cleared, 0), into(reg, 0));
            b.wire(write, into(reg, 1));
            b.wire(clk, into(reg, 2));
            b.output("pc", pc);
            b.output("instr", instr);
        }
        _ => unreachable!(),
    }
    Some(b.build())
}

fn rom(b: &mut ChipBuilder, addr: Source, words: &[u8]) -> Source {
    let cells = words.iter().map(|&v| b.constant(8, v as u64)).collect();
    select_words(b, addr, cells)
}

fn select_words(b: &mut ChipBuilder, addr: Source, mut cells: Vec<Source>) -> Source {
    let widths = vec![1; cells.len().ilog2() as usize];
    let bits = b.split(&widths, addr);
    for bit in 0..widths.len() {
        cells = cells
            .chunks_exact(2)
            .map(|pair| {
                let mux = b.chip_n("Mux", 8, &[pair[0], pair[1], out(bits, bit)]);
                out(mux, 0)
            })
            .collect();
    }
    cells[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, find, levels};
    use makine_sim::{Circuit, Library, Sink, Verdict};

    fn library(fast: bool) -> Library {
        let defs: Vec<_> = levels()
            .iter()
            .take_while(|l| l.chip != "AddSub8")
            .map(|l| crate::reference::reference(l.chip))
            .collect();
        crate::fast::library(&defs, fast)
    }

    #[test]
    fn side_path_uses_only_basic_chips_and_its_own_earlier_chips() {
        for level in LEVELS {
            assert!(level.optional());
            assert!(level
                .allowed_chips()
                .all(|l| !l.optional() || l.section == SECTION));
            for fast in [false, true] {
                assert_eq!(
                    check(level, &library(fast), &reference(level.chip).unwrap()).unwrap(),
                    Verdict::Pass,
                    "{} fast={fast}",
                    level.chip
                );
            }
        }
        for chip in ["AddSub8", "MiniCpu", "Zero32", "Cpu", "Mux4"] {
            assert!(find(chip)
                .unwrap()
                .allowed_chips()
                .all(|l| l.section != SECTION));
        }
    }

    fn rejected(def: &ChipDef) {
        assert!(
            matches!(
                check(find(&def.name).unwrap(), &library(false), def).unwrap(),
                Verdict::Fail(_)
            ),
            "{} accepted a broken circuit",
            def.name
        );
    }

    #[test]
    fn rejects_wrong_rom_words_and_aliased_addresses() {
        for chip in ["Rom4x8", "ProgramRom"] {
            let mut def = reference(chip).unwrap();
            let value = def
                .parts
                .iter_mut()
                .find_map(|p| match &mut p.kind {
                    PartKind::Const { value, .. } if *value != 0 => Some(value),
                    _ => None,
                })
                .unwrap();
            *value ^= 0x80;
            rejected(&def);
        }
        let mut def = reference("AddressDecode2").unwrap();
        for w in &mut def.wires {
            if w.to == Sink::Output(1) {
                w.to = Sink::Output(2);
            } else if w.to == Sink::Output(2) {
                w.to = Sink::Output(1);
            }
        }
        rejected(&def);
    }

    #[test]
    fn rejects_ram_that_ignores_write_enable_or_writes_all_cells() {
        let mut def = reference("Ram4x8").unwrap();
        let decoder = def
            .parts
            .iter()
            .position(|p| p.kind == PartKind::Chip("AddressDecode2".into()))
            .unwrap();
        let one = def.parts.len();
        def.parts
            .push(Part::new(PartKind::Const { width: 1, value: 1 }));
        def.connect(out(one, 0), into(decoder, 1));
        rejected(&def);
        let mut def = reference("Ram4x8").unwrap();
        for reg in 0..def.parts.len() {
            if def.parts[reg].kind == PartKind::Chip("Bit".into()) {
                def.connect(Source::Input(2), into(reg, 1));
            }
        }
        rejected(&def);
    }

    #[test]
    fn rejects_sequencer_without_pause_reset_load_or_halt() {
        for input in 1..=3 {
            let mut def = reference("RomSequencer").unwrap();
            def.wires.retain(|w| w.from != Source::Input(input));
            rejected(&def);
        }
        let mut def = reference("RomSequencer").unwrap();
        let advance = def
            .parts
            .iter()
            .enumerate()
            .find(|(id, p)| {
                p.kind == PartKind::Chip("And".into())
                    && def
                        .wires
                        .iter()
                        .any(|w| w.to == into(*id, 0) && w.from == Source::Input(2))
            })
            .map(|(id, _)| id)
            .unwrap();
        // Ignore the halt detector by feeding run to both inputs of the advance AND.
        def.connect(Source::Input(2), into(advance, 1));
        rejected(&def);
    }

    #[test]
    fn program_rom_bytes_execute_on_the_gate_built_mini_cpu() {
        use crate::mini::MiniDriver;
        use makine_rv::smallasm::{assemble, Dialect};
        let assembled = assemble("LDI 3\nST 0\nADD 0\nST 1\nHLT\n", Dialect::Mini).unwrap();
        assert_eq!(
            PROGRAM[..5]
                .iter()
                .copied()
                .map(u16::from)
                .collect::<Vec<_>>(),
            assembled.words
        );
        let lib = crate::cost::best_library();
        let mut rom = Circuit::compile_def(lib, &reference("ProgramRom").unwrap()).unwrap();
        let mut bytes = [0u8; 16];
        for (addr, byte) in bytes.iter_mut().enumerate() {
            rom.set_input(0, addr as u64);
            rom.eval().unwrap();
            *byte = rom.output(0) as u8;
        }
        assert_eq!(bytes, PROGRAM);
        let mut cpu = MiniDriver::new(
            Circuit::compile_def(lib, &crate::reference::reference("MiniCpu")).unwrap(),
        );
        let mut ram = [0u8; 16];
        cpu.reset().unwrap();
        for _ in 0..8 {
            cpu.step(&bytes, &mut ram).unwrap();
        }
        assert_eq!((cpu.pc(), cpu.acc()), (4, 6));
        assert_eq!(&ram[..2], &[3, 6]);
        assert!(ram[2..].iter().all(|&v| v == 0));
    }
}
