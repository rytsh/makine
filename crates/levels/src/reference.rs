//! Reference solutions for every level, built only from the parts each level
//! allows. Used to prove the levels are solvable.

use makine_sim::{into, out, ChipBuilder, ChipDef, Part, PartKind, Source};

use crate::find;

pub fn reference(chip: &str) -> ChipDef {
    let level = find(chip).unwrap_or_else(|| panic!("no level {chip}"));
    let mut b = ChipBuilder::new(chip, level.inputs, level.outputs);
    match chip {
        "Not" => {
            let x = b.input("in");
            let n = b.nand(x, x);
            b.output("out", n);
        }
        "And" => {
            let (x, y) = (b.input("a"), b.input("b"));
            let n = b.nand(x, y);
            let o = b.chip1("Not", &[n]);
            b.output("out", o);
        }
        "Or" => {
            let (x, y) = (b.input("a"), b.input("b"));
            let nx = b.chip1("Not", &[x]);
            let ny = b.chip1("Not", &[y]);
            let o = b.nand(nx, ny);
            b.output("out", o);
        }
        "Xor" => {
            let (x, y) = (b.input("a"), b.input("b"));
            let n = b.nand(x, y);
            let l = b.nand(x, n);
            let r = b.nand(y, n);
            let o = b.nand(l, r);
            b.output("out", o);
        }
        "Mux" => {
            let (x, y, s) = (b.input("a"), b.input("b"), b.input("sel"));
            let ns = b.chip1("Not", &[s]);
            let l = b.chip1("And", &[x, ns]);
            let r = b.chip1("And", &[y, s]);
            let o = b.chip1("Or", &[l, r]);
            b.output("out", o);
        }
        "Demux" => {
            let (i, s) = (b.input("in"), b.input("sel"));
            let ns = b.chip1("Not", &[s]);
            let x = b.chip1("And", &[i, ns]);
            let y = b.chip1("And", &[i, s]);
            b.output("a", x);
            b.output("b", y);
        }
        "Mux4" | "Mux32" => {
            let ins = [b.input("a"), b.input("b"), b.input("sel")];
            let m = b.chip_n("Mux", if chip == "Mux4" { 4 } else { 32 }, &ins);
            b.output("out", out(m, 0));
        }
        "HalfAdder" => {
            let (x, y) = (b.input("a"), b.input("b"));
            let s = b.chip1("Xor", &[x, y]);
            let k = b.chip1("And", &[x, y]);
            b.output("sum", s);
            b.output("carry", k);
        }
        "FullAdder" => {
            let (x, y, c) = (b.input("a"), b.input("b"), b.input("c"));
            let h1 = b.chip("HalfAdder", &[x, y]);
            let h2 = b.chip("HalfAdder", &[out(h1, 0), c]);
            let k = b.chip1("Or", &[out(h1, 1), out(h2, 1)]);
            b.output("sum", out(h2, 0));
            b.output("carry", k);
        }
        "Adder8" => ripple(&mut b, "FullAdder", 8, 1),
        "Adder4" => ripple(&mut b, "FullAdder", 4, 1),
        "Adder32" => ripple(&mut b, "Adder8", 4, 8),
        "Sub32" => {
            let (x, y) = (b.input("a"), b.input("b"));
            let ny = b.chip_n("Not", 32, &[y]);
            let one = b.constant(1, 1);
            let s = b.chip("Adder32", &[x, out(ny, 0), one]);
            b.output("out", out(s, 0));
        }
        "Latch" => {
            // q = NAND(!set, qn), qn = NAND(!reset, q)
            let (s, r) = (b.input("set"), b.input("reset"));
            let ns = b.chip1("Not", &[s]);
            let nr = b.chip1("Not", &[r]);
            let q = b.part(Part::new(PartKind::Nand), &[ns]);
            let qn = b.nand(nr, out(q, 0));
            b.wire(qn, into(q, 1));
            b.output("q", out(q, 0));
        }
        "DLatch" => {
            let (d, en) = (b.input("d"), b.input("en"));
            let nd = b.chip1("Not", &[d]);
            let s = b.chip1("And", &[d, en]);
            let r = b.chip1("And", &[nd, en]);
            let q = b.chip1("Latch", &[s, r]);
            b.output("q", q);
        }
        "DFF" => {
            let (d, clk) = (b.input("d"), b.input("clk"));
            let nclk = b.chip1("Not", &[clk]);
            let master = b.chip1("DLatch", &[d, nclk]);
            let slave = b.chip1("DLatch", &[master, clk]);
            b.output("q", slave);
        }
        "Bit" => {
            let (d, load, clk) = (b.input("d"), b.input("load"), b.input("clk"));
            let ff = b.part(Part::new(PartKind::Chip("DFF".into())), &[]);
            let m = b.chip1("Mux", &[out(ff, 0), d, load]);
            b.wire(m, into(ff, 0));
            b.wire(clk, into(ff, 1));
            b.output("q", out(ff, 0));
        }
        "Register4" | "Register32" => {
            let ins = [b.input("d"), b.input("load"), b.input("clk")];
            let r = b.chip_n("Bit", if chip == "Register4" { 4 } else { 32 }, &ins);
            b.output("q", out(r, 0));
        }
        "Counter32" => {
            let d = b.input("d");
            let (load, inc, reset, clk) = (
                b.input("load"),
                b.input("inc"),
                b.input("reset"),
                b.input("clk"),
            );
            let reg = b.part(Part::new(PartKind::Chip("Register32".into())), &[]);
            let q = out(reg, 0);
            let four = b.constant(32, 4);
            let zero1 = b.constant(1, 0);
            let plus4 = b.chip("Adder32", &[q, four, zero1]);
            let m1 = b.chip1("Mux32", &[q, out(plus4, 0), inc]);
            let m2 = b.chip1("Mux32", &[m1, d, load]);
            let zero = b.constant(32, 0);
            let m3 = b.chip1("Mux32", &[m2, zero, reset]);
            let one = b.constant(1, 1);
            b.wire(m3, into(reg, 0));
            b.wire(one, into(reg, 1));
            b.wire(clk, into(reg, 2));
            b.output("out", q);
        }
        "Counter12" => {
            let (d, load, inc, reset, clk) = (
                b.input("d"),
                b.input("load"),
                b.input("inc"),
                b.input("reset"),
                b.input("clk"),
            );
            let reg = b.part(
                Part {
                    kind: PartKind::Chip("Bit".into()),
                    repeat: 12,
                },
                &[],
            );
            let q = out(reg, 0);
            let high = b.constant(20, 0);
            let wide = b.join(&[12, 20], &[q, high]);
            let one32 = b.constant(32, 1);
            let zero = b.constant(1, 0);
            let sum = b.chip("Adder32", &[wide, one32, zero]);
            let split = b.split(&[12, 20], out(sum, 0));
            let m1 = b.chip_n("Mux", 12, &[q, out(split, 0), inc]);
            let m2 = b.chip_n("Mux", 12, &[out(m1, 0), d, load]);
            let zero12 = b.constant(12, 0);
            let m3 = b.chip_n("Mux", 12, &[out(m2, 0), zero12, reset]);
            let one = b.constant(1, 1);
            b.wire(out(m3, 0), into(reg, 0));
            b.wire(one, into(reg, 1));
            b.wire(clk, into(reg, 2));
            b.output("out", q);
        }
        other => {
            return crate::reference_cpu::reference(other)
                .unwrap_or_else(|| panic!("no reference solution for {other}"))
        }
    }
    b.build()
}

/// Chain `n` adders of `width` bits each: inputs a, b, c; outputs sum, carry.
fn ripple(b: &mut ChipBuilder, adder: &str, n: usize, width: u8) {
    let widths = vec![width; n];
    let (x, y) = (b.input("a"), b.input("b"));
    let xs = b.split(&widths, x);
    let ys = b.split(&widths, y);
    let mut carry: Source = b.input("c");
    let mut sums = Vec::new();
    for i in 0..n {
        let add = b.chip(adder, &[out(xs, i), out(ys, i), carry]);
        sums.push(out(add, 0));
        carry = out(add, 1);
    }
    let sum = b.join(&widths, &sums);
    b.output("sum", sum);
    b.output("carry", carry);
}
