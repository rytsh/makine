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
    Level {
        chip: "Shade",
        section: SECTION,
        title: "Renkli Piksel",
        description: "color, 12 bitlik bir renk: bit 8..11 kırmızı, 4..7 yeşil, 0..3 mavi; her \
            kanal 0..15. near, pikselin göze yakınlığı (0 en uzak, 255 en yakın). Uzaktaki \
            şeyler daha karanlık görünsün: k = near'ın en üst iki biti (0..3) olsun; her kanal \
            için out = kanal × (k + 1) ÷ 4 (kalan atılır).\n\n\
            Ekranın her pikseli artık bir sayı değil bir renktir. GPU'lar renkleri kanal kanal \
            hesaplar: aynı işlem üç kanala ayrı ayrı uygulanır, Simd4'teki şeritler gibi. \
            Kanal × 3 = kanal + kanal × 2; × 2 ve × 4 sadece kaydırmadır. ÷ 4 de öyle.",
        inputs: &["color[12]", "near[8]"],
        outputs: &["out[12]"],
        test: Test::Function(|i| vec![shade(i[0], i[1])]),
    },
    Level {
        chip: "DepthTest",
        section: SECTION,
        title: "Derinlik Testi",
        description: "on = 1 ise piksel üçgenin içindedir. near, üçgenin bu pikseldeki yakınlığı; \
            old, derinlik tamponunda bu piksel için saklı olan yakınlık. on = 1 ve near > old \
            ise write = 1 olsun: piksel öndekinden daha yakın, çizilmeli.\n\n\
            Üçgenler üst üste binince hangisi görünür? Ressam gibi uzaktan yakına çizmek \
            her zaman işe yaramaz. GPU'lar her piksel için en yakın şeyin yakınlığını bir \
            bellekte, derinlik tamponunda (z-buffer) tutar. Yeni bir piksel ancak daha yakınsa \
            çizilir ve tampon güncellenir. Tampon 0 ile başlar: her şey sonsuz uzakta.",
        inputs: &["on", "near[8]", "old[8]"],
        outputs: &["write"],
        test: Test::Function(|i| vec![(i[0] == 1 && i[1] > i[2]) as u64]),
    },
    Level {
        chip: "FrameScan",
        section: SECTION,
        title: "Üçgen Sırası",
        description: "ScanCounter gibi x ve y'yi tara, ama bir de t (0..3) üçgen sayacı tut. Her \
            yükselen clk kenarında: reset = 1 ise x = y = t = 0. Değilse x bir artar; x 31'den \
            0'a dönünce y bir artar; y de 31'den 0'a dönünce (bir kare bitince) t bir artar. t, \
            3'ten sonra 0'a döner.\n\n\
            Bir sahne birçok üçgenden oluşur. En basit yol üçgenleri sırayla çizmektir: her \
            karede bir üçgen. t, hangi üçgenin köşelerinin bellekten okunacağını söyler. \
            Gerçek GPU'lar milyonlarca üçgeni aynı anda, boru hattı içinde işler.",
        inputs: &["reset", "clk"],
        outputs: &["x[5]", "y[5]", "t[2]"],
        test: Test::Sequence(frame_steps),
    },
    Level {
        chip: "Raster3",
        section: SECTION,
        title: "Sahne Rasterleyici",
        description: "Hepsini birleştir. x, y, t FrameScan'deki gibi ilerlesin. Girişlerdeki \
            köşeler, near ve color, t numaralı üçgenindir (onları belleğe göre Dünya sağlar). old, \
            derinlik tamponundaki (x, y) değeridir. write = 1: (x, y) t'nin içinde ve near > old. \
            c = color, near'a göre Renkli Piksel'deki gibi karartılmış.\n\n\
            Dünya'da: t bir ROM'dan üçgenin köşelerini, rengini ve yakınlığını seçer; bir RAM \
            derinlik tamponudur; renkli ekran write = 1 olan pikselleri c rengine boyar. \
            Dört renkli üçgen, birbirinin önünde ve arkasında, doğru sırayla görünür.",
        inputs: &[
            "ax[5]",
            "ay[5]",
            "bx[5]",
            "by[5]",
            "cx[5]",
            "cy[5]",
            "near[8]",
            "color[12]",
            "old[8]",
            "reset",
            "clk",
        ],
        outputs: &["x[5]", "y[5]", "t[2]", "write", "c[12]"],
        test: Test::Sequence(raster_steps),
    },
];

/// `color` (4 bits each of red, green, blue) darkened by `near`: each
/// channel × (k + 1) / 4, k = the top two bits of `near`.
pub fn shade(color: u64, near: u64) -> u64 {
    let k = (near >> 6) & 3;
    (0..3).fold(0, |acc, ch| {
        let v = (color >> (4 * ch)) & 15;
        acc | ((v * (k + 1)) >> 2) << (4 * ch)
    })
}

/// (x, y, t) after a clock edge: one triangle per frame.
pub fn frame_next(x: u64, y: u64, t: u64, reset: bool) -> (u64, u64, u64) {
    if reset {
        return (0, 0, 0);
    }
    let (nx, ny) = scan_next(x, y, false);
    let t = if x == SIDE - 1 && y == SIDE - 1 {
        (t + 1) % 4
    } else {
        t
    };
    (nx, ny, t)
}

/// A scene for the multi-triangle levels: corners, nearness and colour of
/// each of the four triangles (all clockwise on screen).
pub const SCENE: [([u64; 6], u64, u64); 4] = [
    ([1, 2, 30, 4, 6, 29], 60, 0x36C),
    ([10, 6, 30, 22, 4, 26], 160, 0xE52),
    ([2, 12, 17, 1, 22, 30], 110, 0x4D5),
    ([14, 12, 25, 16, 13, 24], 250, 0xFE3),
];

/// The colours on the 32 × 32 screen once the scene has been drawn with a
/// depth buffer: `None` where nothing was drawn.
pub fn scene_pixels(scene: &[([u64; 6], u64, u64)]) -> Vec<Option<u64>> {
    let mut near = vec![0u64; (SIDE * SIDE) as usize];
    let mut px = vec![None; (SIDE * SIDE) as usize];
    for (v, z, color) in scene {
        for y in 0..SIDE {
            for x in 0..SIDE {
                let mut i = v.to_vec();
                i.extend([x, y]);
                let k = (y * SIDE + x) as usize;
                if triangle(&i) && *z > near[k] {
                    near[k] = *z;
                    px[k] = Some(shade(*color, *z));
                }
            }
        }
    }
    px
}

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

/// Steps for a frame scanner whose inputs end in `reset, clk`: like
/// [`scanned`], with the triangle counter, over several frames.
fn frame_scanned(
    seed: u64,
    cycles: usize,
    more: impl Fn(Option<(u64, u64, u64)>, usize) -> Vec<u64>,
    expect: impl Fn(Option<(u64, u64, u64)>, &[u64]) -> Vec<Option<u64>>,
) -> Vec<TestStep> {
    let mut rng = Rng(seed);
    let mut at: Option<(u64, u64, u64)> = None;
    let mut steps = Vec::new();
    let frame = (SIDE * SIDE) as usize;
    for cycle in 0..cycles {
        // Five whole frames without a reset, so t wraps; then now and then.
        let reset = cycle == 0 || (cycle > 5 * frame + 2 && rng.next_u64().is_multiple_of(97));
        let ins = more(at, cycle);
        let with = |r: bool, clk: u64| {
            let mut v = ins.clone();
            v.extend([r as u64, clk]);
            v
        };
        steps.push(TestStep {
            inputs: with(reset, 0),
            expect: expect(at, &ins),
        });
        let (x, y, t) = at.unwrap_or((0, 0, 0));
        if at.is_some() || reset {
            at = Some(frame_next(x, y, t, reset));
        }
        let ins2 = more(at, cycle);
        let with2 = |r: bool, clk: u64| {
            let mut v = ins2.clone();
            v.extend([r as u64, clk]);
            v
        };
        steps.push(TestStep {
            inputs: with2(reset, 1),
            expect: expect(at, &ins2),
        });
        steps.push(TestStep {
            inputs: with2(!reset, 1),
            expect: expect(at, &ins2),
        });
    }
    steps
}

fn frame_steps() -> Vec<TestStep> {
    frame_scanned(
        82,
        5 * 1024 + 300,
        |_, _| vec![],
        |at, _| match at {
            None => vec![None; 3],
            Some((x, y, t)) => vec![Some(x), Some(y), Some(t)],
        },
    )
}

/// The scene rasterizer's test: the inputs give triangle `t` of a scene
/// (as World's ROM does) and a depth buffer value; the outputs say whether
/// to write and in which colour.
fn raster_steps() -> Vec<TestStep> {
    // The example scene, then one where every triangle is flipped (nothing
    // inside), then thin ones; old varies so the comparison is exercised.
    let flipped: Vec<_> = SCENE
        .iter()
        .map(|(v, z, c)| ([v[0], v[1], v[4], v[5], v[2], v[3]], *z, *c))
        .collect();
    let scenes = [SCENE.to_vec(), flipped];
    frame_scanned(
        83,
        5 * 1024 + 300,
        move |at, cycle| {
            let (x, y, t) = at.unwrap_or((0, 0, 0));
            let scene = &scenes[(cycle / 3000) % 2];
            let (v, z, color) = scene[t as usize];
            // old: the near value of a fixed pattern, so both sides of the
            // comparison come up, including equal.
            let old = match (x * 7 + y * 13) % 5 {
                0 => 0,
                1 => z,
                2 => z.saturating_sub(1),
                3 => (z + 1).min(255),
                _ => (x * 37 + y * 11) & 255,
            };
            let mut i = v.to_vec();
            i.extend([z, color, old]);
            i
        },
        |at, ins| match at {
            None => vec![None; 5],
            Some((x, y, t)) => {
                let mut v = ins[..6].to_vec();
                v.extend([x, y]);
                let (near, color, old) = (ins[6], ins[7], ins[8]);
                let write = triangle(&v) && near > old;
                vec![
                    Some(x),
                    Some(y),
                    Some(t),
                    Some(write as u64),
                    Some(shade(color, near)),
                ]
            }
        },
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
        "Shade" => {
            let (color, near) = (b.input("color"), b.input("near"));
            let n = b.split(&[6, 1, 1], near);
            let (k0, k1) = (out(n, 1), out(n, 2));
            let ch = b.split(&[4, 4, 4], color);
            let zero1 = b.constant(1, 0);
            let mut outs = Vec::new();
            for c in 0..3 {
                // v × (k + 1) = v + k0·v + k1·2v, in 8 bits; then ÷ 4.
                let z4 = b.constant(4, 0);
                let v = b.join(&[4, 4], &[out(ch, c), z4]);
                let z1 = b.constant(1, 0);
                let z3 = b.constant(3, 0);
                let v2 = b.join(&[1, 4, 3], &[z1, out(ch, c), z3]);
                let zero8 = b.constant(8, 0);
                let p1 = b.chip_n("Mux", 8, &[zero8, v, k0]);
                let p2 = b.chip_n("Mux", 8, &[zero8, v2, k1]);
                let s1 = b.chip("Adder8", &[v, out(p1, 0), zero1]);
                let s2 = b.chip("Adder8", &[out(s1, 0), out(p2, 0), zero1]);
                let parts = b.split(&[2, 4, 2], out(s2, 0));
                outs.push(out(parts, 1));
            }
            let r = b.join(&[4, 4, 4], &outs);
            b.output("out", r);
        }
        "DepthTest" => {
            let (on, near, old) = (b.input("on"), b.input("near"), b.input("old"));
            // near > old: old − near borrows. old + NOT near + 1 has no carry.
            let nn = b.chip_n("Not", 8, &[near]);
            let one = b.constant(1, 1);
            let d = b.chip("Adder8", &[old, out(nn, 0), one]);
            let closer = b.chip1("Not", &[out(d, 1)]);
            let w = b.chip1("And", &[on, closer]);
            b.output("write", w);
        }
        "FrameScan" => {
            let (reset, clk) = (b.input("reset"), b.input("clk"));
            let reg = b.part(
                Part {
                    kind: PartKind::Chip("Bit".into()),
                    repeat: 2 * COORD + 2,
                },
                &[],
            );
            let s = b.split(&[COORD, COORD, 2], out(reg, 0));
            let (x, y, t) = (out(s, 0), out(s, 1), out(s, 2));
            let zero1 = b.constant(1, 0);
            let one1 = b.constant(1, 1);
            let one8 = b.constant(8, 1);
            let zero8 = b.constant(8, 0);
            // x + 1, y + (x wrapped), t + (y wrapped too), as ScanCounter.
            let pad = b.constant(3, 0);
            let x8 = b.join(&[COORD, 3], &[x, pad]);
            let xp = b.chip("Adder8", &[x8, one8, zero1]);
            let xs = b.split(&[COORD, 1, 2], out(xp, 0));
            let y8 = b.join(&[COORD, 3], &[y, pad]);
            let yp = b.chip("Adder8", &[y8, zero8, out(xs, 1)]);
            let ys = b.split(&[COORD, 1, 2], out(yp, 0));
            let pad6 = b.constant(6, 0);
            let t8 = b.join(&[2, 6], &[t, pad6]);
            let tp = b.chip("Adder8", &[t8, zero8, out(ys, 1)]);
            let ts = b.split(&[2, 6], out(tp, 0));
            let next = b.join(&[COORD, COORD, 2], &[out(xs, 0), out(ys, 0), out(ts, 0)]);
            let zero = b.constant(2 * COORD + 2, 0);
            let d = b.chip_n("Mux", 2 * COORD + 2, &[next, zero, reset]);
            b.wire(out(d, 0), makine_sim::into(reg, 0));
            b.wire(one1, makine_sim::into(reg, 1));
            b.wire(clk, makine_sim::into(reg, 2));
            b.output("x", x);
            b.output("y", y);
            b.output("t", t);
        }
        "Raster3" => {
            let p = ["ax", "ay", "bx", "by", "cx", "cy"].map(|n| b.input(n));
            let (near, color, old) = (b.input("near"), b.input("color"), b.input("old"));
            let (reset, clk) = (b.input("reset"), b.input("clk"));
            let scan = b.chip("FrameScan", &[reset, clk]);
            let (x, y, t) = (out(scan, 0), out(scan, 1), out(scan, 2));
            let on = b.chip1("Triangle", &[p[0], p[1], p[2], p[3], p[4], p[5], x, y]);
            let w = b.chip1("DepthTest", &[on, near, old]);
            let c = b.chip1("Shade", &[color, near]);
            b.output("x", x);
            b.output("y", y);
            b.output("t", t);
            b.output("write", w);
            b.output("c", c);
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

    /// The scene has overlaps where the depth test decides, and each of
    /// the four colours shows somewhere.
    #[test]
    fn scene_needs_the_depth_buffer() {
        let px = scene_pixels(&SCENE);
        for (_, z, c) in SCENE {
            let want = shade(c, z);
            assert!(px.contains(&Some(want)), "colour {c:03X} missing");
        }
        // Drawn without a depth test (last one wins), it looks different.
        let mut painter = vec![None; px.len()];
        for (v, z, c) in SCENE {
            for (k, p) in painter.iter_mut().enumerate() {
                let mut i = v.to_vec();
                i.extend([k as u64 % SIDE, k as u64 / SIDE]);
                if triangle(&i) {
                    *p = Some(shade(c, z));
                }
            }
        }
        let differ = px.iter().zip(&painter).filter(|(a, b)| a != b).count();
        assert!(differ > 50, "{differ}");
        assert_eq!(shade(0xFE3, 250), 0xFE3);
        assert_eq!(shade(0xFE3, 60), 0x330);
    }

    /// Writing on equal nearness, or without on, is wrong.
    #[test]
    fn depth_test_is_strict() {
        let mut lib = library();
        for chip in ["Simd4", "Edge", "Triangle", "ScanCounter", "Fill", "Shade"] {
            lib.insert(reference(chip).unwrap());
        }
        let level = find("DepthTest").unwrap();
        let mut b = ChipBuilder::new("DepthTest", level.inputs, level.outputs);
        let (on, near, old) = (b.input("on"), b.input("near"), b.input("old"));
        // near ≥ old: near + NOT old + 1 carries.
        let no = b.chip_n("Not", 8, &[old]);
        let one = b.constant(1, 1);
        let d = b.chip("Adder8", &[near, out(no, 0), one]);
        let w = b.chip1("And", &[on, out(d, 1)]);
        b.output("write", w);
        assert!(matches!(
            check(level, &lib, &b.build()).unwrap(),
            Verdict::Fail(_)
        ));
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
