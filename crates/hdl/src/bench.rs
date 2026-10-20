//! Testbenches: a Verilog module that drives the exported chip with test
//! vectors and compares its outputs with what makine expects.
//!
//! The vectors come from a level's own test ([`makine_levels`]-style
//! [`TestStep`]s) or, for chips without a test, from simulating the chip in
//! makine ([`golden_steps`]). Either way the Verilog must behave exactly like
//! the circuit in makine.

use std::fmt::Write;

use makine_lang::{tr, trf};
use makine_sim::{
    mask, ChipDef, Circuit, Library, Part, PartKind, Pin, Rng, Sink, Source, TestStep, Wire,
};

use crate::verilog::{gate_library, Export};
use crate::Options;

/// Whether an input is a clock: it changes after the other inputs, so a
/// rising edge sees the data from before it.
pub(crate) fn is_clock(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "clk" | "clock" | "saat")
}

/// Whether an input is a reset: held high for the first cycle.
pub(crate) fn is_reset(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "reset" | "rst" | "sifirla"
    )
}

/// `def` compiled the way its Verilog behaves: sub-chips by their gates,
/// storage chips by models of their latches and flip-flops.
pub fn compile_like_verilog(
    lib: &Library,
    def: &ChipDef,
    opts: &Options,
) -> Result<Circuit, String> {
    let mut glib = gate_library(lib, opts);
    glib.insert(def.clone());
    if opts.primitives {
        if let Some(p) = crate::verilog::primitive(def) {
            glib.insert_native(def.name.clone(), crate::verilog::primitive_model(p));
            glib.set_fast(&def.name, true);
        }
    }
    // A wrapper with the chip as its only part, so a storage chip at the top
    // is simulated like its Verilog too.
    let mut wrap = ChipDef {
        name: format!("{}__wrap", def.name),
        inputs: def.inputs.clone(),
        outputs: def.outputs.clone(),
        parts: vec![Part::new(PartKind::Chip(def.name.clone()))],
        wires: Vec::new(),
    };
    for i in 0..def.inputs.len() {
        wrap.wires.push(Wire {
            from: Source::Input(i),
            to: Sink::Part { part: 0, pin: i },
        });
    }
    for o in 0..def.outputs.len() {
        wrap.wires.push(Wire {
            from: Source::Part { part: 0, pin: o },
            to: Sink::Output(o),
        });
    }
    Circuit::compile_def(&glib, &wrap).map_err(|e| e.to_string())
}

/// Test vectors for `def`, with the outputs makine's simulation gives.
///
/// Chips with a clock get `cycles` clock cycles of random inputs (a reset
/// input is held for the first one); others get every input combination
/// when there are few, else a fixed random sample.
pub fn golden_steps(
    lib: &Library,
    def: &ChipDef,
    opts: &Options,
    cycles: usize,
) -> Result<Vec<TestStep>, String> {
    let mut circuit = compile_like_verilog(lib, def, opts)?;
    circuit.reset();
    let clock = def.inputs.iter().position(|p| is_clock(&p.name));
    let mut rng = Rng(0x6D61_6B69_6E65);
    let mut vectors: Vec<Vec<u64>> = Vec::new();
    let random = |rng: &mut Rng, p: &Pin| rng.next_u64() & p.mask();
    match clock {
        Some(clk) => {
            for cycle in 0..cycles {
                let ins: Vec<u64> = def
                    .inputs
                    .iter()
                    .map(|p| {
                        if p.width == 1 && is_reset(&p.name) {
                            (cycle == 0 || rng.next_u64().is_multiple_of(16)) as u64
                        } else {
                            random(&mut rng, p)
                        }
                    })
                    .collect();
                for c in [0, 1] {
                    let mut v = ins.clone();
                    v[clk] = c;
                    vectors.push(v);
                }
            }
        }
        None => {
            let total: u32 = def.inputs.iter().map(|p| p.width as u32).sum();
            if total <= 10 {
                for row in 0..1u64 << total {
                    let mut shift = total;
                    vectors.push(
                        def.inputs
                            .iter()
                            .map(|p| {
                                shift -= p.width as u32;
                                row >> shift & p.mask()
                            })
                            .collect(),
                    );
                }
            } else {
                for k in 0..(cycles * 2).max(64) {
                    vectors.push(
                        def.inputs
                            .iter()
                            .map(|p| match k {
                                0 => 0,
                                1 => p.mask(),
                                _ => random(&mut rng, p),
                            })
                            .collect(),
                    );
                }
            }
        }
    }
    vectors
        .into_iter()
        .enumerate()
        .map(|(step, inputs)| {
            let outs = circuit.eval_values(&inputs).map_err(|_| {
                trf!(
                    "devre {step}. adımda salınıyor",
                    "the circuit oscillates at step {step}"
                )
            })?;
            Ok(TestStep {
                inputs,
                expect: outs.into_iter().map(Some).collect(),
            })
        })
        .collect()
}

fn literal(width: u8, value: u64) -> String {
    format!("{width}'h{:X}", value & mask(width))
}

/// A testbench for `export` (the Verilog of `def`) that runs `steps`.
///
/// Each step sets the inputs (clock inputs last, after the others have
/// settled), waits, and compares every output that is not "don't care".
/// It prints `PASS` or `FAIL` and the number of mismatches. Run with
/// `+vcd` to write `wave.vcd` for a waveform viewer.
pub fn testbench(def: &ChipDef, export: &Export, steps: &[TestStep]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "// {}",
        trf!(
            "{} için test tezgâhı: makine'nin beklediği çıktılarla {} adım.",
            "Testbench for {}: {} steps with the outputs makine expects.",
            def.name,
            steps.len()
        )
    );
    let _ = writeln!(s, "`timescale 1ns/1ps");
    let _ = writeln!(s, "module tb;");
    for (p, n) in def.inputs.iter().zip(&export.inputs) {
        let _ = writeln!(s, "    reg {}{n} = 0;", range(p.width));
    }
    for (p, n) in def.outputs.iter().zip(&export.outputs) {
        let _ = writeln!(s, "    wire {}{n};", range(p.width));
    }
    let _ = writeln!(s, "    integer errors = 0;");
    let conns: Vec<String> = export
        .inputs
        .iter()
        .chain(&export.outputs)
        .map(|n| format!(".{n}({n})"))
        .collect();
    let _ = writeln!(s, "\n    {} dut ({});\n", export.top, conns.join(", "));
    let _ = writeln!(s, "    task expect_eq(input integer step, input [8*24-1:0] name, input [63:0] got, input [63:0] want);");
    let _ = writeln!(s, "        if (got !== want) begin");
    let _ = writeln!(s, "            errors = errors + 1;");
    let _ = writeln!(s, "            if (errors <= 20)");
    let _ = writeln!(
        s,
        "                $display(\"{} %0d: %0s = %h, {} %h\", step, name, got, want);",
        tr!("adım", "step"),
        tr!("beklenen", "expected")
    );
    let _ = writeln!(s, "        end");
    let _ = writeln!(s, "    endtask\n");
    let _ = writeln!(s, "    initial begin");
    let _ = writeln!(s, "        if ($test$plusargs(\"vcd\")) begin");
    let _ = writeln!(s, "            $dumpfile(\"wave.vcd\");");
    let _ = writeln!(s, "            $dumpvars(0, tb);");
    let _ = writeln!(s, "        end");
    let _ = writeln!(s, "        #1;");
    let clocks: Vec<bool> = def.inputs.iter().map(|p| is_clock(&p.name)).collect();
    let mut last: Vec<Option<u64>> = vec![None; def.inputs.len()];
    for (i, step) in steps.iter().enumerate() {
        let mut line = String::from("        ");
        for pass in [false, true] {
            let mut any = false;
            for (k, (p, n)) in def.inputs.iter().zip(&export.inputs).enumerate() {
                let v = step.inputs[k] & p.mask();
                if clocks[k] != pass || last[k] == Some(v) {
                    continue;
                }
                last[k] = Some(v);
                let _ = write!(line, "{n} = {}; ", literal(p.width, v));
                any = true;
            }
            if any {
                line.push_str("#1; ");
            }
        }
        for ((p, n), e) in def.outputs.iter().zip(&export.outputs).zip(&step.expect) {
            if let Some(e) = e {
                let _ = write!(
                    line,
                    "expect_eq({i}, \"{n}\", {n}, {}); ",
                    literal(p.width, *e)
                );
            }
        }
        s.push_str(line.trim_end());
        s.push('\n');
    }
    let _ = writeln!(
        s,
        "        if (errors == 0) $display(\"PASS: {} {}\");",
        steps.len(),
        tr!("adım", "steps")
    );
    let _ = writeln!(
        s,
        "        else $display(\"FAIL: %0d {}\", errors);",
        tr!("hata", "mismatches")
    );
    let _ = writeln!(s, "        $finish;");
    let _ = writeln!(s, "    end");
    let _ = writeln!(s, "endmodule");
    s
}

fn range(width: u8) -> String {
    if width == 1 {
        String::new()
    } else {
        format!("[{}:0] ", width - 1)
    }
}
