// Stone Age focus hold + Rain hand spell (#330).
// Run: node --test tests/stone_focus_rain.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { StoneAge, rainHandReady } from '../src/eras/stone.js';

test('the fire hold is focus/insight, not calm', () => {
  const s = new StoneAge();
  assert.match(s.intro.goal, /focus holds/i);
  assert.ok(s.intro.how.some(([k, v]) => k === 'Focus' && /Insight/i.test(v)));
  assert.ok(s.intro.how.some(([k, v]) => k === 'Rain' && /open your palm/i.test(v)));

  s.G = { assistMode: false, world: { continents: [{}, {}] } };
  s.fire = { lit: true }; s.food = 0; s.meat = []; s.hold = { successes: 0 }; s.colonies = {};
  s.people = [{ alive: true }];
  const rows = s.objectives();
  assert.deepEqual(rows.find((r) => r.label === 'Focus holds'), { label: 'Focus holds', value: '0/2', done: false });
  assert.equal(rows.some((r) => /calm/i.test(r.label)), false);
});

test('Rain needs an open palm only while the camera is on', () => {
  assert.equal(rainHandReady(null, 1_000), true, 'no camera: keyboard F fallback');
  assert.equal(rainHandReady({ active: false }, 1_000), true, 'camera off');
  assert.equal(rainHandReady({ active: true, palm: false, at: 1_000 }, 1_100), false);
  assert.equal(rainHandReady({ active: true, palm: true, at: 1_000 }, 1_200), true);
  assert.equal(rainHandReady({ active: true, palm: true, at: 1_000 }, 1_500), false, 'stale hand frame');
});

test('the Rain hold does not progress on focus alone when the palm is closed', () => {
  const s = Object.create(StoneAge.prototype);
  let started = 0;
  s.G = {
    brain: { focus: 1, threshold: () => 0.5 },
    source: { input: { focus: true } },
    handInput: { active: true, palm: false, at: performance.now() },
    advisor: { say() {} },
  };
  s.rainHold = { progress: 0, active: 0, cooldown: 0, hintAt: -99 };
  s.t = 0;
  s.startRain = () => { started++; s.rainHold.progress = 0; };
  for (let i = 0; i < 25; i++) s.updateRain(0.1);
  assert.equal(started, 0, 'focus + closed palm is not enough');

  s.G.handInput = { active: true, palm: true, at: performance.now() };
  for (let i = 0; i < 22; i++) s.updateRain(0.1);
  assert.equal(started, 1, 'open palm + focus casts Rain after ~2 s');
});

test('without a camera, focus alone still casts Rain', () => {
  const s = Object.create(StoneAge.prototype);
  let started = 0;
  s.G = {
    brain: { focus: 1, threshold: () => 0.5 },
    source: { input: { focus: true } },
    handInput: undefined,
    advisor: { say() {} },
  };
  s.rainHold = { progress: 0, active: 0, cooldown: 0, hintAt: -99 };
  s.t = 0;
  s.startRain = () => { started++; s.rainHold.progress = 0; };
  for (let i = 0; i < 22; i++) s.updateRain(0.1);
  assert.equal(started, 1);
});
