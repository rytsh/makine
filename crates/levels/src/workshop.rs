//! Small-bus exercises for the processor workshop. Appended after the
//! existing path so saved progress and the RISC-V prerequisites keep their
//! meaning.
use crate::{Level, Test};
use makine_sim::{mask, TestStep};

pub static LEVELS: &[Level] = &[
    Level {
        chip: "Mux4", section: "6. İşlemci atölyesi", title: "4-bit Seçici",
        description: "İki 4-bit bus'tan birini seç. sel=0 ise a, sel=1 ise b.\n\nTek bitlik MUX'un dört kopyası aynı seçim sinyalini paylaşabilir. Bir işlemcinin hangi değeri yazacağını bu seçiciler belirler.",
        inputs: &["a[4]", "b[4]", "sel"], outputs: &["out[4]"],
        test: Test::Function(|i| vec![if i[2] == 0 { i[0] } else { i[1] }]),
    },
    Level {
        chip: "Adder4", section: "6. İşlemci atölyesi", title: "4-bit Toplayıcı ve Taşıma",
        description: "a+b+c işlemini yap; dört düşük biti sum, beşinci biti carry ver.\n\n15+1 için sum=0 ve carry=1 olmalı. Intel 4004 gibi 4-bit işlemciler daha geniş sayıları taşıma bitiyle parça parça toplar.",
        inputs: &["a[4]", "b[4]", "c"], outputs: &["sum[4]", "carry"],
        test: Test::Function(|i| {let v=i[0]+i[1]+i[2];vec![v&15,v>>4]}),
    },
    Level {
        chip: "Register4", section: "6. İşlemci atölyesi", title: "4-bit Yazmaç",
        description: "load=1 iken yükselen clk kenarında d'yi sakla; diğer zamanlarda q değişmesin.\n\nALU yalnızca hesaplar. Akümülatör, hesaplanan sonucu sonraki komuta kadar saklayan yazmaçtır.",
        inputs: &["d[4]", "load", "clk"], outputs: &["q[4]"],
        test: Test::Sequence(register_steps),
    },
    Level {
        chip: "Counter12", section: "6. İşlemci atölyesi", title: "12-bit Program Sayacı",
        description: "Her yükselen clk kenarında: reset varsa 0; değilse load varsa d; değilse inc varsa bir artır; aksi halde koru.\n\n12 bit 4096 adres seçer. Dallanma, sayacı artırmak yerine başka bir adres yüklemektir. 4095'ten sonraki adres 0 olur.",
        inputs: &["d[12]", "load", "inc", "reset", "clk"], outputs: &["out[12]"],
        test: Test::Sequence(counter_steps),
    },
];

fn register_steps() -> Vec<TestStep> {
    let mut out = Vec::new();
    let mut q = 0;
    for i in 0..96u64 {
        let d = (i * 7) & 15;
        let load = (i % 3 != 0) as u64;
        out.push(step(&[d, load, 0], &[q]));
        if load == 1 {
            q = d;
        }
        out.push(step(&[d, load, 1], &[q]));
        // No new edge: changing d while clk stays high cannot write.
        out.push(step(&[d ^ 15, load, 1], &[q]));
    }
    out
}

fn counter_steps() -> Vec<TestStep> {
    let mut out = Vec::new();
    let mut q = 0;
    for i in 0..96u64 {
        let d = if i == 0 { 4095 } else { (i * 137) & mask(12) };
        let load = (i % 5 == 0) as u64;
        let inc = (i % 3 != 0) as u64;
        let reset = (i % 17 == 16) as u64;
        out.push(step(&[d, load, inc, reset, 0], &[q]));
        q = if reset == 1 {
            0
        } else if load == 1 {
            d
        } else {
            (q + inc) & 4095
        };
        out.push(step(&[d, load, inc, reset, 1], &[q]));
        out.push(step(&[0, 1, 1, 1, 1], &[q]));
    }
    out
}

fn step(inputs: &[u64], values: &[u64]) -> TestStep {
    TestStep {
        inputs: inputs.to_vec(),
        expect: values.iter().copied().map(Some).collect(),
    }
}
