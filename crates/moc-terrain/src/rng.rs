//! Deterministic randomness: SplitMix64 for streams, integer hashes for coherent per-cell values.
//! Same seed, same numbers, on every platform (no floating point in the generators).

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    /// An independent stream for one stage, so adding draws to one stage never shifts another.
    pub fn stream(seed: u32, salt: u64) -> Self {
        let mut r = Rng::new(((seed as u64) << 32) ^ salt.wrapping_mul(0xD1B5_4A32_D192_ED03));
        r.next_u64();
        r
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[inline]
    pub fn u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    #[inline]
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32() * (hi - lo)
    }

    /// Uniform in 0..n (n > 0).
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        (((self.next_u64() >> 32) * n as u64) >> 32) as usize
    }

    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Integer in lo..=hi.
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1).max(1) as usize) as i32
    }
}

/// A well-mixed 32-bit hash of two coordinates and a seed.
#[inline]
pub fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^ (h >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_reproducible_and_distinct() {
        let a: Vec<u32> = (0..8).map({ let mut r = Rng::stream(7, 1); move |_| r.u32() }).collect();
        let b: Vec<u32> = (0..8).map({ let mut r = Rng::stream(7, 1); move |_| r.u32() }).collect();
        let c: Vec<u32> = (0..8).map({ let mut r = Rng::stream(7, 2); move |_| r.u32() }).collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut r = Rng::new(3);
        for _ in 0..1000 {
            let v = r.f32();
            assert!((0.0..1.0).contains(&v));
            assert!(r.below(5) < 5);
        }
    }
}
