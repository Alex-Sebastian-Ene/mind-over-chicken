// Visible aim boundary (#294): screen/world clamps and view classification.
// Run: node --test tests/visible_aim.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  AIM_VIEW, aimViewKind, clampHomeAim, clampScreenAim, HOME_VIEWS, SCREEN_VIEWS,
} from '../src/input/visible-aim.js';

const view = (over = {}) => ({ left: 160, top: 90, viewW: 320, viewH: 180, worldW: 640, worldH: 360, ...over });

test('aimViewKind names the space used by each view, cosmos first', () => {
  assert.equal(aimViewKind({ hasScene: false }), AIM_VIEW.NONE);
  assert.equal(aimViewKind({}), AIM_VIEW.HOME_2D);
  assert.equal(aimViewKind({ is3d: true }), AIM_VIEW.HOME_3D);
  assert.equal(aimViewKind({ civ: true }), AIM_VIEW.CIV);
  assert.equal(aimViewKind({ surface: true }), AIM_VIEW.SURFACE);
  assert.equal(aimViewKind({ cosmos: true, surface: true, civ: true, is3d: true }), AIM_VIEW.COSMOS);
  assert.ok(HOME_VIEWS.has(AIM_VIEW.HOME_2D) && HOME_VIEWS.has(AIM_VIEW.HOME_3D));
  assert.ok(SCREEN_VIEWS.has(AIM_VIEW.SURFACE) && SCREEN_VIEWS.has(AIM_VIEW.CIV) && SCREEN_VIEWS.has(AIM_VIEW.COSMOS));
});

test('screen aim stays inside the viewport with a reticle inset', () => {
  assert.deepEqual(clampScreenAim({ x: 0.5, y: 0.5 }), { x: 0.5, y: 0.5, clamped: false });
  const left = clampScreenAim({ x: -0.2, y: 1.3 });
  assert.ok(left.clamped);
  assert.equal(left.x, 6 / 320);
  assert.equal(left.y, 1 - 6 / 180);
  // No negative inset can escape the viewport.
  const tight = clampScreenAim({ x: 0.5, y: 0.5 }, { width: 0, height: 0, inset: 100 });
  assert.ok(tight.x >= 0 && tight.x <= 1 && tight.y >= 0 && tight.y <= 1);
});

test('home aim is unchanged when it is already inside the camera window', () => {
  const p = clampHomeAim({ x: 0.5, y: 0.5 }, view());
  assert.ok(Math.abs(p.x - 0.5) < 1e-9 && Math.abs(p.y - 0.5) < 1e-9);
  assert.equal(p.clamped, false);
});

test('home aim outside the window clamps to the visible edge, not the world edge', () => {
  // Screen 320×180 is the middle half of the 640×360 world: visible x 160..479, y 90..269.
  const right = clampHomeAim({ x: 0.9, y: 0.5 }, view());
  assert.ok(right.clamped);
  assert.ok(right.x < 480 / 640, `clamped right: ${right.x}`);
  assert.ok(right.x > 470 / 640);
  const top = clampHomeAim({ x: 0.5, y: 0.01 }, view());
  assert.ok(top.clamped);
  assert.ok(top.y > 90 / 360 && top.y < 100 / 360, `clamped top: ${top.y}`);
});

test('home aim follows the window across the east and west seams', () => {
  // Window centred on world (5,3): left 485, top 273, wrapping both ways.
  const seam = view({ left: 485, top: 273 });
  const visible = clampHomeAim({ x: 0.08, y: 0.78 }, seam);
  assert.ok(!visible.clamped, `point is visible across the seam: ${JSON.stringify(visible)}`);
  assert.ok(visible.x < 0.1 || visible.x > 0.7);
  const outside = clampHomeAim({ x: 0.5, y: 0.5 }, seam);
  assert.ok(outside.clamped);
  assert.ok(outside.x < 0.27 || outside.x > 0.73, `clamped to a visible edge copy: ${outside.x}`);
  assert.ok(Number.isFinite(outside.y));
});

test('home aim handles a world smaller than the view and bad input', () => {
  const small = view({ left: 0, top: 0, viewW: 320, viewH: 180, worldW: 200, worldH: 100 });
  const p = clampHomeAim({ x: 0.5, y: 0.5 }, small);
  assert.ok(Number.isFinite(p.x) && Number.isFinite(p.y));
  const bad = clampHomeAim({ x: NaN, y: Infinity }, view());
  assert.ok(Number.isFinite(bad.x) && Number.isFinite(bad.y));
});
