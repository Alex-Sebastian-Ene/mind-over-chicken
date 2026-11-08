//! Emulated EEG headset (`SimSource`) with per-player fingerprint.
//!
//! Generates synthetic 10 Hz brain frames matching real Muse 2 EEG data,
//! allowing headless tests, bots, and demo setups without hardware.
//! Frames carry no heart rate or PPG: the game does not read either (#99, #153).

use crate::math::{clamp, smooth, Mulberry32};
use crate::model::Bands;

pub const FRAME_DT: f32 = 0.1;

#[derive(Debug, Clone, Copy)]
pub struct SimFingerprint {
    pub alpha_gain: f32,
    pub beta_gain: f32,
    pub theta_focus: f32,
    pub gamma_arousal: f32,
}

impl SimFingerprint {
    pub fn random(rng: &mut Mulberry32) -> Self {
        Self {
            alpha_gain: rng.range_f32(0.55, 1.25),
            beta_gain: rng.range_f32(0.55, 1.20),
            theta_focus: rng.range_f32(-0.4, 0.5),
            gamma_arousal: rng.range_f32(0.3, 1.0),
        }
    }

    pub fn default_preset() -> Self {
        Self {
            alpha_gain: 0.9,
            beta_gain: 0.85,
            theta_focus: 0.1,
            gamma_arousal: 0.6,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SimInput {
    pub calm: bool,
    pub focus: bool,
    pub panic: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SimLatent {
    pub calm: f32,
    pub focus: f32,
    pub arousal: f32,
}

impl Default for SimLatent {
    fn default() -> Self {
        Self {
            calm: 0.15,
            focus: 0.1,
            arousal: 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BrainFrame {
    pub bands: Bands,
    pub blink: bool,
    pub tilt: Option<(f32, f32)>,
    pub motion: f32,
    pub quality: f32,
}

#[derive(Debug, Clone)]
pub struct SimSource {
    pub fp: SimFingerprint,
    pub input: SimInput,
    pub latent: SimLatent,
    pub startle: f32,
    pub aim: (f32, f32),
    pub aim_vel: f32,
    pub pending_blink: bool,
    acc: f32,
    rng: Mulberry32,
}

impl SimSource {
    pub fn new(seed: u32) -> Self {
        let mut rng = Mulberry32::new(seed);
        let fp = SimFingerprint::random(&mut rng);
        Self {
            fp,
            input: SimInput::default(),
            latent: SimLatent::default(),
            startle: 0.0,
            aim: (0.5, 0.6),
            aim_vel: 0.0,
            pending_blink: false,
            acc: 0.0,
            rng,
        }
    }

    pub fn with_fingerprint(fp: SimFingerprint, seed: u32) -> Self {
        let rng = Mulberry32::new(seed);
        Self {
            fp,
            input: SimInput::default(),
            latent: SimLatent::default(),
            startle: 0.0,
            aim: (0.5, 0.6),
            aim_vel: 0.0,
            pending_blink: false,
            acc: 0.0,
            rng,
        }
    }

    pub fn blink(&mut self) {
        self.pending_blink = true;
    }

    pub fn set_aim(&mut self, x: f32, y: f32) {
        let dx = x - self.aim.0;
        let dy = y - self.aim.1;
        self.aim_vel += (dx * dx + dy * dy).sqrt();
        self.aim = (clamp(x, 0.0, 1.0), clamp(y, 0.0, 1.0));
    }

    pub fn add_startle(&mut self, amount: f32) {
        self.startle = (self.startle + amount).min(1.0);
    }

    pub fn update<F>(&mut self, dt: f32, mut emit: F)
    where
        F: FnMut(BrainFrame),
    {
        let target_calm = if self.input.calm {
            0.95
        } else if self.input.panic {
            0.02
        } else {
            0.15
        };
        let rate_calm = if self.input.calm { 1.1 } else { 1.6 };
        self.latent.calm = smooth(self.latent.calm, target_calm, rate_calm, dt);

        let target_focus = if self.input.focus { 0.95 } else { 0.12 };
        let rate_focus = if self.input.focus { 1.3 } else { 1.8 };
        self.latent.focus = smooth(self.latent.focus, target_focus, rate_focus, dt);

        let arousal_target = if self.input.panic {
            1.0
        } else {
            clamp(
                0.12 + 0.06 * self.latent.focus - 0.12 * self.latent.calm + self.startle * 0.8,
                0.0,
                1.0,
            )
        };
        let arousal_rate = if self.input.panic { 0.9 } else { 0.35 };
        self.latent.arousal = smooth(self.latent.arousal, arousal_target, arousal_rate, dt);

        self.startle = (self.startle - dt * 0.12).max(0.0);

        self.acc += dt;
        while self.acc >= FRAME_DT {
            self.acc -= FRAME_DT;
            let frame = self.make_frame();
            emit(frame);
        }
    }

    fn make_frame(&mut self) -> BrainFrame {
        let l = self.latent;
        let f = self.fp;

        let mut n = || (self.rng.gauss() * 0.16).exp();

        let mut delta = 0.26 * n();
        let mut theta = (0.17 + 0.06 * f.theta_focus * l.focus + 0.03 * l.calm) * n();
        let mut alpha =
            (0.17 + 0.32 * f.alpha_gain * l.calm - 0.07 * l.focus - 0.06 * l.arousal) * n();
        let mut beta =
            (0.15 + 0.26 * f.beta_gain * l.focus + 0.1 * l.arousal - 0.05 * l.calm) * n();
        let gamma = (0.05 + 0.06 * f.gamma_arousal * l.arousal + 0.02 * l.focus) * n();

        let blink = self.pending_blink;
        if blink {
            delta *= 3.2;
        }

        alpha = alpha.max(0.02);
        beta = beta.max(0.02);
        theta = theta.max(0.02);

        let sum = delta + theta + alpha + beta + gamma;
        self.pending_blink = false;

        let motion = clamp(self.aim_vel * 6.0, 0.0, 1.0);
        self.aim_vel = 0.0;

        BrainFrame {
            bands: Bands {
                delta: delta / sum,
                theta: theta / sum,
                alpha: alpha / sum,
                beta: beta / sum,
                gamma: gamma / sum,
            },
            blink,
            tilt: Some((self.aim.0 * 2.0 - 1.0, self.aim.1 * 2.0 - 1.0)),
            motion,
            quality: 1.0,
        }
    }
}
