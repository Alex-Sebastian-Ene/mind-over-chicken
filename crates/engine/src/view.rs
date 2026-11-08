//! The home world is bigger than the screen (#129): a `world_w × world_h` torus (640×360) seen
//! through a `view_w × view_h` window (320×180, the chunky low-res canvas). This module holds
//! the wrap maths both renderers share:
//!
//! - [`ViewCamera`]: the 2D camera (exported to JS). Where the window sits, how a screen pixel
//!   maps to the world and back, which source rectangles to blit across the seams, and at which
//!   offsets to repeat overlays and lights so they wrap with the map.
//! - [`WorldGrid`]: the world's heightmap at its own size, sampled with wrap on both axes, for
//!   the 3D raycast, billboards and lights.

use wasm_bindgen::prelude::*;

/// `v` wrapped into `[0, size)`.
pub fn wrap(v: f32, size: f32) -> f32 {
    if size <= 0.0 {
        return v;
    }
    let r = v.rem_euclid(size);
    // rem_euclid can return `size` itself for tiny negative inputs (float rounding).
    if r >= size {
        0.0
    } else {
        r
    }
}

/// The shorter signed way round from `b` to `a` on a ring `size` long.
pub fn wrap_delta(a: f32, b: f32, size: f32) -> f32 {
    if size <= 0.0 {
        return a - b;
    }
    let d = (a - b).rem_euclid(size);
    if d > size * 0.5 {
        d - size
    } else {
        d
    }
}

/// Segments `[src, len, dst]` that cover `[0, view)` on screen from a ring `world` long, starting
/// at world coordinate `start` (already wrapped). Works for any view/world ratio.
fn segments(start: f32, view: f32, world: f32) -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    if world <= 0.0 || view <= 0.0 {
        return out;
    }
    let mut src = wrap(start, world);
    let mut dst = 0.0f32;
    while dst < view - 1e-3 && out.len() < 64 {
        let len = (world - src).min(view - dst);
        out.push([src, len, dst]);
        dst += len;
        src = 0.0;
    }
    out
}

/// Integer `k` values so that `[off + k*world, off + (k+1)*world)` overlaps `[lo, hi)`.
fn copies_1d(off: f32, world: f32, lo: f32, hi: f32) -> Vec<f32> {
    let mut out = Vec::new();
    if world <= 0.0 {
        out.push(off);
        return out;
    }
    let k0 = ((lo - off) / world).floor() as i32 - 1;
    let k1 = ((hi - off) / world).ceil() as i32 + 1;
    for k in k0..=k1 {
        let a = off + k as f32 * world;
        if a < hi && a + world > lo {
            out.push(a);
        }
    }
    out
}

/// The 2D camera over the home world. `pan(dx, dy)` moves the map under the window like a drag
/// (the content follows the pointer); everything wraps both ways (a torus, #111).
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct ViewCamera {
    world_w: f32,
    world_h: f32,
    view_w: f32,
    view_h: f32,
    /// Centre of the window in world pixels (kept wrapped, sub-pixel so slow pans accumulate).
    cx: f32,
    cy: f32,
}

#[wasm_bindgen]
impl ViewCamera {
    #[wasm_bindgen(constructor)]
    pub fn new(world_w: f32, world_h: f32, view_w: f32, view_h: f32) -> ViewCamera {
        ViewCamera { world_w, world_h, view_w, view_h, cx: world_w * 0.5, cy: world_h * 0.5 }
    }

    /// A new world size (another map was loaded). The centre stays put, wrapped into it.
    pub fn set_world(&mut self, world_w: f32, world_h: f32) {
        self.world_w = world_w;
        self.world_h = world_h;
        self.cx = wrap(self.cx, world_w);
        self.cy = wrap(self.cy, world_h);
    }

    pub fn world_w(&self) -> f32 { self.world_w }
    pub fn world_h(&self) -> f32 { self.world_h }
    pub fn view_w(&self) -> f32 { self.view_w }
    pub fn view_h(&self) -> f32 { self.view_h }
    pub fn center_x(&self) -> f32 { self.cx }
    pub fn center_y(&self) -> f32 { self.cy }

    pub fn set_center(&mut self, x: f32, y: f32) {
        self.cx = wrap(x, self.world_w);
        self.cy = wrap(y, self.world_h);
    }

    /// Drag the map by (dx, dy) world pixels: the content moves with the pointer, so the window
    /// moves the other way.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.set_center(self.cx - dx, self.cy - dy);
    }

    /// World x at the window's left edge, snapped to whole pixels so the 2D map stays crisp.
    /// Every 2D conversion uses the snapped edge, so what is drawn and what is clicked agree.
    pub fn left(&self) -> f32 {
        wrap((self.cx - self.view_w * 0.5).round(), self.world_w)
    }

    /// World y at the window's top edge (snapped, like `left`).
    pub fn top(&self) -> f32 {
        wrap((self.cy - self.view_h * 0.5).round(), self.world_h)
    }

    /// Screen x (view pixels, 0..view_w) → world x.
    pub fn to_world_x(&self, sx: f32) -> f32 {
        wrap(self.left() + sx, self.world_w)
    }

    /// Screen y (view pixels, 0..view_h) → world y.
    pub fn to_world_y(&self, sy: f32) -> f32 {
        wrap(self.top() + sy, self.world_h)
    }

    /// World x → screen x of its copy nearest the window's centre (may fall off screen).
    pub fn to_screen_x(&self, wx: f32) -> f32 {
        let mid = self.left() + self.view_w * 0.5;
        wrap_delta(wx, mid, self.world_w) + self.view_w * 0.5
    }

    /// World y → screen y of its copy nearest the window's centre.
    pub fn to_screen_y(&self, wy: f32) -> f32 {
        let mid = self.top() + self.view_h * 0.5;
        wrap_delta(wy, mid, self.world_h) + self.view_h * 0.5
    }

    /// The shorter signed east-west way from b to a.
    pub fn delta_x(&self, a: f32, b: f32) -> f32 {
        wrap_delta(a, b, self.world_w)
    }

    /// The shorter signed north-south way from b to a.
    pub fn delta_y(&self, a: f32, b: f32) -> f32 {
        wrap_delta(a, b, self.world_h)
    }

    /// The rectangles that paint the window from a world-sized canvas, cut at the seams:
    /// `[sx, sy, w, h, dx, dy]` per rectangle (1 to 4 of them, more only if the world is
    /// smaller than the view). Draw each with `drawImage(world, sx, sy, w, h, dx, dy, w, h)`.
    pub fn blits(&self) -> Vec<f32> {
        let xs = segments(self.left(), self.view_w, self.world_w);
        let ys = segments(self.top(), self.view_h, self.world_h);
        let mut out = Vec::with_capacity(xs.len() * ys.len() * 6);
        for y in &ys {
            for x in &xs {
                out.extend_from_slice(&[x[0], y[0], x[1], y[1], x[2], y[2]]);
            }
        }
        out
    }

    /// Translations `[ox, oy]` (pairs) that take world coordinates to screen coordinates, one per
    /// copy of the world that reaches the window (widened by `margin` pixels on every side).
    /// Overlays drawn in world units repeat at each, so labels and lights wrap with the map.
    pub fn copies(&self, margin: f32) -> Vec<f32> {
        let xs = copies_1d(-self.left(), self.world_w, -margin, self.view_w + margin);
        let ys = copies_1d(-self.top(), self.world_h, -margin, self.view_h + margin);
        let mut out = Vec::with_capacity(xs.len() * ys.len() * 2);
        for oy in &ys {
            for ox in &xs {
                out.push(*ox);
                out.push(*oy);
            }
        }
        out
    }
}

/// The world's heights (row-major `w × h`, 0..1, sea level 0.33) at the world's own size.
#[derive(Clone, Debug)]
pub struct WorldGrid {
    pub w: usize,
    pub h: usize,
    pub heights: Vec<f32>,
}

impl WorldGrid {
    pub fn new(w: usize, h: usize, heights: Vec<f32>) -> Self {
        debug_assert_eq!(heights.len(), w * h);
        WorldGrid { w, h, heights }
    }

    pub fn width(&self) -> f32 { self.w as f32 }
    pub fn depth(&self) -> f32 { self.h as f32 }

    /// Bilinear height at (x, z), wrapping both ways.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        if self.w == 0 || self.h == 0 || self.heights.len() < self.w * self.h {
            return 0.33;
        }
        let wx = wrap(x, self.width());
        let wz = wrap(z, self.depth());
        let x0 = (wx.floor() as usize).min(self.w - 1);
        let z0 = (wz.floor() as usize).min(self.h - 1);
        let x1 = (x0 + 1) % self.w;
        let z1 = (z0 + 1) % self.h;
        let fx = wx - x0 as f32;
        let fz = wz - z0 as f32;
        let at = |x: usize, z: usize| self.heights[z * self.w + x];
        let h0 = at(x0, z0) * (1.0 - fx) + at(x1, z0) * fx;
        let h1 = at(x0, z1) * (1.0 - fx) + at(x1, z1) * fx;
        h0 * (1.0 - fz) + h1 * fz
    }

    /// (x, z) moved to the copy of the world nearest (ref_x, ref_z).
    pub fn nearest_copy(&self, x: f32, z: f32, ref_x: f32, ref_z: f32) -> (f32, f32) {
        (
            ref_x + wrap_delta(x, ref_x, self.width()),
            ref_z + wrap_delta(z, ref_z, self.depth()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> ViewCamera {
        ViewCamera::new(640.0, 360.0, 320.0, 180.0)
    }

    #[test]
    fn wraps_both_ways() {
        assert_eq!(wrap(-1.0, 640.0), 639.0);
        assert_eq!(wrap(640.0, 640.0), 0.0);
        assert_eq!(wrap(-1e-7, 640.0), 0.0);
        assert_eq!(wrap_delta(630.0, 10.0, 640.0), -20.0);
        assert_eq!(wrap_delta(10.0, 630.0, 640.0), 20.0);
        assert_eq!(wrap_delta(350.0, 10.0, 360.0), -20.0);
    }

    #[test]
    fn starts_centred_and_round_trips_screen_and_world() {
        let c = cam();
        assert_eq!((c.left(), c.top()), (160.0, 90.0));
        for &(sx, sy) in &[(0.0, 0.0), (160.0, 90.0), (319.0, 179.0)] {
            let (wx, wy) = (c.to_world_x(sx), c.to_world_y(sy));
            assert!((c.to_screen_x(wx) - sx).abs() < 1e-4 && (c.to_screen_y(wy) - sy).abs() < 1e-4);
        }
    }

    #[test]
    fn pan_moves_the_content_with_the_pointer_and_wraps() {
        let mut c = cam();
        let before = c.to_world_x(100.0);
        c.pan(10.0, 0.0);
        // The point that was under screen x=100 is now under x=110.
        assert!((c.to_world_x(110.0) - before).abs() < 1e-4);
        // Past the seam, both axes.
        c.set_center(5.0, 3.0);
        c.pan(10.0, 6.0);
        assert!((c.center_x() - 635.0).abs() < 1e-4 && (c.center_y() - 357.0).abs() < 1e-4);
        assert_eq!(c.left(), 475.0);
        assert_eq!(c.top(), 267.0);
        // The centre column (screen x=160) is world x 635; 10 px right of it wraps past the seam.
        assert!((c.to_world_x(160.0) - 635.0).abs() < 1e-4);
        assert!((c.to_world_x(170.0) - 5.0).abs() < 1e-4);
    }

    #[test]
    fn blits_cover_the_view_exactly_across_seams() {
        let mut c = cam();
        // Inside the world: one rectangle.
        assert_eq!(c.blits(), vec![160.0, 90.0, 320.0, 180.0, 0.0, 0.0]);
        // Straddling both seams: four rectangles that tile the 320×180 window.
        c.set_center(620.0, 350.0);
        let b = c.blits();
        assert_eq!(b.len(), 24);
        let area: f32 = b.chunks(6).map(|r| r[2] * r[3]).sum();
        assert_eq!(area, 320.0 * 180.0);
        // First rect starts at the window's top-left in the world, drawn at the screen origin.
        assert_eq!(&b[0..6], &[460.0, 260.0, 180.0, 100.0, 0.0, 0.0]);
        // The wrapped part comes from the world's origin, drawn after the seam.
        assert_eq!(&b[18..24], &[0.0, 0.0, 140.0, 80.0, 180.0, 100.0]);
    }

    #[test]
    fn copies_place_a_world_point_on_screen_at_each_visible_wrap() {
        let mut c = cam();
        c.set_center(620.0, 350.0); // left 460, top 260
        let cp = c.copies(0.0);
        // World point (5, 5) is just past both seams: on screen at (185, 105).
        let hits: Vec<(f32, f32)> = cp.chunks(2).map(|o| (5.0 + o[0], 5.0 + o[1])).filter(|&(x, y)| (0.0..320.0).contains(&x) && (0.0..180.0).contains(&y)).collect();
        assert_eq!(hits, vec![(185.0, 105.0)]);
        // And the screen→world mapping agrees.
        assert_eq!((c.to_world_x(185.0), c.to_world_y(105.0)), (5.0, 5.0));
        // Inside the world, one copy suffices without margin.
        c.set_center(320.0, 180.0);
        assert_eq!(c.copies(0.0), vec![-160.0, -90.0]);
    }

    #[test]
    fn a_world_smaller_than_the_view_still_tiles() {
        let c = ViewCamera::new(200.0, 100.0, 320.0, 180.0);
        let area: f32 = c.blits().chunks(6).map(|r| r[2] * r[3]).sum();
        assert_eq!(area, 320.0 * 180.0);
    }

    #[test]
    fn grid_samples_with_wrap() {
        let mut hs = vec![0.0f32; 4 * 2];
        hs[3] = 1.0; // (x=3, z=0)
        let g = WorldGrid::new(4, 2, hs);
        assert_eq!(g.height_at(3.0, 0.0), 1.0);
        assert_eq!(g.height_at(-1.0, 2.0), 1.0); // wraps to (3, 0)
        assert!((g.height_at(3.5, 0.0) - 0.5).abs() < 1e-6); // halfway to x=0 across the seam
        let (x, z) = g.nearest_copy(3.5, 1.5, 0.2, 0.2);
        assert!((x + 0.5).abs() < 1e-6 && (z + 0.5).abs() < 1e-6);
    }
}
