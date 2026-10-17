//! A small two-pass RV32IM assembler.
//!
//! Supports every RV32IM instruction, labels, `%hi(sym)`/`%lo(sym)`, common
//! pseudo-instructions (`li la mv not neg j jr call ret nop beqz bnez blez
//! bgez bltz bgtz bgt ble bgtu bleu seqz snez halt`) and the directives
//! `.org .align .word .half .byte .ascii .asciz .string .space .zero
//! .equ .set .text .data .globl`.
//!
//! `halt` assembles to `ebreak`, which stops the machine.

use std::collections::HashMap;
use std::fmt;

use crate::isa::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsmError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "satır {}: {}", self.line, self.message)
    }
}

impl std::error::Error for AsmError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    /// Memory image starting at address 0.
    pub bytes: Vec<u8>,
    pub symbols: HashMap<String, u32>,
    /// Source line of each instruction, by address.
    pub lines: HashMap<u32, usize>,
}

impl Program {
    pub fn words(&self) -> Vec<u32> {
        self.bytes
            .chunks(4)
            .map(|c| {
                let mut w = [0u8; 4];
                w[..c.len()].copy_from_slice(c);
                u32::from_le_bytes(w)
            })
            .collect()
    }
}

pub fn assemble(src: &str) -> Result<Program, AsmError> {
    let lines: Vec<Line> = src
        .lines()
        .enumerate()
        .map(|(n, l)| parse_line(n + 1, l))
        .collect::<Result<_, _>>()?;

    // Pass 1: addresses of labels. Instruction sizes must not depend on
    // label values, so `li` with a label operand is rejected (use `la`).
    let mut symbols: HashMap<String, u32> = HashMap::new();
    let mut pc = 0u32;
    for l in &lines {
        for label in &l.labels {
            if symbols.insert(label.clone(), pc).is_some() {
                return Err(err(l.n, format!("'{label}' etiketi iki kez tanımlanmış")));
            }
        }
        if let Some((name, args)) = &l.op {
            if matches!(name.as_str(), ".equ" | ".set") {
                let [sym, val] = args.as_slice() else {
                    return Err(err(l.n, ".equ isim, değer".into()));
                };
                let v = eval(val, &symbols, l.n)?;
                symbols.insert(sym.clone(), v);
                continue;
            }
            pc = advance(pc, name, args, l.n, &symbols)?;
        }
    }

    // Pass 2: emit.
    let mut out = Program {
        symbols: symbols.clone(),
        ..Default::default()
    };
    let mut pc = 0u32;
    for l in &lines {
        let Some((name, args)) = &l.op else { continue };
        if matches!(name.as_str(), ".equ" | ".set") {
            continue;
        }
        let a = Asm {
            args,
            n: l.n,
            pc,
            sym: &symbols,
        };
        let bytes = a.emit(name)?;
        if !name.starts_with('.') {
            out.lines.insert(pc, l.n);
        }
        let next = advance(pc, name, args, l.n, &symbols)?;
        let start = pc as usize;
        if out.bytes.len() < next as usize {
            out.bytes.resize(next as usize, 0);
        }
        out.bytes[start..start + bytes.len()].copy_from_slice(&bytes);
        pc = next;
    }
    Ok(out)
}

struct Line {
    n: usize,
    labels: Vec<String>,
    op: Option<(String, Vec<String>)>,
}

fn err(n: usize, message: String) -> AsmError {
    AsmError { line: n, message }
}

fn parse_line(n: usize, raw: &str) -> Result<Line, AsmError> {
    let mut s = strip_comment(raw).trim();
    let mut labels = Vec::new();
    while let Some(colon) = s.find(':') {
        let (head, rest) = s.split_at(colon);
        let head = head.trim();
        if head.is_empty()
            || !head
                .chars()
                .all(|c| c.is_alphanumeric() || "_.$".contains(c))
        {
            break;
        }
        labels.push(head.to_string());
        s = rest[1..].trim();
    }
    if s.is_empty() {
        return Ok(Line {
            n,
            labels,
            op: None,
        });
    }
    let (name, rest) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
    let args = split_args(rest.trim());
    Ok(Line {
        n,
        labels,
        op: Some((name.to_lowercase(), args)),
    })
}

fn strip_comment(s: &str) -> &str {
    let mut in_str = false;
    let mut prev = ' ';
    for (i, c) in s.char_indices() {
        match c {
            '"' if prev != '\\' => in_str = !in_str,
            '#' | ';' if !in_str => return &s[..i],
            '/' if !in_str && s[i..].starts_with("//") => return &s[..i],
            _ => {}
        }
        prev = c;
    }
    s
}

fn split_args(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut prev = ' ';
    for c in s.chars() {
        match c {
            '"' if prev != '\\' => {
                in_str = !in_str;
                cur.push(c);
            }
            ',' if !in_str => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
        prev = c;
    }
    out.push(cur.trim().to_string());
    out
}

/// Size in bytes of a statement, and thus the next address.
fn advance(
    pc: u32,
    name: &str,
    args: &[String],
    n: usize,
    sym: &HashMap<String, u32>,
) -> Result<u32, AsmError> {
    let count = |per: u32| per * args.len() as u32;
    Ok(match name {
        ".text" | ".data" | ".globl" | ".global" | ".section" | ".type" | ".size" => pc,
        ".org" => {
            let target = eval(one(args, n)?, sym, n)?;
            if target < pc {
                return Err(err(n, ".org geriye gidemez".into()));
            }
            target
        }
        ".align" | ".p2align" => {
            let a = 1u32 << eval(one(args, n)?, sym, n)?;
            pc.div_ceil(a) * a
        }
        ".balign" => {
            let a = eval(one(args, n)?, sym, n)?.max(1);
            pc.div_ceil(a) * a
        }
        ".word" => pc + count(4),
        ".half" => pc + count(2),
        ".byte" => pc + count(1),
        ".ascii" | ".asciz" | ".string" => {
            let s = parse_string(one(args, n)?, n)?;
            pc + s.len() as u32 + (name != ".ascii") as u32
        }
        ".space" | ".zero" => pc + eval(one(args, n)?, sym, n)?,
        "li" => {
            let [_, v] = two(args, n)?;
            match parse_num(v) {
                Some(v) if fits12(v) => pc + 4,
                Some(_) => pc + 8,
                None => return Err(err(n, "li sadece sayı alır; etiket için la kullan".into())),
            }
        }
        "la" | "call" | "tail" => pc + 8,
        _ if name.starts_with('.') => return Err(err(n, format!("bilinmeyen direktif {name}"))),
        _ => pc + 4,
    })
}

fn one(args: &[String], n: usize) -> Result<&str, AsmError> {
    match args {
        [a] => Ok(a),
        _ => Err(err(
            n,
            format!("1 argüman bekleniyordu, {} verildi", args.len()),
        )),
    }
}

fn two(args: &[String], n: usize) -> Result<[&str; 2], AsmError> {
    match args {
        [a, b] => Ok([a, b]),
        _ => Err(err(
            n,
            format!("2 argüman bekleniyordu, {} verildi", args.len()),
        )),
    }
}

fn fits12(v: u32) -> bool {
    let s = v as i32;
    (-2048..2048).contains(&s)
}

fn parse_num(s: &str) -> Option<u32> {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(&h.replace('_', ""), 16).ok()?
    } else if let Some(b) = s.strip_prefix("0b") {
        u64::from_str_radix(&b.replace('_', ""), 2).ok()?
    } else if s.len() == 3 && s.starts_with('\'') && s.ends_with('\'') {
        s.chars().nth(1)? as u64
    } else {
        s.replace('_', "").parse::<u64>().ok()?
    };
    if v > u32::MAX as u64 {
        return None;
    }
    Some(if neg {
        (v as u32).wrapping_neg()
    } else {
        v as u32
    })
}

/// A number, a symbol, or `sym+off` / `sym-off`.
fn eval(s: &str, sym: &HashMap<String, u32>, n: usize) -> Result<u32, AsmError> {
    let s = s.trim();
    if let Some(v) = parse_num(s) {
        return Ok(v);
    }
    if let Some(i) = s[1..].find(['+', '-']).map(|i| i + 1) {
        let (l, r) = s.split_at(i);
        let lv = eval(l, sym, n)?;
        let rv = eval(&r[1..], sym, n)?;
        return Ok(if r.starts_with('+') {
            lv.wrapping_add(rv)
        } else {
            lv.wrapping_sub(rv)
        });
    }
    sym.get(s)
        .copied()
        .ok_or_else(|| err(n, format!("tanımsız sembol '{s}'")))
}

fn parse_string(s: &str, n: usize) -> Result<Vec<u8>, AsmError> {
    let inner = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or_else(|| err(n, "tırnak içinde metin bekleniyordu".into()))?;
    let mut out = Vec::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        out.push(match chars.next() {
            Some('n') => b'\n',
            Some('t') => b'\t',
            Some('r') => b'\r',
            Some('0') => 0,
            Some('\\') => b'\\',
            Some('"') => b'"',
            other => return Err(err(n, format!("bilinmeyen kaçış dizisi \\{other:?}"))),
        });
    }
    Ok(out)
}

/// Split a 32-bit value for `lui`+`addi`: `hi` is rounded so that adding the
/// sign-extended `lo` gives back the value.
pub fn hi_lo(v: u32) -> (u32, u32) {
    let lo = v & 0xFFF;
    let lo = if lo >= 0x800 { lo | 0xFFFF_F000 } else { lo };
    (v.wrapping_sub(lo) & 0xFFFF_F000, lo)
}

struct Asm<'a> {
    args: &'a [String],
    n: usize,
    pc: u32,
    sym: &'a HashMap<String, u32>,
}

impl Asm<'_> {
    fn err(&self, m: impl Into<String>) -> AsmError {
        err(self.n, m.into())
    }

    fn want(&self, k: usize) -> Result<(), AsmError> {
        if self.args.len() != k {
            return Err(self.err(format!(
                "{k} argüman bekleniyordu, {} verildi",
                self.args.len()
            )));
        }
        Ok(())
    }

    fn reg(&self, i: usize) -> Result<u32, AsmError> {
        let s = &self.args[i];
        parse_reg(s).ok_or_else(|| self.err(format!("'{s}' bir register değil")))
    }

    fn imm(&self, i: usize) -> Result<u32, AsmError> {
        let s = self.args[i].trim();
        if let Some(inner) = s.strip_prefix("%hi(").and_then(|r| r.strip_suffix(')')) {
            return Ok(hi_lo(eval(inner, self.sym, self.n)?).0 >> 12);
        }
        if let Some(inner) = s.strip_prefix("%lo(").and_then(|r| r.strip_suffix(')')) {
            return Ok(hi_lo(eval(inner, self.sym, self.n)?).1);
        }
        eval(s, self.sym, self.n)
    }

    fn imm12(&self, i: usize) -> Result<u32, AsmError> {
        let v = self.imm(i)?;
        if !fits12(v) {
            return Err(self.err(format!("{} 12 bite sığmıyor (-2048..2047)", v as i32)));
        }
        Ok(v)
    }

    /// `off(reg)` operand.
    fn mem(&self, i: usize) -> Result<(u32, u32), AsmError> {
        let s = self.args[i].trim();
        let (off, rest) = s
            .split_once('(')
            .ok_or_else(|| self.err(format!("'{s}' bellek adresi değil; örn. 8(sp)")))?;
        let reg = rest
            .strip_suffix(')')
            .ok_or_else(|| self.err("')' eksik"))?
            .trim();
        let r = parse_reg(reg).ok_or_else(|| self.err(format!("'{reg}' bir register değil")))?;
        let off = if off.trim().is_empty() {
            0
        } else {
            let off = [off.to_string()];
            let sub = Asm {
                args: &off,
                ..*self
            };
            sub.imm12(0)?
        };
        Ok((off, r))
    }

    /// PC-relative offset to a label or number.
    fn rel(&self, i: usize, bits: u32) -> Result<u32, AsmError> {
        let target = eval(&self.args[i], self.sym, self.n)?;
        let off = target.wrapping_sub(self.pc);
        let s = off as i32;
        let lim = 1i32 << (bits - 1);
        if s < -lim || s >= lim || s % 2 != 0 {
            return Err(self.err(format!("hedef çok uzak ({s} bayt)")));
        }
        Ok(off)
    }

    fn emit(&self, name: &str) -> Result<Vec<u8>, AsmError> {
        let words = |ws: &[u32]| ws.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>();
        let w = |x: u32| Ok(words(&[x]));
        let a = self.args;

        // Directives.
        match name {
            ".text" | ".data" | ".globl" | ".global" | ".section" | ".type" | ".size" => {
                return Ok(vec![])
            }
            ".org" | ".align" | ".p2align" | ".balign" | ".space" | ".zero" => return Ok(vec![]),
            ".word" => {
                let vs = a
                    .iter()
                    .map(|s| eval(s, self.sym, self.n))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(words(&vs));
            }
            ".half" => {
                let mut out = vec![];
                for s in a {
                    out.extend_from_slice(&(eval(s, self.sym, self.n)? as u16).to_le_bytes());
                }
                return Ok(out);
            }
            ".byte" => {
                return a
                    .iter()
                    .map(|s| Ok(eval(s, self.sym, self.n)? as u8))
                    .collect();
            }
            ".ascii" | ".asciz" | ".string" => {
                let mut s = parse_string(&a[0], self.n)?;
                if name != ".ascii" {
                    s.push(0);
                }
                return Ok(s);
            }
            _ => {}
        }

        let r3 = |f3: u32, f7: u32| -> Result<Vec<u8>, AsmError> {
            self.want(3)?;
            w(enc_r(
                OP_REG,
                self.reg(0)?,
                f3,
                self.reg(1)?,
                self.reg(2)?,
                f7,
            ))
        };
        let i3 = |f3: u32| -> Result<Vec<u8>, AsmError> {
            self.want(3)?;
            w(enc_i(
                OP_IMM,
                self.reg(0)?,
                f3,
                self.reg(1)?,
                self.imm12(2)?,
            ))
        };
        let sh = |f3: u32, f7: u32| -> Result<Vec<u8>, AsmError> {
            self.want(3)?;
            let s = self.imm(2)?;
            if s > 31 {
                return Err(self.err("kaydırma miktarı 0..31 olmalı"));
            }
            w(enc_i(OP_IMM, self.reg(0)?, f3, self.reg(1)?, s | f7 << 5))
        };
        let ld = |f3: u32| -> Result<Vec<u8>, AsmError> {
            self.want(2)?;
            let (off, r) = self.mem(1)?;
            w(enc_i(OP_LOAD, self.reg(0)?, f3, r, off))
        };
        let st = |f3: u32| -> Result<Vec<u8>, AsmError> {
            self.want(2)?;
            let (off, r) = self.mem(1)?;
            w(enc_s(OP_STORE, f3, r, self.reg(0)?, off))
        };
        let br = |f3: u32, swap: bool| -> Result<Vec<u8>, AsmError> {
            self.want(3)?;
            let (x, y) = (self.reg(0)?, self.reg(1)?);
            let (x, y) = if swap { (y, x) } else { (x, y) };
            w(enc_b(OP_BRANCH, f3, x, y, self.rel(2, 13)?))
        };
        let brz = |f3: u32, zero_first: bool| -> Result<Vec<u8>, AsmError> {
            self.want(2)?;
            let r = self.reg(0)?;
            let (x, y) = if zero_first { (0, r) } else { (r, 0) };
            w(enc_b(OP_BRANCH, f3, x, y, self.rel(1, 13)?))
        };
        let hi_lo_pair = |rd: u32, v: u32| -> Vec<u8> {
            let (hi, lo) = hi_lo(v);
            words(&[enc_u(OP_LUI, rd, hi), enc_i(OP_IMM, rd, 0, rd, lo)])
        };
        let pcrel = |rd: u32, target: u32, second: u32| -> Vec<u8> {
            let (hi, lo) = hi_lo(target.wrapping_sub(self.pc));
            words(&[enc_u(OP_AUIPC, rd, hi), second | (lo & 0xFFF) << 20])
        };

        match name {
            "lui" | "auipc" => {
                self.want(2)?;
                let v = self.imm(1)?;
                if v > 0xFFFFF {
                    return Err(self.err("20 bitlik değer bekleniyordu"));
                }
                let op = if name == "lui" { OP_LUI } else { OP_AUIPC };
                w(enc_u(op, self.reg(0)?, v << 12))
            }
            "jal" => match a.len() {
                1 => w(enc_j(OP_JAL, 1, self.rel(0, 21)?)),
                _ => {
                    self.want(2)?;
                    w(enc_j(OP_JAL, self.reg(0)?, self.rel(1, 21)?))
                }
            },
            "jalr" => match a.len() {
                1 => w(enc_i(OP_JALR, 1, 0, self.reg(0)?, 0)),
                2 => {
                    let (off, r) = self.mem(1)?;
                    w(enc_i(OP_JALR, self.reg(0)?, 0, r, off))
                }
                _ => {
                    self.want(3)?;
                    w(enc_i(
                        OP_JALR,
                        self.reg(0)?,
                        0,
                        self.reg(1)?,
                        self.imm12(2)?,
                    ))
                }
            },
            "beq" => br(0, false),
            "bne" => br(1, false),
            "blt" => br(4, false),
            "bge" => br(5, false),
            "bltu" => br(6, false),
            "bgeu" => br(7, false),
            "bgt" => br(4, true),
            "ble" => br(5, true),
            "bgtu" => br(6, true),
            "bleu" => br(7, true),
            "beqz" => brz(0, false),
            "bnez" => brz(1, false),
            "bltz" => brz(4, false),
            "bgez" => brz(5, false),
            "bgtz" => brz(4, true),
            "blez" => brz(5, true),
            "lb" => ld(0),
            "lh" => ld(1),
            "lw" => ld(2),
            "lbu" => ld(4),
            "lhu" => ld(5),
            "sb" => st(0),
            "sh" => st(1),
            "sw" => st(2),
            "addi" => i3(0),
            "slti" => i3(2),
            "sltiu" => i3(3),
            "xori" => i3(4),
            "ori" => i3(6),
            "andi" => i3(7),
            "slli" => sh(1, 0),
            "srli" => sh(5, 0),
            "srai" => sh(5, 0x20),
            "add" => r3(0, 0),
            "sub" => r3(0, 0x20),
            "sll" => r3(1, 0),
            "slt" => r3(2, 0),
            "sltu" => r3(3, 0),
            "xor" => r3(4, 0),
            "srl" => r3(5, 0),
            "sra" => r3(5, 0x20),
            "or" => r3(6, 0),
            "and" => r3(7, 0),
            "mul" => r3(0, 1),
            "mulh" => r3(1, 1),
            "mulhsu" => r3(2, 1),
            "mulhu" => r3(3, 1),
            "div" => r3(4, 1),
            "divu" => r3(5, 1),
            "rem" => r3(6, 1),
            "remu" => r3(7, 1),
            "fence" => w(0x0FF0_000F),
            "ecall" => w(ECALL),
            "ebreak" | "halt" => w(EBREAK),
            "unimp" => w(UNIMP),
            "nop" => w(NOP),
            "mv" => {
                self.want(2)?;
                w(enc_i(OP_IMM, self.reg(0)?, 0, self.reg(1)?, 0))
            }
            "not" => {
                self.want(2)?;
                w(enc_i(OP_IMM, self.reg(0)?, 4, self.reg(1)?, 0xFFF))
            }
            "neg" => {
                self.want(2)?;
                w(enc_r(OP_REG, self.reg(0)?, 0, 0, self.reg(1)?, 0x20))
            }
            "seqz" => {
                self.want(2)?;
                w(enc_i(OP_IMM, self.reg(0)?, 3, self.reg(1)?, 1))
            }
            "snez" => {
                self.want(2)?;
                w(enc_r(OP_REG, self.reg(0)?, 3, 0, self.reg(1)?, 0))
            }
            "li" => {
                self.want(2)?;
                let rd = self.reg(0)?;
                let v = self.imm(1)?;
                if fits12(v) {
                    w(enc_i(OP_IMM, rd, 0, 0, v))
                } else {
                    Ok(hi_lo_pair(rd, v))
                }
            }
            "la" => {
                self.want(2)?;
                let rd = self.reg(0)?;
                let t = self.imm(1)?;
                Ok(pcrel(rd, t, enc_i(OP_IMM, rd, 0, rd, 0)))
            }
            "j" => {
                self.want(1)?;
                w(enc_j(OP_JAL, 0, self.rel(0, 21)?))
            }
            "jr" => {
                self.want(1)?;
                w(enc_i(OP_JALR, 0, 0, self.reg(0)?, 0))
            }
            "ret" => w(enc_i(OP_JALR, 0, 0, 1, 0)),
            "call" | "tail" => {
                self.want(1)?;
                let t = self.imm(0)?;
                let (link, tmp) = if name == "call" { (1, 1) } else { (0, 6) };
                Ok(pcrel(tmp, t, enc_i(OP_JALR, link, 0, tmp, 0)))
            }
            _ => Err(self.err(format!("bilinmeyen komut '{name}'"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{Cpu, Trap};
    use crate::machine::Machine;

    fn run(src: &str) -> Cpu {
        let p = assemble(src).unwrap_or_else(|e| panic!("{e}"));
        let mut m = Machine::new(64 * 1024);
        m.load(0, &p.bytes);
        let mut cpu = Cpu::new();
        cpu.x[2] = m.ram_bytes();
        assert_eq!(cpu.run(&mut m, 1_000_000), Some(Trap::Breakpoint));
        cpu
    }
    #[test]
    fn encodings_match_gnu_as() {
        let p = assemble(
            "addi a0, a0, 1
             sw a1, 8(sp)
             sub a0, a0, a1
             ret
             lui t0, 0x12345
             srai a2, a3, 7
             mulhu a0, a1, a2",
        )
        .unwrap();
        assert_eq!(
            p.words(),
            vec![
                0x0015_0513,
                0x00B1_2423,
                0x40B5_0533,
                0x0000_8067,
                0x1234_52B7,
                0x4076_D613,
                0x02C5_B533
            ]
        );
    }

    #[test]
    fn li_sizes() {
        assert_eq!(assemble("li a0, 5").unwrap().bytes.len(), 4);
        assert_eq!(assemble("li a0, -2048").unwrap().bytes.len(), 4);
        assert_eq!(assemble("li a0, 0x12345678").unwrap().bytes.len(), 8);
        let cpu = run("li a0, 0x12345FFF\nli a1, -1\nli a2, 0x80000000\nhalt");
        assert_eq!(cpu.x[10], 0x1234_5FFF);
        assert_eq!(cpu.x[11], u32::MAX);
        assert_eq!(cpu.x[12], 0x8000_0000);
    }

    #[test]
    fn fibonacci() {
        let cpu = run("      li a0, 0
                   li a1, 1
                   li t0, 10       # 10 rounds
             loop: add a2, a0, a1
                   mv a0, a1
                   mv a1, a2
                   addi t0, t0, -1
                   bnez t0, loop
                   halt");
        assert_eq!(cpu.x[10], 55);
    }

    #[test]
    fn data_and_calls() {
        let cpu = run(r#"     la a0, msg
                    call strlen
                    mv s0, a0
                    la t0, table
                    lw s1, 4(t0)
                    lbu s2, 0(t0)
                    halt
            strlen: mv t0, a0
            next:   lbu t1, 0(t0)
                    beqz t1, done
                    addi t0, t0, 1
                    j next
            done:   sub a0, t0, a0
                    ret
            msg:    .asciz "merhaba\n"
                    .align 2
            table:  .word 0xDEADBEEF, 42
            "#);
        assert_eq!(cpu.x[8], 8);
        assert_eq!(cpu.x[9], 42);
        assert_eq!(cpu.x[18], 0xEF);
    }

    #[test]
    fn errors_have_lines() {
        let e = assemble("nop\nfoo a0\n").unwrap_err();
        assert_eq!(e.line, 2);
        let e = assemble("addi a0, a0, 5000").unwrap_err();
        assert!(e.message.contains("12"));
        let e = assemble("j nowhere").unwrap_err();
        assert!(e.message.contains("nowhere"));
        let e = assemble("x: nop\nx: nop").unwrap_err();
        assert_eq!(e.line, 2);
    }
}
