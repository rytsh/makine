<!--
seviyeler:
-->
# Design an instruction set from scratch

Choosing instruction names is not building a CPU. This chapter connects
three parts: **the ISA contract**, **the assembler**, and **a gate-built
circuit**. Programming → **+ My instruction set** contains ten design
lessons. You can read ahead; completion checks unlock in order. Notes,
completed lessons and each draft's ISA definition are saved in the profile.

## 1. Decide what the programmer can see

For this course we choose a small, testable machine:

- **acc:** an 8-bit arithmetic register, unsigned range 0–255.
- **pc:** a 4-bit instruction address, selecting 16 ROM words.
- **RAM:** 16 cells of 8 bits each.
- **Instruction:** 8 bits, high 4 for opcode, low 4 for operand.
- **Clock:** one instruction at each rising edge.
- **Overflow:** arithmetic wraps modulo 256, PC modulo 16.
- **Reset:** a rising edge sets acc=0 and pc=0.

These decisions say nothing about silicon layout or NAND count. Different
circuits can implement the same ISA, but their programmer-visible results
must agree. This course uses a fixed contract; free designs may choose
other encodings, provided you build matching hardware.

## 2. Calculate the bit budget

```text
bit:       7 6 5 4   3 2 1 0
field:     opcode    operand
ADD 7:     0 1 0 0   0 1 1 1  = 0x47
```

Four opcode bits permit at most 16 operations; four operand bits permit
16 values/addresses. Although acc is 8 bits, **LDI carries only 0–15**.
LDI 255 cannot fit into one byte: use a longer instruction, a second word,
or extra instructions. The assembler cannot magically fix this capacity
problem; it is an ISA decision.

## 3. Specify every instruction's effects

| Opcode | Name | Operand | Visible effect |
|---|---|---|---|
| 0 | NOP | none | preserve state, increment pc |
| 1 | LDI | value | acc=operand |
| 2 | LD | RAM address | acc=RAM[operand] |
| 3 | ST | RAM address | RAM[operand]=acc, preserve acc |
| 4 | ADD | RAM address | acc=(acc+RAM[operand]) mod 256 |
| 5 | SUB | RAM address | acc=(acc-RAM[operand]) mod 256 |
| 6 | JMP | ROM address | pc=operand |
| 7 | JZ | ROM address | if acc=0 pc=operand, otherwise pc+1 |
| 15 | HLT | none | preserve pc, acc and RAM |

Other opcodes act like NOP. This is a **deliberate contract**; another ISA
may trap. The assembler rejects an undefined name; use raw `.word 0x80`
to exercise an undefined opcode in the circuit.

**LDI 3** and **LD 3** differ: with RAM[3]=9 the first puts 3 in acc,
the second 9. This distinction is the first design lesson's check.

## 4. Turn names into bits

```asm
LDI 3
ST 0
JMP done
LDI 9
done: HLT
```

Pass one places labels: done=4. Labels occupy no space. Pass two combines
opcodes and operands: `13 30 64 19 F0`. Addresses here are **ROM words**;
RISC-V byte addresses and pc+4 do not apply. A 16-bit instruction in a
custom design also occupies one ROM word.

## 5. Build the field-splitting circuit

In **MiniDecode**, build `instr[8] → op[4],arg[4]`. One Split 4,4 suffices.
The first Split output contains low bits: the operand. The second contains
the opcode. Try `0x0F` and `0xF0` to catch reversed wiring. This circuit
does not perform arithmetic or access memory yet.

## 6. Derive the control table

The datapath may compute continuously; enable signals decide which
results are stored. Control outputs are ordered as follows:

| Instruction | load | imm | alu | sub | we | jump | hold |
|---|---|---|---|---|---|---|---|
| NOP | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| LDI | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| LD | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ST | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| ADD | 1 | 0 | 1 | 0 | 0 | 0 | 0 |
| SUB | 1 | 0 | 1 | 1 | 0 | 0 | 0 |
| JMP | 0 | 0 | 0 | 0 | 0 | 1 | 0 |
| JZ | 0 | 0 | 0 | 0 | 0 | zero | 0 |
| HLT | 0 | 0 | 0 | 0 | 0 | 0 | 1 |

`load` writes acc; `imm` selects immediate data; `alu` selects the ALU
result; `sub` requests subtraction; `we` writes RAM; `jump` loads PC;
`hold` keeps PC unchanged. Keep the order straight: ADD=1010000, ST=0000100.

## 7. Turn the table into gates

An opcode equality is the AND of the required bits:

```text
is_SUB = op[0] AND NOT op[1] AND op[2] AND NOT op[3]
load = is_LDI OR is_LD OR is_ADD OR is_SUB
alu = is_ADD OR is_SUB
jump = is_JMP OR (is_JZ AND zero)
```

Build these functions in **MiniControl**. All signals must be zero for
undefined opcodes. Writing RAM on HLT or acc on ST is an unwanted side
effect. Tests check these too, not just the result of ADD.

## 8. Learn state-holding blocks separately

The Mini CPU path now has seven circuit levels:

1. **AddSub8:** add and subtract with the same adder.
2. **MiniDecode:** opcode and operand wires.
3. **MiniZero8:** zero detection over every bit.
4. **MiniPc4:** reset > hold > jump > pc+1 priority.
5. **MiniAcc8:** reset > load > keep priority.
6. **MiniControl:** the control table.
7. **MiniCpu:** integration and program execution.

Combinational outputs change with inputs. A register writes only at the
clock edge. **Changing d again while the clock stays high must not write
again.** MiniPc4/MiniAcc8 tests explicitly exercise this. Zero detection
cannot inspect only the sign bit: 00000001 is nonzero too.

## 9. Wire the datapath and trace a cycle

```text
ROM[pc] → MiniDecode → MiniControl
            operand → RAM address / jump target
acc → AddSub8 ← rdata
       ALU or rdata → MUX → immediate or previous result → acc.d
acc → MiniZero8 → zero → MiniControl
MiniControl.jump / hold + operand → MiniPc4
```

ROM and data RAM remain outside the CPU. Data and write permissions
computed before the edge transfer into registers/memory at the edge.
This description does not split an instruction into extra clock stages.

| Instruction | Before pc,acc | Effect at edge | After pc,acc |
|---|---|---|---|
| LDI 3 | 0,0 | write 3 into acc | 1,3 |
| ST 0 | 1,3 | RAM[0]=3; no acc write | 2,3 |
| ADD 0 | 2,3 | acc=3+RAM[0] | 3,6 |
| ST 1 | 3,6 | RAM[1]=6 | 4,6 |
| HLT | 4,6 | no writes; keep PC | 4,6 |

## 10. Final check on your own chip

Pass MiniCpu using your circuit. Enter a free CPU chip name and use
**Make a free CPU from my MiniCpu solution**. It copies your saved solution,
not a supplied reference CPU; only output acc is renamed wdata. Existing
chips are never overwritten.

Final contract: inputs `instr[8],rdata[8],reset,clk`; outputs
`pc[4],addr[4],wdata[8],we`, in that order. **Run lesson check** executes
four independent programs on the actual chip. Every step compares PC,
acc, address and RAM writes against a reference; it also checks no second
write occurs while the clock remains high, HLT preserves state, and reset
clears previous state. A missing or incorrect chip never passes by falling
back to a reference implementation.

After passing, **Build → Load into circuit** opens this CPU with ROM and
RAM in a separate [World](dunya.md) project. Change control hardware too
when you choose new opcodes in free design. An opcode table cannot add
numbers on its own.

## Experiments and debugging

- Change ADD's opcode without changing hardware: which program fails?
- Invert JZ's zero input: where does countdown go wrong?
- Enable load during ST: which previous acc value is lost?
- Reverse RAM address bit order: why do some addresses still look correct?
- Put a nonzero value in acc before reset: what should it be afterwards?
- Choose a 3-bit opcode field in your next design: how many operations fit,
  and which one must you drop? This is a different ISA from the course.

Return to [Programming](programlama.md) for basics, [Mini processor](mini.md)
for the small machine, or [RISC-V](riscv.md) to compare with a real standard.
