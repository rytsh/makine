<!--
seviyeler:
-->
# Write your own program

To build ROM or small memories, also try [Memory and ROM](rom.md) after
the Memory section: five circuit exercises from address decoding to program
ROM and clocked instruction fetching.
Three Mini code tasks under the program ROM levels unlock independently of
the RISC-V tasks here: edit the ROM program, process RAM inputs and write a
labelled countdown. See the [task and experiment guide](rom.md).

Writing code is not a separate tab; it lives **inside the levels**. In the level list, each
circuit shows the lessons (↳) that use it; for example the RAM level lists the address and
memory lessons. Clicking one opens the same level in code view; **Build / inspect circuit**
takes you back to the circuit. **Build → Code → Try in World** in the circuit's **Learn** section
are parts of the same work and share progress.

The code inspector has three sections: **Task** for the current lesson, explanation,
hints and notes; **Files** for drafts and file actions; **Inspect** for the running
program's values. Only the selected section scrolls; context and task checks stay
visible. Expand detailed explanations when you need them.

**Coding in World:** select a ROM (or a Program ROM for the 4004) in World, then in the right
panel's **Learn and code in this world** section (or **Inspect** in code view), use
**Link selected ROM to this draft**, build
the code and press **Upload ROM and restart**. The program runs on the same live circuit; the
upload can be undone with Ctrl+Z. If capacity or word width does not fit, nothing changes.
RV32 code runs in the computer simulator and is not wired into World.

The code view opens before you finish the CPU level. Write
assembly and step through it without an external compiler. The reference
CPU is used until your CPU is complete; then you can choose the engine.

## Your first program

Open a draft with **Files → New**, rename it, and type:

```asm
li a0, 2
li a1, 3
add a0, a0, a1
ebreak
```

**Build** does not run it. **Load from start** prepares a paused machine.
Use **One source line** and watch the result become 5. `li` loads a number,
`add destination,source1,source2` adds, and `ebreak` stops. `a0` is a
register alias, not a RAM address. Inspect numbers in decimal, hex or
binary; signed and unsigned interpretations differ for the same bits.

Drafts save automatically in your profile, including invalid code. Keep
multiple programs, duplicate them, and open/save `.s` files. Opening an
example does not delete your own draft.

## Source lines and machine instructions

Editing the draft does not change running memory. Build and load again;
a warning appears for old images. Build errors do not damage the previous
program.

A line such as `li a0,0x12345678` may emit two real instructions.
**One instruction** runs one machine instruction; **One source line**
runs instructions emitted by that source line. The listing shows addresses,
hex and real instructions. Double-click a line to set a breakpoint.
**Run to cursor** stops at the first instruction of the editor's line.

**Run slowly** advances about 2.5 instructions a second. Continuous running
pauses at a 100,000-instruction safety budget; you can continue. This limit
does not mean your program is wrong.

## See, predict, change, write it yourself

Choose a lesson on the right. Predict, then step through its example. Try
the suggested change. **Start task** to write your solution. Try a hint
before the example solution; revealing it does not change your draft.

| Lesson | Task |
|---|---|
| Registers and constants | Load a value |
| Copy a value | Preserve the source register |
| Addition | Add two registers |
| Bits | Odd/even test |
| Shifts | Multiplication and 32-bit wrapping |
| Signed decisions | Signed maximum |
| Unsigned decisions | Interpret the same bits differently |
| Loops | Sum 1 through n |
| Addresses vs contents | Read the second array word |
| Memory writes | Preserve neighbouring words |
| Memory | Array sum |
| Bytes and signs | lb vs lbu |
| Text | Byte length of a zero-terminated string |
| Functions | Call and return |
| Nested calls | Save ra on the stack and restore sp |
| Console | Print a character |
| Console input | Echo through Enter |
| Graphics | Paint and present a pixel |
| Pixel addressing | Convert x,y to a byte address |
| Project | Copy bytes in memory |

Twenty tasks unlock in order. You may read ahead and inspect examples;
finish missing prerequisites before checking a task. Existing completed
tasks stay recorded. **Study this lesson in detail** can trace the first
32 example instructions' PC/register/memory changes without touching your
program. Prediction and experiment notes are saved in your profile.

## Understanding a loop

```asm
li a0, 0
li t0, 5
loop: add a0, a0, t0
addi t0, t0, -1
bnez t0, loop
ebreak
```

After the first round a0=5 and t0=4. Branch back if the counter is not
zero. The result is 15. Why is it 55 when starting at 10? What if you
remove the decrement? `bnez` is shorthand for `bne t0,x0,loop`.
Details: [RISC-V](riscv.md).

## Testing and debugging

**Test task** checks behaviour with different starting inputs, not source
text against a solution. Zero, empty arrays and negatives are tested too.
Each case uses fresh memory and fixed time. Programs that do not stop in
20,000 instructions fail safely.

The failing input and expected/actual values are shown. **Step with these
inputs** opens the case on the reference CPU. When comparing -3 with 2
using `bltu`, unsigned -3 is a large number; try `blt` instead.

**Larger projects** includes Fibonacci, sorting, nested calls and stack,
console echo, color patterns and a keyboard-controlled square. For keyboard
input open the output section and click the screen. This machine's device
map is not Linux system calls. Before using the stack, initialize sp to a
valid RAM address aligned to 16 bytes.

## From code to hardware

**From instruction to hardware** explains fields and datapaths. It breaks
down work within one cycle, not additional clock stages. **Wires measured
in my CPU circuit** reads the actual circuit's settled values. In circuit
mode the register table comes from the reference run beside it, not direct
register measurements. It is hidden if divergence is detected.

## Mini processor assembly

Use **+ Mini CPU** for the small instruction set. ROM/RAM have 16 addresses:

```asm
LDI 5
loop: ST 14
SUB 15
JZ done
JMP loop
done: HLT
.ram 15, 1
```

`.ram` is initial data, not a ROM instruction. **Build → Load into circuit
→ Step** to watch acc, pc and RAM. After passing MiniCpu your circuit is
used; otherwise the example circuit is explicitly labelled.
See [Mini processor](mini.md).

## 4004 assembly

Use **+ Intel 4004** and leave hex conversion to the assembler:

```asm
FIM P0, 0
SRC P0
LDM 7
WRM
wait: JUN wait
```

**Load into circuit** opens a separate [4004 workshop](4004.md), keeping
your current world. **Clock cycle** executes one instruction. 4004 has no
HLT; self-jump to wait. WPM is encoded but the workshop hardware does not
implement it and asserts FAULT. The assembler checks JCN/ISZ page rules.

## Your own instruction set

The detailed course is [Design an instruction set from scratch](komutseti.md).
Its ten lessons check answers, encodings, circuits and programs, not a read
checkbox. Each draft keeps its own ISA definition; opening a course draft
does not change your previous design.

Use **+ My instruction set** to choose word width (4–16 bits), opcode
field, instruction names and operand types. Upper bits hold the opcode,
lower bits a value or address; no-operand instructions have zero lower
bits. Labels name ROM word addresses: a 16-bit instruction also occupies
one address. Open/save the ISA definition as JSON; it also lives in your
profile.

This is only encoding. Calling an instruction TOPLA does not add an
addition emulator; build its behaviour from gates in a free chip. Enter
the CPU chip name and **Create blank CPU template** for its pins:

| Input | Output |
|---|---|
| instr[word] | pc[address] |
| rdata[word] | addr[address] |
| reset | wdata[word] |
| clk | we |

pc addresses ROM; addr addresses data RAM. With we=1, a rising edge
writes wdata to RAM. Address width is 1–12 bits; pc and addr must match.
Implement reset yourself: in World turn reset on, give it an edge, turn
it off.

**Build → Load into circuit** connects ROM and RAM to your chip in World.
Incompatible pins and oversized ROMs are rejected. This path is separate
from RISC-V; your custom instructions are not Doom-compatible.
See [World](dunya.md).
