//! Wrap-around (torus) grid helpers: resampling and smooth random fields.

use crate::rng::SplitMix64;

/// For each destination index: the first of 4 source taps and their Catmull-Rom weights.
/// Pixel centres are aligned (`align_corners = false`) and indices wrap.
fn cubic_taps(src: usize, dst: usize) -> Vec<([usize; 4], [f32; 4])> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|j| {
            let p = (j as f64 + 0.5) * scale - 0.5;
            let i = p.floor();
            let f = (p - i) as f32;
            let i = i as i64;
            let idx = [-1i64, 0, 1, 2].map(|d| (i + d).rem_euclid(src as i64) as usize);
            let (f2, f3) = (f * f, f * f * f);
            let w = [
                -0.5 * f3 + f2 - 0.5 * f,
                1.5 * f3 - 2.5 * f2 + 1.0,
                -1.5 * f3 + 2.0 * f2 + 0.5 * f,
                0.5 * f3 - 0.5 * f2,
            ];
            (idx, w)
        })
        .collect()
}

fn linear_taps(src: usize, dst: usize) -> Vec<([usize; 4], [f32; 4])> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|j| {
            let p = (j as f64 + 0.5) * scale - 0.5;
            let i = p.floor();
            let f = (p - i) as f32;
            let i = i as i64;
            let a = i.rem_euclid(src as i64) as usize;
            let b = (i + 1).rem_euclid(src as i64) as usize;
            ([a, b, b, b], [1.0 - f, f, 0.0, 0.0])
        })
        .collect()
}

fn separable(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize, tx: &[([usize; 4], [f32; 4])], ty: &[([usize; 4], [f32; 4])]) -> Vec<f32> {
    let mut rows = vec![0.0f32; dw * sh];
    for y in 0..sh {
        let s = &src[y * sw..(y + 1) * sw];
        for (x, (i, w)) in tx.iter().enumerate() {
            rows[y * dw + x] = w[0] * s[i[0]] + w[1] * s[i[1]] + w[2] * s[i[2]] + w[3] * s[i[3]];
        }
    }
    let mut out = vec![0.0f32; dw * dh];
    for (y, (i, w)) in ty.iter().enumerate() {
        let o = &mut out[y * dw..(y + 1) * dw];
        let r: [&[f32]; 4] = i.map(|k| &rows[k * dw..(k + 1) * dw]);
        for x in 0..dw {
            o[x] = w[0] * r[0][x] + w[1] * r[1][x] + w[2] * r[2][x] + w[3] * r[3][x];
        }
    }
    out
}

/// Periodic Catmull-Rom resampling of a sw x sh torus to dw x dh.
pub fn resample_cubic(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    if (sw, sh) == (dw, dh) {
        return src.to_vec();
    }
    separable(src, sw, sh, dw, dh, &cubic_taps(sw, dw), &cubic_taps(sh, dh))
}

/// Periodic bilinear resampling (for 0..1 masks, which must not overshoot).
pub fn resample_linear(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    if (sw, sh) == (dw, dh) {
        return src.to_vec();
    }
    separable(src, sw, sh, dw, dh, &linear_taps(sw, dw), &linear_taps(sh, dh))
}

/// Smooth periodic value noise in [0, 1): an nx x ny lattice of random values, cubic
/// interpolated onto w x h. Wraps on both axes.
pub fn value_noise(w: usize, h: usize, nx: usize, ny: usize, rng: &mut SplitMix64) -> Vec<f32> {
    let (nx, ny) = (nx.max(1), ny.max(1));
    let lattice: Vec<f32> = (0..nx * ny).map(|_| rng.f32()).collect();
    let mut v = resample_cubic(&lattice, nx, ny, w, h);
    for x in &mut v {
        *x = x.clamp(0.0, 1.0);
    }
    v
}

/// Zero-mean periodic fBm, roughly in [-1, 1]: `octaves` value-noise layers, each twice as
/// fine and half as strong as the last, starting at nx x ny cells.
pub fn fbm(w: usize, h: usize, nx: usize, ny: usize, octaves: usize, rng: &mut SplitMix64) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    let (mut amp, mut norm) = (1.0f32, 0.0f32);
    for o in 0..octaves {
        let n = value_noise(w, h, (nx << o).min(w), (ny << o).min(h), rng);
        for (a, b) in out.iter_mut().zip(&n) {
            *a += amp * (b * 2.0 - 1.0);
        }
        norm += amp;
        amp *= 0.5;
    }
    for a in &mut out {
        *a /= norm;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_upsampling_preserves_constants_and_wraps() {
        let src = vec![0.25f32; 6 * 4];
        assert!(resample_cubic(&src, 6, 4, 12, 8).iter().all(|v| (v - 0.25).abs() < 1e-6));
        // A ramp that wraps: the first output column must interpolate between the last and
        // first source columns.
        let ramp: Vec<f32> = (0..4).flat_map(|_| (0..6).map(|x| x as f32)).collect();
        let up = resample_cubic(&ramp, 6, 4, 12, 4);
        assert!((up[0] - 0.968_75).abs() < 1e-4, "{}", up[0]); // Catmull-Rom over 4, 5 | 0, 1
        let lin = resample_linear(&ramp, 6, 4, 12, 4);
        assert!((lin[0] - 1.25).abs() < 1e-5, "{}", lin[0]); // 0.25 * 5 + 0.75 * 0
    }
}
