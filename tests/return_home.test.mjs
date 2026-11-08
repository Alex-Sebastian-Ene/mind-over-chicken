// Death returns the camera home (#297): the policy (src/cosmos/return-home.js) and the one-shot
// move it runs on the zoom state machine (src/cosmos/zoom.js), plus the AI surface's side: it
// says when its capital tribe dies and doesn't regather it in front of a watching player.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  DEATHS, RETURN_BEAT, HOME_FAIL_BEAT, HOME_LINE, returnHomePlan, ReturnHome,
} from '../src/cosmos/return-home.js';
import { ZoomState } from '../src/cosmos/zoom.js';

const home = () => ({ name: 'Home', home: true });
const aiWorld = (name = 'Lumen') => ({ name, surfaceReady: true });
// A zoom state settled on a level, the camera on `focus` (null: the home campaign).
const camAt = (level, focus = null) => {
  const z = new ZoomState();
  z.homeRef = home();
  z.focus = focus;
  z.z = z.target = level;
  return z;
};
const run = (rh, cam, seconds, ctx = {}, dt = 1 / 60) => {
  for (let t = 0; t < seconds; t += dt) { cam.update(dt); rh.update(dt, ctx); }
};

test('the policy table: each death goes to the home globe, with words for other worlds', () => {
  const fail = returnHomePlan({ reason: DEATHS.HOME_FAIL });
  assert.deepEqual([fail.go, fail.level, fail.delay, fail.line, fail.toast], [true, 1, HOME_FAIL_BEAT, null, null]);
  const ai = returnHomePlan({ reason: DEATHS.AI_TRIBE, world: 'Lumen', cause: 'The tribe froze in the dark: blink directly on the fire pit.' });
  assert.equal(ai.go, true);
  assert.equal(ai.level, 1);
  assert.equal(ai.delay, RETURN_BEAT);
  assert.ok(ai.delay >= 1.5 && ai.delay <= 2, 'a short beat that shows the cause');
  assert.equal(ai.cause, 'Lumen: The tribe froze in the dark');   // not the campaign's advice
  assert.match(ai.toast, /^Lumen is lost/);
  assert.equal(ai.line, HOME_LINE);
  const civ = returnHomePlan({ reason: DEATHS.CIV_WIPED });
  assert.deepEqual([civ.go, civ.level, civ.line], [true, 1, HOME_LINE]);
  assert.match(civ.toast, /civilisation is lost/);
});

test('never on a win, a manual quit, the title or end screen, the ending or an intro', () => {
  for (const reason of ['win', 'quit', undefined, 'nonsense']) assert.equal(returnHomePlan({ reason }).go, false, reason);
  assert.equal(returnHomePlan({ reason: DEATHS.HOME_FAIL, screen: 'title' }).go, false);
  assert.equal(returnHomePlan({ reason: DEATHS.AI_TRIBE, screen: 'end' }).go, false);
  assert.equal(returnHomePlan({ reason: DEATHS.CIV_WIPED, finished: true }).go, false);
  assert.equal(returnHomePlan({ reason: DEATHS.HOME_FAIL, intro: true }).go, false);
});

test("doesn't fight the player, repeats, or a world nobody was watching", () => {
  assert.equal(returnHomePlan({ reason: DEATHS.AI_TRIBE, navigating: true }).why, 'player is navigating');
  assert.equal(returnHomePlan({ reason: DEATHS.HOME_FAIL, returning: true }).why, 'already returning');
  assert.equal(returnHomePlan({ reason: DEATHS.AI_TRIBE, watching: false }).why, 'not watching that world');
  assert.equal(returnHomePlan({ reason: DEATHS.HOME_FAIL, at: { home: true, level: 1 } }).why, 'already home');
});

test('a death never lands you, never leaves for the galaxy, unless home is still forming', () => {
  assert.equal(returnHomePlan({ reason: DEATHS.HOME_FAIL, level: 0 }).level, 1);
  assert.equal(returnHomePlan({ reason: DEATHS.HOME_FAIL, at: { home: true, level: 0 } }).level, 1);
  const forming = returnHomePlan({ reason: DEATHS.AI_TRIBE, home: { forming: true, landable: false } });
  assert.equal(forming.level, 2);
  assert.match(forming.toast, /still forming/);
  const autonomous = returnHomePlan({ reason: DEATHS.AI_TRIBE, home: { forming: false, landable: false } });
  assert.equal(autonomous.level, 1);
  assert.match(autonomous.toast, /on its own/);
});

test('home campaign fail: the camera eases to the home globe behind the card', () => {
  const cam = camAt(0);
  const said = [];
  const rh = new ReturnHome(cam, { say: (t) => said.push(t), advise: (t) => said.push(t) });
  assert.equal(rh.request({ reason: DEATHS.HOME_FAIL }).go, true);
  assert.equal(rh.request({ reason: DEATHS.HOME_FAIL }).why, 'already returning');   // one-shot
  run(rh, cam, HOME_FAIL_BEAT - 0.1);
  assert.equal(cam.target, 0, 'the beat first');
  run(rh, cam, 0.2);
  assert.equal(cam.target, 1);
  assert.equal(cam.focus, null);
  assert.ok(cam.z > 0 && cam.z < 1, 'eased, not cut');
  run(rh, cam, 2);
  assert.equal(cam.z, 1);
  assert.equal(rh.active, false);
  assert.deepEqual(said, [], 'the fail card does the talking');
});

test('an AI world dies under you: a beat, its globe, then the home globe', () => {
  const lumen = aiWorld();
  const cam = camAt(0, lumen);
  const said = [], causes = [];
  const rh = new ReturnHome(cam, { cause: (t) => causes.push(t), say: (t) => said.push(t), advise: (t) => said.push(t) });
  const plan = rh.request({ reason: DEATHS.AI_TRIBE, planet: lumen, world: 'Lumen', cause: 'The tribe starved: blink.' });
  assert.equal(plan.go, true);
  assert.deepEqual(causes, ['Lumen: The tribe starved']);
  run(rh, cam, RETURN_BEAT - 0.1);
  assert.equal(cam.focus, lumen, 'still showing the cause');
  run(rh, cam, 0.2);
  assert.equal(cam.focus, lumen, 'leaving: its own globe first, no cut');
  assert.equal(cam.target, 1);
  assert.deepEqual(said, [plan.toast, HOME_LINE]);
  run(rh, cam, 1.2);
  assert.equal(cam.focus, null, 'home took over');
  assert.equal(cam.target, 1);
  assert.equal(rh.active, false);
  assert.ok(RETURN_BEAT + 1.3 < 3, 'home before the dead tribe would regather (RESTART_AFTER)');
});

test('only the world on screen: a death elsewhere leaves the camera alone', () => {
  const lumen = aiWorld(), vex = aiWorld('Vex');
  const onVex = camAt(0, vex);
  assert.equal(new ReturnHome(onVex).request({ reason: DEATHS.AI_TRIBE, planet: lumen }).go, false);
  const onGlobe = camAt(1, lumen);
  assert.equal(new ReturnHome(onGlobe).request({ reason: DEATHS.AI_TRIBE, planet: lumen }).go, false);
});

test('interruptible: moving the camera during the beat or the ease hands it back', () => {
  const lumen = aiWorld();
  let cam = camAt(0, lumen);
  let rh = new ReturnHome(cam);
  rh.request({ reason: DEATHS.AI_TRIBE, planet: lumen });
  run(rh, cam, 0.5);
  cam.zoomTo(1);                    // the player zooms out themselves
  run(rh, cam, 3);
  assert.equal(rh.active, false);
  assert.equal(cam.focus, lumen, 'their move, not ours');

  cam = camAt(0, lumen);
  rh = new ReturnHome(cam);
  rh.request({ reason: DEATHS.AI_TRIBE, planet: lumen });
  run(rh, cam, RETURN_BEAT + 0.1);
  assert.equal(cam.target, 1);
  cam.zoomTo(0);                    // ...and lands straight back
  run(rh, cam, 3);
  assert.equal(cam.focus, lumen);
  assert.equal(cam.z, 0);
});

test('a zoom in flight, the title or end screen, or an intro cancel or block the return', () => {
  const lumen = aiWorld();
  const busy = camAt(0, lumen);
  busy.target = 1;                  // already zooming out
  assert.equal(new ReturnHome(busy).request({ reason: DEATHS.AI_TRIBE, planet: lumen }).why, 'player is navigating');
  const cam = camAt(0);
  const rh = new ReturnHome(cam);
  rh.request({ reason: DEATHS.HOME_FAIL });
  run(rh, cam, 0.2, { screen: 'title' });   // quit to title
  assert.equal(rh.active, false);
  run(rh, cam, 1);
  assert.equal(cam.target, 0);
});

test('reduced motion: the same move as a cut', () => {
  const lumen = aiWorld();
  const cam = camAt(0, lumen);
  const rh = new ReturnHome(cam, { reduced: () => true });
  rh.request({ reason: DEATHS.AI_TRIBE, planet: lumen });
  run(rh, cam, RETURN_BEAT + 0.05);
  assert.equal(cam.focus, null);
  assert.equal(cam.z, 1);
  assert.equal(cam.target, 1);
});

test('home still forming: the galaxy card instead', () => {
  const lumen = aiWorld();
  const cam = camAt(0, lumen);
  cam.homeRef.forming = true;
  const said = [];
  const rh = new ReturnHome(cam, { say: (t) => said.push(t) });
  assert.equal(rh.request({ reason: DEATHS.AI_TRIBE, planet: lumen }).level, 2);
  run(rh, cam, RETURN_BEAT + 0.05);
  assert.equal(cam.target, 2);
  assert.match(said[0], /still forming/);
});

test('main.js routes every death path through returnHome (#297)', () => {
  const main = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.match(main, /returnHome\(\{ reason: 'home-fail' \}\)/);
  assert.match(main, /kind === 'tribe-lost'.*returnHome\(\{ reason: 'ai-tribe'/);
  assert.match(main, /returnHome\(opts\) \{ returnHome\(opts\); \}/);
  assert.match(main, /function quitToTitle\(\) \{[^}]*homeward\.cancel\(\)/);
  assert.match(main, /restartScene\(\);\s*landHome\(\);/);
  const civ = readFileSync(new URL('../src/civ/sandbox.js', import.meta.url), 'utf8');
  assert.match(civ, /returnHome\?\.\(\{ reason: 'civ-wiped' \}\)/);
});

test("an AI surface says when its tribe dies and doesn't regather it while you watch", async () => {
  const { World, initTerrainSync } = await import('../src/world/terrain.js');
  initTerrainSync(readFileSync(new URL('../pkg/mind_over_chicken_engine_bg.wasm', import.meta.url)));
  const { newPlanetState } = await import('../src/cosmos/planet-sim.js');
  const { PlanetSurface } = await import('../src/cosmos/surface.js');
  const p = newPlanetState({ pop: 50, eraIndex: 0 });
  const s = new PlanetSurface(p, World.planet(4242, 'jungle'));
  s.sim.G.fail('The tribe starved: blink to strike giant chickens.');
  const tick = (secs, inView) => { const evs = []; for (let t = 0; t < secs; t += 0.1) evs.push(...s.tick(0.1, inView)); return evs; };
  const first = tick(0.1, true);
  assert.deepEqual(first.filter((e) => e.kind === 'lost').map((e) => e.detail), ['The tribe starved: blink to strike giant chickens.']);
  const watched = tick(4, true);
  assert.equal(watched.some((e) => e.kind === 'lost'), false, 'once per death');
  assert.equal(s.episodes, 0, 'no new tribe in front of the player');
  tick(0.5, false);                 // the camera went home: it regathers off-screen
  assert.equal(s.episodes, 1);
  assert.equal(s.sim.outcome, null);
  assert.equal(s.sim.failReason, null);
});
