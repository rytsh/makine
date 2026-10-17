//! Every level must be solvable using only the parts it allows, and wrong
//! solutions must be rejected.

use makine_levels::{check, find, levels, reference::reference, CheckError};
use makine_sim::{out, ChipBuilder, Library, Verdict};

fn lib_until(chip: &str) -> Library {
    let mut lib = Library::new();
    for l in levels().iter().take_while(|l| l.chip != chip) {
        lib.insert(reference(l.chip));
    }
    lib
}

#[test]
fn all_levels_solvable_in_order() {
    let mut lib = Library::new();
    for &level in levels() {
        let def = reference(level.chip);
        let verdict = check(level, &lib, &def).unwrap();
        assert_eq!(verdict, Verdict::Pass, "level {}", level.chip);
        lib.insert(def);
    }
}

#[test]
fn level_names_unique() {
    let all = levels();
    for (i, l) in all.iter().enumerate() {
        assert!(
            all[..i].iter().all(|o| o.chip != l.chip),
            "duplicate {}",
            l.chip
        );
    }
}

#[test]
fn wrong_solution_is_rejected() {
    let lib = Library::new();
    let level = find("Not").unwrap();
    // Wire input straight to output.
    let mut b = ChipBuilder::new("Not", level.inputs, level.outputs);
    let x = b.input("in");
    b.output("out", x);
    match check(level, &lib, &b.build()).unwrap() {
        Verdict::Fail(f) => {
            assert_eq!(f.inputs, vec![0]);
            assert_eq!(f.actual, vec![0]);
        }
        v => panic!("expected failure, got {v:?}"),
    }
}

#[test]
fn later_chips_are_forbidden() {
    let lib = lib_until("Or");
    let level = find("Not").unwrap();
    let mut b = ChipBuilder::new("Not", level.inputs, level.outputs);
    let x = b.input("in");
    let o = b.chip1("And", &[x, x]);
    b.output("out", o);
    assert_eq!(
        check(level, &lib, &b.build()),
        Err(CheckError::ForbiddenPart("And".into()))
    );
}

#[test]
fn latch_without_memory_fails() {
    let lib = lib_until("Latch");
    let level = find("Latch").unwrap();
    // q = set: forgets as soon as set drops.
    let mut b = ChipBuilder::new("Latch", level.inputs, level.outputs);
    let s = b.input("set");
    b.output("q", s);
    match check(level, &lib, &b.build()).unwrap() {
        Verdict::Fail(f) => assert_eq!(f.step, 1),
        v => panic!("expected failure, got {v:?}"),
    }
}

#[test]
fn level_triggered_dff_fails() {
    // A plain D latch with en = clk is transparent while clk is high, so it
    // must fail the edge-triggered test.
    let lib = lib_until("DFF");
    let level = find("DFF").unwrap();
    let mut b = ChipBuilder::new("DFF", level.inputs, level.outputs);
    let (d, clk) = (b.input("d"), b.input("clk"));
    let q = b.chip1("DLatch", &[d, clk]);
    b.output("q", q);
    assert!(matches!(
        check(level, &lib, &b.build()).unwrap(),
        Verdict::Fail(_)
    ));
}

#[test]
fn adder_without_carry_in_fails() {
    let lib = lib_until("Adder8");
    let level = find("Adder8").unwrap();
    let mut b = ChipBuilder::new("Adder8", level.inputs, level.outputs);
    let (x, y) = (b.input("a"), b.input("b"));
    let w = [1u8; 8];
    let xs = b.split(&w, x);
    let ys = b.split(&w, y);
    let mut carry = b.constant(1, 0);
    let mut sums = Vec::new();
    for i in 0..8 {
        let a = b.chip("FullAdder", &[out(xs, i), out(ys, i), carry]);
        sums.push(out(a, 0));
        carry = out(a, 1);
    }
    let s = b.join(&w, &sums);
    b.output("sum", s);
    b.output("carry", carry);
    match check(level, &lib, &b.build()).unwrap() {
        Verdict::Fail(f) => assert_eq!(f.inputs[2], 1),
        v => panic!("expected failure, got {v:?}"),
    }
}

/// Library of every reference chip, with fast models for speed.
fn fast_lib() -> Library {
    let refs: Vec<_> = levels().iter().map(|l| reference(l.chip)).collect();
    makine_levels::fast::library(refs.iter().filter(|d| d.name != "Cpu"), true)
}

/// A CPU whose ALU computes `alu` instead must fail with a message naming
/// the instruction.
fn broken_cpu_fails(alu_fn: fn(&[u64]) -> Vec<u64>, expect_in_message: &str) {
    let mut lib = fast_lib();
    let level = find("Alu").unwrap();
    lib.insert_native(
        "Alu",
        makine_sim::NativeDef::function(level.inputs, level.outputs, alu_fn),
    );
    lib.set_fast("Alu", true);
    match check(find("Cpu").unwrap(), &lib, &reference("Cpu")).unwrap() {
        Verdict::Wrong { message, .. } => {
            eprintln!("{message}");
            assert!(message.contains(expect_in_message), "message: {message}");
        }
        v => panic!("expected failure, got {v:?}"),
    }
}

#[test]
fn cpu_with_wrong_sra_fails() {
    // sra behaves like srl.
    broken_cpu_fails(
        |i| {
            let op = if i[2] == 13 { 5 } else { i[2] };
            vec![makine_levels::cpu::alu(i[0] as u32, i[1] as u32, op) as u64]
        },
        "sra",
    );
}

#[test]
fn cpu_with_wrong_sltu_fails() {
    broken_cpu_fails(
        |i| {
            let op = if i[2] == 3 { 2 } else { i[2] };
            vec![makine_levels::cpu::alu(i[0] as u32, i[1] as u32, op) as u64]
        },
        "sltu",
    );
}

#[test]
fn cpu_writing_x0_fails() {
    // A register file where x0 is a real register.
    let mut lib = fast_lib();
    let level = find("RegFile").unwrap();
    lib.insert_native(
        "RegFile",
        makine_sim::NativeDef::clocked(
            level.inputs,
            level.outputs,
            &["rs1", "rs2"],
            [0u64; 32],
            |r, i| {
                if i[4] & 1 == 1 {
                    r[i[2] as usize & 31] = i[3];
                }
            },
            |r, i, o| {
                o[0] = r[i[0] as usize & 31];
                o[1] = r[i[1] as usize & 31];
            },
        ),
    );
    match check(find("Cpu").unwrap(), &lib, &reference("Cpu")).unwrap() {
        Verdict::Wrong { message, .. } => {
            eprintln!("{message}");
            assert!(message.contains("x0"), "{message}")
        }
        v => panic!("expected failure, got {v:?}"),
    }
}

/// The gate-level CPU (no fast models at all) passes. Slow in debug builds.
#[test]
#[ignore]
fn cpu_passes_gate_level() {
    use makine_levels::cpu::{run_lockstep, CircuitCpu, TEST_PROGRAMS};
    use makine_sim::Circuit;
    let mut lib = Library::new();
    for &l in levels() {
        lib.insert(reference(l.chip));
    }
    for chip in ["Alu", "RegFile", "MulDiv", "Cpu"] {
        let c = Circuit::compile(&lib, chip).unwrap();
        eprintln!("{chip}: {} NAND", c.gate_count());
    }
    let mut cpu = CircuitCpu::new(Circuit::compile(&lib, "Cpu").unwrap());
    let t = std::time::Instant::now();
    let mut steps = 0;
    for (_, src) in TEST_PROGRAMS {
        let p = makine_rv::asm::assemble(src).unwrap();
        run_lockstep(&mut cpu, &p.bytes, &mut steps).unwrap();
    }
    let dt = t.elapsed().as_secs_f64();
    eprintln!(
        "{steps} instructions in {dt:.2}s = {:.0} instr/s",
        steps as f64 / dt
    );
}

#[test]
#[ignore]
fn cpu_speed_fast_models() {
    use makine_levels::cpu::{run_lockstep, CircuitCpu, TEST_PROGRAMS};
    use makine_sim::Circuit;
    let lib = fast_lib();
    let c = Circuit::compile_def(&lib, &reference("Cpu")).unwrap();
    eprintln!("{}", c.debug_stats());
    let mut cpu = CircuitCpu::new(c);
    let t = std::time::Instant::now();
    let mut steps = 0;
    for _ in 0..20 {
        for (_, src) in TEST_PROGRAMS {
            let p = makine_rv::asm::assemble(src).unwrap();
            run_lockstep(&mut cpu, &p.bytes, &mut steps).unwrap();
        }
    }
    let dt = t.elapsed().as_secs_f64();
    eprintln!(
        "{steps} instructions in {dt:.2}s = {:.0} instr/s",
        steps as f64 / dt
    );
}
