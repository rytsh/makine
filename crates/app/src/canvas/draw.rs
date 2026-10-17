//! Painting parts, gates, pins, ports and wires.

use egui::{
    epaint::CubicBezierShape, pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Shape,
    Stroke, StrokeKind, Vec2,
};
use makine_sim::{PartKind, Pin};

use super::layout::{format_value, pin_label};
use crate::design::{Geometry, Symbol, BUBBLE, GATE_W, HEADER, LEAD, PIN_RADIUS};

use crate::theme;

pub const BG: Color32 = theme::BOARD;
pub(crate) const GRID: Color32 = theme::BOARD_GRID;
/// Parts are black epoxy packages on the board.
pub(crate) const PART_FILL: Color32 = Color32::from_rgb(22, 27, 25);
pub(crate) const PART_HEADER: Color32 = Color32::from_rgb(42, 52, 48);
pub(crate) const NAND_HEADER: Color32 = Color32::from_rgb(124, 74, 40);
pub(crate) const BUS_HEADER: Color32 = Color32::from_rgb(34, 62, 82);
pub(crate) const GATE_FILL: Color32 = Color32::from_rgb(20, 26, 24);
pub(crate) const GATE_LINE: Color32 = theme::INK;
pub(crate) const TEXT: Color32 = theme::INK;
pub(crate) const DIM: Color32 = theme::INK_DIM;
pub const ON: Color32 = theme::SIGNAL;
pub const OFF: Color32 = theme::SIGNAL_OFF;
pub(crate) const WIRE_HOVER: Color32 = Color32::from_rgb(255, 236, 200);
pub(crate) const BUS: Color32 = theme::BUS;
pub(crate) const ERR: Color32 = theme::BAD;

pub(crate) fn wire_color(width: u8, value: Option<u64>) -> Color32 {
    match (width, value) {
        (1, Some(1)) => ON,
        (1, _) => OFF,
        (_, Some(0) | None) => BUS.gamma_multiply(0.55),
        _ => BUS,
    }
}

pub(crate) fn pin_dot(painter: &egui::Painter, at: Pos2, p: &Pin, z: f32, hovered: bool) {
    let r = PIN_RADIUS * z * if hovered { 1.4 } else { 1.0 };
    let fill = if p.width > 1 { BUS } else { DIM };
    painter.circle_filled(at, r, fill);
    painter.circle_stroke(at, r, Stroke::new(1.0, BG));
}

pub(super) fn port(painter: &egui::Painter, r: Rect, p: &Pin, v: u64, z: f32, hovered: bool) {
    let lit = p.width == 1 && v == 1;
    let fill = if lit {
        ON.gamma_multiply(0.35)
    } else {
        PART_FILL
    };
    painter.rect_filled(r, CornerRadius::same((6.0 * z) as u8), fill);
    let border = if hovered {
        TEXT
    } else if lit {
        ON
    } else {
        DIM
    };
    painter.rect_stroke(
        r,
        CornerRadius::same((6.0 * z) as u8),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.text(
        r.center_top() + vec2(0.0, 4.0 * z),
        Align2::CENTER_TOP,
        pin_label(p),
        FontId::proportional(11.0 * z),
        DIM,
    );
    painter.text(
        r.center_bottom() - vec2(0.0, 4.0 * z),
        Align2::CENTER_BOTTOM,
        format_value(p.width, v),
        FontId::monospace(14.0 * z),
        if lit { ON } else { TEXT },
    );
}

/// A wire from source `a` (leaving to the right) to sink `b`, arriving from
/// direction `dir` (pointing away from the sink, towards the wire).
pub(crate) fn bezier(a: Pos2, b: Pos2, dir: Vec2, stroke: Stroke) -> Shape {
    CubicBezierShape::from_points_stroke(
        bezier_points(a, b, dir),
        false,
        Color32::TRANSPARENT,
        stroke,
    )
    .into()
}

/// Control points of the wire from `a` (leaving to the right) into `b`
/// (entered against `dir`).
pub(crate) fn bezier_points(a: Pos2, b: Pos2, dir: Vec2) -> [Pos2; 4] {
    let d = ((b.x - a.x).abs() * 0.5).max(40.0);
    let end = if dir.y != 0.0 {
        let dy = ((b.y - a.y).abs() * 0.5).max(40.0);
        b + dir * dy
    } else {
        b + dir * d
    };
    [a, a + vec2(d, 0.0), end, b]
}

/// Distance from `p` to the wire curve `c`.
pub(crate) fn bezier_distance(c: [Pos2; 4], p: Pos2) -> f32 {
    let at = |t: f32| {
        let u = 1.0 - t;
        (c[0].to_vec2() * (u * u * u)
            + c[1].to_vec2() * (3.0 * u * u * t)
            + c[2].to_vec2() * (3.0 * u * t * t)
            + c[3].to_vec2() * (t * t * t))
            .to_pos2()
    };
    const STEPS: usize = 32;
    (0..STEPS)
        .map(|k| {
            let (a, b) = (
                at(k as f32 / STEPS as f32),
                at((k + 1) as f32 / STEPS as f32),
            );
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        })
        .fold(f32::INFINITY, f32::min)
}

/// A part drawn as a labelled box with a coloured header.
pub(crate) fn draw_box(
    painter: &egui::Painter,
    r: Rect,
    part: &makine_sim::Part,
    label: &str,
    z: f32,
    hl: bool,
) {
    let header = match part.kind {
        PartKind::Nand => NAND_HEADER,
        PartKind::Chip(_) => PART_HEADER,
        _ => BUS_HEADER,
    };
    let radius = (5.0 * z) as u8;
    painter.rect_filled(r, CornerRadius::same(radius), PART_FILL);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEADER * z));
    painter.rect_filled(
        head,
        CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        },
        header,
    );
    painter.text(
        head.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0 * z),
        TEXT,
    );
    if hl {
        painter.rect_stroke(
            r,
            CornerRadius::same(radius),
            Stroke::new(1.5, DIM),
            StrokeKind::Outside,
        );
    }
}

/// Outline of a gate body, in screen space. `x0..x1` is the body (leads
/// excluded), `y0..y1` its height.
pub(crate) fn gate_outline(symbol: Symbol, x0: f32, x1: f32, y0: f32, y1: f32) -> Vec<Pos2> {
    let (w, h, cy) = (x1 - x0, y1 - y0, (y0 + y1) / 2.0);
    let mut pts = Vec::new();
    // Cubic Bezier sampled into points.
    let cubic = |pts: &mut Vec<Pos2>, p: [Pos2; 4], n: usize| {
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            let v = p[0].to_vec2() * (u * u * u)
                + p[1].to_vec2() * (3.0 * u * u * t)
                + p[2].to_vec2() * (3.0 * u * t * t)
                + p[3].to_vec2() * (t * t * t);
            pts.push(v.to_pos2());
        }
    };
    match symbol {
        Symbol::Not => {
            pts.extend([pos2(x0, y0), pos2(x1, cy), pos2(x0, y1)]);
        }
        Symbol::And | Symbol::Nand => {
            // Flat back, half-circle front.
            let mid = x0 + w - h / 2.0;
            pts.push(pos2(x0, y0));
            pts.push(pos2(mid, y0));
            for k in 1..=16 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 16.0;
                pts.push(pos2(mid + a.cos() * h / 2.0, cy + a.sin() * h / 2.0));
            }
            pts.push(pos2(x0, y1));
        }
        Symbol::Or | Symbol::Nor | Symbol::Xor => {
            // Curved back, pointed front.
            let back = w * 0.22;
            pts.push(pos2(x0, y0));
            cubic(
                &mut pts,
                [
                    pos2(x0, y0),
                    pos2(x0 + w * 0.55, y0),
                    pos2(x0 + w * 0.85, y0 + h * 0.2),
                    pos2(x1, cy),
                ],
                12,
            );
            cubic(
                &mut pts,
                [
                    pos2(x1, cy),
                    pos2(x0 + w * 0.85, y1 - h * 0.2),
                    pos2(x0 + w * 0.55, y1),
                    pos2(x0, y1),
                ],
                12,
            );
            cubic(
                &mut pts,
                [
                    pos2(x0, y1),
                    pos2(x0 + back, y1 - h * 0.25),
                    pos2(x0 + back, y0 + h * 0.25),
                    pos2(x0, y0),
                ],
                10,
            );
            pts.pop();
        }
        Symbol::Mux => {
            // Trapezoid, wide on the input side.
            let s = h * 0.18;
            pts.extend([
                pos2(x0, y0),
                pos2(x1, y0 + s),
                pos2(x1, y1 - s),
                pos2(x0, y1),
            ]);
        }
        Symbol::Demux => {
            let s = h * 0.18;
            pts.extend([
                pos2(x0, y0 + s),
                pos2(x1, y0),
                pos2(x1, y1),
                pos2(x0, y1 - s),
            ]);
        }
        Symbol::Box => {}
    }
    pts
}

/// A gate drawn as its logic symbol.
pub(crate) fn draw_gate(
    painter: &egui::Painter,
    r: Rect,
    g: &Geometry,
    part: &makine_sim::Part,
    z: f32,
    hl: bool,
    out_value: Option<u64>,
) {
    let lead = LEAD * z;
    let body_w = GATE_W * z;
    let x0 = r.left() + lead;
    let x1 = x0 + body_w;
    let tall = matches!(g.symbol, Symbol::Mux | Symbol::Demux);
    let (y0, y1) = if tall {
        (r.top(), r.bottom() - lead)
    } else {
        (r.top(), r.bottom())
    };
    let line = if hl { TEXT } else { GATE_LINE };
    let stroke = Stroke::new(2.0 * z.max(0.6), line);

    // Leads from the body edge to each pin.
    let at = r.min.to_vec2();
    for (off, dir) in &g.inputs {
        let p = (*off * z + at).to_pos2();
        let edge = if dir.y > 0.0 {
            // Select: up to the slanted bottom edge.
            pos2(p.x, y1 - (y1 - y0) * 0.09)
        } else {
            let mut x = x0;
            if matches!(g.symbol, Symbol::Or | Symbol::Nor | Symbol::Xor) {
                // Meet the curved back.
                let t = ((p.y - (y0 + y1) / 2.0) / ((y1 - y0) / 2.0)).abs();
                x += body_w * 0.22 * 0.75 * (1.0 - t * t);
            }
            pos2(x, p.y)
        };
        painter.line_segment([p, edge], stroke);
    }
    let out_x = if g.symbol.bubble() {
        x1 + 2.0 * BUBBLE * z
    } else {
        x1
    };
    let out_color = match out_value {
        Some(1) => ON,
        _ => line,
    };
    for off in &g.outputs {
        let p = (*off * z + at).to_pos2();
        let from = if matches!(g.symbol, Symbol::Demux) {
            pos2(x1, p.y)
        } else {
            pos2(out_x, p.y)
        };
        painter.line_segment([from, p], Stroke::new(stroke.width, out_color));
    }

    let outline = gate_outline(g.symbol, x0, x1, y0, y1);
    if g.symbol == Symbol::Xor {
        // The extra curve behind an XOR.
        let gap = 6.0 * z;
        let back = body_w * 0.22;
        let n = 10;
        let pts: Vec<Pos2> = (0..=n)
            .map(|k| {
                let t = k as f32 / n as f32;
                let u = 1.0 - t;
                let p = [
                    pos2(x0 - gap, y1),
                    pos2(x0 - gap + back, y1 - (y1 - y0) * 0.25),
                    pos2(x0 - gap + back, y0 + (y1 - y0) * 0.25),
                    pos2(x0 - gap, y0),
                ];
                (p[0].to_vec2() * (u * u * u)
                    + p[1].to_vec2() * (3.0 * u * u * t)
                    + p[2].to_vec2() * (3.0 * u * t * t)
                    + p[3].to_vec2() * (t * t * t))
                    .to_pos2()
            })
            .collect();
        painter.add(Shape::line(pts, stroke));
    }
    painter.add(Shape::convex_polygon(
        outline.clone(),
        GATE_FILL,
        Stroke::NONE,
    ));
    // Or/Xor outlines are not convex at the back; fill the notch with
    // background so the shape reads correctly.
    if matches!(g.symbol, Symbol::Or | Symbol::Nor | Symbol::Xor) {
        let back = body_w * 0.22;
        let n = 10;
        let mut notch = vec![pos2(x0, y0)];
        for k in 1..n {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            let p = [
                pos2(x0, y0),
                pos2(x0 + back, y0 + (y1 - y0) * 0.25),
                pos2(x0 + back, y1 - (y1 - y0) * 0.25),
                pos2(x0, y1),
            ];
            notch.push(
                (p[0].to_vec2() * (u * u * u)
                    + p[1].to_vec2() * (3.0 * u * u * t)
                    + p[2].to_vec2() * (3.0 * u * t * t)
                    + p[3].to_vec2() * (t * t * t))
                    .to_pos2(),
            );
        }
        notch.push(pos2(x0, y1));
        painter.add(Shape::convex_polygon(notch, BG, Stroke::NONE));
    }
    painter.add(Shape::closed_line(outline, stroke));
    if g.symbol.bubble() {
        let c = pos2(x1 + BUBBLE * z, (y0 + y1) / 2.0);
        painter.circle(c, BUBBLE * z, GATE_FILL, stroke);
    }

    let mid = pos2((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    match g.symbol {
        Symbol::Mux | Symbol::Demux => {
            painter.text(
                mid,
                Align2::CENTER_CENTER,
                if g.symbol == Symbol::Mux {
                    "MUX"
                } else {
                    "DMX"
                },
                FontId::proportional(11.0 * z),
                DIM,
            );
            if g.symbol == Symbol::Mux {
                // Which input is which.
                for (k, (off, _)) in g.inputs.iter().take(2).enumerate() {
                    let p = (*off * z + at).to_pos2();
                    painter.text(
                        pos2(x0 + 4.0 * z, p.y),
                        Align2::LEFT_CENTER,
                        k.to_string(),
                        FontId::proportional(10.0 * z),
                        DIM,
                    );
                }
            } else {
                for (k, off) in g.outputs.iter().enumerate() {
                    let p = (*off * z + at).to_pos2();
                    painter.text(
                        pos2(x1 - 4.0 * z, p.y),
                        Align2::RIGHT_CENTER,
                        k.to_string(),
                        FontId::proportional(10.0 * z),
                        DIM,
                    );
                }
            }
        }
        _ => {}
    }
    if part.repeat > 1 {
        painter.text(
            pos2(mid.x, y0 - 2.0 * z),
            Align2::CENTER_BOTTOM,
            format!("×{}", part.repeat),
            FontId::proportional(11.0 * z),
            BUS,
        );
    }
}
