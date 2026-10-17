//! Manual wire routes. Rendering and hit testing use the same polyline.
use super::Layout;
use egui::{pos2, Pos2};
use makine_world::{End, World};

pub fn points(world: &World, l: &Layout, from: End, to: End) -> Option<Vec<Pos2>> {
    let s = world.wire_style(to)?;
    if s.points.is_empty() {
        return None;
    }
    let mut out = vec![l.source_pos(from)?];
    for p in s
        .points
        .iter()
        .map(|p| l.to_screen(pos2(p.x, p.y)))
        .chain([l.sink_pos(to)?])
    {
        let a = *out.last()?;
        out.push(pos2(p.x, a.y));
        out.push(p);
    }
    Some(out)
}

pub fn wire_distance(world: &World, l: &Layout, from: End, to: End, p: Pos2) -> Option<f32> {
    if let Some(pts) = points(world, l, from, to) {
        Some(
            pts.windows(2)
                .map(|ab| {
                    let d = ab[1] - ab[0];
                    let t = ((p - ab[0]).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
                    p.distance(ab[0] + d * t)
                })
                .fold(f32::INFINITY, f32::min),
        )
    } else {
        let c = crate::canvas::draw::bezier_points(
            l.source_pos(from)?,
            l.sink_pos(to)?,
            l.sink_dir(to),
        );
        Some(crate::canvas::draw::bezier_distance(c, p))
    }
}
