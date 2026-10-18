//! A prerequisite-ordered course for designing an instruction set. Answers
//! are checked against encodings, control tables and actual circuit runs,
//! not merely by clicking a read checkbox.

use makine_lang::{tr, trf};
use makine_rv::smallasm::{Definition, Dialect, Instruction, Operand};

pub struct Lesson {
    pub id: &'static str,
    pub title: &'static str,
    pub objective: &'static str,
    pub explanation: &'static str,
    pub example: &'static str,
    pub task: &'static str,
    pub hint: &'static str,
    pub answer: &'static str,
    pub chip: Option<&'static str>,
}

pub fn all() -> Vec<Lesson> {
    vec![
    Lesson{id:"isa-contract",title:tr!("1. Makineye söz ver", "1. Define the machine contract"),
        objective:tr!("ISA ile donanımın görevlerini ayır.","Separate the ISA from its hardware implementation."),
        explanation:tr!("ISA, programcıyla donanım arasındaki sözleşmedir: komut bitleri, görünen kayıtlar ve bir komutun etkisi. Devrenin nerede durduğu veya kaç NAND kullandığı ISA değildir. Bu eğitimde 8-bit acc, 4-bit pc, 16 komutluk ROM ve 16 bayt RAM seçiyoruz. LDI sayıyı acc'ye koyar; LD aynı sayıyı adres olarak yorumlayıp RAM'i okur. Aynı arg alanı komuta göre farklı anlama gelebilir.","An ISA is a contract between programmer and hardware: instruction bits, visible state and each instruction's effects. Layout and NAND count are not the ISA. Here we choose 8-bit acc, 4-bit pc, 16 ROM words and 16 RAM bytes. LDI puts its operand in acc; LD treats it as an address and reads RAM. The same operand field can mean different things in different instructions."),
        example:tr!("LDI 3 → acc=3. LD 3 ve RAM[3]=9 → acc=9. ST 3 → RAM[3]=acc; acc değişmez.","LDI 3 → acc=3. LD 3 with RAM[3]=9 → acc=9. ST 3 → RAM[3]=acc; acc is unchanged."),
        task:tr!("Şimdi RAM[3]=12 olsun. LDI 3 ardından LD 3 çalışırsa acc kaç olur?", "Now let RAM[3]=12. What is acc after LDI 3 followed by LD 3?"),hint:tr!("İkinci komut bir sabit değil bellek okur.","The second instruction reads memory, not a constant."),answer:"12",chip:None},
    Lesson{id:"isa-fields",title:tr!("2. Bit bütçesi", "2. Budget the bits"),
        objective:tr!("Opcode ile argümanın kapasitesini hesapla.","Calculate opcode and operand capacity."),
        explanation:tr!("8 bitlik komutu üstte 4 bit opcode, altta 4 bit arg olarak böl. 2^4=16 farklı opcode ve 0–15 argüman elde edilir. acc'nin 8 bit olması LDI'nin de 8-bit sabit taşıdığı anlamına gelmez: sabit için yalnızca dört bit ayırdık. Daha geniş sabit için daha uzun komut, ikinci kelime veya ek komut gerekir.","Split an 8-bit instruction into a high 4-bit opcode and a low 4-bit operand. You get 2^4=16 opcode choices and operands 0–15. An 8-bit accumulator does not mean LDI holds an 8-bit constant: only four instruction bits are available. Larger constants require longer instructions, a second word or extra instructions."),
        example:"0100 0111 = (4 << 4) | 7 = 0x47",
        task:tr!("0xA3 komutunun opcode ve arg değerini virgülle ayırarak onluk yaz.","Enter opcode and operand for 0xA3 in decimal, separated by a comma."),hint:tr!("Üst nibble A onluk 10'dur; alt nibble 3.","The high nibble A is decimal 10; the low nibble is 3."),answer:"10,3",chip:Some("MiniDecode")},
    Lesson{id:"isa-encoding",title:tr!("3. Kendi opcode tablon", "3. Your opcode table"),
        objective:tr!("Çakışmayan bir kodlama kur ve assembler'a tanıt.","Build a non-overlapping encoding and give it to the assembler."),
        explanation:tr!("Komut adları insanlar içindir; devre yalnızca bitleri görür. Bu yolun sınanabilir sözleşmesi: NOP=0, LDI=1, LD=2, ST=3, ADD=4, SUB=5, JMP=6, JZ=7, HLT=15. LDI sayı, diğer bellek/atlama komutları adres alır; NOP ve HLT argümansızdır. Serbest tasarımda başka opcode seçebilirsin ama bu dersin sonraki devreleri bu tabloyu uygular. Tabloyu değiştirmek donanımı otomatik değiştirmez.","Names are for people; the circuit sees bits. This course's testable contract is NOP=0, LDI=1, LD=2, ST=3, ADD=4, SUB=5, JMP=6, JZ=7, HLT=15. LDI takes a value; memory/jump instructions take addresses; NOP and HLT take none. Free designs may choose other opcodes, but later course circuits implement this table. Editing the table never automatically changes hardware."),
        example:"LDI 5 = 0x15   ADD 2 = 0x42   HLT = 0xF0",
        task:tr!("Soldaki komut seti tablosunu bu sözleşmeyle eşleştir. Komut adları ve argüman türleri de kontrol edilir.","Match the instruction-set table to this contract. Names and operand types are checked too."),hint:tr!("Ders taslağı aç boş tablo oluşturur; yalnızca alanlar doldurulur, kaydedilmiş tasarımların değişmez.","Open lesson draft creates a blank table in a separate saved design; your existing designs stay unchanged."),answer:"",chip:None},
    Lesson{id:"isa-assembly",title:tr!("4. Etiketlerden ROM'a", "4. From labels to ROM"),
        objective:tr!("Assembler'ın iki geçişini ve adres birimini anla.","Understand two-pass assembly and address units."),
        explanation:tr!("İlk geçiş her komutun ROM adresini sayar ve etiketleri adreslere bağlar. İkinci geçiş adları ve argümanları kodlar. Bu makinede her komut bir ROM kelimesi; done etiketi beşinci komuttaysa adresi 4'tür. RISC-V'deki bayt adresini ve pc+4'ü buraya taşıma. Etiket, kendi başına ROM'da yer kaplamaz.","Pass one counts instruction addresses and binds labels. Pass two encodes names and operands. Each instruction here occupies one ROM word; a label on the fifth instruction has address 4. Do not import RISC-V byte addresses or pc+4. A label itself occupies no ROM word."),
        example:"LDI 3\nST 2\nJMP done\nLDI 9\ndone: HLT",
        task:tr!("Yukarıdaki örnekte JMP done hangi hex baytıdır?", "What hex byte encodes JMP done in the example?"),hint:tr!("JMP opcode 6, done adres 4: 0x60 | 4.","JMP opcode 6, done address 4: 0x60 | 4."),answer:"0x64",chip:None},
    Lesson{id:"isa-decode",title:tr!("5. Bitleri ayıran devre", "5. The field-splitting circuit"),
        objective:tr!("Assembler'ın ürettiği kelimeyi donanımda çöz.","Decode the emitted word in hardware."),
        explanation:tr!("Assembler ROM'a bir sayı koydu. Devrede MiniDecode bu sayının alt dört bitini arg'a, üst dört bitini op'a taşır. Bu bir tablo araması ya da emülatör değil: doğru telleri doğru çıkışa bağlamaktır. Böl parçasının grupları düşük bitten başlar; 4,4 çıkışlarının ilki arg, ikincisi op olur. 0x00, 0x0F, 0xF0, 0xFF ile sınırları dene.","The assembler put a number in ROM. MiniDecode routes its low four bits to arg and high four to op. This is wiring, not a lookup emulator. Split groups start at the low bit: with 4,4 the first output is arg, the second op. Try boundaries 0x00,0x0F,0xF0,0xFF."),
        example:"instr=0xA3 → op=10, arg=3",
        task:tr!("Seviyelerde MiniDecode devreni kur ve testten geçir. Sonra buraya dönüp kontrol et.","Build and pass MiniDecode in Levels, then return and check."),hint:tr!("Böl 4,4 yeter; kapı gerekmez.","Split 4,4 is sufficient; no gate is needed."),answer:"",chip:Some("MiniDecode")},
    Lesson{id:"isa-control",title:tr!("6. Kontrol tablosu", "6. The control table"),
        objective:tr!("Komutun niyetini kontrol sinyallerine dönüştür.","Turn instruction intent into control signals."),
        explanation:tr!("Sinyal sırası: load, imm, alu, sub, we, jump, hold. ADD için acc yazılır ve ALU sonucu seçilir: 1,0,1,0,0,0,0. ST yalnızca we=1 yapar: acc'yi değiştirmez. HLT yalnızca hold=1 yapar. JZ, zero=1 ise jump üretir. Bu tablo bir sonraki dersin kapılarla kurulacak fonksiyonudur; her bit için hangi opcode'larda 1 olduğunu listele.","Signal order: load,imm,alu,sub,we,jump,hold. ADD writes acc and chooses ALU output: 1,0,1,0,0,0,0. ST sets only we, preserving acc. HLT sets only hold. JZ asserts jump only if zero=1. This table defines the gate function you build next; list which opcodes set each output bit."),
        example:"ADD: 1010000   ST: 0000100   HLT: 0000001",
        task:tr!("SUB için load,imm,alu,sub,we,jump,hold sinyallerini bu sırayla yedi bit olarak yaz.", "For SUB, enter load,imm,alu,sub,we,jump,hold in that order as seven bits."),hint:tr!("Sonuç yazılır, ALU seçilir ve çıkarma istenir: load,alu,sub 1; diğerleri 0.","Write the result, select ALU and request subtraction: load,alu,sub are 1; the rest 0."),answer:"1011000",chip:Some("MiniControl")},
    Lesson{id:"isa-control-gates",title:tr!("7. Tabloyu kapılara çevir", "7. Turn the table into gates"),
        objective:tr!("Opcode karşılaştırıcısı ve OR toplamları kur.","Build opcode comparisons and OR combinations."),
        explanation:tr!("op=5 demek bit0=1, bit1=0, bit2=1, bit3=0 demektir: bit0 AND NOT bit1 AND bit2 AND NOT bit3. Her komut için böyle bir eşitlik sinyali çıkar. load=LDI OR LD OR ADD OR SUB; alu=ADD OR SUB; jump=JMP OR (JZ AND zero). Tanımsız opcode'larda bütün sinyaller sıfırdır. Kontrol kombinasyoneldir; burada saat veya kayıt gerekmez.","op=5 means bit0=1,bit1=0,bit2=1,bit3=0: bit0 AND NOT bit1 AND bit2 AND NOT bit3. Form an equality signal for each opcode. load=LDI OR LD OR ADD OR SUB; alu=ADD OR SUB; jump=JMP OR (JZ AND zero). Undefined opcodes have all signals zero. Control is combinational: it needs no clock or register."),
        example:"is_SUB = op[0] AND !op[1] AND op[2] AND !op[3]",
        task:tr!("Kendi MiniControl çipini kurup test et. Burada yalnızca daha önce geçmiş olman değil, güncel kayıtlı çözümün yeniden sınanır.","Build and pass MiniControl. The current saved solution is checked again, not merely its old completion flag."),hint:tr!("Sıfır için ayrı MiniZero8 devresi gerekir. Önce AddSub8, MiniZero8, MiniPc4 ve MiniAcc8 ara seviyelerini tamamla.","Zero needs its own MiniZero8 circuit. Complete AddSub8, MiniZero8, MiniPc4 and MiniAcc8 intermediate levels first."),answer:"",chip:Some("MiniControl")},
    Lesson{id:"isa-state",title:tr!("8. Durum ve saat kenarı", "8. State and clock edges"),
        objective:tr!("Sürekli hesaplama ile kenarda yazmayı ayır.","Separate continuous computation from edge-triggered storage."),
        explanation:tr!("ALU ve MUX'lar giriş değişince hesaplar; acc ve pc yalnızca yükselen kenarda yeni değeri alır. acc için reset > load > koru. pc için reset > hold > jump > pc+1. JZ eski acc'nin sıfır olup olmadığına bakar. ST de kenardan önceki acc'yi belleğe yazar. MiniZero8 tüm sekiz biti kontrol etmelidir; yalnızca en üst bitin sıfır olması yeterli değildir.","ALU and MUX outputs follow inputs; acc and pc store a new value only at a rising edge. acc priority: reset > load > keep. pc priority: reset > hold > jump > pc+1. JZ tests the old acc. ST writes the pre-edge accumulator. MiniZero8 must check all eight bits; a clear top bit alone is not enough."),
        example:"pc=9, target=3, jump=1, hold=1, reset=1 → next pc=0",
        task:tr!("Aynı örnekte reset=0 olsaydı next pc kaç olurdu?", "What would next pc be in the same example with reset=0?"),hint:tr!("hold, jump'tan önceliklidir.","hold takes priority over jump."),answer:"9",chip:Some("MiniPc4")},
    Lesson{id:"isa-wiring",title:tr!("9. Veri yolunu birleştir", "9. Assemble the datapath"),
        objective:tr!("Çözücü, ALU, kayıt ve kontrolü çalışan CPU yap.","Connect decoder, ALU, state and control into a CPU."),
        explanation:tr!("MiniDecode → op MiniControl'e, arg adres çıkışına gider. MiniAcc8.q hem ALU'nun a'sına hem MiniZero8'e gider. rdata ve AddSub8 çıktısı arasında alu seçer; bu sonuçla sıfırla genişletilmiş arg arasında imm seçer. Sonuç MiniAcc8.d'ye, load kontrolü yazma iznine gider. MiniPc4 target=arg, jump/hold kontrol sinyalleridir. RAM yazma verisi her zaman mevcut acc'dir. reset ve clk her iki kayda da bağlanır.","MiniDecode sends op to MiniControl and arg to the address output. MiniAcc8.q feeds ALU input a and MiniZero8. alu chooses rdata or AddSub8's result; imm chooses that result or zero-extended arg. Feed this into MiniAcc8.d and load into write enable. MiniPc4 gets target=arg and jump/hold from control. RAM write data is the current acc. Wire reset and clk to both registers."),
        example:"ROM[pc] → instr → decode → control / datapath → next acc, next pc",
        task:tr!("MiniCpu'yu kendi ara çiplerinle kurup test et. ROM ve veri RAM'i bu çipin dışında kalmalı.","Build and test MiniCpu using your intermediate chips. ROM and data RAM remain outside this chip."),hint:tr!("Önce LDI, sonra ADD, ST, JMP, JZ ve HLT yolunu tek tek izle.","Trace LDI first, then ADD, ST, JMP, JZ and HLT one at a time."),answer:"",chip:Some("MiniCpu")},
    Lesson{id:"isa-run",title:tr!("10. Kendi ISA'n kendi devrende", "10. Your ISA on your circuit"),
        objective:tr!("Özel serbest CPU'yu programlarla doğrula.","Verify your custom free CPU with real programs."),
        explanation:tr!("Eğitimdeki sabit sözleşme için kelime 8 bit, adres 4 bittir. Serbest CPU'nun girişleri instr[8], rdata[8], reset, clk; çıkışları pc[4], addr[4], wdata[8], we olmalı. MiniCpu'nun acc çıkışı burada wdata adını alır; donanım yine aynı eski acc değerini verir. Tablo, assembler ve devrenin üçü de anlaşmalı. Bu son kontrol ROM'u senin kaydedilmiş çipine bağlayıp yükleme/saklama, toplama, dallanma, durma, taşma ve yeniden reset denemelerini yürütür; referans CPU'ya geri düşmez.","The course contract uses 8-bit words and 4-bit addresses. Your free CPU inputs are instr[8],rdata[8],reset,clk; outputs pc[4],addr[4],wdata[8],we. MiniCpu's acc output is named wdata here; hardware still exposes the old acc value. Encoding table, assembler and circuit must all agree. This final check drives ROM and RAM against your saved chip, testing loads/stores, arithmetic, branches, halt, wraparound and resetting again. It never falls back to a reference CPU."),
        example:"LDI 3\nST 0\nADD 0\nST 1\nHLT    # RAM[1]=6",
        task:tr!("CPU çip adını gir. Test, seçili örnek yerine bağımsız programları senin çipinde çalıştırır.","Enter your CPU chip name. Tests run independent programs on your chip rather than trusting the selected example."),hint:tr!("MiniCpu çözümünü temel alıp acc çıkışını wdata olarak adlandırabilirsin. Testte yalnızca takma ad değil devrenin davranışı doğrulanır.","You can start from your MiniCpu solution and rename acc to wdata. Tests verify circuit behaviour, not just its name."),answer:"",chip:None},
]
}

pub fn definition() -> Definition {
    Definition {
        bits: 8,
        opcode_bits: 4,
        instructions: [
            ("NOP", 0, Operand::None),
            ("LDI", 1, Operand::Value),
            ("LD", 2, Operand::Address),
            ("ST", 3, Operand::Address),
            ("ADD", 4, Operand::Address),
            ("SUB", 5, Operand::Address),
            ("JMP", 6, Operand::Address),
            ("JZ", 7, Operand::Address),
            ("HLT", 15, Operand::None),
        ]
        .into_iter()
        .map(|(name, opcode, operand)| Instruction {
            name: name.into(),
            opcode,
            operand,
        })
        .collect(),
    }
}

pub fn check_definition(d: &Definition) -> Result<(), String> {
    d.validate()?;
    let expected = definition();
    if d.bits != 8 || d.opcode_bits != 4 {
        return Err(tr!("Bu eğitim 8-bit kelime ve 4-bit opcode sözleşmesini kullanır. Serbest tasarımın farklı olabilir.","This course uses an 8-bit word and 4-bit opcode contract. Free designs may differ.").into());
    }
    if d.instructions.len() != expected.instructions.len() {
        return Err(tr!(
            "Sözleşmenin dokuz komutunu tanımla.",
            "Define the contract's nine instructions."
        )
        .into());
    }
    for i in expected.instructions {
        if !d.instructions.iter().any(|x| {
            x.name.eq_ignore_ascii_case(&i.name) && x.opcode == i.opcode && x.operand == i.operand
        }) {
            return Err(trf!(
                "{}: opcode {} ve doğru argüman türü gerekli.",
                "{}: requires opcode {} and the correct operand type.",
                i.name,
                i.opcode
            ));
        }
    }
    Ok(())
}

pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// Derived from the actual level specification, so the teaching table
/// cannot silently drift away from simulation behaviour.
pub fn control_rows() -> Vec<(String, Vec<u64>)> {
    makine_levels::find("MiniControl")
        .unwrap()
        .steps()
        .into_iter()
        .filter(|s| {
            let (op, zero) = (s.inputs[0], s.inputs[1]);
            (op <= 7 || op == 15) && (op == 7 || zero == 0)
        })
        .map(|s| {
            let op = s.inputs[0];
            let zero = s.inputs[1];
            let name = definition()
                .instructions
                .into_iter()
                .find(|i| i.opcode as u64 == op)
                .unwrap()
                .name;
            (
                if op == 7 {
                    format!("{name} zero={zero}")
                } else {
                    name
                },
                s.expect.into_iter().map(Option::unwrap).collect(),
            )
        })
        .collect()
}

pub fn fixtures() -> Vec<(&'static str, &'static str, [u8; 16], usize)> {
    let mut ram = [0; 16];
    ram[15] = 1;
    vec![
        (
            "load/store/add",
            "LDI 3\nST 0\nADD 0\nST 1\nHLT",
            [0; 16],
            12,
        ),
        (
            "countdown",
            "LDI 5\nloop: ST 14\nSUB 15\nJZ done\nJMP loop\ndone: HLT",
            ram,
            32,
        ),
        (
            "zero branch",
            "LDI 0\nJZ done\nLDI 9\ndone: ST 0\nHLT",
            [0; 16],
            12,
        ),
        (
            "wrap/undefined",
            "LD 15\nADD 15\nST 0\n.word 0x80\nHLT",
            [255; 16],
            12,
        ),
    ]
}

/// Tests an explicit free chip, with no reference implementation substituted.
pub fn check_cpu(lib: &makine_sim::Library, def: &makine_sim::ChipDef) -> Result<usize, String> {
    use makine_sim::parse_pins;
    if def.inputs != parse_pins(&["instr[8]", "rdata[8]", "reset", "clk"])
        || def.outputs != parse_pins(&["pc[4]", "addr[4]", "wdata[8]", "we"])
    {
        return Err(tr!("Uçlar ve sıraları eğitim sözleşmesiyle eşleşmeli: instr,rdata,reset,clk → pc,addr,wdata,we.","Pins and order must match the course contract: instr,rdata,reset,clk → pc,addr,wdata,we.").into());
    }
    let mut sim = makine_sim::Circuit::compile_def(lib, def).map_err(|e| e.to_string())?;
    sim.set_native_lanes(1);
    let mut checks = 0;
    for (name, src, initial, cycles) in fixtures() {
        let p = makine_rv::smallasm::assemble(src, Dialect::Custom(&definition()))
            .map_err(|e| e.to_string())?;
        let mut rom = [0u8; 16];
        for (d, s) in rom.iter_mut().zip(p.words) {
            *d = s as u8;
        }
        let mut ram = initial;
        let mut expected_ram = initial;
        let mut expected = makine_levels::mini::Mini::default();
        // Do not power-cycle between fixtures: reset must clear existing state.
        sim.set_input(0, 0);
        sim.set_input(1, 0);
        sim.set_input(2, 1);
        for clk in [0, 1, 0] {
            sim.set_input(3, clk);
            sim.eval().map_err(|e| e.to_string())?;
        }
        sim.set_input(2, 0);
        if sim.output(0) != 0 || sim.output(2) != 0 {
            return Err(trf!(
                "{name}: reset pc ve acc'yi sıfırlamadı.",
                "{name}: reset did not clear pc and acc."
            ));
        }
        for n in 0..cycles {
            sim.set_input(3, 0);
            sim.eval().map_err(|e| e.to_string())?;
            if sim.output(0) != expected.pc as u64 {
                return Err(trf!(
                    "{name}: {n}. adım PC beklenen {}, gerçek {}.",
                    "{name}: step {n} PC expected {}, actual {}.",
                    expected.pc,
                    sim.output(0)
                ));
            }
            let instr = rom[expected.pc as usize];
            sim.set_input(0, instr as u64);
            sim.eval().map_err(|e| e.to_string())?;
            let addr = sim.output(1);
            if addr != (instr & 15) as u64 {
                return Err(trf!(
                    "{name}: addr arg alanına eşit olmalı.",
                    "{name}: addr must equal the operand field."
                ));
            }
            sim.set_input(1, ram[addr as usize] as u64);
            sim.eval().map_err(|e| e.to_string())?;
            let write = (sim.output(3) != 0).then(|| (addr as u8, sim.output(2) as u8));
            let want = expected.step(&rom, &mut expected_ram);
            if write != want {
                return Err(trf!(
                    "{name}: {n}. adım RAM yazması yanlış: beklenen {want:?}, gerçek {write:?}.",
                    "{name}: step {n} wrong RAM write: expected {want:?}, actual {write:?}."
                ));
            }
            sim.set_input(3, 1);
            sim.eval().map_err(|e| e.to_string())?;
            if let Some((a, v)) = write {
                ram[a as usize] = v;
            }
            if sim.output(0) != expected.pc as u64 || sim.output(2) != expected.acc as u64 {
                return Err(trf!(
                    "{name}: {n}. adım sonrası beklenen pc={}, acc={}; gerçek pc={}, acc={}.",
                    "{name}: after step {n}, expected pc={}, acc={}; actual pc={}, acc={}.",
                    expected.pc,
                    expected.acc,
                    sim.output(0),
                    sim.output(2)
                ));
            }
            // No second write merely because data changes while clk stays high.
            sim.set_input(1, (ram[addr as usize] ^ 255) as u64);
            sim.eval().map_err(|e| e.to_string())?;
            if sim.output(0) != expected.pc as u64 || sim.output(2) != expected.acc as u64 {
                return Err(tr!(
                    "Saat yüksek kalırken kayıt yeniden yazıldı.",
                    "A register wrote again while the clock stayed high."
                )
                .into());
            }
            checks += 1;
        }
    }
    Ok(checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn course_answers_encodings_and_hardware_agree() {
        check_definition(&definition()).unwrap();
        let lib = makine_levels::cost::best_library();
        let mut def = makine_levels::reference::reference("MiniCpu");
        def.outputs[2].name = "wdata".into();
        assert!(check_cpu(lib, &def).unwrap() > 50);
        let mut bad = def.clone();
        bad.wires.retain(|w| w.to != makine_sim::Sink::Output(3));
        assert!(check_cpu(lib, &bad).is_err());
        let mut wrong = definition();
        wrong.instructions[4].opcode = 8;
        assert!(check_definition(&wrong).is_err());
        assert_eq!(all().len(), 10);
    }

    #[test]
    fn quiz_answers_match_field_math_control_table_and_machine() {
        let lessons = all();
        let answer = |id: &str| lessons.iter().find(|l| l.id == id).unwrap().answer;
        assert_eq!(answer("isa-fields"), format!("{},{}", 0xA3 >> 4, 0xA3 & 15));
        let sub = control_rows()
            .into_iter()
            .find(|(name, _)| name == "SUB")
            .unwrap()
            .1;
        assert_eq!(
            answer("isa-control"),
            sub.iter().map(u64::to_string).collect::<String>()
        );
        let mut rom = [0u8; 16];
        rom[..2].copy_from_slice(&[0x13, 0x23]);
        let mut ram = [0u8; 16];
        ram[3] = 12;
        let mut m = makine_levels::mini::Mini::default();
        m.step(&rom, &mut ram);
        m.step(&rom, &mut ram);
        assert_eq!(answer("isa-contract"), m.acc.to_string());
        assert_eq!(control_rows().len(), 10);
    }
}
