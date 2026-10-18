<!--
seviyeler: HalfAdder, FullAdder, Adder8, Adder32
-->
# Addition

Addition works like you learned in elementary school: right to left, digit by
digit, carrying to the next digit.

![Half adder and a chain of full adders](img/toplayici.png)

## Half adder

Add two bits. The result is at most `1 + 1 = 10` (binary), so two bits:

- **sum** (low digit) = `a XOR b`
- **carry** (high digit) = `a AND b`

Two gates, that's it.

## Full adder

A digit in the middle of a number adds **three** bits: a, b, and the carry
(`c`) coming from the digit on its right. We do this with two half adders:

1. `a + b` (first half adder)
2. the resulting sum `+ c` (second half adder)
3. if either carry is 1, the result's carry is 1: `carry1 OR carry2`

| a | b | c | carry | sum |
|---|---|---|---|---|
| 0 | 0 | 0 | 0 | 0 |
| 0 | 1 | 1 | 1 | 0 |
| 1 | 1 | 1 | 1 | 1 |

> [!NOTE]
> The two carries can't both be 1 at the same time. Why? If the first sum is
> 1, the first carry is 0. That's why XOR could also be used instead of OR.

## The chain: ripple carry

Line up full adders side by side: each one's carry goes to the one on its
left. Line up 8 and you get an 8-bit adder; line up 32 and you get a 32-bit
adder. The `c` input of the rightmost adder is usually 0.

In the game, for the **8-bit Adder**:

1. Split `a[8]` and `b[8]` into bits with **Split 1,1,1,1,1,1,1,1**.
2. Chain 8 Full Adders.
3. Turn the sum bits back into a bus with **Join**.

The **32-bit Adder** is just four 8-bit adders: `Split 8,8,8,8`.

### The speed problem

The bit at the end of the chain has to wait for the carry to **ripple** all
the way from the start. With 32 bits, that's a delay of 32 digits. Real
processors speed this up with tricks like **carry lookahead**: instead of
waiting for the carry one digit at a time, they work out ahead of time whether
it will come. The game's simulation doesn't care about this; but in a real
chip, it decides the clock speed.

## What is the carry for?

The carry coming out of the top adder tells you the result **didn't fit**:
`0xFFFFFFFF + 1 = 0x1_00000000`, but 32 bits only hold `0x00000000`.
We'll also use this carry for [subtraction](cikarma.md) and
[comparison](alu.md).

Next: [Negative numbers and subtraction](cikarma.md).
