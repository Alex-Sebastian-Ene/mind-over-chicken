// Forest fires burn out by themselves, and lighting the campfire never sets the forest alight.
import test from 'node:test';
import assert from 'node:assert/strict';
import { FOREST_FIRE, forestFire, forestFireStep } from '../src/eras/forest-fire.js';

// A tiny seeded random so the run is the same every time.
const seeded = (s) => () => ((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32);

// The Stone Age's own spread rule on a thick 30×30 forest, 6 px apart: light the middle tree.
function burn(seed) {
  const rnd = seeded(seed);
  let trees = [];
  for (let i = 0; i < 30; i++) for (let j = 0; j < 30; j++) trees.push({ x: i * 6, y: j * 6 });
  const take = (x, y) => {
    let best = null, bestD = 16;
    for (const t of trees) { const d = Math.hypot(t.x - x, t.y - y); if (d <= bestD) { bestD = d; best = t; } }
    if (best) trees = trees.filter((t) => t !== best);
    return best;
  };
  const first = take(90, 90);
  let fires = [forestFire(first.x, first.y, 0, rnd)], burned = 1, t = 0;
  while (fires.length && t < 300) {
    for (const f of [...fires]) {
      if (forestFireStep(f, 0.1, rnd) && fires.length < 40) {
        const n = take(f.x, f.y);
        if (n) { fires.push(forestFire(n.x, n.y, f.gen + 1, rnd)); burned++; }
      }
    }
    fires = fires.filter((f) => f.heat > 0);
    t += 0.1;
  }
  return { t, burned };
}

test('a forest fire in thick forest burns out by itself within a minute', () => {
  for (const seed of [1, 2, 3, 4, 5]) {
    const { t, burned } = burn(seed);
    assert.ok(t < 60, `seed ${seed}: still burning after ${t.toFixed(1)} s`);
    assert.ok(burned > 1 && burned < 40, `seed ${seed}: ${burned} trees burned`);
  }
});

test('a burning tree fades over its life and stops spreading past the last generation', () => {
  const f = forestFire(0, 0, FOREST_FIRE.maxGen, () => 0);
  let spread = 0;
  for (let i = 0; i < 200; i++) if (forestFireStep(f, 0.1, () => 0)) spread++;
  assert.equal(spread, 0);
  assert.equal(f.heat, 0);
});

test('the Stone Age does not light trees with a strike that lit the campfire', async () => {
  const { readFile } = await import('node:fs/promises');
  const src = await readFile(new URL('../src/eras/stone.js', import.meta.url), 'utf8');
  assert.match(src, /const tree = atPit \? null : this\.nearestTree\(x, y, 9\);/);
});
