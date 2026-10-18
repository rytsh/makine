<!--
seviyeler: Not, And
-->
# The NAND gate

**NAND** means "NOT AND": **if both inputs are 1, the output is 0; otherwise
it is 1**. Everything in the game is built from this one gate.

| a | b | NAND |
|---|---|---|
| 0 | 0 | **1** |
| 0 | 1 | **1** |
| 1 | 0 | **1** |
| 1 | 1 | 0 |

## A NAND from 4 transistors

In a real chip, a NAND is made of two [pMOS and two nMOS transistors](transistor.md):

![CMOS NAND gate](img/cmos-nand.png)

Let's follow what happens for each input:

- **a = 1, b = 1:** Both nMOS transistors at the bottom conduct. They connect
  the output to ground, which is **0**. The pMOS transistors at the top are off.
- **a or b = 0:** The series path at the bottom is broken, because one of the
  nMOS transistors doesn't conduct. At least one of the pMOS transistors at the
  top conducts and pulls the output up to **1**.

Notice: the output is never connected to both 1 and 0. The **series** setup at
the bottom (both needed) is the exact mirror of the **parallel** setup at the
top (one is enough). That's the basic rule of CMOS.

## Why NAND?

NAND is a **universal** gate: **every** logic circuit can be made from NANDs
alone. That includes AND, OR, XOR, adders, memory, and an entire processor.

It's also one of the cheapest gates in CMOS: 4 transistors. To make an AND you
need to add a NOT after the NAND, which makes 6 transistors.

That's why the game gives you **only NAND**. You'll build the rest.

## First task: NOT

What happens if you give **the same signal to both inputs** of a NAND?

| a | NAND(a, a) |
|---|---|
| 0 | NAND(0, 0) = **1** |
| 1 | NAND(1, 1) = **0** |

That's exactly a **NOT** gate! This is the answer to the first level in the
game.

> [!TIP]
> You can connect one output to more than one input. Drag a wire, drop it on
> an input; then draw another wire from the same output.

Next: [Other logic gates](kapilar.md).
