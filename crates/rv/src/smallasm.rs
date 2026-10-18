//! Assemblers for the 8-bit teaching CPU, 4004, and user-defined fixed-word
//! instruction encodings. A custom encoding never supplies CPU behaviour:
//! the learner still has to build a matching decoder and datapath.

use crate::asm::AsmError;
use makine_lang::{tr, trf};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operand {
    None,
    Value,
    Address,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub name: String,
    pub opcode: u16,
    pub operand: Operand,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub bits: u8,
    pub opcode_bits: u8,
    pub instructions: Vec<Instruction>,
}

impl Default for Definition {
    fn default() -> Self {
        Self {
            bits: 8,
            opcode_bits: 4,
            instructions: vec![
                Instruction {
                    name: "NOP".into(),
                    opcode: 0,
                    operand: Operand::None,
                },
                Instruction {
                    name: "YUKLE".into(),
                    opcode: 1,
                    operand: Operand::Value,
                },
                Instruction {
                    name: "TOPLA".into(),
                    opcode: 4,
                    operand: Operand::Address,
                },
                Instruction {
                    name: "ATLA".into(),
                    opcode: 6,
                    operand: Operand::Address,
                },
                Instruction {
                    name: "DUR".into(),
                    opcode: 15,
                    operand: Operand::None,
                },
            ],
        }
    }
}

impl Definition {
    pub fn validate(&self) -> Result<(), String> {
        if !(4..=16).contains(&self.bits) || self.opcode_bits == 0 || self.opcode_bits >= self.bits
        {
            return Err(tr!(
                "Kelime 4–16 bit; opcode alanı 1 bit ile kelime genişliği-1 arasında olmalı.",
                "Word width must be 4–16 bits; opcode width must be 1 through word width minus 1."
            )
            .into());
        }
        if self.instructions.is_empty() || self.instructions.len() > 256 {
            return Err(tr!("1–256 komut tanımla.", "Define 1–256 instructions.").into());
        }
        let mut names = std::collections::BTreeSet::new();
        let mut codes = std::collections::BTreeSet::new();
        for i in &self.instructions {
            if i.name.is_empty()
                || i.name.starts_with('.')
                || !i
                    .name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(trf!(
                    "Geçersiz komut adı: {}",
                    "Invalid instruction name: {}",
                    i.name
                ));
            }
            if i.opcode as u32 >= 1u32 << self.opcode_bits
                || !codes.insert(i.opcode)
                || !names.insert(i.name.to_ascii_uppercase())
            {
                return Err(tr!(
                    "Komut adları ve opcode'lar benzersiz ve alanlarına sığmalı.",
                    "Instruction names and opcodes must be unique and fit their fields."
                )
                .into());
            }
        }
        Ok(())
    }
    pub fn disasm(&self, word: u16) -> String {
        if self.validate().is_err() {
            return format!(".word {word}");
        }
        let shift = self.bits - self.opcode_bits;
        let op = word >> shift;
        let value = word & ((1u32 << shift) - 1) as u16;
        match self.instructions.iter().find(|i| i.opcode == op) {
            Some(i) if i.operand == Operand::None && value == 0 => i.name.clone(),
            Some(i) if i.operand != Operand::None => format!("{} {value}", i.name),
            _ => format!(".word {word}"),
        }
    }
}

pub enum Dialect<'a> {
    Mini,
    Mcs4,
    Custom(&'a Definition),
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    /// One entry per address: bytes for Mini/4004, fixed words for Custom.
    pub words: Vec<u16>,
    pub lines: BTreeMap<u32, usize>,
    pub symbols: BTreeMap<String, u32>,
    /// Initial data RAM, not part of the program ROM.
    pub ram: Vec<(usize, u8)>,
    pub lengths: BTreeMap<u32, usize>,
}

fn fail(line: usize, message: impl Into<String>) -> AsmError {
    AsmError {
        line,
        message: message.into(),
    }
}

fn number(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(s) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(s, 16).ok()
    } else if let Some(s) = s.strip_prefix("0b") {
        u32::from_str_radix(s, 2).ok()
    } else {
        s.parse().ok()
    }
}

fn value(s: &str, sym: &BTreeMap<String, u32>, line: usize) -> Result<u32, AsmError> {
    number(s).or_else(|| sym.get(s).copied()).ok_or_else(|| {
        fail(
            line,
            trf!(
                "Tanımsız sayı veya etiket: {s}",
                "Undefined number or label: {s}"
            ),
        )
    })
}

fn mini() -> Definition {
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
        .map(|(n, o, a)| Instruction {
            name: n.into(),
            opcode: o,
            operand: a,
        })
        .collect(),
    }
}

struct Line {
    n: usize,
    addr: u32,
    op: String,
    args: Vec<String>,
}

pub fn assemble(source: &str, dialect: Dialect<'_>) -> Result<Program, AsmError> {
    if source.len() > 1024 * 1024 {
        return Err(fail(1, tr!("Kaynak çok büyük.", "Source is too large.")));
    }
    if let Dialect::Custom(d) = &dialect {
        d.validate().map_err(|e| fail(1, e))?;
    }
    let limit = match dialect {
        Dialect::Mini => 16,
        _ => 4096,
    };
    let mut p = Program::default();
    let mut parsed = Vec::new();
    let mut pc = 0;
    for (n, raw) in source.lines().enumerate() {
        let n = n + 1;
        let mut s = raw.split(['#', ';']).next().unwrap_or("").trim();
        if let Some((label, rest)) = s.split_once(':') {
            let label = label.trim();
            if label.is_empty()
                || !label.chars().all(|c| c.is_alphanumeric() || c == '_')
                || p.symbols.insert(label.into(), pc).is_some()
            {
                return Err(fail(
                    n,
                    tr!(
                        "Etiket geçersiz veya iki kez tanımlı.",
                        "Invalid or duplicate label."
                    ),
                ));
            }
            s = rest.trim();
        }
        if s.is_empty() {
            continue;
        }
        let (op, args) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
        let op = op.to_ascii_uppercase();
        let args: Vec<String> = if args.trim().is_empty() {
            vec![]
        } else {
            args.split(',').map(|s| s.trim().to_string()).collect()
        };
        if op == ".EQU" {
            if args.len() != 2 {
                return Err(fail(n, ".equ name, value"));
            }
            let v = value(&args[1], &p.symbols, n)?;
            if p.symbols.insert(args[0].clone(), v).is_some() {
                return Err(fail(n, tr!("Sembol iki kez tanımlı.", "Duplicate symbol.")));
            }
            continue;
        }
        if op == ".ORG" {
            if args.len() != 1 {
                return Err(fail(n, ".org address"));
            }
            let next = value(&args[0], &p.symbols, n)?;
            if next < pc || next > limit {
                return Err(fail(
                    n,
                    tr!(
                        ".org ileriye ve ROM sınırları içinde olmalı.",
                        ".org must move forward within ROM bounds."
                    ),
                ));
            }
            pc = next;
            continue;
        }
        let size = match op.as_str() {
            ".RAM" => 0,
            ".BYTE" | ".WORD" => args.len() as u32,
            _ => {
                if matches!(dialect, Dialect::Mcs4)
                    && matches!(op.as_str(), "FIM" | "JUN" | "JMS" | "JCN" | "ISZ")
                {
                    2
                } else {
                    1
                }
            }
        };
        parsed.push(Line {
            n,
            addr: pc,
            op,
            args,
        });
        pc += size;
        if pc > limit {
            return Err(fail(
                n,
                trf!(
                    "Program {limit} adreslik ROM'a sığmıyor.",
                    "Program does not fit the {limit}-address ROM."
                ),
            ));
        }
    }
    p.words.resize(pc as usize, 0);
    for l in parsed {
        if l.op == ".RAM" {
            if !matches!(dialect, Dialect::Mini) || l.args.len() != 2 {
                return Err(fail(l.n, "Mini: .ram address, byte"));
            }
            let a = value(&l.args[0], &p.symbols, l.n)?;
            let v = value(&l.args[1], &p.symbols, l.n)?;
            if a >= 16 || v >= 256 {
                return Err(fail(
                    l.n,
                    tr!(
                        "RAM adresi 0–15, veri 0–255 olmalı.",
                        "RAM address must be 0–15, data 0–255."
                    ),
                ));
            }
            p.ram.push((a as usize, v as u8));
            continue;
        }
        let words = if matches!(l.op.as_str(), ".BYTE" | ".WORD") {
            if l.args.is_empty() {
                return Err(fail(l.n, tr!("Veri eksik.", "Missing data.")));
            }
            let max = match &dialect {
                Dialect::Custom(d) => (1u32 << d.bits) - 1,
                _ => 255,
            };
            l.args
                .iter()
                .map(|s| {
                    let v = value(s, &p.symbols, l.n)?;
                    if v > max {
                        Err(fail(
                            l.n,
                            tr!("Veri kelimeye sığmıyor.", "Data does not fit the word."),
                        ))
                    } else {
                        Ok(v as u16)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            match &dialect {
                Dialect::Mini => encode(&mini(), &l, &p.symbols)?,
                Dialect::Custom(d) => encode(d, &l, &p.symbols)?,
                Dialect::Mcs4 => mcs4(&l, &p.symbols)?,
            }
        };
        if !l.op.starts_with('.') {
            p.lengths.insert(l.addr, words.len());
        }
        for (k, w) in words.into_iter().enumerate() {
            p.words[l.addr as usize + k] = w;
            p.lines.insert(l.addr + k as u32, l.n);
        }
    }
    Ok(p)
}

fn encode(d: &Definition, l: &Line, sym: &BTreeMap<String, u32>) -> Result<Vec<u16>, AsmError> {
    let i = d
        .instructions
        .iter()
        .find(|i| i.name.eq_ignore_ascii_case(&l.op))
        .ok_or_else(|| {
            fail(
                l.n,
                trf!("Bilinmeyen komut: {}", "Unknown instruction: {}", l.op),
            )
        })?;
    let count = if i.operand == Operand::None { 0 } else { 1 };
    if l.args.len() != count {
        return Err(fail(
            l.n,
            trf!("{count} argüman gerekli.", "{count} operands required."),
        ));
    }
    let v = if count == 0 {
        0
    } else {
        value(&l.args[0], sym, l.n)?
    };
    let shift = d.bits - d.opcode_bits;
    if v >= 1u32 << shift {
        return Err(fail(
            l.n,
            trf!(
                "Değer {shift} bite sığmıyor.",
                "Value does not fit in {shift} bits."
            ),
        ));
    }
    Ok(vec![(i.opcode << shift) | v as u16])
}

fn mcs4(l: &Line, sym: &BTreeMap<String, u32>) -> Result<Vec<u16>, AsmError> {
    let n = l.n;
    let want = |k| {
        if l.args.len() == k {
            Ok(())
        } else {
            Err(fail(
                n,
                trf!("{k} argüman gerekli.", "{k} operands required."),
            ))
        }
    };
    let arg = |k: usize, max: u32| -> Result<u32, AsmError> {
        let v = value(&l.args[k], sym, n)?;
        if v <= max {
            Ok(v)
        } else {
            Err(fail(
                n,
                trf!("Değer 0–{max} arasında olmalı.", "Value must be 0–{max}."),
            ))
        }
    };
    let reg = |k: usize, pair: bool| -> Result<u32, AsmError> {
        let s = l.args[k].to_ascii_uppercase();
        let s = s.strip_prefix(if pair { "P" } else { "R" }).unwrap_or(&s);
        let v = value(s, sym, n)?;
        if v <= if pair { 7 } else { 15 } {
            Ok(v)
        } else {
            Err(fail(n, tr!("Geçersiz register.", "Invalid register.")))
        }
    };
    let single = [
        ("NOP", 0x00),
        ("WRM", 0xe0),
        ("WMP", 0xe1),
        ("WRR", 0xe2),
        ("WPM", 0xe3),
        ("WR0", 0xe4),
        ("WR1", 0xe5),
        ("WR2", 0xe6),
        ("WR3", 0xe7),
        ("SBM", 0xe8),
        ("RDM", 0xe9),
        ("RDR", 0xea),
        ("ADM", 0xeb),
        ("RD0", 0xec),
        ("RD1", 0xed),
        ("RD2", 0xee),
        ("RD3", 0xef),
        ("CLB", 0xf0),
        ("CLC", 0xf1),
        ("IAC", 0xf2),
        ("CMC", 0xf3),
        ("CMA", 0xf4),
        ("RAL", 0xf5),
        ("RAR", 0xf6),
        ("TCC", 0xf7),
        ("DAC", 0xf8),
        ("TCS", 0xf9),
        ("STC", 0xfa),
        ("DAA", 0xfb),
        ("KBP", 0xfc),
        ("DCL", 0xfd),
    ];
    if let Some((_, v)) = single.iter().find(|(s, _)| *s == l.op) {
        want(0)?;
        return Ok(vec![*v]);
    }
    let v = match l.op.as_str() {
        "JUN" | "JMS" => {
            want(1)?;
            let a = arg(0, 4095)?;
            vec![
                (if l.op == "JUN" { 0x40 } else { 0x50 }) | (a >> 8) as u16,
                (a & 255) as u16,
            ]
        }
        "FIM" => {
            want(2)?;
            vec![0x20 | (reg(0, true)? << 1) as u16, arg(1, 255)? as u16]
        }
        "SRC" | "FIN" | "JIN" => {
            want(1)?;
            let base = match l.op.as_str() {
                "SRC" => 0x21,
                "FIN" => 0x30,
                _ => 0x31,
            };
            vec![base | (reg(0, true)? << 1) as u16]
        }
        "JCN" | "ISZ" => {
            want(2)?;
            let a = arg(1, 4095)?;
            if a & 0xf00 != (l.addr + 2) & 0xf00 {
                return Err(fail(
                    n,
                    tr!(
                        "Hedef, sonraki komutla aynı 256-baytlık sayfada olmalı.",
                        "Target must be in the same 256-byte page as the following instruction."
                    ),
                ));
            }
            let lo = if l.op == "JCN" {
                arg(0, 15)?
            } else {
                reg(0, false)?
            };
            vec![
                (if l.op == "JCN" { 0x10 } else { 0x70 }) | lo as u16,
                (a & 255) as u16,
            ]
        }
        "INC" | "ADD" | "SUB" | "LD" | "XCH" => {
            want(1)?;
            let base = match l.op.as_str() {
                "INC" => 0x60,
                "ADD" => 0x80,
                "SUB" => 0x90,
                "LD" => 0xa0,
                _ => 0xb0,
            };
            vec![base | reg(0, false)? as u16]
        }
        "LDM" | "BBL" => {
            want(1)?;
            vec![(if l.op == "LDM" { 0xd0 } else { 0xc0 }) | arg(0, 15)? as u16]
        }
        _ => {
            return Err(fail(
                n,
                trf!(
                    "Bilinmeyen 4004 komutu: {}",
                    "Unknown 4004 instruction: {}",
                    l.op
                ),
            ))
        }
    };
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mini_labels_data_and_bounds() {
        let p = assemble(
            "LDI 5\nloop: ST 14\nSUB 15\nJZ done\nJMP loop\ndone: HLT\n.ram 15,1",
            Dialect::Mini,
        )
        .unwrap();
        assert_eq!(p.words, vec![0x15, 0x3e, 0x5f, 0x75, 0x61, 0xf0]);
        assert_eq!(p.ram, vec![(15, 1)]);
        for s in ["LDI 16", "LD 0,1", "JMP missing", ".org 17", ".ram 16,0"] {
            assert!(assemble(s, Dialect::Mini).is_err(), "{s}");
        }
    }
    #[test]
    fn real_4004_program_and_page_validation() {
        let p = assemble(
            "FIM P0,0\nloop: SRC P0\nRDM\nIAC\nWRM\nRDR\nCLC\nADM\nWMP\nJUN loop",
            Dialect::Mcs4,
        )
        .unwrap();
        assert_eq!(
            p.words,
            vec![0x20, 0, 0x21, 0xe9, 0xf2, 0xe0, 0xea, 0xf1, 0xeb, 0xe1, 0x40, 2]
        );
        assert!(assemble("JCN 4,256", Dialect::Mcs4).is_err());
        assert_eq!(
            assemble(".org 254\nJCN 4,256", Dialect::Mcs4)
                .unwrap()
                .words[254..],
            [0x14, 0]
        );
    }
    #[test]
    fn custom_encoding_does_not_define_behaviour() {
        let mut d = Definition::default();
        d.validate().unwrap();
        let p = assemble("YUKLE 5\nTOPLA 2\nATLA end\nend: DUR", Dialect::Custom(&d)).unwrap();
        assert_eq!(p.words, vec![0x15, 0x42, 0x63, 0xf0]);
        assert_eq!(d.disasm(0x42), "TOPLA 2");
        d.instructions[1].opcode = 0;
        assert!(assemble("NOP", Dialect::Custom(&d)).is_err());
    }
}
