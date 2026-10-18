<!--
seviyeler: Register32, Counter32, Ram8, RegFile
-->
# Registers and RAM

## 32-bit register

A **register** is 32 one-bit registers running on the same clock. In the game
it's **Bit ×32**, a single part. The processor keeps numbers in registers
while it computes.

## Program counter

The processor keeps asking "where is the next instruction?" The **program
counter** (`pc`) holds the answer: a register with a bit of logic in front of
it. On every clock tick:

| condition | pc |
|---|---|
| `reset` | 0 (start from the beginning) |
| `load` | d (**jump** somewhere) |
| `inc` | pc + 4 (next instruction) |
| none | stay the same |

Why +4? In RISC-V every instruction is 32 bits, which is **4 bytes**. In
memory, addresses count byte by byte.

> [!TIP]
> Build the priority order with MUXes: first a MUX for `inc`, then feed its
> output into a MUX for `load`, and finally into a MUX for `reset`. The last
> MUX is the strongest.

## RAM

**RAM** (random access memory) is made of many registers plus an **address**
that picks which one to read or write:

- **Read:** address → MUX tree → that word comes out. It doesn't wait for the
  clock.
- **Write:** address → DEMUX tree → only that register's `load` becomes 1.
  It's written on the clock edge.

For an 8-word RAM, a 3-bit address is enough: 2³ = 8. The 16 MB of RAM in your
computer is the same idea grown to 4 million words. Simulating that with NANDs
would mean hundreds of millions of gates. That's why that part is simulated in
Rust in the game.

## Register file

A RISC-V processor has **32 registers**: `x0` … `x31`. An instruction **reads
two of them** and **writes one** at the same time:

```
add x5, x6, x7     →   x5 = x6 + x7
```

So the register file has:

- two read inputs (`rs1`, `rs2`) and two outputs (`a`, `b`); two separate MUX
  trees,
- a write address (`rd`), data (`d`), and enable (`we`) input; one DEMUX tree.

**x0 is always 0.** Writing to it does nothing. This little rule is very
useful: `x0` becomes the "zero constant," and it's also used for "throw the
result away." In the circuit you don't add a register for x0; you connect a
**Constant 0** to input 0 of the MUX.

Next: [ALU](alu.md).
