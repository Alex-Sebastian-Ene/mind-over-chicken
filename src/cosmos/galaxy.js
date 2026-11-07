// The galaxy: your home planet plus three worlds born from what you were thinking in the
// first ten seconds of play. Each of those worlds listens mainly to ONE brain channel, so you
// steer different civilisations with different mental states:
//   calm  → harvests and growth        focus → research and era progress
//   stress → its goat market's booms and crashes    blink → storms and lightning
// Every world still feels a little of every channel. They run on their own in fast-forward
// (Stone Age → Space Age in about a minute); space-faring worlds send ships to settle barren ones.
//
// The rules every world follows live in planet-sim.js (pure, DOM-free). This file only
// orchestrates them: labels, the news log, ships, world generation and drawing.
//
// AI-god seam: give a planet `planet.god = { act(observation) { return { growth, research, stability } } }`
// (weights, any scale) and that policy chooses the world's effort split instead of its temperament.
// It only adds the civilisation's own decisions: the player's brain still applies on top at every
// level, and a god never reads or writes the player's input. See observe() in planet-sim.js.
//
// Surfaces (#68, completes #43): every non-home world also gets a vast surface (surface.js) with
// its own SimPlanet tribe played by the PPO AI god (src/ai/). The god's moves fill the seam above.
// The surface in view runs every frame; the others run in turns at a lower rate (SURFACE_*).

import { rand, pick, chance } from '../sim/util.js';
import { generateWorld } from './worldgen.js';
import { loading } from '../ui/loading.js';
import { label } from '../eras/stone.js';
import { SIM, TEMPERAMENT, ERAS, newPlanetState, brainEffects, stepPlanet, blinkPlanet, arriveShip, newMind, stepMind, logHistory, reachMilestone, trackCampaignPop } from './planet-sim.js';
import { PlanetSurface } from './surface.js';

export const ERA_NAMES = ['stone', 'farming', 'industry', 'space'];
export const ERA_LABEL = { stone: 'Stone Age', farming: 'Farming', industry: 'Industry', space: 'Space Age' };
const MAX_PLANETS = 10;              // home + three mind worlds + six made ones (#68: was 7)
const STAR = { x: 132, y: 92 };      // left of centre, so the galaxy panel doesn't hide the outer orbits
const ORBIT_FIRST = 30, ORBIT_MAX = 124, ORBIT_SQUASH = 0.42;
// New worlds' maps are set up one at a time, each taking at least this long, so every world
// gets its own moment (and the generator never builds several vast surfaces at once).
export const FORM_SECONDS = 4;
const ORBIT_STEP = (ORBIT_MAX - ORBIT_FIRST) / (MAX_PLANETS - 1);   // the outermost orbit stays on screen
// Surfaces off screen run in turns: at most SURFACE_PER_FRAME of them each frame, in steps of
// SURFACE_DT, catching up at most SURFACE_MAX_LAG seconds (beyond that they simply run slower).
const SURFACE_PER_FRAME = 2, SURFACE_DT = 0.125, SURFACE_MAX_LAG = 0.5, SURFACE_VIEW_DT = 1 / 30;
// The mind-read window comes from the calibration config (#54); read it live, not once.
const NAMES = ['Henhaven', 'Cluckturn', 'Roostia', 'Yolkos', 'Featherfall', 'Eggsodus', 'Pecktopia', 'Gizzardia', 'Coopernicus', 'Bok Prime', 'Omelettia', 'Wattle IV'];
const BIOMES = ['desert', 'ice', 'jungle', 'alien', 'temperate'];
const ATMOS = { temperate: [110, 175, 255], desert: [255, 190, 120], ice: [190, 225, 255], jungle: [120, 230, 170], alien: [210, 140, 255], forming: [150, 140, 130] };

// One world per brain channel. `score` normalises the first-10-second snapshot so channels compare.
// Presentation for each brain channel's world (the rules are in planet-sim.js).
export const CHANNELS = {
  calm: { name: 'Stillwater', biome: 'temperate', born: 'Born from your calm', label: 'Calm', color: '#59d8c8' },
  focus: { name: 'Lumen', biome: 'alien', born: 'Born from your focus', label: 'Focus', color: '#ffcf4a' },
  stress: { name: 'Thrum', biome: 'desert', born: 'Born from your stress', label: 'Stress', color: '#ff5a72' },
  blink: { name: 'Flicker', biome: 'ice', born: 'Born from your blinks', label: 'Blinks', color: '#c9d7ff' },
  mixed: { label: 'Everything', color: '#a3a6c4' },
};

let RustGalaxyClass = null;

export function setRustGalaxy(cls) {
  RustGalaxyClass = cls;
}

export function fmtPop(n) {
  if (n <= 0) return 'barren';
  if (n < 1000) return `${Math.round(n)}`;
  return `${(n / 1000).toFixed(n < 10000 ? 1 : 0)}k`;
}

export class Galaxy {
  constructor() {
    if (RustGalaxyClass) {
      try {
        this.wasm = new RustGalaxyClass(Math.floor(Math.random() * 100000));
      } catch (e) {
        this.wasm = null;
      }
    }
    this.planets = [];
    this.ships = [];
    this.log = [];
    this.time = 0;
    this.usedNames = new Set();
    this.mind = newMind();
    this.revealQueue = [];
    this.formQueue = [];   // worlds waiting for their turn to form: { p, biome, opts }
    this.formSlot = null;  // the one forming now: { p, biome, opts, world, failed }
    this.formBatch = { done: 0, total: 0 };
    this.recentBlinks = [];
    this.attended = null;  // the world the player is zoomed into: hears every channel, blinks strike it
    this.viewing = null;   // the world whose surface is on screen (#68): its tribe runs every frame
    this.surfaceTurn = 0;
    this.onEvent = null;   // (planet, text, kind) → main.js forwards to the advisor
  }

  addHome(world, { autonomous = false } = {}) {
    const p = this.add({ name: 'Home', home: true, biome: world.biome || 'temperate', autonomous, world: null, homeWorld: world, eraIndex: 0, pop: autonomous ? 10 : 0, size: 12, channel: 'mixed', alive: true, seed: world.seed });
    // A sandbox home runs on its own, on a mirror of the same world (built off-thread with its
    // vast surface), so the paused game's map is untouched.
    if (autonomous) {
      p.forming = true; p.alive = false;
      generateWorld(world.seed, p.biome, { vast: true }).then((w) => { p.world = w; p.forming = false; p.alive = true; this.attachSurface(p, w); });
    }
    return p;
  }

  // Three worlds that will be shaped by the player's mind once the snapshot is in.
  addMindWorlds() {
    for (let i = 0; i < 3; i++) this.add({ name: 'Forming…', forming: true, mindSlot: true, biome: 'forming', size: [11, 10, 9][i], pop: 0, eraIndex: 0, channel: null, alive: false });
  }

  makePlanet(opts = {}) {
    if (this.planets.length >= MAX_PLANETS) return null;
    const biome = opts.biome || pick(BIOMES.filter((b) => !this.planets.some((p) => p.biome === b)).concat(BIOMES));
    let name = opts.name;
    if (!name) { const free = NAMES.filter((n) => !this.usedNames.has(n)); name = pick(free.length ? free : NAMES); }
    const inhabited = opts.inhabited ?? chance(0.6);
    const p = this.add({ name, biome: 'forming', size: opts.size ?? Math.round(rand(7, 10)), pop: 0, eraIndex: 0, channel: opts.channel || 'mixed', forming: true, alive: false });
    this.form(p, biome, { pop: inhabited ? (opts.pop ?? 10) : 0, eraIndex: opts.eraIndex ?? 0, born: inhabited ? 'new tribe' : 'barren' });
    return p;
  }

  add(o) {
    const i = this.planets.length;
    const seed = o.seed ?? ((Math.random() * 1e9) | 0);
    const p = {
      ...newPlanetState({ channel: o.channel || 'mixed', pop: o.pop || 0, eraIndex: o.eraIndex || 0 }),
      // presentation only
      labels: [], layers: {}, structs: {}, flash: 0, effect: '', surface: null, surfaceReady: false,
      // #138: neighbouring orbits are ~10 px apart and planets ~20 px across, so random phases
      // often started worlds in an overlapping clump. Spread them by the golden angle from the
      // previous world's current position instead (plus a little jitter).
      orbit: ORBIT_FIRST + i * ORBIT_STEP, phase: this.spreadPhase(i), speed: 0.2 / (1 + i * 0.45), spin: rand(0.25, 0.5),
      seed,
      ...o,
    };
    p.atmosphere = ATMOS[p.biome] || ATMOS.temperate;
    if (p.name) this.usedNames.add(p.name);
    this.planets.push(p);
    return p;
  }

  // Queue the planet's world to form. Worlds form one at a time (stepForming): each one's map
  // (its world and vast surface) is generated off-thread when its turn comes, and it comes to
  // life after FORM_SECONDS or when the map is ready, whichever is later.
  form(p, biome, { pop, eraIndex = 0, born }) {
    p.forming = true;
    p.formPending = true;
    p.formQueued = true;
    p.formT = 0;
    this.formQueue.push({ p, biome, opts: { pop, eraIndex, born } });
    this.formBatch.total++;
    this.showFormPill();
  }

  stepForming(dt) {
    let job = this.formSlot;
    if (!job) {
      job = this.formSlot = this.formQueue.shift() || null;
      if (!job) return;
      job.p.formQueued = false;
      job.p.formT = 0;
      generateWorld(job.p.seed ?? ((Math.random() * 1e9) | 0), job.biome, { vast: true, track: false })
        .then((w) => { job.world = w; }, (err) => { job.failed = err || new Error('world generation failed'); });
      this.showFormPill();
    }
    const p = job.p;
    p.formT = Math.min(FORM_SECONDS, (p.formT || 0) + Math.max(0, dt || 0));
    if (p.formT < FORM_SECONDS || !(job.world || job.failed)) return;
    this.formSlot = null;
    this.formBatch.done++;
    p.formPending = false;
    if (job.world) this.finishForm(p, job.biome, job.opts, job.world);
    else { p.forming = false; console.error('planet failed to form', job.failed); }
    this.showFormPill();
  }

  // Forming progress, 0..1 (0 while still waiting its turn).
  formProgress(p) { return p.forming && p.formPending && !p.formQueued ? (p.formT || 0) / FORM_SECONDS : 0; }

  // The corner pill: "Forming Thrum · 2 of 3", then "3 planets formed".
  showFormPill() {
    const b = this.formBatch;
    const cur = this.formSlot?.p;
    if (cur) loading.start('planets-forming', `Forming ${cur.name} · ${b.done + 1} of ${b.total}`, { doneText: b.total > 1 ? `${b.total} planets formed` : 'Planet formed' });
    else if (!this.formQueue.length && b.total) { loading.done('planets-forming'); this.formBatch = { done: 0, total: 0 }; }
  }

  // The world's map is ready and its turn is up: bring it to life.
  finishForm(p, biome, { pop, eraIndex = 0, born }, world) {
    p.world = world; p.biome = biome; p.atmosphere = ATMOS[biome] || ATMOS.temperate;
    p.forming = false; p.alive = true; p.pop = pop; p.eraIndex = eraIndex; p.progress = 0;
    p.labels.push({ text: born, t: 4, color: CHANNELS[p.channel]?.color || '#9be38a' });
    logHistory(p, pop > 0 ? `Born: ${born.toLowerCase()}` : 'Formed, barren', 'event');
    this.attachSurface(p, world);
    this.note(p, pop > 0 ? `${p.name}: ${born.toLowerCase()}.` : `${p.name} formed. Nobody lives there yet.`);
    this.onEvent?.(p, born, 'born');
  }

  // The world's surface up close, with its AI-god tribe once anyone lives there (#68).
  attachSurface(p, world) {
    if (!world?.vast || p.surface) return;
    try { new PlanetSurface(p, world); } catch (err) { console.error('surface failed', err); }
  }

  // Run the surfaces: the one in view every frame, the rest in turns (cheap proxy brains,
  // lower tick rate). The player's brain rides on top of whichever world they attend.
  tickSurfaces(dt, brain) {
    const list = this.planets.filter((p) => p.surface);
    if (!list.length) return;
    for (const p of list) {
      const s = p.surface;
      if (p.pop > 0 && !s.sim) s.ensureTribe();
      s.setHuman(p === this.attended ? brain : null);
      s.acc = Math.min(SURFACE_MAX_LAG, s.acc + dt);
    }
    const run = (p, step, inView) => {
      const s = p.surface;
      let left = s.acc;
      s.acc = 0;
      while (left > 1e-4) { const h = Math.min(step, left); left -= h; for (const ev of s.tick(h, inView)) this.surfaceEvent(p, ev); }
    };
    const view = this.viewing?.surface ? this.viewing : null;
    if (view) run(view, SURFACE_VIEW_DT, true);
    const others = list.filter((p) => p !== view);
    let done = 0;
    for (let k = 0; k < others.length && done < SURFACE_PER_FRAME; k++) {
      const p = others[(this.surfaceTurn + k) % others.length];
      if (p.surface.acc < SURFACE_DT) continue;
      run(p, SURFACE_DT, false);
      done++;
    }
    this.surfaceTurn = (this.surfaceTurn + Math.max(1, done)) % Math.max(1, others.length);
    // Settlement rebakes (they also feed the globe textures): at most one per frame.
    for (const p of list) if (p.surface.maybeRebake(dt)) break;
  }

  surfaceEvent(p, ev) {
    const colour = ev.kind === 'good' || ev.kind === 'milestone' ? '#ffd166' : '#ff8a9c';
    this.pushLabel(p, { text: ev.text, t: 3, color: colour });
    if (ev.kind !== 'milestone') this.note(p, `${p.name}: ${ev.text}${ev.detail ? ` (${ev.detail})` : ''}.`);
  }

  pushLabel(p, l) {
    p.labels.push(l);
    if (p.labels.length > 4) p.labels.splice(0, p.labels.length - 4);
  }

  note(p, text) {
    this.log.unshift({ text, t: this.time });
    if (this.log.length > 6) this.log.pop();
  }

  get home() { return this.planets.find((p) => p.home); }
  get full() { return this.planets.length >= MAX_PLANETS; }
  get mindReady() { return this.mind.done; }

  // Start angle for the i-th world: the golden angle (~137.5 degrees) past the previous one's
  // current angle, so a batch of new worlds fans out around the star.
  spreadPhase(i) {
    const prev = this.planets[i - 1];
    const base = prev ? prev.phase + this.time * prev.speed : rand(0, Math.PI * 2);
    const speed = 0.2 / (1 + i * 0.45);
    return base + 2.39996 + rand(-0.25, 0.25) - this.time * speed;
  }

  pos(p) {
    const a = p.phase + this.time * p.speed;
    return { x: STAR.x + Math.cos(a) * p.orbit, y: STAR.y + Math.sin(a) * p.orbit * ORBIT_SQUASH, behind: Math.sin(a) < 0 };
  }

  hit(x, y) {
    let best = null, bd = Infinity;
    for (const p of this.planets) {
      const q = this.pos(p);
      const d = Math.hypot(q.x - x, q.y - y);
      if (d < p.size + 5 && d < bd) { best = p; bd = d; }
    }
    return best;
  }

  // Called for every player blink: storms on the blink world.
  onBlink() {
    if (this.wasm) this.wasm.on_blink();
    this.recentBlinks.push(this.time);
    for (const p of this.planets) {
      const attended = p === this.attended;
      if (attended && p.simPlanet) {
        p.simPlanet.humanBlink({ x: 0.5, y: 0.5 });
      }
      if (p.home && !p.autonomous) {
        // The hand-played home takes its lightning on the surface; at globe level it still flashes.
        if (attended) { p.flash = 0.6; p.labels.push({ text: 'lightning strike!', t: 2, color: '#ffe066' }); }
        continue;
      }
      if (!p.alive) continue;
      const ev = blinkPlanet(p, Math.random, attended);
      if (!ev) continue;
      p.flash = 0.6;
      p.labels.push({ text: ev.text, t: 2, color: attended ? '#ffe066' : '#c9d7ff' });
      if (ev.kind === 'storm' && p.storms % 4 === 1) this.note(p, `${p.name}: a storm from your blink. The lightning sparked an idea.`);
    }
  }

  // home: { eraIndex, pop, canLaunch } read from the hand-played game each frame.
  // playDt: seconds of real play this frame (0 while paused), used for the mind snapshot.
  // opts.anger: the camera mood's anger (0..1), which unsettles the world you're looking at.
  update(dt, brain, home, playDt = dt, opts = {}) {
    this.time += dt;
    this.humanBrain = brain;
    this.recentBlinks = this.recentBlinks.filter((t) => this.time - t < 10);
    const fx = brainEffects({
      calm: brain.calm,
      focus: brain.focus,
      stress: brain.stress,           // EEG agitation or face anger, from the brain
      blinksLast10s: this.recentBlinks.length,
      anger: opts.anger ?? 0,
    });
    this.fx = fx;
    if (this.wasm) {
      this.wasm.step(dt, brain.calm || 0, brain.focus || 0, brain.stress || 0, this.recentBlinks.length, brain.blinks || 0);
      const attIdx = this.attended ? this.planets.indexOf(this.attended) : -1;
      this.wasm.set_attended(attIdx);
      const count = this.wasm.planet_count();
      for (let i = 0; i < count && i < this.planets.length; i++) {
        const p = this.planets[i];
        if (p.mindSlot && p.forming && !p.formPending && !this.wasm.planet_is_forming(i)) {
          p.forming = false;
          p.alive = true;
          p.name = this.wasm.planet_name(i);
          p.channel = this.wasm.planet_channel(i);
          p.biome = this.wasm.planet_biome(i);
          this.note(p, `A new world emerged from your ${p.channel}: behold ${p.name}!`);
        }
        if (!p.home) {
          p.pop = this.wasm.planet_pop(i);
          p.eraIndex = this.wasm.planet_era(i);
          p.progress = this.wasm.planet_progress(i);
          p.market = this.wasm.planet_market(i);
        }
      }
    }
    this.sampleMind(playDt, brain, fx);
    this.stepForming(dt);

    for (const p of this.planets) {
      p.labels = p.labels.filter((l) => (l.t -= dt) > 0);
      p.flash = Math.max(0, p.flash - dt);
      if (p.home && !p.autonomous) {
        if (home) {
          if (home.eraIndex > p.eraIndex) {   // the campaign's own history, for the panel
            const era = ERAS[home.eraIndex];
            if (era) { logHistory(p, `Reached ${era.name}`, 'event'); reachMilestone(p, era.key); }
          }
          trackCampaignPop(p, home);   // never "barren" through a time-skip; the civilisation's size (#105)
          p.eraIndex = home.eraIndex;
          if (home.canLaunch && (p.launchT += dt * (0.6 + fx.focus)) >= SIM.LAUNCH_EVERY) { p.launchT = 0; this.launch(p); }
        }
        p.effect = 'You play this one by hand';
        continue;
      }
      if (!p.alive || p.pop <= 0) { p.effect = p.forming ? 'Forming…' : 'Waiting for settlers'; continue; }
      this.simulate(p, dt, fx);
    }
    this.tickSurfaces(dt, brain);

    for (const s of this.ships) {
      s.t += dt / s.dur;
      if (s.t >= 1 && !s.done) {
        s.done = true;
        const to = s.to;
        if (arriveShip(to) === 'settled') {
          to.surface?.ensureTribe();
          to.labels.push({ text: 'settled!', t: 3.5, color: '#9be38a' });
          this.note(to, `Settlers from ${s.from.name} landed on ${to.name}.`);
          this.onEvent?.(to, 'settled', 'settled');
        } else {
          to.labels.push({ text: '+ colonists', t: 2.5, color: '#9be38a' });
        }
      }
    }
    this.ships = this.ships.filter((s) => !s.done);
  }

  // The first ten seconds of play decide what the three mind-worlds become.
  sampleMind(playDt, brain, fx) {
    const m = this.mind;
    if (!m.done) {
      const ranked = stepMind(m, playDt, fx, brain.stats?.blinks ?? 0);
      if (!ranked) return;
      this.planets.filter((p) => p.mindSlot).forEach((p, i) => this.revealQueue.push({ p, ch: ranked[i].ch }));
      this.revealT = 0;
      this.note(null, `Your first ${SIM.MIND_SECONDS} seconds: calm ${Math.round(m.calm * 100)}%, focus ${Math.round(m.focus * 100)}%, stress ${Math.round(m.stress * 100)}%, ${m.blinks} blinks.`);
      return;
    }
    // Staggered reveal so each world gets its moment.
    if (this.revealQueue.length && (this.revealT = (this.revealT ?? 0) - playDt) <= 0) {
      this.revealT = 1.2;
      const { p, ch } = this.revealQueue.shift();
      const c = CHANNELS[ch];
      p.channel = ch; p.name = c.name; p.temperament = TEMPERAMENT[ch]; this.usedNames.add(c.name);
      this.form(p, ch === 'calm' && m.calm > 0.5 ? 'jungle' : c.biome, { pop: 10, born: c.born });
    }
  }

  simulate(p, dt, fx) {
    if (p.simPlanet) {
      if (p === this.attended && this.humanBrain) {
        p.simPlanet.attachHuman(this.humanBrain);
      } else {
        p.simPlanet.attachHuman(null);
      }
      p.simPlanet.update(dt);
    }
    const { events, growth, research } = stepPlanet(p, dt, fx, Math.random, p === this.attended);
    const colour = { era: '#ffd166', good: '#59d8c8', bad: '#ff8a9c', flavour: '#f2e8d5', tier: '#9be38a', setback: '#ff8a9c', milestone: '#ffd166', conflict: '#ff5a72', truce: '#9be38a' };
    for (const ev of events) {
      if (ev.kind === 'launch-ready') { this.launch(p); continue; }
      const goodColour = ev.text === 'goat boom' ? '#7ee08a' : ev.text === 'breakthrough' ? '#ffcf4a' : colour.good;
      const badColour = ev.text === 'goat crash' ? '#ff5a72' : colour.bad;
      this.pushLabel(p, { text: ev.text, t: ev.kind === 'flavour' ? 2.6 : 3, color: ev.kind === 'good' ? goodColour : ev.kind === 'bad' ? badColour : colour[ev.kind] || '#f2e8d5' });
      if (ev.kind === 'era') this.note(p, `${p.name} reached the ${ev.text}.`);
      if (ev.kind === 'good' || ev.kind === 'bad') { this.note(p, `${p.name}: ${ev.text}, thanks to ${ev.cause}.`); this.onEvent?.(p, ev.text, 'event'); }
      if (ev.kind === 'tier') { this.note(p, `${p.name} grew into a ${ev.text}.`); this.onEvent?.(p, `grew into a ${ev.text}`, 'event'); }
      if (ev.kind === 'setback' || ev.kind === 'conflict' || ev.kind === 'truce') { this.note(p, `${p.name}: ${ev.text}${ev.detail ? ` (${ev.detail})` : ''}.`); this.onEvent?.(p, ev.text, 'event'); }
    }
    // Live "what your brain is doing to this world" line for the panel.
    p.effect = {
      calm: `Calm ${pct(fx.calm)} → growth ${signed(growth * 100, 1)}%/s`,
      focus: `Focus ${pct(fx.focus)} → research ×${research.toFixed(1)}`,
      stress: `Stress ${pct(fx.stress)} → goat index ${p.market.toFixed(0)}`,
      blink: `Blinks ${(fx.blinkRate * 10).toFixed(0)}/10s → ${p.storms} storms`,
      mixed: `Growth ${signed(growth * 100, 1)}%/s · research ×${research.toFixed(1)}`,
    }[p.channel || 'mixed'];
  }

  launch(p) {
    const barren = this.planets.filter((q) => q !== p && q.alive && q.pop <= 0 && !this.ships.some((s) => s.to === q));
    const lesser = this.planets.filter((q) => q !== p && q.alive && q.pop > 0 && q.eraIndex < p.eraIndex);
    const target = barren.length ? pick(barren) : (lesser.length && chance(SIM.LESSER_TARGET_CHANCE) ? pick(lesser) : null);
    if (!target) return;
    this.ships.push({ from: p, to: target, t: 0, dur: rand(SIM.SHIP_MIN, SIM.SHIP_MAX), bend: rand(-0.4, 0.4) });
    p.labels.push({ text: 'launch!', t: 2, color: '#ffd166' });
    this.note(p, `${p.name} launched a ship to ${target.name}.`);
  }

  // Texture for a planet's globe: its current era layer plus the structures to light at night.
  textureFor(p, globe, homeLayer, homeStructures) {
    if (p.home && !p.autonomous) return globe.texture(homeLayer, homeStructures, ERA_NAMES[p.eraIndex]);
    if (p.surface?.globeLayer) return globe.texture(p.surface.globeLayer, p.surface.structs, p.pop > 0 ? ERA_NAMES[p.eraIndex] : 'wild');
    if (!p.world) return null;
    const era = p.pop > 0 ? ERA_NAMES[p.eraIndex] : 'wild';
    if (!p.layers[era]) { p.layers[era] = p.world.paint(era); p.structs[era] = [...(p.world.structures || [])]; }
    return globe.texture(p.layers[era], p.structs[era], era);
  }

  shipPos(s) {
    const a = this.pos(s.from), b = this.pos(s.to);
    const t = s.t;
    const mx = (a.x + b.x) / 2 - (b.y - a.y) * s.bend, my = (a.y + b.y) / 2 + (b.x - a.x) * s.bend - 18;
    const u = 1 - t;
    return { x: u * u * a.x + 2 * u * t * mx + t * t * b.x, y: u * u * a.y + 2 * u * t * my + t * t * b.y };
  }

  // Pixel pass. `skip` = a planet drawn separately (the one zooming in/out).
  draw(ctx, globe, homeLayer, homeStructures, { alpha = 1, skip = null, hover = null } = {}) {
    ctx.globalAlpha = alpha;
    ctx.fillStyle = 'rgba(160,170,230,0.28)';
    for (const p of this.planets) {
      const steps = Math.round(p.orbit * 1.6);
      for (let k = 0; k < steps; k += 3) {
        const a = (k / steps) * Math.PI * 2;
        ctx.fillRect(Math.round(STAR.x + Math.cos(a) * p.orbit), Math.round(STAR.y + Math.sin(a) * p.orbit * ORBIT_SQUASH), 1, 1);
      }
    }
    ctx.globalAlpha = 1;
    // Back to front (#138): within each half of the orbit plane, nearer planets (lower on screen) draw last.
    const items = this.planets.map((p) => ({ p, q: this.pos(p) })).sort((a, b) => a.q.y - b.q.y);
    const drawPlanet = ({ p, q }) => {
      if (p === skip) return;
      this.drawPlanet(ctx, globe, p, q.x, q.y, p.size, homeLayer, homeStructures, alpha);
      if (p === hover) {
        ctx.globalAlpha = alpha;
        ctx.fillStyle = '#ffe066';
        const r = p.size + 4;
        for (let k = 0; k < 24; k += 2) { const a = (k / 24) * Math.PI * 2 + this.time; ctx.fillRect(Math.round(q.x + Math.cos(a) * r), Math.round(q.y + Math.sin(a) * r), 1, 1); }
        ctx.globalAlpha = 1;
      }
    };
    items.filter((it) => it.q.behind).forEach(drawPlanet);
    this.drawStar(ctx, alpha);
    items.filter((it) => !it.q.behind).forEach(drawPlanet);
    ctx.globalAlpha = alpha;
    for (const s of this.ships) {
      for (let k = 6; k >= 0; k--) {
        const sp = this.shipPos({ ...s, t: Math.max(0, s.t - k * 0.012) });
        ctx.fillStyle = k === 0 ? '#ffffff' : k < 3 ? '#ffd166' : 'rgba(255,140,60,0.6)';
        ctx.fillRect(Math.round(sp.x), Math.round(sp.y), 1, 1);
      }
    }
    ctx.globalAlpha = 1;
  }

  // One planet at any size: a globe, or a swirling protoplanet while it forms.
  drawPlanet(ctx, globe, p, x, y, r, homeLayer, homeStructures, alpha = 1, opts = {}) {
    const tex = this.textureFor(p, globe, homeLayer, homeStructures);
    if (!tex) { drawProto(ctx, x, y, r, this.time, alpha * (p.formQueued ? 0.55 : 1), this.formProgress(p)); return; }
    const L = opts.light || [STAR.x - x, STAR.y - y, 26];
    globe.draw(ctx, tex, x, y, r, { rot: opts.rot ?? this.time * p.spin + p.phase, light: L, alpha, tilt: opts.tilt ?? 0.3, atmosphere: p.atmosphere });
    if (p.flash > 0) drawPlanetBolt(ctx, x, y, r, p.storms, alpha * Math.min(1, p.flash * 2));
  }

  drawStar(ctx, alpha) {
    const t = this.time;
    ctx.globalAlpha = alpha;
    ctx.globalCompositeOperation = 'lighter';
    const g = ctx.createRadialGradient(STAR.x, STAR.y, 2, STAR.x, STAR.y, 26);
    g.addColorStop(0, 'rgba(255,210,120,0.55)'); g.addColorStop(1, 'rgba(255,160,60,0)');
    ctx.fillStyle = g; ctx.fillRect(STAR.x - 26, STAR.y - 26, 52, 52);
    ctx.globalCompositeOperation = 'source-over';
    const r = 7 + Math.round(Math.sin(t * 2) * 0.5);
    ctx.fillStyle = '#ffb347';
    ctx.beginPath(); ctx.arc(STAR.x, STAR.y, r, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = '#ffe9a8';
    ctx.beginPath(); ctx.arc(STAR.x - 1, STAR.y - 1, r - 2.5, 0, Math.PI * 2); ctx.fill();
    ctx.globalAlpha = 1;
  }

  // Crisp text pass in world units.
  drawOverlay(ctx, { alpha = 1, hover = null } = {}) {
    if (alpha < 0.05) return;
    ctx.globalAlpha = alpha;
    for (const p of this.planets) {
      const q = this.pos(p);
      const ch = CHANNELS[p.channel];
      label(ctx, q.x, q.y + p.size + 7, p.name, p === hover ? '#ffe066' : '#f2e8d5', 5);
      const sub = p.forming ? (p.mindSlot && !this.mind.done ? `reading your mind ${Math.max(0, Math.ceil(SIM.MIND_SECONDS - this.mind.t))}s`
        : p.formQueued ? 'waiting to form' : p.formPending ? `forming ${Math.max(1, Math.ceil(FORM_SECONDS - (p.formT || 0)))}s` : 'forming…')
        : p.pop > 0 ? `${ERA_LABEL[ERA_NAMES[p.eraIndex]]} · ${p.home && !p.autonomous ? `${p.pop} people` : fmtPop(p.pop)}` : 'barren';
      label(ctx, q.x, q.y + p.size + 12.5, sub, p.pop > 0 ? '#a3a6c4' : '#6f7394', 4);
      if (ch && !p.home && p.channel !== 'mixed' && p.alive) label(ctx, q.x, q.y + p.size + 17.5, `listens to ${ch.label.toLowerCase()}`, ch.color, 4);
      p.labels.forEach((l, i) => {
        ctx.globalAlpha = alpha * Math.min(1, l.t);
        label(ctx, q.x, q.y - p.size - 4 - i * 6 - (3 - Math.min(3, l.t)) * 2, l.text, l.color, 5);
        ctx.globalAlpha = alpha;
      });
    }
    ctx.globalAlpha = 1;
  }

  objectives() {
    const alive = this.planets.filter((p) => p.pop > 0);
    const spacefaring = alive.filter((p) => p.eraIndex === 3).length;
    const pop = this.planets.reduce((s, p) => s + Math.max(0, p.home && !p.autonomous ? 0 : p.pop), 0);
    return [
      { label: 'Worlds', value: `${this.planets.length}/${MAX_PLANETS}`, plain: true },
      { label: 'Mind read', value: this.mind.done ? 'yes' : `${Math.max(0, Math.ceil(SIM.MIND_SECONDS - this.mind.t))}s`, done: this.mind.done },
      { label: 'Space-faring', value: `${spacefaring}`, plain: true },
      { label: 'Population', value: fmtPop(pop), plain: true },
    ];
  }
}

function normalise(a) {
  const g = Math.max(0, a.growth ?? 0), r = Math.max(0, a.research ?? 0), s = Math.max(0, a.stability ?? 0);
  const t = g + r + s || 1;
  return { growth: g / t, research: r / t, stability: s / t };
}
const pct = (v) => `${Math.round(v * 100)}%`;
const signed = (v, d) => `${v >= 0 ? '+' : ''}${v.toFixed(d)}`;

// Swirling dust disc for a world that is still forming.
// `done` (0..1) is how far it has formed: the dust pulls in and spins faster while the core
// grows and warms, so the planet visibly condenses over its FORM_SECONDS.
function drawProto(ctx, x, y, r, t, alpha, done = 0) {
  const core = Math.max(2, r * (0.55 + 0.35 * done));
  ctx.globalAlpha = alpha * 0.9;
  ctx.fillStyle = done > 0.66 ? '#5a4a52' : done > 0.33 ? '#463c4a' : '#3a3440';
  ctx.beginPath(); ctx.arc(Math.round(x), Math.round(y), core, 0, Math.PI * 2); ctx.fill();
  if (done > 0) {   // a warm rim where the dust lands
    ctx.globalAlpha = alpha * 0.55 * done;
    ctx.strokeStyle = '#ffcf8a';
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.arc(Math.round(x), Math.round(y), core, 0, Math.PI * 2); ctx.stroke();
    ctx.globalAlpha = alpha * 0.9;
  }
  const n = Math.round(r * 5);
  const spread = 1.35 - 0.45 * done, spin = 1 + 1.5 * done;
  for (let k = 0; k < n; k++) {
    const h1 = ((k * 2654435761) >>> 0) / 4294967296, h2 = ((k * 40503 + 17) % 997) / 997;
    const rr = r * spread * Math.sqrt(h1);
    const a = h2 * Math.PI * 2 + t * spin * (0.6 + (1 - h1) * 1.4);
    ctx.fillStyle = k % 5 === 0 ? '#c9a27a' : k % 3 === 0 ? '#8a7f8f' : '#5f5666';
    ctx.fillRect(Math.round(x + Math.cos(a) * rr), Math.round(y + Math.sin(a) * rr * 0.8), 1, 1);
  }
  ctx.globalAlpha = 1;
}

// Storm / strike: a jagged pixel bolt across a planet's disc.
export function drawPlanetBolt(ctx, x, y, r, seed, alpha) {
  ctx.globalAlpha = alpha;
  ctx.fillStyle = '#ffffff';
  let bx = x - r * 0.3 + Math.sin(seed * 7) * r * 0.3, by = y - r * 0.8;
  const steps = Math.round(r * 1.4);
  for (let k = 0; k < steps; k++) {
    bx += (k % 3 === 0 ? 1 : 0) * (seed % 2 ? 1 : -1) * Math.max(1, r / 20);
    by += 1;
    ctx.fillRect(Math.round(bx), Math.round(by), Math.max(1, Math.round(r / 30)), 1);
  }
  ctx.globalAlpha = 1;
}

// Who is steering a world, for the panel.
export function ownerLabel(p) {
  if (p.home && !p.autonomous) return 'Your world';
  if (p.god) return 'AI god';
  return p.pop > 0 ? 'Unattended' : 'Nobody yet';
}

export function planetDot(p) {
  const c = p.atmosphere || ATMOS.forming;
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

export { STAR, MAX_PLANETS };
