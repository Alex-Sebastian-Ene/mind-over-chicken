// ERA I · STONE AGE
// Blink to throw lightning (light the fire, cook a mammoth, convert Grug).
// Hold Insight/Focus for 5 s when the flame fades. Keep the tribe fed until it grows.
// Calibration: calm cues are labelled `calm`, the rest of play is labelled `rest`.

import { Particles, clamp, dist, rand, chance, floater, pick } from '../sim/util.js';
import { walk, animate, villagerLook, sail } from '../sim/agents.js';
import { forestFire, forestFireStep } from './forest-fire.js';
import { eraTuning } from '../game/options.js'; // difficulty (#220)
import { combatEnabled, modifierEffect } from '../game/replay.js'; // challenge modifiers (#91)
import { drawPerson, drawMammoth, drawFire, makeBolt, drawBolt, drawReticle, drawChicken, drawBoat } from '../render/sprites.js';
import { FireballFX } from '../render/fireball.js';
import { joyBuff, surpriseFires, surpriseEffect, sparkleCount, JOY_COLORS } from '../camera/mood-actions.js'; // #152
import { reducedMotion } from '../ui/motion.js';

// Rain is a camera spell: an open palm held to the camera plus Insight/Focus. There is no
// keyboard fallback. `hand.at` is a performance.now() timestamp.
export function rainHandReady(hand, now = performance.now()) {
  return !!hand?.active && !!hand.palm && now - (Number(hand.at) || 0) <= 350;
}

const POP_START = 10;
const POP_TARGET = 15;
const FOCUS_TARGET = 2;
// 3D firelight strength at the pit (was 1.8, which read as a white glare around the fire).
const FIRE_GLOW = 0.9;
const HOLD_SECONDS = 5;
// The tribe's hunger clock (#298), before the difficulty's `hunger` and `starvationGrace`:
// hunger per second (0 → 1 takes ~110 s; assist mode ~200 s), the opening grace in which nobody
// starves, and how long someone at full hunger holds on before dying. Was 0.017 / 0.008, 75 s, 8 s.
export const STONE_HUNGER = Object.freeze({ rate: 0.009, assistRate: 0.005, grace: 120, starve: 12 });
const NAMES = ['Grug', 'Ugg-a', 'Thak', 'Mo', 'Brakka', 'Nub', 'Ila', 'Krog', 'Zuz', 'Hap', 'Wenna', 'Dorf', 'Yip', 'Ooma'];

export class StoneAge {
  constructor() {
    this.id = 'stone';
    this.era = 'stone';
    this.rustEra = 0;   // the Rust era sim under this scene (crates/engine/src/eras, #194)
    this.label = 'Era I · Stone Age';
    this.intro = {
      title: 'Stone Age',
      goal: `Grow the tribe to ${POP_TARGET} and keep the fire alive with ${FOCUS_TARGET} focus holds.`,
      how: [['Blink', 'Throw lightning where you look. Strike the fire pit to light it, strike a giant chicken for dinner, or strike a tree to start a forest fire.'], ['Focus', 'When the flame fades, hold Insight for 5 seconds.'], ['Rain', 'A forest fire needs rain: hold an open palm to the camera and hold Insight for two seconds.']],
    };
    this.powers = ['blink', 'focus'];
    this.darkness = 0.6;
  }

  enter(G) {
    this.G = G;
    const w = G.world;
    this.layer = w.paint('stone');
    this.particles = new Particles();
    this.fireballs = new FireballFX();
    this.t = 0;
    this.fire = { x: w.sites.fire.x, y: w.sites.fire.y, heat: 0, lit: false, everLit: false };
    this.forest = { fires: [], burned: 0, warned: false };
    this.rainHold = { progress: 0, active: 0, cooldown: 0, hintAt: -99 };
    this.food = 0;
    this.hold = { active: false, progress: 0, successes: 0, broke: 0 };
    this.bolts = [];
    this.flash = 0;
    this.lastBolt = -5;
    this.birthTimer = 0;
    this.spawnTimer = 0;
    this.meat = [];
    this.done = false;
    this.strikes = 0;
    this.grabbedEntity = null;
    this.boats = [];
    this.colonies = {};
    this.expTimer = 0;
    this.people = [];
    for (let i = 0; i < POP_START; i++) this.people.push(this.makePerson(w.randomLand(w.sites.cave.x, w.sites.cave.y, 2, 14), NAMES[i]));
    this.mammoths = [];
    for (let i = 0; i < 4; i++) this.spawnMammoth();
    const cp = w.nearestWalkable ? w.nearestWalkable(this.fire.x + 8, this.fire.y + 4) : { x: this.fire.x + 8, y: this.fire.y + 4 };
    this.chicken = { x: cp.x, y: cp.y, frame: 0, ft: 0, facing: 1, tx: cp.x, ty: cp.y, t: 0 };
    G.brain.setCue('rest');
    G.advisor.say('intro', 'My lord! The tribe is cold, hungry and quietly judging you. Blink to throw lightning at the fire pit.', { priority: 3, cooldown: 999 });
  }

  makePerson(p, name) {
    return { x: p.x, y: p.y, name: name || pick(NAMES), ...villagerLook('stone'), hunger: rand(0.15, 0.4), fear: rand(0.3, 0.5), faith: 0.1,
      state: 'idle', tx: p.x, ty: p.y, decide: rand(0, 0.5), starve: 0, alive: true, frame: 0, ft: rand(0, 4), side: chance(0.5) ? 1 : -1, facing: 1, carry: null, target: null, halo: false };
  }

  spawnMammoth() {
    const w = this.G.world, v = w.sites.village;
    const p = w.randomLand(v.x, v.y, 40, 85, (x, y) => w.walkable(x, y) && w.reachable(x, y));
    this.mammoths.push({ x: p.x, y: p.y, tx: p.x, ty: p.y, alive: true, frame: 0, ft: 0, facing: 1, hunt: 0, lone: 0, side: 1, pause: 0 });
  }

  spawnTutorialMammoth() {
    const w = this.G.world, v = w.sites.village;
    const p = w.randomLand(v.x, v.y, 16, 26, (x, y) => w.walkable(x, y) && w.reachable(x, y));
    const m = { x: p.x, y: p.y, tx: p.x, ty: p.y, alive: true, frame: 0, ft: 0, facing: 1, hunt: 0, lone: 0, side: 1, pause: 0, tutorial: true };
    this.mammoths.unshift(m);
    return m;
  }

  living() { return this.people.filter((p) => p.alive); }
  // Seconds of calm that feed the fire: 5 on Normal (the difficulty table, #220).
  holdSeconds() { return this.G ? eraTuning(this.G).holdSeconds : HOLD_SECONDS; }

  onBlink(aim) {
    const G = this.G;
    if (!combatEnabled(G) || this.t - this.lastBolt < modifierEffect(G, 'blink-storm', 'boltCooldown', 0.5) || this.done) return;
    this.lastBolt = this.t;
    const { x, y } = G.world.aimPoint(aim);   // aims are 0..1 of the world (640×360 at home, #129)
    this.bolts.push({ pts: makeBolt(x, y), life: 0.35 });
    this.flash = 0.35;
    this.strikes++;
    G.stats.lightning++;
    G.audio.thunder();
    this.particles.burst(14, () => ({ x, y, vx: rand(-30, 30), vy: rand(-40, 5), g: 60, life: 0.6, maxLife: 0.6, size: 1, color: pick(['#fff4c2', '#ffd166', '#ffffff']) }));

    // Fire pit & tutorial label (generous target area so clicking label or pit lights fire)
    const atPit = dist(x, y, this.fire.x, this.fire.y) < 24 || dist(x, y, this.fire.x, this.fire.y - 12) < 20;
    if (atPit) {
      const first = !this.fire.everLit;
      this.fire.lit = true; this.fire.everLit = true;
      this.fire.heat = Math.max(this.fire.heat, 0.7);
      G.audio.ignite();
      floater(this.particles, this.fire.x, this.fire.y - 10, first ? 'FIRE!' : 'Relit!', '#ffd166', 8);
      if (first) {
        G.advisor.say('fire', 'FIRE! You have invented fire! Now blink a giant chicken to feed the tribe.', { mood: 'happy', priority: 3, cooldown: 999 });
        this.spawnTutorialMammoth();
      } else {
        G.advisor.say('relit', 'The flame lives again. The tribe pretends it was never worried.', { mood: 'happy', priority: 2 });
      }
    }
    // Lightning on a tree starts a forest fire (#276), but not a strike that lit the campfire:
    // the trees beside the pit used to catch, and the forest fire then never stopped.
    const tree = atPit ? null : this.nearestTree(x, y, 9);
    if (tree) this.igniteForestFire(tree);

    // Mammoths (generous hit radius for lightning strike)
    for (const m of this.mammoths) {
      if (m.alive && dist(x, y, m.x, m.y - 2) < 18) {
        m.alive = false; m.dead = true;
        this.meat.push({ x: m.x, y: m.y, amount: 5, cooked: true, claimed: 0 });
        floater(this.particles, m.x, m.y - 8, 'barbecue', '#ff9a2e', 7);
        G.advisor.say('bbq', 'Roast chicken! Barbecue time. Hunters will carry the meat to the fire.', { mood: 'happy', priority: 2 });
      }
    }
    // People (lightning strikes kill people as well as animals)
    let smoteSomeone = false;
    for (const p of this.people) {
      if (!p.alive) continue;
      const d = dist(x, y, p.x, p.y - 2);
      if (d < 12) {
        if (G.invincible) {
          p.fear = Math.min(1, p.fear + 0.5);
          if (['idle', 'warm', 'dance', 'huddle', 'hunt'].includes(p.state)) { p.state = 'cower'; p.target = null; }
          continue;
        }
        p.alive = false;
        p.dead = true;
        p.deadT = 0;
        p.cause = 'smitten';
        G.stats.smitten = (G.stats.smitten || 0) + 1;
        G.stats.deaths = (G.stats.deaths || 0) + 1;
        this.particles.burst(20, () => ({
          x: p.x,
          y: p.y - 2,
          vx: rand(-25, 25),
          vy: rand(-30, 5),
          g: 45,
          life: 0.7,
          maxLife: 0.7,
          size: 1.5,
          color: pick(['#222222', '#444444', '#ff9a2e', '#ffe066', '#ffffff']),
        }));
        floater(this.particles, p.x, p.y - 8, `${p.name} SMITTEN!`, '#ff4444', 8);
        G.advisor.say('smite', `Direct hit on ${p.name}! Smitten to ashes. The tribe trembles before your divine wrath.`, { mood: 'panic', priority: 3 });
        smoteSomeone = true;
      } else if (d < 45) {
        p.fear = Math.min(1, p.fear + 0.4);
        p.faith = Math.min(1, p.faith + 0.2);
        if (['idle', 'warm', 'dance', 'huddle', 'hunt'].includes(p.state)) {
          p.state = 'cower';
          p.target = null;
        }
      }
    }
    // The scene has no fail() of its own: it fails through G, like the starvation check in
    // update(), so this works for the campaign and for an AI god's SimPlanet alike.
    if (smoteSomeone && this.living().length === 0 && !this.done) {
      this.done = true;
      G.fail?.('Your divine lightning smote every single member of the tribe.');
    }
    if (this.strikes > 6 && this.t < 20) G.advisor.say('spam', 'With respect, my lord, lightning is not a fidget toy.', { priority: 1, cooldown: 30 });
  }

  nearestTree(x, y, maxDist = 16) {
    let best = null, bestD = maxDist;
    for (const t of this.G.world.trees || []) {
      const d = dist(x, y, t.x, t.y);
      if (d <= bestD) { bestD = d; best = t; }
    }
    return best;
  }

  igniteForestFire(tree, gen = 0) {
    const w = this.G.world;
    if (!tree || !w.trees || this.forest.fires.length >= 40) return false;
    const i = w.trees.indexOf(tree);
    if (i < 0) return false;
    w.trees.splice(i, 1);
    this.forest.fires.push(forestFire(tree.x, tree.y, gen));
    this.forest.burned++;
    floater(this.particles, tree.x, tree.y - 8, 'FOREST FIRE!', '#ff9a2e', 8);
    this.G.audio?.thunder?.();
    if (!this.forest.warned) {
      this.forest.warned = true;
      this.G.advisor.say('forest-fire', 'The forest is on fire! Hold an open palm to the camera and Insight for two seconds to summon rain and put it out!', { mood: 'panic', priority: 3, cooldown: 999 });
    }
    return true;
  }

  updateForest(dt) {
    const fires = [...this.forest.fires];
    for (const f of fires) {
      // Each burning tree spreads a little, less each generation, then burns out (forest-fire.js).
      if (forestFireStep(f, dt)) {
        const tree = this.nearestTree(f.x, f.y, 16);
        if (tree) this.igniteForestFire(tree, f.gen + 1);
      }
      if (f.heat > 0) {
        if (Math.random() < dt * 18 * f.heat) this.particles.add({ x: f.x + rand(-2, 2), y: f.y - 4, vx: rand(-4, 4), vy: rand(-16, -6), life: rand(0.4, 0.9), maxLife: 1, size: 1, color: pick(['#ffd166', '#ff9a2e', '#e8452c']) });
        if (Math.random() < dt * 4) this.particles.add({ x: f.x, y: f.y - 8, vx: rand(-2, 2), vy: -7, life: 2, maxLife: 2, size: 1, color: 'rgba(90,90,100,0.55)' });
        for (const p of this.people) {
          if (!p.alive || p.colony || p.state === 'aboard') continue;
          const d = dist(p.x, p.y, f.x, f.y);
          if (d < 20) {
            p.fear = clamp(p.fear + dt * 0.6);
            if (d < 12 && ['idle', 'warm', 'dance', 'huddle', 'hunt'].includes(p.state)) { p.state = 'flee'; p.target = null; }
          }
        }
      }
    }
    this.forest.fires = this.forest.fires.filter((f) => f.heat > 0);
  }

  douseFires() {
    const hadForest = this.forest.fires.length > 0;
    this.forest.fires = [];
    if (this.fire.lit) {
      this.fire.lit = false;
      this.fire.heat = 0;
      if (this.hold.active) this.endHold(false);
    }
    return hadForest;
  }

  startRain() {
    const r = this.rainHold;
    const hadForest = this.douseFires();
    r.progress = 0;
    r.active = 2.5;
    r.cooldown = 8;
    const G = this.G;
    this.castRain(this.fire.x, this.fire.y - 40);
    for (const p of this.people) if (p.alive) p.fear = clamp(p.fear - 0.15);
    floater(this.particles, this.fire.x, this.fire.y - 30, hadForest ? 'rain puts the forest fire out' : 'rain spell', '#9ec9ff', 8);
    G.audio?.good?.();
    G.advisor.say('rain-summoned', hadForest
      ? 'Rain spell! The forest fire is out. The tribe is damp, alive and quietly impressed.'
      : 'Rain falls over the tribe. No fire to put out, but the chickens seem pleased.', { mood: 'happy', priority: 3, cooldown: 10 });
  }

  updateRain(dt) {
    const r = this.rainHold;
    r.cooldown = Math.max(0, r.cooldown - dt);
    if (r.active > 0) {
      r.active -= dt;
      if (this.forest.fires.length || this.fire.lit) this.douseFires();
      if (Math.random() < dt * 50) {
        const c = this.G.camera?.viewCenter?.() || this.fire;
        this.particles.add({ x: c.x + rand(-70, 70), y: c.y + rand(-60, 20), vx: -4, vy: rand(70, 100), life: rand(0.5, 1), maxLife: 1, size: 1, color: 'rgba(158,201,255,0.75)' });
      }
      r.progress = 0;
      return;
    }
    if (r.cooldown > 0) { r.progress = 0; return; }
    const brain = this.G.brain;
    const held = this.G.source?.input?.focus ? 1 : 0;
    const level = Math.max(brain.focus ?? 0, held);
    const thr = brain.threshold ? brain.threshold('focus', 0.5) : 0.5;
    // Rain is a camera spell: a held open palm is required every frame, plus Insight.
    if (!rainHandReady(this.G.handInput, performance.now())) {
      if (level >= thr && this.t - r.hintAt > 8) {
        r.hintAt = this.t;
        this.G.advisor?.say('rain-palm', 'Rain is a camera spell, my lord: hold an open palm to the camera with Insight.', { priority: 2, cooldown: 8 });
      }
      r.progress = Math.max(0, r.progress - dt * 2);
      return;
    }
    r.progress = level >= thr ? r.progress + dt : Math.max(0, r.progress - dt * 1.5);
    if (r.progress >= 2) this.startRain();
  }

  update(dt) {
    const G = this.G, w = G.world, brain = G.brain;
    this.t += dt;
    for (const hit of this.fireballs.update(dt)) this.impactFireball(hit);
    this.particles.update(dt);
    this.bolts = this.bolts.filter((b) => (b.life -= dt) > 0);
    this.flash = Math.max(0, this.flash - dt);
    this.joyStep(dt);   // #152: the joy buff, every frame while you smile

    // ---- Fire and the focus/insight challenge ----
    const f = this.fire;
    if (f.lit) {
      const decayMult = (this.G.assistMode ? 0.5 : 1.0) * eraTuning(this.G).fireDecay * modifierEffect(this.G, 'fragile-fire', 'fireDecay', 1);
      f.heat -= dt * (this.hold.active ? 0.022 : 0.034) * decayMult;
      if (f.heat <= 0) {
        f.heat = 0; f.lit = false;
        if (this.hold.active) this.endHold(false);
        G.audio.bad();
        G.advisor.say('out', 'The fire is out! Darkness! Blink at the pit to relight it, my lord.', { mood: 'panic', priority: 3 });
      }
      if (Math.random() < dt * 14 * f.heat) this.particles.add({ x: f.x + rand(-1, 1), y: f.y - 4, vx: rand(-3, 3), vy: rand(-14, -6), life: rand(0.5, 1.1), maxLife: 1, size: 1, color: pick(['#ffd166', '#ff9a2e', '#e8452c']) });
      if (Math.random() < dt * 3) this.particles.add({ x: f.x, y: f.y - 6, vx: rand(-2, 2), vy: -6, life: 2, maxLife: 2, size: 1, color: 'rgba(120,120,130,0.6)' });
    }
    if (f.lit && !this.hold.active && f.heat < 0.5 && !this.done) {
      this.hold.active = true; this.hold.progress = 0;
      brain.setCue('focus');
      G.advisor.say('focus', 'The flame is fading. Hold your focus, my lord. Keep your palm closed unless you mean rain.', { priority: 3, cooldown: 0 });
      G.audio.blip();
    }
    if (this.hold.active) {
      const tuning = eraTuning(G);
      const thr = brain.threshold('focus', tuning.calmThreshold);
      // The AI god was trained on calm; its calm action still rekindles, but the player's
      // challenge is Insight/Focus.
      const focusLevel = Math.max(brain.focus ?? 0, this.G.source?.input?.focus ? 1 : 0);
      const calmFallback = (this.simulated || this.G.aiGodActive) ? (brain.calm ?? 0) : 0;
      const calmThr = brain.threshold ? brain.threshold('calm', tuning.calmThreshold) : tuning.calmThreshold;
      this.hold.thr = thr;
      if (focusLevel >= thr || calmFallback >= calmThr) this.hold.progress += dt;
      else {
        if (this.hold.progress > 1.5) G.advisor.say('broke', 'Your focus slipped. The fire slipped with it.', { priority: 2, cooldown: 10 });
        this.hold.progress = Math.max(0, this.hold.progress - dt * 1.5);
      }
      if (this.hold.progress >= this.holdSeconds()) this.endHold(true);
    }

    this.updateForest(dt);
    this.updateRain(dt);

    // ---- Mammoths ----
    this.spawnTimer += dt;
    if (this.mammoths.filter((m) => m.alive).length < 3 && this.spawnTimer > 6) { this.spawnTimer = 0; this.spawnMammoth(); }
    for (const m of this.mammoths) {
      if (!m.alive) continue;
      // Invariant: mammoths must never stand or walk on water
      if (w.isWater && (w.isWater(m.x, m.y) || !w.walkable(m.x, m.y))) {
        const safe = w.nearestWalkable(m.x, m.y);
        m.x = safe.x; m.y = safe.y;
        m.tx = safe.x; m.ty = safe.y;
      }
      animate(m, dt, 3);
      m.pause -= dt;
      const arrived = walk(m, m.tx, m.ty, 4, dt, w);
      if (m.pause <= 0 && (arrived || (m.stuck || 0) > 1.5)) {
        m.stuck = 0;
        const p = w.randomLand(m.x, m.y, 8, 26, (x, y) => w.walkable(x, y) && w.reachable(x, y));
        m.tx = p.x; m.ty = p.y; m.pause = rand(0.5, 2.5);
      }
      const hunters = this.people.filter((p) => p.alive && p.target === m && dist(p.x, p.y, m.x, m.y) < 6).length;
      if (hunters >= 2) { m.hunt += dt * hunters; m.pause = 0.5; }
      else if (hunters === 1) {
        m.lone += dt;
        if (m.lone > 4) {
          m.lone = 0;
          const h = this.people.find((p) => p.alive && p.target === m);
          if (h) { h.fear = 1; h.state = 'flee'; h.target = null; floater(this.particles, h.x, h.y - 7, 'nope', '#f2e8d5', 6); }
        }
      }
      if (m.hunt > 2.6) {
        m.alive = false; m.dead = true;
        this.meat.push({ x: m.x, y: m.y, amount: 4, cooked: false, claimed: 0 });
        floater(this.particles, m.x, m.y - 8, 'hunted!', '#f2e8d5', 6);
        if (chance(Math.min(0.9, 0.2 * eraTuning(G).mammothDanger))) {
          const victim = this.people.find((p) => p.alive && p.target === m);
          if (victim) { this.kill(victim); G.advisor.say('trample', `${victim.name} was trampled. The tribe agrees it was the chicken's fault, not yours.`, { priority: 2 }); }
        }
        this.people.forEach((p) => { if (p.target === m) p.target = null; });
      }
    }
    this.mammoths = this.mammoths.filter((m) => m.alive || (m.deadT = (m.deadT || 0) + dt) < 8);
    this.meat = this.meat.filter((m) => m.amount > 0);

    // ---- People ----
    const camp = w.sites.village;
    for (const p of this.people) {
      if (!p.alive) continue;
      // Invariant: people must never stand or walk on water
      if (w.isWater && (w.isWater(p.x, p.y) || !w.walkable(p.x, p.y))) {
        const safe = w.nearestWalkable(p.x, p.y);
        p.x = safe.x; p.y = safe.y;
      }
      animate(p, dt, 6);
      const dFire = dist(p.x, p.y, f.x, f.y);
      if (G.invincible) {
        p.hunger = 0;
        p.starve = 0;
      } else {
        const hungerRate = (this.G.assistMode ? STONE_HUNGER.assistRate : STONE_HUNGER.rate) * eraTuning(this.G).hunger * modifierEffect(this.G, 'fragile-fire', 'hunger', 1);
        p.hunger = clamp(p.hunger + dt * (p.colony ? -0.01 : hungerRate));
        // An opening starvation grace so new players have time to learn (× difficulty)
        const inGrace = this.t < STONE_HUNGER.grace * eraTuning(this.G).starvationGrace;
        if (p.hunger >= 1 && !inGrace) {
          p.starve += dt;
          if (p.starve > STONE_HUNGER.starve) {
            this.kill(p);
            floater(this.particles, p.x, p.y - 7, `${p.name} starved`, '#c9c2b0', 6);
            G.advisor.say('starve', `${p.name} has starved. The tribe asks if you could be slightly more of a god.`, { mood: 'nervous', priority: 2, cooldown: 12 });
            continue;
          }
        } else p.starve = 0;
      }
      p.faith = clamp(p.faith - dt * 0.008);
      if ((f.lit && dFire < 24) || p.colony) p.fear = clamp(p.fear - dt * 0.22);
      else p.fear = clamp(p.fear + dt * 0.03 * (1 - 0.5 * p.faith));

      if (p.colony || p.state === 'embark' || p.state === 'aboard') { this.actTraveller(p, dt); continue; }
      p.decide -= dt;
      if (p.decide <= 0) { this.decide(p); p.decide = rand(0.6, 1.2); }
      this.act(p, dt, camp);
    }

    // ---- Births ----
    this.birthTimer += dt * (1 + 0.6 * (this.joy || 0));   // #152: joy brings babies sooner
    if (this.birthTimer > 2.2) {
      this.birthTimer = 0;
      const alive = this.living();
      const avgHunger = alive.reduce((s, p) => s + p.hunger, 0) / Math.max(1, alive.length);
      const nearFire = alive.filter((p) => dist(p.x, p.y, f.x, f.y) < 30).length;
      if (f.lit && this.food >= 1 && avgHunger < 0.7 && nearFire >= 2 && alive.length < 30 && chance(0.75)) {
        this.food -= 1;
        const baby = this.makePerson(w.randomLand(f.x, f.y, 3, 8));
        baby.hunger = 0.1; baby.fear = 0.1;
        this.people.push(baby);
        G.stats.births++;
        G.audio.pop();
        floater(this.particles, baby.x, baby.y - 7, '+1 baby', '#9be38a', 7);
        G.advisor.say('baby', pick(['A baby! We are naming it after you. We are naming all of them after you.', 'Another baby. The tribe credits your excellent lightning.', 'A new hunter is born. Mostly she cries.']), { mood: 'happy', priority: 1, cooldown: 14 });
      }
    }

    // ---- Expeditions: a well-fed tribe sends settlers across the sea ----
    this.expTimer += dt;
    const open = w.continents.slice(1).filter((c) => !this.colonies[c.id] && !this.boats.some((b) => b.dest === c));
    if (this.expTimer > 4 && f.lit && this.food >= 2 && this.living().length >= 12 && open.length && !this.done) {
      this.expTimer = 0;
      const crew = this.living().filter((p) => ['idle', 'warm', 'dance'].includes(p.state) && !p.colony).slice(0, 3);
      if (crew.length === 3) {
        const c = open[0];
        const boat = { kind: 'raft', x: c.homeHarbor.dock.x, y: c.homeHarbor.dock.y, facing: 1, dest: c, crew, state: 'boarding', t: 0 };
        crew.forEach((p) => { p.state = 'embark'; p.boat = boat; });
        this.boats.push(boat);
        this.food -= 1;
        G.advisor.say('raft', `${crew[0].name}, ${crew[1].name} and ${crew[2].name} built a raft and are leaving for the far shore. They say it is not personal.`, { priority: 2, cooldown: 15 });
      }
    }
    for (const b of this.boats) {
      b.t += dt;
      if (b.state === 'boarding') {
        if (b.crew.every((p) => !p.alive || p.state === 'aboard') || b.t > 20) b.state = 'sailing';
      } else if (b.state === 'sailing') {
        b.moving = true;
        if (sail(b, b.dest.harbor.dock.x, b.dest.harbor.dock.y, 9, dt, w)) {
          b.state = 'landed'; b.moving = false;
          const c = b.dest;
          this.colonies[c.id] = { x: c.village.x, y: c.village.y };
          for (const p of b.crew) if (p.alive) { p.x = c.harbor.land.x; p.y = c.harbor.land.y; p.state = 'settle'; p.colony = c; p.boat = null; p.hunger = Math.min(p.hunger, 0.3); }
          G.stats.colonies = (G.stats.colonies || 0) + 1;
          G.audio.good();
          floater(this.particles, c.village.x, c.village.y - 10, 'New colony', '#9be38a', 8);
          G.advisor.say('colony', `They made it across! Your people now live on ${1 + Object.keys(this.colonies).length} continents. The fish over there are apparently enormous.`, { mood: 'happy', priority: 3, cooldown: 5 });
        }
      }
    }

    // ---- The sacred chicken pecks around the fire ----
    const c = this.chicken;
    if (w.isWater && (w.isWater(c.x, c.y) || !w.walkable(c.x, c.y))) {
      const safe = w.nearestWalkable(c.x, c.y);
      c.x = safe.x; c.y = safe.y;
    }
    animate(c, dt, 5);
    c.t -= dt;
    if (c.t <= 0) { const p = w.randomLand(f.x, f.y, 5, 12); c.tx = p.x; c.ty = p.y; c.t = rand(1.5, 4); }
    walk(c, c.tx, c.ty, 6, dt, w);

    // ---- Live reactions ----
    if ((brain.stress ?? 0) > 0.9) G.advisor.say('stress', 'Your stress is thundering, my lord. Even the chickens are nervous.', { mood: 'panic', priority: 1, cooldown: 25 });

    // ---- Win / lose ----
    const alive = this.living().length;
    if (!this.done && alive <= 3) {
      this.done = true;
      const reason = !this.fire.everLit
        ? 'The tribe froze in the dark: blink directly on the fire pit to light it.'
        : this.food === 0 && !this.meat.length
        ? 'The tribe starved: blink to strike giant chickens and feed your people.'
        : !f.lit
        ? 'The fire went out: hold Insight when the flame fades to kindle it.'
        : 'The tribe has perished: keep the fire lit and hunt giant chickens for dinner.';
      G.fail(reason);
    }
    const focusTarget = this.G.assistMode ? 1 : FOCUS_TARGET;
    if (!this.done && alive >= POP_TARGET && this.hold.successes >= focusTarget && f.lit) {
      this.done = true;
      brain.setCue(null);
      G.audio.fanfare();
      G.advisor.say('win', 'The tribe is fed, warm and frankly a bit smug. They have started planting things on purpose.', { mood: 'happy', priority: 4, cooldown: 999 });
      for (const p of this.living()) p.state = 'dance';
      G.complete(3);
    }
  }

  endHold(success) {
    const G = this.G;
    this.hold.active = false;
    G.brain.setCue('rest');
    if (success) {
      this.hold.successes++;
      this.fire.heat = 1;
      G.audio.good();
      this.particles.burst(30, () => ({ x: this.fire.x, y: this.fire.y - 3, vx: rand(-25, 25), vy: rand(-45, -10), g: 30, life: 1, maxLife: 1, size: 1, color: pick(['#ffe066', '#ff9a2e', '#fff4c2']) }));
      floater(this.particles, this.fire.x, this.fire.y - 14, 'The flame roars', '#ffd166', 7);
      G.advisor.say('focusok', 'Your focus has fed the flame. The tribe weeps with gratitude. Also smoke.', { mood: 'happy', priority: 3, cooldown: 0 });
      for (const p of this.living()) p.faith = clamp(p.faith + 0.3);
    }
  }

  kill(p) {
    if (this.G?.invincible) { p.alive = true; p.state = 'idle'; p.starve = 0; return; }
    p.alive = false; p.state = 'dead'; p.deadT = 0; this.G.stats.deaths++;
  }

  decide(p) {
    const f = this.fire;
    p.carry = p.state === 'carry' ? p.carry : null;
    if (p.state === 'carry' || p.state === 'flee' || p.state === 'fetch') return;
    if (p.fear > 0.85 && !f.lit) { p.state = 'cower'; return; }
    if (p.hunger > 0.42 && this.food > 0 && f.lit) { p.state = 'eat'; return; }
    const pile = this.meat.find((m) => m.amount - m.claimed > 0);
    if (pile && p.hunger > 0.25 && f.lit && p.fear < 0.7) { p.state = 'fetch'; p.pile = pile; pile.claimed++; p.fetchT = 0; return; }
    const prey = this.mammoths.filter((m) => m.alive);
    if (p.hunger > 0.32 && f.lit && p.fear < 0.6 && prey.length) {
      if (!p.target || !p.target.alive) {
        prey.sort((a, b) => this.huntersOn(a) - this.huntersOn(b) || dist(p.x, p.y, a.x, a.y) - dist(p.x, p.y, b.x, b.y));
        const pickM = this.huntersOn(prey[0]) === 1 ? prey[0] : prey.find((m) => this.huntersOn(m) > 0) || prey[0];
        p.target = pickM;
      }
      p.state = 'hunt';
      return;
    }
    p.target = null;
    if (f.lit && p.faith > 0.75) { p.state = 'dance'; return; }
    if (f.lit && p.fear > 0.25) { p.state = 'warm'; return; }
    if (!f.lit) { p.state = p.fear > 0.6 ? 'cower' : 'huddle'; return; }
    p.state = 'idle';
  }

  actTraveller(p, dt) {
    const w = this.G.world;
    if (p.state === 'embark') {
      const c = p.boat.dest;
      if (walk(p, c.homeHarbor.land.x, c.homeHarbor.land.y, 12, dt, w)) p.state = 'aboard';
    } else if (p.state === 'settle') {
      const c = p.colony;
      if (!p.spot || walk(p, p.spot.x, p.spot.y, 6, dt, w)) p.spot = w.randomLand(c.village.x, c.village.y, 2, 9);
      if (Math.random() < dt * 0.15) p.state2 = p.state2 === 'dance' ? null : 'dance';
    }
  }

  huntersOn(m) { return this.people.filter((p) => p.alive && p.target === m).length; }

  act(p, dt, camp) {
    const w = this.G.world, f = this.fire;
    switch (p.state) {
      case 'hunt': {
        const m = p.target;
        if (!m || !m.alive) { p.state = 'idle'; break; }
        walk(p, m.x - 4 * (p.side || 1), m.y + 1, 13, dt, w);
        break;
      }
      case 'fetch': {
        const pile = p.pile;
        p.fetchT += dt;
        if (!pile || pile.amount <= 0 || p.fetchT > 15) { if (pile) pile.claimed = Math.max(0, pile.claimed - 1); if (pile && p.fetchT > 15) pile.amount = 0; p.state = 'idle'; break; }
        if (walk(p, pile.x, pile.y, 12, dt, w)) { pile.amount--; pile.claimed = Math.max(0, pile.claimed - 1); p.carry = pile.cooked ? '#c0562f' : '#b04a4a'; p.cookedCarry = pile.cooked; p.state = 'carry'; }
        break;
      }
      case 'carry':
        if (walk(p, f.x + 3, f.y + 2, 10, dt, w)) {
          this.food += p.cookedCarry ? 2 : 1.5;
          p.carry = null; p.state = 'idle';
          this.G.audio.coin();
          floater(this.particles, f.x, f.y - 10, '+food', '#ffcf5c', 6);
        }
        break;
      case 'eat':
        if (walk(p, f.x + p.side * 5, f.y + 3, 10, dt, w)) {
          if (this.food > 0) { this.food = Math.max(0, this.food - 1); p.hunger = Math.max(0, p.hunger - 0.6); }
          p.state = 'warm';
        }
        break;
      case 'warm':
      case 'dance': {
        if (!p.spot || p.spotT-- <= 0 || !w.walkable(p.spot.x, p.spot.y)) {
          p.spot = w.randomLand(f.x, f.y, 4, 12);
          p.spotT = 300;
        }
        walk(p, p.spot.x, p.spot.y, 8, dt, w);
        break;
      }
      case 'cower': {
        const caveSpot = w.nearestWalkable(w.sites.cave.x + p.side * 2, w.sites.cave.y);
        walk(p, caveSpot.x, caveSpot.y, 12, dt, w);
        break;
      }
      case 'huddle': {
        if (!p.spot || p.spotT-- <= 0 || !w.walkable(p.spot.x, p.spot.y)) {
          p.spot = w.randomLand(f.x, f.y, 3, 10);
          p.spotT = 200;
        }
        walk(p, p.spot.x, p.spot.y, 7, dt, w);
        break;
      }
      case 'flee': {
        const fleeSpot = w.nearestWalkable(camp.x + p.side * 6, camp.y + 4);
        if (walk(p, fleeSpot.x, fleeSpot.y, 16, dt, w)) p.state = 'idle';
        break;
      }
      default: {
        if (!p.spot || dist(p.x, p.y, p.spot.x, p.spot.y) < 1.5) p.spot = w.randomLand(camp.x, camp.y, 4, 22);
        walk(p, p.spot.x, p.spot.y, 5, dt, w);
      }
    }
  }

  objectives() {
    const alive = this.living().length;
    const focusTarget = this.G.assistMode ? 1 : FOCUS_TARGET;
    const foodHint = !this.fire.lit
      ? 'light fire'
      : this.food < 1 && !this.meat.length
      ? 'hunt chicken'
      : this.meat.length
      ? 'carry meat'
      : `${Math.floor(this.food)}`;
    return [
      { label: 'Fire', value: this.fire.lit ? 'lit' : 'out', done: this.fire.lit },
      { label: 'Tribe', value: `${alive}/${POP_TARGET}`, done: alive >= POP_TARGET, bad: alive <= 5 },
      { label: 'Focus holds', value: `${this.hold.successes}/${focusTarget}`, done: this.hold.successes >= focusTarget },
      { label: 'Food', value: foodHint, done: this.food >= 3, plain: this.food >= 1 },
      { label: 'Continents', value: `${1 + Object.keys(this.colonies).length}/${this.G.world.continents.length}`, plain: true },
    ];
  }

  lights() {
    const L = [];
    if (this.fire.lit) L.push({ x: this.fire.x, y: this.fire.y - 2, r: 16 + this.fire.heat * 30 + Math.sin(this.t * 13) * 1.5 });
    for (const f of this.forest.fires) L.push({ x: f.x, y: f.y - 2, r: 14 + f.heat * 26 + Math.sin(this.t * 11 + f.x) * 1.5 });
    for (const b of this.bolts) { const e = b.pts[b.pts.length - 1]; L.push({ x: e.x, y: e.y, r: 45 }); }
    for (const c of Object.values(this.colonies)) L.push({ x: c.x, y: c.y - 2, r: 20 + Math.sin(this.t * 11) * 1.5 });
    return L;
  }

  draw3d(engine, _dt) {
    const f = this.fire;
    // 3D Point Lights
    if (f.lit) {
      engine.add_3d_light(f.x, 10.0, f.y, 20.0 + f.heat * 22.0 + Math.sin(this.t * 13) * 1.2, FIRE_GLOW);
    }
    for (const b of this.bolts) {
      const e = b.pts[b.pts.length - 1];
      engine.add_3d_light(e.x, 12.0, e.y, 55.0, 2.5);
    }
    for (const c of Object.values(this.colonies)) {
      engine.add_3d_light(c.x, 10.0, c.y, 20.0 + Math.sin(this.t * 11) * 1.2, FIRE_GLOW * 0.8);
    }
    // Only the hottest few burning trees cast light, so a spreading fire does not pile up into
    // a white-out (the shader adds every light that reaches a pixel).
    const lit = [...this.forest.fires].sort((a, b) => b.heat - a.heat).slice(0, 6);
    for (const f of lit) engine.add_3d_light(f.x, 11.0, f.y, 14.0 + f.heat * 18.0, FIRE_GLOW * 0.7);
    for (const f of this.forest.fires) engine.add_billboard(4, f.x, f.y, 1, Math.floor(this.t * 6 + f.x) % 3, 0);

    // Billboards
    // Fire pit (variant 1 is unlit firewood logs; variant 0 is roaring flame)
    engine.add_billboard(4, f.x, f.y, 1, Math.floor(this.t * 6) % 3, f.lit ? 0 : 1);

    // Mammoths
    for (const m of this.mammoths) {
      engine.add_billboard(1, m.x, m.y, m.facing || 1, m.frame, m.dead ? 1 : 0);
    }

    // People. They stay on the map when the fire is out: an unlit pit must not make the tribe
    // vanish (the fire going out is a setback, not a reason to hide everyone).
    for (const p of this.people) {
      if (!p.alive && (p.deadT || 0) > 12) continue;
      if (p.state === 'aboard') continue;
      const pose = !p.alive ? 3 : p.state === 'dance' || p.state2 === 'dance' ? 1 : p.state === 'cower' ? 2 : p.halo && p.state === 'warm' ? 1 : 0;
      engine.add_billboard(0, p.x, p.y, p.facing || 1, p.frame, pose);
    }

    // Chicken
    engine.add_billboard(3, this.chicken.x, this.chicken.y, this.chicken.facing || 1, this.chicken.frame, this.chicken.halo ? 1 : 0);

    // Colony fires
    for (const c of Object.values(this.colonies)) {
      engine.add_billboard(4, c.x, c.y, 1, Math.floor(this.t * 6 + 1) % 3, 0);
    }

  }

  draw(ctx) {
    const G = this.G;
    const is3d = Boolean(G?.render3d || globalThis.__moc_render3d);
    const base = is3d ? (G?.world?.layers3d?.stone || this.layer) : this.layer;
    ctx.drawImage(base, 0, 0);
    if (!is3d) G?.world?.drawWater?.(ctx, this.t);
    for (const m of this.meat) { ctx.fillStyle = m.cooked ? '#c0562f' : '#9e3d3d'; ctx.fillRect(m.x - 1, m.y - 1, 3, 2); }
    // In 3D, draw3d stands people, mammoths, fires and the chicken up as sprites; painting them
    // flat as well showed everything twice. Boats have no sprite yet, so they stay painted.
    if (is3d) {
      for (const b of this.boats) drawBoat(ctx, b.x, b.y, b);
      this.particles.draw(ctx);
      return;
    }
    // y-sorted actors
    const actors = [];
    for (const m of this.mammoths) actors.push({ y: m.y, d: () => drawMammoth(ctx, m.x, m.y, m) });
    for (const b of this.boats) actors.push({ y: b.y, d: () => drawBoat(ctx, b.x, b.y, b) });
    for (const c of Object.values(this.colonies)) actors.push({ y: c.y, d: () => drawFire(ctx, c.x, c.y, 0.7, this.t + 3) });
    for (const p of this.people) {
      if (!p.alive && (p.deadT = (p.deadT || 0) + 1 / 60) > 12) continue;
      if (p.state === 'aboard') continue;
      const pose = !p.alive ? 'dead' : p.state === 'dance' || p.state2 === 'dance' ? 'dance' : p.state === 'cower' ? 'panic' : p.halo && p.state === 'warm' ? 'pray' : 'walk';
      actors.push({ y: p.y, d: () => drawPerson(ctx, p.x, p.y, { ...p, pose, hat: p.halo ? '#ffe066' : null }) });
    }
    actors.push({ y: this.chicken.y, d: () => drawChicken(ctx, this.chicken.x, this.chicken.y, this.chicken) });
    actors.push({ y: this.fire.y, d: () => drawFire(ctx, this.fire.x, this.fire.y, this.fire.lit ? this.fire.heat : 0, this.t) });
    for (const f of this.forest.fires) actors.push({ y: f.y, d: () => drawFire(ctx, f.x, f.y, f.heat, this.t + f.x) });
    actors.sort((a, b) => a.y - b.y).forEach((a) => a.d());
    this.particles.draw(ctx);
  }

  drawAfterLight(ctx) {
    for (const b of this.bolts) drawBolt(ctx, b.pts, Math.min(1, b.life * 4));
    // The aim reticle is drawn once, on top of the lighting, by main.js (#301).
    const w = this.G.world;
    if (this.flash > 0) { ctx.fillStyle = `rgba(220,230,255,${this.flash * 0.9})`; ctx.fillRect(0, 0, w.width, w.height); }
  }

  drawOverlay(ctx) {
    if (!this.G?.simulated && !this.simulated) {
      const f = this.fire;
      ctx.textAlign = 'center';
      if (!f.lit && !this.done) {
        const bob = Math.sin(this.t * 5) * 1.5;
        label(ctx, f.x, f.y - 12 + bob, 'blink here to light the fire', '#ffd166', 6);
      } else if (f.lit && this.food < 1 && !this.meat.length && !this.done) {
        const nearby = this.mammoths.find((m) => m.alive && dist(m.x, m.y, f.x, f.y) < 70);
        if (nearby) {
          const bob = Math.sin(this.t * 5) * 1.5;
          label(ctx, nearby.x, nearby.y - 14 + bob, 'blink chicken for food', '#ffd166', 5.5);
        }
      }
      if (this.hold.active) {
        const p = this.hold.progress / this.holdSeconds();
        ctx.lineWidth = 1.4;
        ctx.strokeStyle = 'rgba(10,12,30,0.7)';
        ctx.beginPath(); ctx.arc(f.x, f.y - 3, 13, 0, Math.PI * 2); ctx.stroke();
        ctx.strokeStyle = '#9ec9ff';
        ctx.beginPath(); ctx.arc(f.x, f.y - 3, 13, -Math.PI / 2, -Math.PI / 2 + p * Math.PI * 2); ctx.stroke();
        label(ctx, f.x, f.y - 20, `HOLD INSIGHT  ${(this.holdSeconds() - this.hold.progress).toFixed(1)}s`, '#9ec9ff', 7);
      }
    }
    this.fireballs.draw(ctx);
    this.particles.drawText(ctx);
  }

  // ---- camera (#33): hand casts and face mood. Extra inputs on top of the brain, never instead of it. ----

  // Living-entity pinch-grab support (#120)
  onGrabStart(aim) {
    if (this.done) return null;
    const { x, y } = this.G.world.aimPoint(aim);
    // Reject non-living targets: fire pit, rocks, water
    if (dist(x, y, this.fire.x, this.fire.y) < 14) {
      return { ok: false, reason: 'Fire pit cannot be picked up' };
    }
    // Check sacred chicken
    if (dist(x, y, this.chicken.x, this.chicken.y) < 8) {
      this.grabbedEntity = { entity: this.chicken, kind: 'chicken', label: 'Sacred Chicken' };
      floater(this.particles, this.chicken.x, this.chicken.y - 8, 'grabbed chicken!', '#ffe066', 6);
      return { ok: true, label: 'Sacred Chicken', kind: 'chicken' };
    }
    // Check people
    for (const p of this.living()) {
      if (dist(x, y, p.x, p.y - 2) < 6) {
        this.grabbedEntity = { entity: p, kind: 'person', label: p.name };
        p.state = 'idle';
        floater(this.particles, p.x, p.y - 8, `grabbed ${p.name}!`, '#ffd166', 6);
        return { ok: true, label: p.name, kind: 'person' };
      }
    }
    // Check alive mammoths
    for (const m of this.mammoths) {
      if (m.alive && dist(x, y, m.x, m.y - 2) < 10) {
        this.grabbedEntity = { entity: m, kind: 'mammoth', label: 'Giant chicken' };
        m.pause = 999;
        floater(this.particles, m.x, m.y - 8, 'grabbed a giant chicken!', '#ff9a2e', 6);
        return { ok: true, label: 'Giant chicken', kind: 'mammoth' };
      }
    }
    return { ok: false, reason: 'Pinch over a villager or a chicken to grab' };
  }

  onGrabMove(aim) {
    if (!this.grabbedEntity) return;
    const { x, y } = this.G.world.aimPoint(aim);
    const e = this.grabbedEntity.entity;
    e.x = x;
    e.y = y;
    if (e.tx !== undefined) { e.tx = x; e.ty = y; }
  }

  onGrabEnd({ x: aimX, y: aimY, cancelled = false }) {
    if (!this.grabbedEntity) return;
    const { entity, label: entLabel, kind } = this.grabbedEntity;
    this.grabbedEntity = null;
    if (cancelled) return;
    const w = this.G.world;
    const { x: targetX, y: targetY } = w.aimPoint({ x: aimX, y: aimY });
    const safeLand = w.walkable(targetX, targetY) && w.reachable(targetX, targetY)
      ? { x: targetX, y: targetY }
      : w.randomLand(w.sites.village.x, w.sites.village.y, 10, 30, (x, y) => w.walkable(x, y) && w.reachable(x, y));
    entity.x = safeLand.x;
    entity.y = safeLand.y;
    entity.tx = safeLand.x;
    entity.ty = safeLand.y;
    if (kind === 'mammoth') entity.pause = rand(1, 3);
    floater(this.particles, safeLand.x, safeLand.y - 8, `placed ${entLabel}`, '#9be38a', 6);
  }

  // Charged fireball (#291): independent of the removed spell dispatch. The projectile lands
  // before any impact effect fires (#315).
  onFireball(aim, { color = '#ff9a2e', charge = 1 } = {}) {
    const G = this.G;
    if (!G || this.done || !combatEnabled(G)) return;
    const { x, y } = G.world.aimPoint(aim);
    this.fireballs.launch({ x, y }, { color, charge });
    G.audio?.blip?.();
  }

  impactFireball({ x, y, color = '#ff9a2e', charge = 1 }) {
    const G = this.G;
    const n = 55 + Math.round(charge * 60);
    this.flash = Math.max(this.flash, 1);
    G.shake = Math.max(G.shake || 0, 4.5 + charge * 3);
    G.audio?.thunder?.();
    this.particles.burst(n, () => ({
      x, y, vx: rand(-46, 46), vy: rand(-62, 4), g: 75,
      life: rand(0.5, 0.95), maxLife: 0.95, size: 1, color,
    }));
    this.particles.burst(8 + Math.round(charge * 8), () => ({
      x: x + rand(-4, 4), y: y + rand(-1, 2), vx: rand(-18, 18), vy: rand(-16, -2), g: 24,
      life: rand(0.5, 0.9), maxLife: 0.9, size: 1, color: pick(['#c9c2b0', '#8a7a5a', '#5f5545']),
    }));
    for (const p of this.living()) if (dist(x, y, p.x, p.y) < 22 + charge * 14) p.fear = clamp(p.fear + 0.2 + charge * 0.2);
    // A massive explosion (Alex): everything in the blast takes it. Giant chickens are roasted,
    // trees catch, people near the centre die, everyone further out panics.
    const R = 70 + charge * 90;
    this.flash = Math.max(this.flash, 1);
    G.shake = Math.max(G.shake || 0, 5 + charge * 3);
    for (const m of this.mammoths) {
      if (m.alive && dist(x, y, m.x, m.y) < R) { m.alive = false; m.dead = true; this.meat.push({ x: m.x, y: m.y, amount: 5, cooked: true, claimed: 0 }); }
    }
    let burning = 0;
    for (const t of [...(G.world.trees || [])]) {
      if (burning >= 10 + Math.round(charge * 20)) break;
      if (dist(x, y, t.x, t.y) < R && this.igniteForestFire(t)) burning++;
    }
    for (const p of this.living()) {
      if (p.colony || p.state === 'aboard') continue;
      const d = dist(x, y, p.x, p.y);
      if (d < R * 0.35) this.kill(p);
      else if (d < R * 1.5) { p.fear = clamp(p.fear + 0.7); if (['idle', 'warm', 'dance', 'huddle', 'hunt'].includes(p.state)) { p.state = 'flee'; p.target = null; } }
    }
    // Every rival people in the blast dies too (#fireball).
    const rivals = G.rivals?.smite?.(x, y, R) || 0;
    if (rivals) {
      floater(this.particles, x, y - 26, `${rivals} rival${rivals === 1 ? '' : 's'} incinerated`, '#ff5a72', 8);
      G.advisor?.say('fireball-rivals', `${rivals} of the rival peoples were inside the blast. They are no longer a problem.`, { mood: 'panic', priority: 3, cooldown: 5 });
      G.hud?.showCause?.(`🔥 ${rivals} rival ${rivals === 1 ? 'person' : 'people'} killed in the blast`);
    }
    // And it strikes like lightning at the centre: it lights the pit.
    this.lastBolt = -Infinity;
    this.onBlink(G.world.aimOf(x, y));
    if (this.living().length === 0 && !this.done) {
      this.done = true;
      G.fail?.('Your fireball wiped out the whole tribe.');
    }
  }

  // aim is 0..1 like the brain's. The fist (fireball) normally arrives through onBlink.
  onCast(spell, aim) {
    const G = this.G;
    if (!G || this.done) return;
    const w = G.world, f = this.fire;
    const { x, y } = w.aimPoint(aim);
    switch (spell) {
      case 'palm': {
        // Core move Palm (#122): calm fear, soothe people, feed fire a bit
        for (const p of this.living()) {
          p.fear = clamp(p.fear - 0.3);
          p.faith = clamp(p.faith + 0.15);
        }
        if (f.lit) f.heat = Math.min(1, f.heat + 0.15);
        floater(this.particles, f.x, f.y - 14, 'peace & calm', '#9be38a', 7);
        G.advisor.say('cast-palm', 'Your open palm calms the tribe and soothes the fire.', { mood: 'happy', priority: 1, cooldown: 20 });
        break;
      }
      case 'fireball': this.onBlink(aim); break;
      case 'spawn': {
        // Open palm: a cooked haunch drops where you aim and the tribe carries it to the fire.
        // Aim at the sea and it lands in the larder instead.
        if (w.walkable(x, y) && w.reachable(x, y)) {
          this.meat.push({ x, y, amount: 3, cooked: true, claimed: 0 });
          floater(this.particles, x, y - 8, 'meat from the sky', '#ff9a2e', 6);
        } else {
          this.food += 2;
          floater(this.particles, f.x, f.y - 10, '+2 food', '#ffcf5c', 7);
        }
        this.particles.burst(10, () => ({ x: x + rand(-3, 3), y: y - 18, vx: rand(-6, 6), vy: rand(15, 30), g: 40, life: 0.6, maxLife: 0.6, size: 1, color: pick(['#c0562f', '#ffcf5c', '#f2e8d5']) }));
        G.audio.coin();
        G.advisor.say('cast-spawn', 'Meat. From the sky. The tribe has stopped asking questions.', { mood: 'happy', priority: 1, cooldown: 25 });
        break;
      }
      case 'rain': {
        // Victory: a soft rain. Fear eases and the berry bushes swell.
        for (const p of this.living()) p.fear = clamp(p.fear - 0.35);
        this.food += 1;
        this.castRain(x, y);
        floater(this.particles, x, y - 66, 'gentle rain', '#9ec9ff', 7);
        G.advisor.say('cast-rain', 'A gentle rain. The tribe relaxes, the berries swell. The fire is fine, I checked.', { mood: 'happy', priority: 1, cooldown: 25 });
        break;
      }
      case 'bless': {
        for (const p of this.living()) { p.faith = clamp(p.faith + 0.3); p.fear = clamp(p.fear - 0.1); }
        this.particles.burst(24, () => ({ x: x + rand(-20, 20), y: y + rand(-12, 12), vx: rand(-3, 3), vy: rand(-14, -4), life: 1.2, maxLife: 1.2, size: 1, color: pick(['#ffe066', '#fff4c2']) }));
        floater(this.particles, x, y - 10, 'blessed', '#ffe066', 7);
        G.audio.good();
        G.advisor.say('cast-bless', 'The tribe feels blessed. Several of them are now insufferable about it.', { mood: 'happy', priority: 1, cooldown: 25 });
        break;
      }
      case 'disaster': {
        // Thumb down: a storm at the reticle. People nearby panic and run. Nobody is hurt.
        this.flash = 0.5;
        G.shake = Math.max(G.shake || 0, 1.2);
        G.audio.thunder();
        for (let i = 0; i < 3; i++) this.bolts.push({ pts: makeBolt(x + rand(-16, 16), y + rand(-8, 8)), life: 0.35 + i * 0.12 });
        this.castRain(x, y);
        for (const p of this.living()) {
          const d = dist(x, y, p.x, p.y);
          if (d > 45) continue;
          p.fear = clamp(p.fear + 0.45);
          if (d < 28 && !p.colony && ['idle', 'warm', 'dance', 'huddle', 'hunt'].includes(p.state)) { p.state = 'flee'; p.target = null; }
        }
        if (f.lit && dist(x, y, f.x, f.y) < 20) f.heat = Math.max(0.05, f.heat - 0.25);
        floater(this.particles, x, y - 12, 'STORM', '#9ec9ff', 9);
        G.advisor.say('cast-storm', 'A storm, my lord? On purpose? The tribe is hiding behind the chicken.', { mood: 'panic', priority: 2, cooldown: 20 });
        break;
      }
      case 'peace': {
        for (const p of this.living()) {
          p.fear = clamp(p.fear - 0.25);
          if (!p.colony && (p.state === 'flee' || p.state === 'cower')) p.state = 'idle';
        }
        floater(this.particles, f.x, f.y - 14, 'peace', '#9be38a', 7);
        G.advisor.say('cast-peace', 'Peace descends. Grug has apologised to the chicken.', { mood: 'happy', priority: 1, cooldown: 25 });
        break;
      }
    }
  }

  // Rain over a screen-sized patch of sky around (cx, cy) (the world is bigger than the view, #129).
  castRain(cx, cy = this.G?.camera?.viewCenter?.().y ?? this.fire.y) {
    this.particles.burst(60, () => ({ x: cx + rand(-60, 60), y: cy + rand(-100, 26), vx: -4, vy: rand(70, 100), life: rand(0.5, 1), maxLife: 1, size: 1, color: 'rgba(158,201,255,0.75)' }));
  }

  // The tribe at home (not settlers or sailors): who the face's mood reaches.
  homeTribe() { return this.living().filter((p) => !p.colony && p.state !== 'embark' && p.state !== 'aboard'); }

  // #152: joy is a continuous buff while the face smiles (or the slider says Happy): the tribe at
  // home dances, gains faith, loses fear and has babies sooner, and sparkles rise from the dancers.
  // It stops the moment the smile does (src/camera/mood-actions.js joyBuff).
  joyStep(dt) {
    const joy = this.done ? 0 : joyBuff(this.G.mood);
    this.joy = joy;
    const home = this.homeTribe();
    const dancers = [];
    for (const p of home) {
      const free = joy > 0 && ['idle', 'warm', 'huddle', 'dance', 'cower'].includes(p.state);
      if (free) { p.state2 = 'dance'; p.moodDance = true; dancers.push(p); }
      else if (p.moodDance) { p.moodDance = false; p.state2 = null; }
      if (joy > 0) { p.faith = clamp(p.faith + dt * 0.06 * joy); p.fear = clamp(p.fear - dt * 0.1 * joy); }
    }
    for (let n = sparkleCount(joy, dt, 14); n > 0 && dancers.length; n--) {
      const p = pick(dancers);
      this.particles.add({ x: p.x + rand(-2, 2), y: p.y - 8, vx: rand(-3, 3), vy: rand(-16, -9), life: 0.9, maxLife: 0.9, size: 1, color: pick(JOY_COLORS) });
    }
  }

  // #152: a surprise is a one-shot: lightning cracks on screen, a capped flash and a short shake
  // (none under reduced motion), thunder, and the tribe jumps.
  surpriseStrike() {
    const G = this.G, fx = surpriseEffect(reducedMotion());
    this.flash = Math.max(this.flash, fx.flash);
    G.shake = Math.max(G.shake || 0, fx.shake);
    const c = G.camera?.viewCenter?.() || this.fire;   // somewhere on screen
    for (let i = 0; i < fx.bolts; i++) this.bolts.push({ pts: makeBolt(c.x + rand(-140, 140), c.y + rand(-60, 70)), life: 0.3 + i * 0.12 });
    G.audio?.thunder?.();
    for (const p of this.homeTribe()) p.fear = clamp(p.fear + 0.15);
    floater(this.particles, c.x, c.y - 40, 'SURPRISE!', '#ffe066', 10);
  }

  // Face mood. Joy is the continuous buff above (its onset gets a floater and a faith boost).
  // Surprise fires once per event (mood.surpriseTriggered), not while a face just stays surprised.
  // Anger: the tribe gets restless. Sadness: the night gets darker. Sent on every change of
  // dominant mood (including back to neutral), on a surprise, and every few seconds while strong.
  onMood(mood) {
    const G = this.G;
    if (!G) return;
    this.baseDarkness ??= this.darkness;
    this.darkness = clamp(this.baseDarkness + (mood.dominant === 'sadness' ? 0.25 : 0), 0, 0.9);
    if (this.done) return;
    if (surpriseFires(mood)) this.surpriseStrike();
    const home = this.homeTribe();
    if (mood.dominant === 'joy') {
      if (mood.reason === 'change' && joyBuff(mood) > 0) {
        for (const p of home) p.faith = clamp(p.faith + 0.12);
        floater(this.particles, this.fire.x, this.fire.y - 18, 'the tribe dances!', '#ffe066', 7);
      }
    } else if (mood.dominant === 'anger') {
      for (const p of home) { p.fear = clamp(p.fear + 0.1); p.faith = clamp(p.faith - 0.05); }
      if (mood.reason === 'change') floater(this.particles, this.fire.x, this.fire.y - 16, 'grumbling', '#ff8a9c', 6);
    }
  }
}

export function label(ctx, x, y, text, color, size = 6) {
  if (ctx.__project3d) {
    const pr = ctx.__project3d(x, y);
    if (!pr) return;
    x = pr[0]; y = pr[1];
  }
  ctx.font = `${size}px "Pixelify Sans", monospace`;
  ctx.textAlign = 'center';
  ctx.fillStyle = 'rgba(8,10,24,0.85)';
  ctx.fillText(text, x + 0.6, y + 0.6);
  ctx.fillStyle = color;
  ctx.fillText(text, x, y);
}
