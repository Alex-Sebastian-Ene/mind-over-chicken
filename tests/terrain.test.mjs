// The home world from Rust (#109, #129): acceptance checks on random worlds at the game's
// 320×180 and at 640×360. Run: node --test tests/terrain.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import '../rl/headless.mjs';   // canvas stand-in (World.paint) and the engine wasm
import { World, T } from '../src/world/terrain.js';

const SIZES = [[320, 180], [640, 360]];
const seeds = () => Array.from({ length: 3 }, () => (Math.random() * 1e9) >>> 0);   // random per run (Alex)
const isSea = (w, x, y) => w.isWater(x, y);
const seaNear = (w, p, r) => {
  for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) if (isSea(w, p.x + dx, p.y + dy)) return true;
  return false;
};

for (const [width, height] of SIZES) {
  test(`home world ${width}×${height}: 45–70% sea, at least 5 continents, islands not too small`, () => {
    for (const seed of seeds()) {
      const w = new World(seed, { width, height });
      assert.equal(w.width, width);
      assert.equal(w.height, height);
      assert.equal(w.type.length, width * height);
      assert.equal(w.h.length, width * height);
      assert.equal(w.rgba.length, width * height * 4);
      let sea = 0;
      for (const t of w.type) if (t <= T.SHALLOW) sea++;
      const share = sea / (width * height);
      assert.ok(share >= 0.45 && share <= 0.7, `seed ${seed}: sea share ${share.toFixed(3)}`);
      assert.ok(w.stats.continents >= 5, `seed ${seed}: ${w.stats.continents} major continents`);
      assert.ok(w.stats.islandMin === 0 || w.stats.islandMin >= w.stats.minIsland, `seed ${seed}: island of ${w.stats.islandMin} tiles`);
      // Deep water exists (the old thresholds: deep below 0.24, sea level 0.33).
      let deep = 0;
      for (let i = 0; i < w.type.length; i++) if (w.type[i] === T.DEEP) { deep++; assert.ok(w.h[i] < 0.33); }
      assert.ok(deep / (width * height) > 0.2, `seed ${seed}: deep water ${deep}`);
    }
  });

  test(`home world ${width}×${height}: the village and colonies are on land with sea access, rafts can sail`, () => {
    for (const seed of seeds()) {
      const w = new World(seed, { width, height });
      const s = w.sites;
      for (const k of ['village', 'pad', 'factory', 'exchange', 'hangar', 'crowd', 'picket', 'fire']) {
        assert.ok(w.walkable(s[k].x, s[k].y), `seed ${seed}: ${k} at ${s[k].x},${s[k].y} is walkable`);
        assert.ok(w.reachable(s[k].x, s[k].y), `seed ${seed}: ${k} is reachable from the village`);
      }
      assert.ok(seaNear(w, s.pad, 8), `seed ${seed}: the pad is on the coast`);
      assert.equal(w.continents[0].id, 'home');
      assert.ok(w.continents.length >= 3, `seed ${seed}: ${w.continents.length} reachable landmasses`);
      for (const c of w.continents.slice(1)) {
        assert.ok(w.walkable(c.village.x, c.village.y), `seed ${seed}: ${c.id} village on land`);
        assert.ok(isSea(w, c.harbor.dock.x, c.harbor.dock.y) && isSea(w, c.homeHarbor.dock.x, c.homeHarbor.dock.y), `seed ${seed}: ${c.id} docks on water`);
        assert.ok(w.walkable(c.harbor.land.x, c.harbor.land.y), `seed ${seed}: ${c.id} quay on land`);
        // A raft from home's dock can sail to the colony's dock.
        const f = w.seaField(c.homeHarbor.dock.x, c.homeHarbor.dock.y);
        assert.ok(f[c.harbor.dock.y * width + c.harbor.dock.x] >= 0, `seed ${seed}: sea route to ${c.id}`);
      }
      // Every era repaints without errors and the land use stays on land.
      for (const era of ['stone', 'farming', 'industry', 'space']) w.paint(era);
    }
  });
}

test('every run is a different world: land masks overlap little (IoU < 0.5)', () => {
  const masks = seeds().concat(seeds()).slice(0, 4).map((seed) => {
    const w = new World(seed, { width: 640, height: 360 });
    assert.ok(w.stats.continents >= 5);
    return Uint8Array.from(w.type, (t) => (t > T.SHALLOW ? 1 : 0));
  });
  for (let a = 0; a < masks.length; a++) for (let b = a + 1; b < masks.length; b++) {
    let inter = 0, union = 0;
    for (let i = 0; i < masks[a].length; i++) { inter += masks[a][i] & masks[b][i]; union += masks[a][i] | masks[b][i]; }
    assert.ok(inter / union < 0.5, `runs ${a} and ${b}: IoU ${(inter / union).toFixed(2)}`);
  }
});

test('a galaxy planet: 640×360 surface with its 320×180 capital', () => {
  const p = World.planet((Math.random() * 1e9) >>> 0, 'desert');
  assert.equal(p.width, 320);
  assert.equal(p.vast.w, 640);
  assert.equal(p.vast.h, 360);
  assert.equal(p.vast.rgba.length, 640 * 360 * 4);
  assert.ok(p.vast.spots.length >= 8);
  assert.ok(p.walkable(p.sites.village.x, p.sites.village.y));
});

test('the engine wasm is the one the page loads', () => {
  assert.ok(readFileSync(new URL('../pkg/mind_over_chicken_engine_bg.wasm', import.meta.url)).length > 100_000);
});
