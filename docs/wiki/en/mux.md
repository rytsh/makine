<!--
seviyeler: Mux, Demux, Mux32
-->
# Selector (MUX) and buses

## MUX: "which one goes through?"

A **selector** (multiplexer, MUX) passes one of two inputs to its output.
The **sel** (select) input decides which one goes through:

| sel | output |
|---|---|
| 0 | a |
| 1 | b |

There are MUXes everywhere in a computer. For example:

- Should a register go to the ALU, or a constant number?
- Should the register get the addition result, or the value read from memory?
- Should the next instruction come from `pc + 4`, or from a jump target?

### How do you build one?

```
out = (a AND NOT sel)  OR  (b AND sel)
```

When `sel = 0`, the AND on the right is 0 and the one on the left lets a
through. When `sel = 1`, it's the other way around. One NOT, two ANDs, and one
OR are enough.

## DEMUX: the reverse

A **distributor** (demultiplexer) sends one input to one of two outputs:

| sel | a | b |
|---|---|---|
| 0 | input | 0 |
| 1 | 0 | input |

In the register file, a tree made of DEMUXes answers the question "which
register gets written?"

## Buses: many wires together

A number is 32 bits; drawing 32 separate wires would be a pain. In the game, a
**bus** (e.g. `a[32]`) is 32 wires drawn as one thick blue wire.

### Repeat (×N)

Right-click a part and set its **Repeat** count to 32, and 32 copies of that
part work side by side. Each of its pins becomes 32 bits wide too. That's how
a 32-bit MUX is built with **a single part**: `Mux ×32`.

If you connect a 1-bit wire like `sel` to a wide input, that bit is sent to
**all the copies**. So all 32 MUXes see the same `sel`. Exactly what we want.

### Split and Join

- **Split** breaks a bus into pieces: `Split 8,8,8,8` splits 32 bits into four
  bytes. Bit 0 is the lowest digit.
- **Join** does the reverse.

These are not gates; they just rearrange wires, and they cost nothing in the
circuit. In real chips, too, "taking a wire somewhere else" is free.

> [!TIP]
> Shifting by a fixed amount (e.g. "shift left by 1") is also just wiring:
> `Split 31,1`, then a **Constant 0** on the lowest bit of `Join 1,31`.
> The shifter on the [ALU](alu.md) page uses exactly this idea.

Next: [Addition](toplama.md).
