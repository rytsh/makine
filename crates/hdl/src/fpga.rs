//! A RISC-V computer for an FPGA: the player's CPU with block RAM, a UART
//! and a timer (`soc.v`), wired to a board, with the program in RAM.

use std::fmt::Write;

use makine_lang::{tr, trf};
use makine_sim::{ChipDef, Library};

use crate::project::Project;
use crate::verilog::verilog;
use crate::Options;

const SOC: &str = include_str!("soc.v");
const ULX3S: &str = include_str!("boards/ulx3s.v");
const ULX3S_PLL: &str = include_str!("boards/ulx3s_pll.v");
const ULX3S_LPF: &str = include_str!("boards/ulx3s.lpf");

/// Bytes of RAM in the SoC: 64 KB of the ECP5's block RAM.
pub const RAM_BYTES: usize = 64 * 1024;

/// The pins the CPU chip must have, as in the `Cpu` level.
const CPU_INPUTS: &[(&str, u8)] = &[("instr", 32), ("rdata", 32), ("reset", 1), ("clk", 1)];
const CPU_OUTPUTS: &[(&str, u8)] = &[
    ("pc", 32),
    ("addr", 32),
    ("wdata", 32),
    ("wmask", 4),
    ("we", 1),
];

/// A program image as `$readmemh` reads it: one little-endian word per line.
pub fn hex_image(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 9 / 4 + 9);
    for c in bytes.chunks(4) {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        let _ = writeln!(s, "{:08x}", u32::from_le_bytes(w));
    }
    if bytes.is_empty() {
        s.push_str("00100073\n");
    }
    s
}

/// The project for an ULX3S board around `cpu` (a chip with the `Cpu`
/// level's pins) running `program` (a memory image starting at address 0).
pub fn fpga_project(lib: &Library, cpu: &ChipDef, program: &[u8]) -> Result<Project, String> {
    let pins = |p: &[makine_sim::Pin]| -> Vec<(String, u8)> {
        p.iter().map(|p| (p.name.clone(), p.width)).collect()
    };
    let want = |p: &[(&str, u8)]| -> Vec<(String, u8)> {
        p.iter().map(|(n, w)| (n.to_string(), *w)).collect()
    };
    if pins(&cpu.inputs) != want(CPU_INPUTS) || pins(&cpu.outputs) != want(CPU_OUTPUTS) {
        return Err(tr!(
            "FPGA bilgisayarı, İşlemci seviyesindeki uçlara sahip bir çip ister: instr[32], rdata[32], reset, clk → pc[32], addr[32], wdata[32], wmask[4], we",
            "The FPGA computer needs a chip with the Processor level's pins: instr[32], rdata[32], reset, clk → pc[32], addr[32], wdata[32], wmask[4], we"
        )
        .into());
    }
    if program.len() > RAM_BYTES {
        return Err(trf!(
            "program {} bayt; FPGA'daki RAM {} bayt",
            "the program is {} bytes; the FPGA's RAM is {} bytes",
            program.len(),
            RAM_BYTES
        ));
    }
    // The SoC instantiates the CPU as `mk_Cpu`, whatever its name.
    let mut def = cpu.clone();
    def.name = "Cpu".into();
    let mut lib = lib.clone();
    lib.remove(&cpu.name);
    let export = verilog(&lib, &def, &Options::default())?;
    let depth = cpu_depth(&lib, &def)?;
    let settle = settle_clocks(depth);
    let board = ULX3S.replace(
        ".RAM_INIT(\"program.hex\")",
        &format!(".RAM_INIT(\"program.hex\"),\n        .SETTLE({settle})"),
    );

    let makefile = format!(
        "# {}\n\
         PROGRAM ?= program.hex\n\n\
         .PHONY: all prog sim clean\n\n\
         all: makine.bit\n\n\
         makine.json: cpu.v soc.v ulx3s.v ulx3s_pll.v $(PROGRAM)\n\
         \tcp $(PROGRAM) program.hex 2>/dev/null || true\n\
         \tyosys -q -p \"read_verilog cpu.v soc.v ulx3s.v ulx3s_pll.v; synth_ecp5 -top top -json $@\"\n\n\
         makine.config: makine.json ulx3s.lpf\n\
         \tnextpnr-ecp5 --85k --package CABGA381 --json $< --lpf ulx3s.lpf --lpf-allow-unconstrained --timing-allow-fail --textcfg $@\n\n\
         makine.bit: makine.config\n\
         \tecppack $< $@\n\n\
         prog: makine.bit ## {}\n\
         \topenFPGALoader -b ulx3s $<\n\n\
         sim: ## {}\n\
         \tverilator --cc --exe --build -j 0 -O3 -Wno-fatal -Wno-lint -Wno-UNOPTFLAT -Wno-MULTIDRIVEN --top-module makine_soc -GCLK_HZ=400000 -GBAUD=100000 -GSETTLE=2 soc.v cpu.v sim.cpp -o sim\n\
         \t./obj_dir/sim\n\n\
         clean:\n\
         \trm -rf makine.json makine.config makine.bit obj_dir\n",
        tr!(
            "make: FPGA dosyasını üret (birkaç dakika). make prog: karta yükle. make sim: Verilator'da çalıştır.",
            "make: build the FPGA bitstream (a few minutes). make prog: load it onto the board. make sim: run it in Verilator."
        ),
        tr!("ULX3S'e yükle", "load onto the ULX3S"),
        tr!(
            "bilgisayarda simüle et, UART çıktısını göster",
            "simulate on the computer, show the UART output"
        ),
    );

    let readme = readme(cpu, program.len(), depth, settle);
    Ok(Project {
        files: vec![
            (
                "cpu.v".into(),
                format!(
                    "// {}\n`default_nettype none\n\n{}",
                    trf!(
                        "makine'den dışa aktarılan işlemci: {}",
                        "The processor exported from makine: {}",
                        cpu.name
                    ),
                    export.source
                ),
            ),
            ("soc.v".into(), SOC.into()),
            ("ulx3s.v".into(), board),
            ("ulx3s_pll.v".into(), ULX3S_PLL.into()),
            ("ulx3s.lpf".into(), ULX3S_LPF.into()),
            ("program.hex".into(), hex_image(program)),
            ("sim.cpp".into(), SIM_CPP.into()),
            ("Makefile".into(), makefile),
            ("README.md".into(), readme),
        ],
        notes: vec![],
    })
}

/// Gates on the longest path of the CPU, with storage chips as the
/// flip-flops they are exported as.
fn cpu_depth(lib: &Library, def: &ChipDef) -> Result<usize, String> {
    let gates = crate::verilog::gate_library(lib, &Options::default());
    Ok(makine_sim::Circuit::compile_def(&gates, def)
        .map_err(|e| e.to_string())?
        .depth()
        .gates)
}

/// The SoC clock in nanoseconds (12.5 MHz on the ULX3S).
const CLOCK_NS: f64 = 80.0;

/// Clocks the SoC waits for the CPU's logic to settle (`SETTLE` in
/// `soc.v`), from its longest path. On the ECP5, measured with nextpnr: a
/// path of `depth` NANDs takes about 20 ns + 0.06 ns per gate (the reference
/// CPU's 4615 gates: 255 ns; with the multi-step divider, 208 gates: 27 ns).
/// Twice that, as the board's timing is not checked.
pub fn settle_clocks(depth: usize) -> usize {
    let ns = 2.0 * (20.0 + 0.06 * depth as f64);
    ((ns / CLOCK_NS).ceil() as usize).clamp(1, 255)
}

/// Verilator testbench: runs the SoC and prints what it sends on the UART.
const SIM_CPP: &str = r#"// Runs the SoC in Verilator and decodes its UART output (8N1).
// Built by `make sim` with CLK_HZ=400000 and BAUD=100000: 4 clocks per bit.
#include "Vmakine_soc.h"
#include <cstdio>
#include <cstdlib>

static Vmakine_soc t;
static void tick() { t.clk = 0; t.eval(); t.clk = 1; t.eval(); }

int main(int argc, char **argv) {
    const int CPB = 4;
    long max = argc > 1 ? atol(argv[1]) : 2000000000L;
    t.uart_rx = 1;
    t.reset = 1;
    for (int i = 0; i < 4; i++) tick();
    t.reset = 0;
    int bit = -1, count = 0, byte = 0;
    long cyc = 0, tail = 12 * CPB;
    while (cyc < max && tail > 0) {
        tick();
        cyc++;
        if (t.halted) tail--;
        if (bit < 0) {
            if (!t.uart_tx) { bit = 0; count = CPB + CPB / 2; byte = 0; }
        } else if (--count == 0) {
            if (bit < 8) { byte |= t.uart_tx << bit; bit++; count = CPB; }
            else { putchar(byte); fflush(stdout); bit = -1; }
        }
    }
    fprintf(stderr, "\n[%ld clocks, %s]\n", cyc, t.halted ? "halted" : "still running");
    return t.halted ? 0 : 1;
}
"#;

fn readme(cpu: &ChipDef, program: usize, depth: usize, settle: usize) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# {}\n",
        trf!(
            "{} işlemcili FPGA bilgisayarı",
            "FPGA computer with the {} processor",
            cpu.name
        )
    );
    let _ = writeln!(
        s,
        "{}\n",
        trf!(
            "makine'de NAND kapılarından kurduğun RISC-V işlemci, 64 KB blok RAM, bir seri port (UART, 115200 baud) ve bir zamanlayıcıyla birlikte. Kart: **ULX3S** (Lattice ECP5-85F). RAM'de {} baytlık program var.",
            "The RISC-V processor you built from NAND gates in makine, with 64 KB of block RAM, a serial port (UART, 115200 baud) and a timer. Board: **ULX3S** (Lattice ECP5-85F). RAM holds a {}-byte program.",
            program
        )
    );
    s.push_str(tr!(
        "| Dosya | |\n|---|---|\n\
         | `cpu.v` | senin işlemcin |\n\
         | `soc.v` | bellek, UART, zamanlayıcı ve işlemciyi süren sıralayıcı |\n\
         | `ulx3s.v`, `ulx3s_pll.v`, `ulx3s.lpf` | kartın pinleri ve saati |\n\
         | `program.hex` | RAM'in ilk içeriği |\n\
         | `sim.cpp` | Verilator ile bilgisayarda çalıştırmak için |\n\n\
         ```sh\n\
         make sim     # önce bilgisayarda dene: programın çıktısı ekrana gelir\n\
         make         # FPGA dosyası (makine.bit) – birkaç dakika sürer\n\
         make prog    # karta yükle (openFPGALoader)\n\
         ```\n\n\
         Sonra kartın USB portuna bir seri terminal bağla (`screen /dev/ttyUSB0 115200` ya da \
         `picocom -b 115200 /dev/ttyUSB0`) ve FIRE1 düğmesine bas: program baştan çalışır. \
         LED'ler son yazılan baytı, LED 7 programın bittiğini gösterir.\n\n\
         Başka bir program için: `MK_RAM=64K sdk/build.sh prog.c`, sonra \
         `llvm-objcopy -O binary prog.elf prog.bin` ve makine'nin \"FPGA bilgisayarı\" \
         düğmesiyle projeyi yeniden üret ya da `program.hex`'i değiştirip `make` çalıştır.\n\n\
         Araçlar: [OSS CAD Suite](https://github.com/YosysHQ/oss-cad-suite-build) \
         (yosys, nextpnr-ecp5, ecppack, openFPGALoader, verilator).\n",
        "| File | |\n|---|---|\n\
         | `cpu.v` | your processor |\n\
         | `soc.v` | memory, UART, timer and the sequencer driving the processor |\n\
         | `ulx3s.v`, `ulx3s_pll.v`, `ulx3s.lpf` | the board's pins and clock |\n\
         | `program.hex` | the RAM's initial contents |\n\
         | `sim.cpp` | for running it on the computer with Verilator |\n\n\
         ```sh\n\
         make sim     # try it on the computer first: the program's output appears\n\
         make         # the FPGA bitstream (makine.bit) – takes a few minutes\n\
         make prog    # load it onto the board (openFPGALoader)\n\
         ```\n\n\
         Then open a serial terminal on the board's USB port (`screen /dev/ttyUSB0 115200` or \
         `picocom -b 115200 /dev/ttyUSB0`) and press FIRE1: the program runs from the start. \
         The LEDs show the last byte written, LED 7 that the program finished.\n\n\
         For another program: `MK_RAM=64K sdk/build.sh prog.c`, then \
         `llvm-objcopy -O binary prog.elf prog.bin`, and export again with makine's \
         \"FPGA computer\" button, or replace `program.hex` and run `make`.\n\n\
         Tools: the [OSS CAD Suite](https://github.com/YosysHQ/oss-cad-suite-build) \
         (yosys, nextpnr-ecp5, ecppack, openFPGALoader, verilator).\n"
    ));
    // FETCH, DECODE, SETTLE, STEP, NEXT; loads also ADDR, WAIT, READ and a
    // second SETTLE.
    let clocks = settle + 4;
    let per_second = 1e9 / (CLOCK_NS * clocks as f64);
    let _ = write!(
        s,
        "\n{}\n",
        trf!(
            "## Hız\n\n\
             İşlemcinin en uzun yolu {depth} NAND kapısı. `soc.v` her komutta bu yolun durulmasını \
             {settle} saat bekler: 12,5 MHz'de bir komut {clocks} saat, saniyede yaklaşık {} komut \
             (yükleme komutları biraz daha uzun). Yolu kısaltmanın en etkili yolu bölücüyü çok adımlı \
             yapmaktır: Wiki'deki \"Hızlı işlemci\" sayfasına ve Adım Adım Çarpma ve Bölme \
             seviyesine bak.",
            "## Speed\n\n\
             The processor's longest path is {depth} NAND gates. `soc.v` waits {settle} clocks for it \
             to settle on every instruction: at 12.5 MHz an instruction takes {clocks} clocks, about \
             {} instructions a second (loads a little longer). The best way to shorten the \
             path is a multi-step divider: see the \"A fast processor\" page in the Wiki and the \
             Multiply and Divide, Step by Step level.",
            rate(per_second)
        )
    );
    s
}

/// A number of instructions a second, in words: "650 bin", "2,5 milyon".
fn rate(n: f64) -> String {
    if n >= 1e6 {
        let m = format!("{:.1}", n / 1e6);
        trf!("{} milyon", "{} million", m.replace('.', tr!(",", ".")))
    } else {
        trf!("{} bin", "{} thousand", (n / 1000.0).round())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settle_follows_the_path() {
        // The single-cycle reference CPU waits as long as before; the one
        // with the multi-step divider hardly at all.
        assert_eq!(settle_clocks(4615), 8);
        assert_eq!(settle_clocks(208), 1);
        assert_eq!(settle_clocks(1_000_000), 255);
    }

    #[test]
    fn hex() {
        assert_eq!(
            hex_image(&[0x13, 0, 0, 0, 0x73, 0, 0x10]),
            "00000013\n00100073\n"
        );
    }
}
