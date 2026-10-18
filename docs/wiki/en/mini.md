<!--
seviyeler: AddSub8, MiniControl, MiniCpu
-->
# The mini processor: your first computer

You finished the memory section: you have registers, a counter and RAM.
Before moving on to the RISC-V processor, you can build a **small but
real** processor from the parts you have. This section is optional; the
ALU section does not wait for it.

## An accumulator machine

The mini processor has a single register for arithmetic: **acc** (the
accumulator, 8 bits). Every operation works on acc, and the result goes
back into acc. The first microprocessors of the 1970s (Intel 4004, 8008,
MOS 6502) worked like this.

There is also **pc** (the program counter, 4 bits): the address of the
next instruction. The program sits in a 16-byte ROM and the data in a
16-byte RAM; both are outside the chip.

## Instructions

Each instruction is one byte: the top 4 bits are **op** (what to do), the
low 4 bits are **arg** (a number or a RAM address).

| op | instruction | does |
|---|---|---|
| 1 | `LDI n` | acc = n |
| 2 | `LD a` | acc = RAM[a] |
| 3 | `ST a` | RAM[a] = acc |
| 4 | `ADD a` | acc = acc + RAM[a] |
| 5 | `SUB a` | acc = acc − RAM[a] |
| 6 | `JMP a` | pc = a |
| 7 | `JZ a` | if acc = 0, pc = a |
| 15 | `HLT` | stop (pc stays put) |

Other ops do nothing (`NOP`). Without a jump, pc goes up by one.

## A program

This program multiplies RAM[0] by RAM[1] and writes the result to RAM[2],
by adding again and again (RAM[3] = 1):

```
0: LD 2     acc = result
1: ADD 0    acc += a
2: ST 2     result = acc
3: LD 1     acc = b
4: SUB 3    acc -= 1
5: ST 1     b = acc
6: JZ 8     done if b = 0
7: JMP 0    otherwise again
8: LD 2
9: HLT
```

This machine has no multiply instruction, but with loops and a
conditional jump it can compute anything. That idea is the heart of what
Alan Turing showed in 1936.

## The parts

1. **Add / Subtract:** one adder both adds and subtracts:
   a − b = a + (NOT b) + 1. See the [subtraction](cikarma.md) page.
2. **Mini Control:** the circuit that looks at the instruction's op and
   tells each part what to do. It is only AND, OR and NOT.
3. **Mini Processor:** two registers (acc and pc), selectors in front of
   them, the control and Add/Subtract.

The test runs three programs on your processor and compares acc, pc and
what is written to RAM with a reference at every step. If something
differs, it tells you at which instruction.

Then, in the [ALU](alu.md) section, you will grow the same ideas to 32 bits
and RISC-V's more than 40 instructions.
