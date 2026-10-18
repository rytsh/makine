//! Devices: memory, screens, keyboards and number displays.
//!
//! Each device is one part with bus pins, like a chip in the level editor:
//! a RAM has `a[addr]`, `d[data]`, `load`, `clk` in and `q[data]` out.
//! Devices with behaviour run as native parts in the simulator; a number
//! display only reads its input.

use std::collections::VecDeque;

use makine_lang::{tr, trf};
use makine_sim::{Behavior, Circuit, Clocked, NativeDef, Pin};
use serde::{Deserialize, Serialize};

pub const MAX_ADDR: u8 = 12;
pub const MAX_DATA: u8 = 16;
pub const MAX_SCREEN_BITS: u8 = 5;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Device {
    /// `2^addr` words of `data` bits. Writes `d` to address `a` on a rising
    /// `clk` while `load` is 1; always outputs the word at `a`.
    Ram { addr: u8, data: u8 },
    /// Fixed words: outputs `words[a]` (missing words read 0).
    Rom { addr: u8, data: u8, words: Vec<u64> },
    /// A `2^bits` × `2^bits` black and white screen. On a rising `clk` while
    /// `load` is 1, sets pixel (`x`, `y`) to `d`.
    Screen { bits: u8 },
    /// Keys typed by the player, in a queue. Outputs the oldest key's code
    /// (7 bits) and `ready` while the queue is not empty; a rising `next`
    /// removes it.
    Keyboard,
    /// Shows the number on its input in hexadecimal and decimal.
    Display { bits: u8 },
    /// Edge-triggered storage. Arithmetic and instruction decoding remain
    /// ordinary gates outside this primitive.
    Register { bits: u8 },
    /// 4 KiB program storage with two independent read addresses. `next`
    /// reads (a+1) modulo 4096; `indirect` reads b (for table lookup).
    ProgramRom { words: Vec<u64> },
}

fn pins(specs: &[(&str, u8)]) -> Vec<Pin> {
    specs.iter().map(|(n, w)| Pin::new(*n, *w)).collect()
}

impl Device {
    pub fn name(&self) -> String {
        match self {
            Device::Ram { .. } => "RAM".into(),
            Device::Rom { .. } => "ROM".into(),
            Device::Screen { .. } => tr!("Ekran", "Screen").into(),
            Device::Keyboard => tr!("Klavye", "Keyboard").into(),
            Device::Display { .. } => tr!("Gösterge", "Display").into(),
            Device::Register { .. } => tr!("Yazmaç", "Register").into(),
            Device::ProgramRom { .. } => "Program ROM".into(),
        }
    }

    /// What the pins do, for the player.
    pub fn help(&self) -> &'static str {
        match self {
            Device::Ram { .. } => tr!(
                "a: adres, d: yazılacak veri. clk 0'dan 1'e geçerken load 1 ise d, a \
                 adresine yazılır. q: her an a adresindeki veri.",
                "a: address, d: data to write. When clk goes from 0 to 1 and load is 1, d \
                 is written to address a. q: the data at address a, always."
            ),
            Device::Rom { .. } => tr!(
                "a: adres. q: o adresteki sabit veri. İçeriği aşağıya yaz.",
                "a: address. q: the fixed data at that address. Write the contents below."
            ),
            Device::Screen { .. } => tr!(
                "x, y: piksel. clk 0'dan 1'e geçerken load 1 ise (x, y) pikseli d olur \
                 (1: yanık).",
                "x, y: pixel. When clk goes from 0 to 1 and load is 1, pixel (x, y) \
                 becomes d (1: on)."
            ),
            Device::Keyboard => tr!(
                "Seçip yazınca tuşlar kuyruğa girer. k: en eski tuşun ASCII kodu, ready: \
                 kuyrukta tuş var. next 0'dan 1'e geçince sıradaki tuşa geçilir.",
                "Select it and type: keys go into a queue. k: ASCII code of the oldest key, \
                 ready: the queue has a key. When next goes from 0 to 1, it moves to the next key."
            ),
            Device::Display { .. } => tr!(
                "n girişindeki sayıyı onaltılık ve onluk gösterir.",
                "Shows the number on input n in hexadecimal and decimal."
            ),
            Device::Register { .. } => tr!(
                "clk yükselen kenarında reset 1 ise sıfırlanır; değilse load 1 ise d saklanır. q saklanan değerdir.",
                "On a rising clk edge: if reset is 1 it clears; otherwise if load is 1 it stores d. q is the stored value."
            ),
            Device::ProgramRom { .. } => tr!(
                "4096 bayt program belleği. a: komut adresi, b: dolaylı adres. q: komut, next: sonraki bayt, indirect: b adresindeki bayt. Bu bellek işlemci değildir.",
                "4096 bytes of program memory. a: instruction address, b: indirect address. q: instruction, next: the next byte, indirect: the byte at address b. This memory is not a processor."
            ),
        }
    }

    /// Short ASCII id, used to name the device's native part.
    fn id(&self) -> &'static str {
        match self {
            Device::Ram { .. } => "ram",
            Device::Rom { .. } => "rom",
            Device::Screen { .. } => "screen",
            Device::Keyboard => "kbd",
            Device::Display { .. } => "display",
            Device::Register { .. } => "register",
            Device::ProgramRom { .. } => "program",
        }
    }

    /// The same device with its sizes clamped to what is supported.
    pub fn clamped(&self) -> Device {
        let c = |v: u8, max: u8| v.clamp(1, max);
        match self {
            Device::Ram { addr, data } => Device::Ram {
                addr: c(*addr, MAX_ADDR),
                data: c(*data, MAX_DATA),
            },
            Device::Rom { addr, data, words } => {
                let (addr, data) = (c(*addr, MAX_ADDR), c(*data, MAX_DATA));
                let mask = makine_sim::mask(data);
                let mut words: Vec<u64> = words.iter().map(|w| w & mask).collect();
                words.truncate(1 << addr);
                Device::Rom { addr, data, words }
            }
            Device::Screen { bits } => Device::Screen {
                bits: (*bits).clamp(2, MAX_SCREEN_BITS),
            },
            Device::Keyboard => Device::Keyboard,
            Device::Display { bits } => Device::Display {
                bits: c(*bits, MAX_DATA),
            },
            Device::Register { bits } => Device::Register {
                bits: (*bits).clamp(1, 64),
            },
            Device::ProgramRom { words } => Device::ProgramRom {
                words: words.iter().take(4096).map(|w| w & 255).collect(),
            },
        }
    }

    pub fn inputs(&self) -> Vec<Pin> {
        match self.clamped() {
            Device::Ram { addr, data } => {
                pins(&[("a", addr), ("d", data), ("load", 1), ("clk", 1)])
            }
            Device::Rom { addr, .. } => pins(&[("a", addr)]),
            Device::Screen { bits } => {
                pins(&[("x", bits), ("y", bits), ("d", 1), ("load", 1), ("clk", 1)])
            }
            Device::Keyboard => pins(&[("next", 1)]),
            Device::Display { bits } => pins(&[("n", bits)]),
            Device::Register { bits } => {
                pins(&[("d", bits), ("load", 1), ("reset", 1), ("clk", 1)])
            }
            Device::ProgramRom { .. } => pins(&[("a", 12), ("b", 12)]),
        }
    }

    pub fn outputs(&self) -> Vec<Pin> {
        match self.clamped() {
            Device::Ram { data, .. } | Device::Rom { data, .. } => pins(&[("q", data)]),
            Device::Keyboard => pins(&[("k", 7), ("ready", 1)]),
            Device::Screen { .. } | Device::Display { .. } => Vec::new(),
            Device::Register { bits } => pins(&[("q", bits)]),
            Device::ProgramRom { .. } => pins(&[("q", 8), ("next", 8), ("indirect", 8)]),
        }
    }

    /// The native model, for devices that have behaviour.
    pub fn native(&self) -> Option<NativeDef> {
        let spec = |p: &[Pin]| p.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        let (ins, outs) = (spec(&self.inputs()), spec(&self.outputs()));
        let ins: Vec<&str> = ins.iter().map(String::as_str).collect();
        let outs: Vec<&str> = outs.iter().map(String::as_str).collect();
        match self.clamped() {
            Device::Ram { addr, data } => Some(NativeDef::clocked(
                &ins,
                &outs,
                &["a"],
                RamState {
                    addr,
                    data,
                    words: vec![0; 1 << addr],
                },
                |s, i| {
                    if i[2] & 1 == 1 {
                        s.words[i[0] as usize] = i[1];
                    }
                },
                |s, i, o| o[0] = s.words[i[0] as usize % s.words.len()],
            )),
            Device::Rom { words, .. } => Some(NativeDef::function(&ins, &outs, move |i| {
                vec![words.get(i[0] as usize).copied().unwrap_or(0)]
            })),
            Device::Screen { bits } => Some(NativeDef::clocked(
                &ins,
                &outs,
                &[],
                ScreenState {
                    bits,
                    pixels: vec![false; 1 << (2 * bits)],
                },
                |s, i| {
                    if i[3] & 1 == 1 {
                        let (x, y) = (i[0] as usize, i[1] as usize);
                        s.pixels[y << s.bits | x] = i[2] & 1 == 1;
                    }
                },
                |_, _, _| {},
            )),
            Device::Keyboard => Some(NativeDef::stateful(&ins, &outs, &[], || {
                Box::new(Keyboard::default())
            })),
            Device::Display { .. } => None,
            Device::Register { bits } => Some(NativeDef::clocked(
                &ins,
                &outs,
                &[],
                RegisterState { bits, value: 0 },
                |s, i| {
                    if i[2] == 1 {
                        s.value = 0;
                    } else if i[1] == 1 {
                        s.value = i[0] & makine_sim::mask(s.bits);
                    }
                },
                |s, _, o| o[0] = s.value,
            )),
            Device::ProgramRom { words } => Some(NativeDef::function(&ins, &outs, move |i| {
                let read = |a: u64| words.get((a & 4095) as usize).copied().unwrap_or(0);
                vec![read(i[0]), read(i[0].wrapping_add(1)), read(i[1])]
            })),
        }
    }

    /// Name of this device's native part when it is part number `id`.
    pub fn native_name(&self, id: u64) -> String {
        format!("{}#{id}", self.id())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RamState {
    pub addr: u8,
    pub data: u8,
    pub words: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterState {
    pub bits: u8,
    pub value: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenState {
    pub bits: u8,
    /// Row by row, `2^bits` per row.
    pub pixels: Vec<bool>,
}

#[derive(Default)]
pub struct Keyboard {
    pub queue: VecDeque<u8>,
    edge: makine_sim::ClockEdge,
}

impl Behavior for Keyboard {
    fn eval(&mut self, inputs: &[u64], outputs: &mut [u64]) {
        if self.edge.rising(inputs[0]) {
            self.queue.pop_front();
        }
        let k = self.queue.front().copied();
        outputs[0] = k.map_or(0, u64::from);
        outputs[1] = k.is_some() as u64;
    }

    fn commit(&mut self, inputs: &[u64]) {
        self.edge.commit(inputs, 0);
    }

    fn reset(&mut self) {
        self.queue.clear();
        self.edge = makine_sim::ClockEdge::default();
    }
}

/// Saved contents of a device, carried over when the world is recompiled
/// after an edit, so editing does not wipe memory or the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
    Ram(RamState),
    Screen(ScreenState),
    Keys(VecDeque<u8>),
    Register(RegisterState),
}

/// Contents of the device natively named `name` in `circuit`.
pub fn save(circuit: &mut Circuit, name: &str) -> Option<Saved> {
    if let Some(r) = circuit.native_mut::<Clocked<RegisterState>>(name) {
        return Some(Saved::Register(r.state.clone()));
    }
    if let Some(r) = circuit.native_mut::<Clocked<RamState>>(name) {
        return Some(Saved::Ram(r.state.clone()));
    }
    if let Some(s) = circuit.native_mut::<Clocked<ScreenState>>(name) {
        return Some(Saved::Screen(s.state.clone()));
    }
    let k = circuit.native_mut::<Keyboard>(name)?;
    Some(Saved::Keys(k.queue.clone()))
}

/// Put saved contents back, if the device still has the same shape.
pub fn restore(circuit: &mut Circuit, name: &str, saved: Saved) {
    match saved {
        Saved::Register(s) => {
            if let Some(r) = circuit.native_mut::<Clocked<RegisterState>>(name) {
                if r.state.bits == s.bits {
                    r.state = s;
                }
            }
        }
        Saved::Ram(s) => {
            if let Some(r) = circuit.native_mut::<Clocked<RamState>>(name) {
                if (r.state.addr, r.state.data) == (s.addr, s.data) {
                    r.state = s;
                }
            }
        }
        Saved::Screen(s) => {
            if let Some(r) = circuit.native_mut::<Clocked<ScreenState>>(name) {
                if r.state.bits == s.bits {
                    r.state = s;
                }
            }
        }
        Saved::Keys(q) => {
            if let Some(k) = circuit.native_mut::<Keyboard>(name) {
                k.queue = q;
            }
        }
    }
}

/// The pixels of a screen device.
pub fn screen(circuit: &mut Circuit, name: &str) -> Option<Vec<bool>> {
    Some(
        circuit
            .native_mut::<Clocked<ScreenState>>(name)?
            .state
            .pixels
            .clone(),
    )
}

/// The words of a RAM device.
pub fn ram(circuit: &mut Circuit, name: &str) -> Option<Vec<u64>> {
    Some(
        circuit
            .native_mut::<Clocked<RamState>>(name)?
            .state
            .words
            .clone(),
    )
}

/// Queue a key on a keyboard device. Only 7-bit codes are kept.
pub fn press(circuit: &mut Circuit, name: &str, key: u8) -> bool {
    match circuit.native_mut::<Keyboard>(name) {
        Some(k) if key < 128 => {
            if k.queue.len() < 64 {
                k.queue.push_back(key);
            }
            true
        }
        _ => false,
    }
}

/// Parse ROM contents: numbers separated by spaces, commas or new lines, in
/// hexadecimal (`0x1F` or `1F`) or decimal with a `#` prefix (`#31`). `;`
/// starts a comment.
pub fn parse_words(text: &str) -> Result<Vec<u64>, String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("");
        for tok in line.split([' ', ',', '\t']).filter(|t| !t.is_empty()) {
            let v = if let Some(d) = tok.strip_prefix('#') {
                d.parse::<u64>().ok()
            } else {
                let h = tok
                    .strip_prefix("0x")
                    .or_else(|| tok.strip_prefix("0X"))
                    .unwrap_or(tok);
                u64::from_str_radix(h, 16).ok()
            };
            out.push(v.ok_or_else(|| trf!("'{tok}' bir sayı değil", "'{tok}' is not a number"))?);
        }
    }
    Ok(out)
}

/// ROM contents as text, eight words per line.
pub fn words_text(words: &[u64], data: u8) -> String {
    let digits = (data as usize).div_ceil(4).max(1);
    words
        .chunks(8)
        .map(|c| {
            c.iter()
                .map(|w| format!("{w:0digits$X}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_text() {
        assert_eq!(
            parse_words("1F 0x2, #10 ; yorum\n3").unwrap(),
            vec![0x1F, 2, 10, 3]
        );
        assert!(parse_words("zz").is_err());
        assert_eq!(words_text(&[1, 0xAB], 8), "01 AB");
    }

    #[test]
    fn natives_match_pins() {
        let devices = [
            Device::Ram { addr: 3, data: 5 },
            Device::Rom {
                addr: 2,
                data: 7,
                words: vec![],
            },
            Device::Screen { bits: 3 },
            Device::Keyboard,
        ];
        for d in devices {
            let n = d.native().unwrap();
            assert_eq!(n.inputs, d.inputs(), "{d:?}");
            assert_eq!(n.outputs, d.outputs(), "{d:?}");
        }
        assert!(Device::Display { bits: 4 }.native().is_none());
    }

    #[test]
    fn clamping() {
        assert_eq!(
            Device::Ram { addr: 20, data: 0 }.clamped(),
            Device::Ram { addr: MAX_ADDR, data: 1 }
        );
        let rom = Device::Rom {
            addr: 1,
            data: 4,
            words: vec![0x1F, 2, 3],
        }
        .clamped();
        assert_eq!(
            rom,
            Device::Rom {
                addr: 1,
                data: 4,
                words: vec![0xF, 2]
            }
        );
        assert_eq!(
            Device::Screen { bits: 0 }.clamped(),
            Device::Screen { bits: 2 }
        );
    }
}
