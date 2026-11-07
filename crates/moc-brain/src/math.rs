//! Shared math, statistics, and deterministic random utilities for moc-brain.

pub const EPS: f32 = 1e-4;

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
pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[inline]
pub fn round3(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}

pub fn median(arr: &[f32]) -> f32 {
    if arr.is_empty() {
        return 0.0;
    }
    let mut s = arr.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let m = s.len() / 2;
    if s.len() % 2 == 1 {
        s[m]
    } else {
        (s[m - 1] + s[m]) * 0.5
    }
}

/// Seeded Mulberry32 pseudo-random generator (identical to JS mulberry32 in src/sim/util.js).
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

    /// Next u32 state step
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6d2b79f5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    /// Float in [0, 1) matching JS `mulberry32(seed)()`:
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f64 / 4294967296.0) as f32
    }

    /// Float in [lo, hi)
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    /// Gaussian standard normal distribution (Box-Muller)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mulberry32_sequence() {
        let mut rng = Mulberry32::new(1337);
        let a = rng.next_f32();
        let b = rng.next_f32();
        assert!(a >= 0.0 && a < 1.0);
        assert!(b >= 0.0 && b < 1.0);
        assert_ne!(a, b);

        let mut rng2 = Mulberry32::new(1337);
        assert_eq!(a, rng2.next_f32());
        assert_eq!(b, rng2.next_f32());
    }

    #[test]
    fn test_median() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[]), 0.0);
    }
}
