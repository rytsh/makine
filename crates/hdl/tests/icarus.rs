//! Every reference solution of the learning path, exported and run in a
//! real Verilog simulator against its level's own test vectors.
//!
//! Needs Icarus Verilog (`iverilog`, `vvp`) on the PATH; Yosys is used too
//! when present. Without them the tests only check that exporting works.

use std::path::{Path, PathBuf};
use std::process::Command;

use makine_hdl::{
    golden_steps, testbench, tiny_tapeout, verilog, verilog_project, Options, TinyTapeout,
};
use makine_levels::reference::reference;
use makine_levels::{levels, Test};
use makine_sim::{Library, TestStep};

fn have(tool: &str) -> bool {
    Command::new(tool).arg("-V").output().is_ok()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("makine-hdl-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Compile and run `files` in `dir`; return what the simulation printed.
fn icarus(dir: &Path, files: &[&str]) -> String {
    let out = Command::new("iverilog")
        .current_dir(dir)
        .args(["-g2012", "-o", "sim.vvp"])
        .args(files)
        .output()
        .expect("iverilog");
    assert!(
        out.status.success(),
        "iverilog failed in {}:\n{}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new("vvp")
        .current_dir(dir)
        .arg("sim.vvp")
        .output()
        .expect("vvp");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The reference solutions, as a player who solved every level would have.
fn reference_library() -> Library {
    let mut lib = Library::new();
    for l in levels() {
        lib.insert(reference(l.chip));
    }
    lib
}

/// Test steps a level runs, where it has a table of them.
fn level_steps(chip: &str) -> Option<Vec<TestStep>> {
    let level = makine_levels::find(chip)?;
    match level.test {
        Test::Custom(_) => None,
        _ => Some(level.steps()),
    }
}

#[test]
fn every_reference_exports() {
    let lib = reference_library();
    for l in levels() {
        let def = reference(l.chip);
        for primitives in [false, true] {
            let opts = Options {
                primitives,
                ..Options::default()
            };
            let e = verilog(&lib, &def, &opts).unwrap_or_else(|e| panic!("{}: {e}", l.chip));
            assert!(
                e.source.contains(&format!("module {}(", e.top)),
                "{}",
                l.chip
            );
        }
    }
}

/// Run every level with a table test whose reference has `gates` in
/// `range`, in Icarus. Returns how many ran.
fn run_levels(range: std::ops::Range<usize>, steps_limit: usize) -> usize {
    let lib = reference_library();
    let mut checked = 0;
    for l in levels() {
        let Some(steps) = level_steps(l.chip) else {
            continue;
        };
        let def = reference(l.chip);
        let gates = makine_sim::Circuit::compile_def(&lib, &def)
            .unwrap()
            .gate_count();
        if !range.contains(&gates) {
            continue;
        }
        // Gate-level loops (latches) for the storage levels themselves,
        // primitives for everything that is built on them.
        let opts = Options {
            primitives: !matches!(l.chip, "Latch" | "DLatch" | "DFF" | "Bit"),
            ..Options::default()
        };
        // Icarus simulates every gate: keep gates × steps bounded.
        let limit = steps_limit.min(2_000_000 / gates.max(1)).max(40);
        let steps: Vec<TestStep> = steps.into_iter().take(limit).collect();
        let e = verilog(&lib, &def, &opts).unwrap();
        let dir = scratch(l.chip);
        std::fs::write(dir.join("design.v"), &e.source).unwrap();
        std::fs::write(dir.join("tb.v"), testbench(&def, &e, &steps)).unwrap();
        let out = icarus(&dir, &["design.v", "tb.v"]);
        assert!(out.contains("PASS"), "{}:\n{out}", l.chip);
        checked += 1;
    }
    checked
}

/// The exported Verilog passes the same test the chip passed in makine.
#[test]
fn references_pass_their_levels_in_icarus() {
    if !have("iverilog") {
        eprintln!("iverilog not found; skipping");
        return;
    }
    let checked = run_levels(0..20_000, 1500);
    assert!(checked > 30, "only {checked} levels checked");
}

/// The same for the big chips (the divider has ~10^5 gates, and Icarus
/// simulates every one). Slow: `make test-slow`.
#[test]
#[ignore]
fn big_references_pass_their_levels_in_icarus() {
    if !have("iverilog") {
        eprintln!("iverilog not found; skipping");
        return;
    }
    assert!(run_levels(20_000..usize::MAX, 60) > 0);
}

/// The mini CPU from the reference solutions runs the same as makine's
/// simulation of it, cycle by cycle.
#[test]
fn mini_cpu_matches_makine_in_icarus() {
    if !have("iverilog") {
        eprintln!("iverilog not found; skipping");
        return;
    }
    let lib = reference_library();
    let opts = Options::default();
    let def = reference("MiniCpu");
    let steps = golden_steps(&lib, &def, &opts, 200).unwrap();
    let p = verilog_project(&lib, &def, &steps, &opts).unwrap();
    let dir = scratch("MiniCpu");
    p.write_to(&dir).unwrap();
    let out = icarus(&dir, &["design.v", "tb.v"]);
    assert!(out.contains("PASS"), "{out}");
}

/// The RISC-V CPU from the reference solutions runs the same as makine's
/// simulation of it, cycle by cycle. Slow (about a minute): `make test-slow`.
#[test]
#[ignore]
fn cpu_matches_makine_in_icarus() {
    if !have("iverilog") {
        eprintln!("iverilog not found; skipping");
        return;
    }
    let lib = reference_library();
    let opts = Options::default();
    let def = reference("Cpu");
    let steps = golden_steps(&lib, &def, &opts, 120).unwrap();
    let p = verilog_project(&lib, &def, &steps, &opts).unwrap();
    let dir = scratch("Cpu");
    p.write_to(&dir).unwrap();
    let out = icarus(&dir, &["design.v", "tb.v"]);
    assert!(out.contains("PASS"), "{out}");
}

#[test]
fn tiny_tapeout_project_runs() {
    let lib = reference_library();
    // MiniAcc8: d[8] and load are 9 input bits besides clk and reset, q[8]
    // is 8 output bits: one uio pin is an input.
    let def = reference("MiniAcc8");
    let steps = golden_steps(&lib, &def, &Options::default(), 80).unwrap();
    let tt = TinyTapeout {
        github: "ornek".into(),
        ..TinyTapeout::default()
    };
    let p = tiny_tapeout(&lib, &def, &steps, &tt).unwrap();
    let info = p.file("info.yaml").unwrap();
    assert!(info.contains("top_module:  \"tt_um_ornek_miniacc8\""));
    assert!(info.contains("ui[0]: \"d[0]\""));
    assert!(p
        .file("src/project.v")
        .unwrap()
        .contains("module tt_um_ornek_miniacc8"));

    assert!(info.contains("uio[0]: \"load\""));
    // A chip with too many pins is refused.
    assert!(tiny_tapeout(&lib, &reference("Counter12"), &[], &tt).is_err());

    if !have("iverilog") {
        return;
    }
    let dir = scratch("tt");
    p.write_to(&dir).unwrap();
    let out = icarus(
        &dir.join("test"),
        &["../src/makine.v", "../src/project.v", "tb.v"],
    );
    assert!(out.contains("PASS"), "{out}");
}

/// Yosys accepts the export and synthesizes it without combinational loops.
#[test]
fn yosys_synthesizes() {
    if !have("yosys") {
        eprintln!("yosys not found; skipping");
        return;
    }
    let lib = reference_library();
    for chip in ["Adder8", "Counter12", "MiniCpu"] {
        let def = reference(chip);
        let p = verilog_project(&lib, &def, &[], &Options::default()).unwrap();
        let dir = scratch(&format!("yosys-{chip}"));
        p.write_to(&dir).unwrap();
        let script = format!(
            "read_verilog design.v; synth -top {}; check -assert; stat",
            makine_hdl::verilog(&lib, &def, &Options::default())
                .unwrap()
                .top
        );
        let out = Command::new("yosys")
            .current_dir(&dir)
            .args(["-q", "-p", &script])
            .output()
            .expect("yosys");
        assert!(
            out.status.success(),
            "{chip}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// What the wiki says: the mini CPU needs 33 pin bits, more than Tiny
/// Tapeout's 24, and the refusal says why.
#[test]
fn mini_cpu_does_not_fit_tiny_tapeout() {
    let lib = reference_library();
    let e = tiny_tapeout(&lib, &reference("MiniCpu"), &[], &TinyTapeout::default()).unwrap_err();
    assert!(e.contains("16") && e.contains("17"), "{e}");
}

/// A chip using the bidirectional pins both ways: 10 bits in, 10 bits out,
/// so two uio pins are inputs and two are outputs.
#[test]
fn tiny_tapeout_splits_uio_pins() {
    let lib = reference_library();
    let mut b = makine_sim::ChipBuilder::new("Not10", &["in[10]"], &["out[10]"]);
    let x = b.input("in");
    let n = b.chip_n("Not", 10, &[x]);
    b.output("out", makine_sim::out(n, 0));
    let def = b.build();
    let steps = golden_steps(&lib, &def, &Options::default(), 100).unwrap();
    let p = tiny_tapeout(&lib, &def, &steps, &TinyTapeout::default()).unwrap();
    let info = p.file("info.yaml").unwrap();
    assert!(info.contains("uio[1]: \"in[9]\""), "{info}");
    assert!(info.contains("uio[2]: \"out[8]\""), "{info}");
    assert!(p
        .file("src/project.v")
        .unwrap()
        .contains("assign uio_oe = 8'h0C;"));
    if !have("iverilog") {
        return;
    }
    let dir = scratch("tt-split");
    p.write_to(&dir).unwrap();
    let out = icarus(
        &dir.join("test"),
        &["../src/makine.v", "../src/project.v", "tb.v"],
    );
    assert!(out.contains("PASS"), "{out}");
}

/// Bytes of a flat image of an ELF, from address 0.
fn flat(elf: &[u8]) -> Vec<u8> {
    let img = makine_rv::elf::load(elf).unwrap();
    let mut out = vec![0u8; img.end() as usize];
    for (a, b) in &img.segments {
        out[*a as usize..*a as usize + b.len()].copy_from_slice(b);
    }
    out
}

/// The FPGA computer runs a C program: the reference CPU, the SoC and the
/// UART in Verilator, compiled with the SDK for 64 KB. Needs verilator,
/// clang and ld.lld; slow to build (about a minute): `make test-slow`.
#[test]
#[ignore]
fn fpga_computer_runs_c_in_verilator() {
    if !have("verilator") || !have("clang") {
        eprintln!("verilator or clang not found; skipping");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = scratch("fpga");
    let src = dir.join("hello.c");
    std::fs::write(
        &src,
        "#include <stdio.h>\n#include <stdlib.h>\n\
         int main(void) {\n\
             int *v = malloc(10 * sizeof *v), s = 0;\n\
             for (int i = 0; i < 10; i++) v[i] = i * i;\n\
             for (int i = 0; i < 10; i++) s += v[i];\n\
             printf(\"Merhaba FPGA! %d %d\\n\", s, 1000 / 7);\n\
             return 0;\n\
         }\n",
    )
    .unwrap();
    let out = Command::new(root.join("sdk/build.sh"))
        .env("MK_RAM", "64K")
        .arg(&src)
        .arg("-o")
        .arg(dir.join("hello.elf"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let image = flat(&std::fs::read(dir.join("hello.elf")).unwrap());

    let lib = reference_library();
    let p = makine_hdl::fpga_project(&lib, &reference("Cpu"), &image).unwrap();
    p.write_to(&dir).unwrap();
    let out = Command::new("make")
        .current_dir(&dir)
        .arg("sim")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Merhaba FPGA! 285 142"),
        "{text}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn fpga_project_checks_pins() {
    let lib = reference_library();
    assert!(makine_hdl::fpga_project(&lib, &reference("MiniCpu"), &[]).is_err());
    let p = makine_hdl::fpga_project(&lib, &reference("Cpu"), &[0x73, 0, 0x10, 0]).unwrap();
    assert_eq!(p.file("program.hex"), Some("00100073\n"));
    assert!(p.file("cpu.v").unwrap().contains("module mk_Cpu("));
    assert!(p.file("Makefile").unwrap().contains("nextpnr-ecp5"));
}
