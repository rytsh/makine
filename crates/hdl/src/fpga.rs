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

    let readme = readme(cpu, program.len());
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
            ("ulx3s.v".into(), ULX3S.into()),
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

fn readme(cpu: &ChipDef, program: usize) -> String {
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
         ## Hız\n\n\
         İşlemci NAND kapılarından yapıldığı için en uzun yolu (bölücü) yaklaşık 250 ns sürer. \
         `soc.v` her komutta bu yolun durulmasını bekler: 12,5 MHz'de bir komut yaklaşık \
         1,5 µs, yani saniyede ~650 bin komut. Gerçek işlemciler aynı işi boru hattı \
         (pipeline) ve hızlı toplayıcılarla çok daha hızlı yapar; Wiki'deki \
         \"Hızlı işlemci\" sayfasına bak.\n\n\
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
         ## Speed\n\n\
         The processor is built from NAND gates, so its longest path (the divider) takes \
         about 250 ns. `soc.v` waits for it to settle on every instruction: at 12.5 MHz an \
         instruction takes about 1.5 µs, some 650 thousand instructions a second. Real \
         processors do the same work much faster with pipelines and fast adders; see the \
         \"A fast processor\" page in the Wiki.\n\n\
         Tools: the [OSS CAD Suite](https://github.com/YosysHQ/oss-cad-suite-build) \
         (yosys, nextpnr-ecp5, ecppack, openFPGALoader, verilator).\n"
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex() {
        assert_eq!(
            hex_image(&[0x13, 0, 0, 0, 0x73, 0, 0x10]),
            "00000013\n00100073\n"
        );
    }
}
