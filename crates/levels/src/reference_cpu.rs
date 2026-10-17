//! Reference solutions for sections 4 and 5, built from earlier chips only.
//! They prove the levels can be solved and give the CPU test something to
//! run.

use makine_sim::{into, out, ChipBuilder, ChipDef, Part, PartKind, Source};

use crate::find;

/// Build a constant-amount shift as pure wiring: `out = a << k` (left) or
/// `a >> k` filling with `fill` (right).
fn shift_const(b: &mut ChipBuilder, a: Source, k: u8, left: bool, fill: Source) -> Source {
    if k == 0 {
        return a;
    }
    let keep = 32 - k;
    if left {
        // out[k..32] = a[0..32-k], out[0..k] = 0
        let s = b.split(&[keep, k], a);
        let zero = b.constant(k, 0);
        b.join(&[k, keep], &[zero, out(s, 0)])
    } else {
        // out[0..32-k] = a[k..32], out[32-k..] = fill
        let s = b.split(&[k, keep], a);
        let fills: Vec<Source> = vec![fill; k as usize];
        let mut widths = vec![keep];
        widths.extend(std::iter::repeat_n(1u8, k as usize));
        let mut groups = vec![out(s, 1)];
        groups.extend(fills);
        b.join(&widths, &groups)
    }
}

/// Barrel shifter stages over the 5 bits of `n`.
fn barrel(b: &mut ChipBuilder, a: Source, n: Source, left: bool, fill: Source) -> Source {
    let bits = b.split(&[1; 5], n);
    let mut v = a;
    for i in 0..5 {
        let shifted = shift_const(b, v, 1 << i, left, fill);
        v = b.chip1("Mux32", &[v, shifted, out(bits, i)]);
    }
    v
}

/// OR all bits of a bus of `width` (a power of two) down to one.
fn or_tree(b: &mut ChipBuilder, v: Source, width: u8) -> Source {
    if width == 1 {
        return v;
    }
    let h = width / 2;
    let s = b.split(&[h, h], v);
    let o = b.part(
        Part {
            kind: PartKind::Chip("Or".into()),
            repeat: h,
        },
        &[out(s, 0), out(s, 1)],
    );
    or_tree(b, out(o, 0), h)
}

/// 4-way 32-bit mux from 2-bit select (s0 = low bit).
fn mux4(b: &mut ChipBuilder, v: [Source; 4], s0: Source, s1: Source) -> Source {
    let lo = b.chip1("Mux32", &[v[0], v[1], s0]);
    let hi = b.chip1("Mux32", &[v[2], v[3], s0]);
    b.chip1("Mux32", &[lo, hi, s1])
}

/// N-way mux tree over 32-bit values, select bits low first.
fn mux_tree(b: &mut ChipBuilder, v: &[Source], sel: &[Source], width: u8) -> Source {
    if v.len() == 1 {
        return v[0];
    }
    let half = v.len() / 2;
    let name = if width == 1 { "Mux" } else { "Mux32" };
    let next: Vec<Source> = (0..half)
        .map(|i| b.chip1(name, &[v[2 * i], v[2 * i + 1], sel[0]]))
        .collect();
    mux_tree(b, &next, &sel[1..], width)
}

/// Equality of a bus with a constant: AND of (bit == const bit).
fn eq_const(b: &mut ChipBuilder, v: Source, width: u8, k: u64) -> Source {
    let k_bus = b.constant(width, k);
    let x = b.part(
        Part {
            kind: PartKind::Chip("Xor".into()),
            repeat: width,
        },
        &[v, k_bus],
    );
    let any = or_tree_any(b, out(x, 0), width);
    b.chip1("Not", &[any])
}

/// OR tree for any width (not just powers of two).
fn or_tree_any(b: &mut ChipBuilder, v: Source, width: u8) -> Source {
    if width == 1 {
        return v;
    }
    let bits = b.split(&vec![1; width as usize], v);
    let mut acc = out(bits, 0);
    for i in 1..width as usize {
        acc = b.chip1("Or", &[acc, out(bits, i)]);
    }
    acc
}

fn and3(b: &mut ChipBuilder, x: Source, y: Source, z: Source) -> Source {
    let xy = b.chip1("And", &[x, y]);
    b.chip1("And", &[xy, z])
}

fn or_n(b: &mut ChipBuilder, v: &[Source]) -> Source {
    let mut acc = v[0];
    for &x in &v[1..] {
        acc = b.chip1("Or", &[acc, x]);
    }
    acc
}

pub fn reference(chip: &str) -> Option<ChipDef> {
    let level = find(chip)?;
    let mut b = ChipBuilder::new(chip, level.inputs, level.outputs);
    match chip {
        "Ram8" => {
            let (addr, d, we, clk) = (b.input("addr"), b.input("d"), b.input("we"), b.input("clk"));
            let a = b.split(&[1; 3], addr);
            let mut lines = vec![we];
            for bit in (0..3).rev() {
                let mut next = Vec::new();
                for &l in &lines {
                    let dm = b.chip("Demux", &[l, out(a, bit)]);
                    next.push(out(dm, 0));
                    next.push(out(dm, 1));
                }
                lines = next;
            }
            let words: Vec<Source> = lines
                .iter()
                .map(|&l| b.chip1("Register32", &[d, l, clk]))
                .collect();
            let sel: Vec<Source> = (0..3).map(|i| out(a, i)).collect();
            let q = mux_tree(&mut b, &words, &sel, 32);
            b.output("q", q);
        }
        "Zero32" => {
            let v = b.input("in");
            let any = or_tree(&mut b, v, 32);
            let z = b.chip1("Not", &[any]);
            b.output("zero", z);
        }
        "Compare32" => {
            let (x, y) = (b.input("a"), b.input("b"));
            let ny = b.chip_n("Not", 32, &[y]);
            let one = b.constant(1, 1);
            let d = b.chip("Adder32", &[x, out(ny, 0), one]);
            let eq = b.chip1("Zero32", &[out(d, 0)]);
            let ltu = b.chip1("Not", &[out(d, 1)]);
            let xs = b.split(&[31, 1], x);
            let ys = b.split(&[31, 1], y);
            let ds = b.split(&[31, 1], out(d, 0));
            let differ = b.chip1("Xor", &[out(xs, 1), out(ys, 1)]);
            let lt = b.chip1("Mux", &[out(ds, 1), out(xs, 1), differ]);
            b.output("eq", eq);
            b.output("lt", lt);
            b.output("ltu", ltu);
        }
        "ShiftLeft32" => {
            let (a, n) = (b.input("a"), b.input("n"));
            let zero = b.constant(1, 0);
            let v = barrel(&mut b, a, n, true, zero);
            b.output("out", v);
        }
        "ShiftRight32" => {
            let (a, n, arith) = (b.input("a"), b.input("n"), b.input("arith"));
            let sign = b.split(&[31, 1], a);
            let fill = b.chip1("And", &[out(sign, 1), arith]);
            let v = barrel(&mut b, a, n, false, fill);
            b.output("out", v);
        }
        "Alu" => {
            let (x, y, op) = (b.input("a"), b.input("b"), b.input("op"));
            let ops = b.split(&[1, 1, 1, 1], op);
            let (o0, o1, o2, o3) = (out(ops, 0), out(ops, 1), out(ops, 2), out(ops, 3));
            let zero1 = b.constant(1, 0);
            let add = b.chip("Adder32", &[x, y, zero1]);
            let sub = b.chip1("Sub32", &[x, y]);
            let sum = b.chip1("Mux32", &[out(add, 0), sub, o3]);
            let ys = b.split(&[5, 27], y);
            let sll = b.chip1("ShiftLeft32", &[x, out(ys, 0)]);
            let sr = b.chip1("ShiftRight32", &[x, out(ys, 0), o3]);
            let cmp = b.chip("Compare32", &[x, y]);
            let zero31 = b.constant(31, 0);
            let slt = b.join(&[1, 31], &[out(cmp, 1), zero31]);
            let sltu = b.join(&[1, 31], &[out(cmp, 2), zero31]);
            let xor = b.chip_n("Xor", 32, &[x, y]);
            let or = b.chip_n("Or", 32, &[x, y]);
            let and = b.chip_n("And", 32, &[x, y]);
            // funct3: 0 add/sub 1 sll 2 slt 3 sltu 4 xor 5 srl/sra 6 or 7 and
            let lo = mux4(&mut b, [sum, sll, slt, sltu], o0, o1);
            let hi = mux4(&mut b, [out(xor, 0), sr, out(or, 0), out(and, 0)], o0, o1);
            let r = b.chip1("Mux32", &[lo, hi, o2]);
            b.output("out", r);
        }
        "RegFile" => {
            let (rs1, rs2, rd) = (b.input("rs1"), b.input("rs2"), b.input("rd"));
            let (d, we, clk) = (b.input("d"), b.input("we"), b.input("clk"));
            // Decoder: we routed down a Demux tree by rd bits, high bit
            // first, so the final lines come out in order: lines[k] is
            // high when rd = k.
            let rd_bits = b.split(&[1; 5], rd);
            let mut lines = vec![we];
            for bit in (0..5).rev() {
                let mut next = Vec::new();
                for &l in &lines {
                    let dm = b.chip("Demux", &[l, out(rd_bits, bit)]);
                    next.push(out(dm, 0));
                    next.push(out(dm, 1));
                }
                lines = next;
            }
            let load = lines;
            let zero = b.constant(32, 0);
            let mut regs = vec![zero];
            for &l in &load[1..] {
                let r = b.chip1("Register32", &[d, l, clk]);
                regs.push(r);
            }
            let s1 = b.split(&[1; 5], rs1);
            let s2 = b.split(&[1; 5], rs2);
            let sel1: Vec<Source> = (0..5).map(|i| out(s1, i)).collect();
            let sel2: Vec<Source> = (0..5).map(|i| out(s2, i)).collect();
            let a = mux_tree(&mut b, &regs, &sel1, 32);
            let bv = mux_tree(&mut b, &regs, &sel2, 32);
            b.output("a", a);
            b.output("b", bv);
        }
        "Decoder" => {
            let i = b.input("instr");
            // Fields: [0..7 opcode][7..12 rd][12..15 f3][15..20 rs1][20..25 rs2][25..32 f7]
            let f = b.split(&[7, 5, 3, 5, 5, 7], i);
            b.output("opcode", out(f, 0));
            b.output("rd", out(f, 1));
            b.output("funct3", out(f, 2));
            b.output("rs1", out(f, 3));
            b.output("rs2", out(f, 4));
            // Individual instruction bits.
            let bits = b.split(&[1; 32], i);
            let bit = |k: usize| out(bits, k);
            let sign = bit(31);
            let zero = b.constant(1, 0);
            let mk =
                |b: &mut ChipBuilder, parts: Vec<Source>| -> Source { b.join(&[1; 32], &parts) };
            // I: imm[0..12] = instr[20..32], rest sign
            let mut v: Vec<Source> = (20..32).map(bit).collect();
            v.resize(32, sign);
            let imm_i = mk(&mut b, v);
            // S: imm[0..5] = instr[7..12], imm[5..12] = instr[25..32]
            let mut v: Vec<Source> = (7..12).map(bit).chain((25..32).map(bit)).collect();
            v.resize(32, sign);
            let imm_s = mk(&mut b, v);
            // B: imm[0]=0, [1..5]=instr[8..12], [5..11]=instr[25..31], [11]=instr[7], [12..]=sign
            let mut v: Vec<Source> = vec![zero];
            v.extend((8..12).map(bit));
            v.extend((25..31).map(bit));
            v.push(bit(7));
            v.resize(32, sign);
            let imm_b = mk(&mut b, v);
            // U: imm[0..12]=0, [12..32]=instr[12..32]
            let mut v: Vec<Source> = vec![zero; 12];
            v.extend((12..32).map(bit));
            let imm_u = mk(&mut b, v);
            // J: [0]=0, [1..11]=instr[21..31], [11]=instr[20], [12..20]=instr[12..20], [20..]=sign
            let mut v: Vec<Source> = vec![zero];
            v.extend((21..31).map(bit));
            v.push(bit(20));
            v.extend((12..20).map(bit));
            v.resize(32, sign);
            let imm_j = mk(&mut b, v);
            // Select by opcode bits 2..6 (bits 0,1 are always 11):
            // load 00000, op-imm 00100, auipc 00101, store 01000, op 01100,
            // lui 01101, branch 11000, jalr 11001, jal 11011.
            // bit2 set: U (auipc/lui) or J (jal) or I (jalr has bit2=0!).
            // Use: is_s = op[5]&!op[6]&!op[4]; is_b = op[6]&!op[2]; is_j = op[3];
            // is_u = op[2]&op[4]; else I.
            let n = |b: &mut ChipBuilder, s: Source| b.chip1("Not", &[s]);
            let n6 = n(&mut b, bit(6));
            let n4 = n(&mut b, bit(4));
            let n2 = n(&mut b, bit(2));
            let is_s = and3(&mut b, bit(5), n6, n4);
            let is_b = b.chip1("And", &[bit(6), n2]);
            let is_j = bit(3);
            let is_u = b.chip1("And", &[bit(2), bit(4)]);
            let v = b.chip1("Mux32", &[imm_i, imm_s, is_s]);
            let v = b.chip1("Mux32", &[v, imm_b, is_b]);
            let v = b.chip1("Mux32", &[v, imm_u, is_u]);
            let v = b.chip1("Mux32", &[v, imm_j, is_j]);
            b.output("imm", v);
        }
        "Control" => {
            let (op, f3, f7) = (b.input("opcode"), b.input("funct3"), b.input("funct7"));
            let is = |b: &mut ChipBuilder, k: u64| eq_const(b, op, 7, k);
            let lui = is(&mut b, 0x37);
            let auipc = is(&mut b, 0x17);
            let jal = is(&mut b, 0x6F);
            let jalr = is(&mut b, 0x67);
            let branch = is(&mut b, 0x63);
            let load = is(&mut b, 0x03);
            let store = is(&mut b, 0x23);
            let opimm = is(&mut b, 0x13);
            let opreg = is(&mut b, 0x33);
            let f7s = b.split(&[1, 4, 1, 1], f7);
            let mul = b.chip1("And", &[opreg, out(f7s, 0)]);
            let reg_we = or_n(&mut b, &[lui, auipc, jal, jalr, load, opimm, opreg]);
            let b_imm = or_n(&mut b, &[auipc, jalr, load, store, opimm]);
            // alu_op = {alt, f3} for op/op-imm; alt = f7[5] for op, and for
            // op-imm only when f3 = 5 (srai). 0 for loads/stores/jalr/auipc.
            let arith = b.chip1("Or", &[opimm, opreg]);
            let f3_is5 = eq_const(&mut b, f3, 3, 5);
            let imm_alt_ok = b.chip1("Or", &[opreg, f3_is5]);
            let alt = and3(&mut b, out(f7s, 2), arith, imm_alt_ok);
            let zero3 = b.constant(3, 0);
            let f3_or_0 = b.part(
                Part {
                    kind: PartKind::Chip("Mux".into()),
                    repeat: 3,
                },
                &[zero3, f3, arith],
            );
            let alu_op = b.join(&[3, 1], &[out(f3_or_0, 0), alt]);
            // wb_sel: 1 load, 2 jal/jalr, 3 lui
            let link = b.chip1("Or", &[jal, jalr]);
            let wb0 = b.chip1("Or", &[load, lui]);
            let wb1 = b.chip1("Or", &[link, lui]);
            let wb = b.join(&[1, 1], &[wb0, wb1]);
            b.output("reg_we", reg_we);
            b.output("alu_b_imm", b_imm);
            b.output("alu_op", alu_op);
            b.output("wb_sel", wb);
            b.output("mem_read", load);
            b.output("mem_write", store);
            b.output("branch", branch);
            b.output("jal", jal);
            b.output("jalr", jalr);
            b.output("auipc", auipc);
            b.output("mul", mul);
        }
        "BranchUnit" => {
            let (eq, lt, ltu, f3) = (
                b.input("eq"),
                b.input("lt"),
                b.input("ltu"),
                b.input("funct3"),
            );
            let s = b.split(&[1, 1, 1], f3);
            // f3[2]=0: eq; f3[1]=0: lt; f3[1]=1: ltu; f3[0] inverts.
            let l = b.chip1("Mux", &[lt, ltu, out(s, 1)]);
            let c = b.chip1("Mux", &[eq, l, out(s, 2)]);
            let t = b.chip1("Xor", &[c, out(s, 0)]);
            b.output("take", t);
        }
        "LoadUnit" => {
            let (w, off, f3) = (b.input("word"), b.input("off"), b.input("funct3"));
            // shift right by off*8: n = {off, 000}
            let zero3 = b.constant(3, 0);
            let n = b.join(&[3, 2], &[zero3, off]);
            let zero1 = b.constant(1, 0);
            let v = b.chip1("ShiftRight32", &[w, n, zero1]);
            let vs = b.split(&[7, 1, 7, 1, 16], v);
            let f = b.split(&[1, 1, 1], f3);
            let unsigned = out(f, 2);
            let nu = b.chip1("Not", &[unsigned]);
            let s8 = b.chip1("And", &[out(vs, 1), nu]);
            let s16 = b.chip1("And", &[out(vs, 3), nu]);
            let byte = b.join(&[7, 1, 8, 16], &[out(vs, 0), out(vs, 1), s8, s8]);
            let half = b.join(
                &[7, 1, 7, 1, 16],
                &[out(vs, 0), out(vs, 1), out(vs, 2), out(vs, 3), s16],
            );
            let r = b.chip1("Mux32", &[byte, half, out(f, 0)]);
            let r = b.chip1("Mux32", &[r, v, out(f, 1)]);
            b.output("out", r);
        }
        "StoreUnit" => {
            let (d, off, f3) = (b.input("data"), b.input("off"), b.input("funct3"));
            let zero3 = b.constant(3, 0);
            let n = b.join(&[3, 2], &[zero3, off]);
            let wd = b.chip1("ShiftLeft32", &[d, n]);
            b.output("wdata", wd);
            // mask: base 0001 (sb) / 0011 (sh) / 1111 (sw), shifted by off.
            let f = b.split(&[1, 1, 1], f3);
            let one = b.constant(1, 1);
            let half_or_word = b.chip1("Or", &[out(f, 0), out(f, 1)]);
            let base = b.join(&[1, 1, 1, 1], &[one, half_or_word, out(f, 1), out(f, 1)]);
            let zero28 = b.constant(28, 0);
            let base32 = b.join(&[4, 28], &[base, zero28]);
            let zero3b = b.constant(3, 0);
            let off5 = b.join(&[2, 3], &[off, zero3b]);
            let m = b.chip1("ShiftLeft32", &[base32, off5]);
            let ms = b.split(&[4, 28], m);
            b.output("mask", out(ms, 0));
        }
        "MulDiv" => muldiv(&mut b),
        "Cpu" => cpu(&mut b),
        _ => return None,
    }
    Some(b.build())
}

/// Multiply and divide from adders and muxes.
///
/// Multiplication: 32 shift-and-add rows over 64 bits, done as two 32-bit
/// halves with carries. To stay within 64-bit buses each row adds `a << i`
/// (as a 64-bit value split in two words) when `b[i]` is set. Signed
/// variants correct the high word: mulh = mulhu - (a<0 ? b : 0) - (b<0 ? a : 0).
///
/// Division: restoring long division on magnitudes, 32 rows of
/// subtract-and-select; signs fixed afterwards.
fn muldiv(b: &mut ChipBuilder) {
    let (x, y, f3) = (b.input("a"), b.input("b"), b.input("funct3"));
    let zero1 = b.constant(1, 0);
    let zero32 = b.constant(32, 0);
    let fs = b.split(&[1, 1, 1], f3);
    let (f0, f1, f2) = (out(fs, 0), out(fs, 1), out(fs, 2));
    let ys = b.split(&[1; 32], y);

    // --- multiply (unsigned 32x32 -> 64) ---
    let mut lo = zero32;
    let mut hi = zero32;
    for i in 0..32u8 {
        // a << i as (lo_part, hi_part)
        let lo_part = shift_const(b, x, i, true, zero1);
        let hi_part = if i == 0 {
            zero32
        } else {
            shift_const(b, x, 32 - i, false, zero1)
        };
        let add_lo = b.chip("Adder32", &[lo, lo_part, zero1]);
        let add_hi = b.chip("Adder32", &[hi, hi_part, out(add_lo, 1)]);
        lo = b.chip1("Mux32", &[lo, out(add_lo, 0), out(ys, i as usize)]);
        hi = b.chip1("Mux32", &[hi, out(add_hi, 0), out(ys, i as usize)]);
    }
    let mulhu = hi;
    let xsign = b.split(&[31, 1], x);
    let ysign = b.split(&[31, 1], y);
    let (sa, sb) = (out(xsign, 1), out(ysign, 1));
    // mulh: subtract b if a<0, subtract a if b<0
    let corr_a = b.chip1("Mux32", &[zero32, y, sa]);
    let corr_b = b.chip1("Mux32", &[zero32, x, sb]);
    let h1 = b.chip1("Sub32", &[mulhu, corr_a]);
    let mulh = b.chip1("Sub32", &[h1, corr_b]);
    let mulhsu = h1; // only a is signed

    // --- divide (unsigned magnitudes) ---
    let signed_div = b.chip1("Not", &[f0]);
    let neg_a = b.chip1("And", &[sa, signed_div]);
    let neg_b = b.chip1("And", &[sb, signed_div]);
    let na = b.chip1("Sub32", &[zero32, x]);
    let nb = b.chip1("Sub32", &[zero32, y]);
    let ma = b.chip1("Mux32", &[x, na, neg_a]);
    let mb = b.chip1("Mux32", &[y, nb, neg_b]);
    let mas = b.split(&[1; 32], ma);
    let mut rem = zero32;
    let mut qbits: Vec<Source> = vec![zero1; 32];
    for i in (0..32).rev() {
        // rem = (rem << 1) | a[i]; the bit shifted out of rem makes the
        // 33-bit value larger than any divisor.
        let rs = b.split(&[31, 1], rem);
        let shifted = b.join(&[1, 31], &[out(mas, i), out(rs, 0)]);
        let overflow = out(rs, 1);
        let ndiv = b.chip_n("Not", 32, &[mb]);
        let one = b.constant(1, 1);
        let diff = b.chip("Adder32", &[shifted, out(ndiv, 0), one]);
        // carry = 1 means shifted >= mb.
        let ge = b.chip1("Or", &[out(diff, 1), overflow]);
        rem = b.chip1("Mux32", &[shifted, out(diff, 0), ge]);
        qbits[i] = ge;
    }
    let q = b.join(&[1; 32], &qbits);
    // Signs: quotient negative if signs differ, remainder takes a's sign.
    let qneg = b.chip1("Xor", &[neg_a, neg_b]);
    let nq = b.chip1("Sub32", &[zero32, q]);
    let nrem = b.chip1("Sub32", &[zero32, rem]);
    let qs = b.chip1("Mux32", &[q, nq, qneg]);
    let rs = b.chip1("Mux32", &[rem, nrem, neg_a]);
    // Division by zero: quotient all ones, remainder = a. The long division
    // already gives all-ones quotient and remainder = |a|; fix the signs.
    let bzero = b.chip1("Zero32", &[y]);
    let ones = b.constant(32, 0xFFFF_FFFF);
    let qs = b.chip1("Mux32", &[qs, ones, bzero]);
    let rs = b.chip1("Mux32", &[rs, x, bzero]);
    // Overflow -2^31 / -1: the magnitude path gives q = 2^31 (= -2^31) and
    // rem = 0, which is already right.

    // --- select by funct3 ---
    let mul_r = mux4(b, [lo, mulh, mulhsu, mulhu], f0, f1);
    let div_r = mux4(b, [qs, qs, rs, rs], f0, f1);
    let r = b.chip1("Mux32", &[mul_r, div_r, f2]);
    b.output("out", r);
}

fn cpu(b: &mut ChipBuilder) {
    let (instr, rdata, reset, clk) = (
        b.input("instr"),
        b.input("rdata"),
        b.input("reset"),
        b.input("clk"),
    );
    let zero1 = b.constant(1, 0);
    let one1 = b.constant(1, 1);
    let zero32 = b.constant(32, 0);

    // pc register; its input is wired at the end.
    let pc_reg = b.part(Part::new(PartKind::Chip("Register32".into())), &[]);
    let pc = out(pc_reg, 0);

    let dec = b.chip("Decoder", &[instr]);
    let (opcode, rd, f3, rs1, rs2, imm) = (
        out(dec, 0),
        out(dec, 1),
        out(dec, 2),
        out(dec, 3),
        out(dec, 4),
        out(dec, 5),
    );
    let is = b.split(&[25, 7], instr);
    let f7 = out(is, 1);
    let ctl = b.chip("Control", &[opcode, f3, f7]);
    let c = |k| out(ctl, k);
    let (reg_we, b_imm, alu_op, wb_sel) = (c(0), c(1), c(2), c(3));
    let (mem_write, branch, jal, jalr, auipc, mul) = (c(5), c(6), c(7), c(8), c(9), c(10));

    // Register file; d is wired at the end.
    let rf = b.part(Part::new(PartKind::Chip("RegFile".into())), &[rs1, rs2, rd]);
    let (ra, rb) = (out(rf, 0), out(rf, 1));

    let alu_a = b.chip1("Mux32", &[ra, pc, auipc]);
    let alu_b = b.chip1("Mux32", &[rb, imm, b_imm]);
    let alu = b.chip1("Alu", &[alu_a, alu_b, alu_op]);
    let md = b.chip1("MulDiv", &[ra, rb, f3]);
    let result = b.chip1("Mux32", &[alu, md, mul]);

    // Memory.
    let addr_s = b.split(&[2, 30], alu);
    let off = out(addr_s, 0);
    let zero2 = b.constant(2, 0);
    let addr = b.join(&[2, 30], &[zero2, out(addr_s, 1)]);
    let loaded = b.chip1("LoadUnit", &[rdata, off, f3]);
    let st = b.chip("StoreUnit", &[rb, off, f3]);

    // Next pc.
    let four = b.constant(32, 4);
    let pc4 = b.chip("Adder32", &[pc, four, zero1]);
    let pc_imm = b.chip("Adder32", &[pc, imm, zero1]);
    let cmp = b.chip("Compare32", &[ra, rb]);
    let take = b.chip1("BranchUnit", &[out(cmp, 0), out(cmp, 1), out(cmp, 2), f3]);
    let br = b.chip1("And", &[branch, take]);
    let jump_imm = b.chip1("Or", &[br, jal]);
    let next = b.chip1("Mux32", &[out(pc4, 0), out(pc_imm, 0), jump_imm]);
    let tgt = b.split(&[1, 31], alu);
    let jalr_t = b.join(&[1, 31], &[zero1, out(tgt, 1)]);
    let next = b.chip1("Mux32", &[next, jalr_t, jalr]);
    let next = b.chip1("Mux32", &[next, zero32, reset]);

    // Write back.
    let wbs = b.split(&[1, 1], wb_sel);
    let wb = mux4(
        b,
        [result, loaded, out(pc4, 0), imm],
        out(wbs, 0),
        out(wbs, 1),
    );
    let nreset = b.chip1("Not", &[reset]);
    let we = b.chip1("And", &[reg_we, nreset]);
    b.wire(wb, into(rf, 3));
    b.wire(we, into(rf, 4));
    b.wire(clk, into(rf, 5));

    b.wire(next, into(pc_reg, 0));
    b.wire(one1, into(pc_reg, 1));
    b.wire(clk, into(pc_reg, 2));

    let mwe = b.chip1("And", &[mem_write, nreset]);
    b.output("pc", pc);
    b.output("addr", addr);
    b.output("wdata", out(st, 0));
    b.output("wmask", out(st, 1));
    b.output("we", mwe);
}
