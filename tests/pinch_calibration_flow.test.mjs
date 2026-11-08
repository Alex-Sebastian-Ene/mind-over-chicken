// Pinch calibration flow (#282, #288): four cardinal targets per hand, auto-advance and complete.
// Run: node --test tests/pinch_calibration_flow.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { initMovementCalibration } from '../src/camera/movement-calibration.js';

const el = () => ({ textContent: '', disabled: false, addEventListener() {} });
const ctx = new Proxy({}, {
  get: (o, k) => (k in o ? o[k] : () => {}),
  set: (o, k, v) => { o[k] = v; return true; },
});
const canvas = { width: 960, height: 500, getContext: () => ctx };

test('four cardinal targets per hand auto-advance and complete', () => {
  const state = {
    live: true,
    video: null,
    brain: {},
    hands: {
      left: { visible: true, pinching: true, point: { x: 0.5, y: 0.5 } },
      right: { visible: true, pinching: true, point: { x: 0.5, y: 0.5 } },
    },
  };
  let completed = null;
  const cal = initMovementCalibration({
    canvas, statusEl: el(), msgEl: el(), startBtn: el(), skipBtn: el(), resetBtn: el(),
    getState: () => state,
    onChange: () => {},
    onComplete: (out) => { completed = out; },
  });
  cal.start();
  const targets = [[0.22, 0.5], [0.78, 0.5], [0.5, 0.3], [0.5, 0.7]];
  let now = performance.now();
  for (const side of ['left', 'right']) {
    for (const [x, y] of targets) {
      state.hands[side].point = { x, y };
      for (let i = 0; i < 6; i++) { now += 100; cal.update(now); }
    }
  }
  assert.ok(completed, 'onComplete fired');
  assert.equal(cal.active, false);
  assert.ok(completed.left.calibrated && completed.right.calibrated);
  assert.ok(completed.left.gainX > 0.5 && completed.left.gainY > 0.5);
  assert.equal(completed.head, undefined, 'head calibration removed');
});
