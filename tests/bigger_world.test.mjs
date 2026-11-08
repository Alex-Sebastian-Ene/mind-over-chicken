// The bigger home world (#129): a 640×360 torus seen through a 320×180 camera. The World's own
// size drives lookups, fields, aims and agents, and everything wraps both ways (#111).
// Run: node --test tests/bigger_world.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import '../rl/headless.mjs';   // canvas stand-in (World.paint) and the engine wasm
import { World, T } from '../src/world/terrain.js';
import { HOME_W, HOME_H, W, H, wrap, wrapDelta } from '../src/sim/util.js';
import { walk, sail } from '../src/sim/agents.js';
import { planetDist, aimAt } from '../src/ai/observe.js';
import { SimPlanet } from '../src/ai/planet.js';

const world = new World(1234, { width: HOME_W, height: HOME_H });

test('the home world is bigger than the view and says so', () => {
  assert.equal(HOME_W, 640);
  assert.equal(HOME_H, 360);
  assert.equal(W, 320);
  assert.equal(H, 180);
  assert.equal(world.width, 640);
  assert.equal(world.height, 360);
  assert.equal(world.h.length, 640 * 360);   // world.h stays the heightmap
  assert.ok(world.continents.length >= 5, `${world.continents.length} continents`);
});

test('wrap helpers go the short way round', () => {
  assert.equal(wrap(-1, 640), 639);
  assert.equal(wrap(640, 640), 0);
  assert.equal(wrapDelta(630, 10, 640), -20);
  assert.equal(wrapDelta(5, 355, 360), 10);
  assert.equal(world.dist(1, 1, 639, 359), Math.hypot(2, 2));
  assert.equal(planetDist(1, 1, 639, 359, 640, 360), Math.hypot(2, 2));
});

test('lookups wrap both ways: one pixel past an edge is the other edge', () => {
  for (const y of [0, 100, 359]) assert.equal(world.get(-1, y), world.get(639, y));
  for (const x of [0, 320, 639]) assert.equal(world.get(x, -1), world.get(x, 359));
  assert.equal(world.get(640 + 7, 360 + 3), world.get(7, 3));
  assert.equal(world.typeAt(-5, -5), world.typeAt(635, 355));
  const p = world.nearestWalkable(-3, -3);
  assert.ok(p.x >= 0 && p.x < 640 && p.y >= 0 && p.y < 360, JSON.stringify(p));
});

test('aims are 0..1 of the world and land on the pixel they name', () => {
  for (const [x, y] of [[0, 0], [639, 359], [321, 17]]) {
    const a = world.aimOf(x, y);
    assert.deepEqual(world.aimPoint(a), { x, y });
    const b = aimAt(x, y, 640, 360);
    assert.deepEqual(world.aimPoint(b), { x, y });
  }
  // An aim past the seam wraps onto the map.
  assert.deepEqual(world.aimPoint({ x: 1, y: 1 }), { x: 0, y: 0 });
});

test('the village can walk to the pad, and rafts can reach every other continent', () => {
  const s = world.sites;
  assert.ok(world.reachable(s.pad.x, s.pad.y), 'pad reachable');
  const home = world.continents[0];
  for (const c of world.continents.slice(1)) {
    if (!c.harbor || !home) continue;
    const f = world.seaField(c.harbor.dock.x, c.harbor.dock.y);
    const from = c.homeHarbor?.dock;
    if (!from) continue;
    assert.ok(f[world.idx(from.x, from.y)] >= 0, `continent ${c.id}: sea route from home`);
  }
});

test('agents walk and sail across the seams and stay on the map', () => {
  // A strip of land across the east-west seam and of sea across the north-south one.
  // A hand-made 64×48 map (the generator needs real sizes): no generation, just tiles.
  const w = new World(5, { gen: false });
  Object.assign(w, { width: 64, height: 48, w: 64, type: new Uint8Array(64 * 48).fill(T.DEEP) });
  for (let x = 0; x < 64; x++) w.type[20 * 64 + x] = T.GRASS;
  w.fields = new Map();
  const a = { x: 60, y: 20 };
  for (let i = 0; i < 200 && !walk(a, 4, 20, 10, 0.1, w); i++);
  assert.ok(Math.abs(wrapDelta(a.x, 4, 64)) < 1.5, `walked to ${a.x}`);
  assert.ok(a.x >= 0 && a.x < 64);
  const b = { x: 10, y: 44 };
  for (let i = 0; i < 200 && !sail(b, 10, 4, 10, 0.1, w); i++);
  assert.ok(Math.abs(wrapDelta(b.y, 4, 48)) < 2, `sailed to ${b.y}`);
  assert.ok(b.y >= 0 && b.y < 48);
});

test('the AI god plays the home world at its size', () => {
  const p = new SimPlanet({ seed: 1234, brain: 'proxy', width: HOME_W, height: HOME_H });
  assert.equal(p.world.width, 640);
  for (let i = 0; i < 40; i++) p.update(1 / 16);
  for (const person of p.scene.people) assert.ok(person.x >= 0 && person.x < 640 && person.y >= 0 && person.y < 360);
});
