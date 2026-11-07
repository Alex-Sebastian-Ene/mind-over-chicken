//! Observation space and feature extraction for the Stone Age environment.
//!
//! Reimplementation of `src/ai/observe.js`.
//! Normalizes game state into a 20-dimensional feature vector in [0, 1].

use crate::policy::NUM_INPUTS;

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

/// Raw state representation of the Stone Age scene to observe
#[derive(Debug, Clone, Default)]
pub struct StoneAgeRawState {
    pub fire_lit: bool,
    pub fire_heat: f32,
    pub food: f32,
    pub living_people: usize,
    pub avg_hunger: f32,
    pub starving_count: usize,
    pub avg_fear: f32,
    pub hold_active: bool,
    pub hold_progress: f32,
    pub hold_successes: usize,
    pub nearest_mammoth_dist: f32,
    pub living_mammoths: usize,
    pub colonies: usize,
    pub rafts_in_transit: usize,
    pub people_overseas: usize,
    pub elapsed_time: f32,
    pub calm: f32,
    pub calm_threshold: f32,
    pub bolt_ready: bool,
    pub meat_count: f32,
}

impl StoneAgeRawState {
    /// Compute the 20-dimensional normalized observation vector
    pub fn to_observation_vector(&self) -> [f32; NUM_INPUTS] {
        let alive_frac = clamp(self.living_people as f32 / 30.0, 0.0, 1.0);
        let starving_frac = if self.living_people > 0 {
            clamp(self.starving_count as f32 / self.living_people as f32, 0.0, 1.0)
        } else {
            0.0
        };
        let overseas_frac = if self.living_people > 0 {
            clamp(self.people_overseas as f32 / self.living_people as f32, 0.0, 1.0)
        } else {
            0.0
        };

        let calm_margin = clamp(
            (self.calm - self.calm_threshold + 0.3) / 0.6,
            0.0,
            1.0,
        );

        [
            if self.fire_lit { 1.0 } else { 0.0 },
            clamp(self.fire_heat, 0.0, 1.0),
            clamp(self.food / 20.0, 0.0, 1.0),
            alive_frac,
            clamp(self.avg_hunger, 0.0, 1.0),
            starving_frac,
            clamp(self.avg_fear, 0.0, 1.0),
            if self.hold_active { 1.0 } else { 0.0 },
            clamp(self.hold_progress, 0.0, 1.0),
            clamp(self.hold_successes as f32 / 3.0, 0.0, 1.0),
            clamp(self.nearest_mammoth_dist / 200.0, 0.0, 1.0),
            clamp(self.living_mammoths as f32 / 5.0, 0.0, 1.0),
            clamp(self.colonies as f32 / 2.0, 0.0, 1.0),
            clamp(self.rafts_in_transit as f32 / 2.0, 0.0, 1.0),
            overseas_frac,
            clamp(self.elapsed_time / 120.0, 0.0, 1.0),
            clamp(self.calm, 0.0, 1.0),
            calm_margin,
            if self.bolt_ready { 1.0 } else { 0.0 },
            clamp(self.meat_count / 20.0, 0.0, 1.0),
        ]
    }
}
