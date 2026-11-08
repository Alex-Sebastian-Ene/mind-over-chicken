// Stress: brain.stress is computed once in the brain and read by the HUD, calibration, galaxy and
// eras. It is neutral at rest and rises on agitation, not on a missing calm:
//   stress = max(EEG agitation, face anger, REST_STRESS * (1 - calm))
// Run: node --test tests/stress.test.mjs   (also run by tests/test_stress.py)

import { mulberry32 } from '../rl/headless.mjs';   // canvas stubs so the eras run in Node
import test from 'node:test';
import assert from 'node:assert/strict';
import { Brain, stressFrom, faceAnger, agitationIndex, REST_STRESS, STRESSED } from '../src/brain/brain.js';
import { SimSource } from '../src/brain/sources.js';
import { featuresFromBands } from '../src/brain/model.js';
import { ProxyBrain, SharedBrain } from '../src/ai/proxyBrain.js';
import { Calibration } from '../src/brain/calibration.js';
import { World } from '../src/world/terrain.js';
import { Industry } from '../src/eras/industry.js';
import { SpaceAge } from '../src/eras/space.js';

const near = (a, b, eps = 1e-9) => assert.ok(Math.abs(a - b) <= eps, `${a} vs ${b}`);
const noop = () => {};

test('stress is max(agitation, face anger, REST_STRESS * (1 - calm)), clamped to 0..1', () => {
  near(stressFrom(1), 0);                       // calm, no agitation: no stress
  near(stressFrom(0), REST_STRESS);              // not calm is only a little stress
  near(stressFrom(0.1, 0, 0.9), 0.9);            // agitation
  near(stressFrom(0.9, 0.6), 0.6);               // an angry face stresses even a calm god
  near(stressFrom(0.9, 0.2, 0.7), 0.7);
  near(stressFrom(1.4, -1, 2), 1);
  near(stressFrom(undefined), REST_STRESS);
});

test('face anger only counts while the camera is on and sees a face', () => {
  near(faceAnger(null), 0);
  near(faceAnger({ active: false, face: true, anger: 0.9 }), 0);
  near(faceAnger({ active: true, face: false, anger: 0.9 }), 0);
  near(faceAnger({ active: true, face: true, anger: 0.9 }), 0.9);
});

test('the agitation index rises with gamma and falls with focus-style beta', () => {
  const f = (b) => agitationIndex(featuresFromBands(b));
  const rest = f({ delta: 0.26, theta: 0.17, alpha: 0.2, beta: 0.15, gamma: 0.06 });
  const tense = f({ delta: 0.26, theta: 0.17, alpha: 0.13, beta: 0.27, gamma: 0.2 });
  const focused = f({ delta: 0.26, theta: 0.2, alpha: 0.12, beta: 0.38, gamma: 0.08 });
  assert.ok(tense > rest + 0.4, `tense ${tense} vs rest ${rest}`);
  assert.ok(focused < rest, `focused ${focused} vs rest ${rest}`);
});

// A trained emulated player: rest, calm and focus windows, then the calibration window locks the
// resting agitation level. `run` returns the median stress over the second half of each phase.
function player(seed) {
  Math.random = mulberry32(seed);
  const src = new SimSource();
  const b = new Brain(src);
  const run = (sec, input = {}, cue = null, mood = null) => {
    Object.assign(src.input, { calm: false, focus: false, panic: false }, input);
    b.setCue(cue);
    const xs = [];
    for (let i = 0; i < sec * 20; i++) { b.update(0.05, mood); if (i >= sec * 10) xs.push(b.stress); }
    return xs.sort((x, y) => x - y)[xs.length >> 1];
  };
  run(10, {}, 'rest');
  b.lockAgitationBaseline(10);
  run(10, {}, 'rest');
  run(10, { calm: true }, 'calm');
  run(10, { focus: true }, 'focus');
  run(8);
  return { b, run };
}

for (const seed of [7, 11, 23]) {
  test(`emulated headset (seed ${seed}): rest ≈ 0.15, Shift → 1, Space → 0, focus is not stress`, () => {
    const { b, run } = player(seed);
    const idle = run(8);
    assert.ok(idle >= 0.1 && idle <= 0.3, `idle ${idle}`);
    const shift = run(6, { panic: true });
    assert.ok(shift > 0.9, `Shift ${shift}`);
    run(10);
    const space = run(8, { calm: true });
    assert.ok(space < 0.1, `Space ${space}`);
    const focus = run(8, { focus: true });
    assert.ok(focus <= 0.3, `focus ${focus}`);
    assert.ok(b.focus > 0.6, `focus should still read as focus: ${b.focus}`);
    const shiftOverCalm = run(6, { calm: true, panic: true });
    assert.ok(shiftOverCalm > 0.9, `Shift wins over Space: ${shiftOverCalm}`);
    run(12);
    const back = run(8);
    assert.ok(back <= 0.3, `back to rest after Shift: ${back}`);
  });
}

test('face anger raises stress while the player is calm', () => {
  const { b, run } = player(5);
  run(8, { calm: true });
  const angry = { active: true, face: true, anger: 0.7 };
  run(0.2, { calm: true }, null, angry);
  assert.ok(b.calm > 0.8, `still calm: ${b.calm}`);
  near(b.anger, 0.7);
  near(b.stress, 0.7, 0.05);
});

test('without a calibration window the running average is the resting level', () => {
  Math.random = mulberry32(3);
  const src = new SimSource();
  const b = new Brain(src);
  for (let i = 0; i < 400; i++) b.update(0.05);
  assert.equal(b.agBaseline, null);
  assert.ok(b.stress <= 0.3, `idle ${b.stress}`);
  src.input.panic = true;
  for (let i = 0; i < 120; i++) b.update(0.05);
  assert.ok(b.stress > 0.9, `Shift ${b.stress}`);
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
  near(ai.stress, stressFrom(ai.calm, 0, ai.agitation));
  ai.input.panic = true;
  for (let i = 0; i < 100; i++) ai.update(0.05);
  assert.ok(ai.stress > 0.8, `proxy Stress key: ${ai.stress}`);
  ai.input.panic = false;
  for (let i = 0; i < 400; i++) ai.update(0.05);
  assert.ok(ai.stress < 0.3, `proxy at rest: ${ai.stress}`);
  const human = { calm: 0.3, focus: 0, anger: 0.9, agitation: 0, aim: { x: 0.5, y: 0.5 } };
  const shared = new SharedBrain(ai, human);
  near(shared.stress, 0.9);            // the human's face anger
  human.anger = 0; human.agitation = 0.8;
  near(shared.stress, 0.8);            // the human's agitation
  human.agitation = 0; ai.calm = 0.95;
  near(shared.stress, REST_STRESS * 0.05);   // calm counts as the higher of the two
});

test('calibration: a player at rest seeds a civilisation that is not aggressive', () => {
  const { b } = player(13);
  const cal = new Calibration({ seconds: 5, mode: 'sensor', seed: 1 });
  for (let i = 0; i < 120; i++) { b.update(0.05); cal.update(0.05, b, null); }
  assert.equal(cal.done, true);
  const r = cal.result;
  assert.ok(r.features.stress <= 0.3, `stress ${r.features.stress}`);
  assert.ok(r.aggressiveness < 0.3, `aggressiveness ${r.aggressiveness}`);
  assert.ok(r.violence < 0.3, `violence ${r.violence}`);
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
const fakeBrain = (o) => ({ calm: 0.1, focus: 0.1, stress: REST_STRESS, anger: 0, aim: { x: 0.5, y: 0.5 }, threshold: (k, b = 0.55) => b, model: { trust: () => 0 }, setCue: noop, ...o });

test('industry: the market reads stress; the IPO runs at rest and on calm, not when agitated', () => {
  const brain = fakeBrain({});
  const s = scene(Industry, brain);
  assert.deepEqual(s.powers, ['stress', 'calm']);
  near(s.arousal(), REST_STRESS);
  brain.stress = 1;
  near(s.arousal(), 1);
  s.ipo.state = 'active';
  s.ipo.progress = 1;
  s.update(0.1);
  near(s.ipo.limit, STRESSED);
  assert.ok(s.ipo.progress < 1, 'agitated: the IPO wobbles');
  for (const [calm, stress] of [[0.1, REST_STRESS], [0.95, 0.01]]) {
    brain.calm = calm; brain.stress = stress;
    const p = s.ipo.progress;
    s.update(0.1);
    assert.ok(s.ipo.progress > p, `stress ${stress}: the IPO advances`);
  }
});

test('space age: stress drops parts; the countdown needs focus and low stress, and focus alone passes', () => {
  const brain = fakeBrain({ focus: 0.95, calm: 0, stress: REST_STRESS });
  const s = scene(SpaceAge, brain);
  assert.deepEqual(s.powers, ['focus', 'calm', 'stress']);
  assert.equal(s.stressed(), false);
  brain.stress = 1;
  assert.equal(s.stressed(), true);
  s.phase = 'countdown';
  s.count = 10;
  s.update(0.5);
  near(s.count, 10);
  assert.ok(s.holdT > 0, 'agitated: the count holds');
  brain.stress = REST_STRESS;
  s.update(0.5);
  assert.ok(s.count < 10, 'focused at rest stress: the count runs');
  brain.focus = 0;
  const c = s.count;
  s.update(0.5);
  near(s.count, c);
});

test('space age finale: countdown triggers liftoff, transitions to voyage cinematic, and finishes the game', () => {
  let finished = false;
  let completed = false;
  const brain = fakeBrain({ focus: 0.95, calm: 0.5, stress: 0.1 });
  const s = scene(SpaceAge, brain);
  s.G.finish = () => { finished = true; };
  s.G.complete = () => { completed = true; };

  // Countdown finishes
  s.phase = 'countdown';
  s.count = 0.05;
  s.update(0.1);
  assert.equal(s.phase, 'launch', 'countdown transitions to launch');
  assert.equal(completed, false, 'G.complete must not be called during launch');

  // Liftoff progresses until rocket clears the island
  while (s.phase === 'launch') {
    s.update(0.1);
  }
  assert.equal(s.phase, 'voyage', 'rocket launch transitions to interplanetary voyage');
  assert.ok(s.voyageT >= 0);
  assert.deepEqual(s.lights(), [], 'no surface pad light in outer space voyage');
  assert.equal(completed, false, 'G.complete must not be called during voyage');

  // Voyage progresses for 7.5 seconds
  while (s.phase === 'voyage' && !finished) {
    s.update(0.2);
  }
  assert.equal(finished, true, 'voyage completes by calling G.finish()');
  assert.equal(s.done, true);
  assert.equal(completed, false, 'G.complete must never be called in Space Age');
});

