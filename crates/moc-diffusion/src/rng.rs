//! SplitMix64 (same generator as moc-terrain's `rng::Rng`) plus standard normals.

#[derive(Clone, Debug)]
pub struct SplitMix64 {
    s: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { s: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.s = self.s.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.s;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Fills `out` with independent N(0, 1) samples (Box-Muller, both outputs used).
    pub fn fill_normal(&mut self, out: &mut [f32]) {
        let mut i = 0;
        while i < out.len() {
            let u1 = ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64; // (0, 1)
            let u2 = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            let r = (-2.0 * u1.ln()).sqrt();
            let (s, c) = (2.0 * std::f64::consts::PI * u2).sin_cos();
            out[i] = (r * c) as f32;
            if i + 1 < out.len() {
                out[i + 1] = (r * s) as f32;
            }
            i += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normals_have_unit_moments() {
        let mut r = SplitMix64::new(7);
        let mut v = vec![0.0; 200_001];
        r.fill_normal(&mut v);
        let n = v.len() as f64;
        let m = v.iter().map(|&x| x as f64).sum::<f64>() / n;
        let var = v.iter().map(|&x| (x as f64 - m).powi(2)).sum::<f64>() / n;
        assert!(m.abs() < 0.01 && (var - 1.0).abs() < 0.01, "mean {m} var {var}");
    }
}
