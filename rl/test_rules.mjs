// Checks the two rules for the AI god, against the real sim:
//  1. Simulation only: the AI drives its own SimPlanet and its own brain. It never writes
//     the human's headset inputs, never refuses a human blink, and the human's calm still
//     counts on a world they watch.
//  2. Planetary: distances and aims wrap east-west; the observation covers colonies/rafts.
//
//   node rl/test_rules.mjs        (exit code 0 = pass)

import { mulberry32 } from './headless.mjs';
import assert from 'node:assert/strict';
import { SimPlanet } from '../src/ai/planet.js';
import { AIGod, autopilotStep } from '../src/ai/autopilot.js';
import { Brain } from '../src/brain/brain.js';
import { SimSource } from '../src/brain/sources.js';
import { observe, aimAt, planetDist, wrapDx, OBS_NAMES, OBS_SIZE, scriptedAction } from '../src/ai/observe.js';
import { W, H } from '../src/sim/util.js';

Math.random = mulberry32(12345);
const results = [];
const test = (name, fn) => { fn(); results.push(name); };

test('AI calm goes to the planet\'s own brain, never to the human headset', () => {
  const humanSource = new SimSource();
  const human = new Brain(humanSource);
  const planet = new SimPlanet({ seed: 3 }).attachHuman(human);
  const before = JSON.stringify(humanSource.input);
  for (let i = 0; i < 40; i++) { planet.act(3); planet.advance(0.25); }
  assert.equal(JSON.stringify(humanSource.input), before);
  assert.equal(planet.aiInput.calm, true);
  assert.notEqual(planet.aiInput, humanSource.input);
});

test('the human\'s calm counts on top of the AI\'s', () => {
  const human = { calm: 0.97, focus: 0, aim: { x: 0.5, y: 0.5 }, anger: 0 };
  const planet = new SimPlanet({ seed: 4, brain: 'proxy' }).attachHuman(human);
  assert.ok(planet.G.brain.calm >= 0.97);
  human.calm = 0;
  for (let i = 0; i < 40; i++) { planet.act(3); planet.advance(0.25); }
  assert.ok(planet.G.brain.calm > 0.6, 'AI calm should still apply');
});

test('AI lightning never refuses the human\'s next blink', () => {
  const human = { calm: 0, focus: 0, aim: { x: 0.5, y: 0.5 }, anger: 0 };
  const planet = new SimPlanet({ seed: 5, brain: 'proxy' }).attachHuman(human);
  planet.advance(1);
  const s = planet.scene;
  planet.act(1);                              // AI strikes the fire pit
  assert.equal(s.strikes, 1);
  planet.humanBlink({ x: 0.2, y: 0.2 });      // human blinks immediately after
  assert.equal(s.strikes, 2, 'human blink must go through');
  planet.act(4);                              // AI's own cooldown still holds
  assert.equal(s.strikes, 2);
});

test('the god only drives the planet it is given', () => {
  const planet = new SimPlanet({ seed: 6, brain: 'proxy' });
  const god = new AIGod({ policy: { act: (obs) => scriptedAction(obs) } });
  planet.setGod(god);
  for (let i = 0; i < 200; i++) planet.update(1 / 16);
  assert.ok(planet.scene.fire.everLit, 'the god lit its own fire');
  const step = autopilotStep(planet, null);
  assert.ok(step.name);
});

test('planet wrap: distances and aims go the short way round', () => {
  assert.equal(wrapDx(W - 2), -2);
  assert.equal(wrapDx(-(W - 3)), 3);
  assert.ok(Math.abs(planetDist(1, 50, W - 1, 50) - 2) < 1e-9);
  const a = aimAt(-1, 50);                    // one pixel west of x = 0 is x = W - 1
  assert.ok(Math.abs(a.x * W - (W - 1)) < 1e-9);   // aims are 0..1 of the world (world.aimPoint, #129)
  assert.ok(Math.abs(aimAt(W + 5, 50).x * W - 5) < 1e-9);
  assert.ok(Math.abs(aimAt(5, -1).y * H - (H - 1)) < 1e-9);   // north–south wraps too (#111)
  // On the 640×360 home world, sizes come from the world.
  assert.ok(Math.abs(planetDist(1, 2, 639, 358, 640, 360) - Math.hypot(2, 4)) < 1e-9);
});

test('observation covers the whole planet', () => {
  const planet = new SimPlanet({ seed: 7, brain: 'proxy' });
  const o = observe(planet.scene, planet.G);
  assert.equal(o.length, OBS_SIZE);
  for (const k of ['colonies', 'rafts', 'overseas']) assert.ok(OBS_NAMES.includes(k));
  assert.ok(o.every(Number.isFinite));
});

console.log(`ok: ${results.length} checks\n - ${results.join('\n - ')}`);
