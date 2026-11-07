//! Coherent noise sampled on a torus, so every map is seamless east–west *and* north–south
//! (#111: going off the top brings you back at the bottom).
//!
//! A map position (x, y) becomes a point on two circles, (Rx cos θx, Rx sin θx, Ry cos θy,
//! Ry sin θy), with θx = 2πx/W, θy = 2πy/H and radii W/2π and H/2π. That embedding (the
//! Clifford torus) keeps distances: one tile is one unit of noise space in every direction,
//! and the first and last columns (and rows) are neighbours. 4D gradient noise is sampled
//! there. Domain warping moves the point along the torus's two tangent directions, which keeps
//! the warp seamless too.

use crate::grid::{self, Dims};
use crate::rng::Rng;

/// Gradient noise in 4D (simplex), with a seeded permutation table.
#[derive(Clone)]
pub struct Perlin {
    perm: [u8; 512],
}

/// The 32 gradients: three of ±1 and a zero.
const GRAD4: [[f32; 4]; 32] = {
    let mut g = [[0.0f32; 4]; 32];
    let mut h = 0;
    while h < 32 {
        let zero = h >> 3;
        let s = [if h & 4 == 0 { -1.0 } else { 1.0 }, if h & 2 == 0 { -1.0 } else { 1.0 }, if h & 1 == 0 { -1.0 } else { 1.0 }];
        let mut k = 0;
        let mut c = 0;
        while c < 4 {
            if c != 3 - zero {
                g[h][c] = s[k];
                k += 1;
            }
            c += 1;
        }
        h += 1;
    }
    g
};

#[inline(always)]
fn grad(h: u8, x: f32, y: f32, z: f32, w: f32) -> f32 {
    let g = &GRAD4[(h & 31) as usize];
    g[0] * x + g[1] * y + g[2] * z + g[3] * w
}

/// floor() without a libm call (the wasm and x86-64 baseline targets have no fast floor).
#[inline(always)]
fn ffloor(x: f32) -> f32 {
    let i = x as i32 as f32;
    if i > x { i - 1.0 } else { i }
}

impl Perlin {
    pub fn new(rng: &mut Rng) -> Self {
        let mut p = [0u8; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        for i in (1..256).rev() {
            let j = rng.below(i + 1);
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Perlin { perm }
    }

    /// 4D simplex noise (Gustavson), roughly [-1, 1]: five corners per sample instead of
    /// sixteen, which matters at 3 million tiles.
    #[inline]
    pub fn noise(&self, x: f32, y: f32, z: f32, w: f32) -> f32 {
        const F4: f32 = 0.309_017;
        const G4: f32 = 0.138_196_6;
        let s = (x + y + z + w) * F4;
        let (i, j, k, l) = (ffloor(x + s), ffloor(y + s), ffloor(z + s), ffloor(w + s));
        let t = (i + j + k + l) * G4;
        let (x0, y0, z0, w0) = (x - (i - t), y - (j - t), z - (k - t), w - (l - t));
        // Rank the coordinates to find which simplex we're in.
        let (mut rx, mut ry, mut rz, mut rw) = (0u8, 0u8, 0u8, 0u8);
        if x0 > y0 { rx += 1 } else { ry += 1 }
        if x0 > z0 { rx += 1 } else { rz += 1 }
        if x0 > w0 { rx += 1 } else { rw += 1 }
        if y0 > z0 { ry += 1 } else { rz += 1 }
        if y0 > w0 { ry += 1 } else { rw += 1 }
        if z0 > w0 { rz += 1 } else { rw += 1 }
        let c = |r: u8, n: u8| if r >= n { 1.0f32 } else { 0.0 };
        let (i1, j1, k1, l1) = (c(rx, 3), c(ry, 3), c(rz, 3), c(rw, 3));
        let (i2, j2, k2, l2) = (c(rx, 2), c(ry, 2), c(rz, 2), c(rw, 2));
        let (i3, j3, k3, l3) = (c(rx, 1), c(ry, 1), c(rz, 1), c(rw, 1));
        let p = &self.perm;
        let (ii, jj, kk, ll) = ((i as i32 & 255) as usize, (j as i32 & 255) as usize, (k as i32 & 255) as usize, (l as i32 & 255) as usize);
        // `& 511` keeps every index provably inside the 512-entry table (no bounds checks).
        let h = |a: f32, b: f32, cc: f32, d: f32| -> u8 {
            let q = p[(ll + d as usize) & 511] as usize;
            let q = p[(kk + cc as usize + q) & 511] as usize;
            let q = p[(jj + b as usize + q) & 511] as usize;
            p[(ii + a as usize + q) & 511]
        };
        let mut n = 0.0f32;
        let mut corner = |gx: f32, gy: f32, gz: f32, gw: f32, hh: u8| {
            let t = 0.6 - gx * gx - gy * gy - gz * gz - gw * gw;
            if t > 0.0 {
                let t2 = t * t;
                n += t2 * t2 * grad(hh, gx, gy, gz, gw);
            }
        };
        corner(x0, y0, z0, w0, h(0.0, 0.0, 0.0, 0.0));
        corner(x0 - i1 + G4, y0 - j1 + G4, z0 - k1 + G4, w0 - l1 + G4, h(i1, j1, k1, l1));
        corner(x0 - i2 + 2.0 * G4, y0 - j2 + 2.0 * G4, z0 - k2 + 2.0 * G4, w0 - l2 + 2.0 * G4, h(i2, j2, k2, l2));
        corner(x0 - i3 + 3.0 * G4, y0 - j3 + 3.0 * G4, z0 - k3 + 3.0 * G4, w0 - l3 + 3.0 * G4, h(i3, j3, k3, l3));
        corner(x0 - 1.0 + 4.0 * G4, y0 - 1.0 + 4.0 * G4, z0 - 1.0 + 4.0 * G4, w0 - 1.0 + 4.0 * G4, h(1.0, 1.0, 1.0, 1.0));
        n * 27.0
    }
}

/// Cheap 2D gradient noise whose lattice repeats exactly across the map in both directions,
/// so it is seamless on the torus too. Used for per-tile detail, where 4D noise would cost
/// several times more for the same look.
#[derive(Clone)]
pub struct Periodic2 {
    perm: [u8; 512],
    px: i32,
    py: i32,
    fx: f32,
    fy: f32,
}

impl Periodic2 {
    /// Noise with features about `wavelength` tiles across, on a `map_w`×`map_h` map.
    pub fn new(rng: &mut Rng, map_w: usize, map_h: usize, wavelength: f32) -> Self {
        let mut p = [0u8; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        for i in (1..256).rev() {
            let j = rng.below(i + 1);
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        let px = (map_w as f32 / wavelength).round().max(1.0) as i32;
        let py = (map_h as f32 / wavelength).round().max(1.0) as i32;
        Periodic2 { perm, px, py, fx: px as f32 / map_w as f32, fy: py as f32 / map_h as f32 }
    }

    /// Roughly [-1, 1] at map position (x, y) in tiles.
    #[inline]
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let (x, y) = (x * self.fx, y * self.fy);
        let (x0, y0) = (ffloor(x), ffloor(y));
        let (u, v) = (x - x0, y - y0);
        let (xi, yi) = ((x0 as i32).rem_euclid(self.px), (y0 as i32).rem_euclid(self.py));
        let xj = if xi + 1 == self.px { 0 } else { xi + 1 };
        let yj = if yi + 1 == self.py { 0 } else { yi + 1 };
        let p = &self.perm;
        let hsh = |a: i32, b: i32| p[(p[(a & 255) as usize] as usize + (b & 255) as usize) & 511];
        let g = |h: u8, dx: f32, dy: f32| {
            let k = (h & 7) as usize;
            const G: [(f32, f32); 8] = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.707, 0.707), (-0.707, 0.707), (0.707, -0.707), (-0.707, -0.707)];
            G[k].0 * dx + G[k].1 * dy
        };
        let fu = u * u * u * (u * (u * 6.0 - 15.0) + 10.0);
        let fv = v * v * v * (v * (v * 6.0 - 15.0) + 10.0);
        let a = g(hsh(xi, yi), u, v);
        let b = g(hsh(xj, yi), u - 1.0, v);
        let c = g(hsh(xi, yj), u, v - 1.0);
        let d = g(hsh(xj, yj), u - 1.0, v - 1.0);
        let top = a + (b - a) * fu;
        let bot = c + (d - c) * fu;
        (top + (bot - top) * fv) * 1.4
    }
}

/// A point on the torus plus its two unit tangents (east and south), for warping.
#[derive(Clone, Copy, Debug)]
pub struct Pt {
    pub p: [f32; 4],
    /// (-sin θx, cos θx): the east direction in the first two coordinates.
    pub tx: [f32; 2],
    /// (-sin θy, cos θy): the south direction in the last two.
    pub ty: [f32; 2],
}

impl Pt {
    /// Move `dx` tiles east and `dy` tiles south (to first order, which stays seamless).
    #[inline]
    pub fn moved(&self, dx: f32, dy: f32) -> Pt {
        Pt { p: [self.p[0] + self.tx[0] * dx, self.p[1] + self.tx[1] * dx, self.p[2] + self.ty[0] * dy, self.p[3] + self.ty[1] * dy], ..*self }
    }
}

/// Maps the cells of a `gw`×`gh` grid over a `map_w`×`map_h`-tile map onto the torus. Coarse
/// grids of the same map land on the same torus, so frequencies mean the same on every grid.
#[derive(Clone)]
pub struct Torus {
    pub gw: usize,
    pub gh: usize,
    map_w: f32,
    map_h: f32,
    col: Vec<[f32; 2]>,
    row: Vec<[f32; 2]>,
}

impl Torus {
    pub fn new(map_w: usize, map_h: usize, gw: usize, gh: usize) -> Self {
        let col = (0..gw).map(|x| std::f32::consts::TAU * (x as f32 + 0.5) / gw as f32).map(|a| [a.cos(), a.sin()]).collect();
        let row = (0..gh).map(|y| std::f32::consts::TAU * (y as f32 + 0.5) / gh as f32).map(|a| [a.cos(), a.sin()]).collect();
        Torus { gw, gh, map_w: map_w as f32, map_h: map_h as f32, col, row }
    }

    /// The map's size in tiles.
    pub fn map_size(&self) -> (usize, usize) {
        (self.map_w as usize, self.map_h as usize)
    }

    /// The smaller side of the map, in tiles: a natural "largest feature" scale.
    pub fn map_min(&self) -> f32 {
        self.map_w.min(self.map_h)
    }

    /// Grid cell (x, y).
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> Pt {
        let (rx, ry) = (self.map_w / std::f32::consts::TAU, self.map_h / std::f32::consts::TAU);
        let [cx, sx] = self.col[x];
        let [cy, sy] = self.row[y];
        Pt { p: [rx * cx, rx * sx, ry * cy, ry * sy], tx: [-sx, cx], ty: [-sy, cy] }
    }

    /// A fractional map position in tiles.
    pub fn at_tiles(&self, x: f32, y: f32) -> Pt {
        let (rx, ry) = (self.map_w / std::f32::consts::TAU, self.map_h / std::f32::consts::TAU);
        let (ax, ay) = (std::f32::consts::TAU * x / self.map_w, std::f32::consts::TAU * y / self.map_h);
        let (cx, sx, cy, sy) = (ax.cos(), ax.sin(), ay.cos(), ay.sin());
        Pt { p: [rx * cx, rx * sx, ry * cy, ry * sy], tx: [-sx, cx], ty: [-sy, cy] }
    }
}

/// Fractal Brownian motion: `octaves` layers of Perlin noise.
#[derive(Clone)]
pub struct Fbm {
    pub perlin: Perlin,
    /// Cycles per tile at the first octave.
    pub freq: f32,
    pub octaves: u32,
    pub gain: f32,
}

const OFFS: [[f32; 4]; 8] = [
    [0.0, 0.0, 0.0, 0.0],
    [17.13, -31.7, 7.91, 3.3],
    [-41.3, 12.9, 53.1, -9.7],
    [71.7, 29.3, -19.4, 41.1],
    [-13.9, 87.1, 37.7, -63.3],
    [101.3, -57.9, 11.3, 27.9],
    [-77.1, 3.7, -91.9, 15.5],
    [23.9, 113.3, 67.1, -37.1],
];

impl Fbm {
    pub fn new(rng: &mut Rng, freq: f32, octaves: u32, gain: f32) -> Self {
        Fbm { perlin: Perlin::new(rng), freq, octaves, gain }
    }

    /// Roughly [-1, 1].
    #[inline]
    pub fn sample(&self, p: &Pt) -> f32 {
        self.sample_octaves(p, 0, self.octaves)
    }

    /// Only octaves `first..last`, normalised as the full sum would be, so the low octaves can
    /// live on a coarse grid and the high ones on the fine grid.
    #[inline]
    pub fn sample_octaves(&self, p: &Pt, first: u32, last: u32) -> f32 {
        let (mut s, mut amp, mut f, mut norm) = (0.0, 1.0, self.freq, 0.0);
        let q = p.p;
        for o in 0..self.octaves {
            if o >= first && o < last {
                let off = OFFS[o as usize & 7];
                s += amp * self.perlin.noise(q[0] * f + off[0], q[1] * f + off[1], q[2] * f + off[2], q[3] * f + off[3]);
            }
            norm += amp;
            amp *= self.gain;
            f *= 2.0;
        }
        s / norm.max(1e-6) * 1.5
    }

    /// Ridged multifractal in [0, 1]: sharp crests where the noise crosses zero.
    pub fn ridged(&self, p: &Pt) -> f32 {
        let (mut s, mut amp, mut f, mut norm, mut weight) = (0.0, 1.0, self.freq, 0.0, 1.0f32);
        let q = p.p;
        for o in 0..self.octaves {
            let off = OFFS[o as usize & 7];
            let n = self.perlin.noise(q[0] * f + off[0], q[1] * f + off[1], q[2] * f + off[2], q[3] * f + off[3]);
            let n = (1.0 - n.abs() * 1.6).clamp(0.0, 1.0);
            let n = n * n * weight;
            weight = (n * 1.8).clamp(0.0, 1.0);
            s += n * amp;
            norm += amp;
            amp *= self.gain;
            f *= 2.0;
        }
        (s / norm).clamp(0.0, 1.0)
    }
}

/// Domain warp: displaces a point by up to about `amp` tiles along the torus.
#[derive(Clone)]
pub struct Warp {
    a: Fbm,
    b: Fbm,
    pub amp: f32,
}

impl Warp {
    pub fn new(rng: &mut Rng, freq: f32, octaves: u32, amp: f32) -> Self {
        Warp { a: Fbm::new(rng, freq, octaves, 0.5), b: Fbm::new(rng, freq, octaves, 0.5), amp }
    }

    #[inline]
    pub fn offset(&self, p: &Pt) -> (f32, f32) {
        (self.a.sample(p) * self.amp, self.b.sample(p) * self.amp)
    }

    #[inline]
    pub fn apply(&self, p: &Pt) -> Pt {
        let (dx, dy) = self.offset(p);
        p.moved(dx, dy)
    }
}

/// Sample `f` on a grid `factor` times coarser than `d` (a grid over a `map_w`×`map_h`-tile
/// map) and upsample it to `d`. Low-frequency fields don't need every cell.
pub fn coarse_field(d: Dims, map_w: usize, map_h: usize, factor: usize, f: impl Fn(&Pt) -> f32) -> Vec<f32> {
    let factor = factor.max(1);
    let cd = d.sized(d.w.div_ceil(factor).max(2), d.h.div_ceil(factor).max(2));
    let tor = Torus::new(map_w, map_h, cd.w, cd.h);
    let mut v = vec![0.0f32; cd.n()];
    for y in 0..cd.h {
        for x in 0..cd.w {
            v[y * cd.w + x] = f(&tor.at(x, y));
        }
    }
    if cd == d {
        v
    } else {
        grid::resample(cd, &v, d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torus_is_seamless_both_ways() {
        let t = Torus::new(320, 180, 320, 180);
        let mut rng = Rng::new(5);
        let f = Fbm::new(&mut rng, 1.0 / 40.0, 4, 0.5);
        let d = |p: [f32; 4], q: [f32; 4]| p.iter().zip(q.iter()).map(|(a, b)| (a - b).powi(2)).sum::<f32>().sqrt();
        // Column 319 and column 0 are neighbours, as are row 179 and row 0.
        let (a, b, c) = (t.at(319, 10), t.at(0, 10), t.at(1, 10));
        assert!((d(a.p, b.p) - d(b.p, c.p)).abs() < 1e-3);
        let (a, b, c) = (t.at(10, 179), t.at(10, 0), t.at(10, 1));
        assert!((d(a.p, b.p) - d(b.p, c.p)).abs() < 1e-3);
        assert!((f.sample(&a) - f.sample(&b)).abs() < 0.2);
        let n = Perlin::new(&mut rng);
        let mut lo = 0.0f32;
        let mut hi = 0.0f32;
        for i in 0..5000 {
            let v = n.noise(i as f32 * 0.37, i as f32 * 0.11 - 40.0, 3.3 + i as f32 * 0.07, i as f32 * 0.013);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(lo > -1.2 && hi < 1.2 && hi - lo > 0.8, "{lo} {hi}");
    }
}
