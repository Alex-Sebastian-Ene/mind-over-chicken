// Pinch movement calibration (#282): gain maths and stored shape.
// Run: node --test tests/pinch_calibration.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { gainFromSpan, defaultPinchCalibration } from '../src/camera/movement-calibration.js';
import { HAND_COLORS } from '../src/camera/draw.js';

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

test('default calibration has left/right hands at 1x on both axes', () => {
  const c = defaultPinchCalibration();
  assert.deepEqual(Object.keys(c).sort(), ['left', 'right']);
  for (const k of ['left', 'right']) {
    assert.equal(c[k].gainX, 1);
    assert.equal(c[k].gainY, 1);
    assert.equal(c[k].spanX, 0);
    assert.equal(c[k].spanY, 0);
    assert.equal(c[k].calibrated, false);
  }
});

test('left and right hands have distinct colours', () => {
  assert.notEqual(HAND_COLORS.left, HAND_COLORS.right);
  assert.match(HAND_COLORS.left, /^#[0-9a-f]{6}$/i);
  assert.match(HAND_COLORS.right, /^#[0-9a-f]{6}$/i);
});
