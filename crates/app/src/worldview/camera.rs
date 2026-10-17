//! The orbit camera: where the player looks from, and turning screen points
//! into rays and cells.
//!
//! World cells are (x, y, z) with x east, y south and z the layer (up).
//! Drawing uses right-handed coordinates with Y up: a world point (x, y, z)
//! is drawn at (x, z, y). Positions are kept in `f64` and drawn relative to
//! a nearby cell, so the world can be huge without losing precision.

use egui::{pos2, Pos2, Rect};
use glam::{DMat4, DVec3, Mat4, Vec3};
use makine_world::Pos;

/// Field of view, vertical.
const FOV: f64 = 45.0 * std::f64::consts::PI / 180.0;
pub const MIN_DIST: f64 = 3.0;
pub const MAX_DIST: f64 = 600.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The world point looked at (x, y, height).
    pub target: DVec3,
    /// Angle around the vertical axis; at 45° the camera is south-east of
    /// the target.
    pub yaw: f64,
    /// Angle above the ground.
    pub pitch: f64,
    pub dist: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: DVec3::ZERO,
            yaw: std::f64::consts::FRAC_PI_4,
            pitch: 0.85,
            dist: 24.0,
        }
    }
}

/// World point to drawing coordinates.
pub fn render(w: DVec3) -> DVec3 {
    DVec3::new(w.x, w.z, w.y)
}

/// Drawing coordinates to a world point.
pub fn world(r: DVec3) -> DVec3 {
    DVec3::new(r.x, r.z, r.y)
}

/// A ray in world coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub from: DVec3,
    pub dir: DVec3,
}

impl Ray {
    /// Where the ray crosses height `z` (going forward), in world
    /// coordinates.
    pub fn at_height(&self, z: f64) -> Option<DVec3> {
        if self.dir.z.abs() < 1e-9 {
            return None;
        }
        let t = (z - self.from.z) / self.dir.z;
        (t > 0.0).then(|| self.from + self.dir * t)
    }
}

impl Camera {
    /// The camera's position, in world coordinates.
    pub fn eye(&self) -> DVec3 {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        self.target + DVec3::new(cp * self.yaw.cos(), cp * self.yaw.sin(), sp) * self.dist
    }

    /// The cell drawing is relative to (near the target).
    pub fn origin(&self) -> DVec3 {
        self.target.floor()
    }

    fn far(&self) -> f64 {
        self.dist * 20.0 + 400.0
    }

    /// View and projection relative to `origin`, in drawing coordinates.
    fn matrices(&self, aspect: f64, origin: DVec3) -> DMat4 {
        let (eye, target) = (render(self.eye() - origin), render(self.target - origin));
        let view = DMat4::look_at_rh(eye, target, DVec3::Y);
        let proj = DMat4::perspective_rh(FOV, aspect.max(1e-3), 0.05, self.far());
        proj * view
    }

    /// The matrix the GPU draws with: vertices are relative to
    /// [`Camera::origin`].
    pub fn view_proj(&self, aspect: f64) -> Mat4 {
        self.matrices(aspect, self.origin()).as_mat4()
    }

    /// The eye relative to [`Camera::origin`], in drawing coordinates.
    pub fn eye_rel(&self) -> Vec3 {
        render(self.eye() - self.origin()).as_vec3()
    }

    /// The ray through screen point `p` of `rect`.
    pub fn ray(&self, rect: Rect, p: Pos2) -> Ray {
        let o = self.origin();
        let inv = self
            .matrices((rect.width() / rect.height()) as f64, o)
            .inverse();
        let x = ((p.x - rect.left()) / rect.width() * 2.0 - 1.0) as f64;
        let y = (1.0 - (p.y - rect.top()) / rect.height() * 2.0) as f64;
        let near = world(inv.project_point3(DVec3::new(x, y, 0.0))) + o;
        let far = world(inv.project_point3(DVec3::new(x, y, 1.0))) + o;
        Ray {
            from: near,
            dir: (far - near).normalize(),
        }
    }

    /// Where world point `w` appears in `rect`, if in front of the camera.
    pub fn project(&self, rect: Rect, w: DVec3) -> Option<Pos2> {
        let o = self.origin();
        let m = self.matrices((rect.width() / rect.height()) as f64, o);
        let clip = m * render(w - o).extend(1.0);
        if clip.w <= 1e-6 {
            return None;
        }
        let (x, y) = (clip.x / clip.w, clip.y / clip.w);
        Some(pos2(
            rect.left() + ((x + 1.0) / 2.0) as f32 * rect.width(),
            rect.top() + ((1.0 - y) / 2.0) as f32 * rect.height(),
        ))
    }

    /// The cell on layer `layer` under screen point `p`.
    pub fn pick(&self, rect: Rect, p: Pos2, layer: i32) -> Option<Pos> {
        let hit = self.ray(rect, p).at_height(layer as f64)?;
        let (x, y) = (hit.x.floor(), hit.y.floor());
        let lim = makine_world::world::MAX_COORD as f64;
        (x.abs() <= lim && y.abs() <= lim).then(|| Pos::new(x as i32, y as i32, layer))
    }

    /// Where the centre of `cell`'s floor appears.
    #[cfg(test)]
    pub fn cell_center(&self, rect: Rect, cell: Pos) -> Option<Pos2> {
        let c = DVec3::new(cell.x as f64 + 0.5, cell.y as f64 + 0.5, cell.z as f64);
        self.project(rect, c)
    }

    /// Turn around the target.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx as f64 * 0.008;
        self.pitch = (self.pitch + dy as f64 * 0.008).clamp(0.12, 1.55);
    }

    /// Slide the target so the ground point under `from` ends up under
    /// `to` (the ground is the target's height).
    pub fn pan(&mut self, rect: Rect, from: Pos2, to: Pos2) {
        let h = self.target.z;
        if let (Some(a), Some(b)) = (
            self.ray(rect, from).at_height(h),
            self.ray(rect, to).at_height(h),
        ) {
            let d = a - b;
            // Very flat views can throw the target far away; limit a step.
            let max = self.dist * 2.0;
            self.target += if d.length() > max {
                d * (max / d.length())
            } else {
                d
            };
        }
    }

    /// Zoom by `factor` (> 1 comes closer).
    pub fn zoom(&mut self, factor: f64) {
        self.dist = (self.dist / factor).clamp(MIN_DIST, MAX_DIST);
    }

    /// Look at the box `lo..hi` (world coordinates) so it fits.
    pub fn fit(&mut self, lo: DVec3, hi: DVec3) {
        self.target = (lo + hi) / 2.0;
        let size = (hi - lo).max_element();
        self.dist = (size * 1.3 + 6.0).clamp(MIN_DIST, MAX_DIST);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    #[test]
    fn pick_inverts_projection() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 600.0));
        for cam in [
            Camera::default(),
            Camera {
                target: DVec3::new(1_000_000.0, -2_000_000.0, 500.0),
                yaw: 2.0,
                pitch: 0.4,
                dist: 60.0,
            },
        ] {
            let t = cam.target.floor();
            for (x, y, z) in [(0, 0, 0), (3, -2, 0), (-5, 7, 2), (10, 10, 1), (2, 2, -3)] {
                let cell = Pos::new(t.x as i32 + x, t.y as i32 + y, t.z as i32 + z);
                let at = cam.cell_center(rect, cell).unwrap();
                assert_eq!(cam.pick(rect, at, cell.z), Some(cell), "{cam:?}");
            }
        }
    }

    #[test]
    fn default_view_is_south_east_and_above() {
        let c = Camera::default();
        let e = c.eye();
        assert!(e.x > 0.0 && e.y > 0.0 && e.z > 0.0);
        // East (+x) is to the right on screen, south (+y) to the left.
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let p = |x: f64, y: f64| c.project(rect, DVec3::new(x, y, 0.0)).unwrap();
        assert!(p(5.0, 0.0).x > p(0.0, 0.0).x);
        assert!(p(0.0, 5.0).x < p(0.0, 0.0).x);
    }

    #[test]
    fn pan_keeps_ground_under_pointer() {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let mut c = Camera::default();
        let (a, b) = (pos2(400.0, 300.0), pos2(450.0, 320.0));
        let before = c.ray(rect, a).at_height(0.0).unwrap();
        c.pan(rect, a, b);
        let after = c.ray(rect, b).at_height(0.0).unwrap();
        assert!((before - after).length() < 1e-6);
    }
}
