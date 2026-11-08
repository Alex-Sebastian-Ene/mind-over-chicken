// Lightning that smites the last villager fails the era through G (src/eras/stone.js). It called
// a nonexistent this.fail() and threw, from the campaign and from an AI god's SimPlanet
// (src/ai/planet.js act() → scene.onBlink).
import test from 'node:test';
import assert from 'node:assert/strict';
import '../rl/headless.mjs';   // canvas stand-in (World.paint) and the engine wasm
import { SimPlanet } from '../src/ai/planet.js';

test('smiting the whole tribe fails the era instead of throwing', () => {
  const planet = new SimPlanet({ seed: 7, brain: 'proxy' });
  const s = planet.scene;
  const last = s.people[0];
  for (const p of s.people.slice(1)) p.alive = false;
  s.t = 10; s.lastBolt = -5;
  const w = planet.world;
  const aim = { x: last.x / w.width, y: last.y / w.height };
  assert.doesNotThrow(() => s.onBlink(aim));
  assert.equal(s.living().length, 0);
  assert.equal(s.done, true);
  assert.equal(planet.outcome, 'lose');
});
