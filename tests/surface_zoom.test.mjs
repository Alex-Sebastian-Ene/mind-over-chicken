// Bigger home worlds let the 3D camera pull back further before the globe takes over (Alex: "for
// larger maps you should be able to zoom more before you go planetary").
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { surfaceMaxDistance, TOP_DOWN } from '../src/render/surface-zoom.js';
import { WORLD_SIZES } from '../src/game/options.js';

test('the hand-over distance grows with the world: medium unchanged, huge twice as far', () => {
  const by = Object.fromEntries(WORLD_SIZES.map((s) => [s.id, surfaceMaxDistance(s.w)]));
  assert.equal(by.medium, TOP_DOWN);
  assert.equal(by.small, TOP_DOWN, 'small worlds never hand over sooner than medium');
  assert.ok(by.large > by.medium && by.huge > by.large);
  assert.equal(by.huge, 2 * TOP_DOWN);
  assert.equal(surfaceMaxDistance(undefined), TOP_DOWN);
  assert.equal(surfaceMaxDistance(99999), 2 * TOP_DOWN, 'capped');
});

test('the engine lets the camera reach the widest hand-over, and main.js hands over there', async () => {
  const rs = await readFile(new URL('../crates/engine/src/camera3d.rs', import.meta.url), 'utf8');
  const max = Number(/pub const MAX_DISTANCE: f32 = ([\d.]+);/.exec(rs)[1]);
  assert.ok(max >= 2 * TOP_DOWN, `engine MAX_DISTANCE ${max}`);
  const main = await readFile(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.match(main, /get_camera_distance\(\) >= camMax\(\) - 0\.5 && !\(scene instanceof CivSandbox\)/);
  assert.match(main, /Math\.min\(camMax\(\), d \* f\)/);
});
