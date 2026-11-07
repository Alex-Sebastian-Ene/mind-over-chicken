//! Cheap erosion and diffusion on the simulation grid:
//!
//! - **thermal**: slopes steeper than the talus angle shed material downhill;
//! - **stream power**: rivers cut down in proportion to drainage area^m × slope, solved
//!   implicitly along the flood order (Braun & Willett), so it is stable in a few passes and
//!   carves the valleys between the DLA ridge spurs;
//! - **lowland diffusion**: heat-equation (Laplacian) smoothing, strong on coasts and plains,
//!   none on the mountains, which rounds off coastlines and flattens the lowlands.

use crate::grid::{Dims, N8, SQRT2};
use crate::hydrology::{self, NONE};

/// Thermal erosion: `passes` of moving `rate` of the excess over `talus` to the lowest
/// neighbour (land only).
pub fn thermal(d: Dims, h: &mut [f32], sea: f32, talus: f32, rate: f32, passes: usize) {
    let n = d.n();
    let mut delta = vec![0.0f32; n];
    for _ in 0..passes {
        delta.iter_mut().for_each(|v| *v = 0.0);
        for i in 0..n {
            if h[i] < sea {
                continue;
            }
            let (x, y) = d.xy(i);
            let mut lo = (i, 0.0f32);
            for (k, &(ox, oy)) in N8.iter().enumerate() {
                if let Some(j) = d.idx(x + ox, y + oy) {
                    let step = if k == 0 || k == 2 || k == 5 || k == 7 { SQRT2 } else { 1.0 };
                    let s = (h[i] - h[j]) / step;
                    if s > lo.1 {
                        lo = (j, s);
                    }
                }
            }
            if lo.1 > talus {
                let m = rate * (lo.1 - talus) * 0.5;
                delta[i] -= m;
                delta[lo.0] += m;
            }
        }
        for i in 0..n {
            h[i] += delta[i];
        }
    }
}

/// Stream-power incision, `iters` passes. `k` scales the erosion, `m` the drainage-area
/// exponent; `cell` is the cell size in tiles (so area is in tiles²).
pub fn stream_power(d: Dims, h: &mut [f32], sea: f32, k: f32, m: f32, iters: usize, cell: f32) {
    let ones = vec![cell * cell; d.n()];
    for _ in 0..iters {
        let dr = hydrology::route(d, h, sea, 1e-5);
        let acc = hydrology::accumulate(&dr, &ones);
        for &ci in dr.order.iter() {
            let i = ci as usize;
            let r = dr.recv[i];
            if r == NONE || h[i] < sea {
                continue;
            }
            let r = r as usize;
            let (xi, yi) = d.xy(i);
            let (xr, yr) = d.xy(r);
            let diag = xi != xr && yi != yr;
            let dist = if diag { SQRT2 } else { 1.0 } * cell;
            let f = k * acc[i].powf(m) / dist;
            let target = h[r].max(sea);
            if h[i] > target {
                h[i] = (h[i] + f * target) / (1.0 + f);
            }
        }
    }
}

/// Laplacian smoothing weighted towards low ground: `kappa` at sea level, fading to nothing
/// at `top` (so mountains keep their crests).
pub fn diffuse_lowlands(d: Dims, h: &mut [f32], sea: f32, top: f32, kappa: f32, passes: usize) {
    let n = d.n();
    let mut out = h.to_vec();
    for _ in 0..passes {
        for i in 0..n {
            let v = h[i];
            let w = 1.0 - ((v - sea).abs() / top).clamp(0.0, 1.0);
            if w <= 0.0 {
                out[i] = v;
                continue;
            }
            let mut s = 0.0;
            let mut c = 0.0;
            d.nb4(i, |j| {
                s += h[j];
                c += 1.0;
            });
            out[i] = v + kappa * w * (s / c - v);
        }
        h.copy_from_slice(&out);
    }
}
