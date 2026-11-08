// Face emotion balance: every emotion reaches its full range on a clear expression, a resting face
// and talking read near 0, and the JS tracker matches the Rust one the browser runs (same numbers).
// The faces are the ones in crates/engine/src/camera/emotion.rs's tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  EmotionTracker, emotionsFromBlendshapes, rawEmotions, setRustEmotionTracker, INPUTS, RECIPES,
} from '../src/camera/emotion.js';
import * as engine from '../pkg/mind_over_chicken_engine.js';

engine.initSync({ module: readFileSync(new URL('../pkg/mind_over_chicken_engine_bg.wasm', import.meta.url)) });

const NEUTRAL = {
  browInnerUp: 0.08, browOuterUpLeft: 0.02, browOuterUpRight: 0.02, eyeWideLeft: 0.03, eyeWideRight: 0.03,
  jawOpen: 0.02, mouthSmileLeft: 0.02, mouthSmileRight: 0.02, browDownLeft: 0.05, browDownRight: 0.05,
  mouthPressLeft: 0.05, mouthPressRight: 0.05, mouthFrownLeft: 0.02, mouthFrownRight: 0.02,
};
const BIG_SMILE = { mouthSmileLeft: 0.85, mouthSmileRight: 0.85, jawOpen: 0.15 };
const BROW_LOWER = { browDownLeft: 0.55, browDownRight: 0.55, mouthPressLeft: 0.25, mouthPressRight: 0.25 };
const FROWN_ONLY = { browDownLeft: 0.5, browDownRight: 0.5 };
const SURPRISED = {
  browInnerUp: 0.45, browOuterUpLeft: 0.35, browOuterUpRight: 0.35, eyeWideLeft: 0.2, eyeWideRight: 0.2, jawOpen: 0.35,
};
const SURPRISED_MOUTH_SHUT = {
  browInnerUp: 0.45, browOuterUpLeft: 0.35, browOuterUpRight: 0.35, eyeWideLeft: 0.2, eyeWideRight: 0.2, jawOpen: 0.02,
};
const MID_SURPRISE = {
  browInnerUp: 0.2, browOuterUpLeft: 0.1, browOuterUpRight: 0.1, eyeWideLeft: 0.1, eyeWideRight: 0.1, jawOpen: 0.1,
};
const YAWN = { ...NEUTRAL, jawOpen: 0.9, browInnerUp: 0.15, browOuterUpLeft: 0.05, browOuterUpRight: 0.05 };
const talking = (jaw, brow) => ({ ...NEUTRAL, jawOpen: jaw, browInnerUp: brow, browOuterUpLeft: 0.04, browOuterUpRight: 0.04 });
// MediaPipe hands the tracker a categories array.
const asCategories = (m) => Object.entries(m).map(([categoryName, score]) => ({ categoryName, score }));

const DT = 1 / 30;
const run = (t, face, secs) => Array.from({ length: Math.round(secs / DT) }, () => t.update(face, DT));
const close = (a, b, eps = 1e-9) => Math.abs(a - b) < eps;

test('a resting face reads 0 on every emotion', () => {
  assert.deepEqual(emotionsFromBlendshapes(NEUTRAL), { joy: 0, anger: 0, surprise: 0, sadness: 0 });
  assert.deepEqual(emotionsFromBlendshapes(asCategories(NEUTRAL)), { joy: 0, anger: 0, surprise: 0, sadness: 0 });
});

test('each clear expression reaches its full range', () => {
  const joy = emotionsFromBlendshapes(BIG_SMILE);
  assert.ok(joy.joy >= 0.99, JSON.stringify(joy));
  assert.ok(joy.anger === 0 && joy.surprise === 0 && joy.sadness === 0, JSON.stringify(joy));

  const anger = emotionsFromBlendshapes(BROW_LOWER);
  assert.ok(anger.anger >= 0.95 && anger.joy === 0 && anger.surprise === 0, JSON.stringify(anger));
  assert.ok(emotionsFromBlendshapes(FROWN_ONLY).anger >= 0.65, 'lowered brows alone read as clear anger');

  const sur = emotionsFromBlendshapes(SURPRISED);
  assert.ok(sur.surprise >= 0.99, JSON.stringify(sur));
  assert.ok(sur.joy === 0 && sur.anger === 0 && sur.sadness < 0.2, JSON.stringify(sur));
  assert.ok(emotionsFromBlendshapes(SURPRISED_MOUTH_SHUT).surprise >= 0.95, 'brows up and eyes wide is enough');
});

test('surprise gain: the same numbers as Rust', () => {
  // recipe 0.13 → (0.13 - 0.06) / (0.22 - 0.06) = 0.4375 (emotion.rs surprise_gain_matches_js)
  const em = emotionsFromBlendshapes(MID_SURPRISE);
  assert.ok(close(em.surprise, 0.4375), JSON.stringify(em));
  assert.equal(em.sadness, 0);
  const raw = rawEmotions(SURPRISED);
  assert.ok(close(raw.surprise, 0.35) && close(raw.sadness, 0.135 * 0.65), JSON.stringify(raw));
  // Surprise is mostly the brows: the jaw (talking, yawning) only adds a little.
  const w = Object.fromEntries(RECIPES.surprise);
  assert.ok(w.browInnerUp + w.browOuterUpLeft + w.browOuterUpRight >= 0.7 && w.jawOpen <= 0.1);
});

test('talking or yawning after calibration is not surprise', () => {
  const base = rawEmotions(NEUTRAL);
  for (const jaw of [0.1, 0.25, 0.4, 0.55]) {
    assert.ok(emotionsFromBlendshapes(talking(jaw, 0.12), base).surprise < 0.1, `jaw ${jaw}`);
  }
  assert.ok(emotionsFromBlendshapes(YAWN, base).surprise < 0.45, 'a yawn does not make surprise dominant');
});

test('a surprised face becomes dominant and fires one debounced surprise, then cools down', () => {
  const t = new EmotionTracker();
  const calib = run(t, NEUTRAL, 1.6);
  assert.ok(calib[0].mood.calibrating && !calib.at(-1).mood.calibrating);
  assert.ok(close(t.baseline.surprise, rawEmotions(NEUTRAL).surprise));

  const frames = run(t, asCategories(SURPRISED), 1.0);
  const last = frames.at(-1).mood;
  assert.equal(last.dominant, 'surprise');
  assert.ok(last.surprise > 0.95, JSON.stringify(last));
  const fired = frames.filter((f) => f.surpriseTriggered);
  assert.equal(fired.length, 1);
  assert.equal(fired[0].event, 'surprise');
  assert.ok(frames.indexOf(fired[0]) * DT >= 0.3, 'fires after the 0.3 s hold');

  run(t, NEUTRAL, 1.0);
  assert.ok(run(t, SURPRISED, 1.0).every((f) => !f.surpriseTriggered), 'no second shot inside the 4 s cooldown');
  run(t, NEUTRAL, 3.0);
  assert.equal(run(t, SURPRISED, 1.0).filter((f) => f.surpriseTriggered).length, 1);
});

test('a smile and a frown take the dominant mood too', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0 });
  assert.equal(run(t, BIG_SMILE, 1).at(-1).mood.dominant, 'joy');
  assert.equal(run(t, NEUTRAL, 1).at(-1).mood.dominant, 'neutral');
  const angry = run(t, BROW_LOWER, 1).at(-1).mood;
  assert.equal(angry.dominant, 'anger');
  assert.ok(angry.anger > 0.95);
});

test('talking never makes surprise dominant or fires it; a short flash does not fire', () => {
  const t = new EmotionTracker();
  run(t, NEUTRAL, 1.6);
  for (let i = 0; i < 300; i++) {
    const jaw = 0.05 + 0.45 * (0.5 + 0.5 * Math.sin(i * 0.9));
    const brow = 0.08 + 0.12 * Math.max(0, Math.sin(i * 0.23));
    const { mood, event } = t.update(talking(jaw, brow), DT);
    assert.ok(mood.surprise < 0.3 && event === null && mood.dominant === 'neutral', `frame ${i}: ${JSON.stringify(mood)}`);
  }
  const flash = new EmotionTracker({ calibrateSeconds: 0 });
  const frames = [...run(flash, SURPRISED, 0.3), ...run(flash, NEUTRAL, 1.0)];
  assert.ok(frames.every((f) => !f.surpriseTriggered));
});

// The browser runs RustEmotionTracker (wasm); the JS tracker is the fallback. Same inputs, same output.
test('the Rust tracker gives the same numbers and events as the JS one', () => {
  setRustEmotionTracker(engine.RustEmotionTracker);
  const rust = new EmotionTracker();
  setRustEmotionTracker(null);
  const js = new EmotionTracker();
  assert.ok(rust.wasm && !js.wasm);

  const script = [
    [null, 0.3], [NEUTRAL, 1.6], [SURPRISED, 1.2], [NEUTRAL, 1.0], [BIG_SMILE, 1.0], [SURPRISED, 1.0],
    [BROW_LOWER, 1.0], [null, 0.5], [YAWN, 1.0], [MID_SURPRISE, 1.0], [SURPRISED_MOUTH_SHUT, 8.0], [NEUTRAL, 5.0],
  ];
  let n = 0, surprises = 0;
  for (const [face, secs] of script) {
    for (let i = 0; i < Math.round(secs / DT); i++, n++) {
      const dt = DT * (1 + 0.3 * Math.sin(n));   // uneven frame times
      const cats = face && (n % 2 ? asCategories(face) : face);
      const a = js.update(cats, dt), b = rust.update(cats, dt);
      assert.deepEqual(b.mood, a.mood, `frame ${n}`);
      assert.equal(b.event, a.event, `frame ${n}`);
      assert.equal(b.surpriseTriggered, a.surpriseTriggered, `frame ${n}`);
      surprises += a.surpriseTriggered ? 1 : 0;
    }
  }
  assert.equal(surprises, 2);   // the second SURPRISED falls inside the 4 s cooldown
  assert.deepEqual(INPUTS.filter((k) => !Object.values(RECIPES).flat().some(([name]) => name === k)), []);
});
