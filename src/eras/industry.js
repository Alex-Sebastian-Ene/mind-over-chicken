// ERA II · INDUSTRY
// The Goat Exchange. Prices come from trader agents, half fundamentalists (goats are worth
// what the economy produces) and half chartists (goats are worth whatever they did yesterday,
// plus vibes). Your stress (brain.stress: EEG agitation, or an angry face on camera; about 0.15
// at rest) adds noise and herding, so bubbles and crashes emerge from the agents instead of being
// scripted. Fear spreads between neighbours, unrest turns into strikes at the factory gate and,
// if fear runs hot, riots.
// Brain challenge: the Goat IPO, 8 s of steady nerves (stress below STRESSED).

import { W, H, Particles, clamp, dist, rand, chance, gauss, floater, pick } from '../sim/util.js';
import { walk, animate, villagerLook, sail } from '../sim/agents.js';
import { drawPerson, drawGoat, drawChicken, drawBoat, drawReticle } from '../render/sprites.js';
import { label } from './stone.js';
import { STRESSED } from '../brain/brain.js';

const N = 38;
const GDP_TARGET = 620;
const IPO_SECONDS = 8;
const TICK = 0.2;

export class Industry {
  constructor() {
    this.id = 'industry';
    this.era = 'industry';
    this.label = 'Era II · Industry';
    this.intro = {
      title: 'Industry',
      goal: `Reach ${GDP_TARGET} goats of GDP and land the Goat IPO.`,
      how: [['Stress', 'Your stress drives market volatility. Stay steady or the goats stampede.'], ['Calm', 'A serene god calms the traders and cools the strikers.']],
    };
    this.powers = ['stress', 'calm'];
    this.darkness = 0.12;
  }

  enter(G) {
    this.G = G;
    const w = G.world;
    this.layer = w.paint('industry');
    this.particles = new Particles();
    this.t = 0;
    this.price = 100;
    this.value = 100;
    this.momentum = 0;
    this.history = [];
    this.peak = 100;
    this.gdp = 0;
    this.gdpRate = 0;
    this.confidence = 1;
    this.ipo = { state: 'waiting', progress: 0, attempts: 0 };
    this.tick = 0;
    this.crashCooldown = 0;
    this.recessions = 0;
    this.headlines = ['GOAT EXCHANGE OPENS FOR BUSINESS', 'ANALYSTS: "GOATS ARE THE FUTURE"'];
    this.tickerX = W;
    this.generalStrike = 0;
    this.done = false;
    this.gini = 0;
    const houses = w.structures.filter((s) => s.kind === 'house');
    this.agents = [];
    for (let i = 0; i < N; i++) {
      const home = houses.length ? houses[i % houses.length] : w.sites.village;
      const rich = i < 6;
      const fx = w.sites.factory, exs = w.sites.exchange, pk = w.sites.picket;
      this.agents.push({
        x: home.x + rand(-2, 2), y: home.y + 1, home, ...villagerLook('industry'),
        workSpot: w.randomLand(fx.x, fx.y + 1, 0, 6), tradeSpot: w.randomLand(exs.x, exs.y + 2, 0, 6), picketSpot: w.randomLand(pk.x, pk.y, 0, 9),
        cash: rich ? rand(300, 500) : rand(20, 80), goats: rich ? 8 : 1,
        greed: rand(0.1, 0.9), fear: rand(0.05, 0.2), unrest: rand(0, 0.2),
        state: i % 5 < 3 ? 'work' : 'commute', timer: rand(0, 5), frame: 0, ft: rand(0, 5), facing: 1, side: chance(0.5) ? 1 : -1, spot: null, lastWorth: 0,
        hat: rich ? '#1c1410' : null,
      });
    }
    // Agents that start at work begin inside the factory.
    for (const a of this.agents) if (a.state === 'work') { a.x = a.workSpot.x; a.y = a.workSpot.y; }
    // Trade ships run goats to the other continents and bring back export earnings.
    this.ships = w.continents.slice(1).map((c, i) => ({ kind: 'ship', x: c.homeHarbor.dock.x, y: c.homeHarbor.dock.y, dest: c, leg: 'out', facing: 1, wait: 1 + i * 3, cargo: '#efe8dc', sail: i ? '#e8c46a' : '#f2ead8' }));
    this.exports = 0;
    this.goats = [];
    const ex = w.sites.exchange;
    for (let i = 0; i < 8; i++) { const p = w.randomLand(ex.x, ex.y + 6, 1, 6); this.goats.push({ ...p, frame: 0, ft: 0, facing: 1, side: 1, spot: null }); }
    this.chicken = { x: ex.x + 6, y: ex.y + 2, frame: 0, ft: 0, facing: -1, side: 1 };
    G.brain.setCue('rest');
    G.advisor.setEra('industry');
    G.advisor.say('intro', 'My lord! Bro. The Goat Exchange is open. Keep your nerves steady or the market goes full stampede.', { mood: 'happy', priority: 3, cooldown: 999 });
  }

  // The traders' arousal, 0..1: the god's stress (about 0.15 at rest, 1 when agitated).
  arousal() { return clamp(this.G.brain.stress ?? 0); }

  marketTick() {
    const G = this.G, brain = G.brain;
    const ar = this.arousal();
    this.value = 100 * (1 + (this.gdp / GDP_TARGET) * 0.7) * (this.ipo.state === 'done' ? 1.15 : 1);
    const herd = 1 + 1.6 * ar;
    let demand = 0;
    for (const a of this.agents) {
      if (a.state === 'riot') continue; // strikers still buy and sell goats
      const fundamental = 0.5 * Math.log(this.value / this.price);
      const chart = herd * this.momentum * 22;
      const r = (1 - a.greed) * fundamental + a.greed * chart - a.fear * 0.03 + gauss() * 0.01;
      if (r > 0.004 && a.cash > this.price * 0.3) { demand++; a.cash -= this.price * 0.3; a.goats += 0.3; }
      else if (r < -0.004 && a.goats > 0.3) { demand--; a.cash += this.price * 0.3; a.goats -= 0.3; }
    }
    // Exogenous news every so often. Agents decide whether it becomes a bubble or a panic.
    this.newsT = (this.newsT ?? 10) - TICK;
    if (this.newsT <= 0) {
      this.newsT = rand(10, 16);
      const good = chance(0.45);
      this.momentum += good ? 0.012 : -0.016;
      this.headlines.push(pick(good
        ? ['RUMOUR: GOATS CAN PREDICT RAIN', 'NEIGHBOURING TRIBE ORDERS 900 GOATS', 'GOAT CHEESE DECLARED A HEALTH FOOD']
        : ['RUMOUR: GOATS ARE JUST LARGE CHICKENS', 'GOAT FLU REPORTED IN NEXT VALLEY', 'MAN SELLS ALL GOATS, WILL NOT SAY WHY']));
    }
    const sigma = 0.003 + 0.016 * ar;
    // When traders are frightened, buyers vanish and quotes get marked down.
    const avgFear = this.agents.reduce((sum, a) => sum + a.fear, 0) / N;
    const dlnP = 0.05 * (demand / N) - 0.009 * (avgFear - 0.25) + sigma * gauss();
    this.price = Math.max(2, this.price * Math.exp(dlnP));
    this.momentum = this.momentum * 0.75 + dlnP * 0.25;
    this.history.push(this.price);
    if (this.history.length > 300) this.history.shift();
    const recent = this.history.slice(-50);
    this.peak = Math.max(...recent);

    this.crashCooldown -= TICK;
    if (this.price < this.peak * 0.8 && this.crashCooldown <= 0 && this.t > 4) {
      this.crashCooldown = 9;
      this.recessions++;
      G.stats.recessions++;
      G.audio.crash();
      G.source.addStartle(0.35);
      for (const a of this.agents) a.fear = clamp(a.fear + 0.3);
      this.confidence = Math.max(0.5, this.confidence - 0.15);
      const stress = Math.round((brain.stress ?? 0) * 100);
      this.headlines.push(`GOAT INDEX -${Math.round((1 - this.price / this.peak) * 100)}% AS GOD'S STRESS HITS ${stress}%`);
      G.advisor.say('crash', 'My lord, your stress has caused a goat recession.', { mood: 'panic', priority: 4, cooldown: 6 });
      const c = G.camera?.viewCenter?.() || G.world.sites.exchange;   // mid-screen (#129)
      floater(this.particles, c.x, c.y - 50, 'GOAT RECESSION', '#ff5a72', 12);
    }
    if (this.price > this.value * 1.45) {
      if (G.advisor.say('bubble', 'Goats are up forty percent. Nobody knows why. I have put the temple roof up as collateral.', { mood: 'happy', priority: 2, cooldown: 20 }))
        this.headlines.push('GOAT BUBBLE? "NO SUCH THING," SAYS MAN HOLDING 40 GOATS');
    }
    this.confidence = clamp(this.confidence + TICK * 0.02 * (1 - this.confidence), 0.5, 1.6);
  }

  update(dt) {
    const G = this.G, w = G.world, brain = G.brain;
    this.t += dt;
    this.particles.update(dt);
    this.tick += dt;
    while (this.tick >= TICK) { this.tick -= TICK; this.marketTick(); }
    const ar = this.arousal();

    // Inequality, every frame is fine at this size.
    const worth = this.agents.map((a) => a.cash + a.goats * this.price).sort((a, b) => a - b);
    const total = worth.reduce((s, v) => s + v, 0) || 1;
    let cum = 0, gsum = 0;
    worth.forEach((v) => { cum += v; gsum += cum; });
    this.gini = clamp(1 - (2 * gsum) / (worth.length * total) + 1 / worth.length);
    const wageCut = clamp((this.value - this.price) / this.value);

    const fx = w.sites.factory, ex = w.sites.exchange, picket = w.sites.picket;
    let working = 0, striking = 0, rioting = 0;
    const strikers = this.agents.filter((a) => a.state === 'strike' || a.state === 'riot');
    for (const a of this.agents) {
      animate(a, dt, 6);
      // Fear: contagion from neighbours, falling prices, eased by a calm god.
      let nf = 0, nn = 0;
      for (const b of this.agents) { if (b !== a && Math.abs(b.x - a.x) < 16 && Math.abs(b.y - a.y) < 16) { nf += b.fear; nn++; } }
      if (nn) a.fear += dt * 0.5 * (nf / nn - a.fear);
      // The god's stress frightens traders directly; the goats can smell fear.
      a.fear += dt * (0.25 * Math.min(1, Math.max(0, -this.momentum * 60)) + 0.3 * Math.max(0, ar - 0.3) - 0.06 - 0.4 * brain.calm * (ar < 0.3 ? 1 : 0.3));
      a.fear = clamp(a.fear);
      // Greed follows paper gains.
      const myWorth = a.cash + a.goats * this.price;
      if (a.lastWorth) a.greed = clamp(a.greed + (myWorth > a.lastWorth ? 0.02 : -0.015) * dt * 10);
      a.lastWorth = myWorth;
      // Unrest: low wages, inequality, fear, and other strikers nearby.
      let near = 0;
      for (const s of strikers) if (s !== a && dist(s.x, s.y, a.x, a.y) < 18) near++;
      const poor = a.cash < 60 ? 1 : 0.3;
      a.unrest += dt * (0.05 * wageCut * poor + 0.035 * this.gini * poor + 0.07 * a.fear + 0.02 * Math.min(near, 6) - 0.035 - 0.05 * brain.calm);
      a.unrest = clamp(a.unrest);

      if ((a.state === 'strike' || a.state === 'riot') && a.unrest < 0.32) { a.state = 'commute'; a.spot = null; }
      else if (a.state !== 'strike' && a.state !== 'riot' && a.unrest > 0.72) {
        a.state = 'strike'; a.spot = null;
        if (G.advisor.say('strike', pick(['The mill workers are striking. They want "wages". Is that a type of goat?', 'Strike at the factory gate. They have made signs. The signs are quite good.']), { mood: 'nervous', priority: 2, cooldown: 18 }))
          this.headlines.push('WORKERS DOWN TOOLS OVER GOAT-DENOMINATED WAGES');
      }
      this.move(a, dt, w, fx, ex, picket);
      if (a.state === 'work') working++;
      if (a.state === 'strike') striking++;
      if (a.state === 'riot') rioting++;
    }
    // Riots: a crowd of frightened strikers boils over.
    const angry = strikers.filter((a) => a.fear > 0.55).length;
    if (strikers.length >= 8 && angry >= 5) {
      for (const a of strikers) if (a.fear > 0.45) a.state = 'riot';
      if (Math.random() < dt * 8) this.particles.add({ x: picket.x + rand(-8, 8), y: picket.y - rand(0, 4), vx: rand(-6, 6), vy: rand(-18, -8), life: 1, maxLife: 1, size: 1, color: pick(['#ff9a2e', '#e8452c', '#777']) });
      if (G.advisor.say('riot', 'There is a riot at the mill. Disruptive, bro. Literally.', { mood: 'panic', priority: 3, cooldown: 20 })) this.headlines.push('RIOT AT THE MILL; GOATS UNHARMED, MOSTLY');
    } else for (const a of strikers) if (a.state === 'riot' && a.fear < 0.4) a.state = 'strike';

    // GDP
    this.gdpRate = working * 0.62 * this.confidence * clamp(0.65 + 0.35 * this.price / this.value, 0.4, 1.3) - rioting * 0.6;
    this.gdp = Math.max(0, this.gdp + this.gdpRate * dt);
    if (this.gdp >= GDP_TARGET) this.gdpReached = true;

    // Factory smoke
    if (Math.random() < dt * (2 + working * 0.3)) this.particles.add({ x: fx.x - 4 + (chance(0.5) ? 8 : 0), y: fx.y - 13, vx: rand(2, 6), vy: rand(-7, -4), grow: 0.5, life: 3, maxLife: 3, size: 1, color: 'rgba(90,85,85,0.7)' });

    // Trade ships. They stay in harbour while the market panics.
    const panic = this.agents.reduce((s, a) => s + a.fear, 0) / N > 0.55;
    for (const sh of this.ships) {
      if (sh.wait > 0) { sh.wait -= dt; sh.moving = false; continue; }
      if (panic) { sh.moving = false; continue; }
      sh.moving = true;
      const out = sh.leg === 'out';
      const to = out ? sh.dest.harbor.dock : sh.dest.homeHarbor.dock;
      if (sail(sh, to.x, to.y, 15, dt, w)) {
        sh.leg = out ? 'back' : 'out'; sh.wait = 1.2;
        if (out) sh.cargo = '#c9a35a';
        else {
          const earned = Math.round(14 * this.confidence);
          this.gdp += earned; this.exports += earned; sh.cargo = '#efe8dc';
          G.audio.coin();
          floater(this.particles, sh.x, sh.y - 8, `+${earned} exports`, '#ffcf5c', 6);
        }
      }
    }

    // Goats and the chicken
    for (const g of this.goats) { animate(g, dt, 3); if (!g.spot || walk(g, g.spot.x, g.spot.y, 3, dt, w)) g.spot = w.randomLand(ex.x, ex.y + 6, 1, 7); }
    animate(this.chicken, dt, 4);

    // The IPO
    const ipo = this.ipo;
    if (ipo.state === 'waiting' && this.gdp > GDP_TARGET * 0.33) {
      ipo.state = 'active'; ipo.progress = 0;
      brain.setCue('calm');
      G.audio.bell();
      G.advisor.say('ipo', 'It is IPO time. Steady nerves for eight seconds. Do not look at the ticker. Do NOT look at the ticker.', { mood: 'nervous', priority: 4, cooldown: 0 });
      this.headlines.push('GOAT EXCHANGE ANNOUNCES IPO OF... THE GOAT EXCHANGE');
    }
    if (ipo.state === 'active') {
      ipo.limit = STRESSED;
      if ((brain.stress ?? 1) <= ipo.limit) ipo.progress += dt;
      else {
        if (ipo.progress > 2) { ipo.attempts++; G.advisor.say('ipofail', 'Stress spiked, IPO wobbled. The goats can smell fear.', { mood: 'panic', priority: 3, cooldown: 6 }); }
        ipo.progress = Math.max(0, ipo.progress - dt * 2);
      }
      if (ipo.progress >= IPO_SECONDS) {
        ipo.state = 'done';
        brain.setCue('rest');
        this.confidence += 0.4;
        this.price *= 1.15;
        G.audio.fanfare();
        G.advisor.say('ipook', 'IPO priced! We are now richer than the concept of goats.', { mood: 'happy', priority: 4, cooldown: 0 });
        this.headlines.push('IPO SOARS; GOD DESCRIBED AS "WEIRDLY CHILL"');
        floater(this.particles, ex.x, ex.y - 14, 'IPO PRICED', '#59d8c8', 9);
      }
    }

    if (brain.calm > 0.75 && (brain.stress ?? 1) < 0.25) G.advisor.say('serene', 'Serene god, serene markets. This is the alpha the analysts keep talking about. Literally alpha waves.', { mood: 'happy', priority: 1, cooldown: 30 });

    // Ticker tape
    this.tickerX -= dt * 40;

    // Win / lose
    if (this.done) return;
    if (striking + rioting >= N * 0.8) this.generalStrike += dt; else this.generalStrike = Math.max(0, this.generalStrike - dt);
    if (this.generalStrike > 8) { this.done = true; G.fail('General strike. The economy has stopped. The goats have unionised.'); return; }
    if (this.price < 12) { this.done = true; G.fail('The Great Goat Depression. A goat is now worth less than its hat.'); return; }
    if (this.gdp >= GDP_TARGET && ipo.state === 'done') {
      this.done = true;
      brain.setCue(null);
      G.audio.fanfare();
      G.advisor.say('win', 'GDP target smashed. The engineers want to know if goats can fly. I said we would find out.', { mood: 'happy', priority: 5, cooldown: 999 });
      G.complete(3);
    }
  }

  move(a, dt, w, fx, ex, picket) {
    a.timer -= dt;
    switch (a.state) {
      case 'commute':
        if (walk(a, a.workSpot.x, a.workSpot.y, 14, dt, w)) { a.state = 'work'; a.timer = rand(4, 7); }
        break;
      case 'work':
        if (a.timer <= 0) { if (chance(0.3 + 0.4 * a.greed)) a.state = 'trade-go'; else a.timer = rand(3, 6); }
        break;
      case 'trade-go':
        if (walk(a, a.tradeSpot.x, a.tradeSpot.y, 14, dt, w)) { a.state = 'trade'; a.timer = rand(1.2, 2.5); }
        break;
      case 'trade':
        if (a.timer <= 0) a.state = 'commute';
        break;
      case 'strike':
      case 'riot': {
        if (!a.spot) a.spot = a.state === 'riot' ? w.randomLand(picket.x, picket.y, 0, 10) : a.picketSpot;
        const speed = a.state === 'riot' ? 18 : 10;
        if (walk(a, a.spot.x, a.spot.y, speed, dt, w) && a.state === 'riot') a.spot = null;
        break;
      }
    }
  }

  objectives() {
    const gdpDone = this.gdp >= GDP_TARGET || !!this.gdpReached;
    const gdpVal = gdpDone ? `${GDP_TARGET}/${GDP_TARGET}` : `${Math.floor(this.gdp)}/${GDP_TARGET}`;
    const st = this.ipo.state;
    const ipoProgress = Math.min(IPO_SECONDS, Math.ceil(this.ipo.progress));
    return [
      { label: 'GDP', value: gdpVal, done: gdpDone },
      { label: 'Goat IPO', value: st === 'done' ? 'priced' : st === 'active' ? `${ipoProgress}/${IPO_SECONDS}s` : 'soon', done: st === 'done' },
      { label: 'Goat index', value: this.price.toFixed(1), plain: true, bad: this.price < this.peak * 0.8 },
      { label: 'Striking', value: `${this.agents.filter((a) => a.state === 'strike' || a.state === 'riot').length}`, plain: true },
    ];
  }

  lights() {
    const L = [];
    const w = this.G?.world;
    if (w?.sites) {
      const fx = w.sites.factory, ex = w.sites.exchange;
      if (fx) L.push({ x: fx.x, y: fx.y, r: 24 + Math.sin(this.t * 5) * 2.0 });
      if (ex) L.push({ x: ex.x, y: ex.y, r: 20 });
    }
    return L;
  }

  draw3d(engine, _dt) {
    const w = this.G?.world;
    if (!w) return;
    const fx = w.sites.factory, ex = w.sites.exchange;

    // 3D Point Lights (smokestack furnace and market lanterns)
    if (fx) {
      engine.add_3d_light(fx.x, 8.0, fx.y, 30.0 + Math.sin(this.t * 5.0) * 2.0, 1.4);
    }
    if (ex) {
      engine.add_3d_light(ex.x, 8.0, ex.y, 22.0, 1.2);
    }

    // Billboards
    // Goats (SPRITE_GOAT = 2)
    for (const g of this.goats) {
      engine.add_billboard(2, g.x, g.y, g.facing || 1, g.frame || 0, 0);
    }

    // Agents: workers, traders, strikers (SPRITE_VILLAGER = 0)
    for (const a of this.agents) {
      if (a.state === 'work') continue; // inside factory
      const pose = a.state === 'strike' ? 1 : a.state === 'riot' ? 2 : 0;
      engine.add_billboard(0, a.x, a.y, a.facing || 1, a.frame || 0, pose);
    }

    // Chicken (SPRITE_CHICKEN = 3)
    engine.add_billboard(3, this.chicken.x, this.chicken.y, this.chicken.facing || 1, this.chicken.frame || 0, 0);

    // Aim reticle (SPRITE_RETICLE = 13)
    const aim = this.G.brain?.aim;
    if (aim) {
      const p = this.G.world.aimPoint(aim);
      engine.add_billboard(13, p.x, p.y, 1, 0, 0);
    }
  }

  draw(ctx) {
    const G = this.G;
    const base = G.render3d ? (G.world?.layers3d?.industry || this.layer) : this.layer;
    ctx.drawImage(base, 0, 0);
    if (!G.render3d) G.world.drawWater(ctx, this.t);
    // In 3D, draw3d stands workers, traders, goats and chicken up as billboards
    if (G.render3d) {
      for (const sh of this.ships) drawBoat(ctx, sh.x, sh.y, sh);
      this.particles.draw(ctx);
      return;
    }
    const actors = [];
    for (const g of this.goats) actors.push({ y: g.y, d: () => drawGoat(ctx, g.x, g.y, g) });
    for (const sh of this.ships) actors.push({ y: sh.y, d: () => drawBoat(ctx, sh.x, sh.y, sh) });
    for (const a of this.agents) {
      if (a.state === 'work') continue; // inside the factory
      const pose = a.state === 'strike' ? 'sign' : a.state === 'riot' ? 'panic' : a.state === 'trade' ? 'cheer' : 'walk';
      actors.push({ y: a.y, d: () => drawPerson(ctx, a.x, a.y, { ...a, pose, mark: a.fear > 0.7 && a.state === 'trade' ? '#ff5a72' : null }) });
    }
    actors.push({ y: this.chicken.y, d: () => drawChicken(ctx, this.chicken.x, this.chicken.y, this.chicken) });
    actors.sort((a, b) => a.y - b.y).forEach((a) => a.d());
    this.particles.draw(ctx);
    // Dusk haze, thicker with smog when the factory is busy.
    ctx.fillStyle = `rgba(255, 140, 60, ${0.08 + clamp(this.gdpRate / 40) * 0.06})`;
    ctx.fillRect(0, 0, G.world.width, G.world.height);
  }

  // Fixed to the screen, not the map (#129): the goat index chart and the ticker tape.
  drawScreenOverlay(ctx) {
    // Goat index chart
    const x0 = W - 92, y0 = 6, cw = 86, ch = 38;
    ctx.fillStyle = 'rgba(14,15,26,0.82)'; ctx.fillRect(x0, y0, cw, ch);
    ctx.strokeStyle = 'rgba(242,232,213,0.15)'; ctx.lineWidth = 0.4; ctx.strokeRect(x0, y0, cw, ch);
    const hist = this.history.slice(-140);
    if (hist.length > 2) {
      const lo = Math.min(...hist, this.value) * 0.95, hi = Math.max(...hist, this.value) * 1.05;
      const yOf = (v) => y0 + ch - 4 - ((v - lo) / (hi - lo)) * (ch - 14);
      ctx.strokeStyle = 'rgba(255,207,74,0.35)'; ctx.setLineDash([1.5, 1.5]);
      ctx.beginPath(); ctx.moveTo(x0 + 2, yOf(this.value)); ctx.lineTo(x0 + cw - 2, yOf(this.value)); ctx.stroke(); ctx.setLineDash([]);
      const up = hist[hist.length - 1] >= hist[Math.max(0, hist.length - 25)];
      ctx.strokeStyle = up ? '#7ee08a' : '#ff5a72'; ctx.lineWidth = 0.8;
      ctx.beginPath();
      hist.forEach((v, i) => { const x = x0 + 2 + (i / 139) * (cw - 4); i ? ctx.lineTo(x, yOf(v)) : ctx.moveTo(x, yOf(v)); });
      ctx.stroke();
      ctx.textAlign = 'left'; ctx.font = '5px "Pixelify Sans", monospace'; ctx.fillStyle = '#f2e8d5';
      ctx.fillText(`GOAT INDEX  ${this.price.toFixed(1)} ${up ? '▲' : '▼'}`, x0 + 3, y0 + 7);
      ctx.fillStyle = 'rgba(255,207,74,0.8)'; ctx.textAlign = 'right';
      ctx.fillText('fair value', x0 + cw - 3, yOf(this.value) - 1.5);
    }
    // Ticker tape
    ctx.fillStyle = 'rgba(14,15,26,0.85)'; ctx.fillRect(0, H - 8, W, 8);
    ctx.font = '5px "Pixelify Sans", monospace'; ctx.textAlign = 'left'; ctx.fillStyle = '#ffcf4a';
    const text = this.headlines.slice(-5).join('   ·   ');
    const tw = ctx.measureText(text).width;
    if (this.tickerX < -tw) this.tickerX = W;
    ctx.fillText(text, this.tickerX, H - 2.2);
  }

  // On the map, in world units (repeated at the map's wrap copies).
  drawOverlay(ctx) {
    const G = this.G;
    this.particles.drawText(ctx);
    // IPO ring over the exchange
    const ex = G.world.sites.exchange;
    if (this.ipo.state === 'active') {
      const p = this.ipo.progress / IPO_SECONDS;
      ctx.lineWidth = 1.4;
      ctx.strokeStyle = 'rgba(10,12,30,0.7)'; ctx.beginPath(); ctx.arc(ex.x, ex.y - 5, 12, 0, Math.PI * 2); ctx.stroke();
      ctx.strokeStyle = '#ff5a72'; ctx.beginPath(); ctx.arc(ex.x, ex.y - 5, 12, -Math.PI / 2, -Math.PI / 2 + p * Math.PI * 2); ctx.stroke();
      label(ctx, ex.x, ex.y - 21, `IPO · keep stress under ${Math.round((this.ipo.limit ?? STRESSED) * 100)}%`, '#ff8a9c', 6);
    }
    if (this.agents.some((a) => a.state === 'strike' || a.state === 'riot')) label(ctx, G.world.sites.picket.x, G.world.sites.picket.y + 12, 'picket line', '#f2e8d5', 5);
    // The aim reticle, on the map at the world point aimed at (3D stands it up as a sprite).
    const aim = G.brain?.aim;
    if (aim && !ctx.__project3d) { const p = G.world.aimPoint(aim); drawReticle(ctx, p.x, p.y, this.t, '#ffe066'); }
  }

  // ---- camera (#33): hand casts and face mood. Extra inputs on top of the brain, never instead of it. ----

  // aim is 0..1 like the brain's.
  onCast(spell, aim) {
    const G = this.G;
    if (!G || this.done) return;
    const w = G.world, ex = w.sites.exchange, picket = w.sites.picket;
    const { x, y } = w.aimPoint(aim);   // aims are 0..1 of the world (#129)
    switch (spell) {
      case 'fist':
      case 'fireball': {
        // No lightning power in this era, so the fist throws a fireball. It frightens whoever it lands near.
        G.stats.lightning++;
        G.audio.thunder();
        G.shake = Math.max(G.shake || 0, 0.8);
        this.particles.burst(22, () => ({ x, y, vx: rand(-30, 30), vy: rand(-40, 5), g: 50, life: 0.7, maxLife: 0.7, size: 1, color: pick(['#fff4c2', '#ffd166', '#ff9a2e', '#e8452c']) }));
        for (const a of this.agents) if (dist(x, y, a.x, a.y) < 16) a.fear = clamp(a.fear + 0.35);
        if (dist(x, y, ex.x, ex.y) < 12) { this.price *= 0.95; this.headlines.push('FIREBALL HITS THE GOAT EXCHANGE; TRADERS "FINE"'); }
        floater(this.particles, x, y - 8, 'fireball', '#ff9a2e', 7);
        break;
      }
      case 'palm':
      case 'spawn': {
        // Open palm: goats from heaven, straight into GDP.
        this.gdp += 30;
        const at = w.walkable(x, y) ? { x, y } : w.randomLand(ex.x, ex.y + 6, 1, 6);
        for (let i = 0; i < 2 && this.goats.length < 24; i++) this.goats.push({ x: at.x + rand(-2, 2), y: at.y + rand(-1, 1), frame: 0, ft: 0, facing: 1, side: 1, spot: null });
        floater(this.particles, at.x, at.y - 8, '+2 goats · +30 GDP', '#ffcf5c', 6);
        G.audio.coin();
        this.headlines.push('GOATS FALL FROM THE SKY; GDP UP, ROOFS DOWN');
        break;
      }
      case 'rain': {
        // Victory: good rains. Fear eases and the hay harvest adds to GDP.
        for (const a of this.agents) a.fear = clamp(a.fear - 0.2);
        this.gdp += 10;
        this.castRain(x, y);
        floater(this.particles, x, 24, 'good rains · +10 GDP', '#9ec9ff', 6);
        this.headlines.push('GOOD RAINS: HAY FUTURES CALM');
        break;
      }
      case 'bless': {
        this.confidence = clamp(this.confidence + 0.15, 0.5, 1.6);
        for (const a of this.agents) a.unrest = clamp(a.unrest - 0.1);
        this.particles.burst(20, () => ({ x: ex.x + rand(-10, 10), y: ex.y + rand(-8, 4), vx: rand(-3, 3), vy: rand(-14, -4), life: 1.2, maxLife: 1.2, size: 1, color: pick(['#ffe066', '#fff4c2']) }));
        floater(this.particles, ex.x, ex.y - 14, 'confidence up', '#ffe066', 6);
        G.audio.good();
        this.headlines.push('GOD GIVES THE ECONOMY A THUMBS UP');
        break;
      }
      case 'disaster': {
        // Thumb down: a storm at the reticle. Business confidence crashes and nearby workers panic.
        G.audio.thunder();
        G.shake = Math.max(G.shake || 0, 2);
        this.confidence = Math.max(0.5, this.confidence - 0.3);
        this.momentum -= 0.015;
        for (const a of this.agents) if (dist(x, y, a.x, a.y) < 40) a.fear = clamp(a.fear + 0.4);
        this.castRain(x, y);
        this.particles.burst(16, () => ({ x: x + rand(-8, 8), y: y - rand(0, 30), vx: 0, vy: 0, life: 0.25, maxLife: 0.25, size: 1, color: '#ffffff' }));
        floater(this.particles, x, y - 12, 'STORM', '#9ec9ff', 9);
        this.headlines.push('FREAK STORM; BUSINESS CONFIDENCE CRASHES');
        G.advisor.say('cast-storm', 'A storm, bro? Business confidence just fell off a cliff. So did a goat.', { mood: 'panic', priority: 2, cooldown: 20 });
        break;
      }
      case 'peace': {
        // I-love-you: strikes and riots end, everyone goes back to work.
        let calmed = 0;
        for (const a of this.agents) {
          if (a.state === 'strike' || a.state === 'riot') { a.state = 'commute'; a.spot = null; calmed++; }
          a.unrest = Math.min(a.unrest, 0.2);
          a.fear = clamp(a.fear - 0.2);
        }
        this.generalStrike = 0;
        floater(this.particles, picket.x, picket.y - 8, calmed ? `${calmed} back to work` : 'peace', '#9be38a', 6);
        this.headlines.push('GOD BROKERS PEACE AT THE MILL');
        G.advisor.say('cast-peace', 'Peace at the mill. The strikers are back at work and the signs are going in a museum.', { mood: 'happy', priority: 1, cooldown: 25 });
        break;
      }
    }
  }

  // Rain over a screen-sized patch of sky around (cx, cy) (the world is bigger than the view, #129).
  castRain(cx, cy = this.G?.camera?.viewCenter?.().y ?? this.G.world.sites.exchange.y) {
    this.particles.burst(60, () => ({ x: cx + rand(-60, 60), y: cy + rand(-100, 26), vx: -4, vy: rand(70, 100), life: rand(0.5, 1), maxLife: 1, size: 1, color: 'rgba(158,201,255,0.75)' }));
  }

  // Face mood. Joy calms the markets, anger adds volatility and unrest, surprise brings
  // lightning, sadness darkens the sky.
  onMood(mood) {
    const G = this.G;
    if (!G) return;
    this.baseDarkness ??= this.darkness;
    this.darkness = clamp(this.baseDarkness + (mood.dominant === 'sadness' ? 0.3 : 0), 0, 0.9);
    if (this.done) return;
    const first = mood.reason === 'change';
    if (mood.dominant === 'joy') {
      for (const a of this.agents) a.fear = clamp(a.fear - 0.12);
      this.momentum *= 0.5;
      this.confidence = clamp(this.confidence + 0.04, 0.5, 1.6);
      if (first) this.headlines.push('GOD SMILES; GOAT INDEX STEADIES');
    } else if (mood.dominant === 'anger') {
      this.momentum += gauss() * 0.012;
      for (const a of this.agents) { a.unrest = clamp(a.unrest + 0.1); a.fear = clamp(a.fear + 0.06); }
      if (first) this.headlines.push('GOD SCOWLS; TRADERS SWEAT, WORKERS GRUMBLE');
    } else if (mood.dominant === 'surprise') {
      const fx = G.world.sites.factory;
      G.audio.thunder();
      G.shake = Math.max(G.shake || 0, 1);
      this.particles.burst(18, () => ({ x: fx.x + rand(-6, 6), y: fx.y - 14 - rand(0, 40), vx: 0, vy: 0, life: 0.3, maxLife: 0.3, size: 1, color: '#ffffff' }));
      if (first) this.headlines.push('LIGHTNING OVER THE MILL; NOBODY SAW IT COMING');
    } else if (mood.dominant === 'sadness' && first) {
      this.headlines.push('OVERCAST SKIES; ANALYSTS BLAME THE GOD\'S MOOD');
    }
  }
}
