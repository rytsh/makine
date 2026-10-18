<!--
seviyeler: Zero32, Compare32, ShiftLeft32, ShiftRight32, Alu
-->
# ALU: the arithmetic logic unit

The **ALU** is the processor's calculator. It takes two numbers (`a`, `b`) and
an operation code (`op`), and gives the result.

## Is it zero?

OR all 32 bits together: if any of them is 1, the number is not zero. The
inverse of that result answers the question "is it zero?"

Doing `b0 OR b1 OR b2…` one after another makes a chain of 31 gates. Building
it as a **tree** is better: 32 → 16 → 8 → 4 → 2 → 1, only 5 levels.
In the game you can make each level a single part with `Split 16,16` and `Or ×16`.

## Comparison

[Compute](cikarma.md) `a - b`, then:

| question | how |
|---|---|
| `a = b`? | is the difference zero |
| `a < b` (unsigned)? | is the subtraction's **carry 0** |
| `a < b` (signed)? | if the signs differ: the negative one is smaller. If they're the same: the sign of the difference |

Why the carry for the unsigned part? In the sum `a + NOT(b) + 1`, the carry
shows that the result didn't fit in 32 bits. That is exactly the case `a ≥ b`.

## Shifter (barrel shifter)

`a << n`: shift a to the left by n places (like multiplying by 2ⁿ).

Shifting by a fixed amount is just [wiring](mux.md). For a variable amount,
make each bit of n a **stage**:

```
stage 0: if bit 0 of n is 1, shift by 1, otherwise leave as is
stage 1: if bit 1 of n is 1, shift by 2
stage 2: … 4
stage 3: … 8
stage 4: … 16
```

Each stage is a **Mux32**. Every shift from 0 to 31 is a combination of the
5 stages: 13 = 8 + 4 + 1.

**Shifting right** comes in two kinds:

- **logical** (`srl`): 0s come in from the left
- **arithmetic** (`sra`): the **sign bit** comes in from the left, so
  `-8 >> 1 = -4`

## The whole ALU

| op | operation | | op | operation |
|---|---|---|---|---|
| 0 | add `a + b` | | 8 | sub `a - b` |
| 1 | sll `a << b` | | 5 | srl `a >> b` |
| 2 | slt `a < b ? 1 : 0` (signed) | | 13 | sra `a >> b` (signed) |
| 3 | sltu (unsigned) | | 4 | xor |
| 6 | or | | 7 | and |

The low 3 bits of `op` come from the **funct3** field of the RISC-V
instruction. Bit 3 picks between add/sub and srl/sra.

The simplest way: **compute all the results**, then pick the right one with a
MUX tree controlled by op. Real processors mostly do this too: running the
operations in parallel is easier than figuring out ahead of time which one
will be needed.

Next: [RISC-V instructions](riscv.md).
