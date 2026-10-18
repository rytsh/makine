<!--
seviyeler:
-->
# Getting started

**makine** is a game where you build every part **yourself**: you start from a
single logic gate (NAND) and end with a computer that runs real programs.
This wiki explains the idea behind each part you build.

![A silicon wafer: each square is a processor](img/foto-wafer.jpg)

*A silicon wafer. Each small square is a chip made of billions of
transistors. You will build the same idea, step by step, at a much smaller
scale.*

## Why can a computer be built at all?

There is no magic inside a computer. There are only **two things**:

1. **Switches.** Tiny switches that turn on and off with electricity:
   [transistors](transistor.md).
2. **Clever ways to connect them.**

Connect switches in certain ways and you get **logic gates**. Connect gates
and you get **circuits that add**. Circuits that feed their own output back
start to **remember**. Run circuits that remember and compute step by step
with a **clock**, and you get a **processor**.

Each step builds on the one before. The levels in the game follow exactly
this order.

## How to play

- Drag a part from the **Parts** list on the left onto the canvas.
- Drag from a part's **output pin** (the dot on the right) to an **input pin**
  to draw a wire.
- **Click** the input boxes on the left to switch them between 0 and 1. Wires
  light up live: **green = 1**, gray = 0, blue = multi-bit bus.
- To cut a wire, **right-click it → Cut wire**, or hover over it and press
  **Delete**. Grabbing the end plugged into an input and dragging it into
  empty space also cuts it; drop it on another input and the wire moves there.
- You can also drag the input/output boxes to wherever you like.
- Press **Test**. If you pass, your chip unlocks as a part in the next level.

> [!TIP]
> When you get stuck, read the hint in the level description, then look at the
> related page in this wiki. Every level has a **📖 Read in the wiki** link
> at the bottom.

## Reading order

| Page | What you'll learn |
|---|---|
| [Binary numbers](ikilik.md) | Why a computer uses only 0 and 1 |
| [Transistor](transistor.md) | A switch that runs on electricity |
| [NAND gate](nand.md) | A gate from 4 transistors |
| [Logic gates](kapilar.md) | NOT, AND, OR, XOR, and how to make all of them from NAND |
| [Selectors and buses](mux.md) | Routing data, moving 32 wires at once |
| [Addition](toplama.md) | Half and full adders, how the carry ripples |
| [Negative numbers and subtraction](cikarma.md) | Two's complement |
| [Memory](hafiza.md) | Latch, flip-flop, clock |
| [Registers and RAM](register.md) | Turning bits into words, and words into memory |
| [ALU](alu.md) | The processor's calculator |
| [RISC-V instructions](riscv.md) | The language the processor speaks |
| [Processor](islemci.md) | Putting it all together |
| [The computer and programs](bilgisayar.md) | Screen, keyboard, C programs, Doom |
