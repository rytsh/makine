//! Example worlds, from a single gate to small machines. Each is built
//! from gates and devices only, so it opens in any profile, and each is
//! tested to do what its description says.

use super::Project;
use crate::{Device, Edit, End, Gate, Id, Kind, Module, Placed, Pos, WireStyle, World};
use makine_lang::tr;
use makine_sim::{Part, PartKind};

/// How big an example is, for sorting and the label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    Small,
    Medium,
    Large,
}

pub struct Example {
    /// Stable id (file-name-like).
    pub id: &'static str,
    pub size: Size,
    pub build: fn() -> Project,
}

impl Example {
    pub fn title(&self) -> &'static str {
        match self.id {
            "lamp" => tr!("Anahtar ve lamba", "Switch and lamp"),
            "gates" => tr!("Kapı tezgâhı", "Gate bench"),
            "half_adder" => tr!("Yarım toplayıcı", "Half adder"),
            "blinker" => tr!("Yanıp sönen lamba", "Blinking lamp"),
            "sr_latch" => tr!("SR mandal: ilk hafıza", "SR latch: the first memory"),
            "counter" => tr!("4-bit sayaç", "4-bit counter"),
            "adder4" => tr!("4-bit toplama makinesi", "4-bit adding machine"),
            "traffic" => tr!("Trafik lambası", "Traffic light"),
            "ram_bench" => tr!("RAM tezgâhı", "RAM bench"),
            "byte_ram_bench" => tr!("Bayt RAM'i: izin ve saat", "Byte RAM: enable and clock"),
            "rom_bench" => tr!("ROM tablosunu oku", "Read a ROM table"),
            "program_rom" => tr!(
                "Program ROM'u: adres, opcode, argüman",
                "Program ROM: address, opcode, operand"
            ),
            "screen_line" => tr!("Ekrana çizgi", "A line on the screen"),
            "typewriter" => tr!("Daktilo", "Typewriter"),
            "gpu_triangle" => tr!("GPU: üçgen çiz", "GPU: draw a triangle"),
            "gpu_scene" => tr!("GPU: renkli sahne", "GPU: a colour scene"),
            _ => self.id,
        }
    }

    pub fn about(&self) -> &'static str {
        match self.id {
            "lamp" => tr!(
                "En basit devre: anahtara tıkla, lamba yansın.",
                "The simplest circuit: click the switch and the lamp lights."
            ),
            "gates" => tr!(
                "İki anahtar altı kapıya bağlı: her kapının ne yaptığını dene.",
                "Two switches wired to six gates: try what each gate does."
            ),
            "half_adder" => tr!(
                "XOR toplamı, AND eldeyi verir: 1 + 1 = 10.",
                "XOR gives the sum, AND the carry: 1 + 1 = 10."
            ),
            "blinker" => tr!(
                "Saat kendi kendine 0 ve 1 olur; NOT ile iki lamba sırayla yanar.",
                "The clock turns 0 and 1 on its own; with a NOT, two lamps take turns."
            ),
            "sr_latch" => tr!(
                "Çaprazlama bağlı iki NOR: set'e bir kez bas, lamba yanık kalır. Hafıza böyle doğar.",
                "Two NORs wired crosswise: press set once and the lamp stays on. This is how memory begins."
            ),
            "counter" => tr!(
                "Bir yazmaç ve kapılardan bir +1 devresi: her saat vuruşunda sayı bir artar.",
                "A register and a +1 circuit made of gates: the number goes up on every clock tick."
            ),
            "adder4" => tr!(
                "Dört tam toplayıcı zincir halinde: a ve b'yi değiştir, toplamı göstergede gör.",
                "Four full adders in a chain: change a and b, see the sum on the display."
            ),
            "traffic" => tr!(
                "2 bitlik bir sayaç ve kod çözücü: kırmızı, kırmızı-sarı, yeşil, sarı.",
                "A 2-bit counter and a decoder: red, red-amber, green, amber."
            ),
            "ram_bench" => tr!(
                "16 kelimelik bir RAM: adresi ve veriyi ayarla, yaz'a bas, sonra geri oku.",
                "A 16-word RAM: set the address and data, press write, then read it back."
            ),
            "byte_ram_bench" => tr!(
                "4×8-bit RAM: we ve clk ayrı. Yazma izni açıkken saat 0→1 olunca tek hücreye yazılır. Adresi değiştirip geri oku.",
                "4×8-bit RAM with separate we and clk. A 0→1 clock writes one cell only when enabled. Change the address to read back."
            ),
            "rom_bench" => tr!(
                "Hazır ROM aygıtında 0x12,0xA5,0x3C,0x80 tablosunu incele. Adresi değiştir; okuma saat beklemez. Seviyede aynı davranışı kapılardan kurarsın.",
                "Inspect 0x12,0xA5,0x3C,0x80 in a ROM device. Change addresses; reads need no clock. In the level you build the same behaviour from gates."
            ),
            "program_rom" => tr!(
                "16×8-bit düzenlenebilir ROM: komut baytını opcode ve argümana ayır. Mini taslağını bağlayıp yükleyebilirsin. Bu tezgâh komut gösterir, CPU gibi yürütmez.",
                "Editable 16×8-bit ROM: split each instruction into opcode and operand. Link and upload a Mini draft. This bench displays instructions; it does not execute them as a CPU."
            ),
            "screen_line" => tr!(
                "Bir sayaç ekranın köşegenini çizer: x ve y aynı sayaçtan gelir.",
                "A counter draws the screen's diagonal: x and y come from the same counter."
            ),
            "typewriter" => tr!(
                "Klavyeyi seçip yaz: her tuşun kodu göstergede çıkar ve RAM'e sırayla yazılır.",
                "Select the keyboard and type: each key's code shows on the display and is written to RAM in turn."
            ),
            "gpu_triangle" => tr!(
                "Senin rasterleyicin (Fill) ekranı tarar ve üçgeni çizer. Köşelerin girişlerine tıklayıp üçgeni değiştir.",
                "Your rasterizer (Fill) scans the screen and draws the triangle. Click the corners' inputs to change it."
            ),
            "gpu_scene" => tr!(
                "Senin sahne rasterleyicin (Raster3) dört renkli üçgeni sırayla çizer: köşeler, renk ve yakınlık ROM'larda, derinlik tamponu bir RAM'de. Öndeki üçgen hep üstte kalır. ROM'ları değiştirip sahneyi yeniden kur.",
                "Your scene rasterizer (Raster3) draws four coloured triangles in turn: corners, colour and nearness in ROMs, the depth buffer in a RAM. The front triangle always stays on top. Change the ROMs to build a new scene."
            ),
            _ => "",
        }
    }
}

pub fn all() -> Vec<Example> {
    let e = |id, size, build| Example { id, size, build };
    vec![
        e("lamp", Size::Small, lamp),
        e("gates", Size::Small, gates),
        e("half_adder", Size::Small, half_adder),
        e("blinker", Size::Small, blinker),
        e("sr_latch", Size::Medium, sr_latch),
        e("counter", Size::Medium, counter),
        e("adder4", Size::Medium, adder4),
        e("traffic", Size::Medium, traffic),
        e("ram_bench", Size::Large, ram_bench),
        e("byte_ram_bench", Size::Small, byte_ram_bench),
        e("rom_bench", Size::Small, rom_bench),
        e("program_rom", Size::Small, program_rom),
        e("screen_line", Size::Large, screen_line),
        e("typewriter", Size::Large, typewriter),
    ]
}

/// Examples that use a chip the player builds; they open once it is solved.
pub fn with_chips() -> Vec<(Example, &'static str)> {
    vec![
        (
            Example {
                id: "gpu_triangle",
                size: Size::Large,
                build: gpu_triangle,
            },
            "Fill",
        ),
        (
            Example {
                id: "gpu_scene",
                size: Size::Large,
                build: gpu_scene,
            },
            "Raster3",
        ),
    ]
}

/// Places parts and wires with fresh ids.
struct B {
    w: World,
    next: Id,
    watches: Vec<End>,
}

impl B {
    fn new(name: &str) -> Self {
        Self {
            w: World::new(name),
            next: 1,
            watches: Vec::new(),
        }
    }

    fn put(&mut self, kind: Kind, x: f32, y: f32) -> Id {
        let id = self.next;
        self.next += 1;
        self.w.apply(Edit::Part {
            id,
            part: Some(Placed {
                kind: kind.clone(),
                pos: Pos::new(x, y),
            }),
        });
        assert!(self.w.part(id).is_some(), "refused: {kind:?}");
        id
    }

    fn gate(&mut self, g: Gate, x: f32, y: f32) -> Id {
        self.put(Kind::Gate(g), x, y)
    }

    fn switch(&mut self, x: f32, y: f32) -> Id {
        self.put(Kind::Switch { on: false }, x, y)
    }

    fn lamp(&mut self, x: f32, y: f32) -> Id {
        self.put(Kind::Lamp, x, y)
    }

    fn input(&mut self, name: &str, width: u8, value: u64, x: f32, y: f32) -> Id {
        self.put(
            Kind::Input {
                name: name.into(),
                width,
                value,
            },
            x,
            y,
        )
    }

    fn output(&mut self, name: &str, width: u8, x: f32, y: f32) -> Id {
        self.put(
            Kind::Output {
                name: name.into(),
                width,
            },
            x,
            y,
        )
    }

    fn constant(&mut self, width: u8, value: u64, x: f32, y: f32) -> Id {
        self.put(
            Kind::Part(Part::new(PartKind::Const { width, value })),
            x,
            y,
        )
    }

    fn split(&mut self, widths: Vec<u8>, x: f32, y: f32) -> Id {
        self.put(Kind::Part(Part::new(PartKind::Split(widths))), x, y)
    }

    fn join(&mut self, widths: Vec<u8>, x: f32, y: f32) -> Id {
        self.put(Kind::Part(Part::new(PartKind::Join(widths))), x, y)
    }

    fn device(&mut self, d: Device, x: f32, y: f32) -> Id {
        self.put(Kind::Device(d), x, y)
    }

    /// Wire output `from.1` of part `from.0` into input `to.1` of `to.0`.
    fn wire(&mut self, from: (Id, usize), to: (Id, usize)) {
        self.w.apply(Edit::Wire {
            to: End::new(to.0, to.1),
            from: Some(End::new(from.0, from.1)),
        });
    }

    fn named(&mut self, from: (Id, usize), to: (Id, usize), name: &str) {
        self.wire(from, to);
        self.w.apply(Edit::Style {
            to: End::new(to.0, to.1),
            style: Some(WireStyle {
                name: name.into(),
                points: vec![],
            }),
        });
    }

    fn module(&mut self, name: &str, members: Vec<Id>) -> Id {
        let id = self.next;
        self.next += 1;
        self.w.apply(Edit::Module {
            id,
            module: Some(Module {
                name: name.into(),
                pos: self.w.corner(&members),
                members,
            }),
        });
        id
    }

    fn watch(&mut self, id: Id, pin: usize) {
        self.watches.push(End::new(id, pin));
    }

    fn done(self, collapsed: Vec<Id>) -> Project {
        Project {
            world: self.w,
            chips: vec![],
            watches: self.watches,
            collapsed,
        }
    }
}

fn lamp() -> Project {
    let mut b = B::new(tr!("Anahtar ve lamba", "Switch and lamp"));
    let s = b.switch(60.0, 100.0);
    let l = b.lamp(320.0, 100.0);
    b.wire((s, 0), (l, 0));
    b.done(vec![])
}

fn gates() -> Project {
    let mut b = B::new(tr!("Kapı tezgâhı", "Gate bench"));
    let a = b.switch(40.0, 200.0);
    let c = b.switch(40.0, 320.0);
    let gs = [Gate::And, Gate::Or, Gate::Xor, Gate::Nand, Gate::Nor];
    for (k, g) in gs.into_iter().enumerate() {
        let y = 40.0 + k as f32 * 100.0;
        let id = b.gate(g, 260.0, y);
        b.wire((a, 0), (id, 0));
        b.wire((c, 0), (id, 1));
        let l = b.lamp(440.0, y + 4.0);
        b.wire((id, 0), (l, 0));
    }
    let n = b.gate(Gate::Not, 260.0, 540.0);
    b.wire((a, 0), (n, 0));
    let l = b.lamp(440.0, 544.0);
    b.wire((n, 0), (l, 0));
    b.done(vec![])
}

fn half_adder() -> Project {
    let mut b = B::new(tr!("Yarım toplayıcı", "Half adder"));
    let a = b.switch(40.0, 60.0);
    let c = b.switch(40.0, 200.0);
    let x = b.gate(Gate::Xor, 240.0, 60.0);
    let n = b.gate(Gate::And, 240.0, 200.0);
    for g in [x, n] {
        b.wire((a, 0), (g, 0));
        b.wire((c, 0), (g, 1));
    }
    let sum = b.output(tr!("toplam", "sum"), 1, 440.0, 60.0);
    let carry = b.output(tr!("elde", "carry"), 1, 440.0, 200.0);
    b.wire((x, 0), (sum, 0));
    b.wire((n, 0), (carry, 0));
    b.done(vec![])
}

fn blinker() -> Project {
    let mut b = B::new(tr!("Yanıp sönen lamba", "Blinking lamp"));
    let clk = b.put(Kind::Clock { period: 20 }, 40.0, 100.0);
    let l1 = b.lamp(360.0, 40.0);
    let n = b.gate(Gate::Not, 200.0, 160.0);
    let l2 = b.lamp(360.0, 160.0);
    b.wire((clk, 0), (l1, 0));
    b.wire((clk, 0), (n, 0));
    b.wire((n, 0), (l2, 0));
    b.watch(clk, 0);
    b.done(vec![])
}

fn sr_latch() -> Project {
    let mut b = B::new(tr!("SR mandal", "SR latch"));
    let set = b.input("set", 1, 0, 40.0, 60.0);
    let reset = b.input("reset", 1, 0, 40.0, 240.0);
    let q = b.gate(Gate::Nor, 260.0, 220.0);
    let nq = b.gate(Gate::Nor, 260.0, 80.0);
    // q = NOR(reset, q'), q' = NOR(set, q)
    b.wire((reset, 0), (q, 0));
    b.wire((set, 0), (nq, 0));
    b.named((nq, 0), (q, 1), "q'");
    b.named((q, 0), (nq, 1), "q");
    let l = b.lamp(460.0, 220.0);
    b.wire((q, 0), (l, 0));
    b.watch(q, 0);
    b.done(vec![])
}

/// A ripple +1 on `bits` bits from gates: returns the parts (for a
/// module), the split it reads and the join it drives.
fn incrementer(b: &mut B, bits: u8, x: f32, y: f32) -> (Vec<Id>, Id, Id) {
    let split = b.split(vec![1; bits as usize], x, y);
    let join = b.join(vec![1; bits as usize], x + 520.0, y);
    let mut parts = vec![split, join];
    // bit 0: NOT; bit k: XOR with the carry, carry = AND of the bits below.
    let n0 = b.gate(Gate::Not, x + 180.0, y);
    b.wire((split, 0), (n0, 0));
    b.wire((n0, 0), (join, 0));
    parts.push(n0);
    let mut carry = (split, 0usize);
    for k in 1..bits as usize {
        let yy = y + k as f32 * 90.0;
        let xr = b.gate(Gate::Xor, x + 340.0, yy);
        b.wire((split, k), (xr, 0));
        b.wire(carry, (xr, 1));
        b.wire((xr, 0), (join, k));
        parts.push(xr);
        if k + 1 < bits as usize {
            let an = b.gate(Gate::And, x + 180.0, yy + 30.0);
            b.wire(carry, (an, 0));
            b.wire((split, k), (an, 1));
            parts.push(an);
            carry = (an, 0);
        }
    }
    (parts, split, join)
}

fn counter() -> Project {
    let mut b = B::new(tr!("4-bit sayaç", "4-bit counter"));
    let clk = b.put(Kind::Clock { period: 15 }, 40.0, 60.0);
    let one = b.constant(1, 1, 40.0, 160.0);
    let zero = b.constant(1, 0, 40.0, 240.0);
    let reg = b.device(Device::Register { bits: 4 }, 240.0, 80.0);
    let (parts, split, join) = incrementer(&mut b, 4, 240.0, 320.0);
    b.wire((one, 0), (reg, 1));
    b.wire((zero, 0), (reg, 2));
    b.wire((clk, 0), (reg, 3));
    b.named((reg, 0), (split, 0), "n");
    b.named((join, 0), (reg, 0), "n + 1");
    let m = b.module("+1", parts);
    let shown = b.device(Device::Display { bits: 4 }, 520.0, 80.0);
    b.wire((reg, 0), (shown, 0));
    b.watch(reg, 0);
    b.done(vec![m])
}

/// A full adder from gates; returns its parts and the ends: inputs a, b, c
/// as (part, pin) pairs to wire into, and the sum and carry outputs.
#[allow(clippy::type_complexity)]
fn full_adder(b: &mut B, x: f32, y: f32) -> (Vec<Id>, [(Id, usize); 3], [(Id, usize); 2]) {
    let x1 = b.gate(Gate::Xor, x, y);
    let x2 = b.gate(Gate::Xor, x + 160.0, y);
    let a1 = b.gate(Gate::And, x, y + 80.0);
    let a2 = b.gate(Gate::And, x + 160.0, y + 80.0);
    let o = b.gate(Gate::Or, x + 320.0, y + 80.0);
    b.wire((x1, 0), (x2, 0));
    b.wire((x1, 0), (a2, 0));
    b.wire((a1, 0), (o, 0));
    b.wire((a2, 0), (o, 1));
    // a and b go to x1 and a1, c to x2 and a2: callers wire the ones not
    // returned.
    (
        vec![x1, x2, a1, a2, o],
        [(x1, 0), (x1, 1), (x2, 1)],
        [(x2, 0), (o, 0)],
    )
}

fn adder4() -> Project {
    let mut b = B::new(tr!("4-bit toplama makinesi", "4-bit adding machine"));
    let a = b.input("a", 4, 5, 40.0, 80.0);
    let c = b.input("b", 4, 9, 40.0, 220.0);
    let sa = b.split(vec![1; 4], 220.0, 60.0);
    let sb = b.split(vec![1; 4], 220.0, 240.0);
    b.wire((a, 0), (sa, 0));
    b.wire((c, 0), (sb, 0));
    let join = b.join(vec![1; 5], 1180.0, 160.0);
    let mut carry: Option<(Id, usize)> = None;
    let mut modules = Vec::new();
    for k in 0..4usize {
        let (parts, ins, outs) = full_adder(&mut b, 420.0, 40.0 + k as f32 * 180.0);
        let (a1, x2, a2) = (parts[2], parts[1], parts[3]);
        b.wire((sa, k), ins[0]);
        b.wire((sb, k), ins[1]);
        b.wire((sa, k), (a1, 0));
        b.wire((sb, k), (a1, 1));
        match carry {
            Some(cin) => {
                b.wire(cin, (x2, 1));
                b.wire(cin, (a2, 1));
            }
            None => {
                let z = b.constant(1, 0, 320.0, 40.0);
                b.wire((z, 0), (x2, 1));
                b.wire((z, 0), (a2, 1));
            }
        }
        b.wire(outs[0], (join, k));
        carry = Some(outs[1]);
        let name = makine_lang::trf!("Tam toplayıcı {k}", "Full adder {k}");
        modules.push(b.module(&name, parts));
    }
    b.named(carry.unwrap(), (join, 4), tr!("elde", "carry"));
    let shown = b.device(Device::Display { bits: 5 }, 1360.0, 160.0);
    b.named((join, 0), (shown, 0), "a + b");
    let out = b.output(tr!("toplam", "sum"), 5, 1360.0, 300.0);
    b.wire((join, 0), (out, 0));
    b.done(modules)
}

fn traffic() -> Project {
    let mut b = B::new(tr!("Trafik lambası", "Traffic light"));
    let clk = b.put(Kind::Clock { period: 40 }, 40.0, 60.0);
    let one = b.constant(1, 1, 40.0, 160.0);
    let zero = b.constant(1, 0, 40.0, 240.0);
    let reg = b.device(Device::Register { bits: 2 }, 240.0, 80.0);
    let (parts, split, join) = incrementer(&mut b, 2, 240.0, 320.0);
    b.wire((one, 0), (reg, 1));
    b.wire((zero, 0), (reg, 2));
    b.wire((clk, 0), (reg, 3));
    b.named((reg, 0), (split, 0), tr!("evre", "phase"));
    b.wire((join, 0), (reg, 0));
    let m = b.module("+1", parts);
    // Phases: 0 red, 1 red+amber, 2 green, 3 amber.
    // red = NOT b1; amber = b0; green = b1 AND NOT b0.
    let bits = b.split(vec![1, 1], 520.0, 80.0);
    b.wire((reg, 0), (bits, 0));
    let red_g = b.gate(Gate::Not, 700.0, 40.0);
    b.wire((bits, 1), (red_g, 0));
    let n0 = b.gate(Gate::Not, 700.0, 240.0);
    b.wire((bits, 0), (n0, 0));
    let green_g = b.gate(Gate::And, 860.0, 240.0);
    b.wire((bits, 1), (green_g, 0));
    b.wire((n0, 0), (green_g, 1));
    let red = b.output(tr!("kırmızı", "red"), 1, 1040.0, 40.0);
    let amber = b.output(tr!("sarı", "amber"), 1, 1040.0, 140.0);
    let green = b.output(tr!("yeşil", "green"), 1, 1040.0, 240.0);
    b.wire((red_g, 0), (red, 0));
    b.wire((bits, 0), (amber, 0));
    b.wire((green_g, 0), (green, 0));
    let l1 = b.lamp(1200.0, 40.0);
    let l2 = b.lamp(1200.0, 140.0);
    let l3 = b.lamp(1200.0, 240.0);
    b.wire((red_g, 0), (l1, 0));
    b.wire((bits, 0), (l2, 0));
    b.wire((green_g, 0), (l3, 0));
    b.watch(reg, 0);
    b.done(vec![m])
}

fn ram_bench() -> Project {
    let mut b = B::new(tr!("RAM tezgâhı", "RAM bench"));
    let addr = b.input(tr!("adres", "address"), 4, 0, 40.0, 40.0);
    let data = b.input(tr!("veri", "data"), 8, 42, 40.0, 160.0);
    let write = b.input(tr!("yaz", "write"), 1, 0, 40.0, 280.0);
    let ram = b.device(Device::Ram { addr: 4, data: 8 }, 300.0, 120.0);
    let one = b.constant(1, 1, 40.0, 400.0);
    b.wire((addr, 0), (ram, 0));
    b.wire((data, 0), (ram, 1));
    b.wire((one, 0), (ram, 2));
    // The write input is the clock: going 0 → 1 writes.
    b.named((write, 0), (ram, 3), "clk");
    let shown = b.device(Device::Display { bits: 8 }, 560.0, 120.0);
    b.named((ram, 0), (shown, 0), "q");
    b.done(vec![])
}

fn byte_ram_bench() -> Project {
    let mut b = B::new(tr!(
        "Bayt RAM'i: izin ve saat",
        "Byte RAM: enable and clock"
    ));
    let addr = b.input("addr", 2, 0, 40.0, 40.0);
    let data = b.input("d", 8, 0xA5, 40.0, 160.0);
    let we = b.input("we", 1, 0, 40.0, 280.0);
    let clk = b.input("clk", 1, 0, 40.0, 400.0);
    let ram = b.device(Device::Ram { addr: 2, data: 8 }, 320.0, 160.0);
    for (pin, input) in [addr, data, we, clk].into_iter().enumerate() {
        b.named((input, 0), (ram, pin), ["addr", "d", "we", "clk"][pin]);
    }
    let display = b.device(Device::Display { bits: 8 }, 600.0, 160.0);
    b.named((ram, 0), (display, 0), "q");
    b.watch(ram, 0);
    b.done(vec![])
}

fn rom_bench() -> Project {
    let mut b = B::new(tr!("ROM tablosunu oku", "Read a ROM table"));
    let addr = b.input("addr", 2, 0, 40.0, 80.0);
    let rom = b.device(
        Device::Rom {
            addr: 2,
            data: 8,
            words: vec![0x12, 0xA5, 0x3C, 0x80],
        },
        280.0,
        80.0,
    );
    let display = b.device(Device::Display { bits: 8 }, 560.0, 80.0);
    b.named((addr, 0), (rom, 0), "addr");
    b.named((rom, 0), (display, 0), "q");
    b.watch(rom, 0);
    b.done(vec![])
}

fn program_rom() -> Project {
    let mut b = B::new(tr!(
        "Program ROM'u: adres, opcode, argüman",
        "Program ROM: address, opcode, operand"
    ));
    let addr = b.input("addr", 4, 0, 40.0, 80.0);
    let rom = b.device(
        Device::Rom {
            addr: 4,
            data: 8,
            words: vec![0x13, 0x30, 0x40, 0x31, 0xF0],
        },
        280.0,
        80.0,
    );
    let instr = b.device(Device::Display { bits: 8 }, 560.0, 40.0);
    let split = b.split(vec![4, 4], 560.0, 200.0);
    let arg = b.output("arg", 4, 800.0, 200.0);
    let op = b.output("op", 4, 800.0, 320.0);
    b.named((addr, 0), (rom, 0), "pc");
    b.named((rom, 0), (instr, 0), "instr");
    b.wire((rom, 0), (split, 0));
    b.named((split, 0), (arg, 0), "arg");
    b.named((split, 1), (op, 0), "op");
    b.watch(rom, 0);
    b.done(vec![])
}

fn screen_line() -> Project {
    let mut b = B::new(tr!("Ekrana çizgi", "A line on the screen"));
    let clk = b.put(Kind::Clock { period: 2 }, 40.0, 60.0);
    let one = b.constant(1, 1, 40.0, 160.0);
    let zero = b.constant(1, 0, 40.0, 240.0);
    let reg = b.device(Device::Register { bits: 5 }, 240.0, 80.0);
    let (parts, split, join) = incrementer(&mut b, 5, 240.0, 320.0);
    b.wire((one, 0), (reg, 1));
    b.wire((zero, 0), (reg, 2));
    b.wire((clk, 0), (reg, 3));
    b.named((reg, 0), (split, 0), "n");
    b.wire((join, 0), (reg, 0));
    let m = b.module("+1", parts);
    let screen = b.device(Device::Screen { bits: 5 }, 900.0, 60.0);
    b.named((reg, 0), (screen, 0), "x");
    b.named((reg, 0), (screen, 1), "y");
    b.wire((one, 0), (screen, 2));
    b.wire((one, 0), (screen, 3));
    // Draw while the clock is low, so the pixel is set before n moves on.
    let nclk = b.gate(Gate::Not, 700.0, 300.0);
    b.wire((clk, 0), (nclk, 0));
    b.wire((nclk, 0), (screen, 4));
    b.watch(reg, 0);
    b.done(vec![m])
}

fn typewriter() -> Project {
    let mut b = B::new(tr!("Daktilo", "Typewriter"));
    let kbd = b.device(Device::Keyboard, 40.0, 80.0);
    let clk = b.put(Kind::Clock { period: 4 }, 40.0, 300.0);
    let one = b.constant(1, 1, 40.0, 400.0);
    let zero = b.constant(1, 0, 40.0, 480.0);
    // A step happens when a key is ready and the clock rises: write the
    // key at the cursor, move the cursor, take the next key.
    let go = b.gate(Gate::And, 260.0, 300.0);
    b.wire((kbd, 1), (go, 0));
    b.wire((clk, 0), (go, 1));
    let cursor = b.device(Device::Register { bits: 4 }, 480.0, 420.0);
    let (parts, split, join) = incrementer(&mut b, 4, 480.0, 620.0);
    b.wire((join, 0), (cursor, 0));
    b.wire((one, 0), (cursor, 1));
    b.wire((zero, 0), (cursor, 2));
    b.named((go, 0), (cursor, 3), tr!("yaz", "write"));
    b.named((cursor, 0), (split, 0), tr!("imleç", "cursor"));
    let m = b.module("+1", parts);
    let ram = b.device(Device::Ram { addr: 4, data: 7 }, 900.0, 80.0);
    b.wire((cursor, 0), (ram, 0));
    b.named((kbd, 0), (ram, 1), tr!("tuş", "key"));
    b.wire((one, 0), (ram, 2));
    b.wire((go, 0), (ram, 3));
    b.wire((go, 0), (kbd, 0));
    let key = b.device(Device::Display { bits: 7 }, 260.0, 80.0);
    b.wire((kbd, 0), (key, 0));
    let pos = b.device(Device::Display { bits: 4 }, 900.0, 420.0);
    b.wire((cursor, 0), (pos, 0));
    b.watch(cursor, 0);
    b.done(vec![m])
}

/// The player's Fill chip drawing a triangle on a 32 × 32 screen. The
/// screen draws while the clock is low, so a pixel is set before the scan
/// moves on.
fn gpu_triangle() -> Project {
    let mut b = B::new(tr!("GPU: üçgen", "GPU: triangle"));
    let corners = [
        ("ax", 2),
        ("ay", 3),
        ("bx", 29),
        ("by", 8),
        ("cx", 9),
        ("cy", 27),
    ];
    let mut ins = Vec::new();
    for (k, (name, v)) in corners.iter().enumerate() {
        ins.push(b.input(name, 5, *v, 40.0, 40.0 + k as f32 * 80.0));
    }
    let clk = b.put(Kind::Clock { period: 1 }, 40.0, 560.0);
    let reset = b.input("reset", 1, 0, 40.0, 640.0);
    let fill = b.put(
        Kind::Part(Part::new(PartKind::Chip("Fill".into()))),
        320.0,
        200.0,
    );
    for (k, id) in ins.iter().enumerate() {
        b.wire((*id, 0), (fill, k));
    }
    b.wire((reset, 0), (fill, 6));
    b.wire((clk, 0), (fill, 7));
    let screen = b.device(Device::Screen { bits: 5 }, 760.0, 60.0);
    b.named((fill, 0), (screen, 0), "x");
    b.named((fill, 1), (screen, 1), "y");
    b.named((fill, 2), (screen, 2), tr!("içeride", "inside"));
    let one = b.constant(1, 1, 560.0, 420.0);
    b.wire((one, 0), (screen, 3));
    let nclk = b.gate(Gate::Not, 560.0, 520.0);
    b.wire((clk, 0), (nclk, 0));
    b.wire((nclk, 0), (screen, 4));
    b.watch(fill, 2);
    b.done(vec![])
}

/// A triangle of a scene: corners (ax, ay, bx, by, cx, cy), nearness and
/// 12-bit colour.
pub type SceneTriangle = ([u64; 6], u64, u64);

/// The scene of the multi-triangle GPU levels: for each triangle the
/// corners, nearness and 12-bit colour. Kept here so World does not depend
/// on the levels; a test checks it is the same.
pub const GPU_SCENE: [SceneTriangle; 4] = [
    ([1, 2, 30, 4, 6, 29], 60, 0x36C),
    ([10, 6, 30, 22, 4, 26], 160, 0xE52),
    ([2, 12, 17, 1, 22, 30], 110, 0x4D5),
    ([14, 12, 25, 16, 13, 24], 250, 0xFE3),
];

/// The player's Raster3 drawing a scene of four triangles on a colour
/// screen. Four ROMs hold the scene (indexed by the triangle number t), a
/// RAM is the depth buffer. As in the triangle example, the screen and the
/// buffer are written while the clock is low.
fn gpu_scene() -> Project {
    let mut b = B::new(tr!("GPU: sahne", "GPU: scene"));
    let pack =
        |f: &dyn Fn(&SceneTriangle) -> u64| -> Vec<u64> { GPU_SCENE.iter().map(f).collect() };
    let v = |c: &[u64; 6], k: usize| c[k];
    let roms = [
        (
            tr!("a, bx", "a, bx"),
            15,
            pack(&|t| v(&t.0, 0) | v(&t.0, 1) << 5 | v(&t.0, 2) << 10),
            vec![5u8, 5, 5],
        ),
        (
            tr!("by, c", "by, c"),
            15,
            pack(&|t| v(&t.0, 3) | v(&t.0, 4) << 5 | v(&t.0, 5) << 10),
            vec![5, 5, 5],
        ),
        (tr!("yakınlık", "near"), 8, pack(&|t| t.1), vec![8]),
        (tr!("renk", "colour"), 12, pack(&|t| t.2), vec![12]),
    ];
    let raster = b.put(
        Kind::Part(Part::new(PartKind::Chip("Raster3".into()))),
        560.0,
        240.0,
    );
    let mut pin = 0;
    for (k, (name, data, words, widths)) in roms.into_iter().enumerate() {
        let y = 40.0 + k as f32 * 150.0;
        let rom = b.device(
            Device::Rom {
                addr: 2,
                data,
                words,
            },
            160.0,
            y,
        );
        b.named((raster, 2), (rom, 0), "t");
        if widths.len() == 1 {
            b.named((rom, 0), (raster, pin), name);
            pin += 1;
        } else {
            let s = b.split(widths.clone(), 360.0, y);
            b.named((rom, 0), (s, 0), name);
            for g in 0..widths.len() {
                b.wire((s, g), (raster, pin));
                pin += 1;
            }
        }
    }
    let clk = b.put(Kind::Clock { period: 1 }, 40.0, 660.0);
    let reset = b.input("reset", 1, 0, 40.0, 740.0);
    b.wire((reset, 0), (raster, 9));
    b.wire((clk, 0), (raster, 10));
    let nclk = b.gate(Gate::Not, 760.0, 640.0);
    b.wire((clk, 0), (nclk, 0));

    // The depth buffer: one byte per pixel, at y·32 + x.
    let addr = b.join(vec![5, 5], 860.0, 460.0);
    b.named((raster, 0), (addr, 0), "x");
    b.named((raster, 1), (addr, 1), "y");
    let depth = b.device(Device::Ram { addr: 10, data: 8 }, 1040.0, 420.0);
    b.wire((addr, 0), (depth, 0));
    let near = pin_source(&b, raster, 6);
    b.wire(near, (depth, 1));
    b.named((raster, 3), (depth, 2), tr!("yaz", "write"));
    b.wire((nclk, 0), (depth, 3));
    b.named((depth, 0), (raster, 8), tr!("eski", "old"));

    let screen = b.device(Device::ColorScreen { bits: 5 }, 1040.0, 40.0);
    b.wire((raster, 0), (screen, 0));
    b.wire((raster, 1), (screen, 1));
    b.named((raster, 4), (screen, 2), tr!("renk", "colour"));
    b.wire((raster, 3), (screen, 3));
    b.wire((nclk, 0), (screen, 4));
    b.watch(raster, 2);
    b.watch(raster, 3);
    b.done(vec![])
}

/// Where input `pin` of `part` is wired from.
fn pin_source(b: &B, part: Id, pin: usize) -> (Id, usize) {
    let e = b.w.wire_into(End::new(part, pin)).expect("wired");
    (e.part, e.pin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use makine_sim::Library;

    #[test]
    fn every_example_builds_without_problems() {
        let lib = Library::new();
        for e in all() {
            let p = (e.build)();
            let c = compile(&p.world, &lib);
            assert!(c.problems.is_empty(), "{}: {:?}", e.id, c.problems);
            assert!(!e.title().is_empty() && !e.about().is_empty(), "{}", e.id);
            // Every input pin is wired, except keyboards and the like.
            for (id, part) in p.world.parts() {
                let Some(pins) = part.kind.pins(&lib) else {
                    panic!("{}: part {id} has no pins", e.id)
                };
                for k in 0..pins.inputs.len() {
                    assert!(
                        p.world.wire_into(End::new(id, k)).is_some(),
                        "{}: part {id} ({}) input {k} is not wired",
                        e.id,
                        part.kind.label()
                    );
                }
            }
        }
    }
}
