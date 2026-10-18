<!--
seviyeler: AddressDecode2, Rom4x8, Ram4x8, ProgramRom, RomSequencer
-->
# Memory and ROM: how does a program enter a circuit?

**Memory and ROM** is an optional five-level path after the Memory section.
It does not block the main RISC-V path. Revisit [registers and RAM](register.md)
using small byte-wide circuits, then turn a program into instruction bytes.

## Three different memory sizes

**Address width**, **cell count**, and **data width** are different.
`A` address bits select `2^A` cells. With `D` data bits per cell, capacity
is `2^A × D` bits; divide by eight to obtain bytes.

| Memory | Address bits | Data per cell | Total |
|---|---|---|---|
| Rom4x8 / Ram4x8 | 2 | 8 bits | 4 bytes |
| ProgramRom | 4 | 8 bits | 16 bytes |
| Ram8 on the main path | 3 | 32 bits | 32 bytes |

Address 2 selects the **third cell**; it does not mean “read two bits” or
“output the number 2”. An eight-bit value ranges from 0 to 255. Four
address bits do not imply four-bit data cells.

## 1. Address decoder

Two address bits select four cells. `addr` is not data: it identifies the
cell to access. With `en=1`, exactly one output is high:

| addr | s0 | s1 | s2 | s3 |
|---|---|---|---|---|
| 0 (00) | 1 | 0 | 0 | 0 |
| 1 (01) | 0 | 1 | 0 | 0 |
| 2 (10) | 0 | 0 | 1 | 0 |
| 3 (11) | 0 | 0 | 0 | 1 |

With `en=0`, all outputs are zero. In RAM, en is write enable. Broadcasting
it directly to all registers would write every cell, not just the selected one.

**Try:** change addresses with en disabled. No selection output should turn on.

## 2. Build ROM from constants

ROM is read-only memory. Instead of using a ready-made ROM device, connect
four **8-bit Constants** and a selector tree:

| Address | Data (hex) | Data (decimal) |
|---|---|---|
| 0 | 0x12 | 18 |
| 1 | 0xA5 | 165 |
| 2 | 0x3C | 60 |
| 3 | 0x80 | 128 |

The first MUX layer uses the low address bit to choose within two pairs.
The second layer uses the high bit to choose between those results. Set
each MUX's **Repeat** to 8; one selection bit is shared across eight data
bits. **Split** groups start at the low bit: for `1,1`, the first output is
address bit 0.

ROM has no clock or write enable. An address change changes q, not the stored
contents. Programming this ROM means choosing constants and wiring. The level
expects the table above; experiment with other contents in a copy or in World.

## 3. Build small RAM

Replace the four constants with four `Bit×8` registers. Share d and clk;
connect their individual load inputs to the address decoder outputs. A MUX
tree still handles reading. `we=0` does not disable reads.

Capacity is `4 addresses × 8 bits = 32 bits = 4 bytes`. This differs from
`Ram8` in the main Memory path, which holds eight **32-bit words**.

**Try:** write 0xA5 at address 1 and 0x3C at address 2. Keep the clock high
and change data: without another rising edge, cells must not change. Read
all four addresses and check neighbouring cells were preserved.

### Clock level is not a clock edge

Assume cell 1 was previously written with zero. Only 0→1 writes; changing
d or we while the clock is already high is not another write.

| Step | addr | d | we | clk | q | Why? |
|---|---|---|---|---|---|---|
| Prepare | 1 | 165 | 1 | 0 | 0 | No new edge |
| Write | 1 | 165 | 1 | 1 | 165 | Rising edge |
| Change data | 1 | 60 | 1 | 1 | 165 | Clock stayed high |
| Lower clock | 1 | 60 | 0 | 0 | 165 | Falling edge does not write |
| Keep writes disabled | 1 | 60 | 0 | 1 | 165 | Edge, but we=0 |

The level contract does not require initially zero RAM. Write a cell
before relying on its contents. World's ready-made RAM device starts at
zero as a simulation convenience, not a rule for all physical RAM.
Profiles store the circuit; do not treat running RAM contents as persistent ROM.

### A word can occupy several bytes

In byte-addressed memory, the 32-bit number `0x12345678` occupies four
cells. The RISC-V path uses little-endian byte order:

| Byte address | Contents |
|---|---|
| a | 0x78 |
| a+1 | 0x56 |
| a+2 | 0x34 |
| a+3 | 0x12 |

`Ram8` instead selects 32-bit words directly: its `addr+1` means the next
word. Do not confuse a chip's word address with a program's byte address.

## 4. Encode a program in ROM

A program consists of numbers stored in memory. One Mini CPU instruction
is eight bits:

```text
upper 4 bits: opcode | lower 4 bits: argument
```

Place this program in a 16-address ROM:

| Address | Source instruction | Byte | When executed by the CPU |
|---|---|---|---|
| 0 | LDI 3 | 0x13 | acc = 3 |
| 1 | ST 0 | 0x30 | RAM[0] = acc |
| 2 | ADD 0 | 0x40 | acc = acc + RAM[0] |
| 3 | ST 1 | 0x31 | RAM[1] = acc |
| 4 | HLT | 0xF0 | PC holds |
| 5–15 | NOP | 0x00 | No operation |

`ADD 0` adds **the 3 in RAM[0]**, not immediate zero. Executing the program
leaves RAM[0]=3 and RAM[1]=6. ROM alone does not compute this; it only supplies
the addressed byte. Reversing opcode and argument turns 0x13 into 0x31,
encoding a different instruction.

### Instruction memory and data memory are separate

Mini CPU's `ROM[0]` and `RAM[0]` are in separate memories. `ST 0` does not
overwrite the first instruction, 0x13; it writes data RAM. This separate
instruction/data arrangement is a small example of the Harvard approach.
In the RISC-V computer, the loaded program and ordinary data share the
RAM address space. This ROM experiment does not mean every computer must
fetch instructions from a separate ROM.

Stepping the program on a real Mini CPU with initial acc=0 gives:

| Executed instruction | Next PC | acc | RAM effect |
|---|---|---|---|
| LDI 3 | 1 | 3 | None |
| ST 0 | 2 | 3 | RAM[0]=3 |
| ADD 0 | 3 | 6 | None |
| ST 1 | 4 | 6 | RAM[1]=6 |
| HLT | 4 | 6 | None |

The fetch-only circuit has no acc/RAM columns. It only produces PC and
the instruction byte.

## 5. Fetch instructions from ROM

A 4-bit PC register selects the program ROM address. At a rising edge:

1. If reset is set, PC=0.
2. Otherwise, if load is set, PC=d.
3. Otherwise, if run=1 and the current instruction is not HLT, increment PC.
4. Otherwise hold PC.

`instr` always equals `ROM[pc]`. Stopping at HLT does not require gating
the clock: disable register writes. Reset and load override HLT and pause.
Incrementing address 15 wraps to 0.

**Important:** this circuit only fetches instructions. Without an ALU,
accumulator and data RAM it cannot execute ADD/ST. Build those in the
[Mini processor](mini.md) path.

## Where do I write my own program?

This path now has three **Write code:** activities:

| Linked level | Code task | Checks |
|---|---|---|
| Encode a Program in ROM | Load 5 instead of 3; write 5 and 10 | Two initial RAM states, write order, HLT |
| Encode a Program in ROM | Double RAM[0] into RAM[1] | 0,1,7,127,128,255; wrapping and neighbours |
| Fetch Instructions from ROM | Count down from n to 0 | 0,1,5,15,255; every write, zero case, HLT |

Checks do not compare source text with the example. Assembled bytes run on
the example **gate-built MiniCpu** circuit. Each case starts with fresh
test RAM; a 2048-instruction budget catches endless loops. `.ram` can help
manual experiments but cannot replace task inputs. Passing a code task
does not automatically solve its circuit level or MiniCpu. These three
tasks unlock in their own order without waiting for the 20 RISC-V tasks.
Drafts and experiment notes are saved in your profile.

Code lessons appear as **↳** rows beneath circuit levels; you do not need
to finish the CPU first. For example, **1-bit Register** has *Registers and
constants*, while **RAM** has address, write and array tasks. You can also
open them under **Build → Code → Try in World** in the right panel. Tasks
unlock in order and test your solution with different inputs.

At the Mini CPU level, choose **Write code for my Mini CPU** and type the
five instructions above to inspect the assembled bytes. Change `LDI 3` to
`LDI 5` in a draft: the first byte becomes 0x15, and RAM results become 5 and 10.

To upload code in World, select an editable **ROM** device rather than this
constant-built chip and link it to the draft. Four address bits and eight
data bits fit Mini programs. Build, then **Upload ROM and restart**; upload
is undoable. Editing source alone does not change the running ROM.

## Ready-made World experiments

The level's **Try this concept in World** button opens a new world without
changing your existing one. These also appear in Example projects:

- **Read a ROM table:** visit four bytes with 2-bit addr. This uses a ROM
  device; in the level you build the same table from constants and gates.
- **Byte RAM: enable and clock:** set addr/d, enable we, then raise clk
  from 0 to 1. Disable we and change addresses; reads must still work.
- **Program ROM: address, opcode, operand:** change addr and compare
  instr, op and arg. Select the ROM, link it to the draft in code tools,
  build current source and upload. LDI 5 gives 0x15 at address 0.

The last bench has no CPU: you select addresses; **Clock cycle** does not
execute ADD/ST. To execute code, use **Load into circuit → Step** in code
view with the example or your completed MiniCpu.

## Ideas for further experiments

These are not required tasks in the five levels, but starting points for
further work:

- **Expand memory:** design 16×8 RAM using four 4×8 banks. Low address
  bits select a cell; high bits select a bank. Enable writes only in the
  selected bank and use a final MUX layer for reading.
- **Byte mask:** change only one byte of a 32-bit word. Preserve the other
  three bytes and relate this to [StoreUnit](riscv.md).
- **Tables and patterns:** store squares, glyph patterns or traffic-light
  phases in ROM. ROM can hold data, not just programs.
- **Complete the hardware:** connect MiniCpu, ROM and data RAM in another
  project. Verify RAM receives the old accumulator at the clock edge.

Continue: [Programming and tasks](programlama.md),
[Your instruction set](komutseti.md), [Mini processor](mini.md).
