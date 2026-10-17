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
                return Err(format!("program belleğe sığmıyor (0x{addr:08X})"));
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
                    return Err("devre işlemcisi 0 adresinden başlar".into());
                }
                cpu.reset()?;
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
        while start.elapsed() < budget {
            let chunk = match self.engine {
                Engine::Circuit(_) => 64,
                Engine::Reference(_) => 100_000,
            };
            if let Err(e) = self.step_n(chunk) {
                self.halted = Some(e);
                self.running = false;
                break;
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
                        cpu.step(&mut self.machine).map(|_| ())
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
                format!("Durdu: {h}"),
                egui::FontId::proportional(16.0),
                Color32::LIGHT_RED,
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
            ui.label(format!("{} komut", self.instructions));
            if self.running {
                ui.label(human_speed(self.speed));
            }
        });
        if let Some(h) = &self.halted {
            let color = if h.starts_with("ebreak") {
                Color32::LIGHT_GREEN
            } else {
                Color32::LIGHT_RED
            };
            ui.colored_label(color, format!("Durdu: {h}"));
        }
        ui.label(RichText::new("Konsol").strong());
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
    let src = std::str::from_utf8(bytes).map_err(|_| format!("{name}: metin ya da ELF değil"))?;
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
}
