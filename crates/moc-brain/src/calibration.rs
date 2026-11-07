//! Calibration seed & trait generation (#54, #63, README §3.1 / §4.1).
//!
//! A configurable window at the start of play transforms the player's brain & face signals
//! into seed traits for their starting civilisation: friendliness, aggressiveness, and violence.

use crate::math::{clamp, round3, Mulberry32};

pub const CALIBRATION_SECONDS: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationMode {
    Random,
    Sensor,
}

impl Default for CalibrationMode {
    fn default() -> Self {
        CalibrationMode::Random
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CalibrationConfig {
    pub seconds: f32,
    pub mode: CalibrationMode,
    pub seed: u32,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            seconds: CALIBRATION_SECONDS,
            mode: CalibrationMode::Random,
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CivTraits {
    pub friendliness: f32,
    pub aggressiveness: f32,
    pub violence: f32,
}

/// Random setup for the dummy run: reproducible from the run seed (matches JS randomSeedTraits).
pub fn random_seed_traits(seed: u32) -> CivTraits {
    let mut rng = Mulberry32::new(seed);
    CivTraits {
        friendliness: round3(rng.next_f32()),
        aggressiveness: round3(rng.next_f32()),
        violence: round3(rng.next_f32() * 0.8),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SensorFeatures {
    pub calm: f32,
    pub focus: f32,
    pub hr_delta: f32,
    pub blink_rate: f32,
    pub joy: f32,
    pub anger: f32,
    pub surprise: f32,
    pub sadness: f32,
    pub face_seen: bool,
}

/// Sensor-to-trait formula (documented in docs/calibration.md and PR #63):
pub fn traits_from_features(f: &SensorFeatures) -> CivTraits {
    let pulse = clamp(f.hr_delta / 20.0, 0.0, 1.0);
    let blinks = clamp(f.blink_rate / 6.0, 0.0, 1.0);
    let calm = clamp(f.calm, 0.0, 1.0);
    let focus = clamp(f.focus, 0.0, 1.0);
    let joy = clamp(f.joy, 0.0, 1.0);
    let anger = clamp(f.anger, 0.0, 1.0);

    CivTraits {
        friendliness: round3(clamp(0.55 * calm + 0.35 * joy + 0.1 * (1.0 - anger), 0.0, 1.0)),
        aggressiveness: round3(clamp(0.5 * pulse + 0.35 * anger + 0.15 * focus, 0.0, 1.0)),
        violence: round3(clamp(
            0.45 * anger + 0.35 * pulse * (1.0 - calm) + 0.2 * blinks,
            0.0,
            1.0,
        )),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MoodInput {
    pub active: bool,
    pub face: bool,
    pub joy: f32,
    pub anger: f32,
    pub surprise: f32,
    pub sadness: f32,
}

#[derive(Debug, Clone)]
pub struct CalibrationResult {
    pub traits: CivTraits,
    pub source: CalibrationMode,
    pub seconds: f32,
    pub seed: u32,
    pub features: SensorFeatures,
}

#[derive(Debug, Clone, Default)]
struct Accumulators {
    calm: f32,
    focus: f32,
    hr_delta: f32,
    joy: f32,
    anger: f32,
    surprise: f32,
    sadness: f32,
    face_frames: f32,
    n: f32,
}

#[derive(Debug, Clone)]
pub struct Calibration {
    pub config: CalibrationConfig,
    pub t: f32,
    pub done: bool,
    pub result: Option<CalibrationResult>,
    acc: Accumulators,
    blinks_at_start: Option<u32>,
}

impl Calibration {
    pub fn new(config: CalibrationConfig) -> Self {
        Self {
            config,
            t: 0.0,
            done: false,
            result: None,
            acc: Accumulators::default(),
            blinks_at_start: None,
        }
    }

    pub fn remaining(&self) -> f32 {
        (self.config.seconds - self.t).max(0.0)
    }

    pub fn update(
        &mut self,
        dt: f32,
        calm: f32,
        focus: f32,
        hr_delta: f32,
        total_blinks: u32,
        mood: Option<MoodInput>,
    ) {
        if self.done || dt <= 0.0 {
            return;
        }

        if self.blinks_at_start.is_none() {
            self.blinks_at_start = Some(total_blinks);
        }

        self.acc.calm += calm * dt;
        self.acc.focus += focus * dt;
        self.acc.hr_delta += hr_delta * dt;

        if let Some(m) = mood {
            if m.active && m.face {
                self.acc.joy += m.joy * dt;
                self.acc.anger += m.anger * dt;
                self.acc.surprise += m.surprise * dt;
                self.acc.sadness += m.sadness * dt;
                self.acc.face_frames += dt;
            }
        }

        self.acc.n += dt;
        self.t += dt;

        if self.t >= self.config.seconds {
            self.finish(total_blinks);
        }
    }

    pub fn finish(&mut self, total_blinks: u32) {
        let n = self.acc.n.max(1e-6);
        let face = if self.acc.face_frames > 0.0 {
            self.acc.face_frames
        } else {
            0.0
        };
        let start_blinks = self.blinks_at_start.unwrap_or(total_blinks);
        let blinks = total_blinks.saturating_sub(start_blinks) as f32;

        let features = SensorFeatures {
            calm: round3(self.acc.calm / n),
            focus: round3(self.acc.focus / n),
            hr_delta: round3(self.acc.hr_delta / n),
            blink_rate: round3((blinks / n) * 10.0),
            joy: if face > 0.0 {
                round3(self.acc.joy / face)
            } else {
                0.0
            },
            anger: if face > 0.0 {
                round3(self.acc.anger / face)
            } else {
                0.0
            },
            surprise: if face > 0.0 {
                round3(self.acc.surprise / face)
            } else {
                0.0
            },
            sadness: if face > 0.0 {
                round3(self.acc.sadness / face)
            } else {
                0.0
            },
            face_seen: face > 0.0,
        };

        let traits = match self.config.mode {
            CalibrationMode::Sensor => traits_from_features(&features),
            CalibrationMode::Random => random_seed_traits(self.config.seed),
        };

        self.result = Some(CalibrationResult {
            traits,
            source: self.config.mode,
            seconds: self.config.seconds,
            seed: self.config.seed,
            features,
        });
        self.done = true;
    }
}
