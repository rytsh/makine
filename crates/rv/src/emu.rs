//! A straightforward RV32IM interpreter. It is the ground truth the player's
//! CPU is compared against, and the "turbo" mode that runs real programs
//! once the player's CPU passes.

use crate::isa::*;
use makine_lang::{tr, trf};

/// The memory bus as the CPU sees it. All accesses are aligned 32-bit words;
/// `bytes` is a 4-bit byte-enable mask (bit 0 = lowest address).
pub trait Memory {
    fn fetch(&mut self, addr: u32) -> Option<u32>;
    fn read(&mut self, addr: u32) -> Option<u32>;
    fn write(&mut self, addr: u32, value: u32, bytes: u8) -> Option<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trap {
    /// `ebreak`: the program asked to stop (our "halt").
    Breakpoint,
    /// `ecall`: a request to the environment, see `a7`.
    Ecall,
    IllegalInstruction(u32),
    MisalignedFetch(u32),
    /// Access to an address with nothing behind it.
    AccessFault(u32),
}

impl std::fmt::Display for Trap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Trap::Breakpoint => f.write_str(tr!("ebreak (dur)", "ebreak (halt)")),
            Trap::Ecall => write!(f, "ecall"),
            Trap::IllegalInstruction(i) => f.write_str(&trf!(
                "geçersiz komut 0x{i:08X}",
                "illegal instruction 0x{i:08X}"
            )),
            Trap::MisalignedFetch(a) => f.write_str(&trf!(
                "hizasız komut adresi 0x{a:08X}",
                "misaligned instruction address 0x{a:08X}"
            )),
            Trap::AccessFault(a) => f.write_str(&trf!(
                "geçersiz bellek adresi 0x{a:08X}",
                "invalid memory address 0x{a:08X}"
            )),
        }
    }
}

/// A store the CPU performed: aligned word address, data and byte enables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Store {
    pub addr: u32,
    pub data: u32,
    pub bytes: u8,
}

impl Store {
    /// Data with disabled bytes cleared, for comparing stores.
    pub fn masked(&self) -> u32 {
        self.data & byte_mask(self.bytes)
    }
}

/// Expand a 4-bit byte-enable mask to a 32-bit bit mask.
pub fn byte_mask(bytes: u8) -> u32 {
    (0..4)
        .filter(|i| bytes >> i & 1 == 1)
        .fold(0, |m, i| m | 0xFF << (8 * i))
}

#[derive(Clone, Debug, Default)]
pub struct Cpu {
    pub pc: u32,
    pub x: [u32; 32],
    /// Instructions retired.
    pub instret: u64,
    /// The store done by the last instruction, if any.
    pub last_store: Option<Store>,
}

impl Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    fn load<M: Memory>(&mut self, m: &mut M, addr: u32, f3: u32) -> Result<u32, Trap> {
        let size = 1u32 << (f3 & 3);
        let v = if addr.is_multiple_of(size) {
            let w = m.read(addr & !3).ok_or(Trap::AccessFault(addr))?;
            w >> ((addr & 3) * 8)
        } else {
            // Misaligned: legal but rare; do it a byte at a time.
            let mut v = 0u32;
            for i in 0..size {
                let a = addr.wrapping_add(i);
                let w = m.read(a & !3).ok_or(Trap::AccessFault(a))?;
                v |= (w >> ((a & 3) * 8) & 0xFF) << (8 * i);
            }
            v
        };
        Ok(match f3 {
            0 => v as u8 as i8 as i32 as u32,
            1 => v as u16 as i16 as i32 as u32,
            2 => v,
            4 => v & 0xFF,
            _ => v & 0xFFFF,
        })
    }

    fn store<M: Memory>(&mut self, m: &mut M, addr: u32, val: u32, f3: u32) -> Result<(), Trap> {
        let size = 1u32 << (f3 & 3);
        if !addr.is_multiple_of(size) {
            for i in 0..size {
                let a = addr.wrapping_add(i);
                let sh = (a & 3) * 8;
                m.write(a & !3, (val >> (8 * i) & 0xFF) << sh, 1 << (a & 3))
                    .ok_or(Trap::AccessFault(a))?;
            }
            self.last_store = None;
            return Ok(());
        }
        let shift = (addr & 3) * 8;
        let bytes = (match size {
            1 => 0b0001,
            2 => 0b0011,
            _ => 0b1111,
        }) << (addr & 3);
        let s = Store {
            addr: addr & !3,
            data: val << shift,
            bytes,
        };
        m.write(s.addr, s.data, s.bytes)
            .ok_or(Trap::AccessFault(addr))?;
        self.last_store = Some(s);
        Ok(())
    }

    /// Execute one instruction.
    #[inline]
    pub fn step<M: Memory>(&mut self, m: &mut M) -> Result<(), Trap> {
        let pc = self.pc;
        self.last_store = None;
        if !pc.is_multiple_of(4) {
            return Err(Trap::MisalignedFetch(pc));
        }
        let i = m.fetch(pc).ok_or(Trap::AccessFault(pc))?;
        let mut next = pc.wrapping_add(4);
        let (a, b) = (self.x[rs1(i)], self.x[rs2(i)]);
        let rd = rd(i);
        let f3 = funct3(i);
        let ill = Trap::IllegalInstruction(i);

        let result: Option<u32> = match opcode(i) {
            OP_LUI => Some(imm_u(i)),
            OP_AUIPC => Some(pc.wrapping_add(imm_u(i))),
            OP_JAL => {
                next = pc.wrapping_add(imm_j(i));
                Some(pc.wrapping_add(4))
            }
            OP_JALR if f3 == 0 => {
                next = a.wrapping_add(imm_i(i)) & !1;
                Some(pc.wrapping_add(4))
            }
            OP_BRANCH => {
                let taken = match f3 {
                    0 => a == b,
                    1 => a != b,
                    4 => (a as i32) < (b as i32),
                    5 => (a as i32) >= (b as i32),
                    6 => a < b,
                    7 => a >= b,
                    _ => return Err(ill),
                };
                if taken {
                    next = pc.wrapping_add(imm_b(i));
                }
                None
            }
            OP_LOAD if f3 != 3 && f3 <= 5 => Some(self.load(m, a.wrapping_add(imm_i(i)), f3)?),
            OP_STORE if f3 <= 2 => {
                self.store(m, a.wrapping_add(imm_s(i)), b, f3)?;
                None
            }
            OP_IMM => {
                let imm = imm_i(i);
                let shamt = imm & 31;
                Some(match f3 {
                    0 => a.wrapping_add(imm),
                    1 if funct7(i) == 0 => a << shamt,
                    2 => ((a as i32) < (imm as i32)) as u32,
                    3 => (a < imm) as u32,
                    4 => a ^ imm,
                    5 if funct7(i) == 0 => a >> shamt,
                    5 if funct7(i) == 0x20 => ((a as i32) >> shamt) as u32,
                    6 => a | imm,
                    7 => a & imm,
                    _ => return Err(ill),
                })
            }
            OP_REG => Some(match (funct7(i), f3) {
                (0, 0) => a.wrapping_add(b),
                (0x20, 0) => a.wrapping_sub(b),
                (0, 1) => a << (b & 31),
                (0, 2) => ((a as i32) < (b as i32)) as u32,
                (0, 3) => (a < b) as u32,
                (0, 4) => a ^ b,
                (0, 5) => a >> (b & 31),
                (0x20, 5) => ((a as i32) >> (b & 31)) as u32,
                (0, 6) => a | b,
                (0, 7) => a & b,
                (1, f3) => mul_div(f3, a, b),
                _ => return Err(ill),
            }),
            OP_FENCE => None,
            OP_SYSTEM => match i {
                EBREAK => return Err(Trap::Breakpoint),
                ECALL => return Err(Trap::Ecall),
                _ => return Err(ill),
            },
            _ => return Err(ill),
        };

        if let Some(v) = result {
            if rd != 0 {
                self.x[rd] = v;
            }
        }
        self.pc = next;
        self.instret += 1;
        Ok(())
    }

    /// Run until a trap or `max` instructions. Returns the trap, or `None`
    /// if the budget ran out.
    pub fn run<M: Memory>(&mut self, m: &mut M, max: u64) -> Option<Trap> {
        for _ in 0..max {
            if let Err(t) = self.step(m) {
                return Some(t);
            }
        }
        None
    }
}

/// The M extension. Division by zero and overflow follow the spec: no trap.
pub fn mul_div(f3: u32, a: u32, b: u32) -> u32 {
    let (sa, sb) = (a as i32 as i64, b as i32 as i64);
    match f3 {
        0 => a.wrapping_mul(b),
        1 => ((sa * sb) >> 32) as u32,
        2 => ((sa * b as i64) >> 32) as u32,
        3 => ((a as u64 * b as u64) >> 32) as u32,
        4 => match (a as i32, b as i32) {
            (_, 0) => u32::MAX,
            (i32::MIN, -1) => a,
            (x, y) => (x / y) as u32,
        },
        5 => a.checked_div(b).unwrap_or(u32::MAX),
        6 => match (a as i32, b as i32) {
            (_, 0) => a,
            (i32::MIN, -1) => 0,
            (x, y) => (x % y) as u32,
        },
        _ => a.checked_rem(b).unwrap_or(a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_div_edges() {
        let m = |x: i32| x as u32;
        assert_eq!(mul_div(0, 7, m(-3)), m(-21));
        assert_eq!(mul_div(1, m(-1), m(-1)), 0);
        assert_eq!(mul_div(3, u32::MAX, u32::MAX), 0xFFFF_FFFE);
        assert_eq!(mul_div(2, m(-1), u32::MAX), u32::MAX);
        assert_eq!(mul_div(4, m(-7), 2), m(-3));
        assert_eq!(mul_div(4, 5, 0), u32::MAX);
        assert_eq!(mul_div(4, m(i32::MIN), m(-1)), m(i32::MIN));
        assert_eq!(mul_div(5, 5, 0), u32::MAX);
        assert_eq!(mul_div(6, m(-7), 2), m(-1));
        assert_eq!(mul_div(6, 5, 0), 5);
        assert_eq!(mul_div(6, m(i32::MIN), m(-1)), 0);
        assert_eq!(mul_div(7, 5, 0), 5);
    }

    #[test]
    fn byte_masks() {
        assert_eq!(byte_mask(0b0001), 0xFF);
        assert_eq!(byte_mask(0b1100), 0xFFFF_0000);
        assert_eq!(byte_mask(0b1111), u32::MAX);
    }
}
