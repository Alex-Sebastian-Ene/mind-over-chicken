// camera (#33, #117, #122): unit tests for the camera logic under the 3 core moves and joy+surprise emotion model.
// Run: node --test tests/camera_logic.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { CastController } from '../src/camera/casts.js';
import { SPELLS, ADVANCED_SPELLS, SPELL_BY_ID, spellForKeyCode } from '../src/camera/spells.js';
import { EMOTIONS, EmotionTracker, emotionsFromBlendshapes, rawEmotions, blendshapeMap } from '../src/camera/emotion.js';

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

// ---------- spells (#122) ----------
test('three core moves (keys 1–3) replace the 7-spell row', () => {
  assert.equal(SPELLS.length, 3);
  assert.deepEqual(SPELLS.map((s) => s.key), [1, 2, 3]);
  assert.deepEqual(SPELLS.map((s) => s.id), ['pinch', 'palm', 'fist']);
  assert.equal(spellForKeyCode('Digit1').id, 'pinch');
  assert.equal(spellForKeyCode('Digit2').id, 'palm');
  assert.equal(spellForKeyCode('Digit3').id, 'fist');
  assert.equal(spellForKeyCode('Digit4'), null);

  // Advanced legacy spells are preserved in ADVANCED_SPELLS and SPELL_BY_ID
  assert.ok(ADVANCED_SPELLS.length >= 7);
  assert.ok(SPELL_BY_ID.fireball);
  assert.ok(SPELL_BY_ID.peace);
  assert.ok(SPELL_BY_ID.rain);
});

test('gesture to core cast mapping matches the issue table', () => {
  const c = new CastController();
  assert.equal(c.spellFor('Open_Palm'), 'palm');
  assert.equal(c.spellFor('Closed_Fist'), 'fist');
  assert.equal(c.spellFor('None'), null);
});

// ---------- debounce ----------
test('a gesture must be held ~300 ms before it casts', () => {
  const c = new CastController();
  const short = hold(c, 'Closed_Fist', 0, 250);
  assert.equal(short.casts.length, 0);
  assert.ok(short.last.progress > 0.7 && short.last.progress < 1, `progress ${short.last.progress}`);
  const r = c.update('Closed_Fist', 0.9, 300);
  assert.equal(r.cast, 'fist');
});

test('casts once per hold, even when the hold outlasts the cooldown', () => {
  const c = new CastController();
  const { casts } = hold(c, 'Open_Palm', 0, 20000);
  assert.equal(casts.length, 1);
  assert.equal(casts[0].id, 'palm');
  assert.ok(casts[0].t >= 300 && casts[0].t < 340);
});

test('low-confidence gestures are ignored', () => {
  const c = new CastController();
  assert.equal(hold(c, 'Closed_Fist', 0, 1000, 0.3).casts.length, 0);
});

test('short tracking dropouts do not break a hold, longer ones do', () => {
  const c = new CastController({ graceMs: 150 });
  c.update('Open_Palm', 0.9, 0);
  c.update('Open_Palm', 0.9, 150);
  c.update('None', 0, 200);           // 50 ms dropout
  assert.equal(c.update('Open_Palm', 0.9, 310).cast, 'palm');

  const d = new CastController({ graceMs: 150 });
  d.update('Open_Palm', 0.9, 0);
  d.update('Open_Palm', 0.9, 150);
  d.update('None', 0, 200);
  d.update('None', 0, 400);           // 250 ms since last seen: hold dropped
  assert.equal(d.update('Open_Palm', 0.9, 420).cast, null);
  assert.equal(d.update('Open_Palm', 0.9, 720).cast, 'palm');
});

test('switching gestures restarts the hold', () => {
  const c = new CastController();
  c.update('Open_Palm', 0.9, 0);
  c.update('Open_Palm', 0.9, 200);
  assert.equal(c.update('Closed_Fist', 0.9, 250).cast, null);
  assert.equal(c.update('Closed_Fist', 0.9, 500).cast, null);
  assert.equal(c.update('Closed_Fist', 0.9, 560).cast, 'fist');
});

// ---------- cooldowns ----------
test('each spell has its own cooldown', () => {
  const c = new CastController();
  assert.equal(hold(c, 'Closed_Fist', 0, 400).casts.length, 1);
  c.update('None', 0, 600);
  assert.equal(hold(c, 'Open_Palm', 700, 400).casts.length, 1);
  c.update('None', 0, 1200);
  const again = hold(c, 'Closed_Fist', 1300, 1000);
  assert.equal(again.casts.length, 1);
  assert.ok(again.casts[0].t >= 300 + SPELL_BY_ID.fist.cooldown, `fired at ${again.casts[0].t}`);
});

test('keyboard casts share the cooldowns', () => {
  const c = new CastController();
  assert.equal(c.trigger('palm', 0), true);
  assert.equal(c.trigger('palm', 1000), false);
  assert.ok(Math.abs(c.cooldownFrac('palm', 0) - 1) < 1e-9);
  assert.ok(c.cooldownFrac('palm', SPELL_BY_ID.palm.cooldown / 2) > 0.49);
  assert.equal(c.cooldownLeft('palm', SPELL_BY_ID.palm.cooldown), 0);
  assert.equal(c.trigger('palm', SPELL_BY_ID.palm.cooldown), true);
  c.trigger('palm', 0);
  assert.equal(hold(c, 'Open_Palm', 10, 1000).casts.length, 0);
  assert.equal(c.trigger('nonsense', 0), false);
});

// ---------- emotions (#117: joy and surprise only) ----------
const face = (o = {}) => Object.entries(o).map(([categoryName, score]) => ({ categoryName, score }));
const SMILE = { mouthSmileLeft: 0.8, mouthSmileRight: 0.8, cheekSquintLeft: 0.3 };
const GASP = { jawOpen: 0.65, browInnerUp: 0.7, eyeWideLeft: 0.5, eyeWideRight: 0.5 };

function dominantOf(shape) {
  const e = emotionsFromBlendshapes(face(shape));
  return Object.entries(e).sort((a, b) => b[1] - a[1])[0][0];
}

test('blendshapes map to joy and surprise', () => {
  assert.deepEqual(EMOTIONS, ['joy', 'surprise']);
  assert.equal(dominantOf(SMILE), 'joy');
  assert.equal(dominantOf(GASP), 'surprise');
});

test('accepts a category list or a plain map, scores stay in 0..1', () => {
  assert.deepEqual(blendshapeMap(face({ jawOpen: 0.2 })), { jawOpen: 0.2 });
  assert.deepEqual(rawEmotions({ jawOpen: 0.5 }), rawEmotions(face({ jawOpen: 0.5 })));
  const e = emotionsFromBlendshapes({ mouthSmileLeft: 5, mouthSmileRight: 5, jawOpen: -1 });
  for (const v of Object.values(e)) assert.ok(v >= 0 && v <= 1);
});

function run(tracker, shape, seconds, dt = 1 / 15) {
  const events = [];
  let out;
  let triggered = false;
  for (let t = 0; t < seconds; t += dt) {
    out = tracker.update(shape === null ? null : face(shape), dt);
    if (out.surpriseTriggered) triggered = true;
    if (out.event) events.push({ event: out.event, dominant: out.mood.dominant });
  }
  return { events, mood: out.mood, lastOut: out, triggered };
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

test('tracker: debounced surpriseTriggered event (#117)', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0 });
  // Short flicker under 300 ms does not trigger surprise event
  const short = run(t, GASP, 0.2);
  assert.equal(short.triggered, false);
  // Sustained gasp >= 300 ms triggers surpriseTriggered
  const sustained = run(t, GASP, 0.8);
  assert.equal(sustained.triggered, true);
  // Within 4s cooldown, subsequent gasps do not re-trigger the event
  const cooldown = run(t, GASP, 1.0);
  assert.equal(cooldown.triggered, false);
});

test('tracker: losing the face decays to neutral', () => {
  const t = new EmotionTracker({ calibrateSeconds: 0 });
  run(t, SMILE, 1.5);
  assert.equal(t.dominant, 'joy');
  const lost = run(t, null, 2);
  assert.equal(lost.mood.dominant, 'neutral');
  assert.equal(lost.mood.face, false);
});

test('reset returns to a neutral, uncalibrated tracker', () => {
  const t = new EmotionTracker();
  run(t, SMILE, 3);
  t.reset();
  assert.equal(t.dominant, 'neutral');
  assert.equal(t.calibrated, false);
  assert.deepEqual(t.mood().dominant, 'neutral');
});
