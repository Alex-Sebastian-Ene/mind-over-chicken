// ERA III · SPACE AGE (finale)
// Engineers carry parts to the pad: focus speeds them up, stress (brain.stress: EEG agitation, or
// an angry face on camera) makes them drop things. Calm keeps citizens brave enough to board,
// stress scares them off. Then the 10-second countdown runs on the personal focus classifier
// while your stress has to stay below STRESSED (focus itself does not read as stress). A broken
// count holds; a long hold scrubs and resets the countdown, never the era. Then liftoff, and a
// new world.

import { W, H, Particles, clamp, dist, rand, chance, floater, pick, mulberry32 } from '../sim/util.js';
import { walk, animate, villagerLook } from '../sim/agents.js';
import { drawPerson, drawChicken, drawRocket, drawGantry } from '../render/sprites.js';
import { label } from './stone.js';
import { STRESSED } from '../brain/brain.js';

const ENGINEERS = 10;
const CITIZENS = 28;
const MIN_PASSENGERS = 8;
const COUNT_FROM = 10;
const SCRUB_AFTER = 5;

export class SpaceAge {
  constructor() {
    this.id = 'space';
    this.era = 'space';
    this.label = 'Era III · Space Age';
    this.intro = {
      title: 'Space Age',
      goal: 'Build the rocket, board the citizens, survive a 10-second countdown.',
      how: [['Focus', 'Speeds up the engineers, and holds the final countdown.'], ['Calm', 'Keeps citizens brave enough to board.'], ['Stress', 'Keep it low, or parts get dropped and the count holds.']],
    };
    this.powers = ['focus', 'calm', 'stress'];
    this.darkness = 0;
  }

  enter(G) {
    this.G = G;
    const w = G.world;
    this.layer = w.paint('space');
    this.particles = new Particles();
    this.t = 0;
    this.phase = 'build';
    this.progress = 0;
    this.passengers = 0;
    this.count = COUNT_FROM;
    this.holdT = 0;
    this.scrubs = 0;
    this.rocketY = 0;
    this.rocketV = 0;
    this.voyageT = 0;
    this.drops = 0;
    this.done = false;
    const pad = w.sites.pad, hangar = w.sites.hangar, crowd = w.sites.crowd;
    this.pad = pad;
    this.engineers = [];
    for (let i = 0; i < ENGINEERS; i++) {
      const p = w.randomLand(hangar.x, hangar.y, 1, 6);
      this.engineers.push({ ...p, shirt: '#e0a43a', hair: pick(['#2b1d14', '#5a3b22', '#1c1410']), skin: pick(['#e0b48a', '#a8754f', '#c99470']), hat: '#f2c14e', state: 'load', timer: rand(0, 1.5), carry: null, frame: 0, ft: rand(0, 4), facing: 1, side: chance(0.5) ? 1 : -1 });
    }
    this.citizens = [];
    for (let i = 0; i < CITIZENS; i++) {
      const p = w.randomLand(crowd.x, crowd.y, 1, 14);
      this.citizens.push({ ...p, ...villagerLook('space'), state: 'watch', spot: null, frame: 0, ft: rand(0, 5), facing: 1, side: chance(0.5) ? 1 : -1, courage: rand(0.2, 0.6) });
    }
    this.chicken = { x: crowd.x + 3, y: crowd.y + 2, frame: 0, ft: 0, facing: 1, side: 1, state: 'wait', halo: true };
    this.boardTimer = 0;
    this.stars = Array.from({ length: 140 }, (_, i) => { const r = mulberry32(i * 31 + 7); return { x: r() * W, y: r() * H, b: r() }; });
    this.newWorld = makePlanetTexture(G.world.seed + 99);
    G.brain.setCue(null);
    G.advisor.say('intro', 'My lord, we have built a rocket. Well, the bottom of one. Focus speeds the engineers. Stress makes them drop things.', { mood: 'nervous', priority: 3, cooldown: 999 });
  }

  focusOk() { return this.G.brain.focus >= this.focusThr(); }
  focusThr() { return this.G.brain.threshold('focus', 0.5); }
  stressed() { return (this.G.brain.stress ?? 1) > STRESSED; }

  update(dt) {
    const G = this.G, w = G.world, brain = G.brain;
    this.t += dt;
    this.particles.update(dt);
    const pad = this.pad, hangar = w.sites.hangar;

    if (this.phase === 'build' || this.phase === 'countdown') {
      // Engineers
      const speed = 8 + 16 * brain.focus;
      for (const e of this.engineers) {
        animate(e, dt, 4 + brain.focus * 6);
        e.timer -= dt;
        if (e.state === 'load') { if (walk(e, hangar.x + e.side * 2, hangar.y + 1, speed, dt, w) && e.timer <= 0) { e.state = 'deliver'; e.carry = pick(['#c7ccd6', '#e04a3a', '#3e8ed0', '#f2c14e']); } }
        else if (e.state === 'deliver') {
          if (walk(e, pad.x - 5 * e.side, pad.y + 1, speed, dt, w)) {
            if (this.stressed() && chance(0.3)) {
              this.drops++;
              this.progress = Math.max(0, this.progress - 0.01);
              this.particles.burst(8, () => ({ x: e.x, y: e.y - 2, vx: rand(-12, 12), vy: rand(-16, -4), g: 30, life: 0.8, maxLife: 0.8, size: 1, color: pick(['#999', '#ccc', '#ff9a2e']) }));
              floater(this.particles, e.x, e.y - 8, 'dropped it', '#ff8a9c', 5);
              G.advisor.say('drop', pick(['Someone dropped the fuel valve. I will not say who. It was because of your stress.', 'Another wrench in the ocean. The engineers can feel your stress, my lord.']), { mood: 'panic', priority: 2, cooldown: 10 });
            } else if (this.progress < 1) {
              this.progress = Math.min(1, this.progress + 0.03);
              G.audio.tick();
            }
            e.carry = null; e.state = 'load'; e.timer = rand(0.2, 0.8);
          }
        }
      }
      if (this.progress >= 1 && !this.builtSaid) { this.builtSaid = true; G.audio.good(); G.advisor.say('built', 'The rocket is finished. It is very tall and very pointy. Now we need passengers.', { mood: 'happy', priority: 3 }); }

      // Boarding: calm makes citizens brave, stress sends them running.
      const boarding = this.progress >= 0.5;
      if (boarding && !this.boardSaid) { this.boardSaid = true; G.advisor.say('board', 'Citizens are boarding. Calm keeps them brave. The sacred chicken has seat 1A, obviously.', { mood: 'happy', priority: 2 }); this.chicken.state = 'board'; }
      this.boardTimer += dt;
      for (const c of this.citizens) {
        animate(c, dt, 6);
        c.courage = clamp(c.courage + dt * (0.12 * brain.calm - 0.1 * (brain.stress ?? 1) - 0.02));
        if (c.state === 'watch') {
          if (!c.spot || walk(c, c.spot.x, c.spot.y, 5, dt, w)) c.spot = w.randomLand(w.sites.crowd.x, w.sites.crowd.y, 1, 14);
          if (boarding && c.courage > 0.55 && this.boardTimer > 0.5) { c.state = 'board'; this.boardTimer = 0; }
        } else if (c.state === 'board') {
          if (c.courage < 0.25 && this.stressed()) { c.state = 'flee'; floater(this.particles, c.x, c.y - 7, 'nope', '#f2e8d5', 5); }
          else if (walk(c, pad.x, pad.y, 9, dt, w)) { c.state = 'aboard'; this.passengers++; G.audio.pop(); }
        } else if (c.state === 'flee') {
          if (walk(c, w.sites.crowd.x, w.sites.crowd.y, 14, dt, w)) c.state = 'watch';
        }
      }
      const ch = this.chicken;
      animate(ch, dt, 5);
      if (ch.state === 'board' && walk(ch, pad.x, pad.y, 7, dt, w)) { ch.state = 'aboard'; floater(this.particles, pad.x, pad.y - 26, 'Chicken in seat 1A', '#ffe066', 6); }

      if (this.phase === 'build' && this.progress >= 1 && this.passengers >= MIN_PASSENGERS) {
        this.phase = 'countdown';
        this.count = COUNT_FROM; this.holdT = 0;
        brain.setCue('focus');
        G.audio.bell();
        G.advisor.say('count', 'Final countdown. Hold focus and keep your nerve. Ten seconds. The whole species is watching. No pressure.', { mood: 'nervous', priority: 4, cooldown: 0 });
      }
    }

    if (this.phase === 'countdown') {
      const ok = this.focusOk() && !this.stressed();
      if (ok) {
        const before = Math.ceil(this.count);
        this.count -= dt;
        this.holdT = 0;
        if (Math.ceil(this.count) !== before) G.audio.tick();
      } else {
        this.holdT += dt;
        G.advisor.say('hold', this.focusOk() ? 'HOLD! Your stress, my lord! Breathe!' : 'HOLD! Hold the count! Focus, my lord!', { mood: 'panic', priority: 3, cooldown: 4 });
        if (this.holdT > SCRUB_AFTER) {
          this.scrubs++;
          this.count = COUNT_FROM; this.holdT = 0;
          G.audio.bad();
          G.source.addStartle(0.2);
          G.advisor.say('scrub', 'Launch scrubbed. We reset the clock. Nobody panic. I am panicking.', { mood: 'panic', priority: 4, cooldown: 0 });
        }
      }
      if (this.count <= 0) {
        this.phase = 'launch';
        brain.setCue(null);
        G.audio.rumble(4);
        G.advisor.say('liftoff', 'LIFTOFF! From a fire pit to the stars. I would like to thank the mammoths.', { mood: 'happy', priority: 5, cooldown: 0 });
        for (const c of this.citizens) if (c.state !== 'aboard') c.state = 'cheer';
      }
    }

    if (this.phase === 'launch') {
      this.rocketV += dt * (6 + this.rocketV * 1.2);
      this.rocketY += this.rocketV * dt;
      G.shake = Math.max(G.shake || 0, 1.5);
      for (let i = 0; i < 6; i++) this.particles.add({ x: pad.x + rand(-3, 3), y: pad.y - this.rocketY + 2, vx: rand(-25, 25), vy: rand(0, 14), drag: 1.5, grow: 1.2, life: rand(1, 2.5), maxLife: 2.5, size: 1, color: pick(['rgba(230,230,235,0.8)', 'rgba(200,200,210,0.7)', 'rgba(255,190,90,0.7)']) });
      for (const c of this.citizens) animate(c, dt, 8);
      if (this.rocketY > 220) { this.phase = 'voyage'; this.voyageT = 0; }
    }

    if (this.phase === 'voyage') {
      this.voyageT += dt;
      if (this.voyageT > 7.5 && !this.done) {
        this.done = true;
        G.audio.fanfare();
        G.advisor.say('arrive', 'We have landed on a new world. The chicken stepped out first. History will remember the chicken.', { mood: 'happy', priority: 5, cooldown: 0 });
        G.finish({ passengers: this.passengers, scrubs: this.scrubs, drops: this.drops });
      }
    }
  }

  objectives() {
    if (this.phase === 'voyage') return [{ label: 'Destination', value: 'a new world', done: this.voyageT > 7 }];
    return [
      { label: 'Rocket', value: `${Math.floor(this.progress * 100)}%`, done: this.progress >= 1 },
      { label: 'Passengers', value: `${this.passengers}/${MIN_PASSENGERS}+`, done: this.passengers >= MIN_PASSENGERS },
      { label: 'Countdown', value: this.phase === 'countdown' ? `T−${Math.max(0, Math.ceil(this.count))}` : this.phase === 'launch' ? 'liftoff' : 'not started', done: this.phase === 'launch' },
    ];
  }

  lights() {
    const L = [];
    const pad = this.pad;
    if (pad) {
      if (this.phase === 'launch') {
        L.push({ x: pad.x, y: pad.y - this.rocketY, r: 60 });
      } else if (this.phase === 'countdown' && this.count < 3) {
        L.push({ x: pad.x, y: pad.y, r: 35 + Math.sin(this.t * 20) * 5 });
      } else {
        L.push({ x: pad.x, y: pad.y, r: 24 });
      }
    }
    return L;
  }

  draw3d(engine, _dt) {
    if (this.phase === 'voyage') return;
    const pad = this.pad;
    if (!pad) return;

    // 3D Point Lights: pad beacons and launch ignition
    if (this.phase === 'launch') {
      engine.add_3d_light(pad.x, 10.0 + this.rocketY, pad.y, 65.0, 3.0);
    } else if (this.phase === 'countdown' && this.count < 3) {
      engine.add_3d_light(pad.x, 8.0, pad.y, 40.0 + Math.sin(this.t * 20.0) * 5.0, 2.0);
    } else {
      engine.add_3d_light(pad.x, 8.0, pad.y, 24.0, 1.2);
    }

    // Billboards
    // Rocket (SPRITE_ROCKET = 12)
    engine.add_billboard(12, pad.x, pad.y, 1, Math.floor(this.progress * 100), this.phase === 'launch' ? 1 : 0);

    // Engineers (SPRITE_VILLAGER = 0)
    for (const e of this.engineers) {
      engine.add_billboard(0, e.x, e.y, e.facing || 1, e.frame || 0, 0);
    }

    // Citizens (SPRITE_VILLAGER = 0)
    for (const c of this.citizens) {
      if (c.state === 'aboard') continue;
      const pose = c.state === 'cheer' ? 1 : c.state === 'flee' ? 2 : 0;
      engine.add_billboard(0, c.x, c.y, c.facing || 1, c.frame || 0, pose);
    }

    // Chicken (SPRITE_CHICKEN = 3)
    if (this.chicken.state !== 'aboard') {
      engine.add_billboard(3, this.chicken.x, this.chicken.y, this.chicken.facing || 1, this.chicken.frame || 0, 1);
    }

    // Aim reticle (SPRITE_RETICLE = 13)
    const aim = this.G.brain?.aim;
    if (aim) {
      engine.add_billboard(13, aim.x * (W - 1), aim.y * (H - 1), 1, 0, 0);
    }
  }

  draw(ctx) {
    if (this.phase === 'voyage') return this.drawVoyage(ctx);
    const G = this.G;
    const base = G.render3d ? (G.world?.layers3d?.space || this.layer) : this.layer;
    ctx.drawImage(base, 0, 0);
    if (!G.render3d) G.world.drawWater(ctx, this.t);
    // In 3D, draw3d stands rocket, pad, engineers, citizens and chicken up as billboards
    if (G.render3d) {
      this.particles.draw(ctx);
      return;
    }
    const pad = this.pad;
    const actors = [];
    actors.push({ y: pad.y - 0.5, d: () => drawGantry(ctx, pad.x + 5, pad.y, 24) });
    actors.push({ y: pad.y, d: () => drawRocket(ctx, pad.x, pad.y - this.rocketY, this.progress, this.t, this.phase === 'launch' ? 1 : this.phase === 'countdown' && this.count < 3 ? 0.3 : 0) });
    for (const e of this.engineers) actors.push({ y: e.y, d: () => drawPerson(ctx, e.x, e.y, { ...e, pose: 'walk' }) });
    for (const c of this.citizens) { if (c.state === 'aboard') continue; actors.push({ y: c.y, d: () => drawPerson(ctx, c.x, c.y, { ...c, pose: c.state === 'cheer' ? 'dance' : c.state === 'flee' ? 'panic' : 'walk' }) }); }
    if (this.chicken.state !== 'aboard') actors.push({ y: this.chicken.y, d: () => drawChicken(ctx, this.chicken.x, this.chicken.y, this.chicken) });
    actors.sort((a, b) => a.y - b.y).forEach((a) => a.d());
    this.particles.draw(ctx);
  }

  drawVoyage(ctx) {
    const t = this.voyageT;
    ctx.fillStyle = '#05060f'; ctx.fillRect(0, 0, W, H);
    for (const s of this.stars) { ctx.fillStyle = s.b > 0.8 ? '#ffffff' : s.b > 0.5 ? '#9fb0d9' : '#4a5580'; ctx.fillRect(Math.round((s.x - t * 6 * s.b) % W + W) % W, Math.round(s.y), 1, 1); }
    // Home: the island itself, seen from orbit, shrinking away.
    const k = clamp(t / 4);
    const homeR = 150 * (1 - k) + 18 * k;
    const hx = 60 * (1 - k) + 40 * k + 100 * (1 - k), hy = 220 * (1 - k) + 140 * k;
    ctx.save(); ctx.beginPath(); ctx.arc(hx, hy, homeR, 0, Math.PI * 2); ctx.clip();
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(this.layer, hx - homeR * 1.6, hy - homeR * 0.9, homeR * 3.2, homeR * 1.8);
    ctx.restore();
    // New world growing on the right.
    const k2 = clamp((t - 2) / 5.5);
    const nr = 10 + 110 * k2 * k2;
    const nx = 270 - 40 * k2, ny = 70 + 90 * k2 * k2;
    ctx.save(); ctx.beginPath(); ctx.arc(nx, ny, nr, 0, Math.PI * 2); ctx.clip();
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(this.newWorld, nx - nr, ny - nr, nr * 2, nr * 2);
    ctx.restore();
    // Rocket travelling between them.
    const rk = clamp(t / 7.2);
    const rx = 110 + (nx - 110) * rk, ry = 120 + (ny - nr - 6 - 120) * rk;
    ctx.save(); ctx.translate(Math.round(rx), Math.round(ry)); ctx.rotate(rk < 0.85 ? 1.1 : 1.1 * (1 - (rk - 0.85) / 0.15));
    drawRocket(ctx, 0, 11, 1, this.t, rk < 0.95 ? 0.6 : 0.2);
    ctx.restore();
  }

  drawOverlay(ctx) {
    const G = this.G, brain = G.brain;
    this.particles.drawText(ctx);
    const pad = this.pad;
    if (this.phase === 'build') {
      const bw = 30;
      ctx.fillStyle = 'rgba(14,15,26,0.8)'; ctx.fillRect(pad.x - bw / 2, pad.y - 34, bw, 3);
      ctx.fillStyle = '#ffcf4a'; ctx.fillRect(pad.x - bw / 2, pad.y - 34, bw * this.progress, 3);
      label(ctx, pad.x, pad.y - 37, `rocket ${Math.floor(this.progress * 100)}%`, '#f2e8d5', 5);
      if (this.progress >= 1 && this.passengers < MIN_PASSENGERS) label(ctx, W / 2, 24, `Need ${MIN_PASSENGERS - this.passengers} more passengers. Stay calm so they dare to board.`, '#59d8c8', 6);
    }
    if (this.phase === 'countdown') {
      const n = Math.max(0, Math.ceil(this.count));
      const ok = this.focusOk() && !this.stressed();
      label(ctx, W / 2, 46, ok ? `T−${n}` : 'HOLD', ok ? '#f2e8d5' : '#ff5a72', 30);
      const fpct = Math.round(brain.focus * 100), fthr = Math.round(this.focusThr() * 100);
      label(ctx, W / 2 - 48, 60, `focus ${fpct}% (need ${fthr}%)`, this.focusOk() ? '#ffcf4a' : '#ff8a9c', 6);
      label(ctx, W / 2 + 48, 60, `stress ${Math.round((brain.stress ?? 0) * 100)}% (max ${Math.round(STRESSED * 100)}%)`, this.stressed() ? '#ff8a9c' : '#7ee08a', 6);
      if (this.holdT > 0) {
        ctx.fillStyle = 'rgba(14,15,26,0.8)'; ctx.fillRect(W / 2 - 30, 66, 60, 2);
        ctx.fillStyle = '#ff5a72'; ctx.fillRect(W / 2 - 30, 66, 60 * clamp(this.holdT / SCRUB_AFTER), 2);
        label(ctx, W / 2, 74, 'scrub if the hold runs out', '#ff8a9c', 5);
      }
    }
    if (this.phase === 'voyage') {
      label(ctx, W / 2, 24, this.voyageT < 3.5 ? 'Leaving home.' : 'A new world.', '#f2e8d5', 12);
    }
  }

  // ---- camera (#33): hand casts and face mood. Extra inputs on top of the brain, never instead of it. ----
  // They never touch the countdown itself: that stays a focus-and-nerve challenge.

  // aim is 0..1 like the brain's.
  onCast(spell, aim) {
    const G = this.G;
    if (!G || this.done || this.phase === 'launch' || this.phase === 'voyage') return;
    const pad = this.pad, crowd = G.world.sites.crowd;
    const x = Math.round(aim.x * (W - 1)), y = Math.round(aim.y * (H - 1));
    const watching = this.citizens.filter((c) => c.state !== 'aboard');
    switch (spell) {
      case 'fireball': {
        G.stats.lightning++;
        G.audio.thunder();
        G.shake = Math.max(G.shake || 0, 0.8);
        this.particles.burst(22, () => ({ x, y, vx: rand(-30, 30), vy: rand(-40, 5), g: 50, life: 0.7, maxLife: 0.7, size: 1, color: pick(['#fff4c2', '#ffd166', '#ff9a2e', '#e8452c']) }));
        for (const c of watching) if (dist(x, y, c.x, c.y) < 20) c.courage = clamp(c.courage - 0.3);
        floater(this.particles, x, y - 8, dist(x, y, pad.x, pad.y) < 12 ? 'engine test?' : 'fireball', '#ff9a2e', 6);
        break;
      }
      case 'spawn': {
        // Open palm: a crate of rocket parts lands on the pad.
        const more = this.progress < 1;
        if (more) this.progress = Math.min(1, this.progress + 0.08);
        this.particles.burst(12, () => ({ x: pad.x + rand(-4, 4), y: pad.y - 20, vx: rand(-6, 6), vy: rand(15, 30), g: 40, life: 0.6, maxLife: 0.6, size: 1, color: pick(['#c7ccd6', '#e04a3a', '#3e8ed0', '#f2c14e']) }));
        floater(this.particles, pad.x, pad.y - 30, more ? '+rocket parts' : 'spare parts', '#ffcf4a', 6);
        G.audio.coin();
        break;
      }
      case 'rain': {
        for (const c of watching) c.courage = clamp(c.courage + 0.12);
        this.castRain(x);
        floater(this.particles, x, 24, 'cool rain', '#9ec9ff', 7);
        break;
      }
      case 'bless': {
        // Thumb up: courage. Citizens who were wavering decide to board.
        for (const c of watching) c.courage = clamp(c.courage + 0.3);
        this.particles.burst(20, () => ({ x: crowd.x + rand(-14, 14), y: crowd.y + rand(-8, 6), vx: rand(-3, 3), vy: rand(-14, -4), life: 1.2, maxLife: 1.2, size: 1, color: pick(['#ffe066', '#fff4c2']) }));
        floater(this.particles, crowd.x, crowd.y - 14, 'courage!', '#ffe066', 7);
        G.audio.good();
        G.advisor.say('cast-bless', 'The crowd feels blessed. Three of them asked for window seats.', { mood: 'happy', priority: 1, cooldown: 25 });
        break;
      }
      case 'disaster': {
        G.audio.thunder();
        G.shake = Math.max(G.shake || 0, 2);
        this.castRain(x);
        this.particles.burst(16, () => ({ x: x + rand(-8, 8), y: y - rand(0, 30), vx: 0, vy: 0, life: 0.25, maxLife: 0.25, size: 1, color: '#ffffff' }));
        for (const c of watching) if (dist(x, y, c.x, c.y) < 40) c.courage = clamp(c.courage - 0.35);
        if (this.phase === 'build' && dist(x, y, pad.x, pad.y) < 24 && this.progress > 0) {
          this.progress = Math.max(0, this.progress - 0.05);
          floater(this.particles, pad.x, pad.y - 30, 'storm damage', '#ff8a9c', 6);
        } else floater(this.particles, x, y - 12, 'STORM', '#9ec9ff', 9);
        G.advisor.say('cast-storm', 'A storm over the launch site? Flight Director to god: please stop.', { mood: 'panic', priority: 2, cooldown: 20 });
        break;
      }
      case 'peace': {
        for (const c of watching) { c.courage = clamp(c.courage + 0.1); if (c.state === 'flee') c.state = 'watch'; }
        floater(this.particles, crowd.x, crowd.y - 14, 'peace', '#9be38a', 7);
        break;
      }
    }
  }

  castRain(cx) {
    this.particles.burst(60, () => ({ x: cx + rand(-60, 60), y: rand(-10, H * 0.7), vx: -4, vy: rand(70, 100), life: rand(0.5, 1), maxLife: 1, size: 1, color: 'rgba(158,201,255,0.75)' }));
  }

  // Face mood. Joy and anger push the crowd's courage up or down, surprise flashes the sky,
  // sadness dims the launch site.
  onMood(mood) {
    const G = this.G;
    if (!G) return;
    this.baseDarkness ??= this.darkness;
    this.darkness = clamp(this.baseDarkness + (mood.dominant === 'sadness' ? 0.3 : 0), 0, 0.9);
    if (this.done || this.phase === 'launch' || this.phase === 'voyage') return;
    const crowd = G.world.sites.crowd, first = mood.reason === 'change';
    const watching = this.citizens.filter((c) => c.state !== 'aboard');
    if (mood.dominant === 'joy') {
      for (const c of watching) c.courage = clamp(c.courage + 0.1);
      if (first) floater(this.particles, crowd.x, crowd.y - 14, 'the crowd cheers', '#9be38a', 6);
    } else if (mood.dominant === 'anger') {
      for (const c of watching) c.courage = clamp(c.courage - 0.06);
      if (first) floater(this.particles, crowd.x, crowd.y - 14, 'uneasy murmurs', '#ff8a9c', 6);
    } else if (mood.dominant === 'surprise') {
      G.audio.thunder();
      G.shake = Math.max(G.shake || 0, 1);
      const sx = rand(30, W - 30);
      this.particles.burst(18, () => ({ x: sx + rand(-4, 4), y: rand(0, 50), vx: 0, vy: 0, life: 0.3, maxLife: 0.3, size: 1, color: '#ffffff' }));
    }
  }
}

// A small alien planet texture for the finale.
function makePlanetTexture(seed) {
  const c = document.createElement('canvas');
  c.width = 64; c.height = 64;
  const ctx = c.getContext('2d');
  const r = mulberry32(seed);
  const cols = ['#2a6f7a', '#3f8f86', '#7cc0a0', '#c9b26b', '#8f5fa8', '#6b4a8a'];
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
    const n = Math.sin(x * 0.21 + Math.sin(y * 0.17) * 2) + Math.cos(y * 0.23 + Math.sin(x * 0.13) * 2) + r() * 0.6;
    ctx.fillStyle = cols[clamp(Math.floor((n + 2.3) / 4.6 * cols.length), 0, cols.length - 1)];
    ctx.fillRect(x, y, 1, 1);
  }
  const g = ctx.createRadialGradient(22, 22, 4, 32, 32, 34);
  g.addColorStop(0, 'rgba(255,255,255,0.12)'); g.addColorStop(0.7, 'rgba(0,0,0,0)'); g.addColorStop(1, 'rgba(0,0,10,0.55)');
  ctx.fillStyle = g; ctx.fillRect(0, 0, 64, 64);
  return c;
}
