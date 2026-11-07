//! Cylinder-wrapping noise primitives: domain-warped fBm, ridged multifractals,
//! and cellular distance fields with seamless east-west boundary wrapping.

use moc_planet::rng::Rng;

pub struct TerrainNoise {
    grid: Vec<f32>,
    gw: usize,
    gh: usize,
    period: usize,
}

impl TerrainNoise {
    pub fn new(rng: &mut Rng, period: usize, octaves: u32) -> Self {
        let gw = period << (octaves.saturating_sub(1));
        let gh = gw;
        let grid = (0..gw * gh).map(|_| rng.f32()).collect();
        Self { grid, gw, gh, period }
    }

    #[inline]
    fn at(&self, x: i64, y: i64, period: i64) -> f32 {
        let xi = x.rem_euclid(period) as usize;
        let yi = (y.clamp(0, self.gh as i64 - 1)) as usize;
        self.grid[yi * self.gw + xi]
    }

    pub fn sample(&self, u: f32, v: f32, period: i64) -> f32 {
        let sm = |t: f32| t * t * (3.0 - 2.0 * t);
        let x = u * period as f32;
        let y = v;
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

    /// Fractal Brownian Motion in [0, 1].
    /// `u` in [0, 1) is longitude (cylinder wraps east-west).
    /// `v` is latitude / Y coordinate.
    pub fn fbm(&self, u: f32, v: f32, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut norm) = (0.0, 0.5, 0.0);
        for o in 0..octaves {
            let f = 1i64 << o;
            let period = self.period as i64 * f;
            sum += amp * self.sample(u, v * (f as f32), period);
            norm += amp;
            amp *= 0.5;
        }
        sum / norm
    }

    /// Domain-warped fBm for organic continental coastlines and swirling fluid shapes.
    pub fn warped_fbm(&self, u: f32, v: f32, warp_strength: f32, octaves: u32) -> f32 {
        let q_u = self.fbm(u, v, octaves.min(3));
        let q_v = self.fbm(u + 0.31, v + 5.2, octaves.min(3));

        let warped_u = (u + warp_strength * (q_u - 0.5)).rem_euclid(1.0);
        let warped_v = v + warp_strength * (q_v - 0.5) * (self.period as f32);

        self.fbm(warped_u, warped_v, octaves)
    }

    /// Ridged multifractal noise for sharp alpine mountain crests and fault scarps.
    /// Inverted absolute value creates sharp ridges at value transitions.
    pub fn ridged_fbm(&self, u: f32, v: f32, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut norm) = (0.0, 0.5, 0.0);
        for o in 0..octaves {
            let f = 1i64 << o;
            let period = self.period as i64 * f;
            let val = self.sample(u, v * (f as f32), period);
            let ridge = 1.0 - (2.0 * val - 1.0).abs();
            let ridge_sq = ridge * ridge;
            sum += amp * ridge_sq;
            norm += amp;
            amp *= 0.5;
        }
        sum / norm
    }
}
