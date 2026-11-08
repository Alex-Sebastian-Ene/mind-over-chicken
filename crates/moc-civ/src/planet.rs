//! Planet maths for a planet of any size.
//!
//! The planet is a torus (#111): it wraps east–west *and* north–south, so every distance,
//! neighbour, field, walk and camera takes the short way round in both axes.
//!
//! `moc_planet` fixes the planet at `PW`×`PH` (640×320) in its wrap helpers, `Camera`,
//! `Framebuffer::blit_planet` and `Field`. The civilisation sandbox runs on a bigger planet
//! (default 1280×640, #50), so this module carries the same maths with the size as a value:
//! [`Dims`] (wraps both ways), [`Cam`] (the 320×180 view, panning across
//! the seam), [`blit`] (seam-safe copy of the visible window) and [`LocalField`] (a wrap-aware
//! BFS distance field limited to a window round its target, so building one stays cheap on a
//! big planet). The 320×180 view itself is `moc_planet::fb::Framebuffer`.

use moc_planet::fb::Framebuffer;
use moc_planet::{VH, VW};

/// Planet size in tiles (one tile = one pixel of the planet map).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dims {
    pub w: usize,
    pub h: usize,
}

impl Dims {
    /// The sandbox planet: four times the area of `moc_planet`'s 640×320.
    pub const DEFAULT: Dims = Dims { w: 1280, h: 640 };
    /// `moc_planet`'s size, for tests and comparison.
    pub const SMALL: Dims = Dims { w: moc_planet::PW, h: moc_planet::PH };

    pub fn new(w: usize, h: usize) -> Dims {
        // At least the view in each direction, so the camera always fits.
        Dims { w: w.max(VW * 2), h: h.max(VH + 20) }
    }

    #[inline]
    pub fn n(&self) -> usize {
        self.w * self.h
    }

    #[inline]
    pub fn wrap_x(&self, x: i32) -> i32 {
        x.rem_euclid(self.w as i32)
    }

    #[inline]
    pub fn wrap_xf(&self, x: f32) -> f32 {
        let w = self.w as f32;
        let r = x.rem_euclid(w);
        if r >= w {
            0.0
        } else {
            r
        }
    }

    /// Shortest signed east–west distance from `a` to `b`.
    #[inline]
    pub fn dx(&self, a: f32, b: f32) -> f32 {
        let w = self.w as f32;
        let mut d = (b - a).rem_euclid(w);
        if d > w / 2.0 {
            d -= w;
        }
        d
    }

    #[inline]
    pub fn wrap_y(&self, y: i32) -> i32 {
        y.rem_euclid(self.h as i32)
    }

    #[inline]
    pub fn wrap_yf(&self, y: f32) -> f32 {
        let h = self.h as f32;
        let r = y.rem_euclid(h);
        if r >= h {
            0.0
        } else {
            r
        }
    }

    /// Shortest signed north–south distance from `a` to `b`.
    #[inline]
    pub fn dy(&self, a: f32, b: f32) -> f32 {
        let h = self.h as f32;
        let mut d = (b - a).rem_euclid(h);
        if d > h / 2.0 {
            d -= h;
        }
        d
    }

    /// Shortest signed north–south tile difference from `a` to `b`.
    #[inline]
    pub fn dyi(&self, a: i32, b: i32) -> i32 {
        let h = self.h as i32;
        let mut d = (b - a).rem_euclid(h);
        if d > h / 2 {
            d -= h;
        }
        d
    }

    /// Shortest signed east–west tile difference from `a` to `b`.
    #[inline]
    pub fn dxi(&self, a: i32, b: i32) -> i32 {
        let w = self.w as i32;
        let mut d = (b - a).rem_euclid(w);
        if d > w / 2 {
            d -= w;
        }
        d
    }

    /// Distance the short way round (both ways: the planet is a torus).
    #[inline]
    pub fn dist(&self, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
        let dx = self.dx(ax, bx);
        let dy = self.dy(ay, by);
        (dx * dx + dy * dy).sqrt()
    }

    /// Chebyshev tile distance the short way round.
    #[inline]
    pub fn cheb(&self, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        self.dxi(ax, bx).abs().max(self.dyi(ay, by).abs())
    }

    /// Index of a tile (both coordinates wrap).
    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> usize {
        self.wrap_y(y) as usize * self.w + self.wrap_x(x) as usize
    }

    /// Every row is on the planet: it wraps north–south (#111). Kept so callers read the same.
    #[inline]
    pub fn in_y(&self, _y: i32) -> bool {
        true
    }
}

/// The player's 320×180 window onto the planet: pans freely both ways round the torus.
#[derive(Clone, Copy, Debug)]
pub struct Cam {
    /// Planet x of the view's left edge (wraps).
    pub x: f32,
    /// Planet y of the view's top edge (wraps).
    pub y: f32,
    pub d: Dims,
}

impl Cam {
    pub fn centred_on(d: Dims, px: f32, py: f32) -> Cam {
        let mut c = Cam { x: 0.0, y: 0.0, d };
        c.centre(px, py);
        c
    }

    pub fn centre(&mut self, px: f32, py: f32) {
        self.x = self.d.wrap_xf(px - VW as f32 / 2.0);
        self.y = self.d.wrap_yf(py - VH as f32 / 2.0);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.x = self.d.wrap_xf(self.x + dx);
        self.y = self.d.wrap_yf(self.y + dy);
    }

    /// Planet → screen, the short way round from the *centre* of the view (#230). Every point
    /// in the view comes out inside it, on any planet size: measuring from the view's top-left
    /// corner instead flipped the bottom rows of a 320-high planet (half its height, 160, is
    /// less than the view's 180) to the far side, so villages and agents there vanished.
    pub fn to_screen(&self, px: f32, py: f32) -> (f32, f32) {
        let (hw, hh) = (VW as f32 / 2.0, VH as f32 / 2.0);
        let sx = self.d.dx(self.x + hw, px) + hw;
        let sy = self.d.dy(self.y + hh, py) + hh;
        (sx, sy)
    }

    /// Screen → planet.
    pub fn to_planet(&self, sx: f32, sy: f32) -> (f32, f32) {
        (self.d.wrap_xf(self.x + sx), self.d.wrap_yf(self.y + sy))
    }

    pub fn ix(&self) -> i32 {
        self.x.floor() as i32
    }

    pub fn iy(&self) -> i32 {
        self.y.floor() as i32
    }
}

/// Copy the window the camera sees out of a full-planet RGBA layer, across both seams.
pub fn blit(fb: &mut Framebuffer, layer: &[u8], d: Dims, cam: &Cam) {
    debug_assert_eq!(layer.len(), d.n() * 4);
    let (cx, cy) = (cam.ix(), cam.iy());
    for y in 0..fb.h {
        let py = d.wrap_y(cy + y as i32) as usize;
        let src_row = &layer[py * d.w * 4..(py + 1) * d.w * 4];
        let dst_row = &mut fb.data[y * fb.w * 4..(y + 1) * fb.w * 4];
        let mut x = 0usize;
        while x < fb.w {
            let sx = d.wrap_x(cx + x as i32) as usize;
            let run = (d.w - sx).min(fb.w - x);
            dst_row[x * 4..(x + run) * 4].copy_from_slice(&src_row[sx * 4..(sx + run) * 4]);
            x += run;
        }
    }
}

/// A wrap-aware breadth-first distance field toward one target, over passable tiles within
/// `r` tiles (Chebyshev) of it. Movers outside the window steer greedily until they enter it.
pub struct LocalField {
    pub tx: i32,
    pub ty: i32,
    r: i32,
    side: usize,
    d: Dims,
    dist: Vec<u16>,
}

const UNREACHED: u16 = u16::MAX;

impl LocalField {
    /// The target itself is always the source, even if impassable (a village on a coast).
    pub fn build(d: Dims, tx: i32, ty: i32, r: i32, passable: impl Fn(i32, i32) -> bool) -> LocalField {
        let tx = d.wrap_x(tx);
        let ty = d.wrap_y(ty);
        let side = (2 * r + 1) as usize;
        let mut dist = vec![UNREACHED; side * side];
        let mut queue = std::collections::VecDeque::with_capacity(side * 4);
        let s = r as usize * side + r as usize;
        dist[s] = 0;
        queue.push_back(s);
        while let Some(i) = queue.pop_front() {
            let (lx, ly) = ((i % side) as i32, (i / side) as i32);
            let nd = dist[i] + 1;
            for oy in -1..=1 {
                let ny = ly + oy;
                let py = d.wrap_y(ty - r + ny);
                if ny < 0 || ny >= side as i32 {
                    continue;
                }
                for ox in -1..=1 {
                    let nx = lx + ox;
                    if (ox == 0 && oy == 0) || nx < 0 || nx >= side as i32 {
                        continue;
                    }
                    let j = ny as usize * side + nx as usize;
                    if dist[j] != UNREACHED || !passable(d.wrap_x(tx - r + nx), py) {
                        continue;
                    }
                    dist[j] = nd;
                    queue.push_back(j);
                }
            }
        }
        LocalField { tx, ty, r, side, d, dist }
    }

    #[inline]
    fn local(&self, x: i32, y: i32) -> Option<usize> {
        let lx = self.d.dxi(self.tx, x) + self.r;
        let ly = self.d.dyi(self.ty, y) + self.r;
        if lx < 0 || ly < 0 || lx >= self.side as i32 || ly >= self.side as i32 {
            return None;
        }
        Some(ly as usize * self.side + lx as usize)
    }

    /// Steps to the target; -1 if unreachable or outside the window.
    pub fn at(&self, x: i32, y: i32) -> i32 {
        match self.local(x, y) {
            Some(i) if self.dist[i] != UNREACHED => self.dist[i] as i32,
            _ => -1,
        }
    }

    pub fn reachable(&self, x: i32, y: i32) -> bool {
        self.at(x, y) >= 0
    }

    /// The neighbouring tile one step closer to the target.
    pub fn next_step(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let here = self.at(x, y);
        if here <= 0 {
            return None;
        }
        let mut best = (here, None);
        for oy in -1..=1 {
            for ox in -1..=1 {
                let v = self.at(x + ox, y + oy);
                if v >= 0 && v < best.0 {
                    best = (v, Some((self.d.wrap_x(x + ox), self.d.wrap_y(y + oy))));
                }
            }
        }
        best.1
    }
}

/// Recently used fields toward villages, built on a per-step budget.
pub struct Paths {
    fields: Vec<(LocalField, u64)>,
    tick: u64,
    budget: u32,
    pub builds: u32,
}

pub const FIELD_RADIUS: i32 = 80;
const FIELD_CAP: usize = 48;

impl Default for Paths {
    fn default() -> Self {
        Self::new()
    }
}

impl Paths {
    pub fn new() -> Paths {
        Paths { fields: Vec::new(), tick: 0, budget: 0, builds: 0 }
    }

    /// Allow `n` new fields this step.
    pub fn refill(&mut self, n: u32) {
        self.budget = n;
        self.tick += 1;
    }

    /// The field toward (x, y): cached, or built now if this step's budget allows.
    pub fn get(&mut self, d: Dims, walk: &[bool], x: i32, y: i32) -> Option<&LocalField> {
        let (x, y) = (d.wrap_x(x), d.wrap_y(y));
        let tick = self.tick;
        if let Some(i) = self.fields.iter().position(|(f, _)| f.tx == x && f.ty == y) {
            self.fields[i].1 = tick;
            return Some(&self.fields[i].0);
        }
        if self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        self.builds += 1;
        let f = LocalField::build(d, x, y, FIELD_RADIUS, |px, py| walk[py as usize * d.w + px as usize]);
        if self.fields.len() >= FIELD_CAP {
            let oldest = self.fields.iter().enumerate().min_by_key(|(_, (_, t))| *t).map(|(i, _)| i).unwrap_or(0);
            self.fields.swap_remove(oldest);
        }
        self.fields.push((f, tick));
        self.fields.last().map(|(f, _)| f)
    }

    pub fn has(&self, d: Dims, x: i32, y: i32) -> bool {
        let (x, y) = (d.wrap_x(x), d.wrap_y(y));
        self.fields.iter().any(|(f, _)| f.tx == x && f.ty == y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_on_a_big_planet() {
        let d = Dims::DEFAULT;
        assert_eq!(d.wrap_x(-1), 1279);
        assert_eq!(d.dxi(1275, 4), 9);
        assert!((d.dist(1278.0, 10.0, 2.0, 10.0) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn camera_pans_round_both_ways() {
        let d = Dims::DEFAULT;
        let mut c = Cam::centred_on(d, 5.0, 0.0);
        assert!(c.x > (d.w - VW) as f32);
        assert!(c.y > (d.h - VH) as f32, "centred on the top row, the view starts below the seam ({})", c.y);
        c.pan(0.0, d.h as f32 * 3.0 + 10.0);
        assert!((c.y - d.wrap_yf(d.h as f32 - VH as f32 / 2.0 + 10.0)).abs() < 1e-3, "{}", c.y);
        let c = Cam { x: d.w as f32 - 20.0, y: 40.0, d };
        let (sx, sy) = c.to_screen(10.0, 100.0);
        assert!((sx - 30.0).abs() < 1e-4 && (sy - 60.0).abs() < 1e-4);
        let (px, py) = c.to_planet(sx, sy);
        assert!((px - 10.0).abs() < 1e-4 && (py - 100.0).abs() < 1e-4);
    }

    #[test]
    fn every_point_in_view_lands_on_screen() {
        // #230: on a 320-high planet half the planet (160) is less than the view (180).
        let d = Dims::new(640, 320);
        for &(cx, cy) in &[(0.0, 0.0), (630.0, 300.0), (100.0, 150.0), (639.5, 319.5)] {
            let c = Cam { x: cx, y: cy, d };
            for sy in 0..VH {
                for sx in (0..VW).step_by(7) {
                    let (px, py) = c.to_planet(sx as f32 + 0.5, sy as f32 + 0.5);
                    let (rx, ry) = c.to_screen(px, py);
                    assert!((rx - (sx as f32 + 0.5)).abs() < 1e-2 && (ry - (sy as f32 + 0.5)).abs() < 1e-2, "({sx},{sy}) from cam ({cx},{cy}) came back at ({rx},{ry})");
                }
            }
        }
    }

    #[test]
    fn blit_straddles_the_seam() {
        let d = Dims::DEFAULT;
        let mut layer = vec![0u8; d.n() * 4];
        let put = |l: &mut Vec<u8>, x: usize, y: usize, c: [u8; 4]| l[(y * d.w + x) * 4..(y * d.w + x) * 4 + 4].copy_from_slice(&c);
        put(&mut layer, d.w - 1, 50, [200, 0, 0, 255]);
        put(&mut layer, 0, 50, [0, 200, 0, 255]);
        let mut fb = Framebuffer::default();
        blit(&mut fb, &layer, d, &Cam { x: (d.w - 10) as f32, y: 40.0, d });
        assert_eq!(fb.get(9, 10), Some([200, 0, 0, 255]));
        assert_eq!(fb.get(10, 10), Some([0, 200, 0, 255]));
    }

    #[test]
    fn local_field_routes_round_a_wall_and_across_the_seam() {
        let d = Dims::DEFAULT;
        // A wall at x = 3 from y = 80 to 120, with the target just east of it, across the seam.
        let pass = |x: i32, y: i32| !(x == 3 && (80..=120).contains(&y));
        let f = LocalField::build(d, 6, 100, 40, pass);
        let steps = f.at(d.w as i32 - 4, 100);
        assert!(steps > 20 && steps < 60, "round the wall: {steps}");
        // Following the field crosses the seam the short way and gets there in `steps` steps.
        let (mut x, mut y, mut n) = (d.w as i32 - 4, 100, 0);
        while let Some((nx, ny)) = f.next_step(x, y) {
            assert!(pass(nx, ny), "walked into the wall");
            assert!(nx < 40 || nx > d.w as i32 - 40, "went the long way: {nx}");
            (x, y, n) = (nx, ny, n + 1);
        }
        assert_eq!((x, y, n), (6, 100, steps));
        assert!(!f.reachable(500, 100), "outside the window");
    }
}
