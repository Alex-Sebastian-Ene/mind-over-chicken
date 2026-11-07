// Time-skip cutscenes. The 24-hour scope builds three eras properly and fakes the rest.
//
// FarmingToKingdoms: farms and goat trading appear, then the Kingdom's scholars pose
// three mental-arithmetic problems. Those seconds are labelled `focus`, which is how
// the focus classifier gets trained on this player before the finale needs it.
//
// InfoAge: a short bridge into the Space Age.

import { W, H, Particles, rand, randInt, pick, floater, clamp } from '../sim/util.js';
import { walk, animate, villagerLook } from '../sim/agents.js';
import { drawPerson, drawGoat, drawChicken } from '../render/sprites.js';
import { label } from './stone.js';

function makeProblems() {
  const a = randInt(13, 19), b = randInt(3, 7);
  const c = randInt(6, 12), d = c * randInt(8, 12), e = randInt(5, 19);
  const f = randInt(21, 29), g = randInt(3, 4), h = randInt(6, 15);
  return [
    { q: `${a} × ${b}`, a: a * b, invent: 'The scholars invent writing.' },
    { q: `${d} ÷ ${c} + ${e}`, a: d / c + e, invent: 'The scholars invent law.' },
    { q: `${f} × ${g} − ${h}`, a: f * g - h, invent: 'Too much focus. The scholars invent taxes.' },
  ];
}

export class FarmingToKingdoms {
  constructor() {
    this.id = 'timeskip1';
    this.era = 'farming';
    this.label = 'Time skip · Farming → Kingdoms';
    this.powers = ['focus'];
    this.darkness = 0;
    this.cutscene = true;
  }

  enter(G) {
    this.G = G;
    const w = G.world;
    this.layer = w.paint('farming');
    this.particles = new Particles();
    this.t = 0;
    this.phase = 'farming';
    this.problems = makeProblems();
    this.pi = -1;
    this.pt = 0;
    this.reveal = false;
    const v = w.sites.village;
    this.people = [];
    for (let i = 0; i < 26; i++) {
      const p = w.randomLand(v.x, v.y, 4, 40);
      this.people.push({ ...p, ...villagerLook('stone'), frame: 0, ft: rand(0, 5), facing: 1, side: 1, spot: null, trade: rand(0, 4) });
    }
    this.goats = [];
    for (let i = 0; i < 16; i++) {
      const p = w.randomLand(v.x, v.y, 14, 50);
      this.goats.push({ ...p, frame: 0, ft: 0, facing: 1, side: 1, spot: null });
    }
    this.chicken = { x: v.x + 4, y: v.y + 3, frame: 0, ft: 0, facing: 1, side: 1 };
    G.brain.setCue(null);
    G.advisor.setEra('farming');
    G.advisor.say('farm', 'Eight thousand years later. Calm skies, fat harvests. And for reasons no one can explain, everyone now trades goats.', { mood: 'happy', priority: 3, cooldown: 999 });
  }

  skip() {
    if (this.phase === 'farming') this.startKingdoms();
  }

  startKingdoms() {
    const G = this.G;
    this.phase = 'kingdoms';
    this.pi = 0; this.pt = 0; this.reveal = false;
    G.advisor.setEra('kingdoms');
    G.advisor.say('scholars', 'The Kingdom\'s scholars beg for your wisdom. Work out each sum in your head. They will write down whatever you think.', { priority: 3, cooldown: 999 });
    G.brain.setCue('focus');
    G.audio.bell();
  }

  update(dt) {
    const G = this.G, w = G.world;
    this.t += dt;
    this.particles.update(dt);
    const v = w.sites.village;

    for (const p of this.people) {
      animate(p, dt, 5);
      if (!p.spot || walk(p, p.spot.x, p.spot.y, 7, dt, w)) p.spot = w.randomLand(v.x, v.y, 3, 40);
      p.trade -= dt;
      if (p.trade <= 0 && this.phase === 'farming') {
        p.trade = rand(2, 5);
        floater(this.particles, p.x, p.y - 6, pick(['goat ⇄ grain', '2 goats ⇄ pot', 'goat ⇄ goat?', 'IOU 1 goat']), '#f2e8d5', 5);
      }
    }
    for (const g of this.goats) { animate(g, dt, 3); if (!g.spot || walk(g, g.spot.x, g.spot.y, 4, dt, w)) g.spot = w.randomLand(g.x, g.y, 3, 14); }
    animate(this.chicken, dt, 5);

    if (this.phase === 'farming' && this.t > 5.5) this.startKingdoms();

    if (this.phase === 'kingdoms') {
      this.pt += dt;
      const PROBLEM = 4.4, REVEAL = 1.3;
      if (!this.reveal && this.pt > PROBLEM) {
        this.reveal = true; this.pt = 0;
        G.brain.setCue('rest');
        G.audio.good();
      } else if (this.reveal && this.pt > REVEAL) {
        this.reveal = false; this.pt = 0; this.pi++;
        if (this.pi >= this.problems.length) {
          this.phase = 'done';
          G.brain.setCue(null);
          const trained = G.brain.model.train();
          const n = G.brain.model.count('focus');
          G.advisor.say('wrote', trained
            ? `The scholars have written down how your mind works. ${n} pages of it. Unsettling. Rich and poor now live on different streets.`
            : 'The scholars wrote very little down. Rich and poor now live on different streets.', { mood: 'nervous', priority: 3, cooldown: 999 });
          G.complete(2.5);
        } else {
          G.brain.setCue('focus');
          G.audio.blip();
        }
      }
    }
  }

  objectives() {
    if (this.phase === 'farming') return [{ label: 'Years passing', value: `${Math.floor(clamp(this.t / 5.5) * 8000)}`, plain: true }];
    const done = Math.max(0, this.pi + (this.reveal ? 1 : 0));
    return [{ label: 'Scholars’ sums', value: `${Math.min(3, done)}/3`, done: this.phase === 'done' }];
  }

  lights() { return []; }

  draw(ctx) {
    ctx.drawImage(this.layer, 0, 0);
    this.G.world.drawWater(ctx, this.t);
    const actors = [];
    for (const g of this.goats) actors.push({ y: g.y, d: () => drawGoat(ctx, g.x, g.y, g) });
    for (const p of this.people) actors.push({ y: p.y, d: () => drawPerson(ctx, p.x, p.y, p) });
    actors.push({ y: this.chicken.y, d: () => drawChicken(ctx, this.chicken.x, this.chicken.y, this.chicken) });
    actors.sort((a, b) => a.y - b.y).forEach((a) => a.d());
    this.particles.draw(ctx);
    if (this.phase !== 'farming') { ctx.fillStyle = 'rgba(30,18,40,0.45)'; ctx.fillRect(0, 0, W, H); }
  }

  drawOverlay(ctx) {
    this.particles.drawText(ctx);
    if (this.phase === 'farming') {
      const yrs = Math.floor(clamp(this.t / 5.5) * 8000);
      label(ctx, W / 2, 26, `${yrs.toLocaleString('en-GB')} years later`, '#f2e8d5', 13);
      label(ctx, W / 2, 38, this.t < 2.8 ? 'Farming. Calm skies, fat harvests.' : 'Villagers start trading goats. A market forms.', '#ffd166', 7);
      return;
    }
    if (this.phase === 'kingdoms' && this.pi >= 0 && this.pi < this.problems.length) {
      const pr = this.problems[this.pi];
      // Scroll
      const x0 = W / 2 - 70, y0 = 46, w = 140, h = 74;
      ctx.fillStyle = '#e9dcb8'; ctx.fillRect(x0, y0, w, h);
      ctx.fillStyle = '#c9b88e'; ctx.fillRect(x0 - 4, y0 - 3, w + 8, 4); ctx.fillRect(x0 - 4, y0 + h - 1, w + 8, 4);
      ctx.textAlign = 'center';
      ctx.fillStyle = '#6b4f2a'; ctx.font = '6px "Pixelify Sans", monospace';
      ctx.fillText(`THE SCHOLARS ASK · ${this.pi + 1} of 3`, W / 2, y0 + 12);
      ctx.fillStyle = '#2b1d14'; ctx.font = '20px "Pixelify Sans", monospace';
      ctx.fillText(this.reveal ? `= ${pr.a}` : pr.q, W / 2, y0 + 40);
      ctx.font = '6px "Pixelify Sans", monospace'; ctx.fillStyle = '#6b4f2a';
      if (this.reveal) ctx.fillText(pr.invent, W / 2, y0 + 58);
      else {
        ctx.fillText('Solve it in your head', W / 2, y0 + 56);
        const left = clamp(1 - this.pt / 4.4);
        ctx.fillStyle = '#b5543a'; ctx.fillRect(x0 + 16, y0 + 62, (w - 32) * left, 2);
      }
      if (this.G.source.kind === 'sim' && !this.reveal) label(ctx, W / 2, y0 + h + 14, 'Emulated headset: hold F (or the Inspire tile) while you work it out', '#ffcf4a', 5);
    }
    if (this.phase === 'done') {
      label(ctx, W / 2, 70, 'Writing. Law. Taxes.', '#f2e8d5', 12);
      label(ctx, W / 2, 84, 'Rich and poor split. Unrest simmers.', '#ffd166', 7);
    }
  }
}

export class InfoAge {
  constructor() {
    this.id = 'timeskip2';
    this.era = 'space';
    this.label = 'Time skip · The Information Age';
    this.powers = [];
    this.darkness = 0.55;
    this.cutscene = true;
  }

  enter(G) {
    this.G = G;
    this.layer = G.world.paint('space');
    this.t = 0;
    this.particles = new Particles();
    G.brain.setCue('rest');
    G.advisor.setEra('space');
    G.advisor.say('info', 'Three hundred years later. The goats went digital. Then everyone looked up.', { mood: 'nervous', priority: 3, cooldown: 999 });
  }

  skip() { if (this.t > 1) this.t = 99; }

  update(dt) {
    this.t += dt;
    if (this.t > 6 && !this.left) { this.left = true; this.G.brain.setCue(null); this.G.complete(0.1); }
  }

  objectives() { return [{ label: 'Years passing', value: `${Math.floor(clamp(this.t / 6) * 300)}`, plain: true }]; }

  lights() {
    // City lights twinkle on at night.
    const L = [];
    for (const s of this.G.world.structures) if (s.kind === 'tower' || s.kind === 'house') L.push({ x: s.x, y: s.y - 3, r: 6 });
    return L;
  }

  draw(ctx) { ctx.drawImage(this.layer, 0, 0); this.G.world.drawWater(ctx, this.t); }

  drawOverlay(ctx) {
    label(ctx, W / 2, 26, `${Math.floor(clamp(this.t / 6) * 300)} years later`, '#f2e8d5', 13);
    label(ctx, W / 2, 38, this.t < 3 ? 'The goats went digital.' : 'Everyone looks up.', '#9ec9ff', 7);
  }
}
