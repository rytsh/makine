//! RV32IM instruction encoding: opcodes, field extraction and immediates.

pub const OP_LUI: u32 = 0x37;
pub const OP_AUIPC: u32 = 0x17;
pub const OP_JAL: u32 = 0x6F;
pub const OP_JALR: u32 = 0x67;
pub const OP_BRANCH: u32 = 0x63;
pub const OP_LOAD: u32 = 0x03;
pub const OP_STORE: u32 = 0x23;
pub const OP_IMM: u32 = 0x13;
pub const OP_REG: u32 = 0x33;
pub const OP_FENCE: u32 = 0x0F;
pub const OP_SYSTEM: u32 = 0x73;

pub const EBREAK: u32 = 0x0010_0073;
pub const ECALL: u32 = 0x0000_0073;
/// What compilers emit for `__builtin_trap()`: `csrrw zero, cycle, zero`.
pub const UNIMP: u32 = 0xC000_1073;
pub const NOP: u32 = 0x0000_0013;

pub fn opcode(i: u32) -> u32 {
    i & 0x7F
}
pub fn rd(i: u32) -> usize {
    (i >> 7 & 31) as usize
}
pub fn funct3(i: u32) -> u32 {
    i >> 12 & 7
}
pub fn rs1(i: u32) -> usize {
    (i >> 15 & 31) as usize
}
pub fn rs2(i: u32) -> usize {
    (i >> 20 & 31) as usize
}
pub fn funct7(i: u32) -> u32 {
    i >> 25
}

pub fn imm_i(i: u32) -> u32 {
    ((i as i32) >> 20) as u32
}
pub fn imm_s(i: u32) -> u32 {
    (((i as i32) >> 20) as u32 & !31) | (i >> 7 & 31)
}
pub fn imm_b(i: u32) -> u32 {
    (((i as i32) >> 19) as u32 & !0xFFF) // imm[12] and sign
        | (i << 4 & 0x800) // imm[11] from bit 7
        | (i >> 20 & 0x7E0) // imm[10:5]
        | (i >> 7 & 0x1E) // imm[4:1]
}
pub fn imm_u(i: u32) -> u32 {
    i & 0xFFFF_F000
}
pub fn imm_j(i: u32) -> u32 {
    (((i as i32) >> 11) as u32 & !0xF_FFFF) // imm[20] and sign
        | (i & 0xF_F000) // imm[19:12]
        | (i >> 9 & 0x800) // imm[11]
        | (i >> 20 & 0x7FE) // imm[10:1]
}

pub fn enc_r(op: u32, rd: u32, f3: u32, rs1: u32, rs2: u32, f7: u32) -> u32 {
    f7 << 25 | rs2 << 20 | rs1 << 15 | f3 << 12 | rd << 7 | op
}
pub fn enc_i(op: u32, rd: u32, f3: u32, rs1: u32, imm: u32) -> u32 {
    (imm & 0xFFF) << 20 | rs1 << 15 | f3 << 12 | rd << 7 | op
}
pub fn enc_s(op: u32, f3: u32, rs1: u32, rs2: u32, imm: u32) -> u32 {
    (imm >> 5 & 0x7F) << 25 | rs2 << 20 | rs1 << 15 | f3 << 12 | (imm & 31) << 7 | op
}
pub fn enc_b(op: u32, f3: u32, rs1: u32, rs2: u32, imm: u32) -> u32 {
    (imm >> 12 & 1) << 31
        | (imm >> 5 & 0x3F) << 25
        | rs2 << 20
        | rs1 << 15
        | f3 << 12
        | (imm >> 1 & 0xF) << 8
        | (imm >> 11 & 1) << 7
        | op
}
pub fn enc_u(op: u32, rd: u32, imm: u32) -> u32 {
    (imm & 0xFFFF_F000) | rd << 7 | op
}
pub fn enc_j(op: u32, rd: u32, imm: u32) -> u32 {
    (imm >> 20 & 1) << 31
        | (imm >> 1 & 0x3FF) << 21
        | (imm >> 11 & 1) << 20
        | (imm >> 12 & 0xFF) << 12
        | rd << 7
        | op
}

pub const REG_NAMES: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];

pub fn parse_reg(s: &str) -> Option<u32> {
    if let Some(n) = s.strip_prefix('x') {
        if let Ok(n) = n.parse::<u32>() {
            return (n < 32).then_some(n);
        }
    }
    if s == "fp" {
        return Some(8);
    }
    REG_NAMES.iter().position(|&r| r == s).map(|i| i as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediates_round_trip() {
        for imm in [
            0u32,
            2,
            4,
            0x7FE,
            0x800,
            0xFFE,
            (-2i32) as u32,
            (-4096i32) as u32,
            4094,
        ] {
            let b = enc_b(OP_BRANCH, 0, 1, 2, imm);
            assert_eq!(imm_b(b), imm, "b {imm:#x}");
        }
        for imm in [
            0u32,
            2,
            0x800,
            0xF_FFFE,
            (-2i32) as u32,
            (-(1 << 20)) as u32,
        ] {
            let j = enc_j(OP_JAL, 1, imm);
            assert_eq!(imm_j(j), imm, "j {imm:#x}");
        }
        for imm in [0u32, 1, 0x7FF, (-1i32) as u32, (-2048i32) as u32] {
            assert_eq!(imm_s(enc_s(OP_STORE, 2, 1, 2, imm)), imm);
            assert_eq!(imm_i(enc_i(OP_IMM, 1, 0, 2, imm)), imm);
        }
    }

    #[test]
    fn known_encodings() {
        // Values from GNU as.
        assert_eq!(enc_i(OP_IMM, 10, 0, 10, 1), 0x0015_0513); // addi a0,a0,1
        assert_eq!(enc_s(OP_STORE, 2, 2, 11, 8), 0x00B1_2423); // sw a1,8(sp)
        assert_eq!(enc_b(OP_BRANCH, 0, 10, 11, 8), 0x00B5_0463); // beq a0,a1,+8
        assert_eq!(enc_r(OP_REG, 10, 0, 10, 11, 0x20), 0x40B5_0533); // sub a0,a0,a1
        assert_eq!(enc_i(OP_JALR, 0, 0, 1, 0), 0x0000_8067); // ret
    }
}
