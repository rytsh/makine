//! An optional side path: a small graphics processor. Lanes that do the
//! same work on several pixels at once, the edge function, the triangle
//! test, a counter that scans the screen and the rasterizer that puts them
//! together. In World, the rasterizer draws on a 32×32 screen.

use makine_sim::{out, ChipBuilder, ChipDef, Part, PartKind, Rng, Source, TestStep};

use crate::{Level, Test};

pub const SECTION: &str = "8. Grafik işlemci";

/// Screen coordinates are 5 bits: a 32 × 32 screen.
pub const COORD: u8 = 5;
const SIDE: u64 = 1 << COORD;

pub static LEVELS: &[Level] = &[
    Level {
        chip: "Simd4",
        section: SECTION,
        title: "Dört Şerit: SIMD",
        description: "a ve b, yan yana dört tane 8 bitlik sayı taşır (bayt 0 en düşük bitlerde). \
            Her baytı ayrı topla: out'un bayt k'sı = a'nın bayt k'sı + b'nin bayt k'sı. sat = 0 \
            ise taşan atılır (200 + 100 = 44); sat = 1 ise sonuç 255'te kalır (200 + 100 = 255).\n\n\
            GPU'nun temel fikri budur: tek komut, çok veri (SIMD). Bir pikselin rengini \
            parlatan aynı işlem dört piksele birden uygulanır. Dikkat: bir şeridin eldesi \
            yandakine geçmemeli; her şerit kendi başına bir toplayıcıdır.",
        inputs: &["a[32]", "b[32]", "sat"],
        outputs: &["out[32]"],
        test: Test::Function(simd4),
    },
    Level {
        chip: "Edge",
        section: SECTION,
        title: "Kenar Fonksiyonu",
        description: "a'dan b'ye giden kenar ve bir p noktası (koordinatlar 0..31). \
            E = (bx − ax) × (py − ay) − (by − ay) × (px − ax) hesapla; E ≥ 0 ise right = 1.\n\n\
            E'nin işareti, p'nin kenarın hangi yanında olduğunu söyler. Ekran koordinatlarında \
            y aşağı doğru büyür; E ≥ 0, p kenarın sağında (ya da üstünde) demektir. Farklar \
            −31..31 arasıdır, çarpımlar işaretlidir: Sub32 iki tümleyenli farkı zaten verir. \
            Çarpma için MulDiv'i kullanabilirsin ama çok büyüktür; 6 bitlik işaretli bir çarpma \
            altı satır topla-seç ile olur.",
        inputs: &["ax[5]", "ay[5]", "bx[5]", "by[5]", "px[5]", "py[5]"],
        outputs: &["right"],
        test: Test::Function(|i| vec![(edge(i[0], i[1], i[2], i[3], i[4], i[5]) >= 0) as u64]),
    },
    Level {
        chip: "Triangle",
        section: SECTION,
        title: "Üçgenin İçi",
        description: "Köşeleri a, b, c olan üçgen ve bir p noktası. p, üç kenarın (a→b, b→c, c→a) \
            hepsinin sağındaysa ya da üstündeyse inside = 1.\n\n\
            Köşeler ekranda saat yönünde sıralanırsa içerisi, üç kenarın da sağında kalır. \
            GPU'lar üçgeni böyle çizer: ekranın her pikseli için bu testi yapar, hepsini \
            birbirinden bağımsız ve aynı anda. Buna rasterleme denir.",
        inputs: &[
            "ax[5]", "ay[5]", "bx[5]", "by[5]", "cx[5]", "cy[5]", "px[5]", "py[5]",
        ],
        outputs: &["inside"],
        test: Test::Function(|i| vec![triangle(i) as u64]),
    },
    Level {
        chip: "ScanCounter",
        section: SECTION,
        title: "Ekranı Tara",
        description: "32 × 32'lik ekranın piksellerini satır satır gezen sayaç. Her yükselen clk \
            kenarında: reset = 1 ise x = 0, y = 0. Değilse x bir artar; x 31 iken 0'a döner ve y \
            bir artar. y de 31'den sonra 0'a döner.\n\n\
            Ekranlar görüntüyü böyle çizer: soldan sağa, yukarıdan aşağıya, saniyede onlarca \
            kez. Bir ekran karesi 32 × 32 = 1024 saat sürer.",
        inputs: &["reset", "clk"],
        outputs: &["x[5]", "y[5]"],
        test: Test::Sequence(scan_steps),
    },
    Level {
        chip: "Fill",
        section: SECTION,
        title: "Rasterleyici",
        description: "Ekranı tara ve her piksel için üçgenin içinde olup olmadığını söyle. x, y \
            ScanCounter'daki gibi ilerlesin (reset, clk); on = 1 ise (x, y) üçgenin içindedir.\n\n\
            Bu, bir GPU'nun en temel işidir. Dünya'da on'u ekranın d girişine, x ve y'yi \
            koordinatlarına bağlarsan üçgen çizilir. Gerçek bir GPU aynı işi binlerce \
            çekirdekle, binlerce pikselde birden yapar.",
        inputs: &["ax[5]", "ay[5]", "bx[5]", "by[5]", "cx[5]", "cy[5]", "reset", "clk"],
        outputs: &["x[5]", "y[5]", "on"],
        test: Test::Sequence(fill_steps),
    },
];

fn simd4(i: &[u64]) -> Vec<u64> {
    let mut r = 0;
    for k in 0..4 {
        let (a, b) = (i[0] >> (8 * k) & 255, i[1] >> (8 * k) & 255);
        let s = if i[2] == 1 {
            (a + b).min(255)
        } else {
            (a + b) & 255
        };
        r |= s << (8 * k);
    }
    vec![r]
}

/// The edge function of edge a→b at point p.
pub fn edge(ax: u64, ay: u64, bx: u64, by: u64, px: u64, py: u64) -> i64 {
    let d = |a: u64, b: u64| b as i64 - a as i64;
    d(ax, bx) * d(ay, py) - d(ay, by) * d(ax, px)
}

/// Whether p is inside triangle abc: on the right of (or on) every edge.
/// `i`: ax, ay, bx, by, cx, cy, px, py.
pub fn triangle(i: &[u64]) -> bool {
    let (px, py) = (i[6], i[7]);
    [(0, 2), (2, 4), (4, 0)]
        .iter()
        .all(|&(s, t)| edge(i[s], i[s + 1], i[t], i[t + 1], px, py) >= 0)
}

/// (x, y) after a clock edge.
pub fn scan_next(x: u64, y: u64, reset: bool) -> (u64, u64) {
    if reset {
        (0, 0)
    } else if x == SIDE - 1 {
        (0, (y + 1) % SIDE)
    } else {
        (x + 1, y)
    }
}

/// Steps for a scan counter whose inputs end in `reset, clk`, with `more`
/// inputs before them. `expect(x, y)` gives the outputs for a position.
fn scanned(
    seed: u64,
    cycles: usize,
    more: impl Fn(usize) -> Vec<u64>,
    expect: impl Fn(Option<(u64, u64)>, &[u64]) -> Vec<Option<u64>>,
) -> Vec<TestStep> {
    let mut rng = Rng(seed);
    let mut at: Option<(u64, u64)> = None;
    let mut steps = Vec::new();
    for cycle in 0..cycles {
        // A reset at the start, and now and then (but a whole frame runs
        // without one, so y wraps).
        let reset = cycle == 0 || (cycle > 1100 && rng.next_u64().is_multiple_of(64));
        let ins = more(cycle);
        let with = |r: bool, clk: u64| {
            let mut v = ins.clone();
            v.extend([r as u64, clk]);
            v
        };
        steps.push(TestStep {
            inputs: with(reset, 0),
            expect: expect(at, &ins),
        });
        let (x, y) = at.unwrap_or((0, 0));
        if at.is_some() || reset {
            at = Some(scan_next(x, y, reset));
        }
        steps.push(TestStep {
            inputs: with(reset, 1),
            expect: expect(at, &ins),
        });
        // reset changing while clk stays high is not a new edge.
        steps.push(TestStep {
            inputs: with(!reset, 1),
            expect: expect(at, &ins),
        });
    }
    steps
}

fn scan_steps() -> Vec<TestStep> {
    scanned(
        80,
        1200,
        |_| vec![],
        |at, _| vec![at.map(|(x, _)| x), at.map(|(_, y)| y)],
    )
}

fn fill_steps() -> Vec<TestStep> {
    // A few triangles, each for a whole frame: clockwise, counter-clockwise
    // (nothing inside) and thin ones.
    let tris: [[u64; 6]; 3] = [
        [2, 3, 29, 8, 9, 27],
        [2, 3, 9, 27, 29, 8],
        [0, 0, 31, 31, 0, 31],
    ];
    scanned(
        81,
        1200,
        |cycle| tris[cycle / 400].to_vec(),
        |at, ins| match at {
            None => vec![None; 3],
            Some((x, y)) => {
                let mut v = ins.to_vec();
                v.extend([x, y]);
                vec![Some(x), Some(y), Some(triangle(&v) as u64)]
            }
        },
    )
}

/// Reference solutions.
pub fn reference(chip: &str) -> Option<ChipDef> {
    let level = crate::find(chip)?;
    let mut b = ChipBuilder::new(chip, level.inputs, level.outputs);
    match chip {
        "Simd4" => {
            let (x, y, sat) = (b.input("a"), b.input("b"), b.input("sat"));
            let xs = b.split(&[8; 4], x);
            let ys = b.split(&[8; 4], y);
            let zero = b.constant(1, 0);
            let full = b.constant(8, 255);
            let mut lanes = Vec::new();
            for k in 0..4 {
                let s = b.chip("Adder8", &[out(xs, k), out(ys, k), zero]);
                let clamp = b.chip1("And", &[out(s, 1), sat]);
                let v = b.chip_n("Mux", 8, &[out(s, 0), full, clamp]);
                lanes.push(out(v, 0));
            }
            let r = b.join(&[8; 4], &lanes);
            b.output("out", r);
        }
        "Edge" => {
            let p = ["ax", "ay", "bx", "by", "px", "py"].map(|n| b.input(n));
            let right = edge_circuit(&mut b, [p[0], p[1]], [p[2], p[3]], [p[4], p[5]]);
            b.output("right", right);
        }
        "Triangle" => {
            let p = ["ax", "ay", "bx", "by", "cx", "cy", "px", "py"].map(|n| b.input(n));
            let pt = [p[6], p[7]];
            let e1 = b.chip1("Edge", &[p[0], p[1], p[2], p[3], pt[0], pt[1]]);
            let e2 = b.chip1("Edge", &[p[2], p[3], p[4], p[5], pt[0], pt[1]]);
            let e3 = b.chip1("Edge", &[p[4], p[5], p[0], p[1], pt[0], pt[1]]);
            let both = b.chip1("And", &[e1, e2]);
            let all = b.chip1("And", &[both, e3]);
            b.output("inside", all);
        }
        "ScanCounter" => {
            let (reset, clk) = (b.input("reset"), b.input("clk"));
            let reg = b.part(
                Part {
                    kind: PartKind::Chip("Bit".into()),
                    repeat: 2 * COORD,
                },
                &[],
            );
            let xy = b.split(&[COORD, COORD], out(reg, 0));
            let (x, y) = (out(xy, 0), out(xy, 1));
            let zero1 = b.constant(1, 0);
            let one1 = b.constant(1, 1);
            let pad = b.constant(3, 0);
            let one8 = b.constant(8, 1);
            // x + 1; its bit 5 is set when x was 31.
            let x8 = b.join(&[COORD, 3], &[x, pad]);
            let xp = b.chip("Adder8", &[x8, one8, zero1]);
            let xs = b.split(&[COORD, 1, 2], out(xp, 0));
            let wrap = out(xs, 1);
            // y + wrap.
            let y8 = b.join(&[COORD, 3], &[y, pad]);
            let zero8 = b.constant(8, 0);
            let yp = b.chip("Adder8", &[y8, zero8, wrap]);
            let ys = b.split(&[COORD, 3], out(yp, 0));
            let next = b.join(&[COORD, COORD], &[out(xs, 0), out(ys, 0)]);
            let zero = b.constant(2 * COORD, 0);
            let d = b.chip_n("Mux", 2 * COORD, &[next, zero, reset]);
            b.wire(out(d, 0), makine_sim::into(reg, 0));
            b.wire(one1, makine_sim::into(reg, 1));
            b.wire(clk, makine_sim::into(reg, 2));
            b.output("x", x);
            b.output("y", y);
        }
        "Fill" => {
            let p = ["ax", "ay", "bx", "by", "cx", "cy"].map(|n| b.input(n));
            let (reset, clk) = (b.input("reset"), b.input("clk"));
            let scan = b.chip("ScanCounter", &[reset, clk]);
            let (x, y) = (out(scan, 0), out(scan, 1));
            let on = b.chip1("Triangle", &[p[0], p[1], p[2], p[3], p[4], p[5], x, y]);
            b.output("x", x);
            b.output("y", y);
            b.output("on", on);
        }
        _ => return None,
    }
    Some(b.build())
}

/// A 5-bit coordinate as a 32-bit number.
fn wide(b: &mut ChipBuilder, v: Source) -> Source {
    let zero = b.constant(32 - COORD, 0);
    b.join(&[COORD, 32 - COORD], &[v, zero])
}

/// `a << k`, 32 bits.
fn shl(b: &mut ChipBuilder, a: Source, k: u8) -> Source {
    if k == 0 {
        return a;
    }
    let parts = b.split(&[32 - k, k], a);
    let zero = b.constant(k, 0);
    b.join(&[k, 32 - k], &[zero, out(parts, 0)])
}

/// `a × m`, where `a` is any 32-bit number and `m` a 32-bit number in
/// −32..31: only its low 6 bits are used, as a two's complement number.
/// Six rows of add-and-select; the top row subtracts.
fn mul6(b: &mut ChipBuilder, a: Source, m: Source) -> Source {
    let bits = b.split(&[1, 1, 1, 1, 1, 1, 26], m);
    let mut acc = b.constant(32, 0);
    let zero = b.constant(1, 0);
    for i in 0..6u8 {
        let row = shl(b, a, i);
        let next = if i < 5 {
            out(b.chip("Adder32", &[acc, row, zero]), 0)
        } else {
            b.chip1("Sub32", &[acc, row])
        };
        acc = b.chip1("Mux32", &[acc, next, out(bits, i as usize)]);
    }
    acc
}

/// 1 if p is on the right of (or on) edge a→b.
fn edge_circuit(b: &mut ChipBuilder, a: [Source; 2], e: [Source; 2], p: [Source; 2]) -> Source {
    let [ax, ay, bx, by, px, py] = [a[0], a[1], e[0], e[1], p[0], p[1]].map(|v| wide(b, v));
    let dx = b.chip1("Sub32", &[bx, ax]);
    let dy = b.chip1("Sub32", &[by, ay]);
    let qx = b.chip1("Sub32", &[px, ax]);
    let qy = b.chip1("Sub32", &[py, ay]);
    let m1 = mul6(b, dx, qy);
    let m2 = mul6(b, dy, qx);
    let e = b.chip1("Sub32", &[m1, m2]);
    let sign = b.split(&[31, 1], e);
    b.chip1("Not", &[out(sign, 1)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, find, levels};
    use makine_sim::{Library, Verdict};

    fn library() -> Library {
        let mut lib = Library::new();
        for l in levels() {
            if l.section == SECTION {
                break;
            }
            if !l.optional() {
                lib.insert(crate::reference::reference(l.chip));
            }
        }
        lib
    }

    #[test]
    fn references_pass() {
        let mut lib = library();
        for level in LEVELS {
            assert!(level.optional());
            let def = reference(level.chip).unwrap();
            assert_eq!(
                check(level, &lib, &def).unwrap(),
                Verdict::Pass,
                "{}",
                level.chip
            );
            lib.insert(def);
        }
    }

    #[test]
    fn edge_and_triangle() {
        // A clockwise triangle on screen (y grows down) has its inside on
        // the right of every edge.
        let t = [0, 0, 10, 0, 0, 10];
        let inside = |x, y| {
            let mut v = t.to_vec();
            v.extend([x, y]);
            triangle(&v)
        };
        assert!(inside(1, 1) && inside(0, 0) && inside(5, 5));
        assert!(!inside(6, 6) && !inside(20, 1));
        assert_eq!(simd4(&[0x01FF_C864, 0x0101_6464, 0]), vec![0x0200_2CC8]);
        assert_eq!(simd4(&[0x01FF_C864, 0x0101_6464, 1]), vec![0x02FF_FFC8]);
    }

    /// One adder across all lanes lets a carry leak into the next lane.
    #[test]
    fn carry_between_lanes_fails() {
        let lib = library();
        let mut b = ChipBuilder::new("Simd4", &["a[32]", "b[32]", "sat"], &["out[32]"]);
        let (x, y) = (b.input("a"), b.input("b"));
        let zero = b.constant(1, 0);
        let s = b.chip("Adder32", &[x, y, zero]);
        b.output("out", out(s, 0));
        assert!(matches!(
            check(find("Simd4").unwrap(), &lib, &b.build()).unwrap(),
            Verdict::Fail(_)
        ));
    }

    /// The counter-clockwise triangle in the Fill test has no inside, and
    /// the clockwise one covers a fair part of the screen.
    #[test]
    fn fill_triangles_are_interesting() {
        let count = |t: [u64; 6]| {
            (0..SIDE * SIDE)
                .filter(|&k| {
                    let mut v = t.to_vec();
                    v.extend([k % SIDE, k / SIDE]);
                    triangle(&v)
                })
                .count()
        };
        let n = count([2, 3, 29, 8, 9, 27]);
        assert!((200..600).contains(&n), "{n}");
        assert_eq!(count([2, 3, 9, 27, 29, 8]), 0);
        assert_eq!(makine_sim::mask(COORD), SIDE - 1);
    }
}
