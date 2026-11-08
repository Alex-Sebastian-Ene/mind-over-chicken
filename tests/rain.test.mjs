// Rain or Calm (#276): beta+theta drives the tile; holding F is the emulated fallback.
// Run: node --test tests/rain.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { rainFromBands, RAIN_THRESHOLD } from '../src/brain/brain.js';

test('rain is the beta + theta band sum', () => {
  assert.equal(rainFromBands({ beta: 0.3, theta: 0.2 }), 0.5);
  assert.equal(rainFromBands({ beta: 0.5, theta: 0.3 }), 0.8);
  assert.equal(rainFromBands({ beta: 0, theta: 0 }), 0);
  assert.equal(rainFromBands({ beta: 1, theta: 1 }), 1);
  assert.ok(RAIN_THRESHOLD > 0.4 && RAIN_THRESHOLD < 0.7);
});
