#!/usr/bin/env python3
"""Draw the wiki's diagrams as SVG, then render them to PNG with resvg.

    python3 docs/diagrams/make.py            # needs `resvg` on PATH
    RESVG=/path/to/resvg python3 docs/diagrams/make.py

Gate shapes follow the same proportions as the app's canvas
(crates/app/src/canvas.rs), so the wiki and the editor look alike.
"""

import math
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "wiki", "img")
SVG_DIR = os.path.join(HERE, "svg")

# Colours: the app's dark theme.
BG = "#181a1f"
PANEL = "#23262e"
LINE = "#c8ced8"
DIM = "#8c929e"
TEXT = "#dce0e6"
ON = "#5adc78"
OFF = "#5a606c"
BUS = "#6eaaf0"
WARN = "#e6a050"
RED = "#e65a5a"
FONT = "DejaVu Sans, Ubuntu, sans-serif"
MONO = "DejaVu Sans Mono, monospace"


class Svg:
    def __init__(self, w, h, title=None):
        self.w, self.h = w, h
        self.parts = []
        self.rect(0, 0, w, h, fill=BG, stroke="none", r=0)
        if title:
            self.text(w / 2, 34, title, size=22, weight="bold", anchor="middle")

    def add(self, s):
        self.parts.append(s)

    def line(self, x1, y1, x2, y2, color=LINE, width=2.5, dash=None):
        d = f' stroke-dasharray="{dash}"' if dash else ""
        self.add(
            f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" '
            f'stroke="{color}" stroke-width="{width}" stroke-linecap="round"{d}/>'
        )

    def path(self, d, fill="none", stroke=LINE, width=2.5):
        self.add(
            f'<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{width}" '
            f'stroke-linejoin="round" stroke-linecap="round"/>'
        )

    def poly(self, pts, fill=PANEL, stroke=LINE, width=2.5):
        p = " ".join(f"{x:.1f},{y:.1f}" for x, y in pts)
        self.add(
            f'<polygon points="{p}" fill="{fill}" stroke="{stroke}" stroke-width="{width}" '
            f'stroke-linejoin="round"/>'
        )

    def rect(self, x, y, w, h, fill=PANEL, stroke=LINE, r=6, width=2):
        self.add(
            f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" '
            f'stroke="{stroke}" stroke-width="{width}"/>'
        )

    def circle(self, x, y, r, fill=PANEL, stroke=LINE, width=2.5):
        self.add(
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r}" fill="{fill}" stroke="{stroke}" '
            f'stroke-width="{width}"/>'
        )

    def dot(self, x, y, color=LINE, r=4):
        self.circle(x, y, r, fill=color, stroke="none", width=0)

    def text(self, x, y, s, size=16, color=TEXT, anchor="start", weight="normal", mono=False):
        s = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        f = MONO if mono else FONT
        self.add(
            f'<text x="{x:.1f}" y="{y:.1f}" font-family="{f}" font-size="{size}" '
            f'fill="{color}" text-anchor="{anchor}" font-weight="{weight}">{s}</text>'
        )

    def wire(self, pts, color=LINE, width=2.5):
        d = "M " + " L ".join(f"{x:.1f} {y:.1f}" for x, y in pts)
        self.path(d, stroke=color, width=width)

    def svg(self):
        body = "\n".join(self.parts)
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" '
            f'viewBox="0 0 {self.w} {self.h}">\n{body}\n</svg>\n'
        )


# --- gate symbols ---------------------------------------------------------
# Each draws a gate whose body spans x..x+w, y..y+h and returns pin
# positions: (inputs, output).


def gate(s, kind, x, y, w=70, h=56, out_color=LINE, label=None):
    cy = y + h / 2
    bubble = kind in ("not", "nand", "nor", "xnor")
    if kind == "not":
        s.poly([(x, y), (x + w, cy), (x, y + h)])
        ins = [(x, cy)]
    elif kind in ("and", "nand"):
        r = h / 2
        mid = x + w - r
        d = f"M {x} {y} L {mid} {y} A {r} {r} 0 0 1 {mid} {y + h} L {x} {y + h} Z"
        s.path(d, fill=PANEL)
        ins = [(x, y + h * 0.25), (x, y + h * 0.75)]
    elif kind in ("or", "nor", "xor", "xnor"):
        back = w * 0.22
        d = (
            f"M {x} {y} C {x + w * 0.55} {y} {x + w * 0.85} {y + h * 0.2} {x + w} {cy} "
            f"C {x + w * 0.85} {y + h * 0.8} {x + w * 0.55} {y + h} {x} {y + h} "
            f"C {x + back} {y + h * 0.75} {x + back} {y + h * 0.25} {x} {y} Z"
        )
        s.path(d, fill=PANEL)
        if kind in ("xor", "xnor"):
            g = 8
            s.path(
                f"M {x - g} {y + h} C {x - g + back} {y + h * 0.75} "
                f"{x - g + back} {y + h * 0.25} {x - g} {y}"
            )
        # Inputs meet the curved back.
        def back_x(t):
            return x + back * 0.75 * (1 - t * t)

        ins = [(back_x(0.5), y + h * 0.25), (back_x(0.5), y + h * 0.75)]
    else:
        raise ValueError(kind)
    ox = x + w
    if bubble:
        s.circle(ox + 6, cy, 6)
        ox += 12
    if label:
        s.text(x + w / 2 - 6, y - 8, label, size=13, color=DIM, anchor="middle")
    return ins, (ox, cy)


def lead_in(s, pin, length=30, color=LINE, name=None):
    x, y = pin
    s.line(x - length, y, x, y, color=color)
    if name:
        s.text(x - length - 8, y + 5, name, size=16, anchor="end", mono=True)
    return (x - length, y)


# --- diagrams --------------------------------------------------------------


def gates_table():
    """The basic gates, their symbols and truth tables."""
    rows = [
        ("NOT", "not", "DEĞİL", lambda a, b: 1 - a, 1),
        ("AND", "and", "VE", lambda a, b: a & b, 2),
        ("OR", "or", "VEYA", lambda a, b: a | b, 2),
        ("NAND", "nand", "VE DEĞİL", lambda a, b: 1 - (a & b), 2),
        ("NOR", "nor", "VEYA DEĞİL", lambda a, b: 1 - (a | b), 2),
        ("XOR", "xor", "ÖZEL VEYA", lambda a, b: a ^ b, 2),
    ]
    W, H = 980, 120 + 112 * len(rows)
    s = Svg(W, H, "Temel mantık kapıları")
    s.text(320, 80, "sembol", size=14, color=DIM, anchor="middle")
    s.text(700, 80, "doğruluk tablosu", size=14, color=DIM, anchor="middle")
    for i, (name, kind, tr, f, n) in enumerate(rows):
        y = 100 + i * 112
        s.text(40, y + 36, name, size=22, weight="bold")
        s.text(40, y + 60, tr, size=14, color=DIM)
        ins, out = gate(s, kind, 290, y + 12)
        for k, p in enumerate(ins):
            lead_in(s, p, 40 + (p[0] - 290), name="ab"[k] if n == 2 else "a")
        s.line(out[0], out[1], out[0] + 36, out[1])
        s.text(out[0] + 44, out[1] + 5, "çıkış", size=14, color=DIM)
        # truth table
        tx = 560
        cols = ["a", "b"][:n] + ["çıkış"]
        combos = [(a, b) for a in (0, 1) for b in (0, 1)] if n == 2 else [(0, 0), (1, 0)]
        cw = 44
        for c, h in enumerate(cols):
            s.text(tx + c * cw + (10 if h == "çıkış" else 0), y + 10, h, size=12, color=DIM, anchor="middle")
        for r, (a, b) in enumerate(combos):
            vals = [a, b][:n] + [f(a, b)]
            for c, v in enumerate(vals):
                last = c == len(vals) - 1
                color = (ON if v else OFF) if last else TEXT
                s.text(
                    tx + c * cw + (10 if last else 0),
                    y + 30 + r * 19,
                    str(v),
                    size=15,
                    color=color,
                    anchor="middle",
                    mono=True,
                )
            if n == 1:
                pass
        if n == 1:
            # NOT table is just two rows; fill the space with a note.
            s.text(tx + 140, y + 48, "girişin tersi", size=14, color=DIM)
        else:
            s.text(tx + 200, y + 48, {
                "and": "ikisi de 1 ise 1",
                "or": "en az biri 1 ise 1",
                "nand": "ikisi de 1 ise 0",
                "nor": "ikisi de 0 ise 1",
                "xor": "farklılarsa 1",
            }[kind], size=14, color=DIM)
    return s


def transistor_switch():
    """An nMOS and pMOS transistor as switches."""
    W, H = 900, 400
    s = Svg(W, H, "Transistör: elektrikle açılıp kapanan bir anahtar")

    def mos(cx, cy, kind):
        # Channel and gate, drawn as a simple MOSFET symbol.
        s.line(cx, cy - 70, cx, cy - 30)
        s.line(cx, cy + 30, cx, cy + 70)
        s.line(cx, cy - 30, cx - 18, cy - 30)
        s.line(cx, cy + 30, cx - 18, cy + 30)
        s.line(cx - 18, cy - 40, cx - 18, cy + 40, width=3)
        s.line(cx - 30, cy - 34, cx - 30, cy + 34, width=3)
        gx = cx - 30
        if kind == "p":
            s.circle(gx - 7, cy, 6)
            gx -= 14
        s.line(gx - 50, cy, gx, cy)
        s.text(gx - 58, cy + 5, "kapı", size=14, color=DIM, anchor="end")
        return (cx, cy - 70), (cx, cy + 70)

    # nMOS
    s.text(220, 80, "nMOS", size=20, weight="bold", anchor="middle")
    top, bot = mos(240, 210, "n")
    s.text(260, top[1] + 4, "üst", size=14, color=DIM)
    s.text(260, bot[1] + 4, "alt", size=14, color=DIM)
    s.text(220, 330, "kapı = 1  →  iletir (kapalı anahtar)", size=15, color=ON, anchor="middle")
    s.text(220, 356, "kapı = 0  →  iletmez (açık anahtar)", size=15, color=DIM, anchor="middle")

    # pMOS
    s.text(660, 80, "pMOS", size=20, weight="bold", anchor="middle")
    top, bot = mos(680, 210, "p")
    s.text(700, top[1] + 4, "üst", size=14, color=DIM)
    s.text(700, bot[1] + 4, "alt", size=14, color=DIM)
    s.text(660, 330, "kapı = 0  →  iletir (kapalı anahtar)", size=15, color=ON, anchor="middle")
    s.text(660, 356, "kapı = 1  →  iletmez (açık anahtar)", size=15, color=DIM, anchor="middle")
    s.text(660, 384, "(daire: \u201ctersine çalışır\u201d demek)", size=13, color=DIM, anchor="middle")
    return s


def cmos_nand():
    """The four-transistor CMOS NAND gate."""
    W, H = 900, 640
    s = Svg(W, H, "CMOS NAND: 4 transistör")

    vdd_y, out_y, gnd_y = 90, 300, 590

    def mos(cx, cy, kind, gate_name):
        s.line(cx, cy - 50, cx, cy - 22)
        s.line(cx, cy + 22, cx, cy + 50)
        s.line(cx, cy - 22, cx - 14, cy - 22)
        s.line(cx, cy + 22, cx - 14, cy + 22)
        s.line(cx - 14, cy - 30, cx - 14, cy + 30, width=3)
        s.line(cx - 24, cy - 26, cx - 24, cy + 26, width=3)
        gx = cx - 24
        if kind == "p":
            s.circle(gx - 6, cy, 5)
            gx -= 11
        s.line(gx - 34, cy, gx, cy)
        s.text(gx - 40, cy + 6, gate_name, size=18, anchor="end", mono=True, weight="bold")
        return (cx, cy - 50), (cx, cy + 50)

    # VDD rail
    s.line(220, vdd_y, 640, vdd_y, color=WARN, width=3)
    s.text(660, vdd_y + 6, "+V (1)", size=16, color=WARN)
    # two pMOS in parallel
    pa_top, pa_bot = mos(320, 190, "p", "a")
    pb_top, pb_bot = mos(540, 190, "p", "b")
    for t in (pa_top, pb_top):
        s.line(t[0], t[1], t[0], vdd_y)
        s.dot(t[0], vdd_y)
    s.line(pa_bot[0], pa_bot[1], pa_bot[0], out_y)
    s.line(pb_bot[0], pb_bot[1], pb_bot[0], out_y)
    s.line(320, out_y, 700, out_y)
    s.dot(320, out_y)
    s.dot(540, out_y)
    s.dot(430, out_y)
    s.text(712, out_y + 6, "çıkış", size=18, weight="bold")
    # two nMOS in series
    na_top, na_bot = mos(430, 380, "n", "a")
    nb_top, nb_bot = mos(430, 500, "n", "b")
    s.line(430, out_y, na_top[0], na_top[1])
    s.line(na_bot[0], na_bot[1], nb_top[0], nb_top[1])
    s.line(nb_bot[0], nb_bot[1], 430, gnd_y)
    # ground
    s.line(390, gnd_y, 470, gnd_y, color=DIM, width=3)
    s.line(402, gnd_y + 8, 458, gnd_y + 8, color=DIM, width=3)
    s.line(414, gnd_y + 16, 446, gnd_y + 16, color=DIM, width=3)
    s.text(486, gnd_y + 6, "toprak (0)", size=16, color=DIM)

    # explanations
    s.text(40, 170, "üstte paralel", size=15, color=DIM)
    s.text(40, 192, "pMOS'lar:", size=15, color=DIM)
    s.text(40, 214, "a VEYA b sıfırsa", size=15, color=DIM)
    s.text(40, 236, "çıkışı 1'e çeker", size=15, color=DIM)
    s.text(40, 410, "altta seri", size=15, color=DIM)
    s.text(40, 432, "nMOS'lar:", size=15, color=DIM)
    s.text(40, 454, "a VE b birse", size=15, color=DIM)
    s.text(40, 476, "çıkışı 0'a çeker", size=15, color=DIM)
    return s


def nand_to_all():
    """NOT, AND and OR built from NAND."""
    W, H = 980, 560
    s = Svg(W, H, "Her şey NAND'den")

    # NOT
    s.text(40, 100, "DEĞİL", size=18, weight="bold")
    ins, out = gate(s, "nand", 200, 80)
    s.line(120, 108, 170, 108)
    s.line(170, ins[0][1], 170, ins[1][1])
    s.line(170, ins[0][1], ins[0][0], ins[0][1])
    s.line(170, ins[1][1], ins[1][0], ins[1][1])
    s.dot(170, 108)
    s.text(110, 113, "a", size=16, anchor="end", mono=True)
    s.line(out[0], out[1], out[0] + 40, out[1])
    s.text(out[0] + 50, out[1] + 5, "DEĞİL a", size=15, color=DIM)
    s.text(560, 104, "iki girişe aynı sinyal:", size=14, color=DIM)
    s.text(560, 126, "NAND(a, a) = DEĞİL a", size=15, mono=True)

    # AND
    y = 230
    s.text(40, y + 20, "VE", size=18, weight="bold")
    ins, out = gate(s, "nand", 200, y)
    lead_in(s, ins[0], 60, name="a")
    lead_in(s, ins[1], 60, name="b")
    ins2, out2 = gate(s, "nand", 340, y)
    s.line(out[0], out[1], out[0] + 18, out[1])
    s.line(out[0] + 18, ins2[0][1], out[0] + 18, ins2[1][1])
    s.line(out[0] + 18, ins2[0][1], ins2[0][0], ins2[0][1])
    s.line(out[0] + 18, ins2[1][1], ins2[1][0], ins2[1][1])
    s.dot(out[0] + 18, out[1])
    s.line(out2[0], out2[1], out2[0] + 40, out2[1])
    s.text(out2[0] + 50, out2[1] + 5, "a VE b", size=15, color=DIM)
    s.text(560, y + 24, "NAND'ın tersini al:", size=14, color=DIM)
    s.text(560, y + 46, "DEĞİL(NAND(a, b))", size=15, mono=True)

    # OR
    y = 380
    s.text(40, y + 60, "VEYA", size=18, weight="bold")
    na_in, na_out = gate(s, "nand", 200, y, h=44)
    nb_in, nb_out = gate(s, "nand", 200, y + 80, h=44)
    for g_in, name in ((na_in, "a"), (nb_in, "b")):
        x0 = 150
        s.line(x0 - 30, (g_in[0][1] + g_in[1][1]) / 2, x0, (g_in[0][1] + g_in[1][1]) / 2)
        s.text(x0 - 38, (g_in[0][1] + g_in[1][1]) / 2 + 5, name, size=16, anchor="end", mono=True)
        s.line(x0, g_in[0][1], x0, g_in[1][1])
        s.dot(x0, (g_in[0][1] + g_in[1][1]) / 2)
        s.line(x0, g_in[0][1], g_in[0][0], g_in[0][1])
        s.line(x0, g_in[1][1], g_in[1][0], g_in[1][1])
    o_in, o_out = gate(s, "nand", 380, y + 40)
    s.wire([na_out, (na_out[0] + 20, na_out[1]), (na_out[0] + 20, o_in[0][1]), o_in[0]])
    s.wire([nb_out, (nb_out[0] + 20, nb_out[1]), (nb_out[0] + 20, o_in[1][1]), o_in[1]])
    s.line(o_out[0], o_out[1], o_out[0] + 40, o_out[1])
    s.text(o_out[0] + 50, o_out[1] + 5, "a VEYA b", size=15, color=DIM)
    s.text(600, y + 50, "De Morgan:", size=14, color=DIM)
    s.text(600, y + 72, "a VEYA b =", size=15, mono=True)
    s.text(600, y + 94, "NAND(DEĞİL a, DEĞİL b)", size=15, mono=True)
    return s


def half_full_adder():
    W, H = 980, 520
    s = Svg(W, H, "Toplayıcılar")
    # Half adder
    s.text(40, 80, "Yarım toplayıcı: a + b", size=18, weight="bold")
    xin, xout = gate(s, "xor", 220, 130)
    ain, aout = gate(s, "and", 220, 220)
    ax, bx = 130, 160
    s.text(ax, 118, "a", size=16, anchor="middle", mono=True)
    s.text(bx, 118, "b", size=16, anchor="middle", mono=True)
    s.line(ax, 124, ax, ain[0][1])
    s.line(bx, 124, bx, ain[1][1])
    s.wire([(ax, xin[0][1]), xin[0]])
    s.wire([(bx, xin[1][1]), xin[1]])
    s.wire([(ax, ain[0][1]), ain[0]])
    s.wire([(bx, ain[1][1]), ain[1]])
    for p in ((ax, xin[0][1]), (bx, xin[1][1])):
        s.dot(*p)
    s.line(xout[0], xout[1], xout[0] + 50, xout[1])
    s.text(xout[0] + 58, xout[1] + 5, "toplam (alt basamak)", size=15)
    s.line(aout[0], aout[1], aout[0] + 50, aout[1])
    s.text(aout[0] + 58, aout[1] + 5, "elde (üst basamak)", size=15)
    tx, ty = 690, 130
    s.text(tx, ty, "a + b = elde toplam", size=14, color=DIM, mono=True)
    for i, (a, b) in enumerate([(0, 0), (0, 1), (1, 0), (1, 1)]):
        c, t = (a + b) >> 1, (a + b) & 1
        s.text(tx, ty + 26 + i * 22, f"{a} + {b} =   {c}    {t}", size=15, mono=True)

    # Full adder chain
    y0 = 330
    s.text(40, y0, "Tam toplayıcıları zincirle: 4 bitlik toplama", size=18, weight="bold")
    for i in range(4):
        x = 720 - i * 190
        s.rect(x, y0 + 40, 130, 80, fill=PANEL)
        s.text(x + 65, y0 + 86, "Tam Topl.", size=15, anchor="middle")
        s.text(x + 40, y0 + 30, f"a{i}", size=14, anchor="middle", mono=True)
        s.text(x + 90, y0 + 30, f"b{i}", size=14, anchor="middle", mono=True)
        s.line(x + 40, y0 + 36, x + 40, y0 + 40)
        s.line(x + 90, y0 + 36, x + 90, y0 + 40)
        s.line(x + 65, y0 + 120, x + 65, y0 + 150)
        s.text(x + 65, y0 + 170, f"t{i}", size=14, anchor="middle", mono=True)
        # carry out to the left
        if i < 3:
            s.line(x, y0 + 80, x - 60, y0 + 80, color=WARN)
            s.text(x - 30, y0 + 72, "elde", size=12, color=WARN, anchor="middle")
    s.line(720 + 130, y0 + 80, 720 + 170, y0 + 80, color=WARN)
    s.text(720 + 178, y0 + 85, "0", size=14, color=WARN, mono=True)
    s.line(150, y0 + 80, 110, y0 + 80, color=WARN)
    s.text(100, y0 + 85, "elde", size=14, color=WARN, anchor="end")
    return s


def sr_latch():
    W, H = 900, 420
    s = Svg(W, H, "SR mandalı: iki NAND, birbirine bağlı")
    top_in, top_out = gate(s, "nand", 340, 110)
    bot_in, bot_out = gate(s, "nand", 340, 250)
    # external inputs (active low: through NOT in the game's version)
    lead_in(s, top_in[0], 120, name="DEĞİL set")
    lead_in(s, bot_in[1], 120, name="DEĞİL reset")
    # outputs
    s.line(top_out[0], top_out[1], top_out[0] + 120, top_out[1], color=ON)
    s.text(top_out[0] + 130, top_out[1] + 6, "q", size=20, mono=True, weight="bold")
    s.line(bot_out[0], bot_out[1], bot_out[0] + 120, bot_out[1])
    s.text(bot_out[0] + 130, bot_out[1] + 6, "DEĞİL q", size=16, mono=True)
    # Cross coupling, drawn as the classic X: q goes down-left to the
    # bottom gate, DEĞİL q goes up-left to the top gate.
    xo = top_out[0] + 40
    s.dot(xo, top_out[1], color=ON)
    s.dot(xo, bot_out[1])
    s.wire([(xo, top_out[1]), (xo, 175), (300, 215), (300, bot_in[0][1]), bot_in[0]], color=ON)
    s.wire([(xo, bot_out[1]), (xo, 215), (312, 175), (312, top_in[1][1]), top_in[1]])
    s.text(450, 390, "Her NAND'ın çıkışı diğerinin girişine döner: devre kendi durumunu hatırlar.",
           size=15, color=DIM, anchor="middle")
    return s


def clock_edge():
    W, H = 900, 360
    s = Svg(W, H, "Saat ve yükselen kenar")
    x0, x1 = 80, 840
    period = 160
    # clock
    y_hi, y_lo = 100, 150
    pts = []
    x = x0
    level = 0
    while x < x1:
        y = y_lo if level == 0 else y_hi
        pts.append((x, y))
        nx = min(x + period / 2, x1)
        pts.append((nx, y))
        x = nx
        level ^= 1
    s.wire(pts, color=WARN, width=3)
    s.text(x0 - 10, 130, "clk", size=16, anchor="end", mono=True, color=WARN)
    # rising edges
    for k in range(5):
        ex = x0 + period / 2 + k * period
        if ex < x1:
            s.line(ex, 80, ex, 300, color=DIM, width=1.2, dash="4 4")
            s.text(ex, 72, "↑", size=18, color=WARN, anchor="middle")
    # d: changes at arbitrary times, including a glitch while the clock is
    # high (ignored). q is computed from d, so the picture stays right.
    d_changes = [(x0, 0), (120, 1), (270, 0), (400, 1), (530, 0), (565, 1), (700, 0)]

    def d_at(x):
        v = 0
        for cx, cv in d_changes:
            if cx <= x:
                v = cv
        return v

    def trace(changes, y_lo, y_hi):
        pts = []
        for i, (cx, v) in enumerate(changes):
            y = y_hi if v else y_lo
            if pts:
                pts.append((cx, pts[-1][1]))
            pts.append((cx, y))
        pts.append((x1, pts[-1][1]))
        return pts

    s.wire(trace(d_changes, 230, 200), color=BUS)
    s.text(x0 - 10, 220, "d", size=16, anchor="end", mono=True, color=BUS)
    q_changes = [(x0, 0)]
    for k in range(5):
        ex = x0 + period / 2 + k * period
        if ex < x1 and d_at(ex) != q_changes[-1][1]:
            q_changes.append((ex, d_at(ex)))
    s.wire(trace(q_changes, 290, 260), color=ON)
    s.text(x0 - 10, 280, "q", size=16, anchor="end", mono=True, color=ON)
    s.text(450, 340, "q sadece saatin 0'dan 1'e geçtiği anda (↑) d'yi kopyalar; arada d ne yaparsa yapsın sabit kalır.",
           size=14, color=DIM, anchor="middle")
    return s


def cpu_datapath():
    W, H = 1180, 640
    s = Svg(W, H, "Tek çevrimli RISC-V işlemci: veri yolu")

    def box(x, y, w, h, name, sub=None, color=PANEL):
        s.rect(x, y, w, h, fill=color)
        s.text(x + w / 2, y + h / 2 + (0 if sub else 6), name, size=17, anchor="middle", weight="bold")
        if sub:
            s.text(x + w / 2, y + h / 2 + 22, sub, size=12, color=DIM, anchor="middle")

    def arrow(pts, color=LINE):
        s.wire(pts, color=color)
        (x1, y1), (x2, y2) = pts[-2], pts[-1]
        a = math.atan2(y2 - y1, x2 - x1)
        for da in (2.6, -2.6):
            s.line(x2, y2, x2 + 10 * math.cos(a + da), y2 + 10 * math.sin(a + da), color=color)

    def label(x, y, t, color=DIM):
        s.text(x, y, t, size=12, color=color)

    box(30, 250, 120, 70, "pc", "program sayacı")
    box(210, 230, 150, 110, "Komut", "pc'deki kelime")
    box(430, 80, 160, 80, "Çözücü", "opcode, rd, rs1, imm")
    box(430, 230, 160, 170, "Register", "dosyası x0…x31")
    box(680, 250, 130, 120, "ALU", "+ − & | ^ << >>")
    box(900, 230, 160, 110, "Veri belleği", "yükle / sakla")
    box(680, 470, 130, 70, "Kontrol", "ne yapılacak?")
    box(210, 470, 150, 70, "Sıradaki pc", "pc + 4 ya da atla")

    arrow([(150, 285), (210, 285)])
    arrow([(360, 260), (395, 260), (395, 120), (430, 120)])
    label(366, 252, "instr")
    arrow([(510, 160), (510, 230)])
    label(518, 200, "rs1, rs2, rd")
    arrow([(590, 270), (680, 270)])
    label(620, 262, "a")
    arrow([(590, 350), (680, 350)])
    label(620, 342, "b")
    arrow([(590, 100), (640, 100), (640, 320), (680, 320)], color=BUS)
    label(648, 140, "imm", BUS)
    arrow([(810, 290), (900, 290)])
    label(822, 282, "adres")
    arrow([(590, 385), (860, 385), (860, 320), (900, 320)])
    label(700, 403, "saklanacak veri")
    arrow([(980, 340), (980, 440), (410, 440), (410, 375), (430, 375)], color=ON)
    label(600, 458, "sonuç: register'a geri yaz", ON)
    arrow([(590, 140), (620, 140), (620, 200), (845, 200), (845, 505), (810, 505)], color=WARN)
    label(700, 192, "opcode", WARN)
    arrow([(285, 470), (285, 410), (90, 410), (90, 320)], color=WARN)
    label(100, 402, "sıradaki pc", WARN)

    steps = [
        "1. pc'deki komutu oku",
        "2. çöz: hangi işlem, hangi register'lar?",
        "3. register'ları oku, ALU hesaplasın",
        "4. gerekirse bellekten oku ya da belleğe yaz",
        "5. sonucu register'a yaz, pc'yi ilerlet",
    ]
    for i, t in enumerate(steps):
        s.text(880, 490 + i * 24, t, size=13)
    s.text(30, 610, "Her saat vuruşunda bu beş adımın hepsi bir kerede olur.", size=14, color=DIM)
    return s


def binary_counting():
    W, H = 900, 300
    s = Svg(W, H, "İkilik sayılar: her basamak bir anahtar")
    weights = [128, 64, 32, 16, 8, 4, 2, 1]
    value = 0b00101101
    for i, w in enumerate(weights):
        x = 110 + i * 88
        bit = (value >> (7 - i)) & 1
        s.rect(x, 90, 64, 64, fill=ON if bit else PANEL, stroke=ON if bit else LINE)
        s.text(x + 32, 134, str(bit), size=30, anchor="middle", mono=True,
               color=BG if bit else TEXT, weight="bold")
        s.text(x + 32, 184, str(w), size=16, anchor="middle", color=DIM, mono=True)
    s.text(60, 134, "bit", size=14, color=DIM, anchor="end")
    s.text(60, 184, "değer", size=14, color=DIM, anchor="end")
    s.text(450, 240, "00101101 = 32 + 8 + 4 + 1 = 45", size=22, anchor="middle", mono=True)
    return s


DIAGRAMS = {
    "kapilar": gates_table,
    "transistor": transistor_switch,
    "cmos-nand": cmos_nand,
    "nand-her-sey": nand_to_all,
    "toplayici": half_full_adder,
    "sr-mandal": sr_latch,
    "saat": clock_edge,
    "islemci": cpu_datapath,
    "ikilik": binary_counting,
}


def main():
    resvg = os.environ.get("RESVG") or shutil.which("resvg")
    if not resvg:
        sys.exit("resvg not found: cargo install resvg, or set RESVG=/path/to/resvg")
    os.makedirs(OUT, exist_ok=True)
    os.makedirs(SVG_DIR, exist_ok=True)
    for name, make in DIAGRAMS.items():
        svg_path = os.path.join(SVG_DIR, name + ".svg")
        with open(svg_path, "w") as f:
            f.write(make().svg())
        png = os.path.join(OUT, name + ".png")
        subprocess.run([resvg, "--zoom", "1", svg_path, png], check=True)
        print(png)


if __name__ == "__main__":
    main()
