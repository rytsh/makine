//! Device blocks: memory, screens, keyboards and number displays.
//!
//! A device covers a rectangle of cells. Its 1-bit inputs line the back
//! edge (one per cell, read from behind) and its outputs line the front
//! edge (one per cell, driving the cell in front), so devices connect to
//! ordinary wires. Seen from above with the device facing east:
//!
//! ```text
//!   a0 → ┌───────┐ → q0
//!   a1 → │  RAM  │ → q1
//!   d0 → │       │
//!   ...  └───────┘
//! ```
//!
//! Devices with behaviour (memory, screen, keyboard) run as native parts in
//! the simulator; a number display only reads its input wires.

use std::collections::VecDeque;

use makine_sim::{Behavior, Circuit, ClockEdge, Clocked, NativeDef};
use serde::{Deserialize, Serialize};

use crate::block::Dir;
use crate::world::Pos;

pub const MAX_ADDR: u8 = 8;
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
    /// Shows the number on its inputs in hexadecimal and decimal.
    Display { bits: u8 },
}

fn bits(prefix: &str, n: u8) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i}")).collect()
}

/// The value of `n` one-bit inputs starting at `start`, lowest bit first.
fn value(inputs: &[u64], start: usize, n: usize) -> u64 {
    (0..n).fold(0, |v, i| v | (inputs[start + i] & 1) << i)
}

impl Device {
    pub fn name(&self) -> &'static str {
        match self {
            Device::Ram { .. } => "RAM",
            Device::Rom { .. } => "ROM",
            Device::Screen { .. } => "Ekran",
            Device::Keyboard => "Klavye",
            Device::Display { .. } => "Gösterge",
        }
    }

    /// What the pins do, for the player.
    pub fn help(&self) -> &'static str {
        match self {
            Device::Ram { .. } => {
                "a: adres, d: yazılacak veri. clk 0'dan 1'e geçerken load 1 ise d, a \
                 adresine yazılır. q: her an a adresindeki veri."
            }
            Device::Rom { .. } => "a: adres. q: o adresteki sabit veri. İçeriği aşağıya yaz.",
            Device::Screen { .. } => {
                "x, y: piksel. clk 0'dan 1'e geçerken load 1 ise (x, y) pikseli d olur \
                 (1: yanık)."
            }
            Device::Keyboard => {
                "Seçip yazınca tuşlar kuyruğa girer. k: en eski tuşun ASCII kodu, ready: \
                 kuyrukta tuş var. next 0'dan 1'e geçince sıradaki tuşa geçilir."
            }
            Device::Display { .. } => "n girişlerindeki sayıyı onaltılık ve onluk gösterir.",
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
        }
    }

    /// Input pin names, in order along the back edge.
    pub fn inputs(&self) -> Vec<String> {
        let mut v = Vec::new();
        match self {
            Device::Ram { addr, data } => {
                v.extend(bits("a", *addr));
                v.extend(bits("d", *data));
                v.push("load".into());
                v.push("clk".into());
            }
            Device::Rom { addr, .. } => v.extend(bits("a", *addr)),
            Device::Screen { bits: b } => {
                v.extend(bits("x", *b));
                v.extend(bits("y", *b));
                v.push("d".into());
                v.push("load".into());
                v.push("clk".into());
            }
            Device::Keyboard => v.push("next".into()),
            Device::Display { bits: b } => v.extend(bits("n", *b)),
        }
        v
    }

    /// Output pin names, in order along the front edge.
    pub fn outputs(&self) -> Vec<String> {
        match self {
            Device::Ram { data, .. } | Device::Rom { data, .. } => bits("q", *data),
            Device::Keyboard => {
                let mut v = bits("k", 7);
                v.push("ready".into());
                v
            }
            Device::Screen { .. } | Device::Display { .. } => Vec::new(),
        }
    }

    /// Size in cells: (length along the facing direction, width across it).
    pub fn size(&self) -> (i32, i32) {
        let rows = self.inputs().len().max(self.outputs().len()).max(2) as i32;
        match self {
            Device::Screen { .. } => (rows, rows),
            Device::Display { bits } => ((*bits as i32 + 3) / 4 + 2, rows),
            _ => (3, rows),
        }
    }

    /// The native model, for devices that have behaviour.
    pub fn native(&self) -> Option<NativeDef> {
        let ins = self.inputs();
        let outs = self.outputs();
        let ins: Vec<&str> = ins.iter().map(String::as_str).collect();
        let outs: Vec<&str> = outs.iter().map(String::as_str).collect();
        match self.clamped() {
            Device::Ram { addr, data } => {
                let comb: Vec<&str> = ins[..addr as usize].to_vec();
                Some(NativeDef::clocked(
                    &ins,
                    &outs,
                    &comb,
                    RamState {
                        addr,
                        data,
                        words: vec![0; 1 << addr],
                    },
                    |s, i| {
                        let (a, d) = (s.addr as usize, s.data as usize);
                        if i[a + d] & 1 == 1 {
                            let at = value(i, 0, a) as usize;
                            s.words[at] = value(i, a, d);
                        }
                    },
                    |s, i, o| {
                        let w = s.words[value(i, 0, s.addr as usize) as usize];
                        for (j, o) in o.iter_mut().enumerate() {
                            *o = w >> j & 1;
                        }
                    },
                ))
            }
            Device::Rom { addr, words, .. } => {
                let n = outs.len();
                Some(NativeDef::function(&ins, &outs, move |i| {
                    let w = words
                        .get(value(i, 0, addr as usize) as usize)
                        .copied()
                        .unwrap_or(0);
                    (0..n).map(|j| w >> j & 1).collect()
                }))
            }
            Device::Screen { bits } => Some(NativeDef::clocked(
                &ins,
                &outs,
                &[],
                ScreenState {
                    bits,
                    pixels: vec![false; 1 << (2 * bits)],
                },
                |s, i| {
                    let b = s.bits as usize;
                    if i[2 * b + 1] & 1 == 1 {
                        let x = value(i, 0, b) as usize;
                        let y = value(i, b, b) as usize;
                        s.pixels[y << b | x] = i[2 * b] & 1 == 1;
                    }
                },
                |_, _, _| {},
            )),
            Device::Keyboard => Some(NativeDef::stateful(&ins, &outs, &[], || {
                Box::new(Keyboard::default())
            })),
            Device::Display { .. } => None,
        }
    }

    /// Name of this device's native part when placed at `anchor`.
    pub fn native_name(&self, anchor: Pos) -> String {
        format!("{}@{},{},{}", self.id(), anchor.x, anchor.y, anchor.z)
    }
}

/// Where a device's cells and pins are, given its anchor (the back-left
/// corner: first input pin) and facing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub cells: Vec<Pos>,
    /// Input pins: cell and the side wires attach to.
    pub inputs: Vec<(Pos, Dir)>,
    /// Output pins: cell and the side wires attach to.
    pub outputs: Vec<(Pos, Dir)>,
}

pub fn layout(device: &Device, anchor: Pos, facing: Dir) -> Layout {
    let (len, wide) = device.size();
    let (fx, fy) = facing.delta();
    let (sx, sy) = facing.cw().delta();
    let at = |u: i32, v: i32| {
        Pos::new(
            anchor.x + u * fx + v * sx,
            anchor.y + u * fy + v * sy,
            anchor.z,
        )
    };
    let cells = (0..len)
        .flat_map(|u| (0..wide).map(move |v| (u, v)))
        .map(|(u, v)| at(u, v))
        .collect();
    let inputs = (0..device.inputs().len() as i32)
        .map(|v| (at(0, v), facing.opposite()))
        .collect();
    let outputs = (0..device.outputs().len() as i32)
        .map(|v| (at(len - 1, v), facing))
        .collect();
    Layout {
        cells,
        inputs,
        outputs,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RamState {
    pub addr: u8,
    pub data: u8,
    pub words: Vec<u64>,
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
    edge: ClockEdge,
}

impl Behavior for Keyboard {
    fn eval(&mut self, inputs: &[u64], outputs: &mut [u64]) {
        if self.edge.rising(inputs[0]) {
            self.queue.pop_front();
        }
        let k = self.queue.front().copied();
        for (j, o) in outputs[..7].iter_mut().enumerate() {
            *o = k.map_or(0, |k| (k as u64) >> j & 1);
        }
        outputs[7] = k.is_some() as u64;
    }

    fn commit(&mut self, inputs: &[u64]) {
        self.edge.commit(inputs, 0);
    }

    fn reset(&mut self) {
        self.queue.clear();
        self.edge = ClockEdge::default();
    }
}

/// Saved contents of a device, carried over when the world is recompiled
/// after an edit, so editing does not wipe memory or the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
    Ram(RamState),
    Screen(ScreenState),
    Keys(VecDeque<u8>),
}

/// Contents of the device natively named `name` in `circuit`.
pub fn save(circuit: &mut Circuit, name: &str) -> Option<Saved> {
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
            out.push(v.ok_or_else(|| format!("'{tok}' bir sayı değil"))?);
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
    fn layouts_rotate() {
        let d = Device::Rom {
            addr: 2,
            data: 3,
            words: vec![],
        };
        assert_eq!(d.size(), (3, 3));
        let l = layout(&d, Pos::new(0, 0, 0), Dir::East);
        assert_eq!(l.cells.len(), 9);
        assert_eq!(
            l.inputs,
            vec![
                (Pos::new(0, 0, 0), Dir::West),
                (Pos::new(0, 1, 0), Dir::West)
            ]
        );
        assert_eq!(l.outputs[2], (Pos::new(2, 2, 0), Dir::East));
        // Facing north: inputs on the south edge going west... from the
        // anchor, `cw` of north is east, so pins run east along the edge.
        let l = layout(&d, Pos::new(0, 0, 0), Dir::North);
        assert_eq!(l.inputs[1], (Pos::new(1, 0, 0), Dir::South));
        assert_eq!(l.outputs[0], (Pos::new(0, -2, 0), Dir::North));
    }

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
    fn clamping() {
        assert_eq!(
            Device::Ram { addr: 20, data: 0 }.clamped(),
            Device::Ram { addr: 8, data: 1 }
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
    }
}
