//! Hints for each level, from a nudge to most of the answer. A player who
//! is stuck opens them one at a time before reaching for the solution.
//!
//! 1. the idea, 2. which parts, 3. how they are wired.

use makine_lang::pick;

/// A level's hints in the interface language, the gentlest first.
pub fn hints(chip: &str) -> Vec<&'static str> {
    let (tr, en): (&[&str], &[&str]) = match chip {
        "Not" => (
            &[
                "NAND'ın iki girişine aynı sinyal gelirse ne olur? Tabloya bak: 0,0 ve 1,1 satırları.",
                "Tek bir NAND yeter.",
                "in'i NAND'ın iki girişine birden bağla; çıkışı out'a.",
            ],
            &[
                "What happens if both inputs of a NAND get the same signal? Look at the 0,0 and 1,1 rows of its table.",
                "One NAND is enough.",
                "Wire in to both inputs of the NAND; its output to out.",
            ],
        ),
        "And" => (
            &[
                "NAND, \"AND değil\" demek. AND'in tersini aldıysan, bir daha ters çevirirsen ne olur?",
                "Bir NAND ve bir NOT.",
                "a ve b → NAND → NOT → out.",
            ],
            &[
                "NAND means \"not AND\". If you have the opposite of AND, what happens when you flip it again?",
                "One NAND and one NOT.",
                "a and b → NAND → NOT → out.",
            ],
        ),
        "Or" => (
            &[
                "De Morgan: \"a VEYA b\" ile \"DEĞİL(DEĞİL a VE DEĞİL b)\" aynıdır.",
                "İki NOT ve bir NAND.",
                "a → NOT, b → NOT; ikisinin çıkışları bir NAND'a; NAND'ın çıkışı out.",
            ],
            &[
                "De Morgan: \"a OR b\" is the same as \"NOT(NOT a AND NOT b)\".",
                "Two NOTs and one NAND.",
                "a → NOT, b → NOT; both outputs into one NAND; the NAND's output to out.",
            ],
        ),
        "Xor" => (
            &[
                "XOR, \"en az biri 1\" ile \"ikisi birden değil\"in birleşimidir: (a VEYA b) VE (a NAND b).",
                "OR, NAND ve AND ile olur. Ya da dört NAND ile: n = NAND(a, b) dört yerde işe yarar.",
                "4 NAND ile: n = NAND(a, b); x = NAND(a, n); y = NAND(b, n); out = NAND(x, y).",
            ],
            &[
                "XOR is \"at least one is 1\" together with \"not both\": (a OR b) AND (a NAND b).",
                "It works with OR, NAND and AND. Or with four NANDs: n = NAND(a, b) is useful in several places.",
                "With 4 NANDs: n = NAND(a, b); x = NAND(a, n); y = NAND(b, n); out = NAND(x, y).",
            ],
        ),
        "Mux" => (
            &[
                "İki yol düşün: biri a'yı sadece sel = 0 iken, öteki b'yi sadece sel = 1 iken geçirsin. Sonra ikisini birleştir.",
                "Bir NOT, iki AND, bir OR.",
                "x = a VE (DEĞİL sel), y = b VE sel, out = x VEYA y.",
            ],
            &[
                "Think of two paths: one lets a through only when sel = 0, the other lets b through only when sel = 1. Then combine them.",
                "One NOT, two ANDs, one OR.",
                "x = a AND (NOT sel), y = b AND sel, out = x OR y.",
            ],
        ),
        "Demux" => (
            &[
                "Giriş, sel'e göre iki kapıdan yalnızca birinden geçsin.",
                "Bir NOT ve iki AND.",
                "a = in VE (DEĞİL sel), b = in VE sel.",
            ],
            &[
                "The input should pass through only one of two gates, depending on sel.",
                "One NOT and two ANDs.",
                "a = in AND (NOT sel), b = in AND sel.",
            ],
        ),
        "Mux32" => (
            &[
                "32 tane MUX gerekir ama tek tek koymak zorunda değilsin.",
                "Bir MUX koy, sağ tıkla ve Tekrar'ı 32 yap: her ucu 32 bit olur. 1 bitlik sel kendiliğinden 32 kopyaya dağılır.",
                "a → MUX×32'nin a'sı, b → b'si, sel → sel'i, çıkış → out.",
            ],
            &[
                "It takes 32 MUXes, but you do not have to place them one by one.",
                "Place a MUX, right-click it and set Repeat to 32: each pin becomes 32 bits wide. A 1-bit sel is shared by all 32 copies.",
                "a → the a of MUX×32, b → its b, sel → its sel, its output → out.",
            ],
        ),
        "HalfAdder" => (
            &[
                "Toplama tablosuna bak: sum, girişler farklıyken 1; carry, ikisi de 1 iken 1.",
                "Bir XOR ve bir AND.",
                "sum = a XOR b, carry = a VE b.",
            ],
            &[
                "Look at the addition table: sum is 1 when the inputs differ; carry is 1 when both are 1.",
                "One XOR and one AND.",
                "sum = a XOR b, carry = a AND b.",
            ],
        ),
        "FullAdder" => (
            &[
                "Üç biti toplamak, iki kere iki bit toplamaktır: önce a + b, sonra sonuca c.",
                "İki yarım toplayıcı ve bir OR.",
                "Y1 = a + b; Y2 = Y1.sum + c; sum = Y2.sum; carry = Y1.carry VEYA Y2.carry.",
            ],
            &[
                "Adding three bits is adding two bits twice: first a + b, then add c to the result.",
                "Two half adders and one OR.",
                "H1 = a + b; H2 = H1.sum + c; sum = H2.sum; carry = H1.carry OR H2.carry.",
            ],
        ),
        "Adder8" => (
            &[
                "Kâğıtta toplama gibi: her basamak kendi bitlerini ve bir öncekinin eldesini toplar.",
                "8 tam toplayıcı; a ve b'yi bitlerine ayırmak için Böl, sonucu toplamak için Birleştir.",
                "Bit i'deki tam toplayıcı: a[i], b[i] ve bit i-1'in carry'si (bit 0 için c). Son carry çıkışa.",
            ],
            &[
                "Like adding on paper: each column adds its own bits and the carry from the one before.",
                "8 full adders; Split to break a and b into bits, Join to put the sum together.",
                "The full adder for bit i gets a[i], b[i] and the carry of bit i-1 (c for bit 0). The last carry goes to the output.",
            ],
        ),
        "Adder32" => (
            &[
                "8-bit toplayıcıyı tek bir büyük basamak gibi düşün.",
                "Dört 8-bit toplayıcı; a ve b'yi 8,8,8,8 olarak böl.",
                "Her toplayıcının carry'si bir sonrakinin c'sine; dört sum'ı birleştir.",
            ],
            &[
                "Think of an 8-bit adder as one big column.",
                "Four 8-bit adders; split a and b as 8,8,8,8.",
                "Each adder's carry goes into the next one's c; join the four sums.",
            ],
        ),
        "Sub32" => (
            &[
                "İkiye tümleyen: -b = (DEĞİL b) + 1. Yani a - b = a + DEĞİL b + 1.",
                "32 bitlik bir NOT (Tekrar 32), bir 32-bit toplayıcı ve +1 için bir Sabit.",
                "b → NOT×32 → toplayıcının b'si; a → a'sı; c'ye Sabit 1.",
            ],
            &[
                "Two's complement: -b = (NOT b) + 1. So a - b = a + NOT b + 1.",
                "A 32-bit NOT (Repeat 32), a 32-bit adder, and a Constant for the +1.",
                "b → NOT×32 → the adder's b; a → its a; Constant 1 into c.",
            ],
        ),
        "Latch" => (
            &[
                "Hafıza için bir çıkışın kendine geri dönmesi gerekir: iki NAND'ı çaprazlama bağla.",
                "İki NOT ve iki NAND.",
                "q = NAND(DEĞİL set, q'), q' = NAND(DEĞİL reset, q). İki NAND'ın çıkışlarını birbirinin ikinci girişine bağla.",
            ],
            &[
                "Memory needs an output that feeds back into itself: wire two NANDs crosswise.",
                "Two NOTs and two NANDs.",
                "q = NAND(NOT set, q'), q' = NAND(NOT reset, q). Wire each NAND's output into the other's second input.",
            ],
        ),
        "DLatch" => (
            &[
                "SR mandala, en = 1 iken d = 1 ise set, d = 0 ise reset gönder.",
                "Bir SR mandal, iki AND ve bir NOT.",
                "set = d VE en, reset = (DEĞİL d) VE en.",
            ],
            &[
                "Send the SR latch set when d = 1 and reset when d = 0, only while en = 1.",
                "One SR latch, two ANDs and one NOT.",
                "set = d AND en, reset = (NOT d) AND en.",
            ],
        ),
        "DFF" => (
            &[
                "Ana ve köle: iki D mandal arka arkaya; biri saat 0 iken, öteki saat 1 iken açık.",
                "İki D mandal ve bir NOT.",
                "ana = DMandal(d, DEĞİL clk), köle = DMandal(ana, clk), q = köle.",
            ],
            &[
                "Master and slave: two D latches in a row; one is open while the clock is 0, the other while it is 1.",
                "Two D latches and one NOT.",
                "master = DLatch(d, NOT clk), slave = DLatch(master, clk), q = slave.",
            ],
        ),
        "Bit" => (
            &[
                "D flip-flop her vuruşta d'yi alır. load = 0 iken kendi eski değerini almasını sağla.",
                "Bir D flip-flop ve bir MUX.",
                "DFF.d = MUX(a: DFF.q, b: d, sel: load); DFF.clk = clk; q = DFF.q.",
            ],
            &[
                "A D flip-flop takes d on every tick. When load = 0, make it take its own old value.",
                "One D flip-flop and one MUX.",
                "DFF.d = MUX(a: DFF.q, b: d, sel: load); DFF.clk = clk; q = DFF.q.",
            ],
        ),
        "Register32" => (
            &[
                "32 tane 1-bit kayıt, hepsi aynı load ve clk ile.",
                "Bir Bit parçası, Tekrar 32.",
                "d → Bit×32'nin d'si; load ve clk ortak; çıkış q.",
            ],
            &[
                "32 one-bit registers, all with the same load and clk.",
                "One Bit part with Repeat 32.",
                "d → the d of Bit×32; load and clk shared; its output to q.",
            ],
        ),
        "Counter32" => (
            &[
                "Bir register ve önündeki seçiciler: sıradaki değer ya aynısı, ya +4, ya d, ya 0.",
                "Bir 32-bit register, bir 32-bit toplayıcı (Sabit 4 ile), üç 32-bit seçici.",
                "m1 = MUX(q, q+4, inc); m2 = MUX(m1, d, load); m3 = MUX(m2, 0, reset); register'ın d'si m3, load'u Sabit 1.",
            ],
            &[
                "A register with selectors in front: the next value is the same, +4, d, or 0.",
                "One 32-bit register, one 32-bit adder (with Constant 4), three 32-bit selectors.",
                "m1 = MUX(q, q+4, inc); m2 = MUX(m1, d, load); m3 = MUX(m2, 0, reset); the register's d is m3, its load Constant 1.",
            ],
        ),
        "Ram8" => (
            &[
                "8 register; yazarken sadece addr'ın seçtiğine load gitsin, okurken addr'ın seçtiği çıksın.",
                "8 tane 32-bit register; yazma için DEMUX ağacı, okuma için 32-bit seçici ağacı.",
                "we'yi addr'ın bitleriyle 3 kat DEMUX'tan geçir: 8 load çizgisi. Okumak için 7 tane 32-bit MUX: önce bit 0 çiftleri seçer, sonra bit 1, sonra bit 2.",
            ],
            &[
                "8 registers; when writing only the one addr picks gets load, when reading the one addr picks comes out.",
                "8 32-bit registers; a DEMUX tree for writing, a tree of 32-bit selectors for reading.",
                "Send we through 3 levels of DEMUX with addr's bits: 8 load lines. To read, 7 32-bit MUXes: bit 0 picks within pairs, then bit 1, then bit 2.",
            ],
        ),
        "Zero32" => (
            &[
                "Sayı 0 ise hiçbir biti 1 değildir: bütün bitleri VEYA'la ve tersini al.",
                "Bitlere ayırmak için Böl, OR'lar ve bir NOT.",
                "31 OR ile bütün bitleri birleştir (ağaç gibi kurarsan yol kısalır), sonra NOT.",
            ],
            &[
                "A number is 0 when none of its bits is 1: OR all the bits and invert.",
                "Split to get the bits, ORs, and one NOT.",
                "Combine all bits with 31 ORs (as a tree the path is shorter), then NOT.",
            ],
        ),
        "Compare32" => (
            &[
                "a - b'yi hesapla: sonuç 0 ise eşit. Elde biti işaretsiz karşılaştırmayı söyler.",
                "Bir 32-bit toplayıcı ile a + DEĞİL b + 1, bir sıfır testi; işaretli için en üst bitlere bakan bir XOR ve bir MUX.",
                "eq = Sıfır(fark); ltu = DEĞİL carry; lt: a ile b'nin işaretleri farklıysa a'nın işareti, aynıysa farkın işareti.",
            ],
            &[
                "Work out a - b: if the result is 0 they are equal. The carry bit gives the unsigned comparison.",
                "A 32-bit adder for a + NOT b + 1, a zero test; for signed, an XOR and a MUX looking at the top bits.",
                "eq = Zero(diff); ltu = NOT carry; lt: if the signs of a and b differ, a's sign, otherwise the sign of the difference.",
            ],
        ),
        "ShiftLeft32" => (
            &[
                "Kaydırmayı parçalara böl: n'nin her biti 1, 2, 4, 8 ya da 16 basamaklık bir kaydırmayı açar.",
                "Beş kat 32-bit seçici; her katta Böl ve Birleştir ile hazır kaydırılmış bir sürüm.",
                "Kat k: MUX(önceki, önceki 2^k sola kaymış hali, n[k]). Kaymış hal: alt 32-2^k biti 2^k sıfırın üstüne birleştir.",
            ],
            &[
                "Split the shift into steps: each bit of n turns on a shift by 1, 2, 4, 8 or 16.",
                "Five layers of 32-bit selectors; each layer has a shifted version made with Split and Join.",
                "Layer k: MUX(previous, previous shifted left by 2^k, n[k]). The shifted one: join 2^k zeros under the low 32-2^k bits.",
            ],
        ),
        "ShiftRight32" => (
            &[
                "Sola kaydırıcı gibi, ama sağa; soldan gelen bit arith'e göre 0 ya da işaret biti.",
                "Beş kat 32-bit seçici ve dolgu biti için bir AND.",
                "dolgu = a[31] VE arith; her katta üstten 2^k tane dolgu biti ekle.",
            ],
            &[
                "Like the left shifter, but to the right; the bit coming in from the left is 0 or the sign bit, depending on arith.",
                "Five layers of 32-bit selectors and one AND for the fill bit.",
                "fill = a[31] AND arith; in each layer add 2^k fill bits at the top.",
            ],
        ),
        "Alu" => (
            &[
                "Bütün işlemleri aynı anda hesapla, sonra op ile birini seç.",
                "Toplayıcı, çıkarıcı, kaydırıcılar, karşılaştırıcı, XOR/OR/AND (Tekrar 32) ve bir seçici ağacı.",
                "op'un alt 3 biti (funct3) 8 sonuçtan birini seçer; op[3] toplama/çıkarma ve srl/sra arasında seçer.",
            ],
            &[
                "Compute every operation at once, then pick one with op.",
                "Adder, subtractor, shifters, comparator, XOR/OR/AND (Repeat 32) and a tree of selectors.",
                "The low 3 bits of op (funct3) pick one of 8 results; op[3] picks between add/sub and srl/sra.",
            ],
        ),
        "RegFile" => (
            &[
                "RAM gibi, ama iki okuma kapısı var ve x0 hep 0.",
                "31 tane 32-bit register, yazma için DEMUX ağacı, iki okuma için iki seçici ağacı, x0 için Sabit 0.",
                "we'yi rd'nin 5 bitiyle DEMUX'lardan geçir; rs1 ve rs2 her biri 32 girişli bir MUX ağacını sürer.",
            ],
            &[
                "Like RAM, but with two read ports, and x0 is always 0.",
                "31 32-bit registers, a DEMUX tree for writing, two selector trees for the two reads, a Constant 0 for x0.",
                "Send we through DEMUXes with rd's 5 bits; rs1 and rs2 each drive a 32-input MUX tree.",
            ],
        ),
        "Decoder" => (
            &[
                "Çoğu alan sadece komutun bitlerini doğru yere çekmek: kapı gerekmez.",
                "Alanlar için tek bir Böl (7,5,3,5,5,7). imm için bitleri yeniden dizen Birleştir'ler ve 32-bit seçiciler.",
                "Her biçimin imm'ini ayrı birleştir (I, S, B, U, J); opcode bitlerinden biçimi bul ve seçicilerle doğru olanı geçir.",
            ],
            &[
                "Most fields just route instruction bits to the right place: no gates needed.",
                "One Split (7,5,3,5,5,7) for the fields. For imm, Joins that reorder bits, and 32-bit selectors.",
                "Join each format's imm separately (I, S, B, U, J); work out the format from opcode bits and pass the right one through selectors.",
            ],
        ),
        "Control" => (
            &[
                "Önce opcode'un hangi komut türü olduğunu bul; her çıkış birkaç türün VEYA'sıdır.",
                "Opcode'u sabitlerle karşılaştıran eşitlik testleri, OR'lar ve alu_op için birkaç seçici.",
                "lui, auipc, jal, jalr, branch, load, store, op-imm, op için 9 sinyal çıkar; örneğin reg_we = lui VEYA auipc VEYA jal VEYA jalr VEYA load VEYA op-imm VEYA op.",
            ],
            &[
                "First find which kind of instruction the opcode is; each output is an OR of a few kinds.",
                "Equality tests comparing opcode with constants, ORs, and a few selectors for alu_op.",
                "Make 9 signals for lui, auipc, jal, jalr, branch, load, store, op-imm, op; for example reg_we = lui OR auipc OR jal OR jalr OR load OR op-imm OR op.",
            ],
        ),
        "BranchUnit" => (
            &[
                "funct3'ün bitlerine bak: biri karşılaştırmanın türünü seçer, en alttaki sonucu ters çevirir.",
                "İki MUX ve bir XOR.",
                "l = MUX(lt, ltu, f3[1]); c = MUX(eq, l, f3[2]); take = c XOR f3[0].",
            ],
            &[
                "Look at funct3's bits: some choose the kind of comparison, the lowest one inverts the result.",
                "Two MUXes and one XOR.",
                "l = MUX(lt, ltu, f3[1]); c = MUX(eq, l, f3[2]); take = c XOR f3[0].",
            ],
        ),
        "LoadUnit" => (
            &[
                "İstenen baytı ya da yarım kelimeyi en alta kaydır, sonra üst bitleri doldur.",
                "Bir sağa kaydırıcı (off × 8), Böl/Birleştir, işaret için AND'ler, iki 32-bit seçici.",
                "v = kelime >> (off·8); bayt: alt 8 bit + 24 kez işaret biti (lbu'da 0); yarım: alt 16 + 16 kez; funct3 ile seç.",
            ],
            &[
                "Shift the wanted byte or half word down to the bottom, then fill the upper bits.",
                "A right shifter (off × 8), Split/Join, ANDs for the sign, two 32-bit selectors.",
                "v = word >> (off·8); byte: low 8 bits + 24 copies of the sign bit (0 for lbu); half: low 16 + 16 copies; pick with funct3.",
            ],
        ),
        "StoreUnit" => (
            &[
                "Veriyi doğru bayta kaydır ve hangi baytların yazılacağını gösteren bir maske yap.",
                "İki sola kaydırıcı, bir OR, Sabitler ve Böl/Birleştir.",
                "wdata = data << (off·8); maske = (sb: 0001, sh: 0011, sw: 1111) << off.",
            ],
            &[
                "Shift the data to the right byte and make a mask of which bytes to write.",
                "Two left shifters, an OR, Constants and Split/Join.",
                "wdata = data << (off·8); mask = (sb: 0001, sh: 0011, sw: 1111) << off.",
            ],
        ),
        "MulDiv" => (
            &[
                "Çarpma: elle çarpma gibi, b'nin her biti için a'nın kaydırılmış halini topla. Bölme: elle bölme gibi, her adımda çıkar ve sığıyor mu bak.",
                "Toplayıcılar, çıkarıcılar ve 32-bit seçiciler; işaretli işlemler için mutlak değer ve işaret düzeltmesi.",
                "mul için 32 kat toplama (64-bit sonuç: üst yarısı mulh); div/rem için 32 kat \"kalan = kalan·2 + bit; sığarsa çıkar\". Sıfıra bölmede RISC-V'nin kurallarına bak.",
            ],
            &[
                "Multiplying: like on paper, add a shifted copy of a for each bit of b. Dividing: like long division, subtract at each step if it fits.",
                "Adders, subtractors and 32-bit selectors; absolute values and a sign fix for the signed operations.",
                "32 layers of adding for mul (64-bit result: the top half is mulh); 32 layers of \"rem = rem·2 + bit; subtract if it fits\" for div/rem. Check the RISC-V rules for dividing by zero.",
            ],
        ),
        "Cpu" => (
            &[
                "Her vuruşta: pc'deki komutu oku, çöz, register'ları oku, ALU'da hesapla, belleğe ya da register'a yaz, sıradaki pc'yi seç.",
                "Program sayacı, komut çözücü, kontrol birimi, register dosyası, ALU, dallanma, yükleme/saklama birimleri ve çarpma/bölme; aralarında seçiciler.",
                "ALU'nun b'si: imm ya da rs2 (alu_b_imm). Geri yazma: ALU, bellek, pc+4 ya da imm (wb_sel). Sıradaki pc: pc+4, pc+imm (dallanma alındıysa ya da jal) ya da (rs1+imm) (jalr).",
            ],
            &[
                "On each tick: read the instruction at pc, decode it, read registers, compute in the ALU, write to memory or a register, pick the next pc.",
                "Program counter, decoder, control unit, register file, ALU, branch, load/store units and multiply/divide, with selectors between them.",
                "ALU's b: imm or rs2 (alu_b_imm). Write back: ALU, memory, pc+4 or imm (wb_sel). Next pc: pc+4, pc+imm (branch taken or jal), or (rs1+imm) (jalr).",
            ],
        ),
        "Mux4" => (
            &[
                "Dört bit, her biri için bir seçici.",
                "Bir MUX, Tekrar 4.",
                "a, b ve sel'i MUX×4'e; çıkışı out'a.",
            ],
            &[
                "Four bits, one selector for each.",
                "One MUX with Repeat 4.",
                "a, b and sel into MUX×4; its output to out.",
            ],
        ),
        "Adder4" => (
            &[
                "8-bit toplayıcının küçüğü.",
                "Dört tam toplayıcı, Böl ve Birleştir.",
                "Her basamağın carry'si bir sonrakinin c'sine; ilk c girişten, son carry çıkışa.",
            ],
            &[
                "A smaller 8-bit adder.",
                "Four full adders, Split and Join.",
                "Each column's carry goes to the next one's c; the first c comes from the input, the last carry to the output.",
            ],
        ),
        "Register4" => (
            &[
                "Dört tane 1-bit kayıt, hepsi aynı load ve clk ile.",
                "Bir Bit parçası, Tekrar 4.",
                "d → Bit×4'ün d'si; load ve clk ortak; çıkış q.",
            ],
            &[
                "Four one-bit registers, all with the same load and clk.",
                "One Bit part with Repeat 4.",
                "d → the d of Bit×4; load and clk shared; its output to q.",
            ],
        ),
        "Counter12" => (
            &[
                "Program sayacı gibi ama 12 bit ve +1: sıradaki değer aynısı, +1, d ya da 0.",
                "12 bitlik bir kayıt (Bit×12), +1 için bir toplayıcı, üç seçici (MUX×12).",
                "m1 = MUX(q, q+1, inc); m2 = MUX(m1, d, load); m3 = MUX(m2, 0, reset); kaydın d'si m3, load'u Sabit 1. 32-bit toplayıcıyı Birleştir ve Böl ile 12 bite uydurabilirsin.",
            ],
            &[
                "Like the program counter, but 12 bits and +1: the next value is the same, +1, d, or 0.",
                "A 12-bit register (Bit×12), an adder for the +1, three selectors (MUX×12).",
                "m1 = MUX(q, q+1, inc); m2 = MUX(m1, d, load); m3 = MUX(m2, 0, reset); the register's d is m3, its load Constant 1. Fit the 32-bit adder to 12 bits with Join and Split.",
            ],
        ),
        "AddSub8" => (
            &[
                "a - b = a + (DEĞİL b) + 1. sub'a göre b'yi ters çevir ve eldeye 1 ver.",
                "8 bitlik bir XOR (Tekrar 8) ve bir 8-bit toplayıcı.",
                "b ve sub → XOR×8 → toplayıcının b'si; a → a'sı; sub → c'si.",
            ],
            &[
                "a - b = a + (NOT b) + 1. Invert b depending on sub, and give the carry a 1.",
                "An 8-bit XOR (Repeat 8) and an 8-bit adder.",
                "b and sub → XOR×8 → the adder's b; a → its a; sub → its c.",
            ],
        ),
        "MiniDecode"=>(
            &["Üst ve alt yarı sadece farklı tellerdir.","Bir Böl, gruplar 4,4. Gruplar düşük bitten başlar.","Böl'ün ilk çıkışı arg, ikinci çıkışı op."],
            &["The upper and lower halves are simply different wires.","One Split with groups 4,4, starting at the low bit.","The first Split output is arg, the second op."]
        ),
        "MiniZero8"=>(
            &["Sayı sıfırsa hiçbir biti 1 değildir.","Sekiz biti OR ağacıyla birleştir.","Son OR'un çıkışını NOT ile tersle; 7 OR yeter."],
            &["A number is zero if none of its bits is 1.","Combine eight bits with an OR tree.","Invert the final OR output; 7 ORs are enough."]
        ),
        "MiniPc4"=>(
            &["Sıradaki değeri seçmek ile onu saklamak farklı işlerdir.","Bit×4, Adder8 ile +1, Mux×4 ile jump ve reset; hold yazma iznini kapatır.","d=MUX(MUX(pc+1,target,jump),0,reset); load=(DEĞİL hold) VEYA reset. Toplamın yalnızca alt 4 bitini al."],
            &["Choosing a next value and storing it are separate jobs.","Bit×4, Adder8 for +1, Mux×4 for jump/reset; hold disables writes.","d=MUX(MUX(pc+1,target,jump),0,reset); load=(NOT hold) OR reset. Keep only the low 4 sum bits."]
        ),
        "MiniAcc8"=>(
            &["Reset yükleme olmasa bile kaydı sıfırlamalı.","Bit×8 ve önünde MUX×8; yükleme izni için OR.","d=MUX(d,0,reset); Bit.load=load VEYA reset. clk doğrudan Bit'e."],
            &["Reset must clear the register even with load disabled.","Bit×8 with MUX×8 before it; OR for write enable.","d=MUX(d,0,reset); Bit.load=load OR reset. Wire clk to Bit."]
        ),
        "MiniControl" => (
            &[
                "Önce op'un hangi komut olduğunu bul: her komut için \"op şu sayı mı?\" diye soran bir kapı. Sonra çıkışlar bunların VEYA'sı.",
                "Bitlere ayırmak için Böl, her bitin tersi için NOT, karşılaştırmalar için AND'ler, birleştirmek için OR'lar.",
                "op = 5 için: bit0 VE (DEĞİL bit1) VE bit2 VE (DEĞİL bit3). load = LDI VEYA LD VEYA ADD VEYA SUB; jump = JMP VEYA (JZ VE zero).",
            ],
            &[
                "First find which instruction op is: one gate per instruction asking \"is op this number?\". Then the outputs are ORs of those.",
                "Split for the bits, NOT for each bit's opposite, ANDs for the comparisons, ORs to combine.",
                "For op = 5: bit0 AND (NOT bit1) AND bit2 AND (NOT bit3). load = LDI OR LD OR ADD OR SUB; jump = JMP OR (JZ AND zero).",
            ],
        ),
        "MiniCpu" => (
            &[
                "İki kayıt yeter: 8 bitlik acc ve 4 bitlik pc. Her kaydın önünde \"sıradaki değer ne?\" diye seçen MUX'lar olsun.",
                "Bit×8 ve Bit×4, Mini Kontrol, Topla/Çıkar, +1 için 8-bit toplayıcı, MUX×8 ve MUX×4'ler, zero için OR'lar ve NOT.",
                "instr'ı 4,4 olarak böl (alt: arg, üst: op). acc'nin d'si: rdata, Topla/Çıkar(acc, rdata) ya da arg; reset'te 0. pc'nin d'si: pc+1 ya da arg (jump); load'u DEĞİL hold. addr = arg.",
            ],
            &[
                "Two registers are enough: an 8-bit acc and a 4-bit pc. In front of each, MUXes that choose \"what is the next value?\".",
                "Bit×8 and Bit×4, Mini Control, Add/Subtract, an 8-bit adder for +1, MUX×8 and MUX×4s, ORs and a NOT for zero.",
                "Split instr as 4,4 (low: arg, high: op). acc's d: rdata, AddSub(acc, rdata) or arg; 0 on reset. pc's d: pc+1 or arg (jump); its load is NOT hold. addr = arg.",
            ],
        ),
        "AddressDecode2" => (
            &["İki adres biti dört farklı birleşim verir. Her çıkış yalnızca bir birleşimde açılsın.", "Böl (1,1), iki NOT ve AND kapıları kullan; her seçimi en ile sınırla.", "s0=en AND !a1 AND !a0; s1=en AND !a1 AND a0; s2=en AND a1 AND !a0; s3=en AND a1 AND a0."],
            &["Two address bits give four combinations. Each output enables only one combination.", "Use Split (1,1), two NOTs and AND gates; gate every selection with en.", "s0=en AND !a1 AND !a0; s1=en AND !a1 AND a0; s2=en AND a1 AND !a0; s3=en AND a1 AND a0."],
        ),
        "Rom4x8" => (
            &["Adres, dört sabitten birini seçsin. Veriyi saklamak için saat gerekmez.", "8-bit Sabitler: 0x12, 0xA5, 0x3C, 0x80. Üç MUX×8 ve Böl (1,1).", "a0 ile (0x12,0xA5) ve (0x3C,0x80) çiftlerini seç; a1 ile bu iki sonucu seç. Böl'ün ilk çıkışı a0'dır."],
            &["Let the address select one of four constants. No clock is needed.", "8-bit Constants: 0x12, 0xA5, 0x3C, 0x80. Three MUX×8 and Split (1,1).", "Use a0 to select within (0x12,0xA5) and (0x3C,0x80); use a1 to select between those results. Split's first output is a0."],
        ),
        "Ram4x8" => (
            &["ROM'daki sabitlerin yerine yazılabilir kayıtlar koy. Her kayıt kendi yazma iznine sahip olsun.", "Dört Bit×8, bir AddressDecode2 ve ROM'daki gibi üç MUX×8.", "addr,we → çözücü; s0–s3 ilgili Bit×8'in load'una. d ve clk bütün kayıtlara ortak. Okuma MUX ağacı addr ile q'yu seçer; we okuma yolunu kapatmaz."],
            &["Replace ROM constants with writable registers. Give each register its own write enable.", "Four Bit×8, one AddressDecode2 and three MUX×8 as in ROM.", "addr,we → decoder; s0–s3 → each Bit×8 load. Share d and clk across registers. An addr-selected MUX tree drives q; we must not disable reads."],
        ),
        "ProgramRom" => (
            &["Komut da sekiz bitlik bir veridir. Opcode ve argümanı doğru yarılara koy.", "16 Sabit ve dört katmanlı MUX×8 ağacı; sıfır sabitini paylaşabilirsin.", "LDI 3=0x13, ST 0=0x30, ADD 0=0x40, ST 1=0x31, HLT=0xF0. Katmanlar düşük adres bitinden yükseğe seçim yapar; kalan yapraklar 0."],
            &["An instruction is also eight-bit data. Put opcode and argument in the correct halves.", "16 Constants and four MUX×8 layers; share a zero constant if useful.", "LDI 3=0x13, ST 0=0x30, ADD 0=0x40, ST 1=0x31, HLT=0xF0. Layers select from low address bit to high; remaining leaves are zero."],
        ),
        "RomSequencer" => (
            &["Bir kayıt adresi tutar, ROM o adresteki komutu verir. İzin yoksa adres aynı kalmalı.", "Bit×4, ProgramRom, +1 için Adder8, yükleme/reset için MUX×4; HLT için komutun üst dört bitini AND ile birleştir.", "d_reg=MUX(MUX((pc+1)&15,d,load),0,reset); load_reg=reset OR load OR (run AND !halt). clk doğrudan kayda; pc ROM'un addr'ına, ROM çıkışı instr'a."],
            &["A register holds the address; ROM supplies that instruction. Without permission the address must hold.", "Bit×4, ProgramRom, Adder8 for +1, MUX×4 for load/reset; AND the instruction's upper four bits to detect HLT.", "d_reg=MUX(MUX((pc+1)&15,d,load),0,reset); load_reg=reset OR load OR (run AND !halt). Wire clk directly to the register, pc to ROM addr and ROM output to instr."],
        ),
        "FastAdd32" => (
            &["Yolu uzatan, eldenin bitten bite yürümesi. Üst baytlar eldeyi beklemeden toplanabilir mi?", "Dört Adder8: ilki normal. Diğer üç bayt için ikişer Adder8 (elde 0 ve elde 1) ve MUX×8 ile MUX.", "Bayt k için s0 = a_k+b_k+0, s1 = a_k+b_k+1. Önceki bayttan gelen elde MUX'un sel'i: sum_k = MUX(s0, s1, elde), yeni elde = MUX(s0.carry, s1.carry, elde)."],
            &["What makes the path long is the carry walking from bit to bit. Can the upper bytes be added without waiting for it?", "Four Adder8: the first as usual. For the other three bytes two Adder8 each (carry 0 and carry 1), plus MUX×8 and a MUX.", "For byte k, s0 = a_k+b_k+0 and s1 = a_k+b_k+1. The carry from the byte before is the MUX's sel: sum_k = MUX(s0, s1, carry), new carry = MUX(s0.carry, s1.carry, carry)."],
        ),
        "PipeAdd32" => (
            &["İşi iki parçaya böl ve aralarına bir yazmaç koy. Yazmacın girişi ilk aşamanın bitişi, çıkışı ikinci aşamanın başıdır.", "İki Adder8 alt 16 bit için, Birleştir ile 49 bitlik bir bus (16 toplam + 1 elde + 16 + 16), Bit×49 yazmaç, iki Adder8 üst 16 bit için.", "Yazmaca: alt toplam, alt eldesi, a[16..31], b[16..31]; load = 1, clk doğrudan. Yazmaç çıkışını böl; üst yarıları ve eldeyi topla. sum = alt toplam (yazmaçtan) + üst toplam."],
            &["Split the work in two and put a register between the halves. Its input is the end of the first stage, its output the start of the second.", "Two Adder8 for the low 16 bits, a Join into a 49-bit bus (16 sum + 1 carry + 16 + 16), a Bit×49 register, two Adder8 for the high 16 bits.", "Into the register: low sum, low carry, a[16..31], b[16..31]; load = 1, clk straight in. Split its output; add the high halves with the carry. sum = low sum (from the register) + high sum."],
        ),
        "Forward" => (
            &["Her kaynak için iki soru: EX'teki komut bu register'ı mı yazıyor? Değilse MEM'deki mi?", "5 bitlik eşitlik için Xor×5 ve bir OR ağacı, sıfır olmama için OR ağacı; birkaç AND ve NOT.", "ex_hit = ex_we AND ex_rd≠0 AND ex_rd=rs; mem_hit = aynısı MEM için. f bit 0 = ex_hit, bit 1 = mem_hit AND NOT ex_hit. Birleştir (1,1)."],
            &["Two questions for each source: does the instruction in EX write this register? If not, does the one in MEM?", "Xor×5 and an OR tree for 5-bit equality, an OR tree for not-zero; a few AND and NOT.", "ex_hit = ex_we AND ex_rd≠0 AND ex_rd=rs; mem_hit = the same for MEM. f bit 0 = ex_hit, bit 1 = mem_hit AND NOT ex_hit. Join (1,1)."],
        ),
        "Cache4" => (
            &["Her satır bir yazmaçtır: geçerli biti, etiketi ve veriyi birlikte saklar. Okumak, RAM'deki gibi seçmektir; sonra etiketi karşılaştırırsın.", "Dört Bit×39 yazmaç (1 + 6 + 32), satır seçimi için NOT ve AND, okumak için üç MUX×39, etiket için Xor×6 ve bir OR ağacı.", "Satırın girişi: Birleştir(1, addr[2..7], d). load = fill AND (satır seçili). Seçilen satırı böl; hit = geçerli AND (etiket = addr[2..7]); q = verisi."],
            &["Each line is a register holding the valid bit, the tag and the data together. Reading is selecting, as in RAM; then you compare the tag.", "Four Bit×39 registers (1 + 6 + 32), NOT and AND for picking the line, three MUX×39 for reading, Xor×6 and an OR tree for the tag.", "A line's input: Join(1, addr[2..7], d). load = fill AND (line selected). Split the selected line; hit = valid AND (tag = addr[2..7]); q = its data."],
        ),
        _ => (&[], &[]),
    };
    pick(tr, en).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels;

    #[test]
    fn every_level_has_three_hints_in_both_languages() {
        for l in levels() {
            assert_eq!(hints(l.chip).len(), 3, "{}", l.chip);
            makine_lang::set_for_thread(Some(makine_lang::Lang::En));
            assert_eq!(hints(l.chip).len(), 3, "{}", l.chip);
            makine_lang::set_for_thread(None);
        }
    }
}
