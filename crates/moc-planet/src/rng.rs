//! Mulberry32: tiny, fast and seedable. Produces the same sequence as the JS build's
//! `mulberry32`, so a seed means the same world in both while the port is in progress.

#[derive(Clone, Debug)]
pub struct Rng {
    a: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { a: seed }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.a = self.a.wrapping_add(0x6d2b_79f5);
        let mut t = self.a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4_294_967_296.0
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        self.f64() as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32() * (hi - lo)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn below(&mut self, n: usize) -> usize {
        ((self.f64() * n as f64) as usize).min(n.saturating_sub(1))
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }

    /// Standard normal sample (Box–Muller).
    pub fn gauss(&mut self) -> f32 {
        let mut u = 0.0;
        while u == 0.0 {
            u = self.f64();
        }
        let v = self.f64();
        ((-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_js_mulberry32_sequence() {
        // Reference values from the JS build: mulberry32(12345)
        let mut r = Rng::new(12345);
        let expect = [0.979728267761, 0.306752264500, 0.484205421526];
        for e in expect {
            assert!((r.f64() - e).abs() < 1e-9);
        }
    }

    #[test]
    fn gauss_is_roughly_standard() {
        let mut r = Rng::new(7);
        let n = 20_000;
        let xs: Vec<f32> = (0..n).map(|_| r.gauss()).collect();
        let mean = xs.iter().sum::<f32>() / n as f32;
        let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / n as f32;
        assert!(mean.abs() < 0.03, "mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "var {var}");
    }
}
