// #190: the calibration result shapes your civilisation in the sandbox.
import test from 'node:test';
import assert from 'node:assert/strict';
import { calibrationTraits, calibrationLine } from '../src/civ/calibration-traits.js';

test('a calibration result gives the three traits, clamped; anything else gives none', () => {
  assert.deepEqual(calibrationTraits({ friendliness: 0.7, aggressiveness: 1.4, violence: -0.2, source: 'sensor' }),
    { friendliness: 0.7, aggressiveness: 1, violence: 0 });
  assert.equal(calibrationTraits(null), null);
  assert.equal(calibrationTraits({ friendliness: 0.5 }), null);
});

test('the log line says what was applied', () => {
  assert.match(calibrationLine({ friendliness: 0.62, aggressiveness: 0.31, violence: 0.2 }), /friendly 62%, aggressive 31%, violent 20%/);
});
