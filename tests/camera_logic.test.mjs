// camera (#33): unit tests for the pure camera logic, with mocked MediaPipe outputs.
// Run: node --test tests/camera_logic.test.mjs   (also run by tests/test_camera_logic.py)

import test from 'node:test';
import assert from 'node:assert/strict';
import { CastController } from '../src/camera/casts.js';
import { SPELLS, SPELL_BY_ID, spellForKeyCode } from '../src/camera/spells.js';
import { EmotionTracker, emotionsFromBlendshapes, rawEmotions, blendshapeMap } from '../src/camera/emotion.js';

// Feed a gesture at 30 fps from t0 for ms milliseconds; return the casts that fired.
function hold(c, gesture, t0, ms, score = 0.9, step = 1000 / 30) {
  const casts = [];
  let last;
  for (let t = t0; t <= t0 + ms; t += step) {
    last = c.update(gesture, score, t);
    if (last.cast) casts.push({ id: last.cast, t });
  }
  return { casts, last };
}

// ---------- spells ----------
test('seven spells, keys 1–7, one per canned gesture', () => {
  assert.equal(SPELLS.length, 7);
  assert.deepEqual(SPELLS.map((s) => s.key), [1, 2, 3, 4, 5, 6, 7]);
  assert.deepEqual(new Set(SPELLS.map((s) => s.gesture)), new Set(['Pointing_Up', 'Closed_Fist', 'Open_Palm', 'Victory', 'Thumb_Up', 'Thumb_Down', 'ILoveYou']));
  assert.equal(spellForKeyCode('Digit2').id, 'fireball');
  assert.equal(spellForKeyCode('Numpad7').id, 'peace');
  assert.equal(spellForKeyCode('Digit8'), null);
  assert.equal(spellForKeyCode('KeyF'), null);
});

test('gesture to cast mapping matches the issue table', () => {
  const c = new CastController();
  assert.equal(c.spellFor('Pointing_Up'), 'point');
  assert.equal(c.spellFor('Closed_Fist'), 'fireball');
  assert.equal(c.spellFor('Open_Palm'), 'spawn');
  assert.equal(c.spellFor('Victory'), 'rain');
  assert.equal(c.spellFor('Thumb_Up'), 'bless');
  assert.equal(c.spellFor('Thumb_Down'), 'disaster');
  assert.equal(c.spellFor('ILoveYou'), 'peace');
  assert.equal(c.spellFor('None'), null);
});

// ---------- debounce ----------
test('a gesture must be held ~300 ms before it casts', () => {
  const c = new CastController();
  const short = hold(c, 'Closed_Fist', 0, 250);
  assert.equal(short.casts.length, 0);
  assert.ok(short.last.progress > 0.7 && short.last.progress < 1, `progress ${short.last.progress}`);
  const r = c.update('Closed_Fist', 0.9, 300);
  assert.equal(r.cast, 'fireball');
});

test('casts once per hold, even when the hold outlasts the cooldown', () => {
  const c = new CastController();
  const { casts } = hold(c, 'Open_Palm', 0, 20000);
  assert.equal(casts.length, 1);
  assert.equal(casts[0].id, 'spawn');
  assert.ok(casts[0].t >= 300 && casts[0].t < 340);
});

test('low-confidence gestures are ignored', () => {
  const c = new CastController();
  assert.equal(hold(c, 'Closed_Fist', 0, 1000, 0.3).casts.length, 0);
});

test('short tracking dropouts do not break a hold, longer ones do', () => {
  const c = new CastController({ graceMs: 150 });
  c.update('Victory', 0.9, 0);
  c.update('Victory', 0.9, 150);
  c.update('None', 0, 200);           // 50 ms dropout
  assert.equal(c.update('Victory', 0.9, 310).cast, 'rain');

  const d = new CastController({ graceMs: 150 });
  d.update('Victory', 0.9, 0);
  d.update('Victory', 0.9, 150);
  d.update('None', 0, 200);
  d.update('None', 0, 400);           // 250 ms since last seen: hold dropped
  assert.equal(d.update('Victory', 0.9, 420).cast, null);
  assert.equal(d.update('Victory', 0.9, 720).cast, 'rain');
});

test('switching gestures restarts the hold', () => {
  const c = new CastController();
  c.update('Thumb_Up', 0.9, 0);
  c.update('Thumb_Up', 0.9, 200);
  assert.equal(c.update('Thumb_Down', 0.9, 250).cast, null);
  assert.equal(c.update('Thumb_Down', 0.9, 500).cast, null);
  assert.equal(c.update('Thumb_Down', 0.9, 560).cast, 'disaster');
});

// ---------- cooldowns ----------
test('each spell has its own cooldown', () => {
  const c = new CastController();
  assert.equal(hold(c, 'Closed_Fist', 0, 400).casts.length, 1);
  c.update('None', 0, 600);
  // A different spell is not blocked by the fireball cooldown.
  assert.equal(hold(c, 'Open_Palm', 700, 400).casts.length, 1);
  c.update('None', 0, 1200);
  // Fireball again while still cooling down (1.5 s from t≈300): holds, then fires when ready.
  const again = hold(c, 'Closed_Fist', 1300, 1000);
  assert.equal(again.casts.length, 1);
  assert.ok(again.casts[0].t >= 300 + SPELL_BY_ID.fireball.cooldown, `fired at ${again.casts[0].t}`);
});

test('keyboard casts share the cooldowns', () => {
  const c = new CastController();
  assert.equal(c.trigger('peace', 0), true);
  assert.equal(c.trigger('peace', 1000), false);
  assert.ok(Math.abs(c.cooldownFrac('peace', 0) - 1) < 1e-9);
  assert.ok(c.cooldownFrac('peace', SPELL_BY_ID.peace.cooldown / 2) > 0.49);
  assert.equal(c.cooldownLeft('peace', SPELL_BY_ID.peace.cooldown), 0);
  assert.equal(c.trigger('peace', SPELL_BY_ID.peace.cooldown), true);
  // A key cast puts the gesture on cooldown too.
  c.trigger('bless', 0);
  assert.equal(hold(c, 'Thumb_Up', 10, 1000).casts.length, 0);
  assert.equal(c.trigger('nonsense', 0), false);
});

test('pointing aims immediately, without waiting for the hold', () => {
  const c = new CastController();
  const r = c.update('Pointing_Up', 0.6, 0);
  assert.equal(r.aiming, true);
  assert.equal(r.cast, null);
  assert.equal(c.update('Closed_Fist', 0.9, 10).aiming, false);
});

// ---------- emotions ----------
const face = (o = {}) => Object.entries(o).map(([categoryName, score]) => ({ categoryName, score }));
const SMILE = { mouthSmileLeft: 0.8, mouthSmileRight: 0.8, cheekSquintLeft: 0.3 };
const FROWN_BROWS = { browDownLeft: 0.55, browDownRight: 0.55, mouthPressLeft: 0.4, mouthPressRight: 0.4 };
const GASP = { jawOpen: 0.65, browInnerUp: 0.7, eyeWideLeft: 0.5, eyeWideRight: 0.5 };
const SAD = { mouthFrownLeft: 0.35, mouthFrownRight: 0.35, browInnerUp: 0.55 };

function dominantOf(shape) {
  const e = emotionsFromBlendshapes(face(shape));
  return Object.entries(e).sort((a, b) => b[1] - a[1])[0][0];
}

test('blendshapes map to the right emotion', () => {
  assert.equal(dominantOf(SMILE), 'joy');
  assert.equal(dominantOf(FROWN_BROWS), 'anger');
  assert.equal(dominantOf(GASP), 'surprise');
  assert.equal(dominantOf(SAD), 'sadness');
  const rest = emotionsFromBlendshapes(face({ browDownLeft: 0.05, mouthSmileLeft: 0.04 }));
  for (const v of Object.values(rest)) assert.ok(v < 0.1, JSON.stringify(rest));
});

test('surprise is not read as sadness, a smile is not read as anger', () => {
  const gasp = emotionsFromBlendshapes(face(GASP));
  assert.ok(gasp.sadness < 0.2, `sadness ${gasp.sadness}`);
  const smirk = emotionsFromBlendshapes(face({ ...SMILE, browDownLeft: 0.3, browDownRight: 0.3 }));
  assert.ok(smirk.joy > smirk.anger);
});

test('accepts a category list or a plain map, scores stay in 0..1', () => {
  assert.deepEqual(blendshapeMap(face({ jawOpen: 0.2 })), { jawOpen: 0.2 });
  assert.deepEqual(rawEmotions({ jawOpen: 0.5 }), rawEmotions(face({ jawOpen: 0.5 })));
  const e = emotionsFromBlendshapes({ mouthSmileLeft: 5, mouthSmileRight: 5, jawOpen: -1 });
  for (const v of Object.values(e)) assert.ok(v >= 0 && v <= 1);
});

// Run the tracker at 15 Hz (the face runs on every other video frame).
function run(tracker, shape, seconds, dt = 1 / 15) {
  const events = [];
  let out;
  for (let t = 0; t < seconds; t += dt) {
    out = tracker.update(shape === null ? null : face(shape), dt);
    if (out.event) events.push({ event: out.event, dominant: out.mood.dominant });
  }
  return { events, mood: out.mood };
}

test('tracker: neutral face stays neutral, smile becomes joy with one change event', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0 });
  const calm = run(t, {}, 2);
  assert.equal(calm.mood.dominant, 'neutral');
  assert.equal(calm.events.length, 0);
  assert.equal(calm.mood.neutral, 1);
  const smile = run(t, SMILE, 1.5);
  assert.equal(smile.mood.dominant, 'joy');
  assert.deepEqual(smile.events, [{ event: 'change', dominant: 'joy' }]);
  assert.ok(smile.mood.joy > 0.8 && smile.mood.neutral < 0.2);
});

test('tracker: smoothing ignores a one-frame grimace', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0 });
  run(t, {}, 1);
  t.update(face(GASP), 1 / 15);
  const after = run(t, {}, 0.5);
  assert.equal(after.mood.dominant, 'neutral');
  assert.equal(after.events.length, 0);
});

test('tracker: hysteresis keeps a fading mood until it drops below exit', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0, enter: 0.45, exit: 0.3 });
  run(t, SMILE, 1.5);
  // A half smile scores ~0.4: below enter, above exit. Still joy.
  const half = run(t, { mouthSmileLeft: 0.28, mouthSmileRight: 0.28 }, 2);
  assert.ok(half.mood.joy > 0.3 && half.mood.joy < 0.45, `joy ${half.mood.joy}`);
  assert.equal(half.mood.dominant, 'joy');
  const gone = run(t, {}, 2);
  assert.equal(gone.mood.dominant, 'neutral');
  assert.deepEqual(gone.events, [{ event: 'change', dominant: 'neutral' }]);
});

test('tracker: a strong mood re-emits every strongEvery seconds', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0, strongEvery: 5 });
  const { events } = run(t, FROWN_BROWS, 16);
  assert.equal(events[0].event, 'change');
  assert.equal(events[0].dominant, 'anger');
  assert.equal(events.filter((e) => e.event === 'strong').length, 3);
});

test('tracker: switching moods needs a clear margin', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0 });
  run(t, SMILE, 1.5);
  const { mood, events } = run(t, GASP, 1.5);
  assert.equal(mood.dominant, 'surprise');
  assert.deepEqual(events.map((e) => e.dominant), ['surprise']);
});

test('tracker: losing the face decays to neutral', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0 });
  run(t, SAD, 1.5);
  assert.equal(t.dominant, 'sadness');
  const lost = run(t, null, 2);
  assert.equal(lost.mood.dominant, 'neutral');
  assert.equal(lost.mood.face, false);
});

test('tracker: calibration subtracts the resting face', () => {
  // This player's resting face has lowered brows (glasses, lighting...).
  const REST = { browDownLeft: 0.35, browDownRight: 0.35, mouthPressLeft: 0.12, mouthPressRight: 0.12 };
  const naive = new EmotionTracker({ calibrateSeconds: 0 });
  assert.equal(run(naive, REST, 2).mood.dominant, 'anger');
  const t = new EmotionTracker({ calibrateSeconds: 1.5 });
  const during = run(t, REST, 1);
  assert.equal(during.mood.calibrating, true);
  assert.equal(during.mood.dominant, 'neutral');
  const after = run(t, REST, 2);
  assert.equal(after.mood.dominant, 'neutral');
  assert.equal(after.mood.calibrating, false);
  assert.equal(run(t, FROWN_BROWS, 1.5).mood.dominant, 'anger');
});

test('reset returns to a neutral, uncalibrated tracker', () => {
  const t = new EmotionTracker();
  run(t, SMILE, 3);
  t.reset();
  assert.equal(t.dominant, 'neutral');
  assert.equal(t.calibrated, false);
  assert.deepEqual(t.mood().dominant, 'neutral');
});
