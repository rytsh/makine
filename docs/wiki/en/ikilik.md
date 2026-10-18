<!--
seviyeler:
-->
# Binary numbers

Everything inside a computer (numbers, letters, pictures, Doom's maps) is
made of **0**s and **1**s in the end. Why?

## Why only two digits?

On a wire, electricity is either **there** or **not there**. Reliably measuring
"a little bit there" is hard: noise, heat, and the length of the cable all
nudge the voltage a bit. But the question "high or low?" holds up very well
against those nudges. That's why each wire carries just one thing: **one bit**.

| Voltage | Meaning |
|---|---|
| high (e.g. 1 V) | **1** |
| low (0 V) | **0** |

## Counting in binary

In the decimal system, each digit is worth **10 times** the one before it:
ones, tens, hundreds… In binary, each digit is worth **2 times** as much:
1, 2, 4, 8, 16…

![The digits of a binary number](img/ikilik.png)

The value of a binary number is **the sum of the values of the digits that
are 1**:

```
  0 0 1 0 1 1 0 1
  │ │ │ │ │ │ │ └─ 1
  │ │ │ │ │ │ └─── 2   (0)
  │ │ │ │ │ └───── 4
  │ │ │ │ └─────── 8
  │ │ │ └───────── 16  (0)
  │ │ └─────────── 32
  └─┴───────────── 64, 128 (0)

  32 + 8 + 4 + 1 = 45
```

Counting works like in decimal too, except a "carry" appears after 1:

```
0, 1, 10, 11, 100, 101, 110, 111, 1000, …
```

## Bit, byte, word

| Name | Number of bits | How many different values |
|---|---|---|
| bit | 1 | 2 |
| byte | 8 | 256 |
| halfword | 16 | 65,536 |
| word (RISC-V) | 32 | 4,294,967,296 |

The **buses** in the game (e.g. `a[32]`) are 32 wires side by side: a single
32-bit number.

## Hexadecimal (hex)

Writing 32 zeros and ones is tedious. So we write each group of 4 bits as a
single **hexadecimal** digit: `0`–`9`, then `A`=10 … `F`=15.

```
1101 1110 1010 1101 1011 1110 1110 1111
  D    E    A    D    B    E    E    F    →  0xDEADBEEF
```

In the game, bus values are shown in hex with `0x…`.

## Try it yourself

> [!TIP]
> In the 8-bit Adder level, set the inputs to `0x0F` and `0x01`. The result
> should be `0x10`: 15 + 1 = 16. Adding 1 when the lower four bits are all 1
> carries into the next digit, just like 9 + 1 = 10 in decimal.
