// The AI god's senses and hands for the Stone Age, shared by the headless trainer
// (rl/env_server.mjs) and the in-browser simulated planets (src/ai/planet.js). This file
// plus rl/README.md is the specification a Rust port must reproduce 1:1: feature order,
// scaling constants, action decoding. Change anything here and the policy must be retrained
// (bump OBS_VERSION).
//
// The world is a torus (#111) of its own size (640×360 at home, #129; W x H by default), so every
// distance here is the shorter way round, and aims are 0..1 of the world.

import { W, H, wrap, wrapDelta } from '../sim/util.js';

export const OBS_VERSION = 2;

// One decision every DECISION_DT seconds of game time.
export const DECISION_DT = 0.25;
// The trainer caps an episode at this much game time; `time` is scaled by it.
export const EPISODE_SECONDS = 120;
// StoneAge.onBlink refuses a bolt sooner than this after the previous one.
export const BOLT_COOLDOWN = 0.5;

export const ACTIONS = ['wait', 'strike_fire', 'strike_mammoth', 'calm', 'strike_random'];

export const OBS_NAMES = [
  'fire_lit',        //  0  1 when the pit burns, else 0
  'fire_heat',       //  1  fire.heat clamped to 0..1
  'food',            //  2  food / 10 clamped to 0..1.5
  'alive',           //  3  living people (all continents, rafts included) / 15 clamped to 0..2
  'hunger',          //  4  mean hunger of the living (0 if none)
  'starving',        //  5  share of the living with hunger >= 0.9
  'fear',            //  6  mean fear of the living
  'hold_active',     //  7  1 while the fire focus challenge runs (simulated scenes accept calm)
  'hold_progress',   //  8  hold.progress / 5 clamped to 0..1
  'holds',           //  9  hold.successes / 2 clamped to 0..1
  'mammoth_dist',    // 10  wrapped distance fire -> nearest living mammoth / 100, clamped 0..1; 1 if none
  'mammoths',        // 11  living mammoths / 4 clamped to 0..1
  'colonies',        // 12  colonies founded / (continents - 1) clamped to 0..1
  'rafts',           // 13  rafts boarding or sailing / 2 clamped to 0..1
  'overseas',        // 14  share of the living settled on another continent
  'time',            // 15  scene time / 120 clamped to 0..1
  'calm',            // 16  brain.calm clamped to 0..1 (the AI's calm action; simulated focus fallback)
  'calm_margin',     // 17  brain.calm - brain.threshold('calm', 0.55), clamped to -1..1
  'bolt_ready',      // 18  1 if a bolt now would not be refused by the cooldown
  'meat',            // 19  sum of uncollected meat amounts / 10 clamped to 0..1
];
export const OBS_SIZE = OBS_NAMES.length;

const cap = (v, lo, hi) => (v < lo ? lo : v > hi ? hi : v);

// Shorter separation on a torus `w` x `h` pixels round.
export const wrapDx = (dx, w = W) => wrapDelta(dx, 0, w);
export const wrapX = (x, w = W) => wrap(x, w);
export const planetDist = (ax, ay, bx, by, w = W, h = H) => Math.hypot(wrapDelta(ax, bx, w), wrapDelta(ay, by, h));
const sizeOf = (scene) => [scene.G?.world?.width || W, scene.G?.world?.height || H];

export function calmThreshold(G) {
  return G.brain?.threshold ? G.brain.threshold('calm', 0.55) : 0.55;
}

export function nearestMammoth(scene) {
  const f = scene.fire;
  const [w, h] = sizeOf(scene);
  let best = null, bd = Infinity;
  for (const m of scene.mammoths || []) {
    if (!m.alive) continue;
    const d = planetDist(m.x, m.y, f.x, f.y, w, h);
    if (d < bd) { bd = d; best = m; }
  }
  return best ? { m: best, d: bd } : null;
}

// Observation vector (plain array of OBS_SIZE floats) for a running StoneAge scene.
// `lastBolt` (scene time of the AI's own latest bolt) is passed by hosts that keep the
// AI's lightning cooldown apart from the human's; the trainer leaves it out.
export function observe(scene, G, { lastBolt = -Infinity } = {}) {
  const f = scene.fire;
  const alive = scene.living();
  const n = alive.length;
  let hunger = 0, fear = 0, starving = 0, overseas = 0;
  for (const p of alive) {
    hunger += p.hunger; fear += p.fear;
    if (p.hunger >= 0.9) starving++;
    if (p.colony) overseas++;
  }
  const near = nearestMammoth(scene);
  let mammoths = 0;
  for (const m of scene.mammoths || []) if (m.alive) mammoths++;
  let meat = 0;
  for (const m of scene.meat || []) meat += Math.max(0, m.amount);
  let rafts = 0;
  for (const b of scene.boats || []) if (b.state === 'boarding' || b.state === 'sailing') rafts++;
  const otherContinents = Math.max(1, (G.world?.continents?.length || 3) - 1);
  const calm = cap(G.brain?.calm ?? 0, 0, 1);
  const last = Math.max(scene.lastBolt ?? -Infinity, lastBolt);
  return [
    f.lit ? 1 : 0,
    cap(f.heat, 0, 1),
    cap(scene.food / 10, 0, 1.5),
    cap(n / 15, 0, 2),
    n ? hunger / n : 0,
    n ? starving / n : 0,
    n ? fear / n : 0,
    scene.hold?.active ? 1 : 0,
    cap((scene.hold?.progress || 0) / 5, 0, 1),
    cap((scene.hold?.successes || 0) / 2, 0, 1),
    near ? cap(near.d / 100, 0, 1) : 1,
    cap(mammoths / 4, 0, 1),
    cap(Object.keys(scene.colonies || {}).length / otherContinents, 0, 1),
    cap(rafts / 2, 0, 1),
    n ? overseas / n : 0,
    cap(scene.t / EPISODE_SECONDS, 0, 1),
    calm,
    cap(calm - calmThreshold(G), -1, 1),
    scene.t - last >= BOLT_COOLDOWN && !scene.done ? 1 : 0,
    cap(meat / 10, 0, 1),
  ];
}

// World pixel -> the 0..1 aim StoneAge.onBlink expects (world.aimPoint rounds aim * size).
// Both axes wrap round the torus.
export const aimAt = (x, y, w = W, h = H) => ({ x: wrap(x, w) / w, y: wrap(y, h) / h });

// Turn a discrete action into what the god does this step:
//   aim: where to throw lightning (null = no bolt), calm: hold calm this step.
export function decodeAction(scene, action, rng = Math.random) {
  switch (action) {
    case 1: return { aim: aimAt(scene.fire.x, scene.fire.y, ...sizeOf(scene)), calm: false };
    case 2: {
      const near = nearestMammoth(scene);
      // StoneAge tests the bolt against (m.x, m.y - 2).
      return { aim: near ? aimAt(near.m.x, near.m.y - 2, ...sizeOf(scene)) : null, calm: false };
    }
    case 3: return { aim: null, calm: true };
    case 4: return { aim: { x: rng(), y: rng() }, calm: false }; // anywhere on the planet
    default: return { aim: null, calm: false };
  }
}

// Hand-written baseline god that reads only the observation vector, so it is directly
// comparable with the learned policy. Also the AI god's fallback when no policy loads.
export function scriptedAction(obs) {
  const lit = obs[0], food = obs[2], holdActive = obs[7], mammoths = obs[11], boltReady = obs[18], meat = obs[19];
  if (!lit) return boltReady ? 1 : 0;            // relight the pit
  if (holdActive) return 3;                      // flame fading: be calm
  if (food < 0.2 && meat < 0.1 && mammoths > 0 && boltReady) return 2; // larder empty: cook a mammoth
  return 0;
}
