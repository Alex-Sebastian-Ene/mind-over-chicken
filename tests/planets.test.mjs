// planets (#68): progression rules (tiers, resources, setbacks, internal conflict), the surface
// zoom state machine and the vast surface generator. Run: node --test tests/planets.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  SIM, TIERS, ERAS, CONFLICT, newPlanetState, brainEffects, stepPlanet, stepSettlement, stepConflict, stabilityTarget,
  tierReady, eraReady, metrics, meets, castPlanet, tribeOutcome, arriveShip, easeSplit, ACTION_SPLIT, popCap,
  trackCampaignPop,
} from '../src/cosmos/planet-sim.js';
import { ZoomState } from '../src/cosmos/zoom.js';
import { Galaxy, FORM_SECONDS } from '../src/cosmos/galaxy.js';
import { World, initTerrainSync } from '../src/world/terrain.js';
import { settlementPlan, VW, VH, TILE, wrapX } from '../src/cosmos/vast.js';
import { PlanetSurface } from '../src/cosmos/surface.js';
import { StoneAge, label } from '../src/eras/stone.js';
import { Particles } from '../src/sim/util.js';

// The worlds come from Rust (#109): load the engine wasm once.
initTerrainSync(readFileSync(new URL('../pkg/mind_over_chicken_engine_bg.wasm', import.meta.url)));

const W = 320, H = 180;
const calmFx = (o = {}) => brainEffects({ calm: 0.2, focus: 0.2, stress: 0, blinksLast10s: 0, ...o });
const rngOf = (seed) => { let a = seed >>> 0; return () => { a = (a + 0x6d2b79f5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; };
const never = () => 0.999999;   // rng that never rolls a chance event
const always = () => 1e-9;      // rng that always does (never exactly 0: the market's gaussian redraws 0)
const ALLOC = { growth: 0.4, research: 0.4, stability: 0.2 };
const weights = (attended = false, ch = 'mixed') => (k) => (attended || ch === k ? 1 : SIM.OFF_CHANNEL);
const run = (p, seconds, fx, rng = never, attended = false, dt = 0.1) => {
  const events = [];
  for (let t = 0; t < seconds; t += dt) events.push(...stepPlanet(p, dt, fx, rng, attended).events);
  return events;
};
// A world that meets every condition for a village.
const villageReady = () => Object.assign(newPlanetState({ pop: 30, eraIndex: 1 }), { food: 60, wood: 40, stone: 5, goats: 10, stability: 0.7, progress: 0 });

// ---------- tiers and resources ----------

test('a new world starts as a camp with starting stocks', () => {
  const p = newPlanetState({ pop: 10 });
  assert.equal(p.tier, 0);
  assert.equal(TIERS[p.tier].id, 'camp');
  for (const k of ['food', 'wood', 'stone', 'goats']) assert.equal(p[k], SIM.START_STOCK[k]);
  assert.ok(p.stability > 0.5);
  assert.deepEqual(p.history, []);
});

test('tier rises once every entry condition holds for TIER_HOLD seconds, spending wood and stone', () => {
  const p = villageReady();
  assert.ok(tierReady(p));
  const wood0 = p.wood;
  const events = run(p, SIM.TIER_HOLD - 0.5, calmFx());
  assert.equal(p.tier, 0, 'not before the hold time');
  events.push(...run(p, 1.2, calmFx()));
  assert.equal(p.tier, 1);
  assert.ok(events.some((e) => e.kind === 'tier' && e.id === 'village'));
  assert.ok(events.some((e) => e.kind === 'milestone' && e.id === 'village'));
  assert.ok(p.wood < wood0, 'building the village spent wood');
  assert.ok(p.milestones.includes('village'));
  assert.match(p.history.map((h) => h.text).join('|'), /village/);
});

test('resources gate growth: short of wood, a world stays a camp', () => {
  const p = Object.assign(villageReady(), { wood: 2 });
  p.god = { act: () => ({ growth: 0, research: 1, stability: 0 }) };   // nobody gathers wood
  run(p, 3, calmFx());
  assert.equal(p.tier, 0);
  assert.ok(!tierReady(p));
});

test('tiers raise the population cap through housing', () => {
  const p = newPlanetState({ pop: 10, eraIndex: 1 });
  const camp = popCap(p);
  p.tier = 1;
  assert.ok(popCap(p) > camp);
  assert.equal(camp, Math.min(SIM.CAP[1], TIERS[0].housing * SIM.OVERCROWD_MAX));
});

test('an era only begins once the settlement can hold it', () => {
  const p = Object.assign(newPlanetState({ pop: 40, eraIndex: 1 }), { progress: 1 });
  assert.equal(p.tier, 0);
  assert.ok(!eraReady(p), 'Industry needs a village');
  run(p, 2, calmFx());
  assert.equal(p.eraIndex, 1);
  p.tier = 1;
  const events = run(p, 0.2, calmFx());
  assert.equal(p.eraIndex, 2);
  assert.ok(events.some((e) => e.kind === 'era' && e.text === ERAS[2].name));
});

test('food is eaten every second and hungry people eat their goats', () => {
  const p = Object.assign(newPlanetState({ pop: 50, eraIndex: 0, biome: 'ice' }), { food: 0, goats: 20 });
  p.god = { act: () => ({ growth: 0, research: 1, stability: 0 }) };
  run(p, 2, calmFx());
  assert.ok(p.hunger > 0, `hungry (${p.hunger})`);
  assert.ok(p.goats < 20, 'goats were eaten');
});

// ---------- stability and setbacks ----------

test("stability falls with hunger, overcrowding, the player's anger and stress", () => {
  const p = newPlanetState({ pop: 30 });
  const w = (k) => 1;
  const base = stabilityTarget(p, calmFx(), ALLOC, w);
  assert.ok(stabilityTarget(p, calmFx({ anger: 1 }), ALLOC, w) < base, 'anger');
  assert.ok(stabilityTarget(p, calmFx({ stress: 1 }), ALLOC, w) < base, 'stress');
  assert.ok(stabilityTarget({ ...p, hunger: 0.6 }, calmFx(), ALLOC, w) < base, 'hunger');
  assert.ok(stabilityTarget({ ...p, crowding: 1.3 }, calmFx(), ALLOC, w) < base, 'overcrowding');
  assert.ok(stabilityTarget(p, calmFx({ calm: 0.9 }), ALLOC, w) > base, 'calm helps');
  // Anger reaches the world you're looking at much more than the others.
  const off = stabilityTarget(p, calmFx({ anger: 1 }), ALLOC, weights(false));
  const on = stabilityTarget(p, calmFx({ anger: 1 }), ALLOC, weights(true));
  assert.ok(on < off);
});

test('famine: sustained hunger costs people and is logged', () => {
  const p = Object.assign(newPlanetState({ pop: 52, biome: 'ice' }), { food: 0, goats: 0 });
  p.god = { act: () => ({ growth: 0, research: 1, stability: 0 }) };
  const pop0 = p.pop;
  const events = run(p, SIM.FAMINE_SECONDS + 3, calmFx());
  const famine = events.find((e) => e.kind === 'setback' && e.id === 'famine');
  assert.ok(famine, 'famine fired');
  assert.ok(p.pop < pop0);
  assert.ok(p.history.some((h) => h.kind === 'setback' && /Famine/.test(h.text)));
});

test('plague: overcrowding brings disease', () => {
  const p = Object.assign(newPlanetState({ pop: 52 }), { food: 500 });
  const events = [];
  stepSettlement(p, 0.1, calmFx(), ALLOC, weights(), always, events);
  assert.ok(p.crowding > SIM.PLAGUE_CROWD);
  assert.ok(events.some((e) => e.id === 'plague'));
  assert.ok(p.pop < 52);
});

test('unrest: low stability burns the stores', () => {
  const p = Object.assign(newPlanetState({ pop: 20 }), { stability: 0.2, wood: 50, food: 50 });
  const events = [];
  stepSettlement(p, 0.1, calmFx({ anger: 1, stress: 1 }), ALLOC, weights(true), always, events);
  assert.ok(events.some((e) => e.id === 'unrest'));
  assert.ok(p.wood < 50);
});

test('collapse: a village that empties falls back to a camp, and rebuilding is a milestone', () => {
  const p = Object.assign(newPlanetState({ pop: 10, eraIndex: 1 }), { tier: 1 });
  const events = run(p, SIM.COLLAPSE_SECONDS + 0.5, calmFx());
  assert.equal(p.tier, 0);
  const c = events.find((e) => e.kind === 'setback' && e.id === 'collapse');
  assert.ok(c);
  assert.match(c.detail, /Village fell back to a camp/);
  // Regrow: meet the village needs again.
  Object.assign(p, { pop: 30, food: 60, wood: 40, stone: 5, goats: 10, stability: 0.7, setbackT: 0 });
  const back = run(p, SIM.TIER_HOLD + 0.5, calmFx());
  assert.equal(p.tier, 1);
  assert.ok(back.some((e) => e.kind === 'milestone' && e.id === 'rebuilt'));
});

test('setbacks are spaced at least SETBACK_GAP apart', () => {
  const p = Object.assign(newPlanetState({ pop: 52 }), { food: 500, stability: 0.2 });
  const events = run(p, SIM.SETBACK_GAP - 0.5, calmFx({ anger: 1 }), always, true);
  assert.equal(events.filter((e) => e.kind === 'setback').length, 1);
});

// ---------- internal conflict ----------

test("sustained anger escalates feud → revolt → civil war, and the war is decided", () => {
  const p = Object.assign(newPlanetState({ pop: 200, eraIndex: 1 }), { tier: 1, food: 400, wood: 100, stone: 50, goats: 50 });
  const events = run(p, 60, calmFx({ anger: 1, stress: 1, calm: 0 }), rngOf(3), true);
  const ids = events.filter((e) => e.kind === 'conflict').map((e) => e.id);
  const first = (id) => ids.indexOf(id);
  assert.ok(first('feud') >= 0 && first('revolt') > first('feud') && first('civil_war') > first('revolt'), ids.join(','));
  const ended = events.some((e) => ['crushed', 'split', 'scattered'].includes(e.id) || (e.kind === 'setback' && e.id === 'civil_war'));
  assert.ok(ended, 'the civil war ended one way or another');
  assert.ok(p.history.some((h) => h.kind === 'conflict'));
});

test('a civil war can split the world into two factions or collapse a tier', () => {
  let split = 0, collapsed = 0, crushed = 0;
  for (let seed = 1; seed <= 40; seed++) {
    const p = Object.assign(newPlanetState({ pop: 300, eraIndex: 2 }), { tier: 2, conflict: 3, tension: 1.2, stability: 0.2, inequality: 0.6, warT: SIM.CIVIL_WAR_SECONDS });
    const events = [];
    stepConflict(p, 0.01, calmFx({ anger: 1 }), ALLOC, weights(true), rngOf(seed), events);
    if (p.factions) { split++; assert.equal(p.factions.length, 2); assert.ok(Math.abs(p.factions[0].share + p.factions[1].share - 1) < 1e-9); }
    if (p.tier === 1) collapsed++;
    if (events.some((e) => e.id === 'crushed')) crushed++;
    assert.equal(p.conflict, 1, 'a decided war leaves a feud behind');
  }
  assert.ok(split > 0, 'some wars split the world');
  assert.ok(collapsed > 0, 'some wars collapse a tier');
  assert.equal(crushed, 0, 'an unstable, unequal world never crushes its rebels');
});

test('calm brings conflict back down to a truce, and the peace cast steps it down at once', () => {
  const p = Object.assign(newPlanetState({ pop: 100, eraIndex: 1 }), { tier: 1, food: 400, wood: 100, conflict: 2, tension: 0.62 });
  const events = run(p, 30, calmFx({ calm: 0.9 }), never, true);
  assert.equal(p.conflict, 0);
  assert.ok(events.some((e) => e.kind === 'truce'));
  const q = Object.assign(newPlanetState({ pop: 100 }), { conflict: 3, tension: 1 });
  castPlanet(q, 'peace');
  assert.equal(q.conflict, 2);
  assert.ok(q.tension < 1);
});

test('a split world reunites after a calm, orderly spell', () => {
  const p = Object.assign(newPlanetState({ pop: 100, eraIndex: 1 }), { tier: 1, food: 400, wood: 100, stability: 0.8, tension: 0, factions: [{ id: 'loyalists', share: 0.6 }, { id: 'rebels', share: 0.4 }] });
  const events = run(p, SIM.REUNITE_SECONDS + 2, calmFx({ calm: 0.9 }), never, true);
  assert.equal(p.factions, null);
  assert.ok(events.some((e) => e.id === 'reunited'));
});

// ---------- data tables, AI-god seam and outside events ----------

test('metrics/meets read data/ages.json-style entry conditions', () => {
  const m = metrics(villageReady());
  assert.equal(m.population, 30);
  assert.equal(m.wealth, 10);
  assert.ok(m.food_days > 0);
  assert.ok(meets(m, TIERS[1].enter));
  assert.ok(!meets(m, TIERS[2].enter));
  assert.ok(meets(m, { any: [{ population: 1000 }, { stability: 0.5 }] }));
  assert.ok(!meets(m, { all: { population: 1 }, any: [{ population: 1000 }] }));
  assert.ok(meets(m, null));
  for (const t of CONFLICT) assert.ok(typeof t.enter === 'number');
});

test("the PPO god's moves become the world's effort split", () => {
  const split = { growth: 0.4, research: 0.4, stability: 0.2 };
  for (let i = 0; i < 100; i++) easeSplit(split, 'calm', 0.1);
  assert.ok(Math.abs(split.stability - ACTION_SPLIT.calm.stability) < 0.01);
  const p = newPlanetState({ pop: 20 });
  p.god = { act: () => split };
  const o = [];
  stepPlanet(p, 0.1, calmFx(), never, false);
  assert.ok(Number.isFinite(p.stability));
  assert.equal(o.length, 0);
});

test('the capital tribe reports wins and losses', () => {
  const p = newPlanetState({ pop: 30 });
  const win = tribeOutcome(p, 'win');
  assert.ok(win.some((e) => e.id === 'tribe'));
  assert.ok(p.milestones.includes('tribe'));
  const pop = p.pop;
  const lose = tribeOutcome(p, 'lose');
  assert.ok(lose.some((e) => e.kind === 'setback'));
  assert.ok(p.pop < pop);
});

test('settlers on a barren world start a fresh camp and keep its history', () => {
  const p = newPlanetState({ pop: 0 });
  p.history.push({ t: 0, text: 'Formed, barren', kind: 'event' });
  assert.equal(arriveShip(p), 'settled');
  assert.equal(p.tier, 0);
  assert.equal(p.pop, SIM.SETTLE_POP);
  assert.equal(p.history.length, 2);
});

test('README §4: no research branches or evolution in the progression code', () => {
  for (const f of ['planet-sim.js', 'zoom.js', 'vast.js', 'surface.js', 'galaxy.js', 'panel.js']) {
    const src = readFileSync(new URL(`../src/cosmos/${f}`, import.meta.url), 'utf8');
    for (const re of [/\btech[_ ]?tree\b/i, /\bresearch[_ ]?tree\b/i, /\bevolution[_ ]?system\b/i, /\bevolve\s*\(/i]) assert.ok(!re.test(src), `${f} matches ${re}`);
  }
});

// ---------- the zoom state machine ----------

const planet = (o = {}) => ({ name: 'Lumen', home: false, forming: false, surfaceReady: true, ...o });

// #138: the zoom follows a critically damped spring, so it eases out of rest, eases in to the
// new level, never overshoots and lands exactly; retargeting mid-flight keeps z continuous.
const record = (z, frames, dt = 1 / 60) => { const zs = [z.z]; for (let i = 0; i < frames; i++) { z.update(dt); zs.push(z.z); } return zs; };

test('zoom eases in and out of each level without overshooting', () => {
  const z = new ZoomState();
  z.zoomTo(1);
  const zs = record(z, 120);
  const v = zs.slice(1).map((x, i) => (x - zs[i]) * 60);
  for (let i = 1; i < zs.length; i++) assert.ok(zs[i] >= zs[i - 1] - 1e-12, `monotone at frame ${i}`);
  assert.ok(Math.max(...zs) <= 1, 'never overshoots the target');
  assert.equal(z.z, 1, 'lands exactly on the level');
  assert.equal(z.vz, 0, 'and comes to rest');
  const peak = Math.max(...v);
  assert.ok(v[0] < peak * 0.3, `starts from rest (first ${v[0].toFixed(2)} vs peak ${peak.toFixed(2)})`);
  assert.ok(v.indexOf(peak) >= 6, 'and builds up speed over several frames');
  const settle = zs.findIndex((x) => x >= 0.99);
  assert.ok(settle > 40 && settle < 90, `takes about a second (${settle} frames to 99%)`);
  const tail = v.slice(settle - 5, settle);
  assert.ok(tail.every((x) => x < peak * 0.3), 'and slows down before arriving');
  for (let i = 1; i < v.length - 1; i++) assert.ok(Math.abs(v[i] - v[i - 1]) < 1.2, `no velocity jump at frame ${i}`);
});

test('changing the target mid-zoom keeps the motion continuous', () => {
  const z = new ZoomState();
  z.zoomTo(2);
  const out = record(z, 20);
  const before = z.vz;
  z.zoomTo(0);
  const back = record(z, 180);
  const zs = [...out, ...back.slice(1)];
  const v = zs.slice(1).map((x, i) => (x - zs[i]) * 60);
  assert.ok(before > 0 && v[20] > 0, 'keeps its momentum for a moment instead of reversing instantly');
  for (let i = 1; i < v.length; i++) assert.ok(Math.abs(v[i] - v[i - 1]) < 2, `velocity changes smoothly at frame ${i}`);
  assert.ok(Math.max(...zs) < 2, 'turned round before reaching the galaxy');
  assert.equal(z.z, 0);
  assert.equal(z.vz, 0);
});

test('reversing a fast zoom glides instead of stopping dead', () => {
  const z = new ZoomState();
  z.zoomTo(2);
  const out = record(z, 14);          // well on the way out, moving fast
  z.zoomTo(1);                        // back in one level while z is still below 1
  const back = record(z, 240);
  const zs = [...out, ...back.slice(1)];
  const v = zs.slice(1).map((x, i) => (x - zs[i]) * 60);
  for (let i = 1; i < v.length; i++) assert.ok(Math.abs(v[i] - v[i - 1]) < 2, `velocity changes smoothly at frame ${i}`);
  assert.ok(Math.max(...zs) < 1.5, 'only a small overshoot');
  assert.equal(z.z, 1);
  assert.equal(z.vz, 0);
});

test('zoom stays within 0..2 and ignores bad frame times', () => {
  const z = new ZoomState();
  z.zoomTo(2);
  z.update(NaN); z.update(-1);
  assert.equal(z.z, 0, 'a bad dt does not move it');
  for (let i = 0; i < 10; i++) z.update(5); // a long stall is clamped to 50 ms steps
  assert.ok(z.z > 0 && z.z <= 2);
  for (let i = 0; i < 200; i++) z.update(0.05);
  assert.equal(z.z, 2);
});

test('galaxy → globe → surface on another world, and back out', () => {
  const z = new ZoomState();
  z.homeRef = { name: 'Home', home: true };
  z.zoomTo(2);
  for (let i = 0; i < 100; i++) z.update(0.05);
  assert.equal(z.z, 2);
  assert.equal(z.attended(), null, 'nobody is attended from the galaxy');
  const p = planet();
  assert.ok(z.focusPlanet(p).ok);
  assert.equal(z.target, 1);
  for (let i = 0; i < 100; i++) z.update(0.05);
  assert.equal(z.attended(), p, 'the globe in view is attended');
  assert.equal(z.zoomIn().level, 0);
  for (let i = 0; i < 100; i++) z.update(0.05);
  assert.equal(z.z, 0);
  assert.equal(z.surfaceKind(), 'ai');
  assert.ok(z.aiSurfaceInView);
  assert.equal(z.attended(), p, "its surface takes the player's inputs");
  assert.equal(z.label(), 'Lumen · surface');
  assert.equal(z.zoomIn().level, 0, 'no level below the surface');
  z.zoomOut();
  assert.equal(z.target, 1);
  z.zoomOut();
  assert.equal(z.target, 2);
  for (let i = 0; i < 100; i++) z.update(0.05);
  assert.ok(!z.aiSurfaceInView);
  assert.equal(z.attended(), null);
});

test('a forming world cannot be focused; one without a surface stops at its globe', () => {
  const z = new ZoomState();
  z.zoomTo(2);
  assert.equal(z.focusPlanet(planet({ forming: true })).ok, false);
  const bare = planet({ surfaceReady: false });
  z.focusPlanet(bare);
  assert.equal(z.minLevel(), 1);
  const r = z.zoomIn();
  assert.equal(r.level, 1);
  assert.ok(r.refused);
  bare.surfaceReady = true;
  assert.equal(z.zoomIn().level, 0);
});

test('the home campaign lands on its own scene and takes blinks there directly', () => {
  const z = new ZoomState();
  const home = { name: 'Home', home: true };
  z.homeRef = home;
  assert.equal(z.surfaceKind(), 'campaign');
  assert.equal(z.minLevel(), 0);
  z.zoomTo(2);
  z.focusPlanet(home);
  assert.equal(z.focus, null, 'home is focus null');
  for (let i = 0; i < 40; i++) z.update(0.05);
  assert.equal(z.attended(), home, 'its globe is attended');
  z.zoomIn();
  for (let i = 0; i < 40; i++) z.update(0.05);
  assert.equal(z.surfaceKind(), 'campaign');
  assert.ok(!z.aiSurfaceInView);
  assert.equal(z.attended(), null, 'the campaign scene takes its own blinks');
  assert.equal(z.label(), 'Surface');
});

test('in the sandbox, the autonomous home lands once its surface is ready', () => {
  const z = new ZoomState();
  const home = { name: 'Home', home: true, autonomous: true, surfaceReady: false };
  z.homeRef = home;
  z.sandbox = true;
  z.z = 1; z.target = 2;
  assert.equal(z.minLevel(), 1);
  home.surfaceReady = true;
  assert.equal(z.minLevel(), 0);
  z.focusPlanet(home);
  z.zoomIn();
  for (let i = 0; i < 40; i++) z.update(0.05);
  assert.equal(z.surfaceKind(), 'ai');
  assert.equal(z.attended(), home);
});

test('a world that loses its surface pushes the view back to its globe', () => {
  const z = new ZoomState();
  const p = planet();
  z.zoomTo(2); z.focusPlanet(p); z.zoomIn();
  assert.equal(z.target, 0);
  p.surfaceReady = false;
  z.update(0.05);
  assert.equal(z.target, 1);
});

// ---------- vaster worlds ----------

test('vast surfaces: 4× the area, the capital is the AI world cut from it, more continents', () => {
  const world = World.planet(4242, 'jungle');
  const v = world.vast;
  assert.equal(VW * VH, 4 * W * H);
  assert.equal(v.w * v.h, VW * VH);
  assert.equal(v.type.length, VW * VH);
  assert.deepEqual(v.tile, TILE);
  for (const [x, y] of [[0, 0], [100, 90], [319, 179], [200, 40]]) {
    assert.equal(v.type[(y + TILE.y) * VW + wrapX(x + TILE.x)], world.baseType[y * W + x], `capital pixel ${x},${y}`);
  }
  assert.ok(v.continents.length >= 4, `continents: ${v.continents.length}`);
  assert.ok(v.spots[0].capital);
  assert.equal(v.spots[0].x, wrapX(world.sites.village.x + TILE.x));
  assert.ok(v.spots.length >= 8);
  let land = 0;
  for (const t of v.type) if (t >= 2) land++;
  assert.ok(land / v.type.length > 0.15, 'there is land beyond the capital');
});

test('settlements spread with the tier and the capital stays the biggest', () => {
  const spots = Array.from({ length: 12 }, (_, i) => ({ x: i * 40, y: 100, capital: i === 0 }));
  const sizes = [0, 1, 2, 3].map((tier) => settlementPlan({ tier, pop: [30, 200, 2000, 12000][tier] }, spots));
  for (let t = 1; t < 4; t++) assert.ok(sizes[t].length >= sizes[t - 1].length);
  for (const plan of sizes) for (const s of plan) assert.ok(s.size <= plan[0].size);
  assert.equal(settlementPlan({ tier: 2, pop: 0 }, spots).length, 0);
});

test('the G key visits every AI world in turn, then home', () => {
  const z = new ZoomState();
  const home = { name: 'Home', home: true };
  const a = planet({ name: 'A' }), b = planet({ name: 'B' }), forming = planet({ name: 'F', forming: true }), bare = planet({ name: 'X', surfaceReady: false });
  const planets = [home, a, forming, b, bare];
  z.homeRef = home;
  const seen = [];
  for (let i = 0; i < 4; i++) { const n = z.nextWorld(planets); seen.push(n.name); z.focusPlanet(n); z.zoomTo(0); }
  assert.deepEqual(seen, ['A', 'B', 'Home', 'A']);
  assert.equal(z.target, 0);
  assert.equal(z.surfaceKind(), 'ai');
  assert.equal(new ZoomState().nextWorld([home]).name, 'Home');
});

// #105: the campaign home never reads "barren" through a time-skip, and the Space Age's small
// local cast doesn't erase the civilisation's size.
test('campaign home keeps its population through time-skips and the Space Age', () => {
  const p = { home: true, eraIndex: 0, pop: 0 };
  const step = (eraIndex, pop) => { trackCampaignPop(p, { eraIndex, pop }); p.eraIndex = eraIndex; return p.pop; };
  assert.equal(step(0, 16), 16);      // Stone Age tribe
  assert.equal(step(1, null), 16);    // Farming → Kingdoms montage: no people in the scene
  assert.equal(step(2, 120), 120);    // Industry town
  assert.equal(step(3, null), 120);   // Information Age montage: was 0, i.e. "barren"
  assert.equal(step(3, 38), 120);     // Space Age: 28 citizens + 10 engineers on screen
  assert.ok(p.pop > 0);
});

test('a new campaign run starts the home population again', () => {
  const p = { home: true, eraIndex: 3, pop: 120, peakPop: 120 };
  trackCampaignPop(p, { eraIndex: 0, pop: 12 }); p.eraIndex = 0;
  assert.equal(p.pop, 12);
});

// Worlds form one at a time, FORM_SECONDS each: the next one waits its turn, and its map is only
// generated when that turn comes.
test('new worlds form one by one, FORM_SECONDS each', async () => {
  assert.equal(FORM_SECONDS, 4);
  const g = new Galaxy();
  const err = console.error; console.error = () => {}; // PlanetSurface needs a DOM; the galaxy catches that
  try {
    const [a, b] = ['Avalon', 'Brume'].map((name) => g.makePlanet({ name, inhabited: false }));
    assert.ok(a.forming && b.forming && a.formQueued && b.formQueued, 'both wait in the queue');
    let t = 0;
    const tick = async (sec) => { for (const end = t + sec - 1e-9; t < end; t += 0.1) { g.stepForming(0.1); await new Promise((r) => setImmediate(r)); } };
    await tick(0.5);
    assert.ok(a.forming && !a.formQueued, 'the first starts forming');
    assert.ok(b.formQueued && !b.world, 'the second waits, with no map yet');
    assert.ok(g.formProgress(a) > 0.1 && g.formProgress(b) === 0);
    await tick(3.2);
    assert.ok(a.forming, 'still forming before its 4 seconds are up');
    await tick(0.5);
    assert.ok(!a.forming && a.world && a.alive, 'formed after 4 seconds');
    assert.ok(b.forming && !b.formQueued, 'then the next one starts');
    await tick(3.5);
    assert.ok(b.forming, 'which takes its own 4 seconds');
    await tick(0.6);
    assert.ok(!b.forming && b.world, 'and then forms');
    assert.equal(g.formSlot, null);
    assert.equal(g.formQueue.length, 0);
  } finally { console.error = err; }
});

test('zoom in from planet globe to surface maintains continuous 3D camera zoom (#207)', () => {
  const z = new ZoomState();
  z.target = 1;
  z.z = 1.0;
  assert.equal(z.target, 1);
  assert.equal(z.z, 1.0);

  // Zoom in towards surface
  z.zoomIn();
  assert.equal(z.target, 0);

  // Over spring interpolation down to 0, z smoothly descends without overshooting
  for (let i = 0; i < 20; i++) {
    z.update(0.05);
  }
  assert.ok(z.z < 0.2, `expected descent below 0.2, got ${z.z}`);
  assert.ok(z.z >= 0, `expected non-negative zoom, got ${z.z}`);
});

test('wasm galaxy avoids double-simulation (#196)', () => {
  const g = new Galaxy();
  g.add({ name: 'Home', home: true, alive: true, pop: 20 });
  g.add({ name: 'Lumen', home: false, alive: true, pop: 10 });
  let jsSimulateCalled = 0;
  let jsSampleMindCalled = 0;
  g.simulate = () => { jsSimulateCalled++; };
  g.sampleMind = () => { jsSampleMindCalled++; };

  // Mock wasm galaxy
  g.wasm = {
    step: () => {},
    set_attended: () => {},
    planet_count: () => g.planets.length,
    planet_is_forming: () => false,
    planet_name: (i) => `WasmPlanet${i}`,
    planet_channel: () => 'calm',
    planet_biome: () => 'temperate',
    planet_pop: () => 42,
    planet_era: () => 1,
    planet_progress: () => 0.5,
    planet_market: () => 120,
    ship_count: () => 1,
    ship_from: () => 0,
    ship_to: () => 1,
    ship_t: () => 0.3,
  };

  const brain = { calm: 0.8, focus: 0.5, stress: 0.2, recentBlinks: [], stats: { blinks: 3 } };
  g.update(0.1, 0.1, brain);

  // JS simulate must not run when wasm is active; the mind sample still reveals and forms worlds
  assert.equal(jsSimulateCalled, 0, 'JS simulate must not run when wasm is present');
  assert.equal(jsSampleMindCalled, 1, 'the mind sample still runs: it forms the mind-worlds');

  // Wasm state synchronized
  assert.equal(g.planets[1].pop, 42);
  assert.equal(g.ships.length, 1);
  assert.equal(g.ships[0].t, 0.3);
});

test('PlanetSurface 3D overlay projects settlement labels and particles, skipping behind-camera (#202)', () => {
  const world = World.planet(4242, 'jungle');
  const p = newPlanetState({ pop: 50, eraIndex: 0 });
  const surface = new PlanetSurface(p, world);
  assert.ok(surface.plan.length > 0);

  // Mock canvas context
  const texts = [];
  const ctx = {
    font: '',
    textAlign: '',
    fillStyle: '',
    fillText(text, x, y) { texts.push({ text, x, y }); },
    save() {},
    restore() {},
    translate() {},
  };

  // 1. In 3D mode with __project3d:
  // Project (wx, wz) where wx = s.x / 2, wz = s.y / 2
  const projectedCalls = [];
  ctx.__project3d = (wx, wz) => {
    projectedCalls.push({ wx, wz });
    // First settlement in front of camera, subsequent behind camera (returns null)
    if (projectedCalls.length === 1) return [160, 90];
    return null;
  };

  surface.drawOverlay(ctx);

  // Verifies that project3d was called with 3D terrain coordinates (vast / 2)
  assert.ok(projectedCalls.length > 0);
  assert.equal(projectedCalls[0].wx, surface.plan[0].x / 2);
  assert.equal(projectedCalls[0].wz, surface.plan[0].y / 2);

  // Only the visible settlement in front of camera was rendered (at pr[0], pr[1] - 8 - s.size * 2)
  // Behind-camera settlement (null) was skipped, never drawn at unprojected coordinates
  assert.ok(texts.some((t) => t.text.includes('capital') && Math.abs(t.x - 160) < 1));

  // 2. drawScreenOverlay renders HUD in screen coordinates
  const screenTexts = [];
  const screenCtx = {
    font: '',
    textAlign: '',
    fillStyle: '',
    fillText(text, x, y) { screenTexts.push({ text, x, y }); },
  };
  surface.drawScreenOverlay(screenCtx);
  assert.ok(screenTexts.some((t) => t.text.includes(p.name) && t.x === 4));
  assert.ok(screenTexts.some((t) => t.text.includes('explore') && t.x === 160));
});

test('StoneAge.drawOverlay suppresses campaign tutorial prompts on simulated AI worlds (#202)', () => {
  const normalStone = new StoneAge();
  const mockG = {
    world: {
      paint: () => ({ width: 1, height: 1 }),
      sites: { fire: { x: 50, y: 50 }, cave: { x: 40, y: 40 }, village: { x: 50, y: 50 } },
      randomLand: () => ({ x: 45, y: 45 }),
      nearestWalkable: () => ({ x: 55, y: 55 }),
    },
    brain: { setCue() {} },
    advisor: { say() {} },
  };
  normalStone.enter(mockG);

  // Campaign mode: unlit fire prompts player with "blink here to light the fire"
  const campaignTexts = [];
  const ctx = {
    font: '',
    textAlign: '',
    fillStyle: '',
    fillText(text, x, y) { campaignTexts.push(text); },
  };
  normalStone.drawOverlay(ctx);
  assert.ok(campaignTexts.includes('blink here to light the fire'), 'campaign stone age shows tutorial prompt');

  // Simulated AI mode: tutorial prompt is suppressed
  const simStone = new StoneAge();
  const simG = { ...mockG, simulated: true };
  simStone.enter(simG);

  const simTexts = [];
  simStone.drawOverlay({
    font: '',
    textAlign: '',
    fillStyle: '',
    fillText(text, x, y) { simTexts.push(text); },
  });
  assert.ok(!simTexts.includes('blink here to light the fire'), 'simulated stone age suppresses tutorial prompt');
});

test('label and Particles.drawText skip rendering when __project3d returns null (#202)', () => {
  const texts = [];
  const ctx = {
    font: '',
    textAlign: '',
    fillStyle: '',
    fillText(text, x, y) { texts.push({ text, x, y }); },
    __project3d: () => null, // all world points behind camera
  };

  label(ctx, 100, 100, 'behind camera test', '#fff');
  assert.equal(texts.length, 0, 'label behind camera should not be drawn');

  const p = new Particles();
  p.add({ x: 100, y: 100, life: 1, maxLife: 1, shape: 'text', text: 'behind camera particle', color: '#fff', size: 6 });
  p.drawText(ctx);
  assert.equal(texts.length, 0, 'particle behind camera should not be drawn');
});
