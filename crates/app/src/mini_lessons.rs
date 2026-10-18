//! Small, executable code tasks attached to the memory/ROM circuit path.
//! Tests run the assembled bytes on a gate-built MiniCpu, not source matching.
use makine_lang::{tr, trf};
use makine_levels::mini::MiniDriver;
use makine_rv::smallasm::{assemble, Dialect};
use makine_sim::Circuit;

pub struct Lesson {
    pub id: &'static str,
    pub chip: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    pub task: &'static str,
    pub starter: &'static str,
    pub solution: &'static str,
    pub hint: &'static str,
}

pub fn all() -> Vec<Lesson> {
    vec![
        Lesson {
            id: "mini-rom", chip: "ProgramRom",
            title: tr!("Kod yaz: ROM'daki programı değiştir", "Write code: change the ROM program"),
            about: tr!("LDI sabit yükler, ST RAM'e yazar, ADD ise argümanındaki adresten okur. Kaynak satırları ile Derle'nin gösterdiği baytları karşılaştır. Bu kod alıştırmasının ROM'u, devre seviyesinin sabit test tablosundan ayrıdır.", "LDI loads a constant, ST writes RAM, and ADD reads from its operand address. Compare source lines with the bytes shown by Build. This code exercise has its own ROM, separate from the circuit level's fixed test table."),
            task: tr!("Örnekteki 3'ü 5 yap. RAM[0]'a 5, sonra RAM[1]'e 10 yazıp HLT ile dur. Başka hücreye yazma.", "Change the example's 3 to 5. Write 5 to RAM[0], then 10 to RAM[1], and stop with HLT. Do not write any other cell."),
            starter: "LDI 3\nST 0\nADD 0\nST 1\nHLT\n",
            solution: "LDI 5\nST 0\nADD 0\nST 1\nHLT\n",
            hint: tr!("Yalnızca LDI'nin argümanını değiştir. İlk bayt 0x13 değil 0x15 olur. ADD 0, RAM[0]'daki 5'i ekler.", "Change only the LDI operand. The first byte becomes 0x15 instead of 0x13. ADD 0 adds the 5 in RAM[0]."),
        },
        Lesson {
            id: "mini-memory", chip: "ProgramRom",
            title: tr!("Kod yaz: RAM'den oku, iki katını yaz", "Write code: read RAM and double"),
            about: tr!("Sabit bir sonuç yazmak her girişte çalışmaz. LD 0, RAM[0]'ı acc'ye alır; ADD 0 aynı hücreyi bir kez daha ekler. acc sekiz bittir: 128+128=0, 255+255=254. Test her denemede başlangıç RAM'ini yeniden kurar; .ram başlangıçları test girdilerinin yerine geçmez.", "A constant result does not work for every input. LD 0 reads RAM[0] into acc; ADD 0 adds that cell again. acc is eight bits: 128+128=0, 255+255=254. Each test replaces initial RAM with its fixture; .ram initializers cannot override test inputs."),
            task: tr!("RAM[0]'daki baytın iki katını mod 256 hesaplayıp RAM[1]'e bir kez yaz. RAM[0] ve diğer hücreleri koru; HLT ile dur. 0, 1, 7, 127, 128 ve 255 denenir.", "Double the byte in RAM[0] modulo 256 and write RAM[1] once. Preserve RAM[0] and all other cells; stop with HLT. Inputs include 0, 1, 7, 127, 128 and 255."),
            starter: "# RAM[1] = 2 * RAM[0] (mod 256)\n\nHLT\n",
            solution: "LD 0\nADD 0\nST 1\nHLT\n",
            hint: tr!("LD 0; ADD 0; ST 1; HLT. Elle adımlarken .ram 0,7 ile örnek girdi koyabilirsin; görev testi farklı girdileri kendisi sağlar.", "LD 0; ADD 0; ST 1; HLT. For manual stepping, .ram 0,7 supplies an example input; task checks provide their own different inputs."),
        },
        Lesson {
            id: "mini-loop", chip: "RomSequencer",
            title: tr!("Kod yaz: etiketli geri sayım", "Write code: a labelled countdown"),
            about: tr!("Komut getirme devresi yalnızca adres ilerletiyordu. Mini CPU'da JMP adresi değiştirir; JZ acc sıfırsa atlar. Etiket ROM'da yer kaplamaz. Test RAM[0]'a n ve RAM[15]'e 1 koyar. Elle adımlamak için .ram 0,5 ve .ram 15,1 ekleyebilirsin.", "The fetch circuit only advanced addresses. On Mini CPU, JMP changes the address and JZ branches when acc is zero. A label occupies no ROM word. Tests put n in RAM[0] and 1 in RAM[15]. For manual stepping add .ram 0,5 and .ram 15,1."),
            task: tr!("RAM[0]'dan n'i oku. RAM[1]'e sırayla n,n-1,...,0 yaz, sonra HLT. n=0 için yalnızca bir kez 0 yaz. Diğer hücrelere yazma; 0,1,5,15,255 denenir.", "Read n from RAM[0]. Write n,n-1,...,0 to RAM[1], then HLT. For n=0 write zero exactly once. Do not write other cells; tests include 0,1,5,15,255."),
            starter: "LD 0\nloop:\n# ST, JZ, SUB, JMP\n\nHLT\n",
            solution: "LD 0\nloop: ST 1\nJZ done\nSUB 15\nJMP loop\ndone: HLT\n",
            hint: tr!("Önce mevcut sayıyı yaz, sonra sıfır mı diye bak. Sıfırdan önce çıkarırsan 0, 255'e sarar. Son yazma izinin gerçekten n'den 0'a indiği kontrol edilir.", "Write the current value before checking zero. Subtracting first wraps zero to 255. Tests check the actual write trace counts from n down to zero."),
        },
    ]
}

pub fn find(id: &str) -> Option<Lesson> {
    all().into_iter().find(|l| l.id == id)
}

pub fn unlocked(id: &str, solved: &std::collections::BTreeSet<String>) -> bool {
    let lessons = all();
    lessons
        .iter()
        .position(|l| l.id == id)
        .is_some_and(|i| solved.contains(id) || lessons[..i].iter().all(|l| solved.contains(l.id)))
}

/// Uses fresh RAM and a bounded instruction budget for every case. Comparing
/// writes as well as final values rejects hard-coded answers and stray writes.
pub fn check(id: &str, source: &str) -> Result<String, String> {
    let lesson = find(id).ok_or_else(|| tr!("Görev bulunamadı.", "Task not found.").to_string())?;
    let program = assemble(source, Dialect::Mini)
        .map_err(|e| trf!("Satır {}: {}", "Line {}: {}", e.line, e.message))?;
    let mut rom = [0u8; 16];
    for (to, &v) in rom.iter_mut().zip(&program.words) {
        *to = v as u8;
    }
    let inputs: &[u8] = match lesson.id {
        "mini-rom" => &[0, 173],
        "mini-memory" => &[0, 1, 7, 127, 128, 255],
        _ => &[0, 1, 5, 15, 255],
    };
    for &n in inputs {
        let mut ram = [0xA5u8; 16];
        ram[0] = n;
        ram[15] = 1;
        let mut expected = ram;
        let writes: Vec<(u8, u8)> = match lesson.id {
            "mini-rom" => vec![(0, 5), (1, 10)],
            "mini-memory" => vec![(1, n.wrapping_mul(2))],
            _ => (0..=n).rev().map(|v| (1, v)).collect(),
        };
        for &(addr, value) in &writes {
            expected[addr as usize] = value;
        }
        let c = Circuit::compile(makine_levels::cost::best_library(), "MiniCpu")
            .map_err(|e| e.to_string())?;
        let mut cpu = MiniDriver::new(c);
        cpu.reset()?;
        let mut got = Vec::new();
        let mut halted = false;
        for _ in 0..2048 {
            let pc = cpu.pc() as usize;
            if rom[pc] >> 4 == 15 {
                halted = true;
                break;
            }
            if let Some(write) = cpu.step(&rom, &mut ram)?.write {
                got.push(write);
            }
        }
        if !halted {
            return Err(trf!(
                "Girdi {n}: 2048 komutta HLT'ye ulaşılmadı; PC={}.",
                "Input {n}: HLT was not reached in 2048 instructions; PC={}.",
                cpu.pc()
            ));
        }
        if got != writes {
            let i = got
                .iter()
                .zip(&writes)
                .position(|(a, b)| a != b)
                .unwrap_or(got.len().min(writes.len()));
            return Err(trf!("Girdi {n}, {}. RAM yazması: beklenen {:?}, gelen {:?}. Toplam yazma: beklenen {}, gelen {}.", "Input {n}, RAM write {}: expected {:?}, got {:?}. Total writes: expected {}, got {}.", i + 1, writes.get(i), got.get(i), writes.len(), got.len()));
        }
        if ram != expected {
            return Err(trf!(
                "Girdi {n}: hedef dışındaki RAM hücreleri değişti.",
                "Input {n}: RAM cells outside the target changed."
            ));
        }
    }
    Ok(trf!("{} farklı başlangıçla geçti. Komutlar ve RAM yazma sırası doğrulandı; devre seviyesi ayrıca test edilir.", "Passed {} initial states. Instructions and RAM write order verified; the circuit level is tested separately.", inputs.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lessons_work_in_both_languages_without_unlocking_hardware() {
        for lang in makine_lang::Lang::ALL {
            makine_lang::set_for_thread(Some(lang));
            let mut solved = std::collections::BTreeSet::new();
            for l in all() {
                assert!(makine_levels::find(l.chip).is_some());
                assert!(unlocked(l.id, &solved));
                assert!(check(l.id, l.solution).is_ok(), "{}", l.id);
                assert!(check(l.id, l.starter).is_err(), "{}", l.id);
                assert!(check(l.id, "loop: JMP loop").unwrap_err().contains("2048"));
                solved.insert(l.id.into());
            }
        }
        makine_lang::set_for_thread(None);
    }
    #[test]
    fn rejects_constants_missing_zero_and_initialization_overrides() {
        assert!(check("mini-memory", "LDI 14\nST 1\nHLT").is_err());
        assert!(check("mini-memory", "LD 0\nADD 0\nST 1\nHLT\n.ram 0,7").is_ok());
        assert!(check(
            "mini-loop",
            "LD 0\nloop: SUB 15\nST 1\nJZ done\nJMP loop\ndone: HLT"
        )
        .is_err());
        assert!(check("mini-loop", "LDI 0\nST 1\nHLT").is_err());
        assert!(check("mini-rom", "LDI 5\nST 0\nADD 0\nST 1\nST 2\nHLT").is_err());
    }
}
