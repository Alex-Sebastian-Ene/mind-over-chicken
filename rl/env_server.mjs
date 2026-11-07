#!/usr/bin/env node
// Headless Stone Age environment for reinforcement learning (simulation only).
//
// Builds the same SimPlanet the browser uses for AI-run worlds (src/ai/planet.js): the REAL
// game modules (src/eras/stone.js, src/world/terrain.js, src/sim/*) on a whole planet with
// all three continents, rafts and colonies, plus the AI's own brain (by default the real
// Brain fed by an emulated headset that belongs to the AI). A fake canvas stands in for the
// DOM. It speaks newline-delimited JSON on stdin/stdout:
//
//   {"cmd":"spec"}                        -> {obs_names, actions, decision_dt, max_seconds, reward, ...}
//   {"cmd":"reset","seed":7}              -> {obs, info}
//   {"cmd":"step","action":3}             -> {obs, reward, terminated, truncated, info, action}
//   {"cmd":"step","action":"scripted"}    -> same, the baseline god picks the action
//   {"cmd":"close"}                       -> exits
//
// Self-test / baselines without Python:
//   node rl/env_server.mjs --selftest --policy random|scripted|wait|json --episodes 5 [--brain sim|proxy]

import { mulberry32 } from './headless.mjs';
import { SimPlanet } from '../src/ai/planet.js';
import { scriptedAction, OBS_NAMES, OBS_VERSION, ACTIONS, DECISION_DT, EPISODE_SECONDS } from '../src/ai/observe.js';
import { MLPPolicy } from '../src/ai/policy.js';
import { createInterface } from 'node:readline';
import { readFileSync } from 'node:fs';

export const POP_TARGET = 15;   // StoneAge win: living >= 15 ...
export const CALM_TARGET = 2;   // ... and 2 successful calm holds, with the fire lit
export const SUBSTEPS = 4;      // 4 x 1/16 s of simulation per 0.25 s decision

// Reward design (documented in rl/README.md). Every term is something the player would
// call good or bad; the only shaping term is potential-based, so it cannot change which
// policy is optimal.
export const REWARD = {
  birth: 1.0,          // per baby while the tribe (before the step) is below the target of 15
  birthBeyond: 0.2,    // per baby once the target is met (no farming babies instead of winning)
  death: -1.5,         // per villager starved or trampled (G.stats.deaths)
  hold: 2.0,           // per successful calm hold, for the 2 the win needs
  holdBeyond: 0.5,     // per extra hold (keeps the fire roaring, but not the goal)
  smite: -0.1,         // per villager struck by lightning (G.stats.smitten)
  step: -0.01,         // per 0.25 s decision: win sooner rather than later
  win: 10,             // G.complete() was called
  lose: -10,           // G.fail() was called (<= 3 alive)
  // Potential Phi(s) = holdProgress * hold.progress (seconds) + holdCalm * clamp(brain.calm, 0, 1)
  // while a hold is active, else 0. Each step adds Phi(s') - Phi(s) (Ng et al. 1999), Phi = 0
  // on termination (win/lose), so a full successful hold is still worth exactly `hold`.
  holdProgress: 0.3,
  holdCalm: 0.3,
};

export class StoneEnv {
  constructor({ brain = 'sim', maxSeconds = EPISODE_SECONDS, substeps = SUBSTEPS } = {}) {
    this.brainMode = brain;
    this.maxSeconds = maxSeconds;
    this.substeps = substeps;
    this.planet = null;
    this.autoSeed = (Date.now() ^ (process.pid << 8)) >>> 0;
  }

  spec() {
    return {
      obs_names: OBS_NAMES, obs_size: OBS_NAMES.length, obs_version: OBS_VERSION,
      actions: ACTIONS, n_actions: ACTIONS.length,
      decision_dt: DECISION_DT, substeps: this.substeps, max_seconds: this.maxSeconds,
      brain: this.brainMode, reward: REWARD, pop_target: POP_TARGET, calm_target: CALM_TARGET,
    };
  }

  get scene() { return this.planet.scene; }
  get G() { return this.planet.G; }

  reset(seed) {
    this.seed = Number.isFinite(seed) ? seed >>> 0 : (this.autoSeed = (this.autoSeed * 1664525 + 1013904223) >>> 0);
    // The sim draws from Math.random; seeding it makes an episode replay exactly from its seed.
    Math.random = mulberry32(this.seed ^ 0x9e3779b9);
    this.planet = new SimPlanet({ seed: this.seed, brain: this.brainMode });
    this.steps = 0;
    this.ret = 0;
    this.prev = this.snapshot();
    this.phi = this.potential();
    this.obs = this.planet.observe();
    return { obs: this.obs, info: this.info() };
  }

  snapshot() {
    const s = this.scene, st = this.G.stats;
    return { births: st.births, deaths: st.deaths, smitten: st.smitten, holds: s.hold.successes, alive: s.living().length };
  }

  potential() {
    const h = this.scene.hold;
    if (!h.active) return 0;
    return REWARD.holdProgress * h.progress + REWARD.holdCalm * Math.min(1, Math.max(0, this.G.brain.calm));
  }

  step(action) {
    const planet = this.planet, scene = this.scene;
    if (action === 'scripted') action = scriptedAction(this.obs);
    action = Number(action) | 0;
    if (action < 0 || action >= ACTIONS.length) action = 0;
    planet.act(action, Math.random);
    const dt = DECISION_DT / this.substeps;
    for (let i = 0; i < this.substeps && !planet.outcome; i++) planet.advance(dt);
    this.steps++;

    const now = this.snapshot(), prev = this.prev;
    const births = now.births - prev.births;
    const towards = Math.min(births, Math.max(0, POP_TARGET - prev.alive));
    const holds = now.holds - prev.holds;
    const holdsTowards = Math.min(holds, Math.max(0, CALM_TARGET - prev.holds));
    let r = REWARD.step
      + REWARD.birth * towards + REWARD.birthBeyond * (births - towards)
      + REWARD.death * (now.deaths - prev.deaths)
      + REWARD.hold * holdsTowards + REWARD.holdBeyond * (holds - holdsTowards)
      + REWARD.smite * (now.smitten - prev.smitten);
    const terminated = planet.outcome != null;
    const phi = terminated ? 0 : this.potential();
    r += phi - this.phi;
    if (planet.outcome === 'win') r += REWARD.win;
    if (planet.outcome === 'lose') r += REWARD.lose;
    this.phi = phi;
    this.prev = now;
    this.ret += r;
    const truncated = !terminated && scene.t >= this.maxSeconds - 1e-9;
    this.obs = planet.observe();
    return { obs: this.obs, reward: r, terminated, truncated, info: this.info(), action };
  }

  info() {
    const s = this.scene, st = this.G.stats;
    const alive = s.living();
    return {
      seed: this.seed, t: +s.t.toFixed(3), steps: this.steps, ret: this.ret,
      alive: alive.length, births: st.births, deaths: st.deaths, holds: s.hold.successes,
      food: s.food, lit: s.fire.lit, colonies: Object.keys(s.colonies || {}).length,
      rafts: (s.boats || []).length, overseas: alive.filter((p) => p.colony).length,
      bolts: s.strikes, smitten: st.smitten, calm: this.G.brain.calm,
      outcome: this.planet.outcome, win: this.planet.outcome === 'win',
    };
  }
}

// ---------------- CLI ----------------

function parseArgs(argv) {
  const a = { brain: 'sim', maxSeconds: EPISODE_SECONDS, selftest: false, policy: 'random', episodes: 5, seed: 1000, policyFile: null, quiet: false };
  for (let i = 0; i < argv.length; i++) {
    const k = argv[i], v = argv[i + 1];
    if (k === '--brain') { a.brain = v; i++; }
    else if (k === '--max-seconds') { a.maxSeconds = Number(v); i++; }
    else if (k === '--selftest') a.selftest = true;
    else if (k === '--policy') { a.policy = v; i++; }
    else if (k === '--episodes') { a.episodes = Number(v); i++; }
    else if (k === '--seed') { a.seed = Number(v); i++; }
    else if (k === '--policy-file') { a.policyFile = v; i++; }
    else if (k === '--quiet') a.quiet = true;
  }
  return a;
}

function selftest(args) {
  const env = new StoneEnv({ brain: args.brain, maxSeconds: args.maxSeconds });
  let pick = Math.random;
  const policies = {
    random: () => Math.floor(pick() * ACTIONS.length),
    scripted: (obs) => scriptedAction(obs),
    wait: () => 0,
  };
  let check = null;
  if (args.policy === 'json') {
    // The exact runtime the browser uses (src/ai/policy.js), fed the exported weights.
    const file = args.policyFile || new URL('../src/ai/stone_policy.json', import.meta.url);
    const mlp = new MLPPolicy(JSON.parse(readFileSync(file, 'utf8')));
    check = mlp.selfCheck();
    policies.json = (obs) => mlp.act(obs);
  }
  const policy = policies[args.policy];
  if (!policy) throw new Error(`unknown policy ${args.policy}`);
  const rows = [];
  let steps = 0, bad = 0;
  const t0 = performance.now();
  for (let ep = 0; ep < args.episodes; ep++) {
    let { obs } = env.reset(args.seed + ep);
    pick = mulberry32(args.seed * 7 + ep); // the policy's own dice, independent of the sim's
    const counts = new Array(ACTIONS.length).fill(0);
    const lo = new Array(obs.length).fill(Infinity), hi = new Array(obs.length).fill(-Infinity);
    let done = false, res;
    while (!done) {
      const a = policy(obs);
      counts[a]++;
      res = env.step(a);
      obs = res.obs;
      obs.forEach((v, i) => { if (!Number.isFinite(v)) bad++; lo[i] = Math.min(lo[i], v); hi[i] = Math.max(hi[i], v); });
      if (!Number.isFinite(res.reward)) bad++;
      steps++;
      done = res.terminated || res.truncated;
    }
    const i = res.info;
    rows.push({ ep, seed: i.seed, outcome: i.outcome || 'timeout', t: i.t, ret: +i.ret.toFixed(2), alive: i.alive, births: i.births, deaths: i.deaths, holds: i.holds, bolts: i.bolts, smitten: i.smitten, colonies: i.colonies, actions: counts });
    if (ep === 0) rows[0].obsRange = Object.fromEntries(OBS_NAMES.map((n, k) => [n, [+lo[k].toFixed(3), +hi[k].toFixed(3)]]));
  }
  const secs = (performance.now() - t0) / 1000;
  const summary = {
    policy: args.policy, brain: args.brain, episodes: args.episodes, steps, seconds: +secs.toFixed(2),
    steps_per_sec: Math.round(steps / secs), game_seconds_per_sec: Math.round((steps * DECISION_DT) / secs),
    win_rate: rows.filter((r) => r.outcome === 'win').length / rows.length,
    lose_rate: rows.filter((r) => r.outcome === 'lose').length / rows.length,
    mean_return: +(rows.reduce((s, r) => s + r.ret, 0) / rows.length).toFixed(2),
    non_finite: bad,
  };
  if (check != null) summary.export_max_prob_diff = check;
  const wins = rows.filter((r) => r.outcome === 'win');
  summary.mean_win_seconds = wins.length ? +(wins.reduce((s, r) => s + r.t, 0) / wins.length).toFixed(2) : null;
  console.log(JSON.stringify(args.quiet ? { summary } : { summary, episodes: rows }, null, 1));
}

function serve(args) {
  const env = new StoneEnv({ brain: args.brain, maxSeconds: args.maxSeconds });
  const send = (o) => process.stdout.write(JSON.stringify(o) + '\n');
  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
  rl.on('line', (line) => {
    if (!line.trim()) return;
    let msg;
    try { msg = JSON.parse(line); } catch (e) { send({ error: `bad json: ${e.message}` }); return; }
    try {
      if (msg.cmd === 'step') send(env.step(msg.action));
      else if (msg.cmd === 'reset') {
        if (msg.options?.brain) env.brainMode = msg.options.brain;
        send(env.reset(msg.seed));
      }
      else if (msg.cmd === 'spec') send(env.spec());
      else if (msg.cmd === 'close') { send({ ok: true }); process.exit(0); }
      else send({ error: `unknown cmd ${msg.cmd}` });
    } catch (e) {
      send({ error: String(e && e.stack || e) });
    }
  });
  rl.on('close', () => process.exit(0));
}

const isMain = process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/\\/g, '/').split('/').pop());
if (isMain) {
  const args = parseArgs(process.argv.slice(2));
  if (args.selftest) selftest(args);
  else serve(args);
}
