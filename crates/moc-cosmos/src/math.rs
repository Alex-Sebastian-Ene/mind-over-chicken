//! Deterministic math and PRNG utilities for cosmos simulation.

#[inline]
pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn smooth(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-rate * dt).exp())
}

#[inline]
pub fn round3(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}

/// Seeded Mulberry32 pseudo-random number generator (matches JS mulberry32 in src/sim/util.js).
#[derive(Debug, Clone)]
pub struct Mulberry32 {
    state: u32,
    has_spare_gaussian: bool,
    spare_gaussian: f32,
}

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed,
            has_spare_gaussian: false,
            spare_gaussian: 0.0,
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6d2b79f5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f64 / 4294967296.0) as f32
    }

    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    pub fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        assert!(!slice.is_empty());
        let idx = (self.next_f32() * slice.len() as f32).floor() as usize;
        &slice[idx.min(slice.len() - 1)]
    }

    /// Box-Muller Gaussian standard normal N(0, 1)
    pub fn gauss(&mut self) -> f32 {
        if self.has_spare_gaussian {
            self.has_spare_gaussian = false;
            return self.spare_gaussian;
        }

        let mut u = 0.0f32;
        while u <= 1e-7 {
            u = self.next_f32();
        }
        let mut v = 0.0f32;
        while v <= 1e-7 {
            v = self.next_f32();
        }

        let r = (-2.0 * u.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * v;
        self.spare_gaussian = r * theta.sin();
        self.has_spare_gaussian = true;
        r * theta.cos()
    }
}
