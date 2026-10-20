//! Levels for section 4 onwards: the ALU, register file, and the RISC-V CPU
//! built from them.
//!
//! The CPU is single-cycle: one instruction per clock. Memory sits outside
//! it, the way a real chip talks to RAM over a bus:
//!
//! - `pc[32]` out: address of the current instruction
//! - `instr[32]` in: the word at `pc`
//! - `addr[32]` out, `rdata[32]` in: the data word the instruction reads
//! - `wdata[32]`, `wmask[4]`, `we` out: what to write (byte enables) when
//!   `we` = 1; the write happens on the clock edge
//!
//! All memory addresses on the bus are word-aligned; the CPU shifts bytes and
//! halves into place itself.

use makine_lang::{tr, trf};
use makine_rv::emu::{byte_mask, mul_div, Cpu, Memory, Trap};
use makine_rv::isa::*;
use makine_sim::{mask, Circuit, Rng, TestStep, Verdict};

use crate::{Level, Test};

pub const ALU: &str = "4. ALU";
pub const CPU: &str = "5. İşlemci";

/// ALU operation codes: `funct3` of the instruction, plus 8 for the
/// "alternate" form (sub, sra) selected by bit 30.
pub mod op {
    pub const ADD: u64 = 0;
    pub const SLL: u64 = 1;
    pub const SLT: u64 = 2;
    pub const SLTU: u64 = 3;
    pub const XOR: u64 = 4;
    pub const SRL: u64 = 5;
    pub const OR: u64 = 6;
    pub const AND: u64 = 7;
    pub const SUB: u64 = 8;
    pub const SRA: u64 = 13;
}

pub fn alu(a: u32, b: u32, op: u64) -> u32 {
    let sh = b & 31;
    match op {
        op::ADD => a.wrapping_add(b),
        op::SUB => a.wrapping_sub(b),
        op::SLL | 9 => a << sh,
        op::SLT | 10 => ((a as i32) < (b as i32)) as u32,
        op::SLTU | 11 => (a < b) as u32,
        op::XOR | 12 => a ^ b,
        op::SRL => a >> sh,
        op::SRA => ((a as i32) >> sh) as u32,
        op::OR | 14 => a | b,
        _ => a & b,
    }
}

fn shift_left(i: &[u64]) -> Vec<u64> {
    vec![(i[0] << (i[1] & 31)) & mask(32)]
}

fn shift_right(i: &[u64]) -> Vec<u64> {
    let (a, n, arith) = (i[0] as u32, i[1] as u32 & 31, i[2] == 1);
    vec![if arith {
        ((a as i32) >> n) as u32
    } else {
        a >> n
    } as u64]
}

fn compare(i: &[u64]) -> Vec<u64> {
    let (a, b) = (i[0] as u32, i[1] as u32);
    vec![
        (a == b) as u64,
        ((a as i32) < (b as i32)) as u64,
        (a < b) as u64,
    ]
}

pub static LEVELS: &[Level] = &[
    Level {
        chip: "Zero32",
        section: ALU,
        title: "Sıfır mı?",
        description: "Giriş 0 ise çıkış 1 olsun.\n\n\
            32 biti tek tek VEYA'la: herhangi biri 1 ise sayı sıfır değildir. \
            Bus'ı Böl ile ikiye ayırıp aynı işi yarım genişlikte tekrarlamak (ağaç gibi) \
            hem daha az parça hem daha hızlı devre demek.",
        inputs: &["in[32]"],
        outputs: &["zero"],
        test: Test::Function(|i| vec![(i[0] == 0) as u64]),
    },
    Level {
        chip: "Compare32",
        section: ALU,
        title: "Karşılaştırıcı",
        description: "Üç soru: a = b mi (eq)? İşaretli sayı olarak a < b mi (lt)? \
            İşaretsiz olarak a < b mi (ltu)?\n\n\
            İpucu: a - b'yi hesapla. Eşitlik için fark sıfır mı diye bak. İşaretsiz \
            küçüklük toplayıcının eldesinden çıkar: a + DEĞİL(b) + 1 işleminde elde 0 ise a < b. \
            İşaretli küçüklük için en üst bitlere bak: a ve b'nin işaretleri farklıysa \
            negatif olan küçüktür; aynıysa farkın işareti cevaptır.\n\n\
            Dallanma komutları (beq, blt, bltu...) bu çipe dayanır.",
        inputs: &["a[32]", "b[32]"],
        outputs: &["eq", "lt", "ltu"],
        test: Test::Function(compare),
    },
    Level {
        chip: "ShiftLeft32",
        section: ALU,
        title: "Sola Kaydırıcı",
        description: "a'yı n (0..31) basamak sola kaydır; sağdan 0 gelsin.\n\n\
            Fikir (barrel shifter): n'nin her biti bir aşama. Bit 0 = 1 ise 1 kaydır, \
            bit 1 = 1 ise 2 kaydır, bit 2 için 4, sonra 8 ve 16. Her aşama bir 32-bit MUX: \
            ya olduğu gibi bırak ya kaydırılmışını seç. Sabit miktarda kaydırmak sadece \
            tel işi: Böl ve Birleştir ile bitleri bir yana kaydır, boşluğa Sabit 0 koy. \
            n'nin sadece alt 5 biti önemli.",
        inputs: &["a[32]", "n[5]"],
        outputs: &["out[32]"],
        test: Test::Function(shift_left),
    },
    Level {
        chip: "ShiftRight32",
        section: ALU,
        title: "Sağa Kaydırıcı",
        description: "a'yı n basamak sağa kaydır. arith = 0 ise soldan 0 gelsin (mantıksal), \
            arith = 1 ise a'nın en üst biti (işaret) gelsin (aritmetik).\n\n\
            Aritmetik kaydırma negatif sayıları 2'ye bölmenin yoludur: -8 >> 1 = -4.",
        inputs: &["a[32]", "n[5]", "arith"],
        outputs: &["out[32]"],
        test: Test::Function(shift_right),
    },
    Level {
        chip: "Alu",
        section: ALU,
        title: "ALU",
        description: "İşlemcinin hesap makinesi. op'a göre a ve b ile işlem yap:\n\
            0 add: a + b       8 sub: a - b\n\
            1 sll: a << b       5 srl: a >> b       13 sra: a >> b (işaretli)\n\
            2 slt: a < b ise 1 (işaretli)       3 sltu: a < b ise 1 (işaretsiz)\n\
            4 xor     6 or     7 and\n\n\
            op'un alt 3 biti RISC-V komutlarındaki funct3 alanı, bit 3 ise sub/sra'yı seçen bit. \
            Kaydırmalarda b'nin sadece alt 5 biti kullanılır. Bütün sonuçları hesaplayıp \
            op ile doğru olanı seçebilirsin.",
        inputs: &["a[32]", "b[32]", "op[4]"],
        outputs: &["out[32]"],
        test: Test::Partial(|i| {
            let op = i[2];
            // Codes outside the table are unused by the CPU.
            let used = matches!(op, 0..=8 | 13);
            vec![used.then(|| alu(i[0] as u32, i[1] as u32, op) as u64)]
        }),
    },
    Level {
        chip: "RegFile",
        section: ALU,
        title: "Register Dosyası",
        description: "RISC-V'nin 32 register'ı: x0..x31. Aynı anda iki tanesini oku (rs1 ve rs2 \
            numaralı olanlar a ve b'ye), saat vuruşunda we = 1 ise rd numaralı olana d'yi yaz.\n\n\
            Özel kural: x0 her zaman 0'dır; ona yazmak hiçbir şey yapmaz.\n\n\
            İpucu: 31 tane Register32 (x0 için register gerekmez), hangi register'a yazılacağını \
            seçen bir 'kod çözücü' (decoder; rd'yi 32 tane load sinyaline çevirir; Demux'lardan \
            yapılabilir) ve okumak için iki büyük MUX ağacı. Okuma saat beklemez: rs1 değişince \
            a hemen değişir.",
        inputs: &["rs1[5]", "rs2[5]", "rd[5]", "d[32]", "we", "clk"],
        outputs: &["a[32]", "b[32]"],
        test: Test::Sequence(regfile_steps),
    },
    Level {
        chip: "Decoder",
        section: CPU,
        title: "Komut Çözücü",
        description: "Bir RISC-V komutunu parçalarına ayır. Komut 32 bit; alanlar:\n\
            opcode = bit 0..6, rd = 7..11, funct3 = 12..14, rs1 = 15..19, rs2 = 20..24.\n\n\
            imm, komut türüne göre farklı yerlerden toplanan 'anlık' sayıdır ve 32 bite \
            işaretle genişletilir (en üst bitinin kopyalarıyla doldurulur; bu her zaman \
            komutun bit 31'idir):\n\
            • I türü (opcode 0x03 yükleme, 0x13 hesap, 0x67 jalr): imm = instr[31:20]\n\
            • S türü (0x23 saklama): imm = instr[31:25] ve instr[11:7] yan yana\n\
            • B türü (0x63 dallanma): imm[12|10:5|4:1|11] = instr[31|30:25|11:8|7], imm[0] = 0\n\
            • U türü (0x37 lui, 0x17 auipc): imm = instr[31:12] ve altına 12 sıfır\n\
            • J türü (0x6F jal): imm[20|10:1|11|19:12] = instr[31|30:21|20|19:12], imm[0] = 0\n\n\
            Diğer opcode'larda imm önemsiz. Opcode'un sadece ilgili bitlerine bakman yeterli.",
        inputs: &["instr[32]"],
        outputs: &[
            "opcode[7]",
            "rd[5]",
            "funct3[3]",
            "rs1[5]",
            "rs2[5]",
            "imm[32]",
        ],
        test: Test::Partial(decoder_reference),
    },
    Level {
        chip: "Control",
        section: CPU,
        title: "Kontrol Birimi",
        description: "İşlemcinin şefi: komutun opcode'una (ve funct3/funct7'ye) bakıp her parçaya \
            ne yapacağını söyler. Çıkışlar:\n\
            • reg_we: sonuç bir register'a yazılacak mı (lui, auipc, jal, jalr, yükleme, hesap: 1; \
            dallanma ve saklama: 0)\n\
            • alu_b_imm: ALU'nun b girişi imm mi (1) yoksa rs2 mi (0)? (hesap-imm, yükleme, \
            saklama, jalr: 1; register-register hesap: 0)\n\
            • alu_op[4]: ALU işlemi. Hesap komutlarında {funct7 bit 5, funct3}; ama \
            hesap-imm'de sadece srai için bit 3 kullanılır (addi'de 'sub' yoktur!). Yükleme, \
            saklama ve jalr'da toplama (0).\n\
            • wb_sel[2]: register'a ne yazılacak: 0 = ALU sonucu, 1 = bellekten okunan, \
            2 = pc + 4 (jal/jalr dönüş adresi), 3 = imm (lui).\n\
            • mem_read, mem_write: yükleme mi, saklama mı.\n\
            • branch, jal, jalr: hangi tür atlama.\n\
            • auipc: ALU'nun a girişi pc mi?\n\
            • mul: M eklentisi (opcode 0x33 ve funct7 = 1): sonucu çarpma/bölme biriminden al.\n\n\
            Geçersiz komutlar için çıkışlar önemsiz.",
        inputs: &["opcode[7]", "funct3[3]", "funct7[7]"],
        outputs: &[
            "reg_we",
            "alu_b_imm",
            "alu_op[4]",
            "wb_sel[2]",
            "mem_read",
            "mem_write",
            "branch",
            "jal",
            "jalr",
            "auipc",
            "mul",
        ],
        test: Test::Partial(control_reference),
    },
    Level {
        chip: "BranchUnit",
        section: CPU,
        title: "Dallanma Birimi",
        description: "Dallanma alınacak mı? funct3'e göre:\n\
            0 beq: eşit   1 bne: eşit değil   4 blt: küçük (işaretli)   5 bge: büyük-eşit \
            (işaretli)   6 bltu: küçük (işaretsiz)   7 bgeu: büyük-eşit (işaretsiz)\n\n\
            Girişler Karşılaştırıcı'nın çıkışları. funct3'ün bit 0'ı sonucu tersine çevirir: \
            bne = DEĞİL beq, bge = DEĞİL blt...",
        inputs: &["eq", "lt", "ltu", "funct3[3]"],
        outputs: &["take"],
        test: Test::Partial(|i| {
            let (eq, lt, ltu, f3) = (i[0], i[1], i[2], i[3]);
            vec![match f3 {
                0 => Some(eq),
                1 => Some(eq ^ 1),
                4 => Some(lt),
                5 => Some(lt ^ 1),
                6 => Some(ltu),
                7 => Some(ltu ^ 1),
                _ => None,
            }]
        }),
    },
    Level {
        chip: "LoadUnit",
        section: CPU,
        title: "Yükleme Birimi",
        description:
            "Bellek her zaman 4 baytlık hizalı bir kelime verir (word). Komut ise bir bayt \
            (lb, lbu), yarım kelime (lh, lhu) ya da kelime (lw) istemiş olabilir.\n\n\
            off = adresin alt 2 biti: istenen bayt kelimenin içinde nerede? Önce word'ü off × 8 \
            bit sağa kaydır, sonra funct3'e göre kes:\n\
            0 lb: alt 8 bit, işaretle genişlet   1 lh: alt 16 bit, işaretle genişlet\n\
            2 lw: hepsi   4 lbu: alt 8 bit, sıfırla genişlet   5 lhu: alt 16 bit, sıfırla genişlet",
        inputs: &["word[32]", "off[2]", "funct3[3]"],
        outputs: &["out[32]"],
        test: Test::Partial(|i| {
            let v = (i[0] as u32) >> (i[1] * 8);
            vec![match i[2] {
                0 => Some(v as u8 as i8 as i32 as u32 as u64),
                1 => Some(v as u16 as i16 as i32 as u32 as u64),
                2 => Some(v as u64),
                4 => Some((v & 0xFF) as u64),
                5 => Some((v & 0xFFFF) as u64),
                _ => None,
            }]
        }),
    },
    Level {
        chip: "StoreUnit",
        section: CPU,
        title: "Saklama Birimi",
        description: "Yüklemenin tersi: sb/sh/sw ile bir kelimenin sadece bir kısmını yaz.\n\n\
            data'yı off × 8 bit sola kaydırıp wdata'ya ver. mask[4] hangi baytların yazılacağını \
            söyler (bit 0 = en düşük adresli bayt):\n\
            0 sb: 1 bayt → 0001 << off   1 sh: 2 bayt → 0011 << off   2 sw: 4 bayt → 1111\n\n\
            Hizalı olmayan erişimler (ör. off = 3 iken sh) bu işlemcide desteklenmiyor; \
            o durumlarda çıkış önemsiz.",
        inputs: &["data[32]", "off[2]", "funct3[3]"],
        outputs: &["wdata[32]", "mask[4]"],
        test: Test::Partial(|i| {
            let (d, off, f3) = (i[0] as u32, i[1], i[2]);
            let size = match f3 {
                0 => 1,
                1 => 2,
                2 => 4,
                _ => return vec![None, None],
            };
            if off % size != 0 {
                return vec![None, None];
            }
            let m = ((1u64 << size) - 1) << off;
            // Only the enabled bytes of wdata matter, but the level checks the
            // whole word: shifting is the natural answer.
            vec![Some(((d as u64) << (off * 8)) & mask(32)), Some(m)]
        }),
    },
    Level {
        chip: "MulDiv",
        section: CPU,
        title: "Çarpma ve Bölme",
        description: "RISC-V'nin M eklentisi. funct3'e göre:\n\
            0 mul: a×b'nin alt 32 biti   1 mulh: a×b'nin üst 32 biti (ikisi de işaretli)\n\
            2 mulhsu: üst 32 bit (a işaretli, b işaretsiz)   3 mulhu: üst 32 bit (işaretsiz)\n\
            4 div: a÷b (işaretli)   5 divu: a÷b (işaretsiz)   6 rem: kalan (işaretli)   \
            7 remu: kalan (işaretsiz)\n\n\
            Sıfıra bölme hata vermez: bölüm hep 1'lerden oluşur, kalan a'dır. \
            -2³¹ ÷ -1 = -2³¹, kalan 0.\n\n\
            Çarpma: okulda öğrendiğin gibi, b'nin her biti için a'yı kaydırıp topla (64 bitlik \
            sonuç için 32 toplayıcı). Bölme: uzun bölme, her adımda kalandan bölen çıkıyor mu \
            diye bak. İşaretli işlemler için önce mutlak değer al, sonra işareti düzelt.\n\n\
            Bu en büyük devre; hızlı simülasyon (⚡) burada çok işe yarar.",
        inputs: &["a[32]", "b[32]", "funct3[3]"],
        outputs: &["out[32]"],
        test: Test::Function(|i| vec![mul_div(i[2] as u32, i[0] as u32, i[1] as u32) as u64]),
    },
    Level {
        chip: "Cpu",
        section: CPU,
        title: "RISC-V İşlemci",
        description: "Hepsini birleştir! Her saat vuruşunda bir komut:\n\
            1. pc'deki komut instr girişinden gelir. Komut Çözücü ile parçala.\n\
            2. Register Dosyası'ndan rs1 ve rs2'yi oku.\n\
            3. ALU hesaplasın (b = rs2 ya da imm; a = rs1 ya da auipc için pc).\n\
            4. Yükleme/saklama: adres = ALU sonucu. addr = adresin hizalı hali (alt 2 bit 0), \
            off = alt 2 bit. Okunan word rdata'dan gelir.\n\
            5. Sonucu (wb_sel'e göre) rd'ye yaz.\n\
            6. Sıradaki pc: normalde pc + 4; jal: pc + imm; jalr: (rs1 + imm) & ~1; \
            dallanma alınırsa pc + imm.\n\n\
            Bellek dışarıda: pc çıkışındaki adresteki komut instr'e, addr'deki kelime rdata'ya \
            gelir. we = 1 iken saat vuruşunda wdata, wmask'taki baytlara yazılır.\n\n\
            reset = 1 iken saat vuruşunda pc 0 olsun.\n\n\
            Test, işlemcinde gerçek programlar çalıştırır ve her komuttan sonra register'ları \
            ve bellek yazımlarını bir referans işlemciyle karşılaştırır.",
        inputs: &["instr[32]", "rdata[32]", "reset", "clk"],
        outputs: &["pc[32]", "addr[32]", "wdata[32]", "wmask[4]", "we"],
        test: Test::Custom(cpu_test),
    },
];

pub fn decoder_reference(i: &[u64]) -> Vec<Option<u64>> {
    let ins = i[0] as u32;
    let imm = match opcode(ins) {
        OP_LOAD | OP_IMM | OP_JALR => Some(imm_i(ins)),
        OP_STORE => Some(imm_s(ins)),
        OP_BRANCH => Some(imm_b(ins)),
        OP_LUI | OP_AUIPC => Some(imm_u(ins)),
        OP_JAL => Some(imm_j(ins)),
        _ => None,
    };
    vec![
        Some(opcode(ins) as u64),
        Some(rd(ins) as u64),
        Some(funct3(ins) as u64),
        Some(rs1(ins) as u64),
        Some(rs2(ins) as u64),
        imm.map(|v| v as u64),
    ]
}

/// Control signals for a valid instruction, or all "don't care".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signals {
    pub reg_we: u64,
    pub alu_b_imm: u64,
    pub alu_op: u64,
    pub wb_sel: u64,
    pub mem_read: u64,
    pub mem_write: u64,
    pub branch: u64,
    pub jal: u64,
    pub jalr: u64,
    pub auipc: u64,
    pub mul: u64,
}

pub fn control(opcode: u32, f3: u32, f7: u32) -> Option<Signals> {
    let s = Signals::default();
    Some(match opcode {
        OP_LUI => Signals {
            reg_we: 1,
            wb_sel: 3,
            ..s
        },
        OP_AUIPC => Signals {
            reg_we: 1,
            alu_b_imm: 1,
            auipc: 1,
            ..s
        },
        OP_JAL => Signals {
            reg_we: 1,
            wb_sel: 2,
            jal: 1,
            ..s
        },
        OP_JALR if f3 == 0 => Signals {
            reg_we: 1,
            alu_b_imm: 1,
            wb_sel: 2,
            jalr: 1,
            ..s
        },
        OP_BRANCH if f3 != 2 && f3 != 3 => Signals { branch: 1, ..s },
        OP_LOAD if f3 != 3 && f3 <= 5 => Signals {
            reg_we: 1,
            alu_b_imm: 1,
            wb_sel: 1,
            mem_read: 1,
            ..s
        },
        OP_STORE if f3 <= 2 => Signals {
            alu_b_imm: 1,
            mem_write: 1,
            ..s
        },
        OP_IMM => {
            let alt = if f3 == 5 && f7 == 0x20 { 8 } else { 0 };
            if (f3 == 1 && f7 != 0) || (f3 == 5 && f7 != 0 && f7 != 0x20) {
                return None;
            }
            Signals {
                reg_we: 1,
                alu_b_imm: 1,
                alu_op: f3 as u64 | alt,
                ..s
            }
        }
        OP_REG => match f7 {
            0 => Signals {
                reg_we: 1,
                alu_op: f3 as u64,
                ..s
            },
            0x20 if f3 == 0 || f3 == 5 => Signals {
                reg_we: 1,
                alu_op: f3 as u64 | 8,
                ..s
            },
            1 => Signals {
                reg_we: 1,
                mul: 1,
                ..s
            },
            _ => return None,
        },
        _ => return None,
    })
}

fn control_reference(i: &[u64]) -> Vec<Option<u64>> {
    match control(i[0] as u32, i[1] as u32, i[2] as u32) {
        None => vec![None; 11],
        Some(s) => {
            // Signals that cannot matter for this instruction are "don't care":
            // alu_op when the ALU result is unused (lui, jal, mul).
            let alu_used = s.wb_sel == 0 && s.mul == 0 && s.branch == 0
                || s.mem_read == 1
                || s.mem_write == 1
                || s.jalr == 1;
            vec![
                Some(s.reg_we),
                Some(s.alu_b_imm).filter(|_| alu_used || s.auipc == 1),
                Some(s.alu_op).filter(|_| alu_used || s.auipc == 1),
                Some(s.wb_sel).filter(|_| s.reg_we == 1),
                Some(s.mem_read),
                Some(s.mem_write),
                Some(s.branch),
                Some(s.jal),
                Some(s.jalr),
                Some(s.auipc),
                Some(s.mul),
            ]
        }
    }
}

fn regfile_steps() -> Vec<TestStep> {
    let mut rng = Rng(7);
    let mut regs = [0u64; 32];
    let mut known = [false; 32];
    known[0] = true;
    let mut steps = Vec::new();
    let read = |regs: &[u64; 32], known: &[bool; 32], r: u64| {
        known[r as usize].then_some(regs[r as usize])
    };
    for cycle in 0..400 {
        // Early on, write every register once so reads become checkable.
        let rd = if cycle < 32 {
            cycle
        } else {
            rng.next_u64() % 32
        };
        let d = rng.next_u64() & mask(32);
        let we = if cycle < 32 { 1 } else { rng.bit() };
        let (rs1, rs2) = (rng.next_u64() % 32, rng.next_u64() % 32);
        let expect = |regs: &[u64; 32], known: &[bool; 32]| {
            vec![read(regs, known, rs1), read(regs, known, rs2)]
        };
        steps.push(TestStep {
            inputs: vec![rs1, rs2, rd, d, we, 0],
            expect: expect(&regs, &known),
        });
        if we == 1 && rd != 0 {
            regs[rd as usize] = d;
            known[rd as usize] = true;
        }
        steps.push(TestStep {
            inputs: vec![rs1, rs2, rd, d, we, 1],
            expect: expect(&regs, &known),
        });
        // Reading the register just written must show the new value.
        steps.push(TestStep {
            inputs: vec![rd, rs2, rd, rng.next_u64() & mask(32), we, 1],
            expect: vec![read(&regs, &known, rd), read(&regs, &known, rs2)],
        });
    }
    steps
}

// ---------------------------------------------------------------------------
// Running programs on a circuit CPU

pub const CPU_IN_INSTR: usize = 0;
pub const CPU_IN_RDATA: usize = 1;
pub const CPU_IN_RESET: usize = 2;
pub const CPU_IN_CLK: usize = 3;
pub const CPU_OUT_PC: usize = 0;
pub const CPU_OUT_ADDR: usize = 1;
pub const CPU_OUT_WDATA: usize = 2;
pub const CPU_OUT_WMASK: usize = 3;
pub const CPU_OUT_WE: usize = 4;

/// What one instruction did on a circuit CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cycle {
    pub pc: u32,
    pub instr: u32,
    /// (aligned address, data, byte mask)
    pub store: Option<(u32, u32, u8)>,
    /// Clock cycles it took: 1, or more for a multiply or divide on a CPU
    /// that waits for a multi-step unit.
    pub cycles: u32,
}

/// Clock cycles a multiply or divide may hold the pc for.
pub const MAX_STALL: u32 = 80;

/// Drives a compiled CPU circuit: holds reset, then runs one instruction per
/// clock cycle against a memory.
pub struct CircuitCpu {
    pub circuit: Circuit,
}

impl CircuitCpu {
    pub fn new(mut circuit: Circuit) -> Self {
        circuit.set_native_lanes(1);
        Self { circuit }
    }

    /// Clear all state and hold reset through one clock edge, so pc = 0.
    pub fn reset(&mut self) -> Result<(), String> {
        let c = &mut self.circuit;
        c.reset();
        c.set_input(CPU_IN_INSTR, NOP as u64);
        c.set_input(CPU_IN_RDATA, 0);
        c.set_input(CPU_IN_RESET, 1);
        for clk in [0, 1, 0] {
            c.set_input(CPU_IN_CLK, clk);
            c.eval().map_err(|e| e.to_string())?;
        }
        c.set_input(CPU_IN_RESET, 0);
        Ok(())
    }

    pub fn pc(&self) -> u32 {
        self.circuit.output(CPU_OUT_PC) as u32
    }

    /// Run one instruction. Memory is only read for load instructions, so
    /// devices with side effects (keyboard) see exactly the program's reads.
    /// A multiply or divide that keeps the pc where it is is clocked again
    /// until the pc moves on: the CPU is waiting for a multi-step unit.
    pub fn step<M: Memory>(&mut self, mem: &mut M) -> Result<Cycle, String> {
        let mut cycle = self.clock(mem)?;
        let m_ext = opcode(cycle.instr) == OP_REG && funct7(cycle.instr) == 1;
        while m_ext && self.pc() == cycle.pc {
            if cycle.cycles == MAX_STALL {
                return Err(trf!(
                    "0x{:04X}: '{}' {MAX_STALL} saatte bitmedi; pc ilerlemiyor.",
                    "0x{:04X}: '{}' did not finish in {MAX_STALL} clocks; the pc does not move on.",
                    cycle.pc,
                    makine_rv::disasm::disasm(cycle.instr, cycle.pc)
                ));
            }
            let next = self.clock(mem)?;
            if next.store.is_some() {
                return Err(trf!(
                    "0x{:04X}: '{}' beklerken belleğe yazdı.",
                    "0x{:04X}: '{}' wrote to memory while waiting.",
                    cycle.pc,
                    makine_rv::disasm::disasm(cycle.instr, cycle.pc)
                ));
            }
            cycle.cycles += 1;
        }
        Ok(cycle)
    }

    /// Run one clock cycle.
    fn clock<M: Memory>(&mut self, mem: &mut M) -> Result<Cycle, String> {
        let c = &mut self.circuit;
        let osc = |e: makine_sim::Oscillation| e.to_string();
        // Clock low: logic settles for the instruction at pc.
        c.set_input(CPU_IN_CLK, 0);
        c.eval().map_err(osc)?;
        let pc = c.output(CPU_OUT_PC) as u32;
        let instr = mem.fetch(pc).ok_or_else(|| {
            trf!(
                "pc = 0x{pc:08X}: orada bellek yok",
                "pc = 0x{pc:08X}: there is no memory there"
            )
        })?;
        c.set_input(CPU_IN_INSTR, instr as u64);
        c.eval().map_err(osc)?;
        let rdata = if opcode(instr) == OP_LOAD {
            let addr = c.output(CPU_OUT_ADDR) as u32 & !3;
            mem.read(addr).ok_or_else(|| {
                trf!(
                    "0x{addr:08X} adresinden okunamaz: orada bellek yok",
                    "cannot read from 0x{addr:08X}: there is no memory there"
                )
            })?
        } else {
            0
        };
        c.set_input(CPU_IN_RDATA, rdata as u64);
        c.eval().map_err(osc)?;
        let store = (c.output(CPU_OUT_WE) & 1 == 1).then(|| {
            (
                c.output(CPU_OUT_ADDR) as u32 & !3,
                c.output(CPU_OUT_WDATA) as u32,
                c.output(CPU_OUT_WMASK) as u8,
            )
        });
        // Rising edge: registers update.
        c.set_input(CPU_IN_CLK, 1);
        c.eval().map_err(osc)?;
        if let Some((a, d, m)) = store {
            mem.write(a, d, m).ok_or_else(|| {
                trf!(
                    "0x{a:08X} adresine yazılamaz: orada bellek yok",
                    "cannot write to 0x{a:08X}: there is no memory there"
                )
            })?;
        }
        Ok(Cycle {
            pc,
            instr,
            store,
            cycles: 1,
        })
    }
}

// ---------------------------------------------------------------------------
// The CPU level's test

/// Programs the CPU test runs. Each ends with `ebreak`.
pub const TEST_PROGRAMS: &[(&str, &str)] = &[
    ("aritmetik", include_str!("programs/arith.s")),
    ("dallanma", include_str!("programs/branch.s")),
    ("bellek", include_str!("programs/memory.s")),
    ("fonksiyon", include_str!("programs/calls.s")),
    ("çarpma/bölme", include_str!("programs/muldiv.s")),
];

/// A test program's name in the interface language.
fn program_name(name: &str) -> &str {
    match name {
        "aritmetik" => tr!("aritmetik", "arithmetic"),
        "dallanma" => tr!("dallanma", "branch"),
        "bellek" => tr!("bellek", "memory"),
        "fonksiyon" => tr!("fonksiyon", "function"),
        "çarpma/bölme" => tr!("çarpma/bölme", "multiply/divide"),
        other => other,
    }
}

/// RAM for tests: a flat vector of words, nothing else.
#[derive(Clone)]
pub struct TestRam(pub Vec<u32>);

impl TestRam {
    pub fn new(bytes: usize, image: &[u8]) -> Self {
        let mut words = vec![0u32; bytes / 4];
        for (i, c) in image.chunks(4).enumerate() {
            let mut w = [0u8; 4];
            w[..c.len()].copy_from_slice(c);
            words[i] = u32::from_le_bytes(w);
        }
        Self(words)
    }
}

impl Memory for TestRam {
    fn fetch(&mut self, addr: u32) -> Option<u32> {
        self.0.get(addr as usize / 4).copied()
    }
    fn read(&mut self, addr: u32) -> Option<u32> {
        self.0.get(addr as usize / 4).copied()
    }
    fn write(&mut self, addr: u32, value: u32, bytes: u8) -> Option<()> {
        let w = self.0.get_mut(addr as usize / 4)?;
        let m = byte_mask(bytes);
        *w = (*w & !m) | (value & m);
        Some(())
    }
}

pub const TEST_RAM_BYTES: usize = 16 * 1024;
const TEST_MAX_STEPS: usize = 50_000;

pub(crate) fn cpu_test(circuit: &mut Circuit) -> Verdict {
    let mut cpu = CircuitCpu::new(circuit.clone());
    let mut steps = 0;
    for (name, src) in TEST_PROGRAMS {
        let prog = makine_rv::asm::assemble(src).expect("test programs assemble");
        if let Err(message) = run_lockstep(&mut cpu, &prog.bytes, &mut steps) {
            return Verdict::Wrong {
                step: steps,
                message: trf!(
                    "'{}' programı: {message}",
                    "'{}' program: {message}",
                    program_name(name)
                ),
            };
        }
    }
    Verdict::Pass
}

/// The last instruction that wrote a register, for explaining mistakes.
#[derive(Clone, Copy, Default)]
struct Writer {
    pc: u32,
    instr: u32,
    rs1: u32,
    rs2: u32,
}

fn reg(n: usize) -> String {
    format!("x{n} ({})", REG_NAMES[n])
}

fn describe(pc: u32, instr: u32) -> String {
    format!("0x{pc:04X}: '{}'", makine_rv::disasm::disasm(instr, pc))
}

/// Run a program on the circuit CPU and the reference side by side,
/// comparing the pc and every memory write after each instruction.
/// Registers are inside the circuit; a wrong register shows up as soon as it
/// is stored or steers a branch, and the message traces it back.
pub fn run_lockstep(cpu: &mut CircuitCpu, image: &[u8], steps: &mut usize) -> Result<(), String> {
    let mut ram = TestRam::new(TEST_RAM_BYTES, image);
    let mut ref_ram = ram.clone();
    let mut reference = Cpu::new();
    let mut writers = [Writer::default(); 32];
    let mut prev: Option<Writer> = None;
    cpu.reset()?;

    let blame = |r: usize, reference: &Cpu, writers: &[Writer; 32]| -> String {
        if r == 0 {
            return tr!("x0 her zaman 0 olmalı.", "x0 must always be 0.").into();
        }
        let w = writers[r];
        if w.instr == 0 {
            return trf!(
                "{} bu programda hiç yazılmadı.",
                "{} was never written in this program.",
                reg(r)
            );
        }
        trf!(
            "{} = 0x{:08X} olmalıydı. Bu değeri en son {} yazdı (o anda {} = 0x{:08X}, {} = 0x{:08X}).",
            "{} should have been 0x{:08X}. It was last written by {} (at that time {} = 0x{:08X}, {} = 0x{:08X}).",
            reg(r),
            reference.x[r],
            describe(w.pc, w.instr),
            reg(rs1(w.instr)),
            w.rs1,
            reg(rs2(w.instr)),
            w.rs2
        )
    };

    for _ in 0..TEST_MAX_STEPS {
        *steps += 1;
        let pc = reference.pc;
        let instr = ref_ram.0.get(pc as usize / 4).copied().unwrap_or(0);
        let now = Writer {
            pc,
            instr,
            rs1: reference.x[rs1(instr)],
            rs2: reference.x[rs2(instr)],
        };
        match reference.step(&mut ref_ram) {
            Ok(()) => {}
            Err(Trap::Breakpoint) => return Ok(()),
            Err(t) => {
                return Err(trf!(
                    "referans işlemci durdu: {t}",
                    "the reference processor stopped: {t}"
                ))
            }
        }
        let cycle = cpu.step(&mut ram)?;

        if cycle.pc != pc {
            let mut msg = trf!(
                "pc = 0x{pc:04X} olmalıydı, senin işlemcinde 0x{:04X}.",
                "pc should have been 0x{pc:04X}, in your processor it is 0x{:04X}.",
                cycle.pc
            );
            if let Some(p) = prev {
                msg += &trf!(
                    " Bir önceki komut {} idi",
                    " The previous instruction was {}",
                    describe(p.pc, p.instr)
                );
                if opcode(p.instr) == OP_BRANCH {
                    msg += &format!(
                        " ({} = 0x{:08X}, {} = 0x{:08X})",
                        reg(rs1(p.instr)),
                        p.rs1,
                        reg(rs2(p.instr)),
                        p.rs2
                    );
                }
                msg += ".";
            }
            return Err(msg);
        }

        let want = reference.last_store.map(|s| (s.addr, s.masked(), s.bytes));
        let got = cycle.store.map(|(a, d, m)| (a, d & byte_mask(m), m));
        if want != got {
            let show = |s: Option<(u32, u32, u8)>| match s {
                None => tr!("yazma yok", "no write").to_string(),
                Some((a, d, m)) => trf!(
                    "0x{a:04X} adresine 0x{d:08X} (bayt maskesi {m:04b})",
                    "0x{d:08X} to 0x{a:04X} (byte mask {m:04b})"
                ),
            };
            let mut msg = trf!(
                "{} komutunda bellek yazımı yanlış. Beklenen: {}. Senin işlemcin: {}.",
                "Wrong memory write at instruction {}. Expected: {}. Your processor: {}.",
                describe(pc, instr),
                show(want),
                show(got)
            );
            if let (Some(w), Some(g)) = (want, got) {
                if w.0 != g.0 {
                    msg += &trf!(
                        " Adres yanlış. {}",
                        " Wrong address. {}",
                        blame(rs1(instr), &reference, &writers)
                    );
                } else if w.1 != g.1 {
                    msg += &trf!(
                        " Veri yanlış. {}",
                        " Wrong data. {}",
                        blame(rs2(instr), &reference, &writers)
                    );
                }
            }
            return Err(msg);
        }

        let rd = rd(instr);
        let writes_rd = !matches!(opcode(instr), OP_BRANCH | OP_STORE | OP_FENCE | OP_SYSTEM);
        if writes_rd && rd != 0 {
            writers[rd] = now;
        }
        prev = Some(now);
    }
    Err(trf!(
        "program {TEST_MAX_STEPS} adımda bitmedi",
        "the program did not finish in {TEST_MAX_STEPS} steps"
    ))
}
