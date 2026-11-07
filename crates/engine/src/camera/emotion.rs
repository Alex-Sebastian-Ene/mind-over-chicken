//! Face emotion tracker and blendshape mapping (#33, #108).
//! Pure logic port of src/camera/emotion.js.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Emotion {
    Neutral,
    Joy,
    Anger,
    Surprise,
    Sadness,
}

impl Emotion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Emotion::Neutral => "neutral",
            Emotion::Joy => "joy",
            Emotion::Anger => "anger",
            Emotion::Surprise => "surprise",
            Emotion::Sadness => "sadness",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "joy" => Emotion::Joy,
            "anger" => Emotion::Anger,
            "surprise" => Emotion::Surprise,
            "sadness" => Emotion::Sadness,
            _ => Emotion::Neutral,
        }
    }
}

pub const EMOTIONS: [Emotion; 4] = [
    Emotion::Joy,
    Emotion::Anger,
    Emotion::Surprise,
    Emotion::Sadness,
];

pub const RANGE_JOY: (f64, f64) = (0.1, 0.45);
pub const RANGE_ANGER: (f64, f64) = (0.08, 0.32);
pub const RANGE_SURPRISE: (f64, f64) = (0.1, 0.35);
pub const RANGE_SADNESS: (f64, f64) = (0.06, 0.29);

#[inline]
fn clamp(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

#[inline]
fn smooth(curr: f64, target: f64, rate: f64, dt: f64) -> f64 {
    curr + (target - curr) * (1.0 - (-rate * dt).exp())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawEmotions {
    pub joy: f64,
    pub anger: f64,
    pub surprise: f64,
    pub sadness: f64,
}

pub fn raw_emotions(b: &HashMap<String, f64>) -> RawEmotions {
    let v = |k: &str| -> f64 { clamp(*b.get(k).unwrap_or(&0.0)) };

    let smile = 0.5 * v("mouthSmileLeft") + 0.5 * v("mouthSmileRight");
    let anger_raw = 0.3 * v("browDownLeft")
        + 0.3 * v("browDownRight")
        + 0.2 * v("mouthPressLeft")
        + 0.2 * v("mouthPressRight");
    let surprise_raw = 0.4 * v("jawOpen")
        + 0.3 * v("browInnerUp")
        + 0.15 * v("eyeWideLeft")
        + 0.15 * v("eyeWideRight");
    let sadness_raw = 0.35 * v("mouthFrownLeft")
        + 0.35 * v("mouthFrownRight")
        + 0.3 * v("browInnerUp");

    let jaw_open = v("jawOpen");

    RawEmotions {
        joy: smile,
        anger: anger_raw * (1.0 - 0.7 * smile),
        surprise: surprise_raw,
        sadness: sadness_raw * (1.0 - jaw_open) * (1.0 - smile),
    }
}

pub fn emotions_from_blendshapes(
    categories: &HashMap<String, f64>,
    baseline: Option<&RawEmotions>,
) -> RawEmotions {
    let raw = raw_emotions(categories);
    let base = baseline.copied().unwrap_or(RawEmotions {
        joy: 0.0,
        anger: 0.0,
        surprise: 0.0,
        sadness: 0.0,
    });

    let joy = clamp((raw.joy - base.joy - RANGE_JOY.0) / RANGE_JOY.1);
    let anger = clamp((raw.anger - base.anger - RANGE_ANGER.0) / RANGE_ANGER.1);
    let surprise = clamp((raw.surprise - base.surprise - RANGE_SURPRISE.0) / RANGE_SURPRISE.1);
    let sadness = clamp((raw.sadness - base.sadness - RANGE_SADNESS.0) / RANGE_SADNESS.1);

    RawEmotions {
        joy,
        anger,
        surprise,
        sadness,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mood {
    pub joy: f64,
    pub anger: f64,
    pub surprise: f64,
    pub sadness: f64,
    pub neutral: f64,
    pub dominant: Emotion,
    pub intensity: f64,
    pub face: bool,
}

impl Default for Mood {
    fn default() -> Self {
        Self {
            joy: 0.0,
            anger: 0.0,
            surprise: 0.0,
            sadness: 0.0,
            neutral: 1.0,
            dominant: Emotion::Neutral,
            intensity: 1.0,
            face: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmotionEvent {
    Change,
    Strong,
}

#[derive(Debug, Clone)]
pub struct EmotionTracker {
    pub rate: f64,
    pub enter: f64,
    pub exit: f64,
    pub switch_margin: f64,
    pub strong: f64,
    pub strong_every: f64,
    pub calibrate_seconds: f64,
    pub max_baseline: f64,

    pub scores: [f64; 4], // joy, anger, surprise, sadness
    pub dominant: Emotion,
    pub baseline: RawEmotions,
    pub calib_t: f64,
    pub calib_sum: [f64; 4],
    pub calibrated: bool,
    pub since_emit: f64,
    pub face: bool,
}

impl Default for EmotionTracker {
    fn default() -> Self {
        Self {
            rate: 5.0,
            enter: 0.45,
            exit: 0.3,
            switch_margin: 0.12,
            strong: 0.65,
            strong_every: 6.0,
            calibrate_seconds: 1.5,
            max_baseline: 0.2,
            scores: [0.0; 4],
            dominant: Emotion::Neutral,
            baseline: RawEmotions {
                joy: 0.0,
                anger: 0.0,
                surprise: 0.0,
                sadness: 0.0,
            },
            calib_t: 0.0,
            calib_sum: [0.0; 4],
            calibrated: false,
            since_emit: 0.0,
            face: false,
        }
    }
}

impl EmotionTracker {
    pub fn new(calibrate_seconds: f64) -> Self {
        let mut t = Self::default();
        t.calibrate_seconds = calibrate_seconds;
        t.calibrated = calibrate_seconds <= 0.0;
        t
    }

    pub fn reset(&mut self) {
        self.scores = [0.0; 4];
        self.dominant = Emotion::Neutral;
        self.baseline = RawEmotions {
            joy: 0.0,
            anger: 0.0,
            surprise: 0.0,
            sadness: 0.0,
        };
        self.calib_t = 0.0;
        self.calib_sum = [0.0; 4];
        self.calibrated = self.calibrate_seconds <= 0.0;
        self.since_emit = 0.0;
        self.face = false;
    }

    fn calibrate(&mut self, b: &HashMap<String, f64>, dt: f64) {
        let raw = raw_emotions(b);
        self.calib_sum[0] += raw.joy * dt;
        self.calib_sum[1] += raw.anger * dt;
        self.calib_sum[2] += raw.surprise * dt;
        self.calib_sum[3] += raw.sadness * dt;
        self.calib_t += dt;

        if self.calib_t >= self.calibrate_seconds {
            let inv_t = 1.0 / self.calib_t.max(1e-6);
            self.baseline = RawEmotions {
                joy: (self.calib_sum[0] * inv_t).min(self.max_baseline),
                anger: (self.calib_sum[1] * inv_t).min(self.max_baseline),
                surprise: (self.calib_sum[2] * inv_t).min(self.max_baseline),
                sadness: (self.calib_sum[3] * inv_t).min(self.max_baseline),
            };
            self.calibrated = true;
        }
    }

    pub fn update(
        &mut self,
        categories: Option<&HashMap<String, f64>>,
        dt: f64,
    ) -> (Mood, Option<EmotionEvent>) {
        self.face = categories.is_some();
        let mut target = [0.0; 4];

        if let Some(b) = categories {
            if !self.calibrated {
                self.calibrate(b, dt);
            } else {
                let em = emotions_from_blendshapes(b, Some(&self.baseline));
                target = [em.joy, em.anger, em.surprise, em.sadness];
            }
        }

        for i in 0..4 {
            self.scores[i] = smooth(self.scores[i], target[i], self.rate, dt);
        }

        // Find best non-neutral emotion
        let mut best_idx = 0;
        for i in 1..4 {
            if self.scores[i] > self.scores[best_idx] {
                best_idx = i;
            }
        }
        let best_emotion = match best_idx {
            0 => Emotion::Joy,
            1 => Emotion::Anger,
            2 => Emotion::Surprise,
            _ => Emotion::Sadness,
        };

        let cur = self.dominant;
        let mut next = cur;

        if cur != Emotion::Neutral {
            let cur_idx = match cur {
                Emotion::Joy => 0,
                Emotion::Anger => 1,
                Emotion::Surprise => 2,
                _ => 3,
            };
            if self.scores[cur_idx] >= self.exit {
                if best_emotion != cur && self.scores[best_idx] - self.scores[cur_idx] > self.switch_margin {
                    next = best_emotion;
                }
            } else if self.scores[best_idx] >= self.enter {
                next = best_emotion;
            } else {
                next = Emotion::Neutral;
            }
        } else if self.scores[best_idx] >= self.enter {
            next = best_emotion;
        }

        let mut event = None;
        if next != cur {
            self.dominant = next;
            self.since_emit = 0.0;
            event = Some(EmotionEvent::Change);
        } else if next != Emotion::Neutral {
            self.since_emit += dt;
            let next_idx = match next {
                Emotion::Joy => 0,
                Emotion::Anger => 1,
                Emotion::Surprise => 2,
                _ => 3,
            };
            if self.scores[next_idx] >= self.strong && self.since_emit >= self.strong_every {
                self.since_emit = 0.0;
                event = Some(EmotionEvent::Strong);
            }
        }

        let max_val = self.scores[0].max(self.scores[1]).max(self.scores[2]).max(self.scores[3]);
        let neutral = clamp(1.0 - max_val);
        let intensity = match self.dominant {
            Emotion::Neutral => neutral,
            Emotion::Joy => self.scores[0],
            Emotion::Anger => self.scores[1],
            Emotion::Surprise => self.scores[2],
            Emotion::Sadness => self.scores[3],
        };

        let mood = Mood {
            joy: self.scores[0],
            anger: self.scores[1],
            surprise: self.scores[2],
            sadness: self.scores[3],
            neutral,
            dominant: self.dominant,
            intensity,
            face: self.face,
        };

        (mood, event)
    }
}
