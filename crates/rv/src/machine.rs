//! The rest of the computer: RAM and devices on a memory bus.
//!
//! Memory map:
//!
//! | Address        | What                                                      |
//! |----------------|-----------------------------------------------------------|
//! | `0x0000_0000`  | RAM (size chosen at creation); programs load at 0         |
//! | `0x1000_0000`  | console: write a byte to print it; read gives next key    |
//! |                | byte or `u32::MAX` if none                                 |
//! | `0x1000_0004`  | timer: milliseconds since start (read only)               |
//! | `0x1000_0008`  | keyboard: next key event, or 0 (bit 8 = pressed, low 8 bits = key) |
//! | `0x1000_000C`  | screen control: write any value to show the framebuffer   |
//! | `0x1000_0010`  | screen width (read only)                                  |
//! | `0x1000_0014`  | screen height (read only)                                 |
//! | `0x1000_0018`  | random number (read only)                                 |
//! | `0x1100_0000`  | framebuffer: one `0x00RRGGBB` word per pixel, row by row  |

use std::collections::VecDeque;
use std::time::Instant;

use crate::emu::{byte_mask, Memory};

pub const CONSOLE: u32 = 0x1000_0000;
pub const TIMER: u32 = 0x1000_0004;
pub const KEYBOARD: u32 = 0x1000_0008;
pub const SCREEN_SHOW: u32 = 0x1000_000C;
pub const SCREEN_W: u32 = 0x1000_0010;
pub const SCREEN_H: u32 = 0x1000_0014;
pub const RANDOM: u32 = 0x1000_0018;
pub const FRAMEBUFFER: u32 = 0x1100_0000;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 200;

/// Key codes for non-character keys (characters use their ASCII code).
pub mod key {
    pub const LEFT: u8 = 0x80;
    pub const RIGHT: u8 = 0x81;
    pub const UP: u8 = 0x82;
    pub const DOWN: u8 = 0x83;
    pub const CTRL: u8 = 0x84;
    pub const SHIFT: u8 = 0x85;
    pub const ALT: u8 = 0x86;
    pub const ENTER: u8 = 13;
    pub const ESCAPE: u8 = 27;
    pub const BACKSPACE: u8 = 8;
    pub const TAB: u8 = 9;
}

pub struct Machine {
    pub ram: Vec<u32>,
    pub framebuffer: Vec<u32>,
    /// Bytes the program printed.
    pub console_out: Vec<u8>,
    /// Bytes waiting to be read from the console.
    pub console_in: VecDeque<u8>,
    /// Key events waiting: (key, pressed).
    pub keys: VecDeque<(u8, bool)>,
    /// Number of times the program asked to show the framebuffer.
    pub frames: u64,
    start: Instant,
    /// Fixed time instead of the wall clock, for reproducible tests.
    pub fixed_time: Option<u32>,
    rng: u64,
}

impl Machine {
    pub fn new(ram_bytes: usize) -> Self {
        Self {
            ram: vec![0; ram_bytes.div_ceil(4)],
            framebuffer: vec![0; (WIDTH * HEIGHT) as usize],
            console_out: Vec::new(),
            console_in: VecDeque::new(),
            keys: VecDeque::new(),
            frames: 0,
            start: Instant::now(),
            fixed_time: None,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }

    pub fn ram_bytes(&self) -> u32 {
        (self.ram.len() * 4) as u32
    }

    /// Copy bytes into RAM.
    pub fn load(&mut self, addr: u32, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            let a = addr as usize + i;
            let w = &mut self.ram[a / 4];
            let sh = (a % 4) * 8;
            *w = (*w & !(0xFF << sh)) | (b as u32) << sh;
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        (self.ram[addr as usize / 4] >> ((addr % 4) * 8)) as u8
    }

    pub fn millis(&self) -> u32 {
        self.fixed_time
            .unwrap_or_else(|| self.start.elapsed().as_millis() as u32)
    }

    pub fn console_text(&self) -> String {
        String::from_utf8_lossy(&self.console_out).into_owned()
    }

    /// What a read of `addr` would return, without its side effects
    /// (consuming input, advancing the random generator).
    pub fn peek(&self, addr: u32) -> Option<u32> {
        if let Some(&w) = self.ram.get(addr as usize / 4) {
            return Some(w);
        }
        Some(match addr {
            CONSOLE => self.console_in.front().map_or(u32::MAX, |&b| b as u32),
            TIMER => self.millis(),
            KEYBOARD => self
                .keys
                .front()
                .map_or(0, |&(k, p)| k as u32 | (p as u32) << 8),
            SCREEN_SHOW => 0,
            SCREEN_W => WIDTH,
            SCREEN_H => HEIGHT,
            RANDOM => self.rng as u32,
            a if (FRAMEBUFFER..FRAMEBUFFER + WIDTH * HEIGHT * 4).contains(&a) => {
                self.framebuffer[((a - FRAMEBUFFER) / 4) as usize]
            }
            _ => return None,
        })
    }

    /// RAM word at `addr`, or 0 outside RAM. Instruction fetch only sees RAM.
    pub fn peek_ram(&self, addr: u32) -> u32 {
        self.ram.get(addr as usize / 4).copied().unwrap_or(0)
    }

    fn device_read(&mut self, addr: u32) -> Option<u32> {
        let v = self.peek(addr)?;
        match addr {
            CONSOLE => {
                self.console_in.pop_front();
            }
            KEYBOARD => {
                self.keys.pop_front();
            }
            RANDOM => {
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 7;
                self.rng ^= self.rng << 17;
            }
            _ => {}
        }
        Some(v)
    }

    fn device_write(&mut self, addr: u32, value: u32, bytes: u8) -> Option<()> {
        match addr {
            CONSOLE => {
                if bytes & 1 == 1 {
                    self.console_out.push(value as u8);
                }
            }
            SCREEN_SHOW => self.frames += 1,
            TIMER | KEYBOARD | SCREEN_W | SCREEN_H | RANDOM => {}
            a if (FRAMEBUFFER..FRAMEBUFFER + WIDTH * HEIGHT * 4).contains(&a) => {
                let p = &mut self.framebuffer[((a - FRAMEBUFFER) / 4) as usize];
                let m = byte_mask(bytes);
                *p = (*p & !m) | (value & m);
            }
            _ => return None,
        }
        Some(())
    }
}

impl Memory for Machine {
    #[inline]
    fn fetch(&mut self, addr: u32) -> Option<u32> {
        self.ram.get(addr as usize / 4).copied()
    }

    #[inline]
    fn read(&mut self, addr: u32) -> Option<u32> {
        match self.ram.get(addr as usize / 4) {
            Some(&w) => Some(w),
            None => self.device_read(addr),
        }
    }

    #[inline]
    fn write(&mut self, addr: u32, value: u32, bytes: u8) -> Option<()> {
        match self.ram.get_mut(addr as usize / 4) {
            Some(w) => {
                let m = byte_mask(bytes);
                *w = (*w & !m) | (value & m);
                Some(())
            }
            None => self.device_write(addr, value, bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;
    use crate::emu::{Cpu, Trap};

    pub fn run(src: &str) -> (Cpu, Machine) {
        let p = assemble(src).unwrap_or_else(|e| panic!("{e}"));
        let mut m = Machine::new(64 * 1024);
        m.load(0, &p.bytes);
        let mut cpu = Cpu::new();
        cpu.x[2] = m.ram_bytes();
        assert_eq!(cpu.run(&mut m, 10_000_000), Some(Trap::Breakpoint));
        (cpu, m)
    }

    #[test]
    fn hello_console() {
        let (_, m) = run(r#"
                    la a0, msg
                    li t1, 0x10000000
            next:   lbu t0, 0(a0)
                    beqz t0, done
                    sb t0, 0(t1)
                    addi a0, a0, 1
                    j next
            done:   halt
            msg:    .asciz "Merhaba, dünya!\n"
        "#);
        assert_eq!(m.console_text(), "Merhaba, dünya!\n");
    }

    #[test]
    fn framebuffer_and_keys() {
        let mut m = Machine::new(1024);
        m.keys.push_back((key::LEFT, true));
        assert_eq!(m.peek(KEYBOARD), Some(0x180));
        assert_eq!(m.read(KEYBOARD), Some(0x180));
        assert_eq!(m.read(KEYBOARD), Some(0));
        m.write(FRAMEBUFFER + 4, 0x00FF_8000, 0b1111);
        m.write(FRAMEBUFFER + 4, 0x0000_0012, 0b0001);
        assert_eq!(m.framebuffer[1], 0x00FF_8012);
        assert_eq!(m.read(0x2000_0000), None);
    }

    #[test]
    fn subword_ram() {
        let (cpu, _) = run("
                li t0, 0x400
                li t1, -1
                sw t1, 0(t0)
                sh zero, 2(t0)
                sb zero, 0(t0)
                lw a0, 0(t0)
                lh a1, 0(t0)
                lhu a2, 0(t0)
                lb a3, 1(t0)
                lw a4, 1(t0)
                halt
        ");
        assert_eq!(cpu.x[10], 0x0000_FF00);
        assert_eq!(cpu.x[11], 0xFFFF_FF00);
        assert_eq!(cpu.x[12], 0x0000_FF00);
        assert_eq!(cpu.x[13], u32::MAX);
        // Misaligned word at 0x401: bytes FF 00 00 then 00 from 0x404.
        assert_eq!(cpu.x[14], 0x0000_00FF);
    }
}
