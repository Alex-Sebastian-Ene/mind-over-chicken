//! Telemetry data structures and synthetic generator for the headset bridge.
//!
//! Matches the exact JSON wire protocol expected by `BridgeSource` in `src/brain/sources.js`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BandPowers {
    pub delta: f32,
    pub theta: f32,
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct HeadOrientation {
    pub pitch: f32,
    pub roll: f32,
    pub yaw: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BridgeFrame {
    pub t: f32,
    pub bands: BandPowers,
    pub blink: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accel: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gyro: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<HeadOrientation>,
    pub motion: f32,
    pub quality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusMessage {
    pub r#type: String,
    pub text: String,
}

impl StatusMessage {
    pub fn new(text: &str) -> Self {
        Self {
            r#type: "status".to_string(),
            text: text.to_string(),
        }
    }
}

/// Deterministic / synthetic generator mimicking a Muse 2 headset
#[derive(Debug, Clone)]
pub struct SyntheticGenerator {
    pub time: f32,
    pub hr: f32,
    pub calib_phase: f32,
    pub blink_timer: f32,
}

impl Default for SyntheticGenerator {
    fn default() -> Self {
        Self::new(1337)
    }
}

impl SyntheticGenerator {
    pub fn new(_seed: u32) -> Self {
        Self {
            time: 0.0,
            hr: 72.0,
            calib_phase: 0.0,
            blink_timer: 0.0,
        }
    }

    pub fn next_frame(&mut self, dt: f32) -> BridgeFrame {
        self.time += dt;
        self.calib_phase += dt * 0.5;
        self.blink_timer += dt;

        // Occasional blink every 3..5 seconds
        let blink = if self.blink_timer > 3.8 {
            self.blink_timer = 0.0;
            true
        } else {
            false
        };

        // Smoothly fluctuating band powers
        let wave = (self.calib_phase).sin();
        let mut delta = 0.22 + 0.05 * (wave * 0.8).cos();
        let theta = 0.16 + 0.03 * wave.cos();
        let alpha = 0.24 + 0.10 * wave;
        let beta = 0.25 - 0.08 * wave;
        let gamma = 0.13 + 0.02 * (wave * 1.5).sin();

        if blink {
            delta *= 2.8;
        }

        let total = delta + theta + alpha + beta + gamma;
        let bands = BandPowers {
            delta: delta / total,
            theta: theta / total,
            alpha: alpha / total,
            beta: beta / total,
            gamma: gamma / total,
        };

        // Simulated pulse around 68-74 bpm
        self.hr = 70.0 + 3.0 * (self.time * 0.2).sin();

        // Simulated head tilt
        let pitch = 2.5 * (self.time * 0.4).sin();
        let roll = 1.5 * (self.time * 0.3).cos();
        let ax = roll / 57.3;
        let ay = pitch / 57.3;
        let az = 0.98;

        BridgeFrame {
            t: (self.time * 10.0).round() / 10.0,
            bands,
            blink,
            hr: None,
            accel: Some([ax, ay, az]),
            gyro: Some([0.0, 0.0, 0.0]),
            orientation: Some(HeadOrientation {
                pitch,
                roll,
                yaw: 0.0,
            }),
            motion: 0.02,
            quality: 1.0,
        }
    }
}
