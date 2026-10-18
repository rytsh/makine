<!--
seviyeler: Decoder, Control, BranchUnit, LoadUnit, StoreUnit, MulDiv
-->
# RISC-V instructions

The language a processor understands is called its **instruction set
architecture** (ISA). The processor in the game speaks **RISC-V** (pronounced
"risk five"), an open standard developed at Berkeley in 2010. Billions of
devices, from phone chips to supercomputers, use the same instruction set.
The processor you build can run code produced by a real C compiler.

## Instruction formats

Every instruction is 32 bits. The bits are split into fields:

```
 31        25 24   20 19   15 14  12 11    7 6      0
┌───────────┬───────┬───────┬──────┬───────┬────────┐
│  funct7   │  rs2  │  rs1  │funct3│  rd   │ opcode │   R-type
└───────────┴───────┴───────┴──────┴───────┴────────┘
```

- **opcode:** the instruction's type (arithmetic, load, jump?)
- **rd:** the register the result is written to
- **rs1, rs2:** the registers to read
- **funct3, funct7:** details within the type (add or subtract?)
- **imm:** a constant number built into the instruction (the immediate value)

An important design decision: **rd, rs1, and rs2 are in the same place in
every type.** That way the decoder can start reading registers before it even
knows what the instruction is.

## Assembling the immediate (imm)

The imm is spread across different places depending on the type, and it is
**sign-extended** to 32 bits: its top bit (always bit 31 of the instruction)
is copied into the empty space.

| type | instructions | imm |
|---|---|---|
| I | `addi`, `lw`, `jalr` | `instr[31:20]` |
| S | `sw`, `sb` | `instr[31:25]` + `instr[11:7]` |
| B | `beq`, `blt` … | shuffled, lowest bit always 0 |
| U | `lui`, `auipc` | `instr[31:12]`, with 12 zeros below |
| J | `jal` | shuffled, lowest bit always 0 |

The "shuffled" layout in the B and J types looks strange at first. The goal is
for each imm bit to come from **the same instruction bit** as often as
possible. That keeps the MUXes in the decoder small.

## Instruction groups

| group | examples | what it does |
|---|---|---|
| arithmetic | `add`, `sub`, `and`, `sll`, `slt` | `rd = rs1 op rs2` |
| arithmetic-imm | `addi`, `andi`, `slli` | `rd = rs1 op imm` |
| load | `lw`, `lh`, `lb`, `lbu` | `rd = memory[rs1 + imm]` |
| store | `sw`, `sh`, `sb` | `memory[rs1 + imm] = rs2` |
| branch | `beq`, `bne`, `blt`, `bge` | if the condition is true, `pc = pc + imm` |
| jump | `jal`, `jalr` | `rd = pc + 4`, then jump (function call) |
| upper | `lui`, `auipc` | large constants, pc-relative addresses |
| multiply | `mul`, `div`, `rem` | M extension |

## Byte, halfword, word

Memory always gives back **an aligned 4-byte word**. For `lb` (load one byte),
the **load unit** shifts and trims the word. With `lb` it sign-extends, with
`lbu` it zero-extends.

Storing is the reverse: the **store unit** shifts the data to the right byte
and produces a **byte mask**. That's how memory is told "write only this byte."

## Multiplication and division

- **Multiplication** works like at school: for each bit of b, shift a and add.
  32 bits × 32 bits = a 64-bit result. `mul` gives the lower half, `mulh` the
  upper half.
- **Division** is long division: at each step, "does the divisor fit into the
  remainder?"

In RISC-V, **dividing by zero is not an error**: the quotient is all 1s and
the remainder is `a`. This rule makes the hardware simpler. A program that
wants to check does the check itself.

## Assembly

The programs on your computer are these instructions in number form. People
write them in **assembly**:

```asm
        li   a0, 0          # a0 = 0
        li   t0, 10         # t0 = 10 (counter)
dongu:  add  a0, a0, t0     # a0 += t0
        addi t0, t0, -1     # t0 -= 1
        bnez t0, dongu      # if t0 is not zero, go back
        ebreak              # stop: a0 = 55
```

Registers have nicknames: `a0`–`a7` for function arguments, `t0`–`t6` for
temporary values, `s0`–`s11` for saved values, `sp` for the stack, and `ra`
for the return address.

Next: [Processor](islemci.md).
