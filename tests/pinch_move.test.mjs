// One-hand pinch movement (#270): hand movement pans, head movement tilts, deadzones keep
// hand tremor and head jitter from fighting each other.
// Run: node --test tests/pinch_move.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { pinchMove, lowPass, PinchMoveSmoother, PINCH_HAND_DEADZONE, PINCH_HEAD_DEADZONE, PINCH_MOVE_TAU } from '../src/camera/pinch-move.js';

test('hand movement pans and leaves the camera alone when the head is still', () => {
  const m = pinchMove({ handDx: 0.02, handDy: -0.01 });
  assert.equal(m.panX, 0.02);
  assert.equal(m.panY, -0.01);
  assert.equal(m.tiltX, 0);
  assert.equal(m.tiltY, 0);
});

test('head movement tilts and does not pan the map', () => {
  const m = pinchMove({ headDx: 0.02, headDy: 0.01 });
  assert.equal(m.panX, 0);
  assert.equal(m.panY, 0);
  assert.equal(m.tiltX, 0.02);
  assert.equal(m.tiltY, 0.01);
});

test('hand and head can move together without fighting', () => {
  const m = pinchMove({ handDx: 0.03, handDy: 0.02, headDx: -0.02, headDy: 0.01 });
  assert.deepEqual(m, { panX: 0.03, panY: 0.02, tiltX: -0.02, tiltY: 0.01 });
});

test('deadzones reject hand tremor and head jitter', () => {
  const m = pinchMove({
    handDx: PINCH_HAND_DEADZONE * 0.9,
    handDy: PINCH_HAND_DEADZONE * 0.5,
    headDx: PINCH_HEAD_DEADZONE * 0.9,
    headDy: PINCH_HEAD_DEADZONE * 0.5,
  });
  assert.deepEqual(m, { panX: 0, panY: 0, tiltX: 0, tiltY: 0 });
});

test('a released pinch sends no movement at all', () => {
  const m = pinchMove({ handDx: 0, handDy: 0, headDx: 0, headDy: 0 });
  assert.deepEqual(m, { panX: 0, panY: 0, tiltX: 0, tiltY: 0 });
});

test('low-pass settles on a steady hand delta', () => {
  const s = new PinchMoveSmoother();
  let out;
  for (let i = 0; i < 60; i++) out = s.filter({ handDx: 0.02, handDy: -0.01 }, 1 / 60);
  assert.ok(Math.abs(out.handDx - 0.02) < 0.001, `handDx ${out.handDx}`);
  assert.ok(Math.abs(out.handDy + 0.01) < 0.001, `handDy ${out.handDy}`);
});

test('low-pass damps alternating landmark jitter', () => {
  const s = new PinchMoveSmoother();
  let peak = 0;
  for (let i = 0; i < 30; i++) {
    const out = s.filter({ handDx: i % 2 ? 0.02 : -0.02 }, 1 / 30);
    if (i >= 5) peak = Math.max(peak, Math.abs(out.handDx));
  }
  assert.ok(peak < 0.01, `jitter peak ${peak}`);
});

test('low-pass resets between pinches and keeps zero at zero', () => {
  const s = new PinchMoveSmoother();
  s.filter({ handDx: 0.05, headDx: -0.05 }, 1 / 30);
  s.reset();
  assert.deepEqual(s.filter({}, 1 / 30), { handDx: 0, handDy: 0, headDx: 0, headDy: 0 });
  const z = new PinchMoveSmoother();
  for (let i = 0; i < 10; i++) assert.deepEqual(z.filter({}, 1 / 30), { handDx: 0, handDy: 0, headDx: 0, headDy: 0 });
});

test('lowPass is a time-based exponential step', () => {
  const v = lowPass(0, 1, PINCH_MOVE_TAU, PINCH_MOVE_TAU);
  assert.ok(Math.abs(v - (1 - Math.exp(-1))) < 1e-9, String(v));
});
