// The WebGL HUD mirrors the JS scene on screen (src/ui/hud-mirror.js, #191). The Rust side is
// tested in crates/engine/src/eras/game.rs (mirrored_hud_shows_the_js_scene_...).
import test from 'node:test';
import assert from 'node:assert/strict';
import { objectiveRows, mirrorHud } from '../src/ui/hud-mirror.js';

test('objective rows carry label, value and done/bad/plain flags', () => {
  const scene = { objectives: () => [
    { label: 'Fire', value: 'lit', done: true },
    { label: 'Tribe', value: '4/15', bad: true },
    { label: 'Food', value: 'hunt chicken', plain: true },
    { label: 'Continents', value: '1/5', done: true, bad: true, plain: true },
  ] };
  const r = objectiveRows(scene);
  assert.deepEqual(r.labels, ['Fire', 'Tribe', 'Food', 'Continents']);
  assert.deepEqual(r.values, ['lit', '4/15', 'hunt chicken', '1/5']);
  assert.deepEqual([...r.flags], [1, 2, 4, 7]);
  assert.ok(r.flags instanceof Uint8Array);
  assert.deepEqual(objectiveRows(null), { labels: [], values: [], flags: new Uint8Array(0) });
  assert.deepEqual(objectiveRows({ objectives: () => null }).labels, []);
});

test('mirrorHud hands the visible Stone Age to the engine, with the JS clock', async () => {
  const { StoneAge } = await import('../src/eras/stone.js');
  // The scene as enter() leaves it on a 640×360 world with five continents, fire just lit.
  const s = new StoneAge();
  s.G = { assistMode: false, world: { continents: [{}, {}, {}, {}, {}] } };
  s.fire = { lit: true }; s.food = 0; s.meat = []; s.hold = { successes: 0 }; s.colonies = {};
  s.people = Array.from({ length: 8 }, () => ({ alive: true }));
  const calls = [];
  const engine = { game_mirror: (...a) => calls.push(a) };
  assert.equal(mirrorHud(engine, 42.5, s), true);
  const [runTime, labels, values, flags] = calls[0];
  assert.equal(runTime, 42.5);
  assert.deepEqual(labels.map((l, i) => `${l} ${values[i]}`), ['Fire lit', 'Tribe 8/15', 'Calm holds 0/2', 'Food hunt chicken', 'Continents 1/5']);
  assert.equal(flags[0] & 1, 1, 'fire lit is done');
  assert.equal(mirrorHud({}, 1, s), false, 'an engine without game_mirror (old pkg) is left alone');
  assert.equal(mirrorHud(null, 1, s), false);
});
