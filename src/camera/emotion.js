// camera (#33, #116, #117): face blendshapes → smoothed emotion scores → world mood.
//
// Per #116 / #117 (Option A):
//   - Active camera emotions are JOY and SURPRISE only (anger and sadness are removed from the
//     active camera model; war/raids come from simulation scarcity, territory, stress and the
//     fist gesture).
//   - Joy is a continuous positive mood (growth, fertility, peace, festivals, courage).
//   - Surprise is a short debounced event trigger (300 ms debounce, 4 s cooldown) that triggers
//     sudden events (lightning, storms, breakthroughs).

import { clamp, smooth } from '../sim/util.js';

export const EMOTIONS = ['joy', 'surprise'];

export const RECIPES = {
  joy: [['mouthSmileLeft', 0.5], ['mouthSmileRight', 0.5]],
  surprise: [['jawOpen', 0.4], ['browInnerUp', 0.3], ['eyeWideLeft', 0.15], ['eyeWideRight', 0.15]],
};

// Raw recipe value at which an emotion starts to count, and how much more makes it 1.
export const RANGE = { joy: [0.1, 0.45], surprise: [0.1, 0.35] };

const ZERO = { joy: 0, surprise: 0 };

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

// Weighted recipe sums for joy and surprise.
export function rawEmotions(categories) {
  const b = blendshapeMap(categories);
  const v = (k) => clamp(Number(b[k]) || 0);
  const sum = (recipe) => recipe.reduce((s, [k, w]) => s + w * v(k), 0);
  return {
    joy: sum(RECIPES.joy),
    surprise: sum(RECIPES.surprise),
    anger: 0,
    sadness: 0,
  };
}

// 0..1 emotion scores for one frame, after subtracting the resting-face baseline.
export function emotionsFromBlendshapes(categories, baseline = ZERO) {
  const raw = rawEmotions(categories);
  const out = { joy: 0, surprise: 0, anger: 0, sadness: 0 };
  for (const k of EMOTIONS) {
    const [floor, span] = RANGE[k];
    out[k] = clamp((raw[k] - (baseline[k] || 0) - floor) / span);
  }
  return out;
}

export function neutralMood() {
  return { joy: 0, surprise: 0, anger: 0, sadness: 0, neutral: 1, dominant: 'neutral', intensity: 1, face: false, surpriseTriggered: false };
}

// Smooths per-frame scores, picks a dominant mood with hysteresis and reports events:
// 'change' when the dominant mood changes (including back to neutral), 'strong' every
// `strongEvery` seconds while a non-neutral mood stays above `strong`, and `surpriseTriggered`
// when surprise holds for `surpriseHoldSeconds` (0.3s) and is off `surpriseCooldownSeconds` (4s).
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
    surpriseHoldSeconds = 0.3,
    surpriseCooldownSeconds = 4.0,
  } = {}) {
    Object.assign(this, {
      rate, enter, exit, switchMargin, strong, strongEvery,
      calibrateSeconds, maxBaseline, surpriseHoldSeconds, surpriseCooldownSeconds,
    });
    this.reset();
  }

  reset() {
    this.scores = { ...ZERO };
    this.dominant = 'neutral';
    this.baseline = { ...ZERO };
    this.calib = { t: 0, n: 0, sum: { ...ZERO } };
    this.calibrated = this.calibrateSeconds <= 0;
    this.sinceEmit = 0;
    this.face = false;
    this.surpriseHeld = 0;
    this.surpriseCooldown = 0;
    this.surpriseLatched = false;
  }

  // categories: blendshape list for the current face, or null when no face is in view.
  // Returns { mood, event: null | 'change' | 'strong' | 'surprise' }.
  update(categories, dt) {
    this.face = !!categories;
    let target = ZERO;
    if (categories && !this.calibrated) this.calibrate(categories, dt);
    else if (categories) target = emotionsFromBlendshapes(categories, this.baseline);
    for (const k of EMOTIONS) this.scores[k] = smooth(this.scores[k], target[k], this.rate, dt);

    const s = this.scores;
    this.surpriseCooldown = Math.max(0, this.surpriseCooldown - dt);
    let surpriseTriggered = false;
    if (s.surprise >= this.enter) {
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

    const best = EMOTIONS.reduce((a, k) => (s[k] > s[a] ? k : a), EMOTIONS[0]);
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
    return { mood: this.mood(surpriseTriggered), event, surpriseTriggered: !!surpriseTriggered };
  }

  calibrate(categories, dt) {
    const raw = rawEmotions(categories), c = this.calib;
    c.t += dt; c.n++;
    for (const k of EMOTIONS) c.sum[k] += raw[k];
    if (c.t >= this.calibrateSeconds) {
      for (const k of EMOTIONS) this.baseline[k] = Math.min(this.maxBaseline, c.sum[k] / c.n);
      this.calibrated = true;
    }
  }

  mood(surpriseTriggered = false) {
    const s = this.scores;
    const top = Math.max(...EMOTIONS.map((k) => s[k]));
    const neutral = clamp(1 - top);
    const r = (x) => Math.round(x * 1000) / 1000;
    return {
      joy: r(s.joy),
      surprise: r(s.surprise),
      anger: 0,
      sadness: 0,
      neutral: r(neutral),
      dominant: this.dominant,
      intensity: r(this.dominant === 'neutral' ? neutral : s[this.dominant]),
      face: this.face,
      calibrating: this.face && !this.calibrated,
      surpriseTriggered,
    };
  }
}
