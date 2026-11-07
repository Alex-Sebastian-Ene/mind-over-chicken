//! The few kernels the UNet needs. Tensors are HWC (pixel-major, channels contiguous), and
//! everything wraps around (the map is a torus).
//!
//! Convolutions are register-blocked: 4 pixels x 8 output channels of accumulators stay in
//! registers while the kernel walks every tap and input channel, reading the input and the
//! packed weights sequentially. The inner step is one 8-wide weight load, four broadcasts and
//! four multiply-adds; LLVM turns the `[f32; 8]` lanes into SIMD (2 x v128 on wasm32, where
//! the hot functions enable `simd128` themselves so the engine's wasm-pack build needs no
//! RUSTFLAGS, and one ymm register with AVX2 on x86_64).

/// Output channels per register block (SIMD lanes).
pub const OB: usize = 8;
/// Pixels per register block.
pub const PX: usize = 4;
/// Pixels per register block with AVX2 (8-lane registers: more accumulators hide latency).
pub const PX_AVX2: usize = 4;

/// e^x to ~2e-7 relative error (Cephes `expf`: two-constant range reduction, then a
/// degree-6 polynomial). Branch-free and free of `floor` calls so it vectorises even on
/// baseline x86_64 (SSE2) and wasm.
#[inline(always)]
pub fn fast_exp(x: f32) -> f32 {
    const MAGIC: f32 = 12_582_912.0; // 1.5 * 2^23: adding it rounds to the nearest integer
    let x = x.clamp(-87.0, 88.0);
    let r = x * std::f32::consts::LOG2_E + MAGIC;
    let n = (r.to_bits() as i32).wrapping_sub(MAGIC.to_bits() as i32);
    let nf = r - MAGIC;
    let g = x - nf * 0.693_359_4 - nf * -2.121_944_4e-4;
    let p = (((((1.987_569_1e-4 * g + 1.398_199_9e-3) * g + 8.333_452e-3) * g + 4.166_579_6e-2) * g + 1.666_666_5e-1) * g
        + 5.000_000_1e-1)
        * g
        * g
        + g
        + 1.0;
    p * f32::from_bits(((n + 127) as u32) << 23)
}

#[inline(always)]
pub fn silu(x: f32) -> f32 {
    x / (1.0 + fast_exp(-x))
}

/// A 1x1 or 3x3 convolution with weights packed as [out block][tap][in][OB].
#[derive(Clone, Debug)]
pub struct Conv {
    pub k: usize,
    pub cout: usize,
    pub cin: usize,
    pub packed: Vec<f32>,
    pub bias: Vec<f32>,
}

impl Conv {
    /// `w` in PyTorch layout [cout][cin_total][k][k]; keeps input channels `ci0..ci0+cin`.
    pub fn from_torch(w: &[f32], b: Option<&[f32]>, k: usize, cout: usize, cin_total: usize, ci0: usize, cin: usize) -> Self {
        let taps = k * k;
        let nblk = cout.div_ceil(OB);
        let mut packed = vec![0.0; nblk * taps * cin * OB];
        for o in 0..cout {
            for i in 0..cin {
                for tap in 0..taps {
                    packed[((o / OB * taps + tap) * cin + i) * OB + o % OB] = w[(o * cin_total + ci0 + i) * taps + tap];
                }
            }
        }
        let bias = b.map(|b| b.to_vec()).unwrap_or_else(|| vec![0.0; cout]);
        Conv { k, cout, cin, packed, bias }
    }
}

/// A dense layer, PyTorch layout [out][in].
#[derive(Clone, Debug)]
pub struct Linear {
    pub out: usize,
    pub inp: usize,
    pub w: Vec<f32>,
    pub b: Vec<f32>,
}

impl Linear {
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        (0..self.out)
            .map(|o| {
                let row = &self.w[o * self.inp..(o + 1) * self.inp];
                row.iter().zip(x).fold(self.b[o], |s, (w, v)| s + w * v)
            })
            .collect()
    }
}

#[inline(always)]
fn silu_copy(dst: &mut [f32], src: &[f32], act: bool) {
    if act {
        for (o, v) in dst.iter_mut().zip(src) {
            *o = silu(*v);
        }
    } else {
        dst.copy_from_slice(src);
    }
}

/// HWC h x w x c -> (h+2) x (w+2) x c with a one-pixel wrap-around border, applying SiLU on
/// the way when `act`.
#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
pub fn pad_wrap(src: &[f32], c: usize, h: usize, w: usize, act: bool, dst: &mut [f32]) {
    let rs = (w + 2) * c;
    for y in 0..h + 2 {
        let sy = (y + h - 1) % h;
        let drow = &mut dst[y * rs..(y + 1) * rs];
        if y == 0 || y == h + 1 {
            // Copy an already-built padded row instead of recomputing SiLU.
            continue;
        }
        silu_copy(&mut drow[c..(w + 1) * c], &src[sy * w * c..(sy + 1) * w * c], act);
        drow.copy_within(w * c..(w + 1) * c, 0);
        drow.copy_within(c..2 * c, (w + 1) * c);
    }
    dst.copy_within(h * rs..(h + 1) * rs, 0);
    dst.copy_within(rs..2 * rs, (h + 1) * rs);
}

#[inline(always)]
fn kernel<const P: usize, const K: usize>(inp: &[f32], in_w: usize, cin: usize, y: usize, x: usize, wk: &[f32]) -> [[f32; OB]; P] {
    let mut acc = [[0f32; OB]; P];
    // Bounds for every access below, checked once per block instead of per element.
    assert!(wk.len() >= K * K * cin * OB);
    assert!(inp.len() >= ((y + K - 1) * in_w + x + K - 1 + P) * cin);
    for ky in 0..K {
        for kx in 0..K {
            let wt = (ky * K + kx) * cin * OB;
            let base = ((y + ky) * in_w + x + kx) * cin;
            for ic in 0..cin {
                // SAFETY: wt + ic*OB + OB <= K*K*cin*OB <= wk.len(), and
                // base + (P-1)*cin + ic < ((y+K-1)*in_w + x+K-1 + P)*cin <= inp.len() (asserted).
                let wv: [f32; OB] = unsafe { *(wk.as_ptr().add(wt + ic * OB) as *const [f32; OB]) };
                for (p, a) in acc.iter_mut().enumerate() {
                    let s = unsafe { *inp.get_unchecked(base + p * cin + ic) };
                    for l in 0..OB {
                        a[l] += s * wv[l];
                    }
                }
            }
        }
    }
    acc
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn store<const P: usize>(acc: &[[f32; OB]; P], o0: usize, cout: usize, at0: usize, mul: &[f32; OB], add: &[f32; OB], res: Option<&[f32]>, out: &mut [f32]) {
    let no = (cout - o0).min(OB);
    for (p, a) in acc.iter().enumerate() {
        let at = at0 + p * cout + o0;
        if no == OB {
            let d: &mut [f32; OB] = (&mut out[at..at + OB]).try_into().unwrap();
            match res {
                Some(r) => {
                    let r: &[f32; OB] = r[at..at + OB].try_into().unwrap();
                    for l in 0..OB {
                        d[l] = a[l] * mul[l] + add[l] + r[l];
                    }
                }
                None => {
                    for l in 0..OB {
                        d[l] = a[l] * mul[l] + add[l];
                    }
                }
            }
        } else {
            for l in 0..no {
                out[at + l] = a[l] * mul[l] + add[l] + res.map_or(0.0, |r| r[at + l]);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn conv_body<const K: usize, const P: usize>(inp: &[f32], h: usize, w: usize, k: &Conv, mul: &[f32], add: &[f32], res: Option<&[f32]>, out: &mut [f32]) {
    let (cin, cout) = (k.cin, k.cout);
    let in_w = w + K - 1;
    let nblk = cout.div_ceil(OB);
    let blk = K * K * cin * OB;
    // Epilogue constants padded to whole blocks.
    let mut mp = vec![0.0f32; nblk * OB];
    let mut ap = vec![0.0f32; nblk * OB];
    mp[..cout].copy_from_slice(&mul[..cout]);
    ap[..cout].copy_from_slice(&add[..cout]);
    for y in 0..h {
        let mut x = 0;
        while x + P <= w {
            for ob in 0..nblk {
                let acc = kernel::<P, K>(inp, in_w, cin, y, x, &k.packed[ob * blk..(ob + 1) * blk]);
                let m: &[f32; OB] = mp[ob * OB..ob * OB + OB].try_into().unwrap();
                let a: &[f32; OB] = ap[ob * OB..ob * OB + OB].try_into().unwrap();
                store(&acc, ob * OB, cout, (y * w + x) * cout, m, a, res, out);
            }
            x += P;
        }
        while x < w {
            for ob in 0..nblk {
                let acc = kernel::<1, K>(inp, in_w, cin, y, x, &k.packed[ob * blk..(ob + 1) * blk]);
                let m: &[f32; OB] = mp[ob * OB..ob * OB + OB].try_into().unwrap();
                let a: &[f32; OB] = ap[ob * OB..ob * OB + OB].try_into().unwrap();
                store(&acc, ob * OB, cout, (y * w + x) * cout, m, a, res, out);
            }
            x += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn conv_dispatch<const P: usize>(inp: &[f32], h: usize, w: usize, k: &Conv, mul: &[f32], add: &[f32], res: Option<&[f32]>, out: &mut [f32]) {
    match k.k {
        1 => conv_body::<1, P>(inp, h, w, k, mul, add, res, out),
        3 => conv_body::<3, P>(inp, h, w, k, mul, add, res, out),
        n => panic!("unsupported kernel size {n}"),
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
unsafe fn conv_avx2(inp: &[f32], h: usize, w: usize, k: &Conv, mul: &[f32], add: &[f32], res: Option<&[f32]>, out: &mut [f32]) {
    conv_dispatch::<PX_AVX2>(inp, h, w, k, mul, add, res, out)
}

#[cfg_attr(target_arch = "wasm32", target_feature(enable = "simd128"))]
#[allow(clippy::too_many_arguments)]
fn conv_generic(inp: &[f32], h: usize, w: usize, k: &Conv, mul: &[f32], add: &[f32], res: Option<&[f32]>, out: &mut [f32]) {
    conv_dispatch::<PX>(inp, h, w, k, mul, add, res, out)
}

/// Convolution producing h x w x cout. For k = 3, `inp` is `pad_wrap`ped ((h+2) x (w+2) x
/// cin); for k = 1 it is h x w x cin. Epilogue per output channel o:
/// `out = acc * mul[o] + add[o] (+ res)`; pass mul = 1 and add = bias for a plain conv.
#[allow(clippy::too_many_arguments)]
pub fn conv(inp: &[f32], h: usize, w: usize, k: &Conv, mul: &[f32], add: &[f32], res: Option<&[f32]>, out: &mut [f32]) {
    debug_assert!(inp.len() >= (h + k.k - 1) * (w + k.k - 1) * k.cin);
    debug_assert!(out.len() >= h * w * k.cout);
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: the CPU supports AVX2 (checked just above).
            unsafe { conv_avx2(inp, h, w, k, mul, add, res, out) };
            return;
        }
    }
    conv_generic(inp, h, w, k, mul, add, res, out)
}

/// Plain conv with its own bias.
pub fn conv_bias(inp: &[f32], h: usize, w: usize, k: &Conv, out: &mut [f32]) {
    let ones = vec![1.0; k.cout];
    conv(inp, h, w, k, &ones, &k.bias, None, out)
}

/// PyTorch `pixel_unshuffle(x, 2)` in HWC: h x w x c -> h/2 x w/2 x 4c, channel c*4 + i*2 + j.
pub fn space_to_depth(src: &[f32], c: usize, h: usize, w: usize, dst: &mut [f32]) {
    let (h2, w2) = (h / 2, w / 2);
    for y in 0..h2 {
        for x in 0..w2 {
            let d = &mut dst[(y * w2 + x) * 4 * c..(y * w2 + x + 1) * 4 * c];
            for i in 0..2 {
                for j in 0..2 {
                    let s = &src[((2 * y + i) * w + 2 * x + j) * c..((2 * y + i) * w + 2 * x + j + 1) * c];
                    for (ch, v) in s.iter().enumerate() {
                        d[ch * 4 + i * 2 + j] = *v;
                    }
                }
            }
        }
    }
}

/// PyTorch `pixel_shuffle(x, 2)` to one channel: h x w x 4 -> 2h x 2w.
pub fn depth_to_space1(src: &[f32], h: usize, w: usize, dst: &mut [f32]) {
    let w2 = 2 * w;
    for y in 0..h {
        for x in 0..w {
            let s = &src[(y * w + x) * 4..(y * w + x) * 4 + 4];
            for i in 0..2 {
                for j in 0..2 {
                    dst[(2 * y + i) * w2 + 2 * x + j] = s[i * 2 + j];
                }
            }
        }
    }
}

/// dst = nearest_upsample_x2(low) + skip, HWC with c channels; low is h x w.
pub fn up2_add(low: &[f32], skip: &[f32], c: usize, h: usize, w: usize, dst: &mut [f32]) {
    let w2 = 2 * w;
    for y in 0..2 * h {
        for x in 0..w2 {
            let at = (y * w2 + x) * c;
            let l = &low[((y / 2) * w + x / 2) * c..((y / 2) * w + x / 2 + 1) * c];
            for ((d, s), v) in dst[at..at + c].iter_mut().zip(&skip[at..at + c]).zip(l) {
                *d = v + s;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_exp_is_accurate() {
        let mut worst = 0.0f64;
        let mut x = -80.0f32;
        while x < 80.0 {
            let r = (fast_exp(x) as f64 - (x as f64).exp()).abs() / (x as f64).exp();
            worst = worst.max(r);
            x += 0.0137;
        }
        assert!(worst < 5e-7, "worst relative error {worst}");
        assert!(silu(-200.0).abs() < 1e-30 && (silu(200.0) - 200.0).abs() < 1e-3);
    }

    fn naive(x: &[f32], wt: &[f32], b: &[f32], cin: usize, cout: usize, h: usize, w: usize, k: usize) -> Vec<f32> {
        let r = k / 2;
        let mut out = vec![0.0; h * w * cout];
        for o in 0..cout {
            for y in 0..h {
                for xx in 0..w {
                    let mut s = b[o];
                    for i in 0..cin {
                        for ky in 0..k {
                            for kx in 0..k {
                                let sy = (y + h + ky - r) % h;
                                let sx = (xx + w + kx - r) % w;
                                s += wt[((o * cin + i) * k + ky) * k + kx] * x[(sy * w + sx) * cin + i];
                            }
                        }
                    }
                    out[(y * w + xx) * cout + o] = s;
                }
            }
        }
        out
    }

    #[test]
    fn convs_match_naive_wraparound() {
        // Odd sizes: ragged pixel blocks and a partial output-channel block.
        for (k, cin, cout, h, w) in [(3, 3, 11, 5, 11), (1, 7, 5, 3, 9), (3, 8, 16, 4, 8)] {
            let wt: Vec<f32> = (0..cout * cin * k * k).map(|i| ((i * 37 % 17) as f32 - 8.0) / 9.0).collect();
            let b: Vec<f32> = (0..cout).map(|o| o as f32 * 0.1).collect();
            let x: Vec<f32> = (0..cin * h * w).map(|i| ((i * 13 % 23) as f32 - 11.0) / 7.0).collect();
            let conv_k = Conv::from_torch(&wt, Some(&b), k, cout, cin, 0, cin);
            let mut out = vec![0.0; cout * h * w];
            if k == 3 {
                let mut pad = vec![0.0; cin * (h + 2) * (w + 2)];
                pad_wrap(&x, cin, h, w, false, &mut pad);
                conv_bias(&pad, h, w, &conv_k, &mut out);
            } else {
                conv_bias(&x, h, w, &conv_k, &mut out);
            }
            let want = naive(&x, &wt, &b, cin, cout, h, w, k);
            for (i, (g, e)) in out.iter().zip(&want).enumerate() {
                assert!((g - e).abs() < 1e-4, "k{k} at {i}: {g} vs {e}");
            }
        }
    }

    #[test]
    fn shuffles_invert_each_other() {
        let (h, w) = (4, 6);
        let x: Vec<f32> = (0..h * w).map(|i| i as f32).collect();
        let mut s = vec![0.0; h * w];
        space_to_depth(&x, 1, h, w, &mut s);
        assert_eq!(&s[..4], &[0.0, 1.0, 6.0, 7.0]);
        let mut back = vec![0.0; h * w];
        depth_to_space1(&s, h / 2, w / 2, &mut back);
        assert_eq!(x, back);
    }
}
