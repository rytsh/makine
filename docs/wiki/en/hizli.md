<!--
seviyeler: FastAdd32, PipeAdd32, Forward, Cache4
-->
# A fast processor: why are Apple and ARM cores fast?

The RISC-V you built in the Processor level is single-cycle: every clock
tick runs one instruction from start to finish. It is the clearest design,
but the slowest. This section shows, gate by gate, the ideas that make real
processors fast.

## How fast can the clock tick?

Each NAND gate's output changes a short time after its input does. As a
signal passes from gate to gate, these delays add up. The clock can only
tick once the **longest path** (the critical path) has settled; otherwise
the registers store a half-finished result.

makine measures this path: the "longest path" in the solution panel is the
most NAND gates any signal has to pass through. In the example solutions:

| Circuit | Longest path (NANDs) |
|---|---|
| Adder32 (ripple carry) | 68 |
| FastAdd32 (carry select) | 28 |
| The whole RISC-V processor | 2411 |

The processor's path is that long because the divider does 32 subtractions
one after another. Without division the path drops to 122 gates. That is why
real processors divide step by step over many clocks, not in a single one.

## 1. A shorter path: a fast adder

In a ripple adder the carry walks from the lowest bit to the highest in
turn: 32 full adders for 32 bits. A **carry-select** adder adds the upper
bytes twice: once assuming carry 0, once assuming carry 1. When the real
carry arrives, a MUX picks the right result. The gate count doubles, but the
path halves. This is an **area–speed trade-off**; chip design is full of
them.

Real processors use a more advanced version, the **carry-lookahead** adder,
where a tree-shaped circuit works out the carry in log₂(32) = 5 steps.

## 2. The pipeline

Think of a laundry: washing, drying, ironing. Once the first load moves to
the dryer, the washer does not sit idle; the second load starts washing.
One load takes as long as before, but every stage is always busy.

In processors there are usually five stages:

| Stage | What it does |
|---|---|
| IF (fetch) | reads the instruction at pc from memory |
| ID (decode) | decodes it and reads the registers |
| EX (execute) | the ALU computes |
| MEM (memory) | does a load or a store |
| WB (write back) | writes the result to a register |

Registers go between the stages. On every clock tick, every instruction
moves to the next stage. Because the stages are short, the clock can get up
to five times faster. In the **PipeAdd32** level you do this with an adder:
the work is split in two, a 49-bit register goes between, and the longest
path halves.

Two ideas are worth keeping apart:

- **Latency:** the clocks until one result comes out. A pipeline adds to it.
- **Throughput:** results per clock. A pipeline keeps it, and since the clock
  is faster, much more work gets done per second.

## 3. Hazards and forwarding

A pipeline brings a new problem. Take these two instructions:

```
add x5, x1, x2
sub x6, x5, x3
```

`sub` reads x5 in the ID stage. But `add` is still in EX at that point; its
result will be written to the register file two clocks later. If `sub`
reads the old value, it computes the wrong thing. This is a **data hazard**.

The fix is to **forward** `add`'s result straight to `sub`'s ALU input
without waiting. In the **Forward** level you build the circuit that makes
this decision: which stage's result goes to which input?

Not every hazard can be forwarded away. A load (`lw`) only gets its data in
the MEM stage. If the very next instruction needs that data, the processor
has to **stall** for a clock. Branches have a similar problem: which
instruction to fetch is not known until the branch executes. That is why
real processors do **branch prediction**.

## 4. The cache

A processor takes about a nanosecond to do an addition. Reading a word from
main memory (DRAM) takes about 100 nanoseconds. If the processor waited for
memory on every access, it would spend almost all its time waiting.

A **cache** is a small, fast copy of recently used data kept next to the
processor. Programs tend to use the same data again and again, and data that
sits close together. So even a small cache catches most accesses.

In the **Cache4** level you build a 4-line cache:

- The low bits of the address pick the **line**.
- The upper bits are stored as the **tag**. The tag says which address the
  line's data belongs to.
- The **valid** bit stops an empty line from matching by accident at
  power-on.

Real processors have layers of caches: L1 (tens of KB, a few clocks), L2
(a few MB), L3 (tens of MB). The L1 instruction cache of Apple M1's
performance cores is 192 KB.

## 5. Beyond: Apple and ARM cores

The four ideas in this section are the foundation of modern cores. On top of
them come:

- **Superscalar:** more than one instruction per clock. M1's performance core
  decodes 8 instructions at once.
- **Out-of-order execution:** an instruction that is ready runs even if it is
  not its turn. Hundreds of instructions can be "in flight" at once.
- **Branch prediction:** the processor guesses a branch's outcome from its
  history, and is right more than 95% of the time.
- **Deeper pipelines** and more cache.

Each of these is the parts you built here, made bigger and wired together:
registers, MUXes, comparators and adders.

To run your processor on a real FPGA, see the [A real chip](silikon.md)
page.
