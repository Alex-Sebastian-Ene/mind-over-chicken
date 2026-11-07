//! Brain signal pipeline blending generic EEG rules with the personal model.
//!
//! Handles rolling feature windows, resting heart-rate estimation,
//! blink debouncing, and trust-weighted score blending.

use crate::math::{clamp, median, round3, sigmoid, smooth};
use crate::model::{features_from_bands, Class, PersonalModel, Predictions, NUM_FEATURES};
use crate::sim_source::BrainFrame;

pub const WINDOW_SIZE: usize = 6;
pub const TRAIN_EVERY_SECS: f32 = 2.5;

#[derive(Debug, Clone, Copy, Default)]
pub struct BrainStats {
    pub blinks: u32,
    pub calm_seconds: f32,
    pub focus_seconds: f32,
    pub peak_hr: f32,
    pub frames_seen: u32,
}

#[derive(Debug, Clone)]
pub struct Brain {
    pub model: PersonalModel,
    window: Vec<[f32; NUM_FEATURES]>,
    pub feat: Option<[f32; NUM_FEATURES]>,
    pub calm: f32,
    pub focus: f32,
    pub calm_raw: f32,
    pub focus_raw: f32,
    pub generic_calm: f32,
    pub generic_focus: f32,
    pub personal: Option<Predictions>,
    pub hr: Option<f32>,
    hr_history: Vec<(f32, f32)>, // (time, hr)
    pub rest_hr: Option<f32>,
    pub aim: (f32, f32),
    pub motion: f32,
    pub quality: f32,
    pub cue: Option<Class>,
    pub last_blink_at: f32,
    pub time: f32,
    pub train_timer: f32,
    rest_ref_ab: Vec<f32>,
    rest_ref_bat: Vec<f32>,
    pub stats: BrainStats,
}

impl Default for Brain {
    fn default() -> Self {
        Self::new()
    }
}

impl Brain {
    pub fn new() -> Self {
        Self {
            model: PersonalModel::new(),
            window: Vec::with_capacity(WINDOW_SIZE),
            feat: None,
            calm: 0.0,
            focus: 0.0,
            calm_raw: 0.0,
            focus_raw: 0.0,
            generic_calm: 0.0,
            generic_focus: 0.0,
            personal: None,
            hr: None,
            hr_history: Vec::new(),
            rest_hr: None,
            aim: (0.5, 0.55),
            motion: 0.0,
            quality: 1.0,
            cue: None,
            last_blink_at: -10.0,
            time: 0.0,
            train_timer: 0.0,
            rest_ref_ab: Vec::new(),
            rest_ref_bat: Vec::new(),
            stats: BrainStats::default(),
        }
    }

    pub fn set_cue(&mut self, cue: Option<Class>) {
        self.cue = cue;
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.time += dt;

        // Smooth outputs to prevent noise spikes from accidentally tripping mechanics
        self.calm = smooth(self.calm, self.calm_raw, 4.0, dt);
        self.focus = smooth(self.focus, self.focus_raw, 4.0, dt);

        if self.calm > 0.6 {
            self.stats.calm_seconds += dt;
        }
        if self.focus > 0.6 {
            self.stats.focus_seconds += dt;
        }

        self.train_timer += dt;
        if self.train_timer > TRAIN_EVERY_SECS {
            self.train_timer = 0.0;
            self.model.maybe_train()
        } else {
            false
        }
    }

    /// Ingest a frame. Returns `true` if a debounced blink event occurred.
    pub fn ingest(&mut self, frame: &BrainFrame) -> bool {
        self.stats.frames_seen += 1;
        self.quality = frame.quality;

        if let Some((tx, ty)) = frame.tilt {
            self.aim.0 = smooth(self.aim.0, clamp(0.5 + tx * 0.5, 0.0, 1.0), 10.0, 0.1);
            self.aim.1 = smooth(self.aim.1, clamp(0.5 + ty * 0.5, 0.0, 1.0), 10.0, 0.1);
        }
        self.motion = frame.motion;

        if let Some(hr_val) = frame.hr {
            self.hr = Some(hr_val);
            self.hr_history.push((self.time, hr_val));

            // Keep up to 3 minutes (180 s)
            while !self.hr_history.is_empty() && self.time - self.hr_history[0].0 > 180.0 {
                self.hr_history.remove(0);
            }

            if self.hr_history.len() > 20 {
                // Resting pulse is approximately the 25th percentile of the last 3 minutes
                let mut sorted: Vec<f32> = self.hr_history.iter().map(|&(_, h)| h).collect();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let p25_idx = (sorted.len() as f32 * 0.25).floor() as usize;
                self.rest_hr = Some(sorted[p25_idx]);
            } else if self.rest_hr.is_none() {
                self.rest_hr = Some(hr_val);
            }

            self.stats.peak_hr = self.stats.peak_hr.max(hr_val);
        }

        // Handle blink artifact detection & debouncing (0.45 s lockout)
        if frame.blink {
            if self.time - self.last_blink_at > 0.45 {
                self.last_blink_at = self.time;
                self.stats.blinks += 1;
                return true;
            }
            // Blink frames contain electrical artifact: keep out of feature windows
            return false;
        }

        let f_instant = features_from_bands(&frame.bands);
        self.window.push(f_instant);
        if self.window.len() > WINDOW_SIZE {
            self.window.remove(0);
        }
        if self.window.len() < 3 {
            return false;
        }

        let mut f_avg = [0.0f32; NUM_FEATURES];
        let n = self.window.len() as f32;
        for w in &self.window {
            for j in 0..NUM_FEATURES {
                f_avg[j] += w[j] / n;
            }
        }
        self.feat = Some(f_avg);

        if let Some(cue) = self.cue {
            self.model.add(cue, f_avg);
            if cue == Class::Rest {
                self.rest_ref_ab.push(f_avg[5]);
                self.rest_ref_bat.push(f_avg[6]);
                if self.rest_ref_ab.len() > 300 {
                    self.rest_ref_ab.remove(0);
                    self.rest_ref_bat.remove(0);
                }
            }
        }

        // Generic rule:
        // f[5] = ln(alpha/beta) -> calm
        // f[6] = ln(beta/(alpha+theta)) -> focus
        let base_ab = if self.rest_ref_ab.len() > 20 {
            median(&self.rest_ref_ab)
        } else {
            0.1
        };
        let base_bat = if self.rest_ref_bat.len() > 20 {
            median(&self.rest_ref_bat)
        } else {
            -0.8
        };

        self.generic_calm = sigmoid((f_avg[5] - base_ab - 0.45) * 4.5);
        self.generic_focus = sigmoid((f_avg[6] - base_bat - 0.4) * 4.5);

        let p = self.model.predict(&f_avg);
        self.personal = p;

        let tc = self.model.trust(Class::Calm);
        let tf = self.model.trust(Class::Focus);

        self.calm_raw = if let Some(p_calm) = p.and_then(|pred| pred.calm) {
            self.generic_calm * (1.0 - tc) + p_calm * tc
        } else {
            self.generic_calm
        };

        self.focus_raw = if let Some(p_focus) = p.and_then(|pred| pred.focus) {
            self.generic_focus * (1.0 - tf) + p_focus * tf
        } else {
            self.generic_focus
        };

        false
    }

    /// Dynamic threshold based on how much the model knows the player
    pub fn threshold(&self, kind: Class, base: f32) -> f32 {
        clamp(base + 0.14 * self.model.trust(kind), 0.0, 0.85)
    }

    /// Heart rate delta above resting baseline
    pub fn hr_delta(&self) -> f32 {
        match (self.hr, self.rest_hr) {
            (Some(hr), Some(rest_hr)) => round3(hr - rest_hr),
            _ => 0.0,
        }
    }
}
