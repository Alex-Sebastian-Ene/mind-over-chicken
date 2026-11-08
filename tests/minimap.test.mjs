// The home minimap's window in 3D (src/ui/minimap.js, release QA): it used to stay the 2D
// 320×180 rectangle whatever the 3D camera's zoom, so zoomed in it was twice the size of what
// the screen showed. It is now the ground under the screen's corners.
import test from 'node:test';
import assert from 'node:assert/strict';
import { footprint3d, minimapWindow3d, viewWindow } from '../src/ui/minimap.js';

const opts = { tx: 100, tz: 100, ww: 640, wh: 360, sw: 960, sh: 540 };
// A straight-down camera that sees `span` world px across, wrapped into the world like Rust does.
const topDown = (span, { tx, tz, ww, wh, sw, sh } = opts) => (x, y) => [
  (((tx + (x / sw - 0.5) * span) % ww) + ww) % ww,
  (((tz + (y / sh - 0.5) * span * (sh / sw)) % wh) + wh) % wh,
];

test('the window shrinks as the 3D camera zooms in', () => {
  const near = footprint3d(topDown(160), opts);
  const far = footprint3d(topDown(480), opts);
  const width = (p) => p[1][0] - p[0][0];
  assert.equal(Math.round(width(near)), 160);
  assert.equal(Math.round(width(far)), 480);
});

test('corners across a seam stay next to the camera instead of jumping a world away', () => {
  const o = { ...opts, tx: 10, tz: 350 };
  const p = footprint3d(topDown(160, o), o);
  assert.ok(p[0][0] < 0, 'the left edge sits just west of x = 0');
  assert.ok(p[2][1] > 360, 'the bottom edge sits just south of the last row');
  for (const [x, y] of p) {
    assert.ok(Math.abs(x - o.tx) <= 80 + 1e-6);
    assert.ok(Math.abs(y - o.tz) <= 45 + 1e-6);
  }
});

test('a top corner over the horizon walks down until it meets the ground', () => {
  const ground = topDown(160);
  const tilted = (x, y) => (y < 100 ? [-1, -1] : ground(x, y));
  const p = footprint3d(tilted, opts);
  assert.ok(p, 'still a window');
  assert.ok(p[0][1] > footprint3d(ground, opts)[0][1], 'its top edge is lower than the untilted one');
});

test('no ground in view: no 3D window (the caller falls back to the 2D rectangle)', () => {
  assert.equal(footprint3d(() => null, opts), null);
});


test('off-map 3D footprints are rejected so the minimap can fall back', () => {
  assert.equal(minimapWindow3d(topDown(160), opts)?.length, 4);
  assert.equal(minimapWindow3d(topDown(900), opts), null, 'a quad wider than the world wraps badly');
  // footprint3d asks for top-left, top-right, bottom-right, bottom-left.
  const bow = [[60, 60], [580, 60], [60, 300], [580, 300]];
  let n = 0;
  const crossed = () => bow[n++ % bow.length];
  assert.equal(minimapWindow3d(crossed, opts), null, 'a self-intersecting quad is not a window');
});

test('the fallback camera box stays inside the minimap world', () => {
  assert.deepEqual(viewWindow(320, 180, 640, 360), { x: 160, y: 90, w: 320, h: 180 });
  assert.deepEqual(viewWindow(10, 10, 640, 360), { x: 0, y: 0, w: 320, h: 180 });
  assert.deepEqual(viewWindow(630, 350, 640, 360), { x: 320, y: 180, w: 320, h: 180 });
  assert.deepEqual(viewWindow(90, 50, 200, 100), { x: 0, y: 0, w: 200, h: 100 });
});
