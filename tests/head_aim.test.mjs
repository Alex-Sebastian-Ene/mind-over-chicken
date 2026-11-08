// head aim (#221): a non-finite tilt or camera mapping can no longer poison the aim for good (it used to stay NaN forever, so the wizard
// cursor and the in-game reticle silently drew nothing).
// Run: node --test tests/head_aim.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { Brain, setRustBrain } from '../src/brain/brain.js';

// What the wizard draws (src/ui/eeg-panel.js screenAimOf, #234).
const headScreenAim = (brain) => brain?.screenAim || brain?.aim || null;

const BANDS = { delta: 0.25, theta: 0.2, alpha: 0.2, beta: 0.2, gamma: 0.15 };
const frame = (tilt, extra = {}) => ({ bands: BANDS, blink: false, tilt, motion: 0, quality: 1, ...extra });
const finite = (p) => Number.isFinite(p.x) && Number.isFinite(p.y);

function headsetBrain() {
  const b = new Brain({ update() {} });
  // The home world is twice the screen (#129): screen 0..1 lands on the middle half of the map.
  b.screenToAim = (x, y) => ({ x: 0.25 + x * 0.5, y: 0.25 + y * 0.5 });
  return b;
}

test('the wizard aims with the head screen point, so edge targets are reachable (#221)', () => {
  const b = headsetBrain();
  for (let i = 0; i < 60; i++) b.ingest(frame({ x: 0.96, y: -0.96 }));
  const head = headScreenAim(b);
  assert.ok(head.x > 0.94 && head.y < 0.06, `head screen aim reaches the top-right corner: ${JSON.stringify(head)}`);
  // The map aim is squeezed by the camera mapping; the wizard must not draw that.
  assert.ok(b.aim.x < 0.75 && b.aim.y > 0.25, `map aim is a world point: ${JSON.stringify(b.aim)}`);
  // The sandbox's TOP RIGHT target (0.88, 0.14) highlights within 0.09.
  for (let i = 0; i < 60; i++) b.ingest(frame({ x: 0.76, y: -0.72 }));
  assert.ok(Math.hypot(headScreenAim(b).x - 0.88, headScreenAim(b).y - 0.14) < 0.09);
});

test('a non-finite tilt is ignored and reported, not smoothed into a permanent NaN (#221)', () => {
  const b = headsetBrain();
  for (let i = 0; i < 20; i++) b.ingest(frame({ x: 0.2, y: 0.2 }));
  const before = { ...b.screenAim };
  b.ingest(frame({ x: NaN, y: 0.1 }));
  assert.ok(finite(b.aim) && finite(b.screenAim), 'aim stays finite');
  assert.deepEqual(b.screenAim, before, 'the bad frame changes nothing');
  assert.match(b.aimFault, /non-finite head tilt/);
  b.ingest(frame({ x: 0.2, y: 0.2 }));
  assert.equal(b.aimFault, null, 'the next good frame clears the fault');

  // Already poisoned (e.g. by an older build or a direct write): heals to the centre and moves on.
  b.screenAim = { x: NaN, y: NaN };
  b.ingest(frame({ x: 0, y: 0 }));
  assert.ok(finite(b.screenAim) && finite(b.aim));
  assert.ok(Math.abs(b.screenAim.x - 0.5) < 1e-9);
});

test('a camera mapping that fails keeps the last finite map aim (#221)', () => {
  const b = headsetBrain();
  for (let i = 0; i < 10; i++) b.ingest(frame({ x: 0.4, y: 0.4 }));
  const last = { ...b.aim };
  b.screenToAim = () => ({ x: NaN, y: 0.3 });
  b.ingest(frame({ x: 0.5, y: 0.5 }));
  assert.deepEqual(b.aim, last);
  assert.match(b.aimFault, /could not map/);
  assert.ok(finite(headScreenAim(b)), 'the wizard still has the screen point');
});

test('emulated headset: the map aim is the cursor; no screen point is invented (#221)', () => {
  const b = new Brain({ update() {} });
  b.ingest(frame({ x: 0.6, y: -0.2 }, { isSim: true }));
  assert.equal(b.screenAim, null);
  assert.deepEqual(headScreenAim(b), b.aim);
  assert.ok(Math.abs(b.aim.x - 0.8) < 1e-9 && Math.abs(b.aim.y - 0.4) < 1e-9);
  b.ingest(frame({ x: Infinity, y: 0 }, { isSim: true }));
  assert.ok(finite(b.aim));
  // Switching sources clears the headset's screen point.
  b.screenAim = { x: 0.1, y: 0.1 };
  b.resetTelemetry();
  assert.equal(b.screenAim, null);
});

test('the Rust brain receives the tilt as numbers, not tilt[0] of an object (NaN) (#221)', () => {
  const calls = [];
  class FakeRustBrain {
    ingest(...args) { calls.push(args); }
    set_cue() {}
  }
  setRustBrain(FakeRustBrain);
  try {
    const b = new Brain({ update() {} });
    b.ingest(frame({ x: 0.25, y: -0.5 }));
    b.ingest(frame(null));
    assert.equal(calls.length, 2);
    assert.deepEqual(calls[0].slice(7, 9), [0.25, -0.5]);
    assert.deepEqual(calls[1].slice(7, 9), [0, 0]);
    for (const args of calls) assert.ok(args.every((v) => typeof v === 'boolean' || Number.isFinite(v)), `finite args: ${args}`);
  } finally {
    setRustBrain(null);
  }
});
