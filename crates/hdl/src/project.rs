//! Folders to hand to the tools: plain files, written by the caller.

use std::fmt::Write;
use std::path::Path;

use makine_lang::{tr, trf};
use makine_sim::{ChipDef, Library, TestStep};

use crate::bench::{is_clock, is_reset, testbench};
use crate::verilog::{verilog, Export};
use crate::Options;

/// Files of a project, relative paths and contents.
#[derive(Clone, Debug, Default)]
pub struct Project {
    pub files: Vec<(String, String)>,
    /// Things worth knowing before using it (a latch in a synthesized design,
    /// for instance).
    pub notes: Vec<String>,
}

impl Project {
    pub fn file(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, c)| c.as_str())
    }

    /// Write every file under `dir`, creating folders as needed.
    pub fn write_to(&self, dir: &Path) -> std::io::Result<()> {
        for (path, contents) in &self.files {
            let p = dir.join(path);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, contents)?;
        }
        Ok(())
    }
}

fn header(def: &ChipDef) -> String {
    format!(
        "// {}\n// {}\n`default_nettype none\n\n",
        trf!(
            "makine'den dışa aktarıldı: {}",
            "Exported from makine: {}",
            def.name
        ),
        tr!(
            "Bu dosyayı elle değiştirme; çipi makine'de değiştirip yeniden aktar.",
            "Do not edit by hand; change the chip in makine and export again."
        )
    )
}

fn notes(export: &Export) -> Vec<String> {
    let mut notes = Vec::new();
    if export.primitives.iter().any(|p| p != "DFF") {
        notes.push(tr!(
            "Tasarımda mandal (Latch/DLatch) var. Simülasyonda sorun değil; ama FPGA ve çip araçları saatli flip-flop'ları sever, mandallar zamanlama analizini zorlaştırır.",
            "The design has latches (Latch/DLatch). Fine in simulation, but FPGA and chip tools prefer clocked flip-flops; latches make timing analysis harder."
        ).to_string());
    }
    notes
}

/// A folder for Icarus Verilog and Yosys: the chip, a testbench running
/// `steps`, a synthesis script and a Makefile.
pub fn verilog_project(
    lib: &Library,
    def: &ChipDef,
    steps: &[TestStep],
    opts: &Options,
) -> Result<Project, String> {
    let export = verilog(lib, def, opts)?;
    let design = format!("{}{}", header(def), export.source);
    let tb = testbench(def, &export, steps);
    let synth = format!(
        "# {}\nread_verilog design.v\nsynth -top {top}\nabc -g NAND\nopt_clean\nstat\nwrite_verilog -noattr synth.v\n",
        tr!(
            "Yosys: tasarımı sentezle ve NAND kapılarına indir.",
            "Yosys: synthesize the design down to NAND gates."
        ),
        top = export.top
    );
    let makefile = format!(
        "# {}\n\
         .PHONY: sim wave synth clean\n\n\
         sim: ## {}\n\
         \tiverilog -g2012 -o sim.vvp design.v tb.v\n\
         \tvvp sim.vvp\n\n\
         wave: ## {}\n\
         \tiverilog -g2012 -o sim.vvp design.v tb.v\n\
         \tvvp sim.vvp +vcd\n\
         \tgtkwave wave.vcd\n\n\
         synth: ## {}\n\
         \tyosys -q synth.ys\n\n\
         clean:\n\
         \trm -f sim.vvp wave.vcd synth.v\n",
        tr!(
            "make sim: Icarus Verilog ile test et. make synth: Yosys ile sentezle.",
            "make sim: test with Icarus Verilog. make synth: synthesize with Yosys."
        ),
        tr!("testi çalıştır", "run the test"),
        tr!(
            "sinyalleri GTKWave'de göster",
            "show the signals in GTKWave"
        ),
        tr!("NAND kapılarına sentezle", "synthesize to NAND gates"),
    );
    let readme = readme(def, &export, steps.len());
    Ok(Project {
        files: vec![
            ("design.v".into(), design),
            ("tb.v".into(), tb),
            ("synth.ys".into(), synth),
            ("Makefile".into(), makefile),
            ("README.md".into(), readme),
        ],
        notes: notes(&export),
    })
}

fn readme(def: &ChipDef, export: &Export, steps: usize) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# {}\n", def.name);
    let _ = writeln!(
        s,
        "{}\n",
        trf!(
            "makine'de tasarlanan `{}` çipinin Verilog hali: {} modül, en üstteki `{}`.",
            "The `{}` chip designed in makine, as Verilog: {} modules, the top one is `{}`.",
            def.name,
            export.modules,
            export.top
        )
    );
    let _ = writeln!(s, "| {} | |\n|---|---|", tr!("Dosya", "File"));
    for (f, what) in [
        (
            "design.v",
            tr!(
                "çip ve kullandığı bütün çipler",
                "the chip and every chip it uses"
            ),
        ),
        ("tb.v", tr!("test tezgâhı", "testbench")),
        (
            "synth.ys",
            tr!("Yosys sentez betiği", "Yosys synthesis script"),
        ),
    ] {
        let _ = writeln!(s, "| `{f}` | {what} |");
    }
    let _ = writeln!(
        s,
        "\n```sh\nmake sim     # {}\nmake wave    # {}\nmake synth   # {}\n```\n",
        trf!(
            "{} adımı Icarus Verilog'da çalıştırır",
            "runs the {} steps in Icarus Verilog",
            steps
        ),
        tr!(
            "dalga biçimini GTKWave'de açar",
            "opens the waveform in GTKWave"
        ),
        tr!(
            "Yosys ile NAND kapılarına sentezler",
            "synthesizes to NAND gates with Yosys"
        ),
    );
    let _ = writeln!(
        s,
        "{}",
        tr!(
            "Araçlar: `apt install iverilog yosys gtkwave` ya da [OSS CAD Suite](https://github.com/YosysHQ/oss-cad-suite-build). Testin beklediği çıktılar makine'nin kendi simülasyonundan geliyor; `PASS` görürsen Verilog, makine'deki devreyle aynı davranıyor.",
            "Tools: `apt install iverilog yosys gtkwave` or the [OSS CAD Suite](https://github.com/YosysHQ/oss-cad-suite-build). The expected outputs come from makine's own simulation; `PASS` means the Verilog behaves like the circuit in makine."
        )
    );
    s
}

/// Settings for a Tiny Tapeout project.
#[derive(Clone, Debug)]
pub struct TinyTapeout {
    /// GitHub user name, used in the top module's name.
    pub github: String,
    pub author: String,
    pub title: String,
    pub description: String,
    pub clock_hz: u32,
}

impl Default for TinyTapeout {
    fn default() -> Self {
        Self {
            github: "kullanici".into(),
            author: String::new(),
            title: String::new(),
            description: String::new(),
            clock_hz: 10_000_000,
        }
    }
}

/// Where a chip pin goes on the Tiny Tapeout pins.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Wiring {
    Clock,
    /// `reset` is active high; Tiny Tapeout's `rst_n` is active low.
    Reset,
    /// Bits `lo..lo+width` of the 16 general inputs (`ui_in`, then `uio_in`)
    /// or outputs (`uo_out`, then `uio_out`).
    Bits(u32),
}

/// A [Tiny Tapeout](https://tinytapeout.com) project around `def`: the
/// `info.yaml`, the design in `src/` with a `tt_um_*` wrapper, and a
/// cocotb-free Verilog test in `test/`.
///
/// Tiny Tapeout gives a design 8 inputs, 8 outputs and 8 pins that can be
/// either, plus `clk` and an active-low reset. Inputs named `clk` and
/// `reset` use those; the other inputs fill `ui_in` then `uio_in`, outputs
/// fill `uo_out` then `uio_out`. The bidirectional pins are split: the low
/// ones as inputs, the rest as outputs.
pub fn tiny_tapeout(
    lib: &Library,
    def: &ChipDef,
    steps: &[TestStep],
    tt: &TinyTapeout,
) -> Result<Project, String> {
    let opts = Options::default();
    let export = verilog(lib, def, &opts)?;

    let mut wiring_in = Vec::new();
    let mut next_in = 0u32;
    let (mut clock, mut reset) = (false, false);
    for p in &def.inputs {
        if p.width == 1 && is_clock(&p.name) && !clock {
            clock = true;
            wiring_in.push(Wiring::Clock);
        } else if p.width == 1 && is_reset(&p.name) && !reset {
            reset = true;
            wiring_in.push(Wiring::Reset);
        } else {
            wiring_in.push(Wiring::Bits(next_in));
            next_in += p.width as u32;
        }
    }
    let out_bits: u32 = def.outputs.iter().map(|p| p.width as u32).sum();
    let in_uio = next_in.saturating_sub(8);
    let out_uio = out_bits.saturating_sub(8);
    if in_uio + out_uio > 8 {
        return Err(trf!(
            "Tiny Tapeout'ta 24 uç var (8 giriş, 8 çıkış, 8 iki yönlü); bu çip {} giriş ve {} çıkış biti istiyor. Uçları azalt ya da verileri sırayla gönder (örneğin bir adres girişiyle).",
            "Tiny Tapeout has 24 pins (8 in, 8 out, 8 either way); this chip needs {} input and {} output bits. Use fewer pins, or send the data in turns (with an address input, say).",
            next_in,
            out_bits
        ));
    }
    let top = format!(
        "tt_um_{}",
        crate::ident::sanitize(&tt.github.to_ascii_lowercase()).trim_matches('_')
    ) + "_"
        + &crate::ident::sanitize(&def.name.to_ascii_lowercase());

    // The wrapper.
    let mut w = String::new();
    let _ = writeln!(
        w,
        "/*\n * {}\n * SPDX-License-Identifier: Apache-2.0\n */\n\n`default_nettype none\n",
        trf!(
            "makine çipi {} için Tiny Tapeout sarmalayıcısı.",
            "Tiny Tapeout wrapper for the makine chip {}.",
            def.name
        )
    );
    let _ = writeln!(w, "module {top} (");
    let _ = writeln!(w, "    input  wire [7:0] ui_in,");
    let _ = writeln!(w, "    output wire [7:0] uo_out,");
    let _ = writeln!(w, "    input  wire [7:0] uio_in,");
    let _ = writeln!(w, "    output wire [7:0] uio_out,");
    let _ = writeln!(w, "    output wire [7:0] uio_oe,");
    let _ = writeln!(w, "    input  wire       ena,");
    let _ = writeln!(w, "    input  wire       clk,");
    let _ = writeln!(w, "    input  wire       rst_n");
    let _ = writeln!(w, ");");
    let _ = writeln!(w, "    wire [15:0] in_bits = {{uio_in, ui_in}};");
    let _ = writeln!(w, "    wire [15:0] out_bits;");
    let _ = writeln!(w, "    assign uo_out = out_bits[7:0];");
    // uio: the low `in_uio` pins are inputs, the next `out_uio` outputs.
    let oe: u32 = ((1u32 << out_uio) - 1) << in_uio;
    let _ = writeln!(w, "    assign uio_oe = 8'h{oe:02X};");
    let uio_out = if out_uio == 0 {
        "8'h00".to_string()
    } else if in_uio == 0 {
        "out_bits[15:8]".to_string()
    } else {
        format!("{{out_bits[{}:8], {in_uio}'h0}}", 15 - in_uio)
    };
    let _ = writeln!(w, "    assign uio_out = {uio_out};");
    let mut conns = Vec::new();
    let mut pinout_in = vec![String::new(); 16];
    for ((p, n), wr) in def.inputs.iter().zip(&export.inputs).zip(&wiring_in) {
        let src = match wr {
            Wiring::Clock => "clk".to_string(),
            Wiring::Reset => "~rst_n".to_string(),
            Wiring::Bits(lo) => {
                for b in 0..p.width as u32 {
                    pinout_in[(lo + b) as usize] = pin_label(p, b);
                }
                let hi = lo + p.width as u32 - 1;
                if hi == *lo {
                    format!("in_bits[{lo}]")
                } else {
                    format!("in_bits[{hi}:{lo}]")
                }
            }
        };
        conns.push(format!(".{n}({src})"));
    }
    let mut pinout_out = vec![String::new(); 16];
    let mut lo = 0u32;
    for (p, n) in def.outputs.iter().zip(&export.outputs) {
        let hi = lo + p.width as u32 - 1;
        for b in 0..p.width as u32 {
            // Outputs past the 8 dedicated ones sit above the uio inputs.
            let at = if lo + b < 8 { lo + b } else { lo + b + in_uio };
            pinout_out[at as usize] = pin_label(p, b);
        }
        let dst = if hi == lo {
            format!("out_bits[{lo}]")
        } else {
            format!("out_bits[{hi}:{lo}]")
        };
        conns.push(format!(".{n}({dst})"));
        lo = hi + 1;
    }
    if lo < 16 {
        let _ = writeln!(w, "    assign out_bits[15:{lo}] = 0;");
    }
    let _ = writeln!(
        w,
        "\n    {} core (\n        {}\n    );\n",
        export.top,
        conns.join(",\n        ")
    );
    let mut unused = vec!["ena".to_string()];
    if !clock {
        unused.push("clk".into());
    }
    if !reset {
        unused.push("rst_n".into());
    }
    if next_in < 16 {
        unused.push(format!("in_bits[15:{next_in}]"));
    }
    let _ = writeln!(w, "    wire _unused = &{{{}, 1'b0}};", unused.join(", "));
    let _ = writeln!(w, "endmodule");

    // info.yaml.
    let mut y = String::new();
    let _ = writeln!(y, "# Tiny Tapeout project information");
    let _ = writeln!(y, "project:");
    let title = if tt.title.is_empty() {
        def.name.clone()
    } else {
        tt.title.clone()
    };
    let description = if tt.description.is_empty() {
        trf!(
            "makine'de NAND kapılarından kurulan {} çipi",
            "The {} chip, built from NAND gates in makine",
            def.name
        )
    } else {
        tt.description.clone()
    };
    let _ = writeln!(y, "  title:        {}", yaml(&title));
    let _ = writeln!(y, "  author:       {}", yaml(&tt.author));
    let _ = writeln!(y, "  discord:      \"\"");
    let _ = writeln!(y, "  description:  {}", yaml(&description));
    let _ = writeln!(y, "  language:     \"Verilog\"");
    let _ = writeln!(y, "  clock_hz:     {}", if clock { tt.clock_hz } else { 0 });
    let _ = writeln!(y, "\n  tiles: \"1x1\"\n");
    let _ = writeln!(y, "  top_module:  \"{top}\"\n");
    let _ = writeln!(y, "  source_files:");
    let _ = writeln!(y, "    - \"project.v\"");
    let _ = writeln!(y, "    - \"makine.v\"\n");
    let _ = writeln!(y, "pinout:");
    for (i, label) in pinout_in.iter().take(8).enumerate() {
        let _ = writeln!(y, "  ui[{i}]: {}", yaml(label));
    }
    for (i, label) in pinout_out.iter().take(8).enumerate() {
        let _ = writeln!(y, "  uo[{i}]: {}", yaml(label));
    }
    for i in 0..8 {
        let label = if (i as u32) < in_uio {
            &pinout_in[8 + i]
        } else {
            &pinout_out[8 + i]
        };
        let _ = writeln!(y, "  uio[{i}]: {}", yaml(label));
    }
    let _ = writeln!(y, "\nyaml_version: 6");

    // The test drives the wrapper as the chip would be driven on the board.
    let tb = tt_testbench(def, &top, &wiring_in, in_uio, steps);
    let test_make = "# make: Icarus Verilog ile test et / test with Icarus Verilog\n\
                     .PHONY: sim\n\
                     sim:\n\
                     \tiverilog -g2012 -o sim.vvp ../src/makine.v ../src/project.v tb.v\n\
                     \tvvp sim.vvp\n"
        .to_string();

    let mut notes = notes(&export);
    notes.push(tr!(
        "Tiny Tapeout'a göndermek için: github.com/TinyTapeout/ttsky-verilog-template şablonundan bir depo oluştur, bu klasördeki info.yaml, src/ ve test/ dosyalarını içine kopyala ve GitHub'a gönder. GitHub Actions tasarımı sentezleyip yerleşimi çıkarır; çip için bir sonraki shuttle'a başvurursun.",
        "To submit to Tiny Tapeout: create a repository from github.com/TinyTapeout/ttsky-verilog-template, copy info.yaml, src/ and test/ from this folder into it and push. GitHub Actions synthesizes and lays out the design; then apply for the next shuttle."
    ).to_string());

    let readme = format!(
        "# {title}\n\n{}\n\n{}\n\n```sh\ncd test && make\n```\n\n{}\n",
        description,
        trf!(
            "Üst modül `{}`. Giriş/çıkış uçlarının hangi pine gittiği `info.yaml` içinde.",
            "The top module is `{}`. Which pin each input/output uses is listed in `info.yaml`.",
            top
        ),
        notes.join("\n\n")
    );

    Ok(Project {
        files: vec![
            ("info.yaml".into(), y),
            ("src/project.v".into(), w),
            (
                "src/makine.v".into(),
                format!("{}{}", header(def), export.source),
            ),
            ("test/tb.v".into(), tb),
            ("test/Makefile".into(), test_make),
            ("README.md".into(), readme),
        ],
        notes,
    })
}

fn pin_label(p: &makine_sim::Pin, bit: u32) -> String {
    if p.width == 1 {
        p.name.clone()
    } else {
        format!("{}[{bit}]", p.name)
    }
}

fn yaml(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A testbench for the Tiny Tapeout wrapper, setting the chip's inputs
/// through its pins.
fn tt_testbench(
    def: &ChipDef,
    top: &str,
    wiring: &[Wiring],
    in_uio: u32,
    steps: &[TestStep],
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "`timescale 1ns/1ps\nmodule tb;");
    let _ = writeln!(s, "    reg [15:0] in_bits = 0;");
    let _ = writeln!(s, "    reg clk = 0, rst_n = 1;");
    let _ = writeln!(s, "    wire [7:0] uo_out, uio_out, uio_oe;");
    let _ = writeln!(s, "    integer errors = 0;");
    let _ = writeln!(
        s,
        "    {top} dut (.ui_in(in_bits[7:0]), .uo_out(uo_out), .uio_in(in_bits[15:8]), .uio_out(uio_out), .uio_oe(uio_oe), .ena(1'b1), .clk(clk), .rst_n(rst_n));"
    );
    // Outputs back in chip order: uo_out, then the uio outputs.
    let _ = writeln!(
        s,
        "    wire [15:0] out_bits = {{uio_out >> {in_uio}, uo_out}};"
    );
    let _ = writeln!(s, "    initial begin\n        #1;");
    let mut out_lo = vec![];
    let mut lo = 0;
    for p in &def.outputs {
        out_lo.push(lo);
        lo += p.width as u32;
    }
    for (i, step) in steps.iter().enumerate() {
        let mut data = String::new();
        let mut clock = String::new();
        for ((p, wr), &v) in def.inputs.iter().zip(wiring).zip(&step.inputs) {
            let v = v & p.mask();
            match wr {
                Wiring::Clock => clock = format!("clk = {v}; "),
                Wiring::Reset => {
                    let _ = write!(data, "rst_n = {}; ", 1 - v);
                }
                Wiring::Bits(lo) => {
                    let hi = lo + p.width as u32 - 1;
                    let _ = write!(data, "in_bits[{hi}:{lo}] = {}'h{v:X}; ", p.width);
                }
            }
        }
        let _ = write!(s, "        {data}#1; ");
        if !clock.is_empty() {
            let _ = write!(s, "{clock}#1; ");
        }
        for ((p, e), lo) in def.outputs.iter().zip(&step.expect).zip(&out_lo) {
            if let Some(e) = e {
                let hi = lo + p.width as u32 - 1;
                let _ = write!(
                    s,
                    "if (out_bits[{hi}:{lo}] !== {}'h{:X}) begin errors = errors + 1; if (errors <= 20) $display(\"{} {i}: {} = %h\", out_bits[{hi}:{lo}]); end ",
                    p.width,
                    e & p.mask(),
                    tr!("adım", "step"),
                    p.name
                );
            }
        }
        s.push('\n');
    }
    let _ = writeln!(
        s,
        "        if (errors == 0) $display(\"PASS: {} {}\");\n        else $display(\"FAIL: %0d {}\", errors);\n        $finish;\n    end\nendmodule",
        steps.len(),
        tr!("adım", "steps"),
        tr!("hata", "mismatches")
    );
    s
}
