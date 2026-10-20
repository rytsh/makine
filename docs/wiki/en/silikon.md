<!--
seviyeler:
-->
# A real chip: from makine to silicon

Every chip you build in makine can become real hardware. First it has to
be turned into **Verilog**, the language the chip industry uses to describe
circuits. makine does that translation for you.

## How is a chip designed?

Roughly, a real chip is designed in six steps:

1. **Architecture:** decide what the chip does: instruction set, bit width,
   memory layout.
2. **RTL:** write the circuit in a language like Verilog or VHDL. In makine,
   this is building the chip from gates and chips.
3. **Verification:** simulate the circuit against test vectors. makine's
   **Test** button does this.
4. **Synthesis:** map the circuit onto the gates (cells) in the foundry's
   library, with a tool like Yosys.
5. **Place and route:** put the cells on silicon, draw the wires between
   them, and analyse timing (OpenROAD).
6. **Tape-out:** the resulting GDS file goes to the foundry.

The first three steps happen in makine. For the rest, makine prepares the
files you need.

## A Verilog project

In the panel of a level or a free chip, press **🔌 Take it to real hardware →
Save Verilog project**. These files go into the folder you choose:

| File | Contents |
|---|---|
| `design.v` | Your chip and every chip it uses, one module each. |
| `tb.v` | A testbench: the level's own test, or makine's simulation. |
| `synth.ys` | A Yosys script that synthesizes the design to NAND gates. |
| `Makefile` | `make sim`, `make wave`, `make synth` |

The tools are free: `apt install iverilog yosys gtkwave`, or the
[OSS CAD Suite](https://github.com/YosysHQ/oss-cad-suite-build). If
`make sim` in the folder prints `PASS`, the Verilog behaves exactly like
your circuit in makine.

Nothing is simplified. Every NAND gate becomes `~(a & b)`, and your chips
stay nested modules, as they are in makine.

## Memory: latches and flip-flops

A latch built from NAND gates is a **feedback loop**. Simulators can handle
that; FPGA and chip tools cannot. They expect memory to be built from
ready-made flip-flop cells.

So when exporting, makine writes the `Latch`, `DLatch` and `DFF` chips as
real latches and flip-flops instead of gate loops. Everything else stays
your gates. Your chip passed the same test, so the circuit around it
behaves the same.

**Devices** such as RAM, ROM and the screen are written in Rust, not built
from gates, so they cannot become Verilog. You can build memory from
registers yourself; the `Ram8` level shows how.

## Running on an FPGA

An FPGA is a chip whose circuit you can reprogram. Trying your design on an
FPGA is much cheaper and faster than having a chip made. iCE40-based boards
(iCEBreaker, Tang Nano and others) work with open-source tools:

```sh
yosys -p "synth_ice40 -top mk_MiniCpu -json mini.json" design.v
nextpnr-ice40 --up5k --json mini.json --pcf board.pcf --asc mini.asc
icepack mini.asc mini.bin && iceprog mini.bin
```

Your board's `.pcf` file says which FPGA pin goes to which pin of your chip.

For a sense of size, the example solutions on an iCE40: the mini processor
(`MiniCpu`) uses **70 LUTs and 12 flip-flops** and fits any board. The RISC-V
processor (`Cpu`) uses **about 8800 LUTs and 1024 flip-flops**, all of the
flip-flops in the register file (32 × 32 bits). That does not fit an iCE40
UP5K (5280 LUTs); it needs a bigger FPGA such as an ECP5 (`synth_ecp5` and
`nextpnr-ecp5`).

## The FPGA computer: really run your RISC-V

Once you pass the Processor level (`Cpu`), its panel shows **🖥 Save FPGA
computer**. It builds a small computer around your processor, what
electronics calls an **SoC** (System on Chip):

- **64 KB of RAM:** the FPGA's block memory, with the program already in it.
- **A serial port (UART, 115200 baud):** everything you `printf` comes to
  your computer over the board's USB cable.
- **A timer:** it counts milliseconds, as in makine.
- **A sequencer:** it drives the processor step by step.

The board is the **ULX3S** (Lattice ECP5-85F, fully programmable with
open-source tools). The folder has three commands:

```sh
make sim     # try it on the computer first (Verilator): the output appears
make         # build the FPGA bitstream (makine.bit), takes a few minutes
make prog    # load it onto the board
```

When compiling a C program for the FPGA, give the RAM size:

```sh
MK_RAM=64K sdk/build.sh hello.c
```

**Why a sequencer?** In makine's computer, memory answers instantly. An
FPGA's block memory answers one clock later. And because your processor is
built from NAND gates, its longest path (the divider) takes about 250 ns.
On every instruction the sequencer reads the instruction, waits for the
gates to settle, reads the data, waits again, and then ticks the
processor's clock once. At 12.5 MHz that makes about 650 thousand
instructions a second. The design was placed and routed for the FPGA with
makine's own example processor and passes timing analysis; it uses 13% of
the board.

The way to go faster is what real processors do: split the work into stages
(a pipeline) and speed memory up with a cache. Those are on the **A fast
processor** page.

## Tiny Tapeout: your own chip

[Tiny Tapeout](https://tinytapeout.com) combines many people's small designs
on one chip and has it made, so a real chip costs a few hundred dollars. Each
design gets about 167×108 µm, 8 inputs, 8 outputs and 8 pins that can go
either way, plus a clock (`clk`) and a reset.

**Save Tiny Tapeout project** prepares:

- `info.yaml`: the project's details and what each pin does.
- `src/project.v`: the `tt_um_...` module Tiny Tapeout wants, wiring your
  chip to the pins.
- `src/makine.v`: your chip itself.
- `test/`: a test that runs through the pins.

The pins are assigned like this:

- An input named `clk` goes to Tiny Tapeout's clock.
- An input named `reset` goes to the reset pin. Tiny Tapeout's reset is
  active low (0 means reset); makine converts it for you.
- The other inputs fill `ui_in` in order, and the rest go to `uio`.
- Outputs fill `uo_out` in order, and the rest go to `uio`.

There are 24 pins in total. A register, a counter or an 8-bit ALU fits
(`MiniAcc8`, for instance). The mini processor (`MiniCpu`) needs 16 input
and 17 output bits, so it does not fit as it is. To fit a big design, send
the data in turns: make a free chip, put the processor inside, and let a
select input choose which outputs go out. If you also build the program ROM
from gates inside the chip, the `instr` input is no longer needed.

To submit, create a GitHub repository from the
[ttsky-verilog-template](https://github.com/TinyTapeout/ttsky-verilog-template)
template, copy `info.yaml`, `src/` and `test/` from the folder into it, and
push. GitHub Actions synthesizes the design, lays it out and draws a picture
of the chip. If everything is green, you can apply for the next shuttle.

A word on area: one Tiny Tapeout tile fits a few thousand gates. If your
design is bigger, raise `tiles` in `info.yaml`. `make synth` shows how many
cells it uses.
