// #230: seam-safe projection, minimap window split and the 3D relief rim (civ sandbox).
import test from 'node:test';
import assert from 'node:assert/strict';
import { wrapFrom } from '../src/civ/civ.js';
import { seamSpans, fadeReliefRim, RELIEF_SEA } from '../src/civ/seams.js';

test('wrapFrom takes the short way round', () => {
  assert.equal(wrapFrom(10, 630, 640), -20);
  assert.equal(wrapFrom(630, 10, 640), 20);
  assert.equal(wrapFrom(0, 320, 640), 320);
});

test('everything in the view lands on screen on a 320-high planet', () => {
  // Civ.toScreen's formula, without the wasm: the short way round from the view centre.
  const toScreen = (camX, camY, x, y, pw, ph) => ({ x: wrapFrom(camX + 160, x, pw) + 160, y: wrapFrom(camY + 90, y, ph) + 90 });
  for (const [cx, cy] of [[0, 0], [630, 300], [100, 150]]) {
    for (let sy = 0; sy < 180; sy += 3) {
      const py = (cy + sy + 0.5) % 320, px = (cx + 7.5) % 640;
      const s = toScreen(cx, cy, px, py, 640, 320);
      assert.ok(Math.abs(s.y - (sy + 0.5)) < 1e-6, `row ${sy} from cam y ${cy} came back at ${s.y}`);
      assert.ok(Math.abs(s.x - 7.5) < 1e-6);
    }
  }
});

test('the minimap window splits cleanly at either seam', () => {
  assert.deepEqual(seamSpans(10, 20, 64), [[10, 20]]);
  assert.deepEqual(seamSpans(60, 20, 64), [[60, 4], [0, 16]]);
  assert.deepEqual(seamSpans(-4, 20, 64), [[60, 4], [0, 16]]);
  for (const [a, w] of seamSpans(63.5, 10, 64)) assert.ok(a >= 0 && a < 64 && w > 0);
});

test('the 3D relief rim fades to the sea and leaves the middle alone', () => {
  const W = 320, H = 180, h = new Float32Array(W * H).fill(0.8);
  fadeReliefRim(h, W, H);
  const sea = (v) => Math.abs(v - RELIEF_SEA) < 1e-6;
  for (let x = 0; x < W; x++) assert.ok(sea(h[x]) && sea(h[(H - 1) * W + x]), `rim at column ${x}`);
  for (let y = 0; y < H; y++) assert.ok(sea(h[y * W]) && sea(h[y * W + W - 1]), `rim at row ${y}`);
  assert.ok(Math.abs(h[90 * W + 160] - 0.8) < 1e-6);
});
