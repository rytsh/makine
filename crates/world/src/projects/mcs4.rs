//! An inspectable, gate-built 4004 arithmetic/control workshop.
//!
//! This is NOT a transistor/timing/pin-accurate 4004. It executes one
//! instruction on each rising clock edge, with parallel ROM reads. The
//! arithmetic/register/branch instruction families and accumulator controls
//! are implemented below. The MCS-4 external memory/I/O instructions (Ex),
//! SRC and DCL are intentionally rejected, not silently emulated.
//!
//! Every decision and arithmetic operation is NAND logic. Only storage
//! (registers and program ROM) uses native primitives. Source for the
//! architectural operations: https://pastraiser.com/cpu/i4004/i4004_opcodes.html

use super::Project;
use crate::{Device, Edit, End, Kind, Module, Placed, Pos, WireStyle, World};
use makine_sim::{out, ChipBuilder, ChipDef, Part, PartKind, Source};
use std::ops::Range;

/// A small program using real 4004 opcodes. R0 counts from 0 to 15; A
/// displays R0+3 (with carry), then loops. No special host-side execution.
pub const DEMO: &[u64] = &[
    0xD0, // LDM 0
    0xB0, // XCH R0
    0xD3, // loop: LDM 3
    0xF1, // CLC
    0x80, // ADD R0
    0x60, // INC R0
    0x40, 0x02, // JUN loop
];

pub const SUPPORTED: &str = "NOP, JCN, FIM, FIN, JIN, JUN, JMS, INC, ISZ, ADD, SUB, LD, XCH, BBL, LDM; CLB–KBP. SRC, Ex bellek/GÇ ve DCL henüz yok; HATA=1 yapıp durur.";

/// Display only: the actual decoder is the NAND circuit below.
pub fn disassemble(op: u8, next: u8) -> String {
    let n = op & 15;
    match op >> 4 {
        0 if n == 0 => "NOP".into(),
        1 => format!("JCN {n:X}, {next:02X}"),
        2 if n & 1 == 0 => format!("FIM P{}, {next:02X}", n / 2),
        3 => format!("{} P{}", if n & 1 == 0 { "FIN" } else { "JIN" }, n / 2),
        4 | 5 => format!(
            "{} {:X}{next:02X}",
            if op >> 4 == 4 { "JUN" } else { "JMS" },
            n
        ),
        6 => format!("INC R{n}"),
        7 => format!("ISZ R{n}, {next:02X}"),
        8 => format!("ADD R{n}"),
        9 => format!("SUB R{n}"),
        10 => format!("LD R{n}"),
        11 => format!("XCH R{n}"),
        12 => format!("BBL {n:X}"),
        13 => format!("LDM {n:X}"),
        15 if n <= 12 => [
            "CLB", "CLC", "IAC", "CMC", "CMA", "RAL", "RAR", "TCC", "DAC", "TCS", "STC", "DAA",
            "KBP",
        ][n as usize]
            .into(),
        _ => format!("{op:02X}: desteklenmeyen komut"),
    }
}

/// Gates written as bit vectors (least significant bit first).
struct Logic {
    b: ChipBuilder,
    zero: Source,
    one: Source,
    ranges: Vec<(&'static str, Range<usize>)>,
    count: usize,
}

impl Logic {
    fn new() -> Self {
        let mut b = ChipBuilder::new(
            "Mcs4Control",
            &[
                "op[8]",
                "next[8]",
                "indirect[8]",
                "a[4]",
                "carry",
                "r[64]",
                "pc[12]",
                "stack[36]",
                "test",
            ],
            &[
                "a_next[4]",
                "c_next",
                "r_next[64]",
                "pc_next[12]",
                "stack_next[36]",
                "lookup[12]",
                "fault",
            ],
        );
        let zero = b.constant(1, 0);
        let one = b.constant(1, 1);
        Self {
            b,
            zero,
            one,
            ranges: Vec::new(),
            count: 2,
        }
    }
    fn bits(&mut self, name: &str, n: usize) -> Vec<Source> {
        let i = self.b.input(name);
        let p = self.b.split(&vec![1; n], i);
        self.count += 1;
        (0..n).map(|i| out(p, i)).collect()
    }
    fn nand(&mut self, a: Source, b: Source) -> Source {
        self.count += 1;
        self.b.nand(a, b)
    }
    fn not(&mut self, a: Source) -> Source {
        self.nand(a, a)
    }
    fn and(&mut self, a: Source, b: Source) -> Source {
        let n = self.nand(a, b);
        self.not(n)
    }
    fn or(&mut self, a: Source, b: Source) -> Source {
        let a = self.not(a);
        let b = self.not(b);
        self.nand(a, b)
    }
    fn xor(&mut self, a: Source, b: Source) -> Source {
        let n = self.nand(a, b);
        let x = self.nand(a, n);
        let y = self.nand(b, n);
        self.nand(x, y)
    }
    fn mux(&mut self, a: Source, b: Source, s: Source) -> Source {
        let ns = self.not(s);
        let a = self.nand(a, ns);
        let b = self.nand(b, s);
        self.nand(a, b)
    }
    fn select(&mut self, a: &[Source], b: &[Source], s: Source) -> Vec<Source> {
        a.iter().zip(b).map(|(&a, &b)| self.mux(a, b, s)).collect()
    }
    fn constant(&self, n: usize, v: u64) -> Vec<Source> {
        (0..n)
            .map(|i| if v >> i & 1 == 1 { self.one } else { self.zero })
            .collect()
    }
    fn eq(&mut self, bits: &[Source], n: u64) -> Source {
        let mut yes = self.one;
        for (i, &b) in bits.iter().enumerate() {
            let b = if n >> i & 1 == 1 { b } else { self.not(b) };
            yes = self.and(yes, b);
        }
        yes
    }
    fn any(&mut self, bits: &[Source]) -> Source {
        bits.iter().fold(self.zero, |x, &y| self.or(x, y))
    }
    fn add(&mut self, a: &[Source], b: &[Source], mut c: Source) -> (Vec<Source>, Source) {
        let mut sum = Vec::new();
        for (&a, &b) in a.iter().zip(b) {
            let x = self.xor(a, b);
            sum.push(self.xor(x, c));
            let ab = self.and(a, b);
            let xc = self.and(x, c);
            c = self.or(ab, xc);
        }
        (sum, c)
    }
    fn pick(&mut self, rows: &[Vec<Source>], selectors: &[Source]) -> Vec<Source> {
        let mut v = vec![self.zero; rows[0].len()];
        for (row, &s) in rows.iter().zip(selectors) {
            for (to, &bit) in v.iter_mut().zip(row) {
                let x = self.and(bit, s);
                *to = self.or(*to, x);
            }
        }
        v
    }
    fn output(&mut self, name: &str, bits: &[Source]) {
        let s = if bits.len() == 1 {
            bits[0]
        } else {
            self.count += 1;
            self.b.join(&vec![1; bits.len()], bits)
        };
        self.b.output(name, s);
    }
    fn mark(&mut self, name: &'static str, start: usize) {
        self.ranges.push((name, start..self.count));
    }
}

/// Build combinational next-state logic. Native code never interprets an
/// opcode: all the decodes below become equality gates and multiplexers.
fn control() -> (ChipDef, Vec<(&'static str, Range<usize>)>) {
    let mut g = Logic::new();
    let op = g.bits("op", 8);
    let next = g.bits("next", 8);
    let indirect = g.bits("indirect", 8);
    let a = g.bits("a", 4);
    let c = g.b.input("carry");
    let regs = g.bits("r", 64);
    let pc = g.bits("pc", 12);
    let stack = g.bits("stack", 36);
    let test = g.b.input("test");
    let setup_end = g.count;
    g.ranges.push(("Uçlar / bus ayırıcılar", 0..setup_end));
    let start = g.count;
    let hi = &op[4..];
    let lo = &op[..4];
    let group: Vec<_> = (0..16).map(|v| g.eq(hi, v)).collect();
    let regsel: Vec<_> = (0..16).map(|v| g.eq(lo, v)).collect();
    let pairsel: Vec<_> = (0..8).map(|v| g.eq(&lo[1..], v)).collect();
    let even = g.not(lo[0]);
    let fim = g.and(group[2], even);
    let fin = g.and(group[3], even);
    let jin = g.and(group[3], lo[0]);
    let f: Vec<_> = (0..14)
        .map(|v| {
            let low = g.eq(lo, v);
            g.and(group[15], low)
        })
        .collect();
    // Unassigned 0x01..0x0f and 0xfe/ff are not NOPs.
    let nop = g.eq(&op, 0);
    let acc_ok = g.any(&f[..13]);
    let supported = g.any(&[
        nop, group[1], fim, group[3], group[4], group[5], group[6], group[7], group[8], group[9],
        group[10], group[11], group[12], group[13], acc_ok,
    ]);
    let fault = g.not(supported);
    g.mark("Komut çözücü", start);

    let start = g.count;
    let rows: Vec<_> = regs.chunks(4).map(|r| r.to_vec()).collect();
    let rn = g.pick(&rows, &regsel);
    // A register pair uses its even register as the high nibble.
    let pairs: Vec<_> = rows
        .chunks(2)
        .map(|r| [r[1].clone(), r[0].clone()].concat())
        .collect();
    let rp = g.pick(&pairs, &pairsel);
    let (rninc, _) = g.add(&rn, &g.constant(4, 1), g.zero);
    g.mark("Yazmaç okuma / seçiciler", start);

    let start = g.count;
    let (sum, carry) = g.add(&a, &rn, c);
    // 4004 SUB: A + ~Rn + ~C. Keep the adder's carry-out;
    // unlike a simple borrow flag, C=1 means no borrow on this operation.
    let neg: Vec<_> = rn.iter().map(|&x| g.not(x)).collect();
    let nc = g.not(c);
    let (sub, subcarry) = g.add(&a, &neg, nc);
    let (inc, incc) = g.add(&a, &g.constant(4, 1), g.zero);
    let (dec, decc) = g.add(&a, &g.constant(4, 15), g.zero);
    let inv: Vec<_> = a.iter().map(|&v| g.not(v)).collect();
    let rol = vec![c, a[0], a[1], a[2]];
    let ror = vec![a[1], a[2], a[3], c];
    let tcc = vec![c, g.zero, g.zero, g.zero];
    let (tcs, _) = g.add(&g.constant(4, 9), &tcc, g.zero);
    let decimal: Vec<_> = (10..16).map(|v| g.eq(&a, v)).collect();
    let gt9 = g.any(&decimal);
    let adjust = g.or(c, gt9);
    let (plus6, plus6c) = g.add(&a, &g.constant(4, 6), g.zero);
    let daa = g.select(&a, &plus6, adjust);
    let daac = g.or(c, plus6c);
    let kbp: Vec<_> = (0..4)
        .map(|bit| {
            let vals: Vec<_> = (0..16)
                .filter(|&v| {
                    let out = match v {
                        0 => 0,
                        1 => 1,
                        2 => 2,
                        4 => 3,
                        8 => 4,
                        _ => 15,
                    };
                    out >> bit & 1 == 1
                })
                .map(|v| g.eq(&a, v))
                .collect();
            g.any(&vals)
        })
        .collect();
    let mut an = a.clone();
    let mut cn = c;
    for (sel, value) in [
        (group[8], sum),
        (group[9], sub),
        (group[10], rn.clone()),
        (group[11], rn.clone()),
        (group[12], lo.to_vec()),
        (group[13], lo.to_vec()),
        (f[0], g.constant(4, 0)),
        (f[2], inc),
        (f[4], inv),
        (f[5], rol),
        (f[6], ror),
        (f[7], tcc),
        (f[8], dec),
        (f[9], tcs),
        (f[11], daa),
        (f[12], kbp),
    ] {
        an = g.select(&an, &value, sel);
    }
    let cmc = g.not(c);
    for (sel, value) in [
        (group[8], carry),
        (group[9], subcarry),
        (f[0], g.zero),
        (f[1], g.zero),
        (f[2], incc),
        (f[3], cmc),
        (f[5], a[3]),
        (f[6], a[0]),
        (f[7], g.zero),
        (f[8], decc),
        (f[9], g.zero),
        (f[10], g.one),
        (f[11], daac),
    ] {
        cn = g.mux(cn, value, sel);
    }
    g.mark("ALU 4-bit / akümülatör", start);

    let start = g.count;
    let inc_or_isz = g.or(group[6], group[7]);
    let mut rnext = regs.clone();
    for reg in 0..16 {
        let update_inc = g.and(inc_or_isz, regsel[reg]);
        let xch = g.and(group[11], regsel[reg]);
        let update_fim = g.and(fim, pairsel[reg / 2]);
        let update_fin = g.and(fin, pairsel[reg / 2]);
        let chunk = &mut rnext[reg * 4..reg * 4 + 4];
        for bit in 0..4 {
            let pos = if reg % 2 == 0 { bit + 4 } else { bit };
            let x = g.mux(chunk[bit], rninc[bit], update_inc);
            let x = g.mux(x, a[bit], xch);
            let x = g.mux(x, next[pos], update_fim);
            chunk[bit] = g.mux(x, indirect[pos], update_fin);
        }
    }
    g.mark("Yazmaç yazma / yükleme", start);

    let start = g.count;
    let two = g.any(&[group[1], fim, group[4], group[5], group[7]]);
    let (pc1, _) = g.add(&pc, &g.constant(12, 1), g.zero);
    let (pc2, _) = g.add(&pc, &g.constant(12, 2), g.zero);
    let mut pn = g.select(&pc1, &pc2, two);
    // JCN/ISZ use the page after both bytes have been fetched. FIN/JIN
    // use PC after the one-byte opcode fetch (not the same at a boundary).
    let near = [next.clone(), pc2[8..].to_vec()].concat();
    let far = [next.clone(), lo.to_vec()].concat();
    let regjump = [rp, pc1[8..].to_vec()].concat();
    let az = g.eq(&a, 0);
    let nt = g.not(test);
    let zc = g.and(lo[2], az);
    let cc = g.and(lo[1], c);
    let tc = g.and(lo[0], nt);
    let cond = g.any(&[zc, cc, tc]);
    let cond = g.xor(cond, lo[3]);
    let jcn = g.and(group[1], cond);
    let zero_inc = g.eq(&rninc, 0);
    let nonzero_inc = g.not(zero_inc);
    let isz = g.and(group[7], nonzero_inc);
    let branch = g.or(jcn, isz);
    pn = g.select(&pn, &near, branch);
    let absolute = g.or(group[4], group[5]);
    pn = g.select(&pn, &far, absolute);
    pn = g.select(&pn, &regjump, jin);
    pn = g.select(&pn, &stack[..12], group[12]);
    pn = g.select(&pn, &pc, fault); // unsupported opcode freezes PC
                                    // Three return addresses; JMS pushes PC+2, BBL pops. This is a
                                    // pedagogical stack (not the silicon's physical rotating registers).
    let push = [pc2, stack[..24].to_vec()].concat();
    let pop = [stack[12..].to_vec(), g.constant(12, 0)].concat();
    let sn = g.select(&stack, &push, group[5]);
    let sn = g.select(&sn, &pop, group[12]);
    let lookup = [pairs[0].clone(), pc1[8..].to_vec()].concat();
    g.mark("Program sayacı / çağrı yığını", start);
    let start = g.count;
    g.output("a_next", &an);
    g.output("c_next", &[cn]);
    g.output("r_next", &rnext);
    g.output("pc_next", &pn);
    g.output("stack_next", &sn);
    g.output("lookup", &lookup);
    g.output("fault", &[fault]);
    g.mark("Sonuç bus'ları", start);
    let ranges = g.ranges;
    (g.b.build(), ranges)
}

/// Materialise every logic part into the world so its gates can be seen
/// and edited. Modules are visual groupings, not native CPU substitutes.
pub fn project(program: &[u64]) -> Project {
    let (def, ranges) = control();
    let mut world = World::new("4004 · aritmetik ve kontrol atölyesi");
    let mut id = 1u64;
    fn put(w: &mut World, id: &mut u64, kind: Kind, x: f32, y: f32) -> End {
        let n = *id;
        *id += 1;
        w.apply(Edit::Part {
            id: n,
            part: Some(Placed {
                kind,
                pos: Pos::new(x, y),
            }),
        });
        End::new(n, 0)
    }
    fn link(w: &mut World, from: End, to: End, name: &str) {
        w.apply(Edit::Wire {
            to,
            from: Some(from),
        });
        if !name.is_empty() {
            w.apply(Edit::Style {
                to,
                style: Some(WireStyle {
                    name: name.into(),
                    points: vec![],
                }),
            });
        }
    }
    let clk = put(&mut world, &mut id, Kind::Clock { period: 1 }, -300.0, 0.0);
    let reset = put(
        &mut world,
        &mut id,
        Kind::Input {
            name: "reset".into(),
            width: 1,
            value: 0,
        },
        -300.0,
        90.0,
    );
    let test = put(
        &mut world,
        &mut id,
        Kind::Input {
            name: "test".into(),
            width: 1,
            value: 1,
        },
        -300.0,
        180.0,
    );
    let one = put(
        &mut world,
        &mut id,
        Kind::Part(Part::new(PartKind::Const { width: 1, value: 1 })),
        -300.0,
        270.0,
    );
    let rom = put(
        &mut world,
        &mut id,
        Kind::Device(Device::ProgramRom {
            words: program.to_vec(),
        }),
        0.0,
        -280.0,
    );
    let mut stores = Vec::new();
    for (i, bits) in [4, 1, 64, 12, 36].into_iter().enumerate() {
        let end = put(
            &mut world,
            &mut id,
            Kind::Device(Device::Register { bits }),
            i as f32 * 175.0,
            300.0,
        );
        for (pin, from) in [(1, one), (2, reset), (3, clk)] {
            link(&mut world, from, End::new(end.part, pin), "");
        }
        stores.push(end);
    }
    let inputs = [
        rom,
        End::new(rom.part, 1),
        End::new(rom.part, 2),
        stores[0],
        stores[1],
        stores[2],
        stores[3],
        stores[4],
        test,
    ];
    let mut ends = Vec::new();
    let mut collapsed = Vec::new();
    for (group, (name, range)) in ranges.iter().enumerate() {
        let origin = Pos::new((group % 4) as f32 * 380.0, (group / 4) as f32 * 260.0);
        let mut members = Vec::new();
        for (j, p) in def.parts[range.clone()].iter().enumerate() {
            let end = put(
                &mut world,
                &mut id,
                Kind::Part(p.clone()),
                origin.x + (j % 12) as f32 * 150.0,
                origin.y + (j / 12) as f32 * 100.0,
            );
            ends.push(end);
            members.push(end.part);
        }
        let module = id;
        id += 1;
        world.apply(Edit::Module {
            id: module,
            module: Some(Module {
                name: (*name).into(),
                members,
            }),
        });
        collapsed.push(module);
    }
    let source = |s: Source| match s {
        Source::Input(i) => inputs[i],
        Source::Part { part, pin } => End::new(ends[part].part, pin),
    };
    let mut outputs = vec![None; def.outputs.len()];
    for w in &def.wires {
        let from = source(w.from);
        match w.to {
            makine_sim::Sink::Part { part, pin } => {
                link(&mut world, from, End::new(ends[part].part, pin), "")
            }
            makine_sim::Sink::Output(i) => outputs[i] = Some(from),
        }
    }
    for (i, name) in ["A", "C", "R0…R15", "PC", "Dönüş adresleri"]
        .into_iter()
        .enumerate()
    {
        link(
            &mut world,
            outputs[i].unwrap(),
            End::new(stores[i].part, 0),
            name,
        );
    }
    link(&mut world, stores[3], End::new(rom.part, 0), "PC[12]");
    link(
        &mut world,
        outputs[5].unwrap(),
        End::new(rom.part, 1),
        "FIN adresi",
    );
    let fault = outputs[6].unwrap();
    let control_module = id;
    id += 1;
    world.apply(Edit::Module {
        id: control_module,
        module: Some(Module {
            name: "Denetim ve veri yolu".into(),
            members: ends.iter().map(|e| e.part).collect(),
        }),
    });
    collapsed.push(control_module);
    let root_members: Vec<_> = ends.iter().chain(&stores).map(|e| e.part).collect();
    let root = id;
    id += 1;
    world.apply(Edit::Module {
        id: root,
        module: Some(Module {
            name: "4004 aritmetik / kontrol".into(),
            members: root_members,
        }),
    });
    collapsed.push(root);
    for (i, (from, name, bits)) in [
        (stores[0], "A", 4),
        (stores[1], "C", 1),
        (stores[3], "PC", 12),
        (rom, "Komut", 8),
        (fault, "HATA", 1),
    ]
    .into_iter()
    .enumerate()
    {
        let port = put(
            &mut world,
            &mut id,
            Kind::Output {
                name: name.into(),
                width: bits,
            },
            520.0,
            40.0 + i as f32 * 95.0,
        );
        link(&mut world, from, port, name);
    }
    Project {
        world,
        chips: vec![],
        watches: vec![stores[0], stores[1], stores[3], rom, fault],
        collapsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_sim::{Circuit, Library};

    // A scalar architectural oracle used only in tests. Production next
    // state is exclusively the gate network above.
    fn eval(op: u64, next: u64, a: u64, c: u64, regs: u64, pc: u64, stack: u64) -> Vec<u64> {
        let (def, _) = control();
        let mut sim = Circuit::compile_def(&Library::new(), &def).unwrap();
        sim.eval_values(&[op, next, 0xA5, a, c, regs, pc, stack, 1])
            .unwrap()
    }

    #[test]
    fn arithmetic_and_registers() {
        for a in 0..16 {
            for r in 0..16 {
                for c in 0..2 {
                    let out = eval(0x80, 0, a, c, r, 0, 0);
                    assert_eq!((out[0], out[1]), ((a + r + c) & 15, (a + r + c) >> 4));
                    let out = eval(0x90, 0, a, c, r, 0, 0);
                    let value = a as i64 - r as i64 - c as i64;
                    assert_eq!((out[0], out[1]), ((value & 15) as u64, (value >= 0) as u64));
                }
            }
        }
        assert_eq!(eval(0xD9, 0, 2, 1, 0, 0, 0)[0], 9);
        let e = eval(0xB0, 0, 3, 0, 9, 0, 0);
        assert_eq!(e[0], 9);
        assert_eq!(e[2] & 15, 3);
        assert_eq!(eval(0x60, 0, 0, 0, 15, 0, 0)[2] & 15, 0);
        assert_eq!(eval(0x22, 0xAB, 0, 0, 0, 0, 0)[2] >> 8 & 255, 0xBA);
    }

    #[test]
    fn branches_calls_faults_and_page_boundaries() {
        assert_eq!(eval(0x4A, 0xBC, 0, 0, 0, 0, 0)[3], 0xABC);
        let e = eval(0x52, 0x10, 0, 0, 0, 0x123, 0x456);
        assert_eq!(e[3], 0x210);
        assert_eq!(e[4] & 4095, 0x125);
        let e = eval(0xC7, 0, 0, 0, 0, 0, e[4]);
        assert_eq!(e[3], 0x125);
        assert_eq!(e[0], 7);
        assert_eq!(eval(0x14, 0x45, 0, 0, 0, 0xFF, 0)[3], 0x145);
        assert_eq!(eval(0x14, 0x45, 0, 0, 0, 0xFE, 0)[3], 0x145);
        assert_eq!(eval(0x1C, 0x45, 0, 0, 0, 0xFF, 0)[3], 0x101);
        assert_eq!(eval(0x70, 0x34, 0, 0, 14, 0x100, 0)[3], 0x134);
        assert_eq!(eval(0x70, 0x34, 0, 0, 15, 0x100, 0)[3], 0x102);
        for op in [0x21, 0xE0, 0xFD, 0xFE, 0xFF, 0x01] {
            let e = eval(op, 0, 0, 0, 0, 0x123, 0);
            assert_eq!(e[6], 1, "{op:X}");
            assert_eq!(e[3], 0x123);
        }
    }

    #[test]
    fn accumulator_controls_and_indirect_read() {
        for a in 0..16 {
            for c in 0..2 {
                let mut expect = vec![
                    (0, 0),
                    (a, 0),
                    ((a + 1) & 15, (a + 1) >> 4),
                    (a, c ^ 1),
                    (a ^ 15, c),
                    (((a << 1) | c) & 15, a >> 3),
                    ((a >> 1) | (c << 3), a & 1),
                    (c, 0),
                    ((a + 15) & 15, (a + 15) >> 4),
                    (9 + c, 0),
                    (a, 1),
                ];
                let adjusted = if c == 1 || a > 9 { a + 6 } else { a };
                expect.push((adjusted & 15, c | (adjusted >> 4)));
                expect.push((
                    match a {
                        0 => 0,
                        1 => 1,
                        2 => 2,
                        4 => 3,
                        8 => 4,
                        _ => 15,
                    },
                    c,
                ));
                for (op, (av, cv)) in expect.into_iter().enumerate() {
                    let e = eval(0xF0 + op as u64, 0, a, c, 0, 0, 0);
                    assert_eq!((e[0], e[1]), (av, cv), "F{op:X}, A={a}, C={c}");
                }
            }
        }
        // FIN P2 reads ROM-indirect into R4:R5, preserving the other pairs.
        let e = eval(0x34, 0, 0, 0, 0, 0xFF, 0);
        assert_eq!((e[2] >> 16) & 255, 0x5A);
        assert_eq!(e[3], 0x100);
        assert_eq!(e[5], 0x100);
        // JIN P1 takes the pair R2:R3 in the opcode-fetch page.
        let e = eval(0x33, 0, 0, 0, 0xBA00, 0xFF, 0);
        assert_eq!(e[3], 0x1AB);
    }

    #[test]
    fn project_is_a_real_wired_circuit() {
        assert_eq!(
            crate::device::parse_words(include_str!("../../../../examples/4004/count.hex"))
                .unwrap(),
            DEMO
        );
        let p = project(DEMO);
        assert!(p.world.len() > 1000);
        let c = crate::compile(&p.world, &Library::new());
        assert!(c.problems.is_empty(), "{:?}", c.problems);
        let mut sim = Circuit::compile_def(&c.library_with(&Library::new()), &c.def).unwrap();
        let clock_index = c
            .inputs
            .iter()
            .position(|(_, s)| matches!(s, crate::InputSource::Clock { .. }))
            .unwrap();
        for _ in 0..5 {
            sim.set_input(clock_index, 0);
            sim.eval().unwrap();
            sim.set_input(clock_index, 1);
            sim.eval().unwrap();
        }
        let a = sim.output(c.probe(p.watches[0]).unwrap());
        assert_eq!(a, 3, "ROM LDM/XCH/CLC/ADD executed through gates");
        let json = p.world.to_json();
        let restored = World::from_json(&json).unwrap();
        assert_eq!(restored.modules().count(), p.collapsed.len());
    }
}
