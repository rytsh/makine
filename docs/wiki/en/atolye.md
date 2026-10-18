<!--
seviyeler: Mux4, Adder4, Register4, Counter12
-->
# Processor workshop: big ideas with small buses

On the RISC-V path you built 32-bit parts. In a 4-bit processor you can see
the same ideas with smaller numbers. The four exercises in this section let
you think about the basic parts of the [4004 workshop](4004.md) one at a time.
You don't need to pass these levels to open the example project.

## 4-bit selector

A MUX is the circuit for the question "the computed value, or the old value?"
When `sel=0` the output is `a`; when `sel=1` it is `b`. Put four MUXes side by
side and you select a 4-bit bus; they all read the same `sel` bit.

**Try it:** let a=3 and b=12. When sel changes, the output should switch
between 3 and 12. If you wire the bits in reverse order and see 12 instead of
3, that's no accident: `0011` reversed is `1100`.

## 4-bit adder and carry

A full adder takes three bits: a, b, and the carry from the previous digit.
Chain four of them and the carry moves from the low bit to the high bit.

```
bit 0 → carry → bit 1 → carry → bit 2 → carry → bit 3 → carry
```

`15 + 1 = 16` means a four-bit `sum=0` and `carry=1`. If you throw away the
carry, you throw away part of the result. For `15+15+1` you should get
`sum=15, carry=1`.

## 4-bit register

An adder's output changes when its input changes; a register keeps the last
value written to it. It needs `load=1` **and a rising clock edge**. Even if d
changes while the clock stays at 1, it must not write again.

This is different from a D latch, which just says "pass through while clk=1."
If you use the wrong part, the circuit may do more than one operation in the
same clock pulse.

## 12-bit program counter

12 bits select addresses 0–4095. Priority:

1. reset=1: set to zero.
2. otherwise load=1: go to address d.
3. otherwise inc=1: add one.
4. otherwise keep the old address.

`4095 + 1` is 0 when counted mod 4096. This counter adds **one**; the 32-bit
counter on the RISC-V path added four, for fixed-length instructions. On the
4004, instructions can be one or two bytes, so the control unit picks the
right next address.

## Putting them together

The register stores the current state; the adder and the selectors compute
the next state; the program counter picks the instruction to read. Reading a
processor as these small circuits talking to each other is more useful than
seeing it as one magic box.

Continue: [Organizing with modules](moduller.md), [Watching signals](sinyaller.md).
