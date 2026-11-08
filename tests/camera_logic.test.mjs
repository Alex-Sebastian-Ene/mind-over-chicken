// camera (#33, #117, #122): unit tests for the camera spell table and the joy+surprise emotion model.
// Run: node --test tests/camera_logic.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { SPELLS, SPELL_BY_ID, spellForKeyCode } from '../src/camera/spells.js';
import { EMOTIONS, EmotionTracker, emotionsFromBlendshapes, rawEmotions, blendshapeMap } from '../src/camera/emotion.js';

// ---------- spells (#122, #150, #159) ----------
// The cast debounce, cooldown and fist-to-open fireball tests run against both the JS controller
// and the wasm one the browser uses: tests/camera_casts.test.mjs.
test('three core moves (keys 1–3): Grab, Calm, Fireball; no Strike and no legacy casts', () => {
  assert.equal(SPELLS.length, 3);
  assert.deepEqual(SPELLS.map((s) => s.key), [1, 2, 3]);
  assert.deepEqual(SPELLS.map((s) => s.id), ['pinch', 'palm', 'fireball']);
  assert.equal(spellForKeyCode('Digit1').id, 'pinch');
  assert.equal(spellForKeyCode('Digit2').id, 'palm');
  assert.equal(spellForKeyCode('Digit3').id, 'fireball');
  assert.equal(spellForKeyCode('Digit4'), null);
  assert.equal(SPELL_BY_ID.fireball.gesture, 'Closed_Fist');
  assert.equal(SPELL_BY_ID.fireball.release, 'Open_Palm');
  for (const id of ['fist', 'point', 'spawn', 'rain', 'bless', 'disaster', 'peace']) assert.equal(SPELL_BY_ID[id], undefined, id);
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
