//! Short, local instruction reference. Examples are assembled by tests.
use makine_lang::tr;

pub fn entries() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "li",
            "li a0, 42",
            tr!(
                "Sabit yükle; büyük sayı iki komut olabilir.",
                "Load a constant; large values may need two instructions."
            ),
        ),
        (
            "la",
            "la t0, data\nebreak\ndata: .word 0",
            tr!(
                "Etiketin adresini yükle, içindeki veriyi değil.",
                "Load a label's address, not its contents."
            ),
        ),
        (
            "mv",
            "mv a0, a1",
            tr!("Register kopyala.", "Copy a register."),
        ),
        (
            "add",
            "add a0, a1, a2",
            tr!("İki register topla.", "Add two registers."),
        ),
        (
            "addi",
            "addi t0, t0, -1",
            tr!(
                "-2048…2047 aralığında sabit ekle.",
                "Add a constant in -2048…2047."
            ),
        ),
        (
            "sub",
            "sub a0, a1, a2",
            tr!("a1'den a2'yi çıkar.", "Subtract a2 from a1."),
        ),
        (
            "and / andi",
            "andi a0, a0, 1",
            tr!(
                "Bit maskesi; son bit tek/çift bilgisidir.",
                "Bit mask; the low bit tells odd/even."
            ),
        ),
        (
            "or / xor",
            "xor a0, a1, a2",
            tr!(
                "XOR farklı olan bitleri 1 yapar.",
                "XOR sets differing bits to 1."
            ),
        ),
        (
            "slli / srli / srai",
            "slli t0, t0, 2",
            tr!(
                "Bit kaydır. srai üstten işaret bitini kopyalar.",
                "Shift bits. srai fills with the sign bit."
            ),
        ),
        (
            "lw / sw",
            "lw a0, 0(t0)\nsw a0, 4(t0)",
            tr!(
                "32-bit kelime oku/yaz; ofset bayt cinsinden.",
                "Read/write a 32-bit word; offsets are in bytes."
            ),
        ),
        (
            "lbu / lb / sb",
            "lbu a0, 0(t0)\nsb a0, 1(t0)",
            tr!(
                "Bayt oku/yaz. lb işaretle genişletir; lbu sıfırla.",
                "Read/write a byte. lb sign-extends, lbu zero-extends."
            ),
        ),
        (
            "beq / bne",
            "beq a0, a1, done\ndone: ebreak",
            tr!("Eşit/eşit değilse dallan.", "Branch if equal/not equal."),
        ),
        (
            "blt / bge",
            "blt a0, a1, done\ndone: ebreak",
            tr!(
                "İşaretli küçük/büyük eşit karşılaştırması.",
                "Signed less-than/greater-or-equal comparison."
            ),
        ),
        (
            "bltu / bgeu",
            "bltu a0, a1, done\ndone: ebreak",
            tr!(
                "İşaretsiz karşılaştırma; negatif sayılar büyük görünür.",
                "Unsigned comparison; negative values appear large."
            ),
        ),
        (
            "beqz / bnez",
            "beqz a0, done\ndone: ebreak",
            tr!("Sıfır/sıfır değilse dallan.", "Branch if zero/not zero."),
        ),
        (
            "j",
            "loop: j loop",
            tr!(
                "Koşulsuz atla; bu örnek sonsuz döngüdür.",
                "Jump unconditionally; this example is an infinite loop."
            ),
        ),
        (
            "call / ret",
            "call f\nebreak\nf: ret",
            tr!(
                "ra dönüş adresini tutar. İç içe çağrıda ra'yı yığında koru.",
                "ra holds the return address. Preserve it on the stack for nested calls."
            ),
        ),
        (
            "mul / div / rem",
            "mul a0, a1, a2",
            tr!(
                "Çarpma, bölüm, kalan (M eklentisi).",
                "Multiply, divide, remainder (M extension)."
            ),
        ),
        (
            "ebreak",
            "ebreak",
            tr!(
                "Bu makinede programı durdurur.",
                "Stops the program on this machine."
            ),
        ),
        (
            ".word / .byte",
            "data: .word 1, 2, 3",
            tr!(
                "Belleğe veri yerleştir; çalıştırılacak komut değil.",
                "Place data in memory; not executable instructions."
            ),
        ),
        (
            ".asciz",
            "text: .asciz \"hello\"",
            tr!("Sıfırla biten UTF-8 metin.", "Zero-terminated UTF-8 text."),
        ),
        (
            ".align",
            ".align 2\ndata: .word 7",
            tr!(
                "2^n bayta hizala; .align 2 = 4 bayt.",
                "Align to 2^n bytes; .align 2 means 4 bytes."
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn reference_examples_assemble_in_both_languages() {
        for lang in makine_lang::Lang::ALL {
            makine_lang::set_for_thread(Some(lang));
            for (_, s, _) in super::entries() {
                makine_rv::asm::assemble(s).unwrap();
            }
        }
        makine_lang::set_for_thread(None);
    }
}
