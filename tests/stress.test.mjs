// Stress: brain.stress = max(1 - calm, face anger) is computed once in the brain and read by the
// HUD, calibration and eras (industry market panic and IPO, space-age drops and boarding).
// Run: node --test tests/stress.test.mjs   (also run by tests/test_stress.py)

import { mulberry32 } from '../rl/headless.mjs';   // canvas stubs so the eras run in Node
import test from 'node:test';
import assert from 'node:assert/strict';
import { Brain, stressFrom, faceAnger } from '../src/brain/brain.js';
import { SimSource } from '../src/brain/sources.js';
import { ProxyBrain, SharedBrain } from '../src/ai/proxyBrain.js';
import { World } from '../src/world/terrain.js';
import { Industry } from '../src/eras/industry.js';
import { SpaceAge } from '../src/eras/space.js';

const near = (a, b, eps = 1e-9) => assert.ok(Math.abs(a - b) <= eps, `${a} vs ${b}`);
const noop = () => {};

test('stress is max(1 - calm, face anger), clamped to 0..1', () => {
  near(stressFrom(1), 0);
  near(stressFrom(0), 1);
  near(stressFrom(0.7), 0.3);
  near(stressFrom(0.9, 0.6), 0.6);    // an angry face stresses even a calm god
  near(stressFrom(0.2, 0.6), 0.8);
  near(stressFrom(1.4, -1), 0);
  near(stressFrom(undefined), 1);
});

test('face anger only counts while the camera is on and sees a face', () => {
  near(faceAnger(null), 0);
  near(faceAnger({ active: false, face: true, anger: 0.9 }), 0);
  near(faceAnger({ active: true, face: false, anger: 0.9 }), 0);
  near(faceAnger({ active: true, face: true, anger: 0.9 }), 0.9);
});

test('emulated headset: calm lowers stress; the Stress key raises it, even over the calm key', () => {
  Math.random = mulberry32(7);
  const src = new SimSource();
  const b = new Brain(src);
  const run = (sec, input, cue = null, mood = null) => {
    Object.assign(src.input, { calm: false, focus: false, panic: false }, input);
    b.setCue(cue);
    for (let i = 0; i < sec * 20; i++) b.update(0.05, mood);
  };
  run(20, {}, 'rest');
  run(12, { calm: true }, 'calm');
  assert.ok(b.stress < 0.2, `calm: stress ${b.stress}`);
  near(b.stress, stressFrom(b.calm, 0));
  run(6, { calm: true, panic: true });
  assert.ok(b.stress > 0.8, `Stress key over calm: stress ${b.stress}`);
  run(8, { calm: true });
  const angry = { active: true, face: true, anger: 0.7 };
  run(0.2, { calm: true }, null, angry);
  assert.ok(b.calm > 0.8, `still calm: ${b.calm}`);
  near(b.anger, 0.7);
  near(b.stress, 0.7, 0.05);
});

test('emulated headset frames carry no heart-rate or PPG fields', () => {
  const frames = [];
  const src = new SimSource();
  src.update(0.35, (f) => frames.push(f));
  assert.ok(frames.length >= 3);
  for (const f of frames) {
    for (const k of ['bands', 'blink', 'tilt', 'motion', 'quality']) assert.ok(k in f, k);
    for (const k of ['hr', 'ppg']) assert.equal(k in f, false, k);
  }
});

test('ProxyBrain and SharedBrain derive stress the same way', () => {
  const ai = new ProxyBrain();
  near(ai.stress, stressFrom(ai.calm));
  const human = { calm: 0.3, focus: 0, anger: 0.9, aim: { x: 0.5, y: 0.5 } };
  const shared = new SharedBrain(ai, human);
  near(shared.stress, 0.9);            // the human's face anger
  human.anger = 0;
  near(shared.stress, 0.7);            // 1 - max(ai calm, human calm)
  ai.calm = 0.95;
  near(shared.stress, 0.05);           // the AI's calm counts on a shared world, as calm does
});

function scene(Era, brain, seed = 3) {
  Math.random = mulberry32(seed);
  const G = {
    world: new World(seed), brain, advisor: { say: () => true, setEra: noop }, audio: new Proxy({}, { get: () => noop }),
    source: { addStartle: noop }, stats: { recessions: 0 }, shake: 0, complete: noop, fail: noop, finish: noop,
  };
  const s = new Era();
  G.scene = s;
  s.enter(G);
  return s;
}
const fakeBrain = (o) => ({ calm: 0, focus: 0, stress: 1, anger: 0, aim: { x: 0.5, y: 0.5 }, threshold: (k, b = 0.55) => b, model: { trust: () => 0 }, setCue: noop, ...o });

test('industry: the market reads stress, and the IPO needs eight seconds of low stress', () => {
  const brain = fakeBrain({ calm: 0.1, stress: 0.9 });
  const s = scene(Industry, brain);
  assert.deepEqual(s.powers, ['stress', 'calm']);
  near(s.arousal(), 0.9);
  brain.stress = 0.2;
  near(s.arousal(), 0.2);
  s.ipo.state = 'active';
  s.update(0.1);
  near(s.ipo.limit, 0.45);             // 1 - the calm bar
  brain.calm = 0.1; brain.stress = 0.9;
  s.ipo.progress = 1;
  s.update(0.1);
  assert.ok(s.ipo.progress < 1, 'stressed: the IPO wobbles');
  brain.calm = 0.9; brain.stress = 0.1;
  const p = s.ipo.progress;
  s.update(0.1);
  assert.ok(s.ipo.progress > p, 'calm: the IPO advances');
});

test('space age: stress drops parts and scares boarders; the countdown runs on focus alone', () => {
  const brain = fakeBrain({ focus: 0.95, calm: 0, stress: 1 });
  const s = scene(SpaceAge, brain);
  assert.deepEqual(s.powers, ['focus', 'calm', 'stress']);
  assert.equal(s.stressed(), true);
  brain.stress = 0.3;
  assert.equal(s.stressed(), false);
  brain.stress = 1;
  s.phase = 'countdown';
  s.count = 10;
  s.update(0.5);
  assert.ok(s.count < 10, 'focused with stress at 1: the count still runs');
  brain.focus = 0;
  const c = s.count;
  s.update(0.5);
  near(s.count, c);
  assert.ok(s.holdT > 0);
});
