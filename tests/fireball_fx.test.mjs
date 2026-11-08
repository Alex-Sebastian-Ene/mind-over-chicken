// Fireball projectile (#315): travel timing and ground-slam impact callback.
// Run: node --test tests/fireball_fx.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { FireballFX, FIREBALL_FX } from '../src/render/fireball.js';

test('a shot lands once, after its travel time, at the target', () => {
  const fx = new FireballFX({ travel: 0.5, impact: 0.2 });
  fx.launch({ x: 120, y: 80 }, { color: '#ff3b30', charge: 1 });
  assert.equal(fx.active, 1);
  assert.deepEqual(fx.update(0.2), []);
  const hits = fx.update(0.31);
  assert.equal(hits.length, 1);
  assert.equal(hits[0].x, 120);
  assert.equal(hits[0].y, 80);
  assert.equal(hits[0].color, '#ff3b30');
  assert.equal(hits[0].charge, 1);
  assert.deepEqual(fx.update(0.1), [], 'the same shot never impacts twice');
});

test('the shot is removed after the impact ring finishes', () => {
  const fx = new FireballFX({ travel: 0.1, impact: 0.2 });
  fx.launch({ x: 0, y: 0 });
  fx.update(0.1);
  fx.update(0.21);
  assert.equal(fx.active, 0);
});

test('charge scales the starting height and the orb, and colour is kept', () => {
  const low = new FireballFX().launch({ x: 0, y: 100 }, { charge: 0.2 });
  const high = new FireballFX().launch({ x: 0, y: 100 }, { charge: 1 });
  assert.ok(high.y0 < low.y0, 'a full charge starts higher');
  assert.equal(high.color, '#ff9a2e');
  assert.ok(FIREBALL_FX.travel > 0.2 && FIREBALL_FX.travel < 1);
});

test('a custom start point is respected', () => {
  const fx = new FireballFX();
  const s = fx.launch({ x: 10, y: 20 }, { from: { x: 30, y: 40 } });
  assert.equal(s.x0, 30);
  assert.equal(s.y0, 40);
});
