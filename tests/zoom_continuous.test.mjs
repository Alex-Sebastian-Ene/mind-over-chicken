// #143: wheel and pinch move the zoom continuously (no 320 ms gate, no level-per-notch steps),
// the nearest level becomes the target, and the spring settles there once the input stops.
import test from 'node:test';
import assert from 'node:assert/strict';
import { ZoomState } from '../src/cosmos/zoom.js';

class Landable extends ZoomState { canLand() { return true; } }
const run = (z, secs) => { for (let t = 0; t < secs; t += 1 / 60) z.update(1 / 60); };

test('small nudges move the zoom smoothly, with no gate between them', () => {
  const z = new Landable();
  for (let i = 0; i < 10; i++) z.nudge(0.025);        // ten trackpad ticks, back to back
  assert.ok(Math.abs(z.z - 0.25) < 1e-9, `z=${z.z}`);
  assert.equal(z.target, 0);                         // still nearest the surface
});

test('the zoom holds where the player put it while they scroll, then settles on the nearest level', () => {
  const z = new Landable();
  z.nudge(0.7);
  assert.equal(z.target, 1);                         // passed the midpoint: the globe is the target
  z.update(1 / 60);
  assert.ok(Math.abs(z.z - 0.7) < 1e-9, 'held during the input');
  run(z, 2);
  assert.equal(z.z, 1);
  z.nudge(-0.3);                                     // back towards the surface, but not past half
  run(z, 2);
  assert.equal(z.z, 1);                              // settles back on the globe
});

test('continuous zoom respects the levels it may reach', () => {
  const z = new ZoomState();
  z.sandbox = true;                                  // a sandbox home with no surface yet: the globe is the floor
  z.z = 1; z.target = 1;
  z.nudge(-0.8);
  assert.ok(z.z >= 1, `z=${z.z}`);
  z.nudge(5);
  assert.equal(z.z, 2);
  assert.equal(z.target, 2);
});
