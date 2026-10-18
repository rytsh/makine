//! Executable lessons. Solutions are tested by behaviour, never by matching
//! source text. Each case gets a fresh, deterministic machine and a budget.

use makine_lang::{tr, trf};
use makine_rv::{
    asm,
    emu::{Cpu, Trap},
    machine::Machine,
};

pub struct Lesson {
    pub id: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    pub predict: &'static str,
    pub answer: &'static str,
    pub change: &'static str,
    pub task: &'static str,
    pub example: &'static str,
    pub starter: &'static str,
    pub solution: &'static str,
    pub hint: &'static str,
}

pub fn all() -> Vec<Lesson> {
    vec![
        Lesson {
            id: "sum", title: tr!("1. İlk komutlar", "1. First instructions"),
            about: tr!("Register işlemcinin küçük çalışma kutusudur. li sayı koyar; add iki kutudaki sayıyı toplar. x0 her zaman sıfırdır. ebreak programı durdurur.", "A register is a small working box inside the CPU. li puts a number in one; add adds two registers. x0 is always zero. ebreak stops the program."),
            predict: tr!("Sonunda a0 kaç olur?", "What will a0 be at the end?"), answer: "5",
            change: tr!("2 ve 3'ü değiştir, her komutta register'ları izle.", "Change 2 and 3, watch the registers at each instruction."),
            task: tr!("Başlangıçtaki a0 ile a1'i topla; sonucu a0'da bırak. Sabit sayı yazma: farklı girişlerle denenecek.", "Add the starting a0 and a1; leave the result in a0. Do not hard-code numbers: different inputs will be tested."),
            example: "li a0, 2\nli a1, 3\nadd a0, a0, a1\nebreak\n",
            starter: "# a0 + a1 -> a0\n\nebreak\n", solution: "add a0, a0, a1\nebreak\n",
            hint: tr!("add hedef, kaynak1, kaynak2. Hedef, kaynaklardan biri olabilir.", "add destination, source1, source2. The destination can also be a source."),
        },
        Lesson {
            id: "bits", title: tr!("2. Bitlerle çalışma", "2. Working with bits"),
            about: tr!("AND bir maske uygular. Sayının en alt biti tek/çift bilgisidir. Kaydırmalar bitleri taşır; XOR farklı bitleri bulur.", "AND applies a mask. The lowest bit tells whether a number is odd or even. Shifts move bits; XOR finds differing bits."),
            predict: "13 AND 1 = ?", answer: "1",
            change: tr!("Maskeyi 3 yap. Hangi bitler kalıyor?", "Change the mask to 3. Which bits remain?"),
            task: tr!("a0 tekse a0=1, çiftse a0=0 yap.", "Set a0 to 1 if its starting value is odd, or 0 if even."),
            example: "li a0, 13\nandi a0, a0, 1\nebreak\n", starter: "# odd(a0) -> a0\n\nebreak\n",
            solution: "andi a0, a0, 1\nebreak\n", hint: tr!("En alt bit için maske 1.", "The mask for the lowest bit is 1."),
        },
        Lesson {
            id: "max", title: tr!("3. Karar verme", "3. Making decisions"),
            about: tr!("Dallanma koşul doğruysa başka bir etikete gider. blt işaretli, bltu işaretsiz karşılaştırır. Aynı bitler iki farklı sayı olarak yorumlanabilir.", "A branch goes to a label if its condition is true. blt compares signed numbers, bltu unsigned ones. The same bits can mean two different numbers."),
            predict: tr!("-3 ile 2'den hangisi büyük?", "Which is greater, -3 or 2?"), answer: "2",
            change: tr!("blt'yi bltu yap; negatif sayıyla sonucu karşılaştır.", "Change blt to bltu; compare the result for a negative input."),
            task: tr!("a0 ve a1'in işaretli olarak büyüğünü a0'da bırak.", "Leave the signed maximum of a0 and a1 in a0."),
            example: "li a0, -3\nli a1, 2\nblt a1, a0, done\nmv a0, a1\ndone: ebreak\n",
            starter: "# max_signed(a0, a1) -> a0\n\nebreak\n",
            solution: "bge a0, a1, done\nmv a0, a1\ndone: ebreak\n",
            hint: tr!("a0 zaten büyükse kopyalamayı atla. Negatif girişlerde işaretsiz dallanma kullanma.", "If a0 is already larger, skip the copy. Do not use an unsigned branch for negative inputs."),
        },
        Lesson {
            id: "loop", title: tr!("4. Döngüler", "4. Loops"),
            about: tr!("Etiket adresin adıdır. Döngü, aynı komutlara geri dallanır. Her tur koşulu değiştirmeyi unutursan sonsuza dek dönebilir.", "A label names an address. A loop branches back to the same instructions. Unless you change its condition each round, it may run forever."),
            predict: tr!("1+2+3+4+5 kaç?", "What is 1+2+3+4+5?"), answer: "15",
            change: tr!("Başlangıç sayacını 10 yap. Sonuç 55 mi?", "Start the counter at 10. Is the result 55?"),
            task: tr!("Başlangıçtaki n=a0 için 1'den n'e kadar topla; a0'a yaz. n=0 da denenir.", "For starting n=a0, sum 1 through n and return it in a0. n=0 is tested too."),
            example: "li a0, 0\nli t0, 5\nloop: add a0, a0, t0\naddi t0, t0, -1\nbnez t0, loop\nebreak\n",
            starter: "# sum(1..a0) -> a0\n\nebreak\n",
            solution: "mv t0, a0\nli a0, 0\nbeqz t0, done\nloop: add a0, a0, t0\naddi t0, t0, -1\nbnez t0, loop\ndone: ebreak\n",
            hint: tr!("n'i t0'da tut; toplamı sıfırla. Sıfır girişini döngüden önce kontrol et.", "Keep n in t0; clear the sum. Check for zero before the loop."),
        },
        Lesson {
            id: "memory", title: tr!("5. Bellek ve diziler", "5. Memory and arrays"),
            about: tr!("Register sayı veya adres tutabilir. lw adresteki 4 baytı okur; sw yazar. Sonraki kelime için adresi 4 artır. Bellek küçük uçludur: düşük bayt önce gelir.", "A register may hold a value or an address. lw reads 4 bytes at that address; sw writes them. Add 4 to reach the next word. Memory is little-endian: the low byte comes first."),
            predict: tr!("Üç kelime toplam kaç bayt?", "How many bytes are three words?"), answer: "12",
            change: tr!(".word değerlerini değiştir; bellek ve register'daki farkı izle.", "Change the .word values; inspect memory and register changes."),
            task: tr!("a0 dizinin adresi, a1 uzunluğu. İşaretli 32-bit kelimeleri topla; a0'a yaz. Boş dizi de denenir.", "a0 is the array address, a1 its length. Sum its signed 32-bit words; return in a0. Empty arrays are tested."),
            example: "la t0, data\nlw a0, 0(t0)\nlw t1, 4(t0)\nadd a0, a0, t1\nebreak\n.align 2\ndata: .word 7, 8, 9\n",
            starter: "# sum_words(a0=address, a1=count) -> a0\n\nebreak\n",
            solution: "mv t0, a0\nli a0, 0\nbeqz a1, done\nloop: lw t1, 0(t0)\nadd a0, a0, t1\naddi t0, t0, 4\naddi a1, a1, -1\nbnez a1, loop\ndone: ebreak\n",
            hint: tr!("Adres ve toplam farklı register'larda olsun. Her kelime 4 bayt.", "Keep the address and sum in different registers. Each word takes 4 bytes."),
        },
        Lesson {
            id: "string", title: tr!("6. Metin ve baytlar", "6. Text and bytes"),
            about: tr!("Metin baytlardan oluşur. .asciz sona sıfır ekler. lbu bir baytı sıfırla genişleterek okur; lw dört bayt okur, aynı şey değildir.", "Text is made of bytes. .asciz adds a terminating zero. lbu reads one byte and zero-extends it; lw reads four bytes, which is different."),
            predict: tr!("ASCII metin 'makine' kaç bayt (son sıfır hariç)?", "How many bytes in ASCII 'makine', excluding the final zero?"), answer: "6",
            change: tr!("Metni değiştir, sonundaki 00 baytını bellekte bul.", "Change the text; find its final 00 byte in memory."),
            task: tr!("a0'daki adresin gösterdiği sıfırla biten metnin bayt uzunluğunu a0'da döndür.", "Return the byte length of the zero-terminated string pointed to by a0."),
            example: "la t0, text\nlbu a0, 0(t0)\nebreak\ntext: .asciz \"makine\"\n",
            starter: "# strlen(a0=address) -> a0\n\nebreak\n",
            solution: "mv t0, a0\nli a0, 0\nloop: lbu t1, 0(t0)\nbeqz t1, done\naddi a0, a0, 1\naddi t0, t0, 1\nj loop\ndone: ebreak\n",
            hint: tr!("Sıfırı sayma. Karakter değil bayt saydığını unutma (UTF-8 birden fazla bayt olabilir).", "Do not count the zero. You count bytes, not characters (UTF-8 may use several bytes)."),
        },
        Lesson {
            id: "function", title: tr!("7. Fonksiyon ve yığın", "7. Functions and stack"),
            about: tr!("call dönüş adresini ra'ya yazar; ret oraya döner. İç içe çağrıda eski ra'yı yığında sakla. sp geçerli, hizalı RAM adresi olmalı; burada örnek onu başlatır.", "call writes a return address to ra; ret returns there. Save the old ra on the stack for nested calls. sp must be a valid aligned RAM address; this example initializes it."),
            predict: tr!("double(7) kaç?", "What is double(7)?"), answer: "14",
            change: tr!("Fonksiyonu iki kez çağır, arada ra'yı izle.", "Call the function twice and watch ra."),
            task: tr!("a0'ı iki katına çıkaran double fonksiyonu yaz. Giriş kodu call double ile çağırmalı ve ebreak'e dönmeli.", "Write a double function. Your entry code must call double and return to ebreak with twice the starting a0."),
            example: "li sp, 0x10000\nli a0, 7\ncall double\nebreak\ndouble: add a0, a0, a0\nret\n",
            starter: "call double\nebreak\ndouble:\n# a0 *= 2\nret\n",
            solution: "call double\nebreak\ndouble: add a0, a0, a0\nret\n",
            hint: tr!("add a0,a0,a0 sayıyı iki katına çıkarır. ret, ra'daki adrese döner.", "add a0,a0,a0 doubles the number. ret returns to the address in ra."),
        },
        Lesson {
            id: "console", title: tr!("8. Konsol", "8. Console"),
            about: tr!("G/Ç de adreslerle yapılır. 0x10000000'a sb bir karakter basar. O adresten okuma sıradaki tuşu alır; tuş yoksa -1 gelir. Bu, makine'nin aygıt haritasıdır; Linux sistem çağrısı değildir.", "I/O uses addresses too. sb to 0x10000000 prints a character. Reading takes the next key, or returns -1 when empty. This is makine's device map, not Linux system calls."),
            predict: tr!("65 hangi ASCII harfi?", "Which ASCII letter is 65?"), answer: "A",
            change: tr!("65 yerine 66 yaz; sonra iki karakter bas.", "Use 66 instead of 65; then print two characters."),
            task: tr!("a0'daki ASCII karakterini konsola bir kez bas; dur.", "Print the ASCII character in a0 once, then stop."),
            example: "li t0, 0x10000000\nli a0, 65\nsb a0, 0(t0)\nebreak\n",
            starter: "# print(a0)\n\nebreak\n",
            solution: "li t0, 0x10000000\nsb a0, 0(t0)\nebreak\n",
            hint: tr!("Adresi li ile t0'a koy; sb a0,0(t0).", "Put the address in t0 with li; sb a0,0(t0)."),
        },
        Lesson {
            id: "pixel", title: tr!("9. Ekran ve piksel", "9. Screen and pixel"),
            about: tr!("Ekran 320×200. Piksel adresi 0x11000000 + 4×(y×320+x). Renk 0x00RRGGBB. Çizdikten sonra 0x1000000C'ye yazarak ekranı göster.", "The screen is 320×200. A pixel's address is 0x11000000 + 4×(y×320+x). Color is 0x00RRGGBB. Write to 0x1000000C after drawing to present the screen."),
            predict: tr!("İkinci piksel ilkinden kaç bayt ileride?", "How many bytes after the first is the second pixel?"), answer: "4",
            change: tr!("Kırmızıyı yeşile çevir; adresi 4 artırıp yan pikseli boya.", "Change red to green; add 4 to the address to paint the next pixel."),
            task: tr!("a0'daki renkle ekranın ilk pikselini boya ve ekranı göster.", "Paint the first pixel with the color in a0 and present the screen."),
            example: "li t0, 0x11000000\nli a0, 0xff0000\nsw a0, 0(t0)\nli t1, 0x1000000c\nsw zero, 0(t1)\nebreak\n",
            starter: "# pixel(0,0) = a0; present\n\nebreak\n",
            solution: "li t0, 0x11000000\nsw a0, 0(t0)\nli t1, 0x1000000c\nsw zero, 0(t1)\nebreak\n",
            hint: tr!("Piksel ve ekranı göster adresleri farklı. İkisine de yazmalısın.", "The pixel and present addresses differ. Write to both."),
        },
        Lesson {
            id: "project", title: tr!("10. Küçük proje: bellek kopyalama", "10. Small project: memory copy"),
            about: tr!("Döngü, adres ve bayt bilgisini birleştir. Programın tek bir örnekte çalışması yetmez: boş, kısa ve farklı verilerde dene.", "Combine loops, addresses and bytes. One working example is not enough: try empty, short and different data."),
            predict: tr!("0 bayt kopyalamak belleği değiştirir mi? (hayır/no)", "Should copying 0 bytes change memory? (no)"), answer: tr!("hayır", "no"),
            change: tr!("lbu/sb yerine lw/sw kullansan neyi değiştirmek gerekir?", "What must change if you use lw/sw instead of lbu/sb?"),
            task: tr!("a0 kaynak, a1 hedef, a2 bayt sayısı. Çakışmayan bölgeler arasında tam a2 bayt kopyala; çevredeki veriyi bozma.", "a0 is source, a1 destination, a2 byte count. Copy exactly a2 bytes between non-overlapping regions; do not corrupt surrounding data."),
            example: "la t0, src\nla t1, dst\nlbu t2, 0(t0)\nsb t2, 0(t1)\nebreak\nsrc: .asciz \"hi\"\ndst: .space 4\n",
            starter: "# copy(a0=source, a1=dest, a2=count)\n\nebreak\n",
            solution: "beqz a2, done\nloop: lbu t0, 0(a0)\nsb t0, 0(a1)\naddi a0, a0, 1\naddi a1, a1, 1\naddi a2, a2, -1\nbnez a2, loop\ndone: ebreak\n",
            hint: tr!("Her tur bir bayt oku/yaz, iki adresi de bir artır, sayacı azalt. Önce sıfırı kontrol et.", "Each round read/write one byte, advance both addresses, decrease the count. Check zero first."),
        },
    ]
}

pub fn projects() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            tr!("İç içe fonksiyonlar ve yığın", "Nested calls and stack"),
            include_str!("../programs/nested.s"),
        ),
        (
            tr!("Fibonacci dizisi", "Fibonacci sequence"),
            include_str!("../programs/fibonacci.s"),
        ),
        (
            tr!("Dizi sıralama", "Array sorting"),
            include_str!("../programs/sort.s"),
        ),
        (
            tr!("Konsolda yankı", "Console echo"),
            include_str!("../programs/echo.s"),
        ),
        (
            tr!("Metin ve fonksiyon çağrısı", "Text and function calls"),
            include_str!("../programs/hello.s"),
        ),
        (
            tr!("Ekranda renkli desen", "Screen color pattern"),
            include_str!("../programs/colors.s"),
        ),
        (
            tr!("Klavyeyle hareket eden kare", "Keyboard-controlled square"),
            include_str!("../programs/keys.s"),
        ),
    ]
}

#[derive(Clone, Debug)]
pub struct Case {
    pub registers: Vec<(usize, u32)>,
    pub data: Vec<(u32, Vec<u8>)>,
    pub expect: Vec<(usize, u32)>,
    pub memory: Vec<(u32, Vec<u8>)>,
    pub console: Option<String>,
    pub pixel: Option<u32>,
}

fn case(regs: &[(usize, u32)], result: u32) -> Case {
    Case {
        registers: regs.to_vec(),
        data: vec![],
        expect: vec![(10, result)],
        memory: vec![],
        console: None,
        pixel: None,
    }
}

pub fn cases(id: &str) -> Vec<Case> {
    match id {
        "sum" => [(2, 3), (0, 0), (90, 17), (u32::MAX, 2)]
            .into_iter()
            .map(|(a, b)| case(&[(10, a), (11, b)], a.wrapping_add(b)))
            .collect(),
        "bits" => [0, 1, 2, 13, 255, u32::MAX]
            .into_iter()
            .map(|a| case(&[(10, a)], a & 1))
            .collect(),
        "max" => [
            (-3i32, 2i32),
            (5, 1),
            (-7, -2),
            (0, 0),
            (i32::MIN, i32::MAX),
        ]
        .into_iter()
        .map(|(a, b)| case(&[(10, a as u32), (11, b as u32)], a.max(b) as u32))
        .collect(),
        "loop" => [0, 1, 5, 10, 30]
            .into_iter()
            .map(|n| case(&[(10, n)], n * (n + 1) / 2))
            .collect(),
        "memory" => [vec![], vec![7u32], vec![1, 2, 3], vec![u32::MAX, 4, 8]]
            .into_iter()
            .map(|v| {
                let mut c = case(
                    &[(10, 0x400), (11, v.len() as u32)],
                    v.iter().fold(0u32, |s, x| s.wrapping_add(*x)),
                );
                c.data
                    .push((0x400, v.iter().flat_map(|v| v.to_le_bytes()).collect()));
                c
            })
            .collect(),
        "string" => ["", "a", "makine", "Türkçe"]
            .into_iter()
            .map(|s| {
                let mut c = case(&[(10, 0x400)], s.len() as u32);
                c.data.push((0x400, [s.as_bytes(), &[0]].concat()));
                c
            })
            .collect(),
        "function" => [0u32, 7, 20, 123]
            .into_iter()
            .map(|v| case(&[(10, v), (2, 0x10000)], v * 2))
            .collect(),
        "console" => [65u32, 90, 48]
            .into_iter()
            .map(|v| {
                let mut c = case(&[(10, v)], v);
                c.expect.clear();
                c.console = Some((v as u8 as char).to_string());
                c
            })
            .collect(),
        "pixel" => [0xff0000, 0x00ff00, 0x123456]
            .into_iter()
            .map(|v| {
                let mut c = case(&[(10, v)], v);
                c.expect.clear();
                c.pixel = Some(v);
                c
            })
            .collect(),
        "project" => [b"".as_slice(), b"a", b"makine", &[0, 255, 4, 8]]
            .into_iter()
            .map(|v| {
                let mut c = case(&[(10, 0x400), (11, 0x500), (12, v.len() as u32)], 0);
                c.expect.clear();
                c.data = vec![(0x400, v.to_vec()), (0x4ff, vec![0xaa; v.len() + 2])];
                c.memory = vec![(0x4ff, [&[0xaa], v, &[0xaa]].concat())];
                c
            })
            .collect(),
        _ => vec![],
    }
}

pub fn initialize(p: &asm::Program, c: &Case) -> (Cpu, Machine) {
    let mut cpu = Cpu::new();
    let mut m = Machine::new(crate::computer::RAM_BYTES);
    m.fixed_time = Some(0);
    m.load(0, &p.bytes);
    for &(r, v) in &c.registers {
        cpu.x[r] = v;
    }
    for (addr, data) in &c.data {
        m.load(*addr, data);
    }
    (cpu, m)
}

pub struct Report {
    pub passed: usize,
    pub total: usize,
    pub failure: Option<(Case, String)>,
}

pub fn check(id: &str, p: &asm::Program) -> Report {
    let cases = cases(id);
    let mut report = Report {
        passed: 0,
        total: cases.len(),
        failure: None,
    };
    for c in cases {
        let (mut cpu, mut m) = initialize(p, &c);
        let mut stopped = false;
        let mut error = None;
        for _ in 0..20_000 {
            match cpu.step(&mut m) {
                Ok(()) => {}
                Err(Trap::Breakpoint) => {
                    stopped = true;
                    break;
                }
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        let bad = error
            .or_else(|| {
                (!stopped).then(|| {
                    tr!(
                        "Komut sınırı: döngünün koşulunu kontrol et.",
                        "Instruction limit: check your loop condition."
                    )
                    .into()
                })
            })
            .or_else(|| {
                c.expect.iter().find_map(|&(r, v)| {
                    (cpu.x[r] != v).then(|| {
                        trf!(
                            "x{r}: beklenen {v}, senin {} (işaretli: {}).",
                            "x{r}: expected {v}, yours {} (signed: {}).",
                            cpu.x[r],
                            cpu.x[r] as i32
                        )
                    })
                })
            })
            .or_else(|| {
                c.memory.iter().find_map(|(a, bytes)| {
                    bytes.iter().enumerate().find_map(|(k, &b)| {
                        let addr = *a + k as u32;
                        let got = (m.peek_ram(addr) >> ((addr & 3) * 8)) as u8;
                        (got != b).then(|| {
                            trf!(
                                "Bellek 0x{addr:x}: beklenen {b}, senin {got}.",
                                "Memory 0x{addr:x}: expected {b}, yours {got}."
                            )
                        })
                    })
                })
            })
            .or_else(|| {
                c.console.as_ref().and_then(|s| {
                    (m.console_text() != *s).then(|| {
                        tr!(
                            "Konsol çıktısı beklenen karakter değil.",
                            "Console output is not the expected character."
                        )
                        .into()
                    })
                })
            })
            .or_else(|| {
                c.pixel.and_then(|v| {
                    (m.framebuffer[0] != v || m.frames == 0).then(|| {
                        tr!(
                            "İlk pikseli boya ve ekranı göster adresine yaz.",
                            "Paint the first pixel and write to the present address."
                        )
                        .into()
                    })
                })
            });
        if let Some(message) = bad {
            report.failure = Some((c, message));
            break;
        }
        report.passed += 1;
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_lessons_have_executable_examples_and_passing_solutions() {
        for l in all() {
            asm::assemble(l.example).unwrap_or_else(|e| panic!("{}: {e}", l.id));
            let p = asm::assemble(l.solution).unwrap();
            let r = check(l.id, &p);
            assert!(r.total >= 3, "{}", l.id);
            assert!(r.failure.is_none(), "{}: {:?}", l.id, r.failure);
        }
    }
    #[test]
    fn loops_and_wrong_answers_fail_without_hanging() {
        let p = asm::assemble("loop: j loop").unwrap();
        assert!(check("sum", &p).failure.unwrap().1.contains("sınırı"));
        let p = asm::assemble("ebreak").unwrap();
        assert!(check("sum", &p).failure.is_some());
    }

    #[test]
    fn larger_projects_sort_fibonacci_echo_and_nested_calls() {
        let run = |src: &str, input: &[u8]| {
            let p = asm::assemble(src).unwrap();
            let mut m = Machine::new(2 * 1024 * 1024);
            m.load(0, &p.bytes);
            m.console_in.extend(input);
            m.fixed_time = Some(0);
            let mut c = Cpu::new();
            let mut done = false;
            for _ in 0..100_000 {
                match c.step(&mut m) {
                    Ok(()) => {}
                    Err(Trap::Breakpoint) => {
                        done = true;
                        break;
                    }
                    Err(e) => panic!("{e}"),
                }
            }
            assert!(done);
            (p, c, m)
        };
        let (p, _, m) = run(include_str!("../programs/sort.s"), &[]);
        let a = p.symbols["values"];
        let vals: Vec<i32> = (0..6).map(|k| m.peek_ram(a + k * 4) as i32).collect();
        assert_eq!(vals, vec![-3, -1, 0, 2, 7, 9]);
        let (p, _, m) = run(include_str!("../programs/fibonacci.s"), &[]);
        let a = p.symbols["results"];
        assert_eq!(m.peek_ram(a + 11 * 4), 89);
        let (_, c, _) = run(include_str!("../programs/nested.s"), &[]);
        assert_eq!(c.x[10], 11);
        assert_eq!(c.x[2], 0x10000);
        let (_, _, m) = run(include_str!("../programs/echo.s"), b"hi\n");
        assert_eq!(m.console_text(), "hi\n");
        for (_, s) in projects() {
            asm::assemble(s).unwrap();
        }
    }
}
