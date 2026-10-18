//! An interlude between memory and the RISC-V path: a tiny 8-bit
//! accumulator processor the player builds from the chips they have, and
//! sees run programs. It is optional: later levels do not need it.
//!
//! The machine: an 8-bit accumulator `acc`, a 4-bit `pc`, 16 bytes of
//! program (ROM) and 16 bytes of data (RAM) outside the chip. Each
//! instruction is one byte, `op` in the high nibble and `arg` in the low:
//!
//! | op | name | does                          |
//! |----|------|-------------------------------|
//! | 0  | NOP  | nothing                       |
//! | 1  | LDI n| acc = n                       |
//! | 2  | LD a | acc = ram[a]                  |
//! | 3  | ST a | ram[a] = acc                  |
//! | 4  | ADD a| acc = acc + ram[a]            |
//! | 5  | SUB a| acc = acc - ram[a]            |
//! | 6  | JMP a| pc = a                        |
//! | 7  | JZ a | if acc = 0: pc = a            |
//! | 15 | HLT  | pc stays                      |
//!
//! Other ops do nothing. Otherwise pc = pc + 1 (wrapping at 16).

use makine_lang::{tr, trf};
use makine_sim::{out, ChipBuilder, ChipDef, Circuit, Part, PartKind, Source, Verdict};

use crate::{find, Level, Test};

pub const SECTION: &str = "★ Mini işlemci";

pub static LEVELS: &[Level] = &[
    Level {
        chip: "AddSub8",
        section: SECTION,
        title: "Topla / Çıkar",
        description: "sub = 0 ise a + b, sub = 1 ise a - b ver (8 bit, taşan kısım atılır).\n\n\
                      Çıkarma için ayrı bir devre gerekmez: a - b = a + (DEĞİL b) + 1. \
                      b'yi sub ile XOR'larsan sub = 1 iken ters çevrilir; +1'i de toplayıcının \
                      elde girişi verir.",
        inputs: &["a[8]", "b[8]", "sub"],
        outputs: &["out[8]"],
        test: Test::Function(|i| {
            let (a, b) = (i[0] as u8, i[1] as u8);
            vec![if i[2] == 1 {
                a.wrapping_sub(b)
            } else {
                a.wrapping_add(b)
            } as u64]
        }),
    },
    Level {
        chip: "MiniControl",
        section: SECTION,
        title: "Mini Kontrol",
        description: "Mini işlemcinin şefi: komutun op'una (ve acc'nin 0 olup olmadığına) bakıp \
                      kime ne yapacağını söyle.\n\n\
                      op 1 LDI: load, imm. op 2 LD: load. op 3 ST: we. op 4 ADD: load, alu. \
                      op 5 SUB: load, alu, sub. op 6 JMP: jump. op 7 JZ: zero = 1 ise jump. \
                      op 15 HLT: hold. Diğerleri: hepsi 0.",
        inputs: &["op[4]", "zero"],
        outputs: &["load", "imm", "alu", "sub", "we", "jump", "hold"],
        test: Test::Function(control),
    },
    Level {
        chip: "MiniCpu",
        section: SECTION,
        title: "Mini İşlemci",
        description: "Hepsini birleştir: 8-bit akümülatörlü küçük bir işlemci. Her saat \
                      vuruşunda bir komut.\n\n\
                      Komut bir bayt: üst 4 bit op, alt 4 bit arg. addr hep arg'dır; rdata \
                      RAM'de o adreste duran bayttır. Yükselen kenarda acc ve pc güncellenir; \
                      we = 1 ise RAM'e acc yazılır. reset = 1 iken kenarda pc ve acc 0 olur.\n\n\
                      Test, işlemcinde üç program çalıştırır (geri sayım, çarpma, Fibonacci) ve \
                      her adımı bir referansla karşılaştırır.",
        inputs: &["instr[8]", "rdata[8]", "reset", "clk"],
        outputs: &["pc[4]", "addr[4]", "acc[8]", "we"],
        test: Test::Custom(cpu_test),
    },
];

pub const NOP: u8 = 0;
pub const LDI: u8 = 1;
pub const LD: u8 = 2;
pub const ST: u8 = 3;
pub const ADD: u8 = 4;
pub const SUB: u8 = 5;
pub const JMP: u8 = 6;
pub const JZ: u8 = 7;
pub const HLT: u8 = 15;

/// An instruction byte.
pub const fn ins(op: u8, arg: u8) -> u8 {
    op << 4 | (arg & 15)
}

/// An instruction as text, e.g. `ADD 3`.
pub fn disasm(i: u8) -> String {
    let (op, a) = (i >> 4, i & 15);
    match op {
        LDI => format!("LDI {a}"),
        LD => format!("LD {a}"),
        ST => format!("ST {a}"),
        ADD => format!("ADD {a}"),
        SUB => format!("SUB {a}"),
        JMP => format!("JMP {a}"),
        JZ => format!("JZ {a}"),
        HLT => "HLT".into(),
        _ => "NOP".into(),
    }
}

/// The control signals for `op` (MiniControl's table).
fn control(i: &[u64]) -> Vec<u64> {
    let (op, zero) = (i[0] as u8, i[1]);
    let b = |x: bool| x as u64;
    vec![
        b(matches!(op, LDI | LD | ADD | SUB)),
        b(op == LDI),
        b(matches!(op, ADD | SUB)),
        b(op == SUB),
        b(op == ST),
        b(op == JMP || (op == JZ && zero == 1)),
        b(op == HLT),
    ]
}

/// The reference machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mini {
    pub pc: u8,
    pub acc: u8,
}

impl Mini {
    /// Run one instruction. Returns the RAM write, if any: (address, byte).
    pub fn step(&mut self, rom: &[u8; 16], ram: &mut [u8; 16]) -> Option<(u8, u8)> {
        let i = rom[self.pc as usize & 15];
        let (op, a) = (i >> 4, i & 15);
        let mut next = (self.pc + 1) & 15;
        let mut write = None;
        match op {
            LDI => self.acc = a,
            LD => self.acc = ram[a as usize],
            ST => {
                ram[a as usize] = self.acc;
                write = Some((a, self.acc));
            }
            ADD => self.acc = self.acc.wrapping_add(ram[a as usize]),
            SUB => self.acc = self.acc.wrapping_sub(ram[a as usize]),
            JMP => next = a,
            JZ if self.acc == 0 => next = a,
            HLT => next = self.pc,
            _ => {}
        }
        self.pc = next;
        write
    }
}

/// A program with its starting data, and how many cycles the test runs.
pub struct Program {
    pub name: &'static str,
    pub rom: [u8; 16],
    pub ram: [u8; 16],
    pub cycles: usize,
}

fn rom(code: &[u8]) -> [u8; 16] {
    let mut r = [0; 16];
    r[..code.len()].copy_from_slice(code);
    r
}

/// The programs the test runs, also shown to the player.
pub fn programs() -> Vec<Program> {
    let mut count_ram = [0; 16];
    count_ram[15] = 1;
    let mut mul_ram = [0; 16];
    (mul_ram[0], mul_ram[1], mul_ram[3]) = (13, 7, 1);
    let mut fib_ram = [0; 16];
    fib_ram[1] = 1;
    vec![
        Program {
            name: "geri sayım",
            // acc = 5; loop: ram[14] = acc; acc -= 1; if 0 stop; again.
            rom: rom(&[
                ins(LDI, 5),
                ins(ST, 14),
                ins(SUB, 15),
                ins(JZ, 5),
                ins(JMP, 1),
                ins(ST, 14),
                ins(HLT, 0),
            ]),
            ram: count_ram,
            cycles: 40,
        },
        Program {
            name: "çarpma",
            // ram[2] = ram[0] * ram[1], by adding ram[0] ram[1] times.
            rom: rom(&[
                ins(LD, 2),
                ins(ADD, 0),
                ins(ST, 2),
                ins(LD, 1),
                ins(SUB, 3),
                ins(ST, 1),
                ins(JZ, 8),
                ins(JMP, 0),
                ins(LD, 2),
                ins(HLT, 0),
                ins(9, 9),
            ]),
            ram: mul_ram,
            cycles: 80,
        },
        Program {
            name: "Fibonacci",
            // Forever: ram[2] = ram[0] + ram[1]; shift down. Wraps at 256.
            rom: rom(&[
                ins(LD, 0),
                ins(ADD, 1),
                ins(ST, 2),
                ins(LD, 1),
                ins(ST, 0),
                ins(LD, 2),
                ins(ST, 1),
                ins(JMP, 0),
            ]),
            ram: fib_ram,
            cycles: 120,
        },
    ]
}

fn program_name(name: &str) -> &str {
    match name {
        "geri sayım" => tr!("geri sayım", "countdown"),
        "çarpma" => tr!("çarpma", "multiply"),
        other => other,
    }
}

const IN_INSTR: usize = 0;
const IN_RDATA: usize = 1;
const IN_RESET: usize = 2;
const IN_CLK: usize = 3;
const OUT_PC: usize = 0;
const OUT_ADDR: usize = 1;
const OUT_ACC: usize = 2;
const OUT_WE: usize = 3;

/// Drives a compiled MiniCpu against a ROM and RAM.
pub struct MiniDriver {
    pub circuit: Circuit,
}

/// What one cycle did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiniCycle {
    pub pc: u8,
    pub instr: u8,
    pub write: Option<(u8, u8)>,
}

impl MiniDriver {
    pub fn new(mut circuit: Circuit) -> Self {
        circuit.set_native_lanes(1);
        Self { circuit }
    }

    pub fn reset(&mut self) -> Result<(), String> {
        let c = &mut self.circuit;
        c.reset();
        c.set_input(IN_INSTR, 0);
        c.set_input(IN_RDATA, 0);
        c.set_input(IN_RESET, 1);
        for clk in [0, 1, 0] {
            c.set_input(IN_CLK, clk);
            c.eval().map_err(|e| e.to_string())?;
        }
        c.set_input(IN_RESET, 0);
        Ok(())
    }

    pub fn pc(&self) -> u8 {
        self.circuit.output(OUT_PC) as u8
    }

    pub fn acc(&self) -> u8 {
        self.circuit.output(OUT_ACC) as u8
    }

    pub fn step(&mut self, rom: &[u8; 16], ram: &mut [u8; 16]) -> Result<MiniCycle, String> {
        let c = &mut self.circuit;
        let osc = |e: makine_sim::Oscillation| e.to_string();
        c.set_input(IN_CLK, 0);
        c.eval().map_err(osc)?;
        let pc = c.output(OUT_PC) as u8 & 15;
        let instr = rom[pc as usize];
        c.set_input(IN_INSTR, instr as u64);
        c.eval().map_err(osc)?;
        let addr = c.output(OUT_ADDR) as u8 & 15;
        c.set_input(IN_RDATA, ram[addr as usize] as u64);
        c.eval().map_err(osc)?;
        let write = (c.output(OUT_WE) & 1 == 1).then(|| (addr, c.output(OUT_ACC) as u8));
        c.set_input(IN_CLK, 1);
        c.eval().map_err(osc)?;
        if let Some((a, v)) = write {
            ram[a as usize] = v;
        }
        Ok(MiniCycle { pc, instr, write })
    }
}

/// Run `p` on the circuit and the reference side by side.
pub fn run_lockstep(cpu: &mut MiniDriver, p: &Program) -> Result<(), String> {
    cpu.reset()?;
    let (mut ram, mut ref_ram) = (p.ram, p.ram);
    let mut reference = Mini::default();
    if cpu.pc() != 0 || cpu.acc() != 0 {
        return Err(tr!(
            "reset sonrası pc ve acc 0 olmalı.",
            "after reset, pc and acc must be 0."
        )
        .into());
    }
    for n in 0..p.cycles {
        let before = reference;
        let instr = p.rom[before.pc as usize];
        let want = reference.step(&p.rom, &mut ref_ram);
        let got = cpu.step(&p.rom, &mut ram)?;
        let what = trf!(
            "{}. adım, pc = {}: {}",
            "step {}, pc = {}: {}",
            n + 1,
            before.pc,
            disasm(instr)
        );
        if got.write != want {
            return Err(match want {
                Some((a, v)) => trf!(
                    "{what}: RAM[{a}]'e {v} yazılmalıydı, {}.",
                    "{what}: {v} should have been written to RAM[{a}], {}.",
                    match got.write {
                        Some((ga, gv)) => trf!(
                            "senin işlemcin RAM[{ga}]'e {gv} yazdı",
                            "your processor wrote {gv} to RAM[{ga}]"
                        ),
                        None => tr!("hiçbir şey yazılmadı", "nothing was written").into(),
                    }
                ),
                None => trf!(
                    "{what}: bir şey yazılmamalıydı, ama we = 1.",
                    "{what}: nothing should have been written, but we = 1."
                ),
            });
        }
        if cpu.acc() != reference.acc {
            return Err(trf!(
                "{what}: sonra acc = {} olmalıydı, senin işlemcinde {}.",
                "{what}: afterwards acc should be {}, in your processor it is {}.",
                reference.acc,
                cpu.acc()
            ));
        }
        if cpu.pc() != reference.pc {
            return Err(trf!(
                "{what}: sonra pc = {} olmalıydı, senin işlemcinde {}.",
                "{what}: afterwards pc should be {}, in your processor it is {}.",
                reference.pc,
                cpu.pc()
            ));
        }
    }
    Ok(())
}

fn cpu_test(circuit: &mut Circuit) -> Verdict {
    let mut cpu = MiniDriver::new(circuit.clone());
    for p in programs() {
        if let Err(message) = run_lockstep(&mut cpu, &p) {
            return Verdict::Wrong {
                step: 0,
                message: trf!(
                    "'{}' programı: {message}",
                    "'{}' program: {message}",
                    program_name(p.name)
                ),
            };
        }
    }
    Verdict::Pass
}

/// Reference solutions, from the chips before this section.
pub fn reference(chip: &str) -> Option<ChipDef> {
    let level = find(chip)?;
    let mut b = ChipBuilder::new(chip, level.inputs, level.outputs);
    match chip {
        "AddSub8" => {
            let (x, y, s) = (b.input("a"), b.input("b"), b.input("sub"));
            let flip = b.chip_n("Xor", 8, &[y, s]);
            let sum = b.chip("Adder8", &[x, out(flip, 0), s]);
            b.output("out", out(sum, 0));
        }
        "MiniControl" => {
            let (op, zero) = (b.input("op"), b.input("zero"));
            let bits = b.split(&[1, 1, 1, 1], op);
            let on: Vec<Source> = (0..4).map(|k| out(bits, k)).collect();
            let off: Vec<Source> = on.iter().map(|&s| b.chip1("Not", &[s])).collect();
            let mut is = |k: u8| {
                let pick = |n: usize| if k >> n & 1 == 1 { on[n] } else { off[n] };
                let lo = b.chip1("And", &[pick(0), pick(1)]);
                let hi = b.chip1("And", &[pick(2), pick(3)]);
                b.chip1("And", &[lo, hi])
            };
            let (ldi, ld, st, add, sub, jmp, jz, hlt) = (
                is(LDI),
                is(LD),
                is(ST),
                is(ADD),
                is(SUB),
                is(JMP),
                is(JZ),
                is(HLT),
            );
            let alu = b.chip1("Or", &[add, sub]);
            let get = b.chip1("Or", &[ldi, ld]);
            let load = b.chip1("Or", &[get, alu]);
            let jz_taken = b.chip1("And", &[jz, zero]);
            let jump = b.chip1("Or", &[jmp, jz_taken]);
            b.output("load", load);
            b.output("imm", ldi);
            b.output("alu", alu);
            b.output("sub", sub);
            b.output("we", st);
            b.output("jump", jump);
            b.output("hold", hlt);
        }
        "MiniCpu" => {
            let (instr, rdata) = (b.input("instr"), b.input("rdata"));
            let (reset, clk) = (b.input("reset"), b.input("clk"));
            let f = b.split(&[4, 4], instr);
            let (arg, op) = (out(f, 0), out(f, 1));
            let reg = |b: &mut ChipBuilder, n: u8| {
                b.part(
                    Part {
                        kind: PartKind::Chip("Bit".into()),
                        repeat: n,
                    },
                    &[],
                )
            };
            let accr = reg(&mut b, 8);
            let pcr = reg(&mut b, 4);
            let (acc, pc) = (out(accr, 0), out(pcr, 0));
            // zero = no bit of acc is 1.
            let bits = b.split(&[1; 8], acc);
            let mut any = out(bits, 0);
            for k in 1..8 {
                any = b.chip1("Or", &[any, out(bits, k)]);
            }
            let zero = b.chip1("Not", &[any]);
            let ctl = b.chip("MiniControl", &[op, zero]);
            let sig = |k: usize| out(ctl, k);
            // acc: memory, the adder, or the immediate; 0 on reset.
            let alu = b.chip1("AddSub8", &[acc, rdata, sig(3)]);
            let zero4 = b.constant(4, 0);
            let imm = b.join(&[4, 4], &[arg, zero4]);
            let v1 = b.chip_n("Mux", 8, &[rdata, alu, sig(2)]);
            let v2 = b.chip_n("Mux", 8, &[out(v1, 0), imm, sig(1)]);
            let zero8 = b.constant(8, 0);
            let accd = b.chip_n("Mux", 8, &[out(v2, 0), zero8, reset]);
            let accload = b.chip1("Or", &[sig(0), reset]);
            b.wire(out(accd, 0), makine_sim::into(accr, 0));
            b.wire(accload, makine_sim::into(accr, 1));
            b.wire(clk, makine_sim::into(accr, 2));
            // pc: + 1, or arg on a jump; held on HLT; 0 on reset.
            let zero4b = b.constant(4, 0);
            let pc8 = b.join(&[4, 4], &[pc, zero4b]);
            let one = b.constant(8, 1);
            let no_carry = b.constant(1, 0);
            let inc = b.chip("Adder8", &[pc8, one, no_carry]);
            let low = b.split(&[4, 4], out(inc, 0));
            let pn = b.chip_n("Mux", 4, &[out(low, 0), arg, sig(5)]);
            let zero4c = b.constant(4, 0);
            let pcd = b.chip_n("Mux", 4, &[out(pn, 0), zero4c, reset]);
            let run = b.chip1("Not", &[sig(6)]);
            let pcload = b.chip1("Or", &[run, reset]);
            b.wire(out(pcd, 0), makine_sim::into(pcr, 0));
            b.wire(pcload, makine_sim::into(pcr, 1));
            b.wire(clk, makine_sim::into(pcr, 2));
            b.output("pc", pc);
            b.output("addr", arg);
            b.output("acc", acc);
            b.output("we", sig(4));
        }
        _ => return None,
    }
    Some(b.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_programs_do_what_they_say() {
        let ps = programs();
        let run = |p: &Program| {
            let (mut m, mut ram) = (Mini::default(), p.ram);
            for _ in 0..p.cycles {
                m.step(&p.rom, &mut ram);
            }
            (m, ram)
        };
        let (m, ram) = run(&ps[0]);
        assert_eq!((m.acc, ram[14], m.pc), (0, 0, 6));
        let (m, ram) = run(&ps[1]);
        assert_eq!((m.acc, ram[2], m.pc), (91, 91, 9));
        let (_, ram) = run(&ps[2]);
        assert!(ram[0] != 0 && ram[1] != 0);
    }

    #[test]
    fn a_wrong_cpu_is_explained() {
        // The reference CPU with SUB wired as ADD: the countdown goes up.
        let lib = crate::cost::best_library();
        let mut def = reference("MiniCpu").unwrap();
        for p in &mut def.parts {
            if p.kind == PartKind::Chip("AddSub8".into()) {
                p.kind = PartKind::Chip("Adder8".into());
            }
        }
        // Adder8 has a carry input where AddSub8 has sub: same pins.
        let c = Circuit::compile_def(lib, &def).unwrap();
        let mut cpu = MiniDriver::new(c);
        let e = run_lockstep(&mut cpu, &programs()[0]).unwrap_err();
        assert!(e.contains("SUB 15"), "{e}");
    }
}
