//! Instruction fields for teaching. The diagram describes combinational
//! work inside a cycle, not extra pipeline stages or extra clock ticks.

use crate::isa::*;
use makine_lang::tr;

pub struct Decoded {
    pub format: &'static str,
    pub immediate: Option<i32>,
    pub path: &'static str,
    pub meaning: &'static str,
}

pub fn decode(i: u32) -> Decoded {
    let (format,immediate,path,meaning)=match opcode(i) {
        OP_REG=>("R",None,tr!("Register dosyası → ALU / çarpma-bölme → rd", "Register file → ALU / multiply-divide → rd"),tr!("İki register okunur; seçilen işlemin sonucu rd'ye yazılır. x0'a yazılan değer atılır.", "Read two registers; write the chosen operation's result to rd. Writes to x0 are discarded.")),
        OP_IMM=>("I",Some(imm_i(i) as i32),tr!("rs1 + komuttaki sabit → ALU → rd", "rs1 + instruction immediate → ALU → rd"),tr!("İkinci işlenen register değil, komutun içindeki sabittir. addi aralığı -2048…2047'dir; büyük sayılarda li kullan.", "The second operand is an immediate, not another register. addi accepts -2048…2047; use li for larger constants.")),
        OP_LOAD=>("I",Some(imm_i(i) as i32),tr!("rs1 + ofset → bellek oku → genişlet → rd", "rs1 + offset → memory read → extend → rd"),tr!("Adres = rs1 + ofset. lb/lh işaretle, lbu/lhu sıfırla genişletir. Bellek okumaları G/Ç'de yan etkili olabilir.", "Address = rs1 + offset. lb/lh sign-extend; lbu/lhu zero-extend. Device reads may have side effects.")),
        OP_STORE=>("S",Some(imm_s(i) as i32),tr!("rs1 + ofset → adres; rs2 → bellek yaz", "rs1 + offset → address; rs2 → memory write"),tr!("sb 1, sh 2, sw 4 bayt yazar. Komut rd'ye değer yazmaz.", "sb writes 1, sh 2, sw 4 bytes. This instruction does not write a destination register.")),
        OP_BRANCH=>("B",Some(imm_b(i) as i32),tr!("rs1 / rs2 → karşılaştır → sıradaki PC seçimi", "rs1 / rs2 → compare → next PC selection"),tr!("Koşul doğruysa PC = mevcut PC + ofset; değilse PC + 4. İmzalı ve imzasız karşılaştırma farklıdır.", "If true, PC = current PC + offset; otherwise PC + 4. Signed and unsigned comparisons differ.")),
        OP_JAL=>("J",Some(imm_j(i) as i32),tr!("PC + 4 → rd; PC + ofset → yeni PC", "PC + 4 → rd; PC + offset → next PC"),tr!("Dönüş adresi rd'ye kaydedilir. j kısa yazımı rd=x0 kullanır, bu yüzden dönüş adresini saklamaz.", "Save the return address in rd. The j pseudo-instruction uses rd=x0, so it discards the return address.")),
        OP_JALR=>("I",Some(imm_i(i) as i32),tr!("PC + 4 → rd; rs1 + ofset → yeni PC", "PC + 4 → rd; rs1 + offset → next PC"),tr!("Hedefin en alt biti temizlenir. ret, jalr x0,0(ra) kısa yazımıdır.", "Clear the target's lowest bit. ret is shorthand for jalr x0,0(ra).")),
        OP_LUI|OP_AUIPC=>("U",Some(imm_u(i) as i32),tr!("Üst sabit / PC + üst sabit → rd", "Upper immediate / PC + upper immediate → rd"),tr!("Alt 12 bit sıfırdır. li/la gibi kısa yazımlar birden fazla gerçek komut üretebilir.", "The low 12 bits are zero. Pseudo-instructions such as li/la may emit several real instructions.")),
        _=>("—",None,tr!("Kontrol / durma", "Control / stop"),tr!("ebreak burada programı durdurur. Bu görünüm desteklenen RV32IM komutlarını açıklar.", "ebreak stops the program here. This view explains supported RV32IM instructions.")),
    };
    Decoded {
        format,
        immediate,
        path,
        meaning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_immediates_and_types() {
        let p = crate::asm::assemble("addi a0,a1,-4\nsw a0,8(sp)\nbeq a0,a1,0").unwrap();
        let w = p.words();
        assert_eq!(decode(w[0]).immediate, Some(-4));
        assert_eq!(decode(w[1]).format, "S");
        assert_eq!(decode(w[2]).immediate, Some(-8));
    }
}
