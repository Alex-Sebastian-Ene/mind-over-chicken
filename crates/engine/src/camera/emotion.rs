//! Face emotion tracker and blendshape mapping (#33, #108, #117).
//! Pure logic port of src/camera/emotion.js. Keep the two identical: same recipes, calibration and
//! tracker logic, with the floating-point operations in the same order (tests/emotion.test.mjs runs
//! this tracker through wasm next to the JS one on the same faces).

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

    /// Index into `EmotionTracker::scores` (JS SCORED order); `None` for neutral.
    pub fn index(&self) -> Option<usize> {
        match self {
            Emotion::Joy => Some(0),
            Emotion::Anger => Some(1),
            Emotion::Surprise => Some(2),
            Emotion::Sadness => Some(3),
            Emotion::Neutral => None,
        }
    }
}

/// Scored every frame, in the order ties for the dominant mood are broken (JS SCORED).
pub const EMOTIONS: [Emotion; 4] = [
    Emotion::Joy,
    Emotion::Anger,
    Emotion::Surprise,
    Emotion::Sadness,
];

/// Weighted MediaPipe Face Landmarker blendshapes, summed in this order (JS RECIPES).
pub const RECIPE_JOY: [(&str, f64); 2] = [("mouthSmileLeft", 0.5), ("mouthSmileRight", 0.5)];
pub const RECIPE_ANGER: [(&str, f64); 4] = [
    ("browDownLeft", 0.3),
    ("browDownRight", 0.3),
    ("mouthPressLeft", 0.2),
    ("mouthPressRight", 0.2),
];
/// Mostly the brows (0.7). The jaw also opens to talk or yawn, so it only adds a little.
pub const RECIPE_SURPRISE: [(&str, f64); 6] = [
    ("browInnerUp", 0.3),
    ("browOuterUpLeft", 0.2),
    ("browOuterUpRight", 0.2),
    ("eyeWideLeft", 0.1),
    ("eyeWideRight", 0.1),
    ("jawOpen", 0.1),
];
pub const RECIPE_SADNESS: [(&str, f64); 3] = [
    ("mouthFrownLeft", 0.35),
    ("mouthFrownRight", 0.35),
    ("browInnerUp", 0.3),
];

/// Per-emotion calibration (JS CALIBRATION), in recipe units above the resting-face baseline:
/// `rest` is a deadzone read as 0, `full` is what a clear expression reaches, read as 1.
/// score = clamp((raw - baseline - rest) / (full - rest)).
/// Surprise's blendshapes peak far lower than a smile's, so it reads full at 0.22 (gain 6.25).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    pub rest: f64,
    pub full: f64,
}

pub const CAL_JOY: Calibration = Calibration { rest: 0.1, full: 0.55 };
pub const CAL_ANGER: Calibration = Calibration { rest: 0.08, full: 0.4 };
pub const CAL_SURPRISE: Calibration = Calibration { rest: 0.06, full: 0.22 };
pub const CAL_SADNESS: Calibration = Calibration { rest: 0.06, full: 0.35 };

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

impl RawEmotions {
    pub const ZERO: RawEmotions = RawEmotions {
        joy: 0.0,
        anger: 0.0,
        surprise: 0.0,
        sadness: 0.0,
    };

    pub fn to_array(&self) -> [f64; 4] {
        [self.joy, self.anger, self.surprise, self.sadness]
    }
}

#[inline]
fn shape(b: &HashMap<String, f64>, k: &str) -> f64 {
    clamp(*b.get(k).unwrap_or(&0.0))
}

fn recipe_sum(b: &HashMap<String, f64>, recipe: &[(&str, f64)]) -> f64 {
    recipe.iter().fold(0.0, |s, &(k, w)| s + w * shape(b, k))
}

/// Weighted recipe sums. A smile damps anger; an open jaw or a smile damps sadness.
pub fn raw_emotions(b: &HashMap<String, f64>) -> RawEmotions {
    let smile = recipe_sum(b, &RECIPE_JOY);
    RawEmotions {
        joy: smile,
        anger: recipe_sum(b, &RECIPE_ANGER) * (1.0 - 0.7 * smile),
        surprise: recipe_sum(b, &RECIPE_SURPRISE),
        sadness: recipe_sum(b, &RECIPE_SADNESS) * (1.0 - shape(b, "jawOpen")) * (1.0 - smile),
    }
}

#[inline]
fn score(raw: f64, base: f64, cal: Calibration) -> f64 {
    clamp((raw - base - cal.rest) / (cal.full - cal.rest))
}

/// 0..1 emotion scores for one frame, after subtracting the resting-face baseline.
pub fn emotions_from_blendshapes(
    categories: &HashMap<String, f64>,
    baseline: Option<&RawEmotions>,
) -> RawEmotions {
    let raw = raw_emotions(categories);
    let base = baseline.copied().unwrap_or(RawEmotions::ZERO);
    RawEmotions {
        joy: score(raw.joy, base.joy, CAL_JOY),
        anger: score(raw.anger, base.anger, CAL_ANGER),
        surprise: score(raw.surprise, base.surprise, CAL_SURPRISE),
        sadness: score(raw.sadness, base.sadness, CAL_SADNESS),
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
    /// A face is in view but the resting baseline is still being measured.
    pub calibrating: bool,
    /// The one-shot surprise event fired this frame.
    pub surprise_triggered: bool,
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
            calibrating: false,
            surprise_triggered: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmotionEvent {
    Change,
    Surprise,
    Strong,
}

impl EmotionEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            EmotionEvent::Change => "change",
            EmotionEvent::Surprise => "surprise",
            EmotionEvent::Strong => "strong",
        }
    }
}

/// Smooths per-frame scores, picks a dominant mood with hysteresis and reports events:
/// `Change` when the dominant mood changes (including back to neutral), `Surprise` when surprise
/// holds at `surprise_fire` or more for `surprise_hold_seconds` and is off its cooldown (re-arming
/// once surprise drops below `exit`), and `Strong` every `strong_every` seconds while a non-neutral
/// mood stays above `strong`. Same as the JS EmotionTracker.
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
    pub surprise_fire: f64,
    pub surprise_hold_seconds: f64,
    pub surprise_cooldown_seconds: f64,

    pub scores: [f64; 4], // joy, anger, surprise, sadness
    pub dominant: Emotion,
    pub baseline: RawEmotions,
    pub calib_t: f64,
    pub calib_sum: [f64; 4],
    pub calibrated: bool,
    pub since_emit: f64,
    pub face: bool,
    pub surprise_held: f64,
    pub surprise_cooldown: f64,
    pub surprise_latched: bool,
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
            surprise_fire: 0.6,
            surprise_hold_seconds: 0.3,
            surprise_cooldown_seconds: 4.0,
            scores: [0.0; 4],
            dominant: Emotion::Neutral,
            baseline: RawEmotions::ZERO,
            calib_t: 0.0,
            calib_sum: [0.0; 4],
            calibrated: false,
            since_emit: 0.0,
            face: false,
            surprise_held: 0.0,
            surprise_cooldown: 0.0,
            surprise_latched: false,
        }
    }
}

impl EmotionTracker {
    pub fn new(calibrate_seconds: f64) -> Self {
        let mut t = Self::default();
        t.calibrate_seconds = calibrate_seconds;
        t.reset();
        t
    }

    pub fn reset(&mut self) {
        self.scores = [0.0; 4];
        self.dominant = Emotion::Neutral;
        self.baseline = RawEmotions::ZERO;
        self.calib_t = 0.0;
        self.calib_sum = [0.0; 4];
        self.calibrated = self.calibrate_seconds <= 0.0;
        self.since_emit = 0.0;
        self.face = false;
        self.surprise_held = 0.0;
        self.surprise_cooldown = 0.0;
        self.surprise_latched = false;
    }

    /// Time-weighted mean of the raw recipes over the first `calibrate_seconds` with a face.
    fn calibrate(&mut self, b: &HashMap<String, f64>, dt: f64) {
        let raw = raw_emotions(b).to_array();
        for i in 0..4 {
            self.calib_sum[i] += raw[i] * dt;
        }
        self.calib_t += dt;

        if self.calib_t >= self.calibrate_seconds {
            let inv_t = 1.0 / self.calib_t.max(1e-6);
            let base: [f64; 4] =
                std::array::from_fn(|i| (self.calib_sum[i] * inv_t).min(self.max_baseline));
            self.baseline = RawEmotions {
                joy: base[0],
                anger: base[1],
                surprise: base[2],
                sadness: base[3],
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
                target = emotions_from_blendshapes(b, Some(&self.baseline)).to_array();
            }
        }

        for i in 0..4 {
            self.scores[i] = smooth(self.scores[i], target[i], self.rate, dt);
        }
        let s = self.scores;

        // Surprise one-shot: hold at `surprise_fire`, then cooldown; re-arm below `exit`.
        self.surprise_cooldown = (self.surprise_cooldown - dt).max(0.0);
        let mut surprise_triggered = false;
        if s[2] >= self.surprise_fire {
            self.surprise_held += dt;
            if !self.surprise_latched
                && self.surprise_held >= self.surprise_hold_seconds
                && self.surprise_cooldown <= 0.0
            {
                surprise_triggered = true;
                self.surprise_latched = true;
                self.surprise_cooldown = self.surprise_cooldown_seconds;
            }
        } else if s[2] < self.exit {
            self.surprise_held = 0.0;
            self.surprise_latched = false;
        }

        // Best scored emotion; ties go to the first (JS SCORED order).
        let mut best = 0;
        for i in 1..4 {
            if s[i] > s[best] {
                best = i;
            }
        }
        let best_emotion = EMOTIONS[best];

        let cur = self.dominant;
        let mut next = cur;
        match cur.index() {
            Some(c) if s[c] >= self.exit => {
                if best_emotion != cur && s[best] >= self.enter && s[best] > s[c] + self.switch_margin {
                    next = best_emotion;
                }
            }
            _ => {
                next = if s[best] >= self.enter { best_emotion } else { Emotion::Neutral };
            }
        }

        let mut event = None;
        self.since_emit += dt;
        if next != cur {
            self.dominant = next;
            event = Some(EmotionEvent::Change);
            self.since_emit = 0.0;
        } else if surprise_triggered {
            event = Some(EmotionEvent::Surprise);
            self.since_emit = 0.0;
        } else if let Some(n) = next.index() {
            if s[n] >= self.strong && self.since_emit >= self.strong_every {
                event = Some(EmotionEvent::Strong);
                self.since_emit = 0.0;
            }
        }

        let neutral = clamp(1.0 - s[0].max(s[1]).max(s[2]).max(s[3]));
        let intensity = match self.dominant.index() {
            Some(i) => s[i],
            None => neutral,
        };

        let mood = Mood {
            joy: s[0],
            anger: s[1],
            surprise: s[2],
            sadness: s[3],
            neutral,
            dominant: self.dominant,
            intensity,
            face: self.face,
            calibrating: self.face && !self.calibrated,
            surprise_triggered,
        };

        (mood, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic MediaPipe faces at typical magnitudes; tests/emotion.test.mjs uses the same faces
    // and expects the same numbers.
    fn face(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|&(k, v)| (k.to_string(), v)).collect()
    }

    fn neutral() -> HashMap<String, f64> {
        face(&[
            ("browInnerUp", 0.08),
            ("browOuterUpLeft", 0.02),
            ("browOuterUpRight", 0.02),
            ("eyeWideLeft", 0.03),
            ("eyeWideRight", 0.03),
            ("jawOpen", 0.02),
            ("mouthSmileLeft", 0.02),
            ("mouthSmileRight", 0.02),
            ("browDownLeft", 0.05),
            ("browDownRight", 0.05),
            ("mouthPressLeft", 0.05),
            ("mouthPressRight", 0.05),
            ("mouthFrownLeft", 0.02),
            ("mouthFrownRight", 0.02),
        ])
    }

    fn big_smile() -> HashMap<String, f64> {
        face(&[("mouthSmileLeft", 0.85), ("mouthSmileRight", 0.85), ("jawOpen", 0.15)])
    }

    fn brow_lower() -> HashMap<String, f64> {
        face(&[
            ("browDownLeft", 0.55),
            ("browDownRight", 0.55),
            ("mouthPressLeft", 0.25),
            ("mouthPressRight", 0.25),
        ])
    }

    fn surprised() -> HashMap<String, f64> {
        face(&[
            ("browInnerUp", 0.45),
            ("browOuterUpLeft", 0.35),
            ("browOuterUpRight", 0.35),
            ("eyeWideLeft", 0.2),
            ("eyeWideRight", 0.2),
            ("jawOpen", 0.35),
        ])
    }

    fn mid_surprise() -> HashMap<String, f64> {
        face(&[
            ("browInnerUp", 0.2),
            ("browOuterUpLeft", 0.1),
            ("browOuterUpRight", 0.1),
            ("eyeWideLeft", 0.1),
            ("eyeWideRight", 0.1),
            ("jawOpen", 0.1),
        ])
    }

    // Talking: the jaw moves, the inner brows twitch a little.
    fn talking(jaw: f64, brow: f64) -> HashMap<String, f64> {
        let mut f = neutral();
        f.insert("jawOpen".to_string(), jaw);
        f.insert("browInnerUp".to_string(), brow);
        f.insert("browOuterUpLeft".to_string(), 0.04);
        f.insert("browOuterUpRight".to_string(), 0.04);
        f
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn neutral_face_reads_zero() {
        let em = emotions_from_blendshapes(&neutral(), None);
        assert_eq!(em.to_array(), [0.0; 4]);
    }

    #[test]
    fn each_clear_expression_reaches_its_full_range() {
        let joy = emotions_from_blendshapes(&big_smile(), None);
        assert!(joy.joy >= 0.99, "{joy:?}");
        assert!(joy.anger == 0.0 && joy.surprise == 0.0 && joy.sadness == 0.0, "{joy:?}");

        let anger = emotions_from_blendshapes(&brow_lower(), None);
        assert!(anger.anger >= 0.95, "{anger:?}");
        assert!(anger.joy == 0.0 && anger.surprise == 0.0, "{anger:?}");

        let sur = emotions_from_blendshapes(&surprised(), None);
        assert!(sur.surprise >= 0.99, "{sur:?}");
        assert!(sur.joy == 0.0 && sur.anger == 0.0 && sur.sadness < 0.2, "{sur:?}");
    }

    #[test]
    fn surprise_gain_matches_js() {
        // recipe 0.13 → (0.13 - 0.06) / (0.22 - 0.06) = 0.4375
        let em = emotions_from_blendshapes(&mid_surprise(), None);
        assert!(close(em.surprise, 0.4375), "{em:?}");
        assert_eq!(em.sadness, 0.0);
        let raw = raw_emotions(&surprised());
        assert!(close(raw.surprise, 0.35), "{raw:?}");
        assert!(close(raw.sadness, 0.135 * 0.65), "{raw:?}");
    }

    #[test]
    fn talking_is_not_surprise() {
        let base = raw_emotions(&neutral());
        for jaw in [0.1, 0.25, 0.4, 0.55] {
            let em = emotions_from_blendshapes(&talking(jaw, 0.12), Some(&base));
            assert!(em.surprise < 0.1, "jaw {jaw}: {em:?}");
        }
    }

    fn run(t: &mut EmotionTracker, f: Option<&HashMap<String, f64>>, secs: f64) -> Vec<(Mood, Option<EmotionEvent>)> {
        let dt = 1.0 / 30.0;
        let n = (secs / dt).round() as usize;
        (0..n).map(|_| t.update(f, dt)).collect()
    }

    #[test]
    fn surprised_face_dominates_and_fires_once() {
        let mut t = EmotionTracker::new(1.5);
        let calib = run(&mut t, Some(&neutral()), 1.6);
        assert!(calib[0].0.calibrating && !calib.last().unwrap().0.calibrating);
        assert!(close(t.baseline.surprise, raw_emotions(&neutral()).surprise));

        let frames = run(&mut t, Some(&surprised()), 1.0);
        let last = frames.last().unwrap().0;
        assert_eq!(last.dominant, Emotion::Surprise);
        assert!(last.surprise > 0.95, "{last:?}");
        let fired: Vec<_> = frames.iter().filter(|(m, _)| m.surprise_triggered).collect();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].1, Some(EmotionEvent::Surprise));

        // Back to rest and surprised again inside the 4 s cooldown: no second shot.
        run(&mut t, Some(&neutral()), 1.0);
        let again = run(&mut t, Some(&surprised()), 1.0);
        assert!(again.iter().all(|(m, _)| !m.surprise_triggered));
        // After the cooldown it fires again.
        run(&mut t, Some(&neutral()), 3.0);
        let later = run(&mut t, Some(&surprised()), 1.0);
        assert_eq!(later.iter().filter(|(m, _)| m.surprise_triggered).count(), 1);
    }

    #[test]
    fn talking_never_fires_surprise() {
        let mut t = EmotionTracker::new(1.5);
        run(&mut t, Some(&neutral()), 1.6);
        let dt = 1.0 / 30.0;
        for i in 0..300 {
            let x = i as f64;
            let jaw = 0.05 + 0.45 * (0.5 + 0.5 * (x * 0.9).sin());
            let brow = 0.08 + 0.12 * (x * 0.23).sin().max(0.0);
            let (m, ev) = t.update(Some(&talking(jaw, brow)), dt);
            assert!(m.surprise < 0.3, "frame {i}: {m:?}");
            assert!(ev.is_none() && m.dominant == Emotion::Neutral, "frame {i}: {m:?}");
        }
    }

    #[test]
    fn a_short_flash_does_not_fire() {
        let mut t = EmotionTracker::new(0.0);
        let mut frames = run(&mut t, Some(&surprised()), 0.3);
        frames.extend(run(&mut t, Some(&neutral()), 1.0));
        assert!(frames.iter().all(|(m, _)| !m.surprise_triggered));
    }
}
