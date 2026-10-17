//! Turn a 32-bit instruction back into assembly text.

use crate::isa::*;

pub fn disasm(i: u32, pc: u32) -> String {
    let r = |n: usize| REG_NAMES[n];
    let (d, s1, s2) = (r(rd(i)), r(rs1(i)), r(rs2(i)));
    let f3 = funct3(i);
    let simm = |v: u32| v as i32;
    match opcode(i) {
        OP_LUI => format!("lui {d}, 0x{:x}", imm_u(i) >> 12),
        OP_AUIPC => format!("auipc {d}, 0x{:x}", imm_u(i) >> 12),
        OP_JAL => format!("jal {d}, 0x{:x}", pc.wrapping_add(imm_j(i))),
        OP_JALR if f3 == 0 => format!("jalr {d}, {}({s1})", simm(imm_i(i))),
        OP_BRANCH => {
            let n = ["beq", "bne", "?", "?", "blt", "bge", "bltu", "bgeu"][f3 as usize];
            format!("{n} {s1}, {s2}, 0x{:x}", pc.wrapping_add(imm_b(i)))
        }
        OP_LOAD => {
            let n = ["lb", "lh", "lw", "?", "lbu", "lhu", "?", "?"][f3 as usize];
            format!("{n} {d}, {}({s1})", simm(imm_i(i)))
        }
        OP_STORE => {
            let n = ["sb", "sh", "sw", "?", "?", "?", "?", "?"][f3 as usize];
            format!("{n} {s2}, {}({s1})", simm(imm_s(i)))
        }
        OP_IMM => {
            let imm = imm_i(i);
            match f3 {
                1 => format!("slli {d}, {s1}, {}", imm & 31),
                5 if funct7(i) == 0x20 => format!("srai {d}, {s1}, {}", imm & 31),
                5 => format!("srli {d}, {s1}, {}", imm & 31),
                _ => {
                    let n = ["addi", "", "slti", "sltiu", "xori", "", "ori", "andi"][f3 as usize];
                    format!("{n} {d}, {s1}, {}", simm(imm))
                }
            }
        }
        OP_REG => {
            let n = match (funct7(i), f3) {
                (0, 0) => "add",
                (0x20, 0) => "sub",
                (0, 1) => "sll",
                (0, 2) => "slt",
                (0, 3) => "sltu",
                (0, 4) => "xor",
                (0, 5) => "srl",
                (0x20, 5) => "sra",
                (0, 6) => "or",
                (0, 7) => "and",
                (1, f) => [
                    "mul", "mulh", "mulhsu", "mulhu", "div", "divu", "rem", "remu",
                ][f as usize],
                _ => "?",
            };
            format!("{n} {d}, {s1}, {s2}")
        }
        OP_FENCE => "fence".into(),
        OP_SYSTEM if i == EBREAK => "ebreak".into(),
        OP_SYSTEM if i == ECALL => "ecall".into(),
        _ => format!(".word 0x{i:08x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;

    #[test]
    fn round_trip() {
        let src = "add a0, a1, a2
            sub t0, t1, t2
            addi sp, sp, -16
            lw ra, 12(sp)
            sb a0, -1(s0)
            srai a0, a0, 3
            mulhu a0, a1, a2
            lui a0, 0x12345
            ebreak";
        let p = assemble(src).unwrap();
        let got: Vec<String> = p
            .words()
            .iter()
            .enumerate()
            .map(|(k, &w)| disasm(w, 4 * k as u32))
            .collect();
        let want: Vec<String> = src.lines().map(|l| l.trim().to_string()).collect();
        assert_eq!(got, want);
        assert_eq!(
            disasm(assemble("x: beq a0, a1, x").unwrap().words()[0], 0x100),
            "beq a0, a1, 0x100"
        );
    }
}
