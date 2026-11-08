// camera (#33, #116, #117): face blendshapes → smoothed emotion scores → world mood.
//
// Per #116 / #117 (Option A):
//   - The camera panel shows JOY and SURPRISE (EMOTIONS).
//   - Joy is a continuous positive mood (growth, fertility, peace, festivals, courage).
//   - Surprise is a short debounced event trigger (300 ms hold, 4 s cooldown) that triggers
//     sudden events (lightning, storms, breakthroughs).
//   - Anger and sadness are still scored (SCORED): face anger feeds brain stress and civ raids
//     (src/brain/brain.js, src/civ/sandbox.js), and the eras react to every dominant mood.
//
// In the browser the tracker runs in Rust (crates/engine/src/camera/emotion.rs, RustEmotionTracker).
// Keep the two identical: same recipes, calibration and tracker logic, with the floating-point
// operations in the same order. tests/emotion.test.mjs runs both on the same faces.

import { clamp, smooth } from '../sim/util.js';

// Shown in the camera and debug panels (#117).
export const EMOTIONS = ['joy', 'surprise'];

// Scored every frame, in Rust's index order (ties for the dominant mood go to the first).
export const SCORED = ['joy', 'anger', 'surprise', 'sadness'];

// Weighted MediaPipe Face Landmarker blendshapes, summed in this order.
export const RECIPES = {
  joy: [['mouthSmileLeft', 0.5], ['mouthSmileRight', 0.5]],
  anger: [['browDownLeft', 0.3], ['browDownRight', 0.3], ['mouthPressLeft', 0.2], ['mouthPressRight', 0.2]],
  // Mostly the brows (0.7). The jaw also opens to talk or yawn, so it only adds a little.
  surprise: [
    ['browInnerUp', 0.3], ['browOuterUpLeft', 0.2], ['browOuterUpRight', 0.2],
    ['eyeWideLeft', 0.1], ['eyeWideRight', 0.1], ['jawOpen', 0.1],
  ],
  sadness: [['mouthFrownLeft', 0.35], ['mouthFrownRight', 0.35], ['browInnerUp', 0.3]],
};

// Per-emotion calibration, in recipe units above the resting-face baseline:
//   rest  deadzone, read as 0 (a neutral face, talking, tracker jitter);
//   full  what a clear expression reaches, read as 1.
// score = clamp((raw - baseline - rest) / (full - rest)).
// MediaPipe's blendshapes peak at very different sizes: a big smile drives mouthSmile to 0.8-0.9,
// but a fully surprised face gives browInnerUp/browOuterUp about 0.4-0.6, eyeWide 0.15-0.3 and
// jawOpen about 0.3, a surprise recipe of only 0.25-0.35. On joy's scale surprise topped out near
// 20%, so surprise reads full at 0.22 (gain 1/0.16 = 6.25, joy's is 1/0.45 = 2.2).
export const CALIBRATION = {
  joy: { rest: 0.1, full: 0.55 },
  anger: { rest: 0.08, full: 0.4 },
  surprise: { rest: 0.06, full: 0.22 },
  sadness: { rest: 0.06, full: 0.35 },
};

// The blendshapes the Rust tracker reads, in RustEmotionTracker.update_direct's argument order.
export const INPUTS = [
  'mouthSmileLeft', 'mouthSmileRight', 'browDownLeft', 'browDownRight', 'mouthPressLeft', 'mouthPressRight',
  'jawOpen', 'browInnerUp', 'browOuterUpLeft', 'browOuterUpRight', 'eyeWideLeft', 'eyeWideRight',
  'mouthFrownLeft', 'mouthFrownRight',
];

const ZERO = { joy: 0, anger: 0, surprise: 0, sadness: 0 };

// Accepts Face Landmarker's `categories` array ({categoryName, score}) or a plain {name: score} map.
export function blendshapeMap(categories) {
  if (!categories) return {};
  if (Array.isArray(categories)) {
    const m = {};
    for (const c of categories) m[c.categoryName] = c.score;
    return m;
  }
  return categories;
}

// Weighted recipe sums. A smile damps anger; an open jaw or a smile damps sadness.
export function rawEmotions(categories) {
  const b = blendshapeMap(categories);
  const v = (k) => clamp(Number(b[k]) || 0);
  const sum = (recipe) => recipe.reduce((s, [k, w]) => s + w * v(k), 0);
  const smile = sum(RECIPES.joy);
  return {
    joy: smile,
    anger: sum(RECIPES.anger) * (1 - 0.7 * smile),
    surprise: sum(RECIPES.surprise),
    sadness: sum(RECIPES.sadness) * (1 - v('jawOpen')) * (1 - smile),
  };
}

// 0..1 emotion scores for one frame, after subtracting the resting-face baseline.
export function emotionsFromBlendshapes(categories, baseline = ZERO) {
  const raw = rawEmotions(categories);
  const out = { ...ZERO };
  for (const k of SCORED) {
    const { rest, full } = CALIBRATION[k];
    out[k] = clamp((raw[k] - (baseline[k] || 0) - rest) / (full - rest));
  }
  return out;
}

export function neutralMood() {
  return { joy: 0, surprise: 0, anger: 0, sadness: 0, neutral: 1, dominant: 'neutral', intensity: 1, face: false, surpriseTriggered: false };
}

let RustEmotionTrackerClass = null;

export function setRustEmotionTracker(cls) {
  RustEmotionTrackerClass = cls;
}

const r3 = (x) => Math.round(x * 1000) / 1000;

// Smooths per-frame scores, picks a dominant mood with hysteresis and reports events:
// 'change' when the dominant mood changes (including back to neutral), 'surprise' (with
// `surpriseTriggered`) when surprise holds at `surpriseFire` or more for `surpriseHoldSeconds`
// (0.3 s) and is off its `surpriseCooldownSeconds` (4 s) cooldown, re-arming once surprise drops
// below `exit`; and 'strong' every `strongEvery` seconds while a non-neutral mood stays above `strong`.
export class EmotionTracker {
  constructor({
    rate = 5,
    enter = 0.45,
    exit = 0.3,
    switchMargin = 0.12,
    strong = 0.65,
    strongEvery = 6,
    calibrateSeconds = 1.5,
    maxBaseline = 0.2,
    surpriseFire = 0.6,
    surpriseHoldSeconds = 0.3,
    surpriseCooldownSeconds = 4.0,
  } = {}) {
    // The Rust tracker uses the defaults above; only calibrateSeconds is passed through.
    if (RustEmotionTrackerClass) {
      try {
        this.wasm = new RustEmotionTrackerClass(calibrateSeconds);
      } catch (e) {
        this.wasm = null;
      }
    }
    Object.assign(this, {
      rate, enter, exit, switchMargin, strong, strongEvery,
      calibrateSeconds, maxBaseline, surpriseFire, surpriseHoldSeconds, surpriseCooldownSeconds,
    });
    this.reset();
  }

  reset() {
    if (this.wasm) this.wasm.reset();
    this.scores = { ...ZERO };
    this.dominant = 'neutral';
    this.baseline = { ...ZERO };
    this.calib = { t: 0, sum: { ...ZERO } };
    this.calibrated = this.calibrateSeconds <= 0;
    this.sinceEmit = 0;
    this.face = false;
    this.surpriseHeld = 0;
    this.surpriseCooldown = 0;
    this.surpriseLatched = false;
  }

  // categories: blendshape list for the current face, or null when no face is in view.
  // Returns { mood, event: null | 'change' | 'strong' | 'surprise', surpriseTriggered }.
  update(categories, dt) {
    if (this.wasm) {
      const b = blendshapeMap(categories);
      const res = this.wasm.update_direct(!!categories, ...INPUTS.map((k) => Number(b[k]) || 0), dt);
      const surpriseTriggered = !!res[9];
      return {
        mood: {
          joy: r3(res[1]),
          surprise: r3(res[3]),
          anger: r3(res[2]),
          sadness: r3(res[4]),
          neutral: r3(res[5]),
          dominant: res[0],
          intensity: r3(res[6]),
          face: !!res[7],
          calibrating: !!res[10],
          surpriseTriggered,
        },
        event: res[8] || null,
        surpriseTriggered,
      };
    }

    this.face = !!categories;
    let target = ZERO;
    if (categories && !this.calibrated) this.calibrate(categories, dt);
    else if (categories) target = emotionsFromBlendshapes(categories, this.baseline);
    for (const k of SCORED) this.scores[k] = smooth(this.scores[k], target[k], this.rate, dt);

    const s = this.scores;
    this.surpriseCooldown = Math.max(0, this.surpriseCooldown - dt);
    let surpriseTriggered = false;
    if (s.surprise >= this.surpriseFire) {
      this.surpriseHeld += dt;
      if (!this.surpriseLatched && this.surpriseHeld >= this.surpriseHoldSeconds && this.surpriseCooldown <= 0) {
        surpriseTriggered = true;
        this.surpriseLatched = true;
        this.surpriseCooldown = this.surpriseCooldownSeconds;
      }
    } else if (s.surprise < this.exit) {
      this.surpriseHeld = 0;
      this.surpriseLatched = false;
    }

    const best = SCORED.reduce((a, k) => (s[k] > s[a] ? k : a), SCORED[0]);
    const cur = this.dominant;
    let next = cur;
    if (cur !== 'neutral' && s[cur] >= this.exit) {
      if (best !== cur && s[best] >= this.enter && s[best] > s[cur] + this.switchMargin) next = best;
    } else next = s[best] >= this.enter ? best : 'neutral';

    let event = null;
    this.sinceEmit += dt;
    if (next !== cur) { this.dominant = next; event = 'change'; this.sinceEmit = 0; }
    else if (surpriseTriggered) { event = 'surprise'; this.sinceEmit = 0; }
    else if (next !== 'neutral' && s[next] >= this.strong && this.sinceEmit >= this.strongEvery) { event = 'strong'; this.sinceEmit = 0; }
    return { mood: this.mood(surpriseTriggered), event, surpriseTriggered };
  }

  // Time-weighted mean of the raw recipes over the first `calibrateSeconds` with a face.
  calibrate(categories, dt) {
    const raw = rawEmotions(categories), c = this.calib;
    for (const k of SCORED) c.sum[k] += raw[k] * dt;
    c.t += dt;
    if (c.t >= this.calibrateSeconds) {
      const invT = 1 / Math.max(c.t, 1e-6);
      for (const k of SCORED) this.baseline[k] = Math.min(c.sum[k] * invT, this.maxBaseline);
      this.calibrated = true;
    }
  }

  mood(surpriseTriggered = false) {
    const s = this.scores;
    const neutral = clamp(1 - Math.max(...SCORED.map((k) => s[k])));
    return {
      joy: r3(s.joy),
      surprise: r3(s.surprise),
      anger: r3(s.anger),
      sadness: r3(s.sadness),
      neutral: r3(neutral),
      dominant: this.dominant,
      intensity: r3(this.dominant === 'neutral' ? neutral : s[this.dominant]),
      face: this.face,
      calibrating: this.face && !this.calibrated,
      surpriseTriggered,
    };
  }
}
