// ERA I · STONE AGE
// Blink to throw lightning (light the fire, cook a mammoth, convert Grug).
// Hold calm for 5 s when the flame fades. Keep the tribe fed until it grows.
// Calibration: calm cues are labelled `calm`, the rest of play is labelled `rest`.

import { W, H, Particles, clamp, dist, rand, chance, floater, pick } from '../sim/util.js';
import { walk, animate, villagerLook, sail } from '../sim/agents.js';
import { drawPerson, drawMammoth, drawFire, makeBolt, drawBolt, drawReticle, drawChicken, drawBoat } from '../render/sprites.js';

const POP_START = 10;
const POP_TARGET = 15;
const CALM_TARGET = 2;
const HOLD_SECONDS = 5;
const NAMES = ['Grug', 'Ugg-a', 'Thak', 'Mo', 'Brakka', 'Nub', 'Ila', 'Krog', 'Zuz', 'Hap', 'Wenna', 'Dorf', 'Yip', 'Ooma'];

export class StoneAge {
  constructor() {
    this.id = 'stone';
    this.era = 'stone';
    this.label = 'Era I · Stone Age';
    this.intro = {
      title: 'Stone Age',
      goal: `Grow the tribe to ${POP_TARGET} and keep the fire alive with ${CALM_TARGET} calm holds.`,
      how: [['Blink', 'Throw lightning where you look. Strike the fire pit to light it, strike a mammoth for dinner.'], ['Calm', 'When the flame fades, stay calm for 5 seconds.']],
    };
    this.powers = ['blink', 'calm'];
    this.darkness = 0.6;
  }

  enter(G) {
    this.G = G;
    const w = G.world;
    this.layer = w.paint('stone');
    this.particles = new Particles();
    this.t = 0;
    this.fire = { x: w.sites.fire.x, y: w.sites.fire.y, heat: 0, lit: false, everLit: false };
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
    this.boats = [];
    this.colonies = {};
    this.expTimer = 0;
    this.people = [];
    for (let i = 0; i < POP_START; i++) this.people.push(this.makePerson(w.randomLand(w.sites.cave.x, w.sites.cave.y, 2, 14), NAMES[i]));
    this.mammoths = [];
    for (let i = 0; i < 4; i++) this.spawnMammoth();
    this.chicken = { x: this.fire.x + 8, y: this.fire.y + 4, frame: 0, ft: 0, facing: 1, tx: this.fire.x + 8, ty: this.fire.y + 4, t: 0 };
    G.brain.setCue('rest');
    G.advisor.say('intro', 'My lord! The tribe is cold, hungry and quietly judging you. Blink to throw lightning at the fire pit.', { priority: 3, cooldown: 999 });
  }

  makePerson(p, name) {
    return { x: p.x, y: p.y, name: name || pick(NAMES), ...villagerLook('stone'), hunger: rand(0.15, 0.4), fear: rand(0.3, 0.5), faith: 0.1,
      state: 'idle', tx: p.x, ty: p.y, decide: rand(0, 0.5), starve: 0, alive: true, frame: 0, ft: rand(0, 4), side: chance(0.5) ? 1 : -1, facing: 1, carry: null, target: null, halo: false };
  }

  spawnMammoth() {
    const w = this.G.world, v = w.sites.village;
    const p = w.randomLand(v.x, v.y, 40, 85, (x, y) => w.get(x, y) === 3 && w.reachable(x, y));
    this.mammoths.push({ x: p.x, y: p.y, tx: p.x, ty: p.y, alive: true, frame: 0, ft: 0, facing: 1, hunt: 0, lone: 0, side: 1, pause: 0 });
  }

  living() { return this.people.filter((p) => p.alive); }

  onBlink(aim) {
    const G = this.G;
    if (this.t - this.lastBolt < 0.5 || this.done) return;
    this.lastBolt = this.t;
    const x = Math.round(aim.x * (W - 1)), y = Math.round(aim.y * (H - 1));
    this.bolts.push({ pts: makeBolt(x, y), life: 0.35 });
    this.flash = 0.35;
    this.strikes++;
    G.stats.lightning++;
    G.audio.thunder();
    this.particles.burst(14, () => ({ x, y, vx: rand(-30, 30), vy: rand(-40, 5), g: 60, life: 0.6, maxLife: 0.6, size: 1, color: pick(['#fff4c2', '#ffd166', '#ffffff']) }));

    // Fire pit
    if (dist(x, y, this.fire.x, this.fire.y) < 9) {
      const first = !this.fire.everLit;
      this.fire.lit = true; this.fire.everLit = true;
      this.fire.heat = Math.max(this.fire.heat, 0.7);
      G.audio.ignite();
      floater(this.particles, this.fire.x, this.fire.y - 10, first ? 'FIRE!' : 'Relit!', '#ffd166', 8);
      if (first) G.advisor.say('fire', 'FIRE! You have invented fire! Please do not invent it on Grug.', { mood: 'happy', priority: 3, cooldown: 999 });
      else G.advisor.say('relit', 'The flame lives again. The tribe pretends it was never worried.', { mood: 'happy', priority: 2 });
    }
    // Mammoths
    for (const m of this.mammoths) {
      if (m.alive && dist(x, y, m.x, m.y - 2) < 7) {
        m.alive = false; m.dead = true;
        this.meat.push({ x: m.x, y: m.y, amount: 5, cooked: true, claimed: 0 });
        floater(this.particles, m.x, m.y - 8, 'barbecue', '#ff9a2e', 7);
        G.advisor.say('bbq', 'Cooked mammoth! We shall call it... barbecue. Someone carry it to the fire.', { mood: 'happy', priority: 2 });
      }
    }
    // People
    for (const p of this.people) {
      if (!p.alive) continue;
      const d = dist(x, y, p.x, p.y - 2);
      if (d < 4) {
        p.halo = true; p.faith = 1; p.fear = Math.min(1, p.fear + 0.4);
        G.stats.smitten++;
        floater(this.particles, p.x, p.y - 8, `${p.name} found religion`, '#ffe066', 6);
        G.advisor.say('smite', `My lord, that was ${p.name}. ${p.name} has found religion. Also, eyebrows are gone.`, { priority: 2 });
      } else if (d < 50) {
        p.fear = Math.min(1, p.fear + 0.12);
        p.faith = Math.min(1, p.faith + 0.15);
      }
    }
    if (this.strikes > 6 && this.t < 20) G.advisor.say('spam', 'With respect, my lord, lightning is not a fidget toy.', { priority: 1, cooldown: 30 });
  }

  update(dt) {
    const G = this.G, w = G.world, brain = G.brain;
    this.t += dt;
    this.particles.update(dt);
    this.bolts = this.bolts.filter((b) => (b.life -= dt) > 0);
    this.flash = Math.max(0, this.flash - dt);

    // ---- Fire and the calm challenge ----
    const f = this.fire;
    if (f.lit) {
      f.heat -= dt * (this.hold.active ? 0.022 : 0.034);
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
      brain.setCue('calm');
      G.advisor.say('calm', 'The flame is fading. Be still, my lord. Breathe like a sleepy boulder.', { priority: 3, cooldown: 0 });
      G.audio.blip();
    }
    if (this.hold.active) {
      const thr = brain.threshold('calm', 0.55);
      this.hold.thr = thr;
      if (brain.calm >= thr) this.hold.progress += dt;
      else {
        if (this.hold.progress > 1.5) G.advisor.say('broke', 'You flinched. The fire flinched. We all flinched.', { priority: 2, cooldown: 10 });
        this.hold.progress = Math.max(0, this.hold.progress - dt * 1.5);
      }
      if (this.hold.progress >= HOLD_SECONDS) this.endHold(true);
    }

    // ---- Mammoths ----
    this.spawnTimer += dt;
    if (this.mammoths.filter((m) => m.alive).length < 3 && this.spawnTimer > 6) { this.spawnTimer = 0; this.spawnMammoth(); }
    for (const m of this.mammoths) {
      if (!m.alive) continue;
      animate(m, dt, 3);
      m.pause -= dt;
      if (m.pause <= 0 && walk(m, m.tx, m.ty, 4, dt, w)) {
        const v = w.sites.village;
        const p = w.randomLand(m.x, m.y, 6, 22, (x, y) => dist(x, y, v.x, v.y) > 32);
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
        if (chance(0.2)) {
          const victim = this.people.find((p) => p.alive && p.target === m);
          if (victim) { this.kill(victim); G.advisor.say('trample', `${victim.name} was trampled. The tribe agrees it was the mammoth's fault, not yours.`, { priority: 2 }); }
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
      animate(p, dt, 6);
      const dFire = dist(p.x, p.y, f.x, f.y);
      p.hunger = clamp(p.hunger + dt * (p.colony ? -0.01 : 0.017));
      p.faith = clamp(p.faith - dt * 0.008);
      if ((f.lit && dFire < 24) || p.colony) p.fear = clamp(p.fear - dt * 0.22);
      else p.fear = clamp(p.fear + dt * 0.03 * (1 - 0.5 * p.faith));
      if (p.hunger >= 1) {
        p.starve += dt;
        if (p.starve > 8) {
          this.kill(p);
          floater(this.particles, p.x, p.y - 7, `${p.name} starved`, '#c9c2b0', 6);
          G.advisor.say('starve', `${p.name} has starved. The tribe asks if you could be slightly more of a god.`, { mood: 'nervous', priority: 2, cooldown: 12 });
          continue;
        }
      } else p.starve = 0;

      if (p.colony || p.state === 'embark' || p.state === 'aboard') { this.actTraveller(p, dt); continue; }
      p.decide -= dt;
      if (p.decide <= 0) { this.decide(p); p.decide = rand(0.6, 1.2); }
      this.act(p, dt, camp);
    }

    // ---- Births ----
    this.birthTimer += dt;
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
    animate(c, dt, 5);
    c.t -= dt;
    if (c.t <= 0) { const p = w.randomLand(f.x, f.y, 5, 12); c.tx = p.x; c.ty = p.y; c.t = rand(1.5, 4); }
    walk(c, c.tx, c.ty, 6, dt, w);

    // ---- Live reactions ----
    if (brain.hr && brain.hrDelta() > 18) {
      G.advisor.say('hr', 'Your pulse is thundering, my lord. Even the mammoths are nervous.', { mood: 'panic', priority: 1, cooldown: 25 });
    } else if ((brain.panic || 0) > 0.7) {
      G.advisor.say('hr', 'Your tension is high, my lord. Even the mammoths are nervous.', { mood: 'panic', priority: 1, cooldown: 25 });
    }

    // ---- Win / lose ----
    const alive = this.living().length;
    if (!this.done && alive <= 3) { this.done = true; G.fail('The tribe has perished. The gods try again.'); }
    if (!this.done && alive >= POP_TARGET && this.hold.successes >= CALM_TARGET && f.lit) {
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
      G.advisor.say('calmok', 'Your serenity has fed the flame. The tribe weeps with gratitude. Also smoke.', { mood: 'happy', priority: 3, cooldown: 0 });
      for (const p of this.living()) p.faith = clamp(p.faith + 0.3);
    }
  }

  kill(p) { p.alive = false; p.state = 'dead'; p.deadT = 0; this.G.stats.deaths++; }

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
        if (!p.spot || p.spotT-- <= 0) { const a = rand(0, Math.PI * 2), r = rand(5, 12); p.spot = { x: f.x + Math.cos(a) * r, y: f.y + Math.sin(a) * r * 0.7 }; p.spotT = 300; }
        walk(p, p.spot.x, p.spot.y, 8, dt, w);
        break;
      }
      case 'cower':
        walk(p, w.sites.cave.x + p.side * 2, w.sites.cave.y, 12, dt, w);
        break;
      case 'huddle': {
        if (!p.spot || p.spotT-- <= 0) { p.spot = w.randomLand(f.x, f.y, 3, 10); p.spotT = 200; }
        walk(p, p.spot.x, p.spot.y, 7, dt, w);
        break;
      }
      case 'flee':
        if (walk(p, camp.x + p.side * 6, camp.y + 4, 16, dt, w)) p.state = 'idle';
        break;
      default: {
        if (!p.spot || dist(p.x, p.y, p.spot.x, p.spot.y) < 1.5) p.spot = w.randomLand(camp.x, camp.y, 4, 22);
        walk(p, p.spot.x, p.spot.y, 5, dt, w);
      }
    }
  }

  objectives() {
    const alive = this.living().length;
    return [
      { label: 'Fire', value: this.fire.lit ? 'lit' : 'out', done: this.fire.lit },
      { label: 'Tribe', value: `${alive}/${POP_TARGET}`, done: alive >= POP_TARGET, bad: alive <= 5 },
      { label: 'Calm holds', value: `${this.hold.successes}/${CALM_TARGET}`, done: this.hold.successes >= CALM_TARGET },
      { label: 'Food', value: `${Math.floor(this.food)}`, done: false, plain: true },
      { label: 'Continents', value: `${1 + Object.keys(this.colonies).length}/${this.G.world.continents.length}`, plain: true },
    ];
  }

  lights() {
    const L = [];
    if (this.fire.lit) L.push({ x: this.fire.x, y: this.fire.y - 2, r: 16 + this.fire.heat * 30 + Math.sin(this.t * 13) * 1.5 });
    for (const b of this.bolts) { const e = b.pts[b.pts.length - 1]; L.push({ x: e.x, y: e.y, r: 45 }); }
    for (const c of Object.values(this.colonies)) L.push({ x: c.x, y: c.y - 2, r: 20 + Math.sin(this.t * 11) * 1.5 });
    return L;
  }

  draw3d(engine, _dt) {
    const f = this.fire;
    // 3D Point Lights
    if (f.lit) {
      engine.add_3d_light(f.x, 10.0, f.y, 25.0 + f.heat * 30.0 + Math.sin(this.t * 13) * 1.5, 1.8);
    }
    for (const b of this.bolts) {
      const e = b.pts[b.pts.length - 1];
      engine.add_3d_light(e.x, 12.0, e.y, 55.0, 2.5);
    }
    for (const c of Object.values(this.colonies)) {
      engine.add_3d_light(c.x, 10.0, c.y, 22.0 + Math.sin(this.t * 11) * 1.5, 1.4);
    }

    // Billboards
    // Fire pit
    engine.add_billboard(4, f.x, f.y, 1, Math.floor(this.t * 6) % 3, 0);

    // Mammoths
    for (const m of this.mammoths) {
      engine.add_billboard(1, m.x, m.y, m.facing || 1, m.frame, m.dead ? 1 : 0);
    }

    // People
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
    const base = G.render3d ? (G.world?.layers3d?.stone || this.layer) : this.layer;
    ctx.drawImage(base, 0, 0);
    if (!G.render3d) G.world.drawWater(ctx, this.t);
    for (const m of this.meat) { ctx.fillStyle = m.cooked ? '#c0562f' : '#9e3d3d'; ctx.fillRect(m.x - 1, m.y - 1, 3, 2); }
    // In 3D, draw3d stands people, mammoths, fires and the chicken up as sprites; painting them
    // flat as well showed everything twice. Boats have no sprite yet, so they stay painted.
    if (G.render3d) {
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
    actors.sort((a, b) => a.y - b.y).forEach((a) => a.d());
    this.particles.draw(ctx);
  }

  drawAfterLight(ctx) {
    for (const b of this.bolts) drawBolt(ctx, b.pts, Math.min(1, b.life * 4));
    const a = this.G.brain.aim;
    // Painted into the map in 3D too: the map drapes over the terrain, so it lands exactly on the
    // ground point under the cursor (the standing sprite version sat above it, or didn't show).
    drawReticle(ctx, a.x * (W - 1), a.y * (H - 1), this.t, '#ffe066');
    if (this.flash > 0) { ctx.fillStyle = `rgba(220,230,255,${this.flash * 0.9})`; ctx.fillRect(0, 0, W, H); }
  }

  drawOverlay(ctx) {
    const f = this.fire;
    ctx.textAlign = 'center';
    if (!f.lit && !this.done) {
      const bob = Math.sin(this.t * 5) * 1.5;
      label(ctx, f.x, f.y - 12 + bob, 'blink here to light the fire', '#ffd166', 6);
    }
    if (this.hold.active) {
      const p = this.hold.progress / HOLD_SECONDS;
      ctx.lineWidth = 1.4;
      ctx.strokeStyle = 'rgba(10,12,30,0.7)';
      ctx.beginPath(); ctx.arc(f.x, f.y - 3, 13, 0, Math.PI * 2); ctx.stroke();
      ctx.strokeStyle = '#59d8c8';
      ctx.beginPath(); ctx.arc(f.x, f.y - 3, 13, -Math.PI / 2, -Math.PI / 2 + p * Math.PI * 2); ctx.stroke();
      label(ctx, f.x, f.y - 20, `HOLD CALM  ${(HOLD_SECONDS - this.hold.progress).toFixed(1)}s`, '#59d8c8', 7);
    }
    this.particles.drawText(ctx);
  }

  // ---- camera (#33): hand casts and face mood. Extra inputs on top of the brain, never instead of it. ----

  // aim is 0..1 like the brain's. The fist (fireball) normally arrives through onBlink.
  onCast(spell, aim) {
    const G = this.G;
    if (!G || this.done) return;
    const w = G.world, f = this.fire;
    const x = Math.round(aim.x * (W - 1)), y = Math.round(aim.y * (H - 1));
    switch (spell) {
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
        this.castRain(x);
        floater(this.particles, x, 24, 'gentle rain', '#9ec9ff', 7);
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
        for (let i = 0; i < 3; i++) this.bolts.push({ pts: makeBolt(clamp(x + rand(-16, 16), 2, W - 3), clamp(y + rand(-8, 8), 4, H - 3)), life: 0.35 + i * 0.12 });
        this.castRain(x);
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
        G.advisor.say('cast-peace', 'Peace descends. Grug has apologised to the mammoth.', { mood: 'happy', priority: 1, cooldown: 25 });
        break;
      }
    }
  }

  castRain(cx) {
    this.particles.burst(60, () => ({ x: cx + rand(-60, 60), y: rand(-10, H * 0.7), vx: -4, vy: rand(70, 100), life: rand(0.5, 1), maxLife: 1, size: 1, color: 'rgba(158,201,255,0.75)' }));
  }

  // Face mood. Joy: the tribe dances and believes. Anger: it gets restless. Surprise: the sky
  // flashes. Sadness: the night gets darker. Sent on every change of dominant mood (including
  // back to neutral) and again every few seconds while a mood stays strong.
  onMood(mood) {
    const G = this.G;
    if (!G) return;
    this.baseDarkness ??= this.darkness;
    this.darkness = clamp(this.baseDarkness + (mood.dominant === 'sadness' ? 0.25 : 0), 0, 0.9);
    const home = this.living().filter((p) => !p.colony && p.state !== 'embark' && p.state !== 'aboard');
    if (mood.dominant !== 'joy') for (const p of home) if (p.moodDance) { p.moodDance = false; p.state2 = null; }
    if (this.done) return;
    if (mood.dominant === 'joy') {
      for (const p of home) {
        p.faith = clamp(p.faith + 0.12);
        p.fear = clamp(p.fear - 0.15);
        if (['idle', 'warm', 'huddle', 'dance', 'cower'].includes(p.state)) { p.state2 = 'dance'; p.moodDance = true; }
      }
    } else if (mood.dominant === 'anger') {
      for (const p of home) { p.fear = clamp(p.fear + 0.1); p.faith = clamp(p.faith - 0.05); }
      if (mood.reason === 'change') floater(this.particles, this.fire.x, this.fire.y - 16, 'grumbling', '#ff8a9c', 6);
    } else if (mood.dominant === 'surprise') {
      this.flash = Math.max(this.flash, 0.3);
      this.bolts.push({ pts: makeBolt(rand(20, W - 20), rand(30, H - 20)), life: 0.3 });
      G.audio.thunder();
      for (const p of home) p.fear = clamp(p.fear + 0.05);
    }
  }
}

export function label(ctx, x, y, text, color, size = 6) {
  if (ctx.__project3d) {
    const pr = ctx.__project3d(x, y);
    if (pr) { x = pr[0]; y = pr[1]; }
  }
  ctx.font = `${size}px "Pixelify Sans", monospace`;
  ctx.textAlign = 'center';
  ctx.fillStyle = 'rgba(8,10,24,0.85)';
  ctx.fillText(text, x + 0.6, y + 0.6);
  ctx.fillStyle = color;
  ctx.fillText(text, x, y);
}
