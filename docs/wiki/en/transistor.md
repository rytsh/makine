<!--
seviyeler: Not
-->
# Transistor

Everything in a computer is made of billions of copies of the same tiny part:
the **transistor**. You can think of a transistor as a **switch** that turns
on and off with electricity.

![The surface of a processor chip, under a microscope](img/foto-die.jpg)

*The surface of a processor chip. Each colored block is a circuit made of
thousands of transistors: compute units, memories, and the copper wires that
connect them.*

## Like a switch

You flip a normal light switch with your finger. In a transistor, the "finger"
is another wire. This wire is called the **gate**.

![nMOS and pMOS transistors](img/transistor.png)

Every chip today has two kinds of transistors:

| Kind | Gate = 1 | Gate = 0 |
|---|---|---|
| **nMOS** | conducts (switch closed) | doesn't conduct (switch open) |
| **pMOS** | doesn't conduct | conducts |

pMOS works the exact opposite way of nMOS. The small circle in its symbol
means "works the opposite way." You'll see the same small circle at the output
of the [NAND gate](nand.md).

## Why two kinds?

nMOS is good at connecting a wire to **0 (ground)**. pMOS is good at
connecting it to **1 (+V)**. A design that uses both together is called
**CMOS** (*Complementary MOS*). In CMOS:

- An output is pulled either to 1 by the pMOS transistors or to 0 by the nMOS
  transistors. **Never both** at once.
- So the circuit only uses current while its state is changing, and almost
  none while it waits. That's how your phone's battery lasts a whole day.

## How small?

| Year | Processor | Transistor count |
|---|---|---|
| 1971 | Intel 4004 | 2,300 |
| 1985 | Intel 386 | 275,000 |
| 2000 | Pentium 4 | 42 million |
| 2020s | Phone processors | 15–20 billion |

The RISC-V processor you'll build in the game comes to about **140,000 NAND
gates**. Since each NAND is 4 transistors, that's about **560,000
transistors**: as much as a processor from the late 1980s!

## In the game

The game doesn't go down to the transistor level. The starting part is a
ready-made **NAND** gate built from 4 transistors. You'll see how it's built
on the next page: [NAND gate](nand.md).
