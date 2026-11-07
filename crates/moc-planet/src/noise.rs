//! Value noise that tiles east–west, so terrain has no seam where the planet wraps.

use crate::rng::Rng;

pub struct WrapNoise {
    grid: Vec<f32>,
    gw: usize,
    gh: usize,
    /// Cells around the planet at the base octave.
    period: usize,
}

impl WrapNoise {
    /// `period` cells span the full longitude at octave 0; octaves up to `octaves`
    /// keep tiling because the grid is `period * 2^(octaves-1)` cells wide.
    pub fn new(rng: &mut Rng, period: usize, octaves: u32) -> Self {
        let gw = period << (octaves.saturating_sub(1));
        let gh = gw;
        let grid = (0..gw * gh).map(|_| rng.f32()).collect();
        Self { grid, gw, gh, period }
    }

    #[inline]
    fn at(&self, x: i64, y: i64, period: i64) -> f32 {
        let xi = x.rem_euclid(period) as usize;
        let yi = y.clamp(0, self.gh as i64 - 1) as usize;
        self.grid[yi * self.gw + xi]
    }

    fn value(&self, x: f32, y: f32, period: i64) -> f32 {
        let sm = |t: f32| t * t * (3.0 - 2.0 * t);
        let xi = x.floor() as i64;
        let yi = y.floor() as i64;
        let xf = sm(x - xi as f32);
        let yf = sm(y - yi as f32);
        let a = self.at(xi, yi, period);
        let b = self.at(xi + 1, yi, period);
        let c = self.at(xi, yi + 1, period);
        let d = self.at(xi + 1, yi + 1, period);
        let top = a + (b - a) * xf;
        let bot = c + (d - c) * xf;
        top + (bot - top) * yf
    }

    /// Fractal noise in [0, 1]. `u` is longitude in [0, 1) (wraps), `v` is in planet cells.
    pub fn fbm(&self, u: f32, v: f32, octaves: u32) -> f32 {
        let (mut s, mut amp, mut norm) = (0.0, 0.5, 0.0);
        for o in 0..octaves {
            let f = 1i64 << o;
            let period = self.period as i64 * f;
            s += amp * self.value(u * period as f32, v * f as f32, period);
            norm += amp;
            amp *= 0.5;
        }
        s / norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_around_the_planet() {
        let mut r = Rng::new(42);
        let n = WrapNoise::new(&mut r, 8, 4);
        for i in 0..20 {
            let v = i as f32 * 0.21;
            assert!((n.fbm(0.0, v, 4) - n.fbm(1.0, v, 4)).abs() < 1e-5);
        }
    }
}
