// Navigation UX (#284): pinch drag rotation matches the pointer, and blink clicks select/land.
// Run: node --test tests/navigation_ux.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { Cosmos } from '../src/cosmos/cosmos.js';

const fakeCosmos = (z = 2) => {
  const c = Object.create(Cosmos.prototype);
  c.z = z;
  c.rot = 0;
  c.spinVel = 0;
  c.say = () => {};
  c.focusPlanet = (p) => { c.selected = p; };
  c.zoomIn = () => { c.zoomed = true; };
  c.canLand = () => true;
  c.world = () => ({ name: 'Home' });
  return c;
};

test('pinch drag rotates the globe by the same formula as a pointer drag', () => {
  const c = fakeCosmos(1);
  assert.equal(c.dragRotate(74), true);
  assert.equal(c.rot, -1);
  assert.ok(Math.abs(c.spinVel + 1 / 0.033) < 1e-9);
  c.z = 2;
  assert.equal(c.dragRotate(10), false, 'galaxy has no drag rotation');
});

test('blink click in the galaxy selects the planet under the reticle', () => {
  const c = fakeCosmos(2);
  const planet = { name: 'Lumen' };
  c.galaxy = { hit: (x, y) => (x === 10 && y === 20 ? planet : null) };
  assert.equal(c.clickAt(10, 20, {}), planet);
  assert.equal(c.selected, planet);
  c.selected = null;
  assert.equal(c.clickAt(1, 2, {}), null);
});

test('blink click on the globe lands inside the globe and ignores the rim outside', () => {
  const c = fakeCosmos(1);
  const home = { name: 'Home' };
  c.world = () => home;
  assert.equal(c.clickAt(160, 90, {}), home);
  assert.equal(c.zoomed, true);
  c.zoomed = false;
  assert.equal(c.clickAt(0, 0, {}), null);
  assert.equal(c.zoomed, false);
});
