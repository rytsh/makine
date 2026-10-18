<!--
seviyeler: Cpu
-->
# Processor

All the parts are ready. Now we put them together into a **single-cycle**
processor. On each clock tick, one instruction runs from start to finish.

![Data path of a single-cycle RISC-V processor](img/islemci.png)

## An instruction's journey

Let's follow the instruction `add x5, x6, x7`:

1. **Fetch.** The `pc` register goes to memory as an address. The 32 bits
   that come back land on the `instr` input.
2. **Decode.** The [instruction decoder](riscv.md) separates the fields:
   opcode = arithmetic, rd = 5, rs1 = 6, rs2 = 7.
3. **Read.** The [register file](register.md) puts x6 and x7 on its `a` and
   `b` outputs. It doesn't wait for the clock.
4. **Compute.** The [ALU](alu.md) computes `a + b`. The control unit looks at
   the opcode and tells the ALU "add."
5. **Write back.** The result goes to the register file's `d` input. The
   control unit says `we = 1`. When the clock **rises**, it's written to x5.
6. **Advance.** On the same edge, `pc` becomes `pc + 4`.

Until the clock rises, steps 1–4 are just signals passing through gates.
They all "settle into place" at the same time. On the edge, two things
happen: the register and the pc are updated. Then the same thing starts over
for the next instruction.

## Control unit

The processor's boss. It looks at the opcode and tells every MUX and every
enable signal what to do:

| instruction | ALU b | ALU op | written back | memory | pc |
|---|---|---|---|---|---|
| `add` | rs2 | funct | ALU | — | +4 |
| `addi` | imm | funct | ALU | — | +4 |
| `lw` | imm | add | **memory** | read | +4 |
| `sw` | imm | add | — | **write** | +4 |
| `beq` | rs2 | — | — | — | **conditional** |
| `jal` | — | — | pc + 4 | — | **pc + imm** |
| `lui` | — | — | **imm** | — | +4 |

Signals not in the table are called "don't care": for that instruction, their
value doesn't matter. The game's test doesn't check them either.

## How is it tested?

The processor test runs real programs (arithmetic, branches, memory, function
calls, multiplication/division) **on two processors side by side**: yours, and
a reference processor written in Rust. After every instruction, the pcs and
the memory writes are compared.

When it finds a difference, the test tells you which instruction was wrong and
traces **backward** to where the wrong value came from:

> The memory write in instruction `0x0098: 'sw t0, 52(s0)'` is wrong… x5 (t0)
> should have been 0xFEF56DF7. This value was last written by
> `0x0094: 'sra t0, a4, t1'`.

Here the problem isn't in `sw`, it's in the `sra` before it: the ALU is doing
the arithmetic shift wrong.

## The cost of a single cycle

In a single-cycle processor, the clock has to wait for **the slowest
instruction** to finish. That's usually `lw`, which reads from memory and then
adds. Real processors use a **pipeline** instead. They split an instruction
into 5 stages, and a different instruction works in each stage. Like a factory
assembly line: each instruction still takes 5 steps, but one instruction
**finishes** at every step.

Next: [The computer and programs](bilgisayar.md).
