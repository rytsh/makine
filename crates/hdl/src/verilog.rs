//! Writing chips as Verilog modules.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use makine_lang::trf;
use makine_sim::{mask, ChipDef, Library, NativeDef, PartKind, Pin, Sink, Source};

use crate::ident::Names;
use crate::Options;

/// The storage chips of the learning path that can be written as real
/// latches and flip-flops, with the pins they must have.
const PRIMITIVES: &[(&str, &[&str], &[&str])] = &[
    ("Latch", &["set", "reset"], &["q"]),
    ("DLatch", &["d", "en"], &["q"]),
    ("DFF", &["d", "clk"], &["q"]),
];

/// The storage chip `def` stands for, if it has its pins.
pub(crate) fn primitive(def: &ChipDef) -> Option<&'static str> {
    let names = |pins: &[Pin]| -> Vec<(String, u8)> {
        pins.iter().map(|p| (p.name.clone(), p.width)).collect()
    };
    let want = |specs: &[&str]| -> Vec<(String, u8)> {
        specs.iter().map(|s| (s.to_string(), 1)).collect()
    };
    PRIMITIVES
        .iter()
        .find(|(name, ins, outs)| {
            def.name == *name
                && names(&def.inputs) == want(ins)
                && names(&def.outputs) == want(outs)
        })
        .map(|(name, _, _)| *name)
}

/// A fast model of a storage chip, behaving like the Verilog written for it
/// (power-on value 0), for simulating the exported design in makine.
pub(crate) fn primitive_model(name: &str) -> NativeDef {
    let pins = PRIMITIVES.iter().find(|p| p.0 == name).expect("primitive");
    let (ins, outs) = (pins.1, pins.2);
    match name {
        "Latch" => NativeDef::level(
            ins,
            outs,
            0u64,
            |q, i, o| o[0] = latch(*q, i),
            |q, i| *q = latch(*q, i),
        ),
        "DLatch" => NativeDef::level(
            ins,
            outs,
            0u64,
            |q, i, o| o[0] = if i[1] & 1 == 1 { i[0] & 1 } else { *q },
            |q, i| {
                if i[1] & 1 == 1 {
                    *q = i[0] & 1
                }
            },
        ),
        _ => NativeDef::clocked(
            ins,
            outs,
            &[],
            0u64,
            |q, i| *q = i[0] & 1,
            |q, _, o| o[0] = *q,
        ),
    }
}

fn latch(q: u64, i: &[u64]) -> u64 {
    if i[0] & 1 == 1 {
        1
    } else if i[1] & 1 == 1 {
        0
    } else {
        q
    }
}

/// `lib`'s chips, simulated by their gates, with storage chips replaced by
/// models of the Verilog written for them when `opts` asks for that. The
/// exported design behaves like a circuit compiled against this library.
pub fn gate_library(lib: &Library, opts: &Options) -> Library {
    let mut out = Library::new();
    for def in lib.chips() {
        if opts.primitives {
            if let Some(p) = primitive(def) {
                out.insert_native(def.name.clone(), primitive_model(p));
                out.set_fast(&def.name, true);
            }
        }
        out.insert(def.clone());
    }
    out
}

/// A chip written as Verilog.
#[derive(Clone, Debug)]
pub struct Export {
    /// Every module, the top one last.
    pub source: String,
    /// Name of the top module.
    pub top: String,
    /// Verilog port names of the top module, in the order of the chip's
    /// pins.
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    /// Number of modules written.
    pub modules: usize,
    /// Storage chips written as latches or flip-flops.
    pub primitives: Vec<String>,
}

/// One module to write: a chip with its Verilog names.
struct Module<'a> {
    def: &'a ChipDef,
    name: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    primitive: Option<&'static str>,
}

/// `top` and every chip it uses, as Verilog. Sub-chips come from `lib`.
pub fn verilog(lib: &Library, top: &ChipDef, opts: &Options) -> Result<Export, String> {
    // Chips in dependency order, users after the chips they use.
    let mut order: Vec<&ChipDef> = Vec::new();
    let mut done = BTreeSet::new();
    visit(lib, top, opts, &mut Vec::new(), &mut done, &mut order)?;

    let mut module_names = Names::default();
    let mut modules: BTreeMap<String, Module> = BTreeMap::new();
    for def in &order {
        let mut ports = Names::default();
        let inputs = def.inputs.iter().map(|p| ports.fresh(&p.name)).collect();
        let outputs = def.outputs.iter().map(|p| ports.fresh(&p.name)).collect();
        modules.insert(
            def.name.clone(),
            Module {
                def,
                name: module_names.fresh(&format!("{}{}", opts.prefix, def.name)),
                inputs,
                outputs,
                primitive: opts.primitives.then(|| primitive(def)).flatten(),
            },
        );
    }

    let mut source = String::new();
    let mut primitives = Vec::new();
    for def in &order {
        let m = &modules[&def.name];
        match m.primitive {
            Some(p) => {
                primitives.push(def.name.clone());
                write_primitive(&mut source, m, p);
            }
            None => write_module(&mut source, m, &modules, lib)?,
        }
        source.push('\n');
    }
    let top_module = &modules[&top.name];
    Ok(Export {
        top: top_module.name.clone(),
        inputs: top_module.inputs.clone(),
        outputs: top_module.outputs.clone(),
        modules: order.len(),
        primitives,
        source,
    })
}

fn visit<'a>(
    lib: &'a Library,
    def: &'a ChipDef,
    opts: &Options,
    stack: &mut Vec<String>,
    done: &mut BTreeSet<String>,
    order: &mut Vec<&'a ChipDef>,
) -> Result<(), String> {
    if done.contains(&def.name) {
        return Ok(());
    }
    if stack.contains(&def.name) {
        return Err(trf!(
            "{} kendi kendini içeriyor",
            "{} contains itself",
            def.name
        ));
    }
    stack.push(def.name.clone());
    if !(opts.primitives && primitive(def).is_some()) {
        for part in &def.parts {
            let PartKind::Chip(name) = &part.kind else {
                continue;
            };
            let Some(sub) = lib.get(name) else {
                return Err(if lib.native(name).is_some() {
                    trf!(
                        "{name} Rust'la yazılmış bir parça (bellek, cihaz); kapılardan oluşmadığı için Verilog'a çevrilemez",
                        "{name} is a part written in Rust (memory, device); it is not made of gates, so it cannot become Verilog"
                    )
                } else {
                    trf!("bilinmeyen çip: {name}", "unknown chip: {name}")
                });
            };
            visit(lib, sub, opts, stack, done, order)?;
        }
    }
    stack.pop();
    done.insert(def.name.clone());
    order.push(def);
    Ok(())
}

/// `[w-1:0] ` for a bus, nothing for one bit.
fn range(width: u32) -> String {
    if width == 1 {
        String::new()
    } else {
        format!("[{}:0] ", width - 1)
    }
}

/// Bits `lo..lo+w` of a net named `name` that is `total` bits wide.
fn slice(name: &str, total: u32, lo: u32, w: u32) -> String {
    if lo == 0 && w == total {
        name.to_string()
    } else if w == 1 {
        format!("{name}[{lo}]")
    } else {
        format!("{name}[{}:{lo}]", lo + w - 1)
    }
}

fn literal(width: u32, value: u64) -> String {
    format!("{width}'h{:X}", value & mask(width as u8))
}

fn ports(m: &Module) -> String {
    let mut lines = Vec::new();
    for (p, n) in m.def.inputs.iter().zip(&m.inputs) {
        lines.push(format!("    input  wire {}{n}", range(p.width as u32)));
    }
    for (p, n) in m.def.outputs.iter().zip(&m.outputs) {
        lines.push(format!("    output wire {}{n}", range(p.width as u32)));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", lines.join(",\n"))
    }
}

fn write_primitive(out: &mut String, m: &Module, kind: &str) {
    let (i, o) = (&m.inputs, &m.outputs[0]);
    let _ = writeln!(
        out,
        "// {}: {}",
        m.def.name,
        match kind {
            "Latch" => "SR latch (set wins), power-on 0",
            "DLatch" => "D latch, transparent while en = 1, power-on 0",
            _ => "D flip-flop, rising edge of clk, power-on 0",
        }
    );
    let _ = writeln!(out, "module {}({});", m.name, ports(m));
    let _ = writeln!(out, "    reg state = 1'b0;");
    let _ = writeln!(out, "    assign {o} = state;");
    match kind {
        "Latch" => {
            let _ = writeln!(out, "    always @*");
            let _ = writeln!(out, "        if ({}) state = 1'b1;", i[0]);
            let _ = writeln!(out, "        else if ({}) state = 1'b0;", i[1]);
        }
        "DLatch" => {
            let _ = writeln!(out, "    always @*");
            let _ = writeln!(out, "        if ({}) state = {};", i[1], i[0]);
        }
        _ => {
            let _ = writeln!(out, "    always @(posedge {})", i[1]);
            let _ = writeln!(out, "        state <= {};", i[0]);
        }
    }
    let _ = writeln!(out, "endmodule");
}

fn write_module(
    out: &mut String,
    m: &Module,
    modules: &BTreeMap<String, Module>,
    lib: &Library,
) -> Result<(), String> {
    let def = m.def;
    let mut names = Names::default();
    for n in m.inputs.iter().chain(&m.outputs) {
        names.fresh(n);
    }
    let pins: Vec<_> = def
        .parts
        .iter()
        .map(|p| {
            lib.part_pins(p)
                .ok_or_else(|| trf!("bilinmeyen parça: {}", "unknown part: {}", p.kind.name()))
        })
        .collect::<Result<_, _>>()?;
    // A wire for every pin of every part.
    let part_in: Vec<Vec<String>> = pins
        .iter()
        .enumerate()
        .map(|(i, p)| {
            p.inputs
                .iter()
                .map(|pin| names.fresh(&format!("p{i}_{}", pin.name)))
                .collect()
        })
        .collect();
    let part_out: Vec<Vec<String>> = pins
        .iter()
        .enumerate()
        .map(|(i, p)| {
            p.outputs
                .iter()
                .map(|pin| names.fresh(&format!("p{i}_{}", pin.name)))
                .collect()
        })
        .collect();

    let _ = writeln!(out, "// {}", def.name);
    let _ = writeln!(out, "module {}({});", m.name, ports(m));
    for (i, p) in pins.iter().enumerate() {
        for (pin, n) in p
            .inputs
            .iter()
            .chain(&p.outputs)
            .zip(part_in[i].iter().chain(&part_out[i]))
        {
            let _ = writeln!(out, "    wire {}{n};", range(pin.width as u32));
        }
    }
    if !def.parts.is_empty() {
        out.push('\n');
    }

    // Parts.
    for (i, part) in def.parts.iter().enumerate() {
        let copies = part.repeat.max(1) as u32;
        let (ins, outs) = (&part_in[i], &part_out[i]);
        let (in_w, out_w): (Vec<u32>, Vec<u32>) = (
            pins[i].inputs.iter().map(|p| p.width as u32).collect(),
            pins[i].outputs.iter().map(|p| p.width as u32).collect(),
        );
        match &part.kind {
            PartKind::Nand => {
                let _ = writeln!(out, "    assign {} = ~({} & {});", outs[0], ins[0], ins[1]);
            }
            PartKind::Const { width, value } => {
                let lit = literal(*width as u32, *value);
                let v = if copies == 1 {
                    lit
                } else {
                    format!("{{{copies}{{{lit}}}}}")
                };
                let _ = writeln!(out, "    assign {} = {v};", outs[0]);
            }
            PartKind::Split(widths) | PartKind::Join(widths) => {
                let split = matches!(part.kind, PartKind::Split(_));
                let sum: u32 = widths.iter().map(|&w| w as u32).sum();
                for k in 0..copies {
                    let mut off = 0;
                    for (g, &w) in widths.iter().enumerate() {
                        let w = w as u32;
                        let bus = slice(
                            if split { &ins[0] } else { &outs[0] },
                            sum * copies,
                            k * sum + off,
                            w,
                        );
                        let group =
                            slice(if split { &outs[g] } else { &ins[g] }, w * copies, k * w, w);
                        let (to, from) = if split { (group, bus) } else { (bus, group) };
                        let _ = writeln!(out, "    assign {to} = {from};");
                        off += w;
                    }
                }
            }
            PartKind::Chip(name) => {
                let sub = &modules[name];
                for k in 0..copies {
                    let inst = names.fresh(&if copies == 1 {
                        format!("u{i}")
                    } else {
                        format!("u{i}_{k}")
                    });
                    let mut conns = Vec::new();
                    for (j, port) in sub.inputs.iter().enumerate() {
                        let w = in_w[j] / copies;
                        conns.push(format!(".{port}({})", slice(&ins[j], in_w[j], k * w, w)));
                    }
                    for (j, port) in sub.outputs.iter().enumerate() {
                        let w = out_w[j] / copies;
                        conns.push(format!(".{port}({})", slice(&outs[j], out_w[j], k * w, w)));
                    }
                    let _ = writeln!(
                        out,
                        "    {} {inst} (\n        {}\n    );",
                        sub.name,
                        conns.join(",\n        ")
                    );
                }
            }
        }
    }

    // Wires. A 1-bit source drives every line of a wider sink; a sink with
    // no wire is 0, as in the simulator.
    let mut driven = BTreeSet::new();
    let mut assigns = Vec::new();
    for w in &def.wires {
        let (from, from_w) = match w.from {
            Source::Input(i) => (&m.inputs[i], def.inputs[i].width as u32),
            Source::Part { part, pin } => {
                (&part_out[part][pin], pins[part].outputs[pin].width as u32)
            }
        };
        let (to, to_w) = sink(m, &part_in, &pins, w.to);
        driven.insert(w.to);
        let value = if from_w == 1 && to_w > 1 {
            format!("{{{to_w}{{{from}}}}}")
        } else {
            from.clone()
        };
        assigns.push(format!("    assign {to} = {value};"));
    }
    let sinks = (0..def.outputs.len()).map(Sink::Output).chain(
        pins.iter()
            .enumerate()
            .flat_map(|(part, p)| (0..p.inputs.len()).map(move |pin| Sink::Part { part, pin })),
    );
    for s in sinks.filter(|s| !driven.contains(s)) {
        let (to, w) = sink(m, &part_in, &pins, s);
        assigns.push(format!("    assign {to} = {};", literal(w, 0)));
    }
    if !assigns.is_empty() {
        out.push('\n');
        for a in assigns {
            out.push_str(&a);
            out.push('\n');
        }
    }
    let _ = writeln!(out, "endmodule");
    Ok(())
}

fn sink<'a>(
    m: &'a Module,
    part_in: &'a [Vec<String>],
    pins: &[makine_sim::PartPins],
    s: Sink,
) -> (&'a String, u32) {
    match s {
        Sink::Output(i) => (&m.outputs[i], m.def.outputs[i].width as u32),
        Sink::Part { part, pin } => (&part_in[part][pin], pins[part].inputs[pin].width as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_sim::ChipBuilder;

    #[test]
    fn writes_hierarchy() {
        let mut lib = Library::new();
        let mut b = ChipBuilder::new("Not", &["in"], &["out"]);
        let x = b.input("in");
        let n = b.nand(x, x);
        b.output("out", n);
        lib.insert(b.build());

        let mut b = ChipBuilder::new("Not8", &["in[8]"], &["out[8]"]);
        let x = b.input("in");
        let n = b.chip_n("Not", 8, &[x]);
        b.output("out", makine_sim::out(n, 0));
        let top = b.build();

        let e = verilog(&lib, &top, &Options::default()).unwrap();
        assert_eq!(e.top, "mk_Not8");
        assert_eq!(e.modules, 2);
        assert!(e.source.contains("module mk_Not("));
        assert!(e.source.contains("assign p0_out = ~(p0_a & p0_b);"));
        assert!(e.source.contains("mk_Not u0_7 ("));
        assert!(e.source.contains(".in(p0_in[7])"));
    }

    #[test]
    fn natives_are_refused() {
        let mut lib = Library::new();
        lib.insert_native("Ram", primitive_model("DFF"));
        let mut b = ChipBuilder::new("Top", &["d", "clk"], &["q"]);
        let ins = [b.input("d"), b.input("clk")];
        let r = b.chip1("Ram", &ins);
        b.output("q", r);
        assert!(verilog(&lib, &b.build(), &Options::default()).is_err());
    }
}
