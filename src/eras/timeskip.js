// Time-skip cutscenes. The 24-hour scope builds three eras properly and fakes the rest.
//
// FarmingToKingdoms: farms and goat trading appear, then the Kingdom's scholars write down
// three inventions while the player concentrates (#223: no arithmetic, it read as a calculator).
// Those seconds are labelled `focus`, which is how the focus classifier gets trained on this
// player before the finale needs it; the scroll's ink runs faster the harder they focus.
//
// InfoAge: a short bridge into the Space Age.

import { W, H, Particles, rand, pick, floater, clamp } from '../sim/util.js';
import { walk, animate, villagerLook } from '../sim/agents.js';
import { drawPerson, drawGoat, drawChicken } from '../render/sprites.js';
import { FireballFX } from '../render/fireball.js';
import { label } from './stone.js';
import { joyBuff, surpriseFires, surpriseEffect, sparkleCount, JOY_COLORS } from '../camera/mood-actions.js'; // #152
import { reducedMotion } from '../ui/motion.js';

// What the scholars write down, one focus window each.
export const INVENTIONS = [
  { name: 'Writing', invent: 'The scholars invent writing.' },
  { name: 'Law', invent: 'The scholars invent law.' },
  { name: 'Taxes', invent: 'Too much focus. The scholars invent taxes.' },
];
// How much of an invention's name is inked: time alone writes half of it over the window, and
// focus writes faster (the full name is revealed when the window ends either way).
export function inkedLetters(name, t, ink, window) {
  const share = Math.max(Math.min(1, t / window) * 0.5, Math.min(1, ink));
  return name.slice(0, Math.ceil(name.length * share));
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
    this.fireballs = new FireballFX();
    this.t = 0;
    this.phase = 'farming';
    this.problems = INVENTIONS;
    this.ink = 0;
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
    this.pi = 0; this.pt = 0; this.reveal = false; this.ink = 0;
    G.advisor.setEra('kingdoms');
    G.advisor.say('scholars', 'The Kingdom\'s scholars will write down whatever you think, my lord. Concentrate, and the ink runs faster.', { priority: 3, cooldown: 999 });
    G.brain.setCue('focus');
    G.audio.bell();
  }

  update(dt) {
    const G = this.G, w = G.world;
    this.t += dt;
    for (const hit of this.fireballs.update(dt)) this.impactFireball(hit);
    this.particles.update(dt);
    montageJoy(this, dt);   // #152
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
      // Focus fills the scroll (#223): half the name by time alone, the rest by focusing.
      if (!this.reveal) this.ink += (dt / PROBLEM) * (0.4 + 1.6 * clamp(G.brain.focus || 0));
      if (!this.reveal && this.pt > PROBLEM) {
        this.reveal = true; this.pt = 0;
        G.brain.setCue('rest');
        G.audio.good();
      } else if (this.reveal && this.pt > REVEAL) {
        this.reveal = false; this.pt = 0; this.pi++; this.ink = 0;
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
    return [{ label: 'Scholars’ scrolls', value: `${Math.min(3, done)}/3`, done: this.phase === 'done' }];
  }

  lights() { return []; }

  draw3d(engine, _dt) {
    for (const g of this.goats) {
      engine.add_billboard(2, g.x, g.y, g.facing || 1, 0, 0);
    }
    for (const p of this.people) {
      engine.add_billboard(0, p.x, p.y, p.facing || 1, p.frame || 0, 0);
    }
    engine.add_billboard(3, this.chicken.x, this.chicken.y, this.chicken.facing || 1, this.chicken.frame || 0, 0);
  }

  draw(ctx) {
    const G = this.G;
    const base = G.render3d ? (G.world?.layers3d?.farming || this.layer) : this.layer;
    ctx.drawImage(base, 0, 0);
    if (!G.render3d) this.G.world.drawWater(ctx, this.t);
    if (G.render3d) {
      this.particles.draw(ctx);
      if (this.phase !== 'farming') { ctx.fillStyle = 'rgba(30,18,40,0.45)'; ctx.fillRect(0, 0, W, H); }
      return;
    }
    const actors = [];
    for (const g of this.goats) actors.push({ y: g.y, d: () => drawGoat(ctx, g.x, g.y, g) });
    for (const p of this.people) actors.push({ y: p.y, d: () => drawPerson(ctx, p.x, p.y, p) });
    actors.push({ y: this.chicken.y, d: () => drawChicken(ctx, this.chicken.x, this.chicken.y, this.chicken) });
    actors.sort((a, b) => a.y - b.y).forEach((a) => a.d());
    this.particles.draw(ctx);
    if (this.phase !== 'farming') { ctx.fillStyle = 'rgba(30,18,40,0.45)'; ctx.fillRect(0, 0, this.G.world.width, this.G.world.height); }
  }

  // On the map, in world units (repeated at the map's wrap copies).
  drawOverlay(ctx) {
    this.fireballs.draw(ctx);
    this.particles.drawText(ctx);
  }

  // The montage's captions and the scholars' scroll, fixed to the screen (#129).
  drawScreenOverlay(ctx) {
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
      ctx.fillText(`THE SCHOLARS LISTEN · ${this.pi + 1} of 3`, W / 2, y0 + 12);
      ctx.fillStyle = '#2b1d14'; ctx.font = '20px "Pixelify Sans", monospace';
      ctx.fillText(this.reveal ? pr.name : inkedLetters(pr.name, this.pt, this.ink, 4.4), W / 2, y0 + 40);
      ctx.font = '6px "Pixelify Sans", monospace'; ctx.fillStyle = '#6b4f2a';
      if (this.reveal) ctx.fillText(pr.invent, W / 2, y0 + 58);
      else {
        ctx.fillText('Concentrate: the ink runs faster', W / 2, y0 + 56);
        const left = clamp(1 - this.pt / 4.4);
        ctx.fillStyle = '#b5543a'; ctx.fillRect(x0 + 16, y0 + 62, (w - 32) * left, 2);
      }
      if (this.G.source.kind === 'sim' && !this.reveal) label(ctx, W / 2, y0 + h + 14, 'Emulated headset: hold F to concentrate', '#ffcf4a', 5);
    }
    if (this.phase === 'done') {
      label(ctx, W / 2, 70, 'Writing. Law. Taxes.', '#f2e8d5', 12);
      label(ctx, W / 2, 84, 'Rich and poor split. Unrest simmers.', '#ffd166', 7);
    }
  }

  // ---- camera (#33): the montage has no gameplay to change, so casts and moods show a label. ----
  onFireball(aim, { color = '#ff9a2e', charge = 1 } = {}) {
    if (!this.G) return;
    const p = this.G.world.aimPoint(aim);
    this.fireballs.launch({ x: p.x, y: p.y }, { color, charge });
    this.G.audio?.blip?.();
  }

  impactFireball({ x, y, color = '#ff9a2e', charge = 1 }) {
    this.particles.burst(18 + Math.round(charge * 20), () => ({ x, y, vx: rand(-38, 38), vy: rand(-46, 4), g: 60, life: rand(0.45, 0.85), maxLife: 0.85, size: 1, color }));
  }

  onCast(spell, aim) {
    const text = MONTAGE_CASTS[spell];
    if (!text || !this.G) return;
    const p = this.G.world.aimPoint(aim);   // aims are 0..1 of the world (#129)
    floater(this.particles, p.x, p.y - 6, text, '#ffd166', 6);
  }

  onMood(mood) { montageMood(this, mood); }
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
    this.fireballs = new FireballFX();
    G.brain.setCue('rest');
    G.advisor.setEra('space');
    G.advisor.say('info', 'Three hundred years later. The goats went digital. Then everyone looked up.', { mood: 'nervous', priority: 3, cooldown: 999 });
  }

  skip() { if (this.t > 1) this.t = 99; }

  update(dt) {
    this.t += dt;
    for (const hit of this.fireballs.update(dt)) this.impactFireball(hit);
    this.particles.update(dt);
    montageJoy(this, dt);   // #152
    if (this.t > 6 && !this.left) { this.left = true; this.G.brain.setCue(null); this.G.complete(0.1); }
  }

  objectives() { return [{ label: 'Years passing', value: `${Math.floor(clamp(this.t / 6) * 300)}`, plain: true }]; }

  lights() {
    // City lights twinkle on at night.
    const L = [];
    for (const s of this.G.world.structures) if (s.kind === 'tower' || s.kind === 'house') L.push({ x: s.x, y: s.y - 3, r: 6 });
    return L;
  }

  draw(ctx) { ctx.drawImage(this.layer, 0, 0); this.G.world.drawWater(ctx, this.t); this.particles.draw(ctx); }

  // Captions fixed to the screen (#129).
  drawScreenOverlay(ctx) {
    label(ctx, W / 2, 26, `${Math.floor(clamp(this.t / 6) * 300)} years later`, '#f2e8d5', 13);
    label(ctx, W / 2, 38, this.t < 3 ? 'The goats went digital.' : 'Everyone looks up.', '#9ec9ff', 7);
  }

  drawOverlay(ctx) {
    this.fireballs.draw(ctx);
    this.particles.drawText(ctx);
  }

  // ---- camera (#33): this short bridge draws no gameplay, so the fireball just slams into the map. ----
  onFireball(aim, { color = '#ff9a2e', charge = 1 } = {}) {
    if (!this.G) return;
    const p = this.G.world.aimPoint(aim);
    this.fireballs.launch({ x: p.x, y: p.y }, { color, charge });
    this.G.audio?.blip?.();
  }

  impactFireball({ x, y, color = '#ff9a2e', charge = 1 }) {
    this.particles.burst(18 + Math.round(charge * 20), () => ({ x, y, vx: rand(-38, 38), vy: rand(-46, 4), g: 60, life: rand(0.45, 0.85), maxLife: 0.85, size: 1, color }));
  }

  onCast(spell) {
    const text = MONTAGE_CASTS[spell];
    if (text) this.G?.advisor.say(`cast-info-${spell}`, `Even in the montage: ${text}.`, { priority: 2, cooldown: 2 });
  }

  onMood(mood) { montageMood(this, mood); }
}

// #152: the montages are short, so joy and surprise only decorate them. Joy: sparkles drift over
// the middle of the screen while you smile. Surprise (once per event): a capped flash, a short
// shake (none under reduced motion), thunder and a caption. Any change of mood gets a caption.
function montageJoy(scene, dt) {
  scene.flash = Math.max(0, (scene.flash || 0) - dt);
  const G = scene.G, joy = joyBuff(G?.mood);
  if (!joy) return;
  const c = G.camera?.viewCenter?.() || G.world?.sites?.village || { x: W / 2, y: H / 2 };
  for (let n = sparkleCount(joy, dt, 10); n > 0; n--) {
    scene.particles.add({ x: c.x + rand(-60, 60), y: c.y - rand(0, 40), vx: rand(-3, 3), vy: rand(-14, -8), life: 1, maxLife: 1, size: 1, color: pick(JOY_COLORS) });
  }
}

function montageMood(scene, mood) {
  const G = scene.G;
  if (!G) return;
  const c = G.camera?.viewCenter?.() || G.world?.sites?.village || { x: W / 2, y: H / 2 };   // mid-screen
  if (surpriseFires(mood)) {
    const fx = surpriseEffect(reducedMotion());
    scene.flash = Math.max(scene.flash || 0, fx.flash);
    G.shake = Math.max(G.shake || 0, fx.shake);
    G.audio?.thunder?.();
    floater(scene.particles, c.x, c.y - 52, 'SURPRISE!', '#ffe066', 10);
  }
  const text = MONTAGE_MOODS[mood.dominant];
  if (text && mood.reason === 'change') floater(scene.particles, c.x, c.y - 38, text, '#9ec9ff', 6);
}

// camera (#33): what a cast or a mood looks like from ten thousand years away.
const MONTAGE_CASTS = {
  point: 'the god points at something',
  fireball: 'a fireball, history looks away',
  spawn: 'a bumper harvest',
  rain: 'spring rain on the fields',
  bless: 'a golden age',
  disaster: 'a great flood',
  peace: 'a long peace',
};
const MONTAGE_MOODS = {
  joy: 'the god smiles on the harvest',
  anger: 'the god frowns; the scholars sweat',
  surprise: 'the sky flashes',
  sadness: 'a grey century',
};
