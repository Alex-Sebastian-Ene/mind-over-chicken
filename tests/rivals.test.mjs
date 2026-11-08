// #212 / epic #220: rival civilisations on the home world, through the JS glue (src/civ/rivals.js)
// and the real moc_civ.wasm, on a small made-up world of four islands.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Rivals, RIVAL, playerState, applyRivalLine } from '../src/civ/rivals.js';

const T = { DEEP: 0, SHALLOW: 1, SAND: 2, GRASS: 3, FOREST: 4, HILL: 5 };

function islands(width = 320, height = 180) {
  const type = new Uint8Array(width * height);
  const h = new Float32Array(width * height);
  const continents = [
    { id: 'home', cx: 70, cy: 50, rx: 45, ry: 30 },
    { id: 'east', cx: 230, cy: 45, rx: 50, ry: 28 },
    { id: 'south', cx: 90, cy: 135, rx: 48, ry: 30 },
    { id: 'far', cx: 245, cy: 130, rx: 45, ry: 32 },
  ].map((c) => ({ ...c, village: { x: c.cx, y: c.cy }, area: Math.round(Math.PI * c.rx * c.ry) }));
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      let best = 9;
      for (const c of continents) best = Math.min(best, ((x - c.cx) / c.rx) ** 2 + ((y - c.cy) / c.ry) ** 2);
      type[i] = best < 0.6 ? ((x * 7 + y * 3) % 11 === 0 ? T.FOREST : T.GRASS) : best < 0.85 ? (best < 0.75 ? T.GRASS : T.SAND) : best < 1.2 ? T.SHALLOW : T.DEEP;
      h[i] = best < 0.85 ? 0.45 + 0.2 * (1 - best) : 0.1;
    }
  }
  return { width, height, type, baseType: type, h, continents, seed: 11, sites: { village: continents[0].village } };
}

const wasm = () => new WebAssembly.Instance(new WebAssembly.Module(readFileSync(new URL('../src/civ/moc_civ.wasm', import.meta.url))), {}).exports;

test('rivals settle the other islands and live, with names, villages and people', () => {
  const world = islands();
  const r = new Rivals(wasm(), world, { rivals: 3, council: 'rules' });
  r.setScene(0);
  const scene = { fire: { x: 70, y: 50 }, colonies: {}, boats: [], food: 4, living: () => new Array(12).fill({}) };
  const lines = [];
  for (let k = 0; k < 900; k++) lines.push(...r.update(0.2, playerState(scene, world, 0)));
  assert.ok(r.total >= 1, `rivals placed: ${r.total}`);
  assert.ok(r.alive() >= 1, 'a rival people is alive');
  assert.ok(r.villages.length / r.villageStride >= 1, 'they have villages');
  const owners = new Set();
  for (let i = 0; i < r.villages.length; i += r.villageStride) owners.add(r.villages[i + 2]);
  owners.delete(r.mirror);
  assert.ok(owners.size >= 1, 'villages belong to rivals, not only to you');
  const f = [...owners][0];
  assert.ok(r.name(f).length > 1, 'a rival has a name');
  assert.match(r.label(f), /·/, 'its hover label names its age and stance');
  assert.ok(r.agents.length > 0, 'rival people walk the map');
  const terr = r.territory(10);
  assert.ok(terr && terr.w === 32 && terr.data.length === terr.w * terr.h * 4, 'territory paints for the minimap');
  assert.ok(lines.every((l) => Number.isFinite(l.kind) && typeof l.text === 'string'), 'lines decode');
  assert.ok(lines.some((l) => l.kind === RIVAL.ARRIVE || l.text.length), 'the rivals make news');
  // The rival sim stays inside the frame budget (#212: ≈2 ms at ×1; a 5 Hz step here).
  assert.ok(r.perf.avg < 8, `avg step ${r.perf.avg.toFixed(2)} ms`);
  r.free();
});

test('your people in the Stone Age: camp, colonies and boats; later every continent', () => {
  const world = islands();
  const scene = { fire: { x: 70, y: 50 }, colonies: { east: { x: 230, y: 45 } }, boats: [{ x: 150, y: 50 }], food: 3, living: () => [1, 2, 3] };
  const st = playerState(scene, world, 0);
  assert.deepEqual(st.sites, [{ x: 70, y: 50 }, { x: 230, y: 45 }]);
  assert.deepEqual(st.boats, [{ x: 150, y: 50 }]);
  assert.equal(st.pop, 3);
  assert.equal(st.food, 3);
  const later = playerState({ progress: 0.4, phase: 'build' }, world, 4);
  assert.equal(later.sites.length, 4, 'camp plus the three other continents');
  assert.equal(later.launch, 0.4);
  assert.equal(later.launched, false);
});

test('a raid takes food and people (never the last three); trade brings food; wars move the chicken market', () => {
  const people = Array.from({ length: 5 }, (_, i) => ({ alive: true, x: 10 * i, y: 0 }));
  const scene = { food: 2, living: () => people.filter((p) => p.alive), kill: (p) => { p.alive = false; } };
  const said = [];
  const advisor = { say: (k, t) => said.push(k) };
  let d = applyRivalLine({ kind: RIVAL.RAID, tone: 2, food: -3, hurt: 4, x: 0, y: 0, text: 'Raid!', say: 'They took the meat.', key: 'rival-raid' }, { scene, advisor });
  assert.equal(scene.food, 0, 'food never goes below zero');
  assert.equal(d.hurt, 2, 'only two of five are hurt: three always survive');
  assert.ok(!people[0].alive && !people[1].alive && people[4].alive, 'the nearest to the raid fall');
  assert.deepEqual(said, ['rival-raid']);
  d = applyRivalLine({ kind: RIVAL.TRADE, tone: 1, food: 1.5, hurt: 0, text: 'Trade' }, { scene });
  assert.equal(scene.food, 1.5);
  const market = { price: 100, headlines: [] };
  applyRivalLine({ kind: RIVAL.WAR, tone: 2, market: -0.1, text: 'War in the east' }, { scene: market });
  assert.equal(Math.round(market.price), 90);
  assert.deepEqual(market.headlines, ['WAR IN THE EAST']);
});
