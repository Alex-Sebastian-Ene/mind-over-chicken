// civ (#50): "Sandbox: four civilisations". Four peoples start on a big wrapping planet with
// one village and four agents each; they grow, claim land, trade, raid, go to war and rebel.
// The simulation is Rust (crates/moc-civ, loaded by ./civ.js); this scene feeds it the brain,
// draws its view, and turns its event log into news and advisor lines.
//
// The player is a god, not a general: calm cools every chief, a racing pulse and an angry
// face heat them up, focus fattens harvests, blinks throw lightning, and the camera casts
// (#33: onCast/onMood) act on the world. The player's own civilisation (★) is seeded by the
// calibration window (#54, G.calibration): from the mind read with ?calib_mode=sensor, at
// random (reproducible from ?seed=) in the default dummy run. The run seed also picks the
// planet and the other three civilisations.

import { W, H, clamp } from '../sim/util.js';
import { makeBolt, drawBolt, drawReticle } from '../render/sprites.js';
import { createCiv, EVENT, CAST } from './civ.js';

const MIND_SECONDS = 10;
const DAY = 80;            // seconds per day–night cycle (DAY_SECONDS in the Rust sim)
const PAN_SPEED = 140;     // planet pixels per second
const NEWS_HOLD = 4;       // seconds each headline stays on the ticker
const PAN_KEYS = {
  KeyA: [-1, 0], ArrowLeft: [-1, 0], KeyD: [1, 0], ArrowRight: [1, 0],
  KeyW: [0, -1], ArrowUp: [0, -1], KeyS: [0, 1], ArrowDown: [0, 1],
};

// Camera keys. Captured before main.js sees them while this scene is playing, so A/D pan our
// planet rather than the surface strip and the arrows pan instead of nudging the aim.
let active = null;
const held = new Set();
const playing = () => active && active.G?.scene === active;
window.addEventListener('keydown', (e) => {
  if (!PAN_KEYS[e.code] || e.ctrlKey || e.metaKey || e.altKey || !playing()) return;
  if (e.target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
  held.add(e.code);
  e.preventDefault();
  e.stopImmediatePropagation();
}, true);
window.addEventListener('keyup', (e) => {
  if (held.delete(e.code) && playing()) e.stopImmediatePropagation();
}, true);
window.addEventListener('blur', () => held.clear());

// Shadowed pixel-font text in low-res units (like the eras' label(), with alignment).
function label(ctx, x, y, text, color, size = 6, align = 'center') {
  ctx.font = `${size}px "Pixelify Sans", monospace`;
  ctx.textAlign = align;
  ctx.fillStyle = 'rgba(8,10,24,0.85)';
  ctx.fillText(text, x + 0.6, y + 0.6);
  ctx.fillStyle = color;
  ctx.fillText(text, x, y);
}

const smoothstep = (a, b, t) => { const x = clamp((t - a) / (b - a)); return x * x * (3 - 2 * x); };
const pct = (v) => `${Math.round(v * 100)}%`;

export class CivSandbox {
  constructor({ cosmos = null, seed = null } = {}) {
    this.id = 'civ';
    this.era = 'kingdoms';
    this.label = 'Sandbox · Four civilisations';
    this.intro = {
      title: 'Four civilisations',
      goal: 'Four peoples share one big planet. Watch them grow, fight for land and split. Your mind tilts war and peace.',
      how: [
        ['Calm', 'Hold Space: friendlier chiefs, trade, peace treaties, fewer rebels.'],
        ['Pulse', 'Hold Shift: a racing pulse (and an angry face) brings raids, wars and rebellions.'],
        ['Focus', 'Hold F: richer harvests, faster growth.'],
        ['Blink', 'Click or B: lightning where you look. It scares rebels back into line.'],
        ['Look around', 'A/D/W/S or the arrow keys pan the planet; right-drag works too.'],
      ],
    };
    this.powers = ['blink', 'calm', 'focus', 'heart'];
    this.darkness = 0.1;
    this.flash = 0;
    this.cosmos = cosmos;
    this.seedArg = seed;
    this.seed = 0;
    this.civ = null;
    this.error = null;
    this.t = 0;
    this.bolts = [];
    this.news = [];          // {text, t, kind}
    this.shown = null;       // {text, until}
    this.factions = [];
    this.byId = new Map();
    this.villages = [];
    this.stats = null;
    this.mini = null;
    this.miniTimer = 0;
    this.infoTimer = 0;
    this.mind = { t: 0, calm: 0, focus: 0, hr: 0, n: 0, blinks0: null, done: false, shownUntil: 0, text: '' };
    this.moodHold = { calm: 0, pulse: 0, anger: 0 };
    this.mood = null;
    this.perf = { stepMs: 0, renderMs: 0, worstStepMs: 0, samples: 0, sumStep: 0, sumRender: 0 };
  }

  // ---------- lifecycle ----------
  enter(G) {
    this.G = G;
    active = this;
    held.clear();
    G.brain.setCue(null);
    G.advisor.say('civ-intro', 'Four peoples, one planet, my lord. They will grow, squabble and split. Your moods are now everyone\'s problem.', { priority: 3, cooldown: 999 });
    // calibration (#54): one run seed for the planet, the civilisations and the dummy seed.
    this.seed = (this.seedArg ?? G.calibration?.config?.seed ?? Math.random() * 2 ** 32) >>> 0;
    createCiv(this.seed)
      .then((civ) => {
        if (G.scene !== this) { civ.free(); return; }
        this.civ = civ;
        this.refresh();
        this.mini = civ.minimap();
        this.miniCanvas = Object.assign(document.createElement('canvas'), { width: civ.miniW, height: civ.miniH });
        this.miniCanvas.getContext('2d').putImageData(this.mini, 0, 0);
        console.info(`[civ] planet ${civ.pw}×${civ.ph}, seed ${this.seed}`);
        G.calibration?.onDone((result) => { if (G.scene === this) this.applyCalibration(result); });
      })
      .catch((err) => {
        this.error = err;
        console.error('[civ] could not load the simulation:', err);
        G.advisor.say('civ-error', 'The planet would not form, my lord. (The simulation failed to load.)', { mood: 'panic', priority: 3, cooldown: 999 });
      });
  }

  refresh() {
    const civ = this.civ;
    this.factions = civ.factions();
    this.byId = new Map(this.factions.map((f) => [f.id, f]));
    this.villages = civ.villages();
    this.stats = civ.stats();
  }

  name(i) { return this.byId.get(i)?.name ?? 'someone'; }
  player() { return this.factions.find((f) => f.isPlayer && f.alive) || null; }

  // ---------- simulation ----------
  update(dt) {
    const civ = this.civ;
    if (!civ) return;
    const G = this.G, b = G.brain;
    this.t += dt;

    // Camera: keys.
    let px = 0, py = 0;
    for (const k of held) { px += PAN_KEYS[k][0]; py += PAN_KEYS[k][1]; }
    if (px || py) civ.pan(px * PAN_SPEED * dt, py * PAN_SPEED * dt);

    // The brain (and the face, if the camera is on) tilts every chief this step.
    const calm = b.calm || 0, focus = b.focus || 0, hr = b.hrDelta() || 0;
    const anger = G.mood && G.mood.active !== false ? (G.mood.anger || 0) : (this.mood?.anger || 0);
    civ.setInputs(calm, focus, hr, anger);
    if (!this.G.calibration) this.readMind(dt, calm, focus, hr);

    const t0 = performance.now();
    civ.step(Math.min(dt, 0.05));
    const ms = performance.now() - t0;
    const p = this.perf;
    p.stepMs = ms; p.sumStep += ms; p.samples++; p.worstStepMs = Math.max(p.worstStepMs, ms);

    const events = civ.events();
    if (events.length) this.refresh();
    for (const e of events) this.onEvent(e);
    this.infoTimer -= dt;
    if (this.infoTimer <= 0) { this.infoTimer = 0.25; this.refresh(); }
    this.miniTimer -= dt;
    if (this.miniTimer <= 0 && this.miniCanvas) {
      this.miniTimer = 0.5;
      this.miniCanvas.getContext('2d').putImageData(civ.minimap(), 0, 0);
    }

    // Day and night.
    const phase = (civ.time / DAY) % 1;
    const night = 0.5 - 0.5 * Math.cos(phase * Math.PI * 2);
    this.darkness = 0.05 + 0.42 * smoothstep(0.55, 0.95, night);
    this.flash = Math.max(0, this.flash - dt * 2);
    for (const bolt of this.bolts) bolt.life -= dt;
    this.bolts = this.bolts.filter((bolt) => bolt.life > 0);
    if (this.shown && this.t > this.shown.until) this.shown = null;
    if (!this.shown) {
      let next = null;   // the oldest headline not yet shown (news is newest first)
      for (const n of this.news) if (!n.seen) next = n;
      if (next) { next.seen = true; this.shown = { text: next.text, kind: next.kind, until: this.t + NEWS_HOLD }; }
    }
    this.reactToMood(dt, calm, hr, anger);
  }

  // calibration (#54): the window's result seeds the player's civilisation. In sensor mode the
  // mind read sets all five traits (calm → friendliness, pulse → aggressiveness, blinks →
  // violence and less cohesion, focus → fertility), with #54's formula for the README three.
  // The dummy run leaves the player's civilisation as the run seed made it.
  applyCalibration(r) {
    const m = this.mind;
    if (m.done || !this.civ) return;
    m.done = true;
    const f = r.features || {};
    const sensor = r.source === 'sensor';
    if (sensor) {
      this.civ.seedMind(f.calm ?? 0, f.focus ?? 0, f.hrDelta ?? 0, ((f.blinkRate ?? 0) * r.seconds) / 10);
      this.civ.setPlayerTraits(r.friendliness, r.aggressiveness, r.violence);
    }
    this.refresh();
    const me = this.player();
    if (!me) return;
    const words = describe(me);
    m.text = sensor
      ? `Read from your first ${r.seconds} s: calm ${pct(f.calm ?? 0)}, focus ${pct(f.focus ?? 0)}, pulse ${(f.hrDelta ?? 0) >= 0 ? '+' : ''}${Math.round(f.hrDelta ?? 0)} → ${me.name}: ${words.join(', ')}.`
      : `Dummy run (seed ${r.seed}): the ${me.name} were born at random: ${words.join(', ')}. Add ?calib_mode=sensor to seed them from your mind.`;
    m.shownUntil = this.t + 9;
    this.G.advisor.say('civ-mind', sensor
      ? `I have read your mind, my lord. The ${me.name} are made in your image: ${words.join(', ')}.`
      : `The ${me.name} are yours, my lord: ${words.join(', ')}. The stars chose them, not your mind. Not yet.`, { mood: 'happy', priority: 3, cooldown: 999 });
  }

  // Without the calibration module: the first ten seconds of play shape the player's civilisation.
  readMind(dt, calm, focus, hr) {
    const m = this.mind;
    if (m.done) return;
    if (m.blinks0 == null) m.blinks0 = this.G.brain.stats.blinks;
    m.t += dt; m.n++;
    m.calm += calm; m.focus += focus; m.hr += hr;
    if (m.t < MIND_SECONDS) return;
    m.done = true;
    const c = m.calm / m.n, f = m.focus / m.n, h = m.hr / m.n, blinks = this.G.brain.stats.blinks - m.blinks0;
    this.civ.seedMind(c, f, h, blinks);
    this.refresh();
    const me = this.player();
    if (!me) return;
    const words = describe(me);
    m.text = `Read from your first ${MIND_SECONDS} s: calm ${pct(c)}, focus ${pct(f)}, pulse ${h >= 0 ? '+' : ''}${Math.round(h)}, ${blinks} blinks → ${me.name}: ${words.join(', ')}.`;
    m.shownUntil = this.t + 9;
    this.G.advisor.say('civ-mind', `I have read your mind, my lord. The ${me.name} are made in your image: ${words.join(', ')}.`, { mood: 'happy', priority: 3, cooldown: 999 });
  }

  reactToMood(dt, calm, hr, anger) {
    const h = this.moodHold, say = this.G.advisor.say.bind(this.G.advisor);
    h.calm = calm > 0.65 ? h.calm + dt : 0;
    h.pulse = hr > 12 ? h.pulse + dt : 0;
    h.anger = anger > 0.4 ? h.anger + dt : 0;
    const wars = this.factions.reduce((s, f) => s + (f.alive ? f.wars : 0), 0) / 2;
    if (h.calm > 4) say('civ-calm', wars ? 'Your calm spreads, my lord. The chiefs are tired of war and want soup.' : 'Your calm spreads, my lord. Chiefs unclench their fists and swap grain.', { mood: 'happy', priority: 1, cooldown: 25 });
    if (h.pulse > 3) say('civ-pulse', 'Your pulse is racing, my lord, and so are the war drums.', { mood: 'panic', priority: 2, cooldown: 25 });
    if (h.anger > 3) say('civ-anger', 'That scowl, my lord. Every village is grumbling now. Rebels are sharpening sticks.', { mood: 'panic', priority: 2, cooldown: 25 });
  }

  // ---------- news ----------
  onEvent(e) {
    const A = this.name(e.a), B = this.name(e.b), say = this.G.advisor.say.bind(this.G.advisor);
    const hot = (this.G.brain.hrDelta() || 0) > 10;
    let text = null;
    switch (e.kind) {
      case EVENT.CONTACT:
        text = `First contact: the ${A} meet the ${B}`;
        say('civ-contact', `The ${A} have met the ${B}. They are measuring each other's spears.`, { priority: 1, cooldown: 20 });
        break;
      case EVENT.WAR:
        text = `WAR: the ${A} declare war on the ${B}`;
        say('civ-war', `The ${A} have declared war on the ${B}!${hot ? ' Your racing pulse is not helping, my lord.' : ''}`, { mood: 'panic', priority: 2, cooldown: 10 });
        this.G.audio?.bad?.();
        break;
      case EVENT.PEACE:
        text = `Peace between the ${A} and the ${B}`;
        say('civ-peace', `The ${A} and the ${B} made peace. Someone brought soup.`, { mood: 'happy', priority: 1, cooldown: 15 });
        break;
      case EVENT.CAPTURE:
        text = `The ${A} capture a village from the ${B}`;
        say('civ-capture', `The ${A} took a village from the ${B}. The villagers have new opinions now.`, { priority: 1, cooldown: 20 });
        break;
      case EVENT.REBELLION:
        text = `REBELLION: the ${B} break away from the ${A} (${e.n} village${e.n === 1 ? '' : 's'})`;
        say('civ-rebel', `Rebellion! The ${B} have split from the ${A}. Family dinners will be awkward.`, { mood: 'panic', priority: 3, cooldown: 8 });
        this.G.audio?.bad?.();
        break;
      case EVENT.WIPED:
        text = `The ${A} are no more${e.b >= 0 ? `, swallowed by the ${B}` : ''}`;
        say('civ-wiped', `The ${A} are gone, my lord. History will be written by the ${e.b >= 0 ? B : 'survivors'}.`, { mood: 'panic', priority: 3, cooldown: 10 });
        break;
      case EVENT.RAID:
        text = `The ${A} raid the ${B} for ${e.n} food`;
        say('civ-raid', `The ${A} raided the ${B} and ran off with the grain.`, { priority: 1, cooldown: 20 });
        break;
      case EVENT.FAMINE:
        text = `Famine among the ${A}`;
        say('civ-famine', `The ${A} are starving, my lord. A bounty (open palm) would help.`, { mood: 'panic', priority: 2, cooldown: 30 });
        break;
      case EVENT.TRADE:
        text = `The ${A} send grain to the ${B}`;
        if ((this.G.brain.calm || 0) > 0.55) say('civ-trade', `Your calm is catching, my lord: the ${A} are sharing grain with the ${B}.`, { mood: 'happy', priority: 1, cooldown: 30 });
        break;
      case EVENT.CAPITAL: text = `The ${A} have a new capital`; break;
      case EVENT.RAZED: text = e.b >= 0 ? `A village of the ${A} is razed` : null; break;
      default: break;
    }
    if (text) {
      this.news.unshift({ text, t: this.t, kind: e.kind, seen: false });
      // Keep a short backlog; skip stale headlines rather than queue them for minutes.
      for (const n of this.news.slice(4)) n.seen = true;
      if (this.news.length > 40) this.news.length = 40;
    }
  }

  // ---------- the god's hand ----------
  aimPlanet(aim) {
    return this.civ.toPlanet(aim.x * (W - 1), aim.y * (H - 1));
  }

  onBlink(aim) {
    const civ = this.civ, G = this.G;
    if (!civ) return;
    const p = this.aimPlanet(aim);
    const killed = civ.lightning(p.x, p.y);
    const s = civ.toScreen(p.x, p.y);
    this.bolts.push({ x: p.x, y: p.y, pts: makeBolt(s.x, s.y), sx: s.x, sy: s.y, life: 0.35 });
    this.flash = 0.3;
    G.stats.lightning++;
    G.stats.smitten += killed;
    G.audio?.thunder?.();
    G.shake = Math.max(G.shake || 0, 0.6);
    if (killed > 0) G.advisor.say('civ-smite', `${killed === 1 ? 'Someone' : `${killed} people`} just met their god, my lord. The neighbours have gone very quiet.`, { priority: 1, cooldown: 12 });
  }

  // camera (#49): hand casts. aim is 0..1 like the brain's.
  onCast(spell, aim) {
    const civ = this.civ, G = this.G;
    const kind = CAST[spell];
    if (!civ || !kind) return;
    const p = this.aimPlanet(aim);
    const n = civ.cast(kind, p.x, p.y);
    const say = G.advisor.say.bind(G.advisor);
    if (spell === 'fireball') { this.flash = 0.4; G.audio?.thunder?.(); G.shake = Math.max(G.shake || 0, 1.2); }
    if (spell === 'disaster') { this.flash = 0.5; G.audio?.thunder?.(); G.shake = Math.max(G.shake || 0, 2); say('civ-cast-storm', 'A storm! That will settle a few arguments. And a few villages.', { mood: 'panic', priority: 2, cooldown: 15 }); }
    if (spell === 'spawn') { G.audio?.coin?.(); say('civ-cast-food', 'Food from the sky. Nobody will ask questions.', { mood: 'happy', priority: 1, cooldown: 15 }); }
    if (spell === 'peace') { G.audio?.good?.(); say('civ-cast-peace', n ? `Peace! ${n} war${n === 1 ? '' : 's'} ended near your hand.` : 'Peace descends. Nobody was fighting there, but they appreciate it.', { mood: 'happy', priority: 2, cooldown: 15 }); }
    if (spell === 'bless') { G.audio?.good?.(); say('civ-cast-bless', 'Blessed! The grumbling stops. For now.', { mood: 'happy', priority: 1, cooldown: 15 }); }
    if (spell === 'rain') say('civ-cast-rain', 'Rain! The fires go out and the fields drink.', { mood: 'happy', priority: 1, cooldown: 15 });
  }

  // camera (#49): face mood. G.mood carries it every frame; this keeps a copy and reacts.
  onMood(mood) {
    this.mood = mood;
  }

  // A drag from main.js (world units, both axes): the content follows the pointer, so the camera
  // moves the other way. This planet is far bigger than the view, so north–south matters here.
  panBy(dx, dy) { this.civ?.pan(-dx, -dy); }

  // ---------- drawing ----------
  draw(ctx) {
    // The surface strip pans east–west in main.js (drag, wheel); this planet has its own
    // camera, so take that pan over and keep the strip still.
    const cos = this.cosmos;
    if (cos && cos.panX) {
      const d = cos.panX > W / 2 ? cos.panX - W : cos.panX;
      this.civ?.pan(-d, 0);
      cos.panX = 0;
    }
    const civ = this.civ;
    if (!civ) {
      ctx.fillStyle = '#0b1020';
      ctx.fillRect(0, 0, W, H);
      return;
    }
    const t0 = performance.now();
    ctx.putImageData(civ.render(), 0, 0);
    const ms = performance.now() - t0;
    this.perf.renderMs = ms;
    this.perf.sumRender += ms;
    for (const bolt of this.bolts) {
      const s = civ.toScreen(bolt.x, bolt.y);
      ctx.save();
      ctx.translate(Math.round(s.x - bolt.sx), Math.round(s.y - bolt.sy));
      drawBolt(ctx, bolt.pts, Math.min(1, bolt.life * 4));
      ctx.restore();
    }
    const a = this.G.brain.aim;
    drawReticle(ctx, a.x * (W - 1), a.y * (H - 1), this.t, '#ffe066');
    if (this.flash > 0) { ctx.fillStyle = `rgba(220,230,255,${this.flash * 0.8})`; ctx.fillRect(0, 0, W, H); }
  }

  drawOverlay(ctx) {
    const civ = this.civ;
    ctx.textAlign = 'center';
    if (!civ) {
      label(ctx, W / 2, H / 2, this.error ? 'The planet failed to form.' : 'Forming a planet…', this.error ? '#ff5a72' : '#ffe066', 8);
      return;
    }
    // Names at capitals.
    for (const f of this.factions) {
      if (!f.alive || !f.capital) continue;
      const s = civ.toScreen(f.capital.x + 0.5, f.capital.y);
      if (s.x < -30 || s.x > W + 30 || s.y < 4 || s.y > H + 4) continue;
      const tag = f.isPlayer ? `★ ${f.name}` : f.name;
      label(ctx, s.x, s.y - 9, f.wars ? `${tag} ⚔` : tag, f.css, 5);
    }
    // Overview map with the camera's window.
    if (this.miniCanvas) {
      const mw = civ.miniW, mh = civ.miniH, x0 = W - mw - 3, y0 = 3;
      ctx.fillStyle = 'rgba(8,10,24,0.75)';
      ctx.fillRect(x0 - 1, y0 - 1, mw + 2, mh + 2);
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(this.miniCanvas, x0, y0);
      const kx = mw / civ.pw, ky = mh / civ.ph;
      const cx = civ.camX * kx, cy = civ.camY * ky, cw = W * kx, ch = H * ky;
      ctx.strokeStyle = '#ffe066';
      ctx.lineWidth = 0.5;
      ctx.strokeRect(x0 + cx, y0 + cy, Math.min(cw, mw - cx), ch);
      if (cx + cw > mw) ctx.strokeRect(x0, y0 + cy, cx + cw - mw, ch);
    }
    // Mind read.
    const m = this.mind;
    const cal = this.G.calibration;
    const left = cal ? cal.remaining : MIND_SECONDS - m.t;
    if (!m.done) label(ctx, 4, 9, `${cal?.config?.mode === 'random' ? 'Seeding your civilisation' : 'Reading your mind'}… ${Math.max(0, Math.ceil(left))} s`, '#59d8c8', 5, 'left');
    else if (this.t < m.shownUntil) wrapLabel(ctx, 4, 9, m.text, '#59d8c8', 5, W - 80);
    // News ticker.
    ctx.fillStyle = 'rgba(8,10,24,0.72)';
    ctx.fillRect(0, H - 9, W, 9);
    const n = this.shown;
    const color = !n ? '#a3a6c4' : [EVENT.WAR, EVENT.REBELLION, EVENT.WIPED, EVENT.FAMINE].includes(n.kind) ? '#ff8a7a' : n.kind === EVENT.PEACE || n.kind === EVENT.TRADE ? '#9be38a' : '#ffe066';
    const s = this.stats;
    ctx.font = '5px "Pixelify Sans", monospace';
    ctx.fillStyle = color;
    ctx.textAlign = 'left';
    ctx.fillText(n ? n.text : 'All quiet. For now.', 3, H - 2.6);
    if (s) {
      ctx.textAlign = 'right';
      ctx.fillStyle = '#7d80a0';
      ctx.fillText(`${Math.round(s.agents)} people · wars ${s.wars} · rebellions ${s.rebellions} · sim ${this.perf.stepMs.toFixed(1)} ms`, W - 3, H - 2.6);
    }
    ctx.textAlign = 'left';
  }

  // Villages glow at night (the engine takes 16 lights).
  lights() {
    const civ = this.civ;
    if (!civ || this.darkness < 0.15) return [];
    const L = [];
    for (const v of this.villages) {
      const s = civ.toScreen(v.x + 0.5, v.y);
      if (s.x < -10 || s.x > W + 10 || s.y < -10 || s.y > H + 10) continue;
      L.push({ x: s.x, y: s.y - 1, r: 9 + Math.min(14, v.pop) + Math.sin(this.t * 9 + v.id) * 0.8, pop: v.pop });
    }
    L.sort((p, q) => q.pop - p.pop);
    for (const bolt of this.bolts) { const s = civ.toScreen(bolt.x, bolt.y); L.unshift({ x: s.x, y: s.y, r: 45 }); }
    return L.slice(0, 16);
  }

  // The HUD scoreboard: population and villages per civilisation.
  objectives() {
    if (!this.civ) return [{ label: 'Planet', value: this.error ? 'failed' : 'forming…', plain: true }];
    const alive = this.factions.filter((f) => f.alive).sort((p, q) => q.pop - p.pop);
    const top = alive.slice(0, 6);
    const me = this.player();
    if (me && !top.includes(me)) top[top.length - 1] = me;
    const rows = top.map((f) => ({
      label: f.isPlayer ? `★ ${f.name}` : f.name,
      value: `${Math.round(f.pop)} · ${Math.round(f.villages)} vil${f.wars ? ' · war' : f.raids ? ' · raid' : ''}`,
      bad: f.wars > 0,
      plain: !f.wars,
    }));
    if (alive.length > top.length) rows.push({ label: 'Others', value: `${alive.length - top.length}`, plain: true });
    return rows;
  }

  skip() {}
}

function describe(f) {
  return [
    f.friendliness > 0.6 ? 'friendly' : f.friendliness < 0.35 ? 'suspicious' : 'wary',
    f.aggressiveness > 0.55 ? 'quick to fight' : f.aggressiveness < 0.3 ? 'slow to anger' : 'proud',
    f.violence > 0.55 ? 'ruthless' : 'merciful',
    f.fertility > 0.6 ? 'and very fertile' : 'and hard-working',
  ];
}

function wrapLabel(ctx, x, y, text, color, size, maxW) {
  ctx.font = `${size}px "Pixelify Sans", monospace`;
  const words = text.split(' ');
  let line = '';
  for (const w of words) {
    const test = line ? `${line} ${w}` : w;
    if (ctx.measureText(test).width > maxW && line) {
      label(ctx, x, y, line, color, size, 'left');
      line = w; y += size + 1;
    } else line = test;
  }
  if (line) label(ctx, x, y, line, color, size, 'left');
}
