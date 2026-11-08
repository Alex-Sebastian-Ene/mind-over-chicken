// render perf (#214): the civ mesh key quantizes the camera to a pan cell, so a 1-pixel pan
// keeps the existing 3D terrain mesh instead of rebuilding and re-uploading it every frame.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { CIV_MESH_PAN_STRIDE, panBucket, civMeshKey } from '../src/render/render-perf.js';

test('the pan cell is stable for every 1-pixel move inside it', () => {
  const base = 96; // 96..103 share cell 12 at the default 8-px stride
  const key = civMeshKey(7, base, base);
  for (let d = 0; d < CIV_MESH_PAN_STRIDE; d++) {
    assert.equal(civMeshKey(7, base + 0.9, base + 0.9), key);
    assert.equal(civMeshKey(7, base + d, base + d), key);
  }
  assert.notEqual(civMeshKey(7, base + CIV_MESH_PAN_STRIDE, base), key);
});

test('crossing a cell boundary changes the key exactly once', () => {
  const a = civMeshKey(7, CIV_MESH_PAN_STRIDE - 0.01, 0);
  const b = civMeshKey(7, CIV_MESH_PAN_STRIDE, 0);
  assert.notEqual(a, b);
  assert.equal(civMeshKey(7, CIV_MESH_PAN_STRIDE + 0.5, 0), b, 'the next cell is stable too');
});

test('the key still separates worlds and both axes', () => {
  assert.notEqual(civMeshKey(1, 0, 0), civMeshKey(2, 0, 0));
  assert.notEqual(civMeshKey(1, 0, 0), civMeshKey(1, CIV_MESH_PAN_STRIDE, 0));
  assert.notEqual(civMeshKey(1, 0, 0), civMeshKey(1, 0, CIV_MESH_PAN_STRIDE));
});

test('bad camera values and bad strides never throw or alias the wrong cell', () => {
  assert.equal(panBucket(NaN), 0);
  assert.equal(panBucket(Infinity), 0);
  assert.equal(panBucket(-0.001), -1);
  assert.equal(panBucket(5, 0), panBucket(5));
  assert.equal(panBucket(5, -3), panBucket(5));
  assert.equal(civMeshKey(undefined, undefined, undefined), 'civ_0_0_0');
});

test('a 128-px pan rebuilds 17 times instead of 129 (one per occupied 8-px cell)', () => {
  const seen = new Set();
  const naive = new Set();
  for (let x = 0; x <= 128; x++) {
    seen.add(civMeshKey(1, x, 0));
    naive.add(civMeshKey(1, x, 0, 1)); // stride 1 = the old Math.round(cam) behaviour
  }
  assert.equal(seen.size, 17);
  assert.equal(naive.size, 129);
});

test('main.js routes every 3D world through the engine mesh cache and the quantized civ key', async () => {
  const main = await readFile(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.match(main, /import \{ civMeshKey \} from '\.\/render\/render-perf\.js'/);
  assert.match(main, /const key = civMeshKey\(scene\.seed \|\| 0, scene\.civ\?\.camX \|\| 0, scene\.civ\?\.camY \|\| 0\)/);
  assert.doesNotMatch(main, /civ_\$\{scene\.seed \|\| 0\}_\$\{Math\.round\(scene\.civ\?\.camX/,
    'the per-pixel Math.round civ key is gone');
  assert.match(main, /engine\.mesh_needed\(key\)/);
  assert.match(main, /renderStats\(\)/);
});
