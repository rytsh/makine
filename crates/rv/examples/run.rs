//! Run a RISC-V ELF or assembly file on the reference emulator.
//!
//! cargo run --release -p makine-rv --example run -- program.elf [max_instructions] [frame.ppm]
//!
//! Prints the console output, the instruction count and speed. With a third
//! argument, writes the last frame shown as a PPM image.

use std::time::Instant;

use makine_rv::emu::{Cpu, Trap};
use makine_rv::machine::{Machine, HEIGHT, WIDTH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: run <program> [max] [frame.ppm]");
    let max: u64 = args.get(2).map_or(u64::MAX, |s| s.parse().expect("max"));
    let bytes = std::fs::read(path).expect("read program");
    let mut m = Machine::new(16 << 20);
    let mut cpu = Cpu::new();
    if bytes.starts_with(b"\x7fELF") {
        let img = makine_rv::elf::load(&bytes).expect("elf");
        for (a, b) in &img.segments {
            m.load(*a, b);
        }
        cpu.pc = img.entry;
    } else {
        let p = makine_rv::asm::assemble(std::str::from_utf8(&bytes).unwrap()).expect("asm");
        m.load(0, &p.bytes);
    }
    let t = Instant::now();
    let trap = cpu.run(&mut m, max);
    let dt = t.elapsed().as_secs_f64();
    print!("{}", m.console_text());
    eprintln!(
        "\n--- {} instructions in {dt:.2}s ({:.1} MIPS), {} frames, stop: {:?} at pc 0x{:08X}",
        cpu.instret,
        cpu.instret as f64 / dt / 1e6,
        m.frames,
        trap.map_or("budget".to_string(), |t: Trap| t.to_string()),
        cpu.pc
    );
    if let Some(out) = args.get(3) {
        let mut ppm = format!("P6 {WIDTH} {HEIGHT} 255\n").into_bytes();
        for p in &m.framebuffer {
            ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
        }
        std::fs::write(out, ppm).expect("write frame");
    }
}
