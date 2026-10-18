<p align="center"><img src="assets/logo.png" width="128" alt="makine logosu"></p>

# makine

## Processor workshop

The **World** is a live circuit workbench with nested, collapsible modules,
named buses, manual wire waypoints, alignment, a navigable overview map and
a per-simulation-step logic analyser (16 probes, 256 samples per probe).
Modules preserve the actual gates, wiring and layout; closing a module does
not replace it with an emulator. Modules and routes are shared over LAN;
open/closed views and probes are local to each player.

**World → Örnek projeler → 4004 atölyesi aç** creates a separate example
world with a gate-built 4-bit arithmetic/control core and an editable 4 KiB
program ROM. Registers and ROM are storage primitives, not an opcode
interpreter. This is **not a complete or timing-accurate 4004/MCS-4**: SRC,
DCL and external memory/I/O instructions are not implemented; unsupported
opcodes assert a fault and hold PC. See [the scope and example program
(Turkish)](docs/wiki/4004.md).

Four new exercises cover a 4-bit multiplexer, carry adder, register and
12-bit program counter. The existing RISC-V / Doom path is unchanged.

*[Türkçe](README.md)*

An educational tool where you build a computer yourself, starting from a single NAND
gate. In the last level, the RISC-V CPU you designed runs real programs, including
compiled C programs and Doom.

## Running

```sh
make run            # or: cargo run --release -p makine
```

Common commands are in the Makefile; run `make` to list them:

| Command | Does |
|---|---|
| `make run` | run the app |
| `make run-gl` | run with OpenGL instead of Vulkan/DirectX |
| `make test` | run the tests |
| `make check` | format check, clippy and tests |
| `make doom` | build Doom |
| `make c SRC=prog.c` | compile a C program |
| `make windows` | build the Windows `.exe` from Linux |
| `make dist-windows` | zip the `.exe` + Doom into `dist/makine-windows-x64.zip` |
| `make clean` | delete build output |

Graphics go through wgpu, which picks whichever of Vulkan, DirectX 12, Metal or
OpenGL works. If no window appears, `WGPU_BACKEND=gl` forces OpenGL (on Windows:
`set WGPU_BACKEND=gl`, then `makine.exe`).

Progress is saved automatically in `~/.local/share/makine/` on Linux and
`%APPDATA%\makine\data\` on Windows. Each profile has its own file (`profile-1.json`,
`profile-2.json`, …), listed in `profiles.json`. Profiles are managed on the home page
(click the logo at the top left); a profile can be moved to another computer as text or
as a file.

Prebuilt versions: the [Releases](https://github.com/rytsh/makine/releases) page has
zip files for Windows and Linux (Doom included).

### Windows

1. Install [Rust](https://rustup.rs) (`rustup-init.exe`, default options). When asked,
   also install the "Visual Studio Build Tools" ("Desktop development with C++").
2. Get the source and run it:

   ```powershell
   git clone https://github.com/rytsh/makine
   cd makine
   cargo run --release -p makine
   ```

   The program is built as `target\release\makine.exe`. It is a single file and can be
   copied anywhere.

3. Doom and C programs need [LLVM](https://github.com/llvm/llvm-project/releases)
   (`clang` and `ld.lld`): `winget install LLVM.LLVM`, then open a new terminal.

   ```powershell
   powershell -ExecutionPolicy Bypass -File examples\doom\build.ps1
   powershell -ExecutionPolicy Bypass -File sdk\build.ps1 examples\c\merhaba.c
   ```

   If you put `doom.elf` next to `makine.exe`, the app finds it there too.

To build for Windows from Linux: `cargo install cargo-xwin`, then
`cargo xwin build --release -p makine --target x86_64-pc-windows-msvc`. The C runtime
is linked statically (`.cargo/config.toml`), so `makine.exe` also runs on machines
without the Visual C++ Redistributable.

## How to play

The interface is in Turkish. The basics:

- **Drag** a part from the "Parçalar" (parts) list onto the canvas.
- Drag from an output pin to an input pin to **draw a wire**.
- **Click** the input boxes on the left to change their values; wires light up live.
- **Right-click** a part to delete it ("Sil") or configure it: repeat count ("Tekrar",
  N copies side by side), bus groups, or a constant's value. Right-clicking an input pin
  disconnects its wire.
- Scroll to zoom, drag empty space to pan.
- NOT, AND, OR, XOR, NAND and MUX are drawn with standard logic symbols; the
  **"Kapı sembolleri"** checkbox switches back to labelled boxes.
- **F11** toggles full screen (as does the **"Tam ekran"** checkbox; the setting is
  remembered).
- Click parts to select them (Shift adds, Shift+drag draws a box); selected parts move
  together. **Delete** removes, **Ctrl+C / Ctrl+V / Ctrl+D** copy, paste and duplicate;
  **Ctrl+Z / Ctrl+Y** undo and redo.
- **"Test et"** checks your chip. Once it passes, the chip becomes a part you can use
  in later levels.

## Wiki

The **📖 Wiki** section on the left explains the idea behind every part: from
the transistor to NAND, De Morgan to adders, flip-flops to the RISC-V CPU. Each
level has a **"📖 Wiki'de oku"** link to its page, and pages link back to their
levels. The pages are in Turkish.

The pages are plain Markdown in `docs/wiki/` (readable on GitHub too) and are
compiled into the executable, so it stays a single file. To add a page:

1. Write `docs/wiki/new.md`, starting with the levels it explains:
   `<!--` / `seviyeler: Latch, DFF` / `-->`
2. Add it to `PAGES` in `crates/app/src/wiki.rs`.
3. Put images in `docs/wiki/img/` and add them to `IMAGES`.

Tests catch missing pages, broken links, images that are not embedded, and
levels without a page. Diagrams are drawn by `docs/diagrams/make.py`
(`make diagrams`); the photos are AI-generated.

## Sections

1. **Logic:** NOT, AND, OR, XOR, MUX, DEMUX, 32-bit MUX
2. **Arithmetic:** half/full adder, 8- and 32-bit adder, subtractor
3. **Memory:** SR latch, D latch, D flip-flop, 1-bit register, 32-bit register,
   program counter, 8-word RAM
   - **★ Mini processor (optional):** add/subtract, control and a small processor with an
     8-bit accumulator; once it passes, you step through programs on your own circuit
4. **ALU:** zero test, comparator, left/right shifter, ALU, register file
5. **CPU:** instruction decoder, control unit, branch unit, load/store units,
   multiply/divide, and the **RV32IM CPU** that ties them together

The CPU test runs real programs on your CPU and on a reference CPU side by side and
compares them after every instruction. When they differ, it tells you which
instruction and which register went wrong.

## The computer

Finishing the CPU unlocks **Bilgisayar** (computer) on the left: your CPU + 16 MB RAM +
a 320×200 screen + console + keyboard. You can load the example programs (assembly) or
your own (assembly `.s` or a RISC-V ELF).

- **⚡ Fast simulation ("Hızlı simülasyon"):** chips that passed their test are
  simulated by a Rust model with the same behaviour instead of gate by gate. When off,
  everything runs at NAND level (the CPU is ~140k NAND gates, ~1,500 instructions per
  second). When on, ~100k instructions per second.
- **🚀 Turbo:** uses the reference emulator your CPU was tested against (~200 million
  instructions per second). Needed for Doom.
- **🎮 Play ("Oyna"):** shows only the computer's screen and goes full screen, like a
  game. **F10** leaves.

### Doom

```sh
examples/doom/build.sh          # downloads doomgeneric + DOOM1.WAD, builds build/doom.elf
                                # Windows: examples\doom\build.ps1
```

Then Bilgisayar > **Doom**. Click the screen; arrow keys move, Ctrl fires, Space opens
doors, Enter selects in menus. Requires `clang`, `ld.lld`, `curl` and `git`.

### Your own C program

```sh
sdk/build.sh examples/c/merhaba.c -o merhaba.elf
# Windows: sdk\build.ps1 examples\c\merhaba.c -Out merhaba.elf
```

`sdk/` contains a small libc (printf, malloc, strings, files), startup code and a linker
script. See `sdk/include/makine.h` for the devices. The first build downloads the
floating-point and 64-bit division routines from LLVM compiler-rt.

Memory map:

| Address | Device |
|---|---|
| `0x00000000` | RAM; programs start here |
| `0x10000000` | console: write a byte to print it, read the next typed character |
| `0x10000004` | timer: milliseconds since start |
| `0x10000008` | keyboard: next key event (bit 8 = pressed, low byte = key) |
| `0x1000000C` | write to show the screen |
| `0x11000000` | screen pixels, one `0x00RRGGBB` word each, row by row |

## Layout

| Folder | Contents |
|---|---|
| `crates/sim` | Simulator: chip model, flattening to NAND gates, 64-lane evaluation, native (Rust) parts |
| `crates/levels` | Levels, tests, reference solutions, fast models |
| `crates/rv` | RV32IM: reference emulator, assembler, disassembler, ELF loader, machine (RAM + devices) |
| `crates/app` | egui desktop app |
| `sdk` | libc, startup code and linker script for C programs |
| `examples` | C example and the Doom build script |

```sh
cargo test                                                       # all tests
cargo test --release -p makine-levels -- --ignored               # test the CPU at NAND level
cargo run --release -p makine-rv --example run -- program.elf    # run from the command line
WGPU_BACKEND=vulkan cargo test -p makine screenshot -- --ignored # UI screenshots
```

## Releases

Pushing a tag like `v0.1.0` runs `.github/workflows/release.yml`, which tests and
builds makine and Doom on Windows and Linux and attaches the zips to a GitHub release.

## Licenses

makine is MIT licensed. The bundled DejaVu fonts (`crates/app/fonts/`) are under
the Bitstream Vera license, see `LICENSE-DejaVu.txt`. The files the Doom build script downloads come with their own
licenses and are not part of the repository: doomgeneric (GPL-2.0), DOOM1.WAD (id
Software shareware, freely redistributable), compiler-rt (Apache-2.0 with LLVM
exception).
