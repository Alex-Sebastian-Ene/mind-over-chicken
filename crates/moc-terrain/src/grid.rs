//! Grid plumbing shared by every stage: wrap-aware indexing, diffusion (blur), resampling,
//! distance transforms, connected components and the majority (despeckle) filter.

/// Grid size. `wrap_x`/`wrap_y`: opposite edges are neighbours. Every map is a torus (#111):
/// it wraps east–west and north–south.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dims {
    pub w: usize,
    pub h: usize,
    pub wrap_x: bool,
    pub wrap_y: bool,
}

pub const N4: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
pub const N8: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
pub const SQRT2: f32 = std::f32::consts::SQRT_2;

impl Dims {
    /// `wrap`: a torus (both axes wrap); otherwise a plain rectangle.
    pub fn new(w: usize, h: usize, wrap: bool) -> Self {
        Dims { w, h, wrap_x: wrap, wrap_y: wrap }
    }

    pub fn torus(w: usize, h: usize) -> Self {
        Dims::new(w, h, true)
    }

    /// Same wrapping, another size (pyramid levels, coarse grids).
    pub fn sized(&self, w: usize, h: usize) -> Self {
        Dims { w, h, ..*self }
    }

    #[inline]
    pub fn n(&self) -> usize {
        self.w * self.h
    }

    /// Index of (x, y), wrapping on wrapping axes; None off the grid. Cheap for neighbours
    /// (no division unless far outside).
    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> Option<usize> {
        let (w, h) = (self.w as i32, self.h as i32);
        let y = if (y as u32) < h as u32 {
            y
        } else if !self.wrap_y {
            return None;
        } else if y < 0 && y >= -h {
            y + h
        } else if y >= h && y < 2 * h {
            y - h
        } else {
            y.rem_euclid(h)
        };
        let x = if (x as u32) < w as u32 {
            x
        } else if !self.wrap_x {
            return None;
        } else if x < 0 && x >= -w {
            x + w
        } else if x >= w && x < 2 * w {
            x - w
        } else {
            x.rem_euclid(w)
        };
        Some((y * w + x) as usize)
    }

    #[inline]
    pub fn xy(&self, i: usize) -> (i32, i32) {
        ((i % self.w) as i32, (i / self.w) as i32)
    }

    /// Signed shortest x offset from `a` to `b` (through the seam on a wrapping axis).
    #[inline]
    pub fn dx(&self, a: f32, b: f32) -> f32 {
        let d = b - a;
        if self.wrap_x {
            let w = self.w as f32;
            d - w * (d / w).round()
        } else {
            d
        }
    }

    #[inline]
    pub fn dy(&self, a: f32, b: f32) -> f32 {
        let d = b - a;
        if self.wrap_y {
            let h = self.h as f32;
            d - h * (d / h).round()
        } else {
            d
        }
    }

    #[inline]
    pub fn dist2(&self, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
        let dx = self.dx(ax, bx);
        let dy = self.dy(ay, by);
        dx * dx + dy * dy
    }

    /// The 8 neighbours of `i` that exist, with their step length.
    #[inline]
    pub fn nb8(&self, i: usize, mut f: impl FnMut(usize, f32)) {
        let (x, y) = self.xy(i);
        for (k, &(ox, oy)) in N8.iter().enumerate() {
            if let Some(j) = self.idx(x + ox, y + oy) {
                f(j, if k == 0 || k == 2 || k == 5 || k == 7 { SQRT2 } else { 1.0 });
            }
        }
    }

    #[inline]
    pub fn nb4(&self, i: usize, mut f: impl FnMut(usize)) {
        let (x, y) = self.xy(i);
        for &(ox, oy) in N4.iter() {
            if let Some(j) = self.idx(x + ox, y + oy) {
                f(j);
            }
        }
    }
}

/// Diffusion: `passes` of a separable [1 2 1]/4 binomial blur (the discrete heat equation).
pub fn blur(d: Dims, f: &mut [f32], passes: usize) {
    if passes == 0 {
        return;
    }
    let (w, h) = (d.w, d.h);
    let mut tmp = vec![0.0f32; f.len()];
    for _ in 0..passes {
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let xl = if x == 0 { if d.wrap_x { w - 1 } else { 0 } } else { x - 1 };
                let xr = if x + 1 == w { if d.wrap_x { 0 } else { w - 1 } } else { x + 1 };
                tmp[row + x] = 0.25 * f[row + xl] + 0.5 * f[row + x] + 0.25 * f[row + xr];
            }
        }
        for y in 0..h {
            let yu = if y == 0 { if d.wrap_y { h - 1 } else { 0 } } else { y - 1 } * w;
            let yd = if y + 1 == h { if d.wrap_y { 0 } else { h - 1 } } else { y + 1 } * w;
            let row = y * w;
            for x in 0..w {
                f[row + x] = 0.25 * tmp[yu + x] + 0.5 * tmp[row + x] + 0.25 * tmp[yd + x];
            }
        }
    }
}

/// Bilinear resample from `s` to `d` (cell centres aligned; x wraps on wrapping grids).
pub fn resample(s: Dims, src: &[f32], d: Dims) -> Vec<f32> {
    let mut out = vec![0.0f32; d.n()];
    let (sx_scale, sy_scale) = (s.w as f32 / d.w as f32, s.h as f32 / d.h as f32);
    // Per-column lookup: x0, x1, fx.
    let cols: Vec<(usize, usize, f32)> = (0..d.w)
        .map(|x| {
            let sx = (x as f32 + 0.5) * sx_scale - 0.5;
            let x0f = sx.floor();
            let fx = sx - x0f;
            let x0 = x0f as i32;
            if s.wrap_x {
                (x0.rem_euclid(s.w as i32) as usize, (x0 + 1).rem_euclid(s.w as i32) as usize, fx)
            } else {
                (x0.clamp(0, s.w as i32 - 1) as usize, (x0 + 1).clamp(0, s.w as i32 - 1) as usize, fx)
            }
        })
        .collect();
    for y in 0..d.h {
        let sy = (y as f32 + 0.5) * sy_scale - 0.5;
        let y0f = sy.floor();
        let fy = sy - y0f;
        let (y0, y1) = if s.wrap_y {
            ((y0f as i32).rem_euclid(s.h as i32) as usize, (y0f as i32 + 1).rem_euclid(s.h as i32) as usize)
        } else {
            ((y0f as i32).clamp(0, s.h as i32 - 1) as usize, (y0f as i32 + 1).clamp(0, s.h as i32 - 1) as usize)
        };
        let (r0, r1) = (y0 * s.w, y1 * s.w);
        let row = y * d.w;
        for (x, &(x0, x1, fx)) in cols.iter().enumerate() {
            let a = src[r0 + x0] + (src[r0 + x1] - src[r0 + x0]) * fx;
            let b = src[r1 + x0] + (src[r1 + x1] - src[r1 + x0]) * fx;
            out[row + x] = a + (b - a) * fy;
        }
    }
    out
}

/// Halve a grid by averaging 2×2 blocks (odd edges repeat).
pub fn halve(s: Dims, src: &[f32]) -> (Dims, Vec<f32>) {
    let d = s.sized(s.w.div_ceil(2), s.h.div_ceil(2));
    let mut out = vec![0.0f32; d.n()];
    for y in 0..d.h {
        for x in 0..d.w {
            let (x0, y0) = (2 * x, 2 * y);
            let (x1, y1) = ((x0 + 1).min(s.w - 1), (y0 + 1).min(s.h - 1));
            out[y * d.w + x] = 0.25 * (src[y0 * s.w + x0] + src[y0 * s.w + x1] + src[y1 * s.w + x0] + src[y1 * s.w + x1]);
        }
    }
    (d, out)
}

/// Distance (in cells, Euclidean-ish chamfer 1/√2) from every cell to the nearest source, and
/// the value carried from that nearest source. Wrap-aware.
pub fn distance_carry(d: Dims, src: &[bool], value: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = d.n();
    let mut dist = vec![f32::INFINITY; n];
    let mut val = vec![0.0f32; n];
    for i in 0..n {
        if src[i] {
            dist[i] = 0.0;
            val[i] = value[i];
        }
    }
    let (w, h) = (d.w as i32, d.h as i32);
    let fwd: [(i32, i32, f32); 4] = [(-1, 0, 1.0), (-1, -1, SQRT2), (0, -1, 1.0), (1, -1, SQRT2)];
    let bwd: [(i32, i32, f32); 4] = [(1, 0, 1.0), (1, 1, SQRT2), (0, 1, 1.0), (-1, 1, SQRT2)];
    // Two rounds so distances carry across the seam on wrapping grids.
    for _round in 0..if d.wrap_x || d.wrap_y { 2 } else { 1 } {
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let mut best = dist[i];
                let mut bv = val[i];
                for &(ox, oy, c) in fwd.iter() {
                    if let Some(j) = d.idx(x + ox, y + oy) {
                        if dist[j] + c < best {
                            best = dist[j] + c;
                            bv = val[j];
                        }
                    }
                }
                dist[i] = best;
                val[i] = bv;
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = (y * w + x) as usize;
                let mut best = dist[i];
                let mut bv = val[i];
                for &(ox, oy, c) in bwd.iter() {
                    if let Some(j) = d.idx(x + ox, y + oy) {
                        if dist[j] + c < best {
                            best = dist[j] + c;
                            bv = val[j];
                        }
                    }
                }
                dist[i] = best;
                val[i] = bv;
            }
        }
    }
    (dist, val)
}

pub fn distance(d: Dims, src: &[bool]) -> Vec<f32> {
    let zeros = vec![0.0f32; d.n()];
    distance_carry(d, src, &zeros).0
}

/// Connected components of cells where `inside` holds (8-neighbour, wrap-aware). Returns the
/// label per cell (0 = outside, k+1 = component k) and each component's size.
pub fn components(d: Dims, inside: &[bool]) -> (Vec<u32>, Vec<usize>) {
    let (w, h) = (d.w as i32, d.h as i32);
    let wu = d.w;
    let mut label = vec![0u32; d.n()];
    let mut sizes = Vec::new();
    let mut stack: Vec<(i32, i32)> = Vec::new();
    for sy in 0..h {
        for sx in 0..w {
            let s = sy as usize * wu + sx as usize;
            if !inside[s] || label[s] != 0 {
                continue;
            }
            let id = sizes.len() as u32 + 1;
            label[s] = id;
            stack.push((sx, sy));
            let mut size = 0;
            while let Some((x, y)) = stack.pop() {
                size += 1;
                if x > 0 && x < w - 1 && y > 0 && y < h - 1 {
                    let i = y as usize * wu + x as usize;
                    for &(ox, oy) in N8.iter() {
                        let j = (i as isize + oy as isize * wu as isize + ox as isize) as usize;
                        if inside[j] && label[j] == 0 {
                            label[j] = id;
                            stack.push((x + ox, y + oy));
                        }
                    }
                } else {
                    for &(ox, oy) in N8.iter() {
                        if let Some(j) = d.idx(x + ox, y + oy) {
                            if inside[j] && label[j] == 0 {
                                label[j] = id;
                                stack.push(((j % wu) as i32, (j / wu) as i32));
                            }
                        }
                    }
                }
            }
            sizes.push(size);
        }
    }
    (label, sizes)
}

/// Smoothstep from `a` to `b`.
#[inline]
pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn distance_wraps() {
        let d = Dims::new(20, 12, true);
        let mut src = vec![false; d.n()];
        src[0] = true; // (0, 0)
        let dist = distance(d, &src);
        assert!((dist[19] - 1.0).abs() < 1e-4, "{}", dist[19]);
        assert!((dist[10] - 10.0).abs() < 1e-4);
        assert!((dist[11 * 20] - 1.0).abs() < 1e-4, "across the north-south seam");
    }

    #[test]
    fn resample_is_seamless() {
        let s = Dims::new(8, 4, true);
        let src: Vec<f32> = (0..s.n()).map(|i| (i % 8) as f32 + (i / 8) as f32 * 10.0).collect();
        let out = resample(s, &src, Dims::new(32, 16, true));
        // The seam column interpolates between 7 and 0, not 7 and 7; same for the rows.
        let v = out[5 * 32];
        assert!(v - 10.0 > 1.0 && v - 10.0 < 6.0, "{v}");
        assert!(out[3] > 5.0 && out[3] < 30.0, "{}", out[3]);
    }
}
