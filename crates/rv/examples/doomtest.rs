//! Play Doom headless for a few seconds of game time and save frames.
//! cargo run --release -p makine-rv --example doomtest -- doom.elf outdir
use makine_rv::emu::Cpu;
use makine_rv::machine::{key, Machine, HEIGHT, WIDTH};

fn save(m: &Machine, path: &str) {
    let mut ppm = format!("P6 {WIDTH} {HEIGHT} 255\n").into_bytes();
    for p in &m.framebuffer {
        ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    std::fs::write(path, ppm).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = makine_rv::elf::load(&std::fs::read(&a[1]).unwrap()).unwrap();
    let mut m = Machine::new(16 << 20);
    for (addr, b) in &img.segments {
        m.load(*addr, b);
    }
    let mut cpu = Cpu::new();
    cpu.pc = img.entry;
    // Simulated time: 1 ms per 100k instructions (100 MHz machine).
    let mut ms = 0u32;
    let script: &[(u32, u8, bool)] = &[
        (3000, key::ENTER, true),
        (3100, key::ENTER, false), // menu: new game
        (3500, key::ENTER, true),
        (3600, key::ENTER, false), // episode
        (4000, key::ENTER, true),
        (4100, key::ENTER, false), // skill
        (6000, key::UP, true),
        (7500, key::UP, false), // walk
        (7600, key::CTRL, true),
        (7700, key::CTRL, false), // fire
        (8000, key::LEFT, true),
        (8600, key::LEFT, false),
    ];
    let mut next = 0;
    let t = std::time::Instant::now();
    while ms < 10_000 {
        m.fixed_time = Some(ms);
        if let Some(trap) = cpu.run(&mut m, 100_000) {
            eprintln!("stopped: {trap} at {:08x}", cpu.pc);
            break;
        }
        ms += 1;
        while next < script.len() && script[next].0 <= ms {
            m.keys.push_back((script[next].1, script[next].2));
            next += 1;
        }
        if ms.is_multiple_of(2500) {
            save(&m, &format!("{}/frame-{ms}.ppm", a[2]));
        }
    }
    eprintln!(
        "{} instr, {} frames, {:.2}s",
        cpu.instret,
        m.frames,
        t.elapsed().as_secs_f64()
    );
    print!(
        "{}",
        m.console_text()
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n")
    );
}
