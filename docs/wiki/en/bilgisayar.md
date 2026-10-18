<!--
seviyeler:
-->
# The computer and programs

When you finish the processor, **🖥 Computer** unlocks on the left. Your
processor is now connected to memory, a screen, and a keyboard.

![Logic chips on a breadboard](img/foto-breadboard.jpg)

*This is how it starts in real life too: a breadboard, a few logic chips,
and some wires.*

## Memory map

The processor only says "read/write at this address." The **memory map**
decides what lives at which address:

| Address | What's there |
|---|---|
| `0x00000000` | RAM (16 MB). The program is loaded here, and the processor starts here |
| `0x10000000` | console: a byte written here is printed on screen; reading gives the next key |
| `0x10000004` | time: milliseconds since startup |
| `0x10000008` | keyboard: the next key event (bit 8 = pressed, low byte = key) |
| `0x1000000C` | writing here updates the screen |
| `0x11000000` | screen: 320×200 pixels, each one a `0x00RRGGBB` word |

This is called **memory-mapped I/O**. You don't need a special instruction to
draw on the screen, just `sw`. Real computers mostly work this way too.

## First program

```asm
        li   t1, 0x10000000     # console address
        li   t0, 'H'
        sb   t0, 0(t1)          # write 'H'
        li   t0, 'i'
        sb   t0, 0(t1)          # write 'i'
        ebreak                  # stop
```

The **Examples** section has ready-made programs: *Hello* (console), *Colors*
(screen), and *Keyboard* (a square you move with the arrow keys).

## Speed: ⚡ and 🚀

| Mode | What is simulated | Speed (approx.) |
|---|---|---|
| ⚡ off | everything at NAND level | ~1,500 instructions/s |
| ⚡ on | chips that passed their tests, replaced by fast models with the same behavior | ~100,000 instructions/s |
| 🚀 Turbo | the reference processor (the one yours is tested against) | ~200,000,000 instructions/s |

With ⚡ on, the result doesn't change: each model passes the same test your
chip passed. Chip designers work like this too. First they write a fast
behavioral model, then they compare the gate-level circuit against it.

## C programs

Writing assembly takes a long time. Instead, you can write C and turn it into
RISC-V instructions with a **compiler**:

```c
#include <stdio.h>
#include <makine.h>

int main(void) {
    printf("Merhaba C!\n");
    for (int t = 0;; t++) {
        for (int i = 0; i < 320 * 200; i++)
            MK_SCREEN[i] = (i + t) * 0x010203;
        mk_show();
    }
}
```

```sh
make c SRC=program.c        # or: sdk/build.sh program.c
```

Open the resulting `program.elf` with **Load from file**.

## Doom

**Doom**, the legendary game from 1993, is written in C and its source code is
open. You can compile it for RISC-V and run it on your computer:

```sh
make doom
```

Then **👹 Doom** → **🎮 Play**. The arrows walk, Ctrl fires, space opens
doors. F10 quits.

Doom needs tens of millions of instructions per second, so it runs with
🚀 Turbo. But the program that runs is **the same**: the instruction set your
processor was tested on, the same memory map, the same screen. You started
with one NAND gate and ended up building a real computer.

## Going further

- **Nand2Tetris** ("From NAND to Tetris"): the course and book that inspired
  this game.
- **Ben Eater**'s "8-bit breadboard computer" videos: the same journey, with
  real chips.
- **"Digital Design and Computer Architecture: RISC-V Edition"** (Harris &
  Harris): a detailed look at single-cycle and pipelined RISC-V.
- The RISC-V specification: <https://riscv.org/specifications/>
