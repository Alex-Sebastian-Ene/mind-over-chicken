// Pinch movement calibration (#282): gain maths and stored shape.
// Run: node --test tests/pinch_calibration.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { gainFromSpan, defaultPinchCalibration } from '../src/camera/movement-calibration.js';

test('a comfortable 0.3 span keeps 1x', () => {
  assert.equal(gainFromSpan(0.3), 1);
});

test('small movement gets a larger gain and large movement a smaller one', () => {
  assert.ok(gainFromSpan(0.15) > 1.5);
  assert.ok(gainFromSpan(0.6) < 0.7);
});

test('gain is bounded to 0.5..2.5', () => {
  assert.equal(gainFromSpan(0.001), 2.5);
  assert.equal(gainFromSpan(10), 0.5);
  assert.ok(Number.isFinite(gainFromSpan(NaN)));
});

test('default calibration has three uncalibrated 1x inputs', () => {
  const c = defaultPinchCalibration();
  for (const k of ['left', 'right', 'head']) {
    assert.equal(c[k].gain, 1);
    assert.equal(c[k].span, 0);
    assert.equal(c[k].calibrated, false);
  }
});
