// civ (#50): "Sandbox: four civilisations". Four peoples on a big wrapping planet; the player
// founds them one at a time (#98): choose one, place it, seed its temper from the mind for 5 s,
// and it begins while the others wait. They grow, claim land, trade, raid, go to war and rebel.
// The simulation is Rust (crates/moc-civ, loaded by ./civ.js); this scene feeds it the brain,
// draws its view, and turns its event log into news and advisor lines.
//
// The player is a god, not a general: calm cools every chief, stress and an angry face heat
// them up, focus fattens harvests, blinks throw lightning, and the camera casts (#33:
// onCast/onMood) act on the world. Each civilisation climbs the twenty ages of
// data/ages.json (#69) on its own, and its people fight each other as well as their neighbours.
// The words on screen (headlines, the age and conflict log, advisor lines, HUD rows, capital
// labels) and the speed control (Period or the ×1 button: ×1/×4/×16) live in the Rust crate;
// this file forwards input and draws. The player's own civilisation (★) is seeded by the
// calibration window (#54, G.calibration): from the mind read with ?calib_mode=sensor, at
// random (reproducible from ?seed=) in the default dummy run. The run seed also picks the
// planet and the other three civilisations.

import { W, H, clamp } from '../sim/util.js';
import { makeBolt, drawBolt, drawReticle } from '../render/sprites.js';
import { createCiv, EVENT, CAST, TONE, SETUP } from './civ.js';
import { seamSpans, fadeReliefRim } from './seams.js';
import { calibrationTraits, calibrationLine } from './calibration-traits.js';

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
  if (e.ctrlKey || e.metaKey || e.altKey || !playing()) return;
  if (e.target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
  if (e.code === 'Period' && !e.repeat) {
    active.cycleSpeed();
    e.preventDefault();
    e.stopImmediatePropagation();
    return;
  }
  // Tab offers the next civilisation while founding them (#98).
  if (e.code === 'Tab' && active.setupActive()) {
    active.civ.setupNext();
    e.preventDefault();
    e.stopImmediatePropagation();
    return;
  }
  if (!PAN_KEYS[e.code]) return;
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

// Planet sizes, the player's pick and the random roll live in planet-size.js (DOM-free, tested).
export { PLANET_SIZES, planetSize, planetChoice, initPlanetSizePicker } from './planet-size.js';
import { planetSize, planetChoice } from './planet-size.js';

const smoothstep = (a, b, t) => { const x = clamp((t - a) / (b - a)); return x * x * (3 - 2 * x); };

// The overview map's size on screen (#88).
const MINIMAP_SCALE = 0.6;

// The 5 s mind read while a civilisation is founded (#88): a visible bar and what to do.
export function seedingCardText(progress, seconds = 5) {
  const t = Math.min(seconds, Math.max(0, progress) * seconds);
  return `Reading your mind: ${t.toFixed(1)} / ${seconds} s. Relax, blink, look around.`;
}
function drawSeedingCard(ctx, progress) {
  const w = 150, h = 16, x = Math.round((W - w) / 2), y = H - 34;
  ctx.fillStyle = 'rgba(8,10,24,0.82)';
  ctx.fillRect(x, y, w, h);
  ctx.strokeStyle = 'rgba(89,216,200,0.6)';
  ctx.lineWidth = 0.5;
  ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
  label(ctx, x + w / 2, y + 6, seedingCardText(progress), '#e8e4d8', 4.5);
  ctx.fillStyle = 'rgba(255,255,255,0.12)';
  ctx.fillRect(x + 6, y + 10, w - 12, 3);
  ctx.fillStyle = '#59d8c8';
  ctx.fillRect(x + 6, y + 10, Math.round((w - 12) * Math.min(1, Math.max(0, progress))), 3);
}

export class CivSandbox {
  constructor({ cosmos = null, seed = null } = {}) {
    this.id = 'civ';
    this.era = 'kingdoms';
    this.label = 'Sandbox · A crowded planet';
    this.intro = {
      title: 'A crowded planet',
      goal: 'Found your four peoples on a planet of the size you picked (or a random one), one at a time: choose one, place it, and seed its temper from your mind for 5 seconds. Rival peoples rise on their own: 4 on a small planet, up to 16 on a huge one. Watch them all climb twenty ages, trade, ally, fight and split. Your mind tilts war and peace.',
      how: [
        ['Calm', 'Hold Space: friendlier chiefs, trade, peace treaties, fewer rebels.'],
        ['Stress', 'Hold Shift (or frown at the camera): stress and anger bring raids, wars and rebellions.'],
        ['Focus', 'Hold F: richer harvests, faster growth.'],
        ['Blink', 'Click or B: lightning where you look. It scares rebels back into line.'],
        ['Look around', 'A/D/W/S or the arrow keys pan the planet; right-drag works too.'],
        ['Time', 'Period (.) or the ×1 button: run the world at ×1, ×4 or ×16. Each people climbs twenty ages on its own.'],
      ],
    };
    this.powers = ['blink', 'calm', 'focus', 'stress'];
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
    this.grabbedEntity = null;
    this.mini = null;
    this.miniTimer = 0;
    this.infoTimer = 0;
    this.moodHold = { calm: 0, stress: 0, anger: 0 };
    this.mood = null;
    this.perf = { stepMs: 0, renderMs: 0, worstStepMs: 0, samples: 0, sumStep: 0, sumRender: 0, ring: new Float32Array(600), per: new Float32Array(600), n: 0, avg: 0, p95: 0, stepAvg: 0, stepP95: 0, at: 0 };
    this.log = [];           // the age and conflict log (lines from Rust)
    this.newsSeq = 0;
    this.speedBtn = null;
  }

  cycleSpeed() {
    if (!this.civ) return;
    const k = this.civ.cycleSpeed();
    if (this.speedBtn) this.speedBtn.textContent = `×${k}`;
  }

  // A small button over the planet (bottom left) for the time speed.
  makeSpeedButton() {
    const frame = document.querySelector('.frame');
    if (!frame) return;
    let b = frame.querySelector('.civ-speed');
    if (!b) {
      b = document.createElement('button');
      b.type = 'button';
      b.className = 'zbtn civ-speed';
      b.title = 'Time speed (.)';
      b.setAttribute('aria-label', 'Time speed (.)');
      Object.assign(b.style, { position: 'absolute', left: '8px', bottom: '8px', width: 'auto', padding: '0 8px', fontSize: '14px' });
      frame.appendChild(b);
    }
    b.onclick = () => { if (active) active.cycleSpeed(); };
    b.textContent = `×${this.civ?.speed ?? 1}`;
    this.speedBtn = b;
  }

  // ---------- lifecycle ----------
  enter(G) {
    this.G = G;
    active = this;
    held.clear();
    G.brain.setCue(null);
    // calibration (#54): one run seed for the planet, the civilisations and the dummy seed.
    this.seed = (this.seedArg ?? G.calibration?.config?.seed ?? Math.random() * 2 ** 32) >>> 0;
    // Planets come in sizes (Alex), rolled from the run seed: the bigger the planet, the more
    // peoples on it (the player's four plus rivals that arrive on their own). Overrides:
    // ?planet=small|medium|large|huge or WxH, and ?civs=4..20.
    const q = new URLSearchParams(location.search);
    const size = planetSize(this.seed, q.get('planet') || planetChoice());
    const civs = Math.max(4, Math.min(20, Number(q.get('civs')) || size.civs));
    this.planetSize = { ...size, civs };
    G.advisor.say('civ-intro', `A ${size.name} planet, my lord, and ${civs} peoples on it: your four and ${civs - 4} rivals. They will grow, squabble and split. Your moods are now everyone's problem.`, { priority: 3, cooldown: 999 });
    createCiv(this.seed, { staged: true, rivals: civs - 4, width: size.w, height: size.h })
      .then((civ) => {
        if (G.scene !== this) { civ.free(); return; }
        this.civ = civ;
        this.makeSpeedButton();
        this.refresh();
        this.mini = civ.minimap();
        this.miniCanvas = Object.assign(document.createElement('canvas'), { width: civ.miniW, height: civ.miniH });
        this.miniCanvas.getContext('2d').putImageData(this.mini, 0, 0);
        console.info(`[civ] planet ${civ.pw}×${civ.ph}, seed ${this.seed}`);
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
    // Calibration → your civilisation (#190), once founding is over so it never fights the
    // 5 s seeding windows; the log says what was applied.
    if (!this.calibApplied && civ.setupPhase === SETUP.done && G.calibration?.done) {
      this.calibApplied = true;
      const t = calibrationTraits(G.calibration.result);
      if (t && civ.setPlayerTraits(t.friendliness, t.aggressiveness, t.violence)) {
        this.log.unshift({ text: calibrationLine(t), t: this.t, tone: TONE.info });
        if (this.log.length > 4) this.log.length = 4;
      }
    }

    // Camera: keys.
    let px = 0, py = 0;
    for (const k of held) { px += PAN_KEYS[k][0]; py += PAN_KEYS[k][1]; }
    if (px || py) civ.pan(px * PAN_SPEED * dt, py * PAN_SPEED * dt);

    // The brain (and the face, if the camera is on) tilts every chief this step.
    const calm = b.calm || 0, focus = b.focus || 0;
    const anger = G.mood && G.mood.active !== false ? (G.mood.anger || 0) : (this.mood?.anger || 0);
    const stress = stressOf(b, calm, anger);
    civ.setInputs(calm, focus, stress, anger);
    civ.setJoy(G.mood && G.mood.active !== false ? (G.mood.joy || 0) : (this.mood?.joy || 0));
    if (this.speedBtn) this.speedBtn.hidden = !playing();
    // Founding (#98): the aim shows where the civilisation being placed may begin.
    if (civ.setupPhase === SETUP.place) { const p = this.aimPlanet(b.aim); civ.setupHover(p.x, p.y); }

    // The world runs at its own speed (×1/×4/×16) in steps of at most 0.1 s.
    const t0 = performance.now();
    // Real time (the 5 s seeding window is real seconds even in a slow tab); Rust caps the work.
    civ.advance(Math.min(dt, 0.25));
    const ms = performance.now() - t0;
    const p = this.perf;
    p.stepMs = ms; p.sumStep += ms; p.samples++; p.worstStepMs = Math.max(p.worstStepMs, ms);
    p.per[p.n % p.per.length] = ms / Math.max(1, civ.lastSteps);
    p.ring[p.n++ % p.ring.length] = ms;
    if (this.t - p.at > 1) {
      // Mean and p95 of the sim's milliseconds per frame, and per step, over the last 600 frames.
      p.at = this.t;
      const k = Math.min(p.n, p.ring.length);
      const stat = (arr) => { const v = Array.from(arr.subarray(0, k)).sort((x, y) => x - y); return [v.reduce((s, x) => s + x, 0) / Math.max(1, k), v[Math.min(k - 1, Math.floor(k * 0.95))] || 0]; };
      [p.avg, p.p95] = stat(p.ring);
      [p.stepAvg, p.stepP95] = stat(p.per);
      window.__civPerf = { frameAvg: p.avg, frameP95: p.p95, stepAvg: p.stepAvg, stepP95: p.stepP95, steps: civ.lastSteps, worst: p.worstStepMs, frames: p.n, agents: civ.agents, speed: civ.speed, time: civ.time };
    }

    const events = civ.events();
    if (events.length) { this.refresh(); this.readNews(); }
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
      if (next) { next.seen = true; this.shown = { text: next.text, tone: next.tone, until: this.t + NEWS_HOLD }; }
    }
    this.reactToMood(dt, calm, stress, anger);
  }

  reactToMood(dt, calm, stress, anger) {
    const h = this.moodHold, say = this.G.advisor.say.bind(this.G.advisor);
    h.calm = calm > 0.65 ? h.calm + dt : 0;
    h.stress = stress > 0.6 ? h.stress + dt : 0;
    h.anger = anger > 0.4 ? h.anger + dt : 0;
    const wars = this.factions.reduce((s, f) => s + (f.alive ? f.wars : 0), 0) / 2;
    if (h.calm > 4) say('civ-calm', wars ? 'Your calm spreads, my lord. The chiefs are tired of war and want soup.' : 'Your calm spreads, my lord. Chiefs unclench their fists and swap grain.', { mood: 'happy', priority: 1, cooldown: 25 });
    if (h.stress > 3) say('civ-stress', 'You are tense, my lord, and so are the war drums.', { mood: 'panic', priority: 2, cooldown: 25 });
    if (h.anger > 3) say('civ-anger', 'That scowl, my lord. Every village is grumbling now. Rebels are sharpening sticks.', { mood: 'panic', priority: 2, cooldown: 25 });
  }

  // ---------- news (the words come from Rust: crates/moc-civ/src/news.rs) ----------
  readNews() {
    const { lines, next } = this.civ.news(this.newsSeq);
    this.newsSeq = next;
    const say = this.G.advisor.say.bind(this.G.advisor);
    for (const l of lines) {
      if (l.text) {
        this.news.unshift({ text: l.text, t: this.t, tone: l.tone, seen: false });
        // Keep a short backlog; skip stale headlines rather than queue them for minutes.
        for (const n of this.news.slice(4)) n.seen = true;
        if (this.news.length > 40) this.news.length = 40;
      }
      if (l.log) {
        this.log.unshift({ text: l.logText, t: this.t, tone: l.tone });
        if (this.log.length > 4) this.log.length = 4;
      }
      if (l.say) say(l.key, l.say, { mood: l.tone === TONE.bad ? 'panic' : l.tone === TONE.good ? 'happy' : undefined, priority: l.priority, cooldown: l.cooldown });
      if (l.kind === EVENT.WAR || l.kind === EVENT.REBELLION || l.kind === EVENT.CIVIL_WAR) this.G.audio?.bad?.();
      if (l.kind === EVENT.AGE_UP) this.G.audio?.good?.();
    }
  }

  // ---------- the god's hand ----------
  aimPlanet(aim) {
    return this.civ.toPlanet(aim.x * (W - 1), aim.y * (H - 1));
  }

  // Still founding the civilisations (#98)?
  setupActive() {
    const ph = this.civ?.setupPhase;
    return ph === SETUP.choose || ph === SETUP.place || ph === SETUP.seed;
  }

  onBlink(aim) {
    const civ = this.civ, G = this.G;
    if (!civ) return;
    const p = this.aimPlanet(aim);
    // While founding, a click or blink chooses, places, or (seeding) makes the people wilder.
    if (this.setupActive()) { civ.setupClick(p.x, p.y); this.refresh(); return; }
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

  // Living-entity pinch grab (#120)
  // Pinch-grab (#120, #188): pick up a living person and set them down elsewhere. The Rust sim
  // owns everyone's position, so the move goes through it (villages stay where they were built).
  onGrabStart(aim) {
    const civ = this.civ;
    if (!civ) return null;
    const p = this.aimPlanet(aim);
    const i = civ.agentNear(p.x, p.y, 6);
    if (i < 0) return { ok: false, reason: 'Pinch over a person to pick them up' };
    const p0 = civ.agentPos(i);
    this.grabbedEntity = { agent: i, p0, label: 'Villager' };
    return { ok: true, label: 'Villager', kind: 'person' };
  }

  onGrabMove(aim) {
    const g = this.grabbedEntity;
    if (!g || !this.civ) return;
    const p = this.aimPlanet(aim);
    this.civ.moveAgent(g.agent, p.x, p.y);   // they ride along under the hand (only over land)
  }

  onGrabEnd({ cancelled = false } = {}) {
    const g = this.grabbedEntity;
    if (!g) return;
    this.grabbedEntity = null;
    if (cancelled && this.civ) this.civ.moveAgent(g.agent, g.p0.x, g.p0.y);
  }

  // Charged fireball (#291): a bright hit at the cursor.
  onFireball(aim, { color = '#ff9a2e', charge = 1 } = {}) {
    const G = this.G;
    if (!G) return;
    const p = this.aimPlanet(aim);
    this.flash = 0.4;
    G.audio?.thunder?.();
    G.shake = Math.max(G.shake || 0, 1.2);
    if (this.particles) this.particles.burst(16 + Math.round(charge * 16), () => ({ x: p.x, y: p.y, vx: rand(-30, 30), vy: rand(-40, 5), g: 50, life: rand(0.4, 0.7), maxLife: 0.7, size: 1, color }));
    this.G.advisor?.say('civ-fireball', `A fireball! ${Math.round(charge * 100)}% charged and very visible.`, { mood: 'panic', priority: 2, cooldown: 8 });
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
    if (spell === 'palm' || spell === 'peace') { G.audio?.good?.(); say('civ-cast-peace', n ? `Peace! ${n} war${n === 1 ? '' : 's'} ended near your hand.` : 'Peace descends. Nobody was fighting there, but they appreciate it.', { mood: 'happy', priority: 2, cooldown: 15 }); }
    if (spell === 'disaster') { this.flash = 0.5; G.audio?.thunder?.(); G.shake = Math.max(G.shake || 0, 2); say('civ-cast-storm', 'A storm! That will settle a few arguments. And a few villages.', { mood: 'panic', priority: 2, cooldown: 15 }); }
    if (spell === 'spawn') { G.audio?.coin?.(); say('civ-cast-food', 'Food from the sky. Nobody will ask questions.', { mood: 'happy', priority: 1, cooldown: 15 }); }
    if (spell === 'bless') { G.audio?.good?.(); say('civ-cast-bless', 'Blessed! The grumbling stops. For now.', { mood: 'happy', priority: 1, cooldown: 15 }); }
    if (spell === 'rain') say('civ-cast-rain', 'Rain! The fires go out and the fields drink.', { mood: 'happy', priority: 1, cooldown: 15 });
  }

  // camera (#49): face mood. G.mood carries it every frame; this keeps a copy and reacts.
  onMood(mood) {
    this.mood = mood;
  }

  // In proportion to the scroll: a mouse notch zooms about ×1.25, a trackpad glides (Alex: easier zoom).
  onWheel(delta) {
    this.zoom = clamp((this.zoom || 1.0) * Math.exp(-clamp(delta, -300, 300) * 0.0022), 0.6, 2.5);
  }

  // A drag from main.js (world units, both axes): the content follows the pointer, so the camera
  // moves the other way. This planet is far bigger than the view, so north–south matters here.
  panBy(dx, dy) {
    const z = this.zoom || 1.0;
    this.civ?.pan(-dx / z, -dy / z);
  }

  centerColumn() { return W / 2; }

  getVisibleHeightmap() {
    const civ = this.civ;
    if (!civ) return null;
    const view = civ.render();
    if (!view || !view.data) return null;
    const data = view.data;
    if (!this._heightmap || this._heightmap.length !== W * H) {
      this._heightmap = new Float32Array(W * H);
      this._smoothHeight = new Float32Array(W * H);
    }
    const h = this._heightmap;

    for (let i = 0, p = 0; i < W * H; i++, p += 4) {
      const r = data[p], g = data[p + 1], b = data[p + 2];
      // Water
      if (b > r + 15 && b > 70) {
        h[i] = b > 140 ? 0.27 : 0.18;
      } else if (r > 200 && g > 200 && b > 200) {
        // Snow / ice
        h[i] = 0.88;
      } else if (r > 100 && g > 95 && b > 85 && Math.abs(r - g) < 18) {
        // Mountain
        h[i] = 0.78;
      } else if (r > 110 && g > 105 && b < 100) {
        // Hills
        h[i] = 0.62;
      } else if (g > r && g > b) {
        // Plains / forest
        h[i] = g > 120 ? 0.42 : 0.48;
      } else if (r > 170 && g > 150 && b < 160) {
        // Sand / beach
        h[i] = 0.35;
      } else {
        // Settlement / structure / road
        h[i] = 0.42;
      }
    }

    // 1-pass fast smoothing box blur for rolling terrain
    const s = this._smoothHeight;
    for (let y = 0; y < H; y++) {
      const y0 = Math.max(0, y - 1) * W;
      const y1 = y * W;
      const y2 = Math.min(H - 1, y + 1) * W;
      for (let x = 0; x < W; x++) {
        const x0 = Math.max(0, x - 1);
        const x1 = x;
        const x2 = Math.min(W - 1, x + 1);
        s[y1 + x1] = (h[y0 + x0] + h[y0 + x1] + h[y0 + x2] +
                      h[y1 + x0] + h[y1 + x1] + h[y1 + x2] +
                      h[y2 + x0] + h[y2 + x1] + h[y2 + x2]) * 0.111111;
      }
    }
    return fadeReliefRim(s, W, H);
  }

  lights() {
    const L = [];
    for (const bolt of this.bolts) {
      const s = this.civ?.toScreen(bolt.x, bolt.y);
      if (s) L.push({ x: s.x, y: s.y, r: 60 * Math.min(1, bolt.life * 4) });
    }
    return L;
  }

  draw3d(engine, _dt) {
    if (!engine) return;
    for (const bolt of this.bolts) {
      const s = this.civ?.toScreen(bolt.x, bolt.y);
      if (s && s.x >= -20 && s.x <= W + 20 && s.y >= -20 && s.y <= H + 20) {
        engine.add_3d_light(s.x, 12.0, s.y, 60.0 * Math.min(1, bolt.life * 4), 2.5);
      }
    }
    for (const f of this.factions) {
      if (!f.alive || !f.capital) continue;
      const s = this.civ?.toScreen(f.capital.x + 0.5, f.capital.y);
      if (s && s.x >= 0 && s.x <= W && s.y >= 0 && s.y <= H) {
        engine.add_3d_light(s.x, 9.0, s.y, 20.0, 1.2);
      }
    }
  }

  // ---------- drawing ----------
  draw(ctx) {
    // Every pan (drag, A/D/W/S, wheel) reaches this planet's own camera through panBy: main.js
    // routes cosmos.pan here while the sandbox is on (#129).
    const civ = this.civ;
    if (!civ) {
      ctx.fillStyle = '#0b1020';
      ctx.fillRect(0, 0, W, H);
      return;
    }
    const t0 = performance.now();
    if (!this.viewCanvas) {
      this.viewCanvas = Object.assign(document.createElement('canvas'), { width: W, height: H });
    }
    this.viewCanvas.getContext('2d').putImageData(civ.render(), 0, 0);
    const z = this.zoom || 1.0;
    if (Math.abs(z - 1.0) < 0.01) {
      ctx.drawImage(this.viewCanvas, 0, 0);
    } else {
      ctx.save();
      ctx.translate(W / 2, H / 2);
      ctx.scale(z, z);
      ctx.translate(-W / 2, -H / 2);
      ctx.drawImage(this.viewCanvas, 0, 0);
      ctx.restore();
    }
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
    // Names (and ages) at capitals.
    for (const f of this.factions) {
      if (!f.alive || !f.capital) continue;
      const s = civ.toScreen(f.capital.x + 0.5, f.capital.y);
      if (s.x < -30 || s.x > W + 30 || s.y < 4 || s.y > H + 4) continue;
      label(ctx, s.x, s.y - 9, civ.capitalLabel(f.id), f.css, 5);
    }
    // Overview map with the camera's window.
    if (this.miniCanvas) {
      // Smaller by default (#88): an overview, not a second map.
      const mw = Math.round(civ.miniW * MINIMAP_SCALE), mh = Math.round(civ.miniH * MINIMAP_SCALE), x0 = W - mw - 3, y0 = 3;
      ctx.fillStyle = 'rgba(8,10,24,0.75)';
      ctx.fillRect(x0 - 1, y0 - 1, mw + 2, mh + 2);
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(this.miniCanvas, x0, y0, mw, mh);
      const kx = mw / civ.pw, ky = mh / civ.ph;
      const cx = civ.camX * kx, cy = civ.camY * ky, cw = W * kx, ch = H * ky;
      ctx.strokeStyle = '#ffe066';
      ctx.lineWidth = 0.5;
      // The camera's window, split where it crosses either seam (the planet is a torus, #111).
      for (const [rx, rw] of seamSpans(cx, cw, mw)) {
        for (const [ry, rh] of seamSpans(cy, ch, mh)) ctx.strokeRect(x0 + rx, y0 + ry, rw, rh);
      }
    }
    // Founding the civilisations (#98): the words come from Rust.
    if (this.setupActive()) {
      let y = 9;
      for (const [i, line] of civ.setupLines().entries()) y = wrapLabel(ctx, 4, y, line, i === 0 ? '#59d8c8' : '#e8e4d8', 5, W - 80) + 1;
      if (civ.setupPhase === SETUP.seed) drawSeedingCard(ctx, civ.setupProgress);
    } else {
      // The age and conflict log: the last few age-ups, Dark Ages and escalations.
      let y = 9;
      for (const l of this.log) {
        if (this.t - l.t > 40) continue;
        label(ctx, 4, y, l.text, TONE_COLOUR[l.tone], 5, 'left');
        y += 6;
      }
    }
    // News ticker.
    ctx.fillStyle = 'rgba(8,10,24,0.72)';
    ctx.fillRect(0, H - 9, W, 9);
    const n = this.shown;
    const color = n ? TONE_COLOUR[n.tone] : '#a3a6c4';
    const s = this.stats;
    ctx.font = '5px "Pixelify Sans", monospace';
    ctx.fillStyle = color;
    ctx.textAlign = 'left';
    ctx.fillText(n ? n.text : 'All quiet. For now.', 3, H - 2.6);
    if (s) {
      ctx.textAlign = 'right';
      ctx.fillStyle = '#7d80a0';
      ctx.fillText(`×${s.speed} · ${Math.round(s.agents)} people · wars ${s.wars} · civil wars ${s.civilWars} · ${this.civ?.ppo ? 'PPO councils' : 'rules'} · sim ${this.perf.avg.toFixed(1)} ms`, W - 3, H - 2.6);
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

  // The HUD scoreboard: each people's age, size and trouble (rows written by Rust).
  objectives() {
    if (!this.civ) return [{ label: 'Planet', value: this.error ? 'failed' : 'forming…', plain: true }];
    if (this.t - (this.hudAt ?? -1) > 0.25) { this.hudAt = this.t; this.hud = this.civ.hudRows().map((r) => ({ ...r, plain: !r.bad })); }
    return this.hud;
  }

  // Enter: while founding, take the civilisation offered or begin it in its homeland.
  skip() { if (this.setupActive()) { this.civ.setupEnter(); this.refresh(); } }
}

// Stress, 0..1 and 0 at rest: the brain's own reading when it has one (neutral at rest), else
// max(1 - calm, anger) measured from where it sits at rest (0.8; resting calm is low).
const STRESS_REST = 0.8;
function stressOf(brain, calm, anger) {
  if (typeof brain?.stress === 'number') return clamp(brain.stress);
  return clamp((Math.max(1 - calm, anger) - STRESS_REST) / (1 - STRESS_REST));
}

const TONE_COLOUR = ['#ffe066', '#9be38a', '#ff8a7a', '#ffb36b'];

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
  return y + size;
}
