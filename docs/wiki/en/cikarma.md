<!--
seviyeler: Sub32
-->
# Negative numbers and subtraction

## Two's complement

With 32 bits you can write numbers from 0 to 4,294,967,295. For negative
numbers, computers use a clever method called **two's complement**:

> **-x = NOT(x) + 1**

An 8-bit example:

```
 5        = 0000 0101
 NOT 5    = 1111 1010
 +1       = 1111 1011   ← this is -5
```

Let's check: `5 + (-5)`

```
   0000 0101
 + 1111 1011
 -----------
 1 0000 0000   → the overflowing carry is dropped → 0000 0000 = 0 ✓
```

Here's the magic: **the adder circuit works correctly with negative numbers
without any change**. You don't need a separate "negative addition" circuit.

### Top bit = sign

In two's complement, if the top bit is 1, the number is negative:

| 8 bits | unsigned | signed |
|---|---|---|
| `0000 0001` | 1 | 1 |
| `0111 1111` | 127 | 127 |
| `1000 0000` | 128 | **-128** |
| `1111 1111` | 255 | **-1** |

The same bits can be read in two different ways. Which one depends on the
**instruction**, not the bits: `blt` compares signed, `bltu` unsigned.

## Subtraction

```
a - b  =  a + (-b)  =  a + NOT(b) + 1
```

As a circuit:

1. Flip the 32 bits of `b` with **Not ×32**.
2. Give `a` and `NOT b` to the 32-bit Adder.
3. For the **+1**, connect a **Constant 1** to the adder's `c` (carry) input.

No extra part needed: the adder's carry input is there for exactly this job.

> [!TIP]
> In this level you'll use the **Constant** part: right-click → width 1,
> value 1.

## Overflow

A signed 32-bit number holds at most `2,147,483,647`. Add 1 to it and you get
`-2,147,483,648`! This is called **overflow**. RISC-V doesn't treat it as an
error, it just wraps around. A program can check for it if it wants to.

Next: [Memory](hafiza.md).
