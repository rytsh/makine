//! The computer: the player's CPU (or the reference emulator, in turbo mode)
//! wired to RAM, a screen, a console and a keyboard.

use std::time::{Duration, Instant};

use egui::{Color32, ColorImage, RichText, TextureHandle, TextureOptions, Ui};
use makine_levels::cpu::CircuitCpu;
use makine_rv::emu::{Cpu, Trap};
use makine_rv::machine::{key, Machine, HEIGHT, WIDTH};
use makine_sim::{Circuit, Library};

pub const RAM_BYTES: usize = 16 << 20;

/// Built-in example programs (assembly source).
pub const EXAMPLES: &[(&str, &str)] = &[
    ("Merhaba", include_str!("../programs/hello.s")),
    ("Renkler", include_str!("../programs/colors.s")),
    ("Klavye", include_str!("../programs/keys.s")),
];

pub enum Engine {
    /// The player's CPU circuit.
    Circuit(Box<CircuitCpu>),
    /// The reference emulator ("turbo").
    Reference(Cpu),
}

pub struct Computer {
    pub machine: Machine,
    pub engine: Engine,
    pub running: bool,
    pub halted: Option<String>,
    pub instructions: u64,
    /// Instructions per second, measured.
    pub speed: f64,
    speed_window: (Instant, u64),
    texture: Option<TextureHandle>,
    shown_frame: u64,
    /// Program image to reload on reset.
    image: Vec<(u32, Vec<u8>)>,
    entry: u32,
    /// Modifier keys as last reported, to turn changes into key events.
    modifiers: egui::Modifiers,
    /// Addresses to stop at before running the instruction there.
    pub breakpoints: std::collections::BTreeSet<u32>,
    /// With the player's CPU: the reference emulator run beside it, for
    /// showing registers (the circuit's are inside it). Dropped (and the
    /// registers hidden) once the two disagree or a device is read.
    pub shadow: Option<Cpu>,
    /// Where the shadow first disagreed with the circuit.
    pub shadow_lost: Option<String>,
}

impl Computer {
    pub fn new(engine: Engine) -> Self {
        Self {
            machine: Machine::new(RAM_BYTES),
            engine,
            running: false,
            halted: None,
            instructions: 0,
            speed: 0.0,
            speed_window: (Instant::now(), 0),
            texture: None,
            shown_frame: u64::MAX,
            image: Vec::new(),
            entry: 0,
            modifiers: egui::Modifiers::NONE,
            breakpoints: Default::default(),
            shadow: None,
            shadow_lost: None,
        }
    }

    /// The registers, if known: the emulator's own, or the shadow's.
    pub fn registers(&self) -> Option<&[u32; 32]> {
        match &self.engine {
            Engine::Reference(c) => Some(&c.x),
            Engine::Circuit(_) => self.shadow.as_ref().map(|s| &s.x),
        }
    }

    /// Build the player's CPU from their library.
    pub fn circuit_engine(lib: &Library) -> Result<Engine, String> {
        let c = Circuit::compile(lib, "Cpu").map_err(|e| e.to_string())?;
        let mut cpu = CircuitCpu::new(c);
        cpu.reset()?;
        Ok(Engine::Circuit(Box::new(cpu)))
    }

    pub fn load_program(
        &mut self,
        segments: Vec<(u32, Vec<u8>)>,
        entry: u32,
    ) -> Result<(), String> {
        for (addr, bytes) in &segments {
            if *addr as usize + bytes.len() > RAM_BYTES {
                return Err(makine_lang::trf!(
                    "program belleğe sığmıyor (0x{addr:08X})",
                    "the program does not fit in memory (0x{addr:08X})"
                ));
            }
        }
        self.image = segments;
        self.entry = entry;
        self.reset()
    }

    pub fn reset(&mut self) -> Result<(), String> {
        self.machine = Machine::new(RAM_BYTES);
        for (addr, bytes) in &self.image {
            self.machine.load(*addr, bytes);
        }
        self.halted = None;
        self.instructions = 0;
        self.shown_frame = u64::MAX;
        match &mut self.engine {
            Engine::Circuit(cpu) => {
                if self.entry != 0 {
                    return Err(makine_lang::tr!(
                        "devre işlemcisi 0 adresinden başlar",
                        "the circuit processor starts at address 0"
                    )
                    .into());
                }
                cpu.reset()?;
                self.shadow = Some(Cpu::new());
                self.shadow_lost = None;
            }
            Engine::Reference(cpu) => {
                *cpu = Cpu::new();
                cpu.pc = self.entry;
            }
        }
        Ok(())
    }

    /// Run for about `budget` of wall time.
    pub fn run_for(&mut self, budget: Duration) {
        if !self.running || self.halted.is_some() {
            return;
        }
        let start = Instant::now();
        // The first instruction may be at a breakpoint we just stopped at.
        let mut first = true;
        'run: while start.elapsed() < budget {
            let chunk = match self.engine {
                Engine::Circuit(_) => 64,
                Engine::Reference(_) if self.breakpoints.is_empty() => 100_000,
                Engine::Reference(_) => 10_000,
            };
            for _ in 0..chunk {
                if !first && self.breakpoints.contains(&self.pc()) {
                    self.running = false;
                    break 'run;
                }
                first = false;
                if let Err(e) = self.step_n(1) {
                    self.halted = Some(e);
                    self.running = false;
                    break 'run;
                }
            }
        }
        let (t0, n0) = self.speed_window;
        let dt = t0.elapsed().as_secs_f64();
        if dt > 0.5 {
            self.speed = (self.instructions - n0) as f64 / dt;
            self.speed_window = (Instant::now(), self.instructions);
        }
    }

    pub fn step_n(&mut self, n: u64) -> Result<(), String> {
        for _ in 0..n {
            let r = match &mut self.engine {
                Engine::Circuit(cpu) => {
                    let pc = cpu.pc();
                    let instr = self.machine.peek_ram(pc);
                    if instr == makine_rv::isa::EBREAK {
                        Err(Trap::Breakpoint.to_string())
                    } else {
                        // The shadow runs on a copy of the memory, so the
                        // program's reads of devices happen once.
                        let shadow = self.shadow.take().map(|mut s| {
                            let mut m = Peek(&self.machine);
                            let ok = s.step(&mut m).is_ok();
                            (s, ok)
                        });
                        let r = cpu.step(&mut self.machine).map(|_| ());
                        if let Some((s, ok)) = shadow {
                            if ok && s.pc == cpu.pc() && r.is_ok() {
                                self.shadow = Some(s);
                            } else if self.shadow_lost.is_none() {
                                self.shadow_lost = Some(makine_lang::trf!(
                                    "0x{pc:08X}: senin işlemcin referanstan ayrıldı",
                                    "0x{pc:08X}: your processor went apart from the reference"
                                ));
                            }
                        }
                        r
                    }
                }
                Engine::Reference(cpu) => cpu.step(&mut self.machine).map_err(|t| t.to_string()),
            };
            match r {
                Ok(()) => self.instructions += 1,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn pc(&self) -> u32 {
        match &self.engine {
            Engine::Circuit(c) => c.pc(),
            Engine::Reference(c) => c.pc,
        }
    }

    pub fn press(&mut self, k: u8, pressed: bool) {
        self.machine.keys.push_back((k, pressed));
        if pressed {
            self.machine.console_in.push_back(k);
        }
    }

    /// Upload the framebuffer if a new frame was shown.
    fn update_texture(&mut self, ctx: &egui::Context) {
        if self.machine.frames == self.shown_frame && self.texture.is_some() {
            return;
        }
        self.shown_frame = self.machine.frames;
        let pixels: Vec<Color32> = self
            .machine
            .framebuffer
            .iter()
            .map(|&p| Color32::from_rgb((p >> 16) as u8, (p >> 8) as u8, p as u8))
            .collect();
        let img = ColorImage::new([WIDTH as usize, HEIGHT as usize], pixels);
        match &mut self.texture {
            Some(t) => t.set(img, TextureOptions::NEAREST),
            None => self.texture = Some(ctx.load_texture("screen", img, TextureOptions::NEAREST)),
        }
    }

    /// The screen alone, as large as fits in `ui` with square pixels, on
    /// black. Keyboard input goes to the machine. For the full-window
    /// "play" mode.
    pub fn screen_only(&mut self, ui: &mut Ui) {
        self.update_texture(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, Color32::BLACK);
        let scale = (full.width() / WIDTH as f32).min(full.height() / (HEIGHT as f32 * 1.2));
        // Doom-era games assume 4:3 with tall pixels: 320x200 shown at 1.2x height.
        let size = egui::vec2(WIDTH as f32 * scale, HEIGHT as f32 * 1.2 * scale);
        let rect = egui::Rect::from_center_size(full.center(), size);
        if let Some(t) = &self.texture {
            ui.painter().image(
                t.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        if let Some(h) = &self.halted {
            ui.painter().text(
                full.center_bottom() - egui::vec2(0.0, 20.0),
                egui::Align2::CENTER_BOTTOM,
                makine_lang::trf!("Durdu: {h}", "Stopped: {h}"),
                egui::FontId::proportional(16.0),
                crate::theme::BAD,
            );
        }
        self.keyboard(ui);
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.update_texture(ui.ctx());
        let avail = ui.available_width().min(960.0);
        let scale = (avail / WIDTH as f32).floor().max(1.0);
        if let Some(t) = &self.texture {
            let size = egui::vec2(WIDTH as f32 * scale, HEIGHT as f32 * scale);
            let r = ui.add(egui::Image::new((t.id(), size)).sense(egui::Sense::click()));
            if r.clicked() {
                r.request_focus();
            }
            if r.has_focus() || self.running {
                self.keyboard(ui);
            }
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("pc = 0x{:08X}", self.pc())).monospace());
            ui.label(makine_lang::trf!(
                "{} komut",
                "{} instructions",
                self.instructions
            ));
            if self.running {
                ui.label(human_speed(self.speed));
            }
        });
        if let Some(h) = &self.halted {
            let color = if h.starts_with("ebreak") {
                crate::theme::GOOD
            } else {
                crate::theme::BAD
            };
            ui.colored_label(color, makine_lang::trf!("Durdu: {h}", "Stopped: {h}"));
        }
        ui.label(RichText::new(makine_lang::tr!("Konsol", "Console")).strong());
        let text = self.machine.console_text();
        egui::ScrollArea::vertical()
            .id_salt("console")
            .max_height(160.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(if text.is_empty() { " " } else { &text }).monospace(),
                    )
                    .wrap(),
                );
            });
    }

    fn keyboard(&mut self, ui: &mut Ui) {
        let (events, mods) = ui.input(|i| (i.events.clone(), i.modifiers));
        for (was, now, code) in [
            (self.modifiers.ctrl, mods.ctrl, key::CTRL),
            (self.modifiers.shift, mods.shift, key::SHIFT),
            (self.modifiers.alt, mods.alt, key::ALT),
        ] {
            if was != now {
                self.machine.keys.push_back((code, now));
            }
        }
        self.modifiers = mods;
        for e in events {
            match e {
                egui::Event::Key {
                    key: k,
                    pressed,
                    repeat: false,
                    ..
                } => {
                    if let Some(code) = key_code(k) {
                        self.press(code, pressed);
                    }
                }
                egui::Event::Text(t) => {
                    for c in t.chars().filter(char::is_ascii) {
                        if !c.is_ascii_alphanumeric() && c != ' ' {
                            self.machine.console_in.push_back(c as u8);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn key_code(k: egui::Key) -> Option<u8> {
    use egui::Key as K;
    Some(match k {
        K::ArrowLeft => key::LEFT,
        K::ArrowRight => key::RIGHT,
        K::ArrowUp => key::UP,
        K::ArrowDown => key::DOWN,
        K::Enter => key::ENTER,
        K::Escape => key::ESCAPE,
        K::Backspace => key::BACKSPACE,
        K::Tab => key::TAB,
        K::Space => b' ',
        _ => {
            let name = k.name();
            let c = name.chars().next()?;
            if name.len() == 1 && c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase() as u8
            } else {
                return None;
            }
        }
    })
}

/// The machine's memory seen without side effects: the shadow emulator
/// reads through it (device reads give what the circuit will see) and its
/// writes are dropped, since the circuit does the real ones.
struct Peek<'a>(&'a Machine);

impl makine_rv::emu::Memory for Peek<'_> {
    fn fetch(&mut self, addr: u32) -> Option<u32> {
        Some(self.0.peek_ram(addr))
    }
    fn read(&mut self, addr: u32) -> Option<u32> {
        self.0.peek(addr)
    }
    fn write(&mut self, _: u32, _: u32, _: u8) -> Option<()> {
        Some(())
    }
}

pub fn human_speed(ips: f64) -> String {
    if ips >= 1e6 {
        format!("{:.1} MHz", ips / 1e6)
    } else if ips >= 1e3 {
        format!("{:.1} kHz", ips / 1e3)
    } else {
        format!("{ips:.0} Hz")
    }
}

/// Parse a program: RISC-V ELF, or assembly source.
/// Memory segments (address, bytes) and the entry point.
pub type Image = (Vec<(u32, Vec<u8>)>, u32);

pub fn parse_program(name: &str, bytes: &[u8]) -> Result<Image, String> {
    if bytes.starts_with(b"\x7fELF") {
        let img = makine_rv::elf::load(bytes)?;
        return Ok((img.segments, img.entry));
    }
    let src = std::str::from_utf8(bytes).map_err(|_| {
        makine_lang::trf!(
            "{name}: metin ya da ELF değil",
            "{name}: not text and not an ELF"
        )
    })?;
    let p = makine_rv::asm::assemble(src).map_err(|e| e.to_string())?;
    Ok((vec![(0, p.bytes)], 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_example(name: &str, engine: Engine, steps: u64) -> Computer {
        let src = EXAMPLES.iter().find(|(n, _)| *n == name).unwrap().1;
        let (segs, entry) = parse_program(name, src.as_bytes()).unwrap();
        let mut c = Computer::new(engine);
        c.load_program(segs, entry).unwrap();
        c.machine.fixed_time = Some(0);
        let r = c.step_n(steps);
        assert!(matches!(&r, Ok(()) | Err(_)), "{r:?}");
        c
    }

    #[test]
    fn hello_runs_on_reference() {
        let c = run_example("Merhaba", Engine::Reference(Cpu::new()), 1_000_000);
        assert!(
            c.machine.console_text().contains("Merhaba"),
            "{}",
            c.machine.console_text()
        );
    }

    #[test]
    fn colors_draws_a_frame() {
        let c = run_example("Renkler", Engine::Reference(Cpu::new()), 5_000_000);
        assert!(c.machine.frames >= 1);
        assert!(c.machine.framebuffer.iter().any(|&p| p != 0));
    }

    /// A C program (built with sdk/build.sh, if clang is available) runs on
    /// the circuit CPU and matches the reference emulator.
    #[test]
    fn c_program_on_circuit_cpu() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = std::env::temp_dir().join(format!("makine-c-{}.elf", std::process::id()));
        let built = std::process::Command::new(root.join("sdk/build.sh"))
            .arg(root.join("examples/c/merhaba.c"))
            .arg("-o")
            .arg(&out)
            .output();
        let Ok(o) = built else { return };
        if !o.status.success() {
            eprintln!("skipping: {}", String::from_utf8_lossy(&o.stderr));
            return;
        }
        let bytes = std::fs::read(&out).unwrap();
        std::fs::remove_file(&out).ok();
        let (segs, entry) = parse_program("merhaba", &bytes).unwrap();

        use makine_levels::reference::reference;
        let refs: Vec<_> = makine_levels::levels()
            .iter()
            .map(|l| reference(l.chip))
            .collect();
        let lib = makine_levels::fast::library(refs.iter(), true);
        let mut c = Computer::new(Computer::circuit_engine(&lib).unwrap());
        c.load_program(segs.clone(), entry).unwrap();
        c.step_n(20_000).unwrap();
        let mut r = Computer::new(Engine::Reference(Cpu::new()));
        r.load_program(segs, entry).unwrap();
        r.step_n(20_000).unwrap();
        assert_eq!(c.machine.console_text(), "Merhaba C! 40 + 2 = 42\n");
        assert_eq!(c.pc(), r.pc());
        assert_eq!(c.machine.ram, r.machine.ram);
    }

    #[test]
    fn hello_runs_on_reference_circuit_cpu() {
        use makine_levels::reference::reference;
        let refs: Vec<_> = makine_levels::levels()
            .iter()
            .map(|l| reference(l.chip))
            .collect();
        let lib = makine_levels::fast::library(refs.iter(), true);
        let engine = Computer::circuit_engine(&lib).unwrap();
        let c = run_example("Merhaba", engine, 3_000);
        assert!(
            c.machine.console_text().contains("Merhaba"),
            "{}",
            c.machine.console_text()
        );
    }

    /// With the player's CPU, the registers come from the emulator run
    /// beside it, and a breakpoint stops the run before its instruction.
    #[test]
    fn shadow_registers_and_breakpoints() {
        use makine_levels::reference::reference;
        let refs: Vec<_> = makine_levels::levels()
            .iter()
            .map(|l| reference(l.chip))
            .collect();
        let lib = makine_levels::fast::library(refs.iter(), true);
        let src = "li a0, 5\nli a1, 7\nadd a2, a0, a1\nhere:\nj here\n";
        let (segs, entry) = parse_program("t", src.as_bytes()).unwrap();
        let mut c = Computer::new(Computer::circuit_engine(&lib).unwrap());
        c.load_program(segs, entry).unwrap();
        c.breakpoints.insert(8);
        c.running = true;
        c.run_for(Duration::from_millis(50));
        assert!(!c.running, "stopped at the breakpoint");
        assert_eq!(c.pc(), 8);
        let x = c.registers().unwrap();
        assert_eq!((x[10], x[11], x[12]), (5, 7, 0));
        c.step_n(1).unwrap();
        assert_eq!(c.registers().unwrap()[12], 12);
        assert!(c.shadow_lost.is_none());
    }
}
