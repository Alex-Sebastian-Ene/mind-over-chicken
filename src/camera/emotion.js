// camera (#33): face blendshapes → smoothed emotion scores → world mood. Pure logic, so Node
// can unit-test it with mocked blendshapes.
//
// Face Landmarker returns 52 ARKit-style blendshapes, each 0..1. Each emotion is a weighted
// sum of a few of them, minus this player's resting face (measured over the first
// `calibrateSeconds` with a face in view), mapped through a per-emotion [floor, floor + span]
// range to 0..1, then smoothed. Neutral is whatever no emotion claims.
// The ranges are first guesses: they need tuning on real webcams and faces.

import { clamp, smooth } from '../sim/util.js';

export const EMOTIONS = ['joy', 'anger', 'surprise', 'sadness'];

export const RECIPES = {
  joy: [['mouthSmileLeft', 0.5], ['mouthSmileRight', 0.5]],
  anger: [['browDownLeft', 0.3], ['browDownRight', 0.3], ['mouthPressLeft', 0.2], ['mouthPressRight', 0.2]],
  surprise: [['jawOpen', 0.4], ['browInnerUp', 0.3], ['eyeWideLeft', 0.15], ['eyeWideRight', 0.15]],
  sadness: [['mouthFrownLeft', 0.35], ['mouthFrownRight', 0.35], ['browInnerUp', 0.3]],
};

// Raw recipe value at which an emotion starts to count, and how much more makes it 1.
export const RANGE = { joy: [0.1, 0.45], anger: [0.08, 0.32], surprise: [0.1, 0.35], sadness: [0.06, 0.29] };

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

// Weighted recipe sums, with cross-suppression: an open jaw is surprise rather than sadness
// (both raise the inner brows), and a smile cancels most of a frown or a pressed mouth.
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
  const out = {};
  for (const k of EMOTIONS) {
    const [floor, span] = RANGE[k];
    out[k] = clamp((raw[k] - (baseline[k] || 0) - floor) / span);
  }
  return out;
}

export function neutralMood() {
  return { joy: 0, anger: 0, surprise: 0, sadness: 0, neutral: 1, dominant: 'neutral', intensity: 1, face: false };
}

// Smooths per-frame scores, picks a dominant mood with hysteresis and reports events:
// 'change' when the dominant mood changes (including back to neutral), 'strong' every
// `strongEvery` seconds while a non-neutral mood stays above `strong`.
export class EmotionTracker {
  constructor({ rate = 5, enter = 0.45, exit = 0.3, switchMargin = 0.12, strong = 0.65, strongEvery = 6, calibrateSeconds = 1.5, maxBaseline = 0.2 } = {}) {
    Object.assign(this, { rate, enter, exit, switchMargin, strong, strongEvery, calibrateSeconds, maxBaseline });
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
  }

  // categories: blendshape list for the current face, or null when no face is in view.
  // Returns { mood, event: null | 'change' | 'strong' }.
  update(categories, dt) {
    this.face = !!categories;
    let target = ZERO;
    // While calibrating, the resting face is being measured: report neutral until it's known.
    if (categories && !this.calibrated) this.calibrate(categories, dt);
    else if (categories) target = emotionsFromBlendshapes(categories, this.baseline);
    for (const k of EMOTIONS) this.scores[k] = smooth(this.scores[k], target[k], this.rate, dt);

    const s = this.scores;
    const best = EMOTIONS.reduce((a, k) => (s[k] > s[a] ? k : a), EMOTIONS[0]);
    const cur = this.dominant;
    let next = cur;
    if (cur !== 'neutral' && s[cur] >= this.exit) {
      if (best !== cur && s[best] >= this.enter && s[best] > s[cur] + this.switchMargin) next = best;
    } else next = s[best] >= this.enter ? best : 'neutral';

    let event = null;
    this.sinceEmit += dt;
    if (next !== cur) { this.dominant = next; event = 'change'; this.sinceEmit = 0; }
    else if (next !== 'neutral' && s[next] >= this.strong && this.sinceEmit >= this.strongEvery) { event = 'strong'; this.sinceEmit = 0; }
    return { mood: this.mood(), event };
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

  mood() {
    const s = this.scores;
    const top = Math.max(...EMOTIONS.map((k) => s[k]));
    const neutral = clamp(1 - top);
    const r = (x) => Math.round(x * 1000) / 1000;
    return {
      joy: r(s.joy), anger: r(s.anger), surprise: r(s.surprise), sadness: r(s.sadness), neutral: r(neutral),
      dominant: this.dominant,
      intensity: r(this.dominant === 'neutral' ? neutral : s[this.dominant]),
      face: this.face,
      calibrating: this.face && !this.calibrated,
    };
  }
}
