//! Verilog names for makine names.
//!
//! makine allows any letter in chip and pin names (`Toplayıcı`, `seç`) and
//! pin names that start with a digit; Verilog identifiers are ASCII letters,
//! digits and `_`, starting with a letter or `_`, and must not be keywords.

use std::collections::BTreeSet;

const KEYWORDS: &[&str] = &[
    "always",
    "and",
    "assign",
    "automatic",
    "begin",
    "buf",
    "bufif0",
    "bufif1",
    "case",
    "casex",
    "casez",
    "cell",
    "cmos",
    "config",
    "deassign",
    "default",
    "defparam",
    "design",
    "disable",
    "edge",
    "else",
    "end",
    "endcase",
    "endconfig",
    "endfunction",
    "endgenerate",
    "endmodule",
    "endprimitive",
    "endspecify",
    "endtable",
    "endtask",
    "event",
    "for",
    "force",
    "forever",
    "fork",
    "function",
    "generate",
    "genvar",
    "highz0",
    "highz1",
    "if",
    "ifnone",
    "incdir",
    "include",
    "initial",
    "inout",
    "input",
    "instance",
    "integer",
    "join",
    "large",
    "liblist",
    "library",
    "localparam",
    "macromodule",
    "medium",
    "module",
    "nand",
    "negedge",
    "nmos",
    "nor",
    "noshowcancelled",
    "not",
    "notif0",
    "notif1",
    "or",
    "output",
    "parameter",
    "pmos",
    "posedge",
    "primitive",
    "pull0",
    "pull1",
    "pulldown",
    "pullup",
    "pulsestyle_ondetect",
    "pulsestyle_onevent",
    "rcmos",
    "real",
    "realtime",
    "reg",
    "release",
    "repeat",
    "rnmos",
    "rpmos",
    "rtran",
    "rtranif0",
    "rtranif1",
    "scalared",
    "showcancelled",
    "signed",
    "small",
    "specify",
    "specparam",
    "strong0",
    "strong1",
    "supply0",
    "supply1",
    "table",
    "task",
    "time",
    "tran",
    "tranif0",
    "tranif1",
    "tri",
    "tri0",
    "tri1",
    "triand",
    "trior",
    "trireg",
    "unsigned",
    "use",
    "uwire",
    "vectored",
    "wait",
    "wand",
    "weak0",
    "weak1",
    "while",
    "wire",
    "wor",
    "xnor",
    "xor",
    // SystemVerilog, so the output also reads in tools that default to it.
    "bit",
    "byte",
    "int",
    "logic",
    "shortint",
    "longint",
    "string",
    "type",
    "var",
    "void",
    "interface",
    "package",
    "program",
    "class",
    "final",
    "unique",
    "priority",
    "do",
    "break",
    "continue",
    "return",
    "const",
    "enum",
    "struct",
    "union",
    "typedef",
    "import",
    "export",
];

/// `name` as a Verilog identifier (not yet unique).
pub fn sanitize(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => s.push(c),
            'ç' => s.push('c'),
            'Ç' => s.push('C'),
            'ğ' => s.push('g'),
            'Ğ' => s.push('G'),
            'ı' => s.push('i'),
            'İ' => s.push('I'),
            'ö' => s.push('o'),
            'Ö' => s.push('O'),
            'ş' => s.push('s'),
            'Ş' => s.push('S'),
            'ü' => s.push('u'),
            'Ü' => s.push('U'),
            _ if c.is_alphanumeric() => s.push_str(&format!("u{:x}", c as u32)),
            _ => s.push('_'),
        }
    }
    if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert_str(0, "p_");
    }
    if KEYWORDS.contains(&s.as_str()) {
        s.push('_');
    }
    s
}

/// Hands out identifiers that are unique within one scope (a module).
#[derive(Default)]
pub struct Names {
    used: BTreeSet<String>,
}

impl Names {
    /// A unique identifier based on `name`.
    pub fn fresh(&mut self, name: &str) -> String {
        let base = sanitize(name);
        let mut candidate = base.clone();
        let mut n = 1;
        // Compare case-insensitively: some tools and file systems do.
        while self.used.contains(&candidate.to_ascii_lowercase()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        self.used.insert(candidate.to_ascii_lowercase());
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        assert_eq!(sanitize("Toplayıcı"), "Toplayici");
        assert_eq!(sanitize("0..7"), "p_0__7");
        assert_eq!(sanitize("and"), "and_");
        assert_eq!(sanitize("in"), "in");
        let mut n = Names::default();
        assert_eq!(n.fresh("seç"), "sec");
        assert_eq!(n.fresh("sec"), "sec_1");
        assert_eq!(n.fresh("SEC"), "SEC_2");
    }
}
