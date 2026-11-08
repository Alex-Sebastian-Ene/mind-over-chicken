// The Stone Age tribe's hunger clock (#298): slower hunger, a longer grace, a longer starve.
import test from 'node:test';
import assert from 'node:assert/strict';
import { STONE_HUNGER } from '../src/eras/stone.js';

test('the tribe gets hungry slowly and holds out longer', () => {
  assert.ok(STONE_HUNGER.rate >= 0.008 && STONE_HUNGER.rate <= 0.010, `hunger rate ${STONE_HUNGER.rate}/s`);
  assert.ok(STONE_HUNGER.assistRate < STONE_HUNGER.rate, 'assist mode is gentler still');
  assert.equal(STONE_HUNGER.grace, 120);
  assert.equal(STONE_HUNGER.starve, 12);
  // From fed (hunger 0) to dead without food, past the grace: was 1/0.017 + 8 ≈ 67 s.
  const outlast = 1 / STONE_HUNGER.rate + STONE_HUNGER.starve;
  assert.ok(outlast >= 110, `a person lasts ${outlast.toFixed(0)} s without food`);
  assert.ok(Object.isFrozen(STONE_HUNGER));
});
