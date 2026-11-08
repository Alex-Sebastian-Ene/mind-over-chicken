// Charged fireball (#291): fist → pull back → thrust + palm, colour mapping.
import test from 'node:test';
import assert from 'node:assert/strict';
import { FireballController, fireballColor, FIREBALL } from '../src/camera/fireball.js';

const hand = (scale = 1, pose = 'fist') => {
  const pts = Array.from({ length: 21 }, () => ({ x: 0.5, y: 0.6, z: 0 }));
  pts[0] = { x: 0.5, y: 0.78, z: 0 };
  pts[9] = { x: 0.5, y: 0.78 - 0.2 * scale, z: 0 };
  pts[5] = { x: 0.45, y: 0.64, z: 0 };
  pts[17] = { x: 0.55, y: 0.64, z: 0 };
  pts[4] = { x: 0.49, y: 0.67, z: 0 };
  pts[8] = pose === 'palm' ? { x: 0.5, y: 0.54, z: 0 } : { x: 0.49, y: 0.72, z: 0 };
  return pts;
};
const fist = { name: 'Closed_Fist', score: 0.9 };
const palm = { name: 'Open_Palm', score: 0.9 };
const mood = { joy: 1, surprise: 0 };

const feed = (c, n, { scale = 1, pose = 'fist', gesture = fist, mood: m = mood, pinching = false, start = 1000, step = 50 } = {}) => {
  let out;
  for (let i = 0; i < n; i++) {
    out = c.update({ hand: hand(scale, pose), gesture, mood: m, now: start + i * step, aspect: 4 / 3, pinching });
  }
  return out;
};

test('a fist alone charges but never fires; pull-back raises charge', () => {
  const c = new FireballController();
  assert.equal(feed(c, 1, { start: 1000 }).state, 'idle');
  assert.equal(feed(c, 8, { start: 1000 }).state, 'charging');
  const a = feed(c, 1, { start: 1000 }).charge;
  const b = feed(c, 20, { start: 2000, scale: 0.75 }).charge;
  assert.ok(b > a, `${a} -> ${b}`);
  assert.ok(b > FIREBALL.minCharge);
});

test('forward thrust then palm releases exactly one fireball with the mood colour', () => {
  const c = new FireballController();
  feed(c, 10, { start: 1000 });
  feed(c, 20, { start: 1500, scale: 0.8 });
  const thrust = feed(c, 2, { start: 2600, scale: 1.2 });
  assert.equal(thrust.state, 'primed', `state ${thrust.state} thrust ${thrust.thrust}`);
  const out = c.update({ hand: hand(1.2, 'palm'), gesture: palm, mood, now: 2700, aspect: 4 / 3 });
  assert.equal(out.cast, true);
  assert.equal(out.state, 'cooldown');
  assert.match(out.color, /^#[0-9a-f]{6}$/i);
  const again = c.update({ hand: hand(1.2, 'palm'), gesture: palm, mood, now: 2750, aspect: 4 / 3 });
  assert.equal(again.cast, false);
});

test('opening the palm after a fist cast (no thrust needed); thrust without palm does not fire', () => {
  const c = new FireballController();
  feed(c, 45, { start: 1000 });
  const released = c.update({ hand: hand(1, 'palm'), gesture: palm, mood, now: 3300, aspect: 4 / 3 });
  assert.equal(released.cancelled, false);
  assert.equal(released.cast, true);

  const d = new FireballController();
  feed(d, 10, { start: 1000 });
  feed(d, 20, { start: 1500, scale: 0.8 });
  feed(d, 2, { start: 2600, scale: 1.2 });
  const noPalm = d.update({ hand: hand(1.2, 'fist'), gesture: fist, mood, now: 3200, aspect: 4 / 3 });
  assert.equal(noPalm.cast, false);
  assert.notEqual(noPalm.state, 'released');
});

test('pinch cancels and low charge cannot release', () => {
  const c = new FireballController();
  feed(c, 10, { start: 1000 });
  const p = c.update({ hand: hand(1, 'fist'), gesture: fist, mood, now: 1600, aspect: 4 / 3, pinching: true });
  assert.equal(p.cancelled, true);

  const d = new FireballController();
  feed(d, 10, { start: 1000 });
  const early = d.update({ hand: hand(1.2, 'fist'), gesture: fist, mood, now: 1300, aspect: 4 / 3 });
  assert.ok(early.charge < FIREBALL.minCharge);
  const palmOut = d.update({ hand: hand(1.2, 'palm'), gesture: palm, mood, now: 1350, aspect: 4 / 3 });
  assert.equal(palmOut.cast, false);
});

test('colour: joy red, surprise blue, neutral ember, surprise normalised at 0.2', () => {
  assert.match(fireballColor({ joy: 1, surprise: 0 }), /^#[0-9a-f]{6}$/i);
  const red = fireballColor({ joy: 1, surprise: 0 });
  const blue = fireballColor({ joy: 0, surprise: 0.2 });
  assert.notEqual(red, blue);
  assert.equal(fireballColor({ joy: 0, surprise: 0 }), '#ff9a2e');
  const half = fireballColor({ joy: 0, surprise: 0.1 });
  assert.notEqual(half, fireballColor({ joy: 0, surprise: 0.2 }));
});

test('keyboard fallback charges with hold 3 and throws on release', () => {
  const c = new FireballController();
  c.keyboardDown(mood, 1000);
  for (let i = 1; i <= 30; i++) c.update({ mood, now: 1000 + i * 50 });
  const out = c.keyboardUp(mood, 2600);
  assert.equal(out.cast, true);
  assert.equal(c.keyboard, false);
});
