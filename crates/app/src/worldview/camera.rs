//! The player's eye: a first-person camera flying through the world, like
//! Minecraft's creative mode.
//!
//! World cells are (x, y, z) with x east, y south and z the layer (up).
//! Drawing uses right-handed coordinates with Y up: a world point (x, y, z)
//! is drawn at (x, z, y). Positions are kept in `f64` and drawn relative to
//! the cell the eye is in, so the world can be huge without losing
//! precision.

use egui::{pos2, Pos2, Rect};
use glam::{DMat4, DVec3, Mat4, Vec3};
use makine_world::block::Dir;

/// Field of view, vertical.
const FOV: f64 = 70.0 * std::f64::consts::PI / 180.0;
/// Radians turned per point the mouse moves.
const LOOK_SPEED: f64 = 0.0035;
/// Cells per second.
pub const FLY_SPEED: f64 = 7.0;
/// How far blocks are drawn, in cells.
pub const VIEW_DIST: f64 = 96.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Where the eye is (x, y, height).
    pub eye: DVec3,
    /// Look direction around the vertical axis: 0 looks east, π/2 south.
    pub yaw: f64,
    /// Look direction above the horizon (negative looks down).
    pub pitch: f64,
}

impl Default for Camera {
    fn default() -> Self {
        let mut c = Self {
            eye: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        };
        c.look_from(DVec3::new(-6.0, -6.0, 7.0), DVec3::new(0.5, 0.5, 0.0));
        c
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
    #[cfg(test)]
    pub fn at_height(&self, z: f64) -> Option<DVec3> {
        if self.dir.z.abs() < 1e-9 {
            return None;
        }
        let t = (z - self.from.z) / self.dir.z;
        (t > 0.0).then(|| self.from + self.dir * t)
    }
}

impl Camera {
    /// Put the eye at `eye`, looking at `target`.
    pub fn look_from(&mut self, eye: DVec3, target: DVec3) {
        self.eye = eye;
        self.look_at(target);
    }

    /// Turn to look at `target`.
    pub fn look_at(&mut self, target: DVec3) {
        let d = target - self.eye;
        self.yaw = d.y.atan2(d.x);
        self.pitch = d.z.atan2(d.truncate().length()).clamp(-1.55, 1.55);
    }

    /// Where the eye looks, in world coordinates (unit length).
    pub fn forward(&self) -> DVec3 {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        DVec3::new(cp * self.yaw.cos(), cp * self.yaw.sin(), sp)
    }

    /// The horizontal direction closest to the look direction.
    pub fn facing(&self) -> Dir {
        let f = self.forward();
        if f.x.abs() >= f.y.abs() {
            if f.x >= 0.0 {
                Dir::East
            } else {
                Dir::West
            }
        } else if f.y >= 0.0 {
            Dir::South
        } else {
            Dir::North
        }
    }

    /// Turn by a mouse movement (in points).
    pub fn turn(&mut self, dx: f32, dy: f32) {
        self.yaw = (self.yaw + dx as f64 * LOOK_SPEED).rem_euclid(std::f64::consts::TAU);
        self.pitch = (self.pitch - dy as f64 * LOOK_SPEED).clamp(-1.55, 1.55);
    }

    /// Fly: `forward` and `right` along the ground (whatever the pitch),
    /// `up` straight up, each -1..1, for `dist` cells.
    pub fn fly(&mut self, forward: f64, right: f64, up: f64, dist: f64) {
        let ahead = DVec3::new(self.yaw.cos(), self.yaw.sin(), 0.0);
        // Seen from above, with y pointing south, right of east is south.
        let side = DVec3::new(-self.yaw.sin(), self.yaw.cos(), 0.0);
        let m = ahead * forward + side * right + DVec3::Z * up;
        if m.length() > 1e-9 {
            self.eye += m.normalize() * dist;
            let lim = makine_world::world::MAX_COORD as f64;
            self.eye = self.eye.clamp(DVec3::splat(-lim), DVec3::splat(lim));
        }
    }

    /// The cell drawing is relative to (the eye's).
    pub fn origin(&self) -> DVec3 {
        self.eye.floor()
    }

    /// View and projection relative to `origin`, in drawing coordinates.
    fn matrices(&self, aspect: f64, origin: DVec3) -> DMat4 {
        let eye = render(self.eye - origin);
        let view = DMat4::look_to_rh(eye, render(self.forward()), DVec3::Y);
        let proj = DMat4::perspective_rh(FOV, aspect.max(1e-3), 0.05, VIEW_DIST * 2.5);
        proj * view
    }

    /// The matrix the GPU draws with: vertices are relative to
    /// [`Camera::origin`].
    pub fn view_proj(&self, aspect: f64) -> Mat4 {
        self.matrices(aspect, self.origin()).as_mat4()
    }

    /// The eye relative to [`Camera::origin`], in drawing coordinates.
    pub fn eye_rel(&self) -> Vec3 {
        render(self.eye - self.origin()).as_vec3()
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
            from: self.eye,
            dir: (far - near).normalize(),
        }
    }

    /// Where world point `w` appears in `rect`, if in front of the eye.
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

    /// Look at the box `lo..hi` (world coordinates) from above its
    /// south-east, so all of it shows.
    pub fn fit(&mut self, lo: DVec3, hi: DVec3) {
        let centre = (lo + hi) / 2.0;
        let size = (hi - lo).max_element();
        let back = DVec3::new(-1.0, -1.0, 1.1).normalize() * -1.0;
        let back = DVec3::new(back.x.abs(), back.y.abs(), back.z.abs());
        self.look_from(centre + back * (size * 0.9 + 6.0), centre);
    }

    /// The cell on layer `layer` under screen point `p`.
    #[cfg(test)]
    pub fn pick(&self, rect: Rect, p: Pos2, layer: i32) -> Option<makine_world::Pos> {
        let hit = self.ray(rect, p).at_height(layer as f64)?;
        Some(makine_world::Pos::new(
            hit.x.floor() as i32,
            hit.y.floor() as i32,
            layer,
        ))
    }

    /// Where the centre of `cell`'s floor appears.
    #[cfg(test)]
    pub fn cell_center(&self, rect: Rect, cell: makine_world::Pos) -> Option<Pos2> {
        let c = DVec3::new(cell.x as f64 + 0.5, cell.y as f64 + 0.5, cell.z as f64);
        self.project(rect, c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;
    use makine_world::Pos;

    #[test]
    fn pick_inverts_projection() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 600.0));
        let far = DVec3::new(1_000_000.0, -2_000_000.0, 500.0);
        let mut cams = vec![Camera::default()];
        let mut c = Camera::default();
        c.look_from(far + DVec3::new(-8.0, 5.0, 12.0), far);
        cams.push(c);
        for cam in cams {
            let t = (cam.eye + cam.forward() * 10.0).floor();
            for (x, y, z) in [(0, 0, -3), (3, -2, -4), (-2, 2, -3), (1, 1, -5)] {
                let cell = Pos::new(t.x as i32 + x, t.y as i32 + y, t.z as i32 + z);
                let at = cam.cell_center(rect, cell).unwrap();
                assert_eq!(cam.pick(rect, at, cell.z), Some(cell), "{cam:?}");
            }
        }
    }

    #[test]
    fn the_centre_ray_looks_forward() {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let c = Camera::default();
        let r = c.ray(rect, rect.center());
        assert!((r.dir - c.forward()).length() < 1e-9);
        assert_eq!(r.from, c.eye);
    }

    #[test]
    fn mouse_turns_and_keys_fly() {
        let mut c = Camera {
            eye: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        };
        assert_eq!(c.facing(), Dir::East);
        // Moving the mouse right turns right: from east towards south.
        c.turn(100.0, 0.0);
        assert!(c.yaw > 0.0);
        c.turn(-100.0, 0.0);
        // Moving it down looks down.
        c.turn(0.0, 50.0);
        assert!(c.pitch < 0.0);
        // Forward follows the ground even when looking down.
        c.fly(1.0, 0.0, 0.0, 2.0);
        assert!((c.eye - DVec3::new(2.0, 0.0, 0.0)).length() < 1e-9);
        // Right of east is south (+y); up is +z.
        c.fly(0.0, 1.0, 0.0, 1.0);
        c.fly(0.0, 0.0, 1.0, 3.0);
        assert!((c.eye - DVec3::new(2.0, 1.0, 3.0)).length() < 1e-9);
    }

    #[test]
    fn look_at_turns_towards_the_point() {
        let mut c = Camera::default();
        c.look_from(DVec3::new(0.0, 0.0, 5.0), DVec3::new(0.0, 10.0, 5.0));
        assert_eq!(c.facing(), Dir::South);
        assert!(c.pitch.abs() < 1e-9);
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let p = c.project(rect, DVec3::new(0.0, 10.0, 5.0)).unwrap();
        assert!((p - rect.center()).length() < 1e-3);
        // East is on the right when looking south? No: on the left.
        let east = c.project(rect, DVec3::new(3.0, 10.0, 5.0)).unwrap();
        assert!(east.x < rect.center().x);
    }
}
