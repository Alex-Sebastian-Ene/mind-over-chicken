// Zoom controller: 0 = the playable surface, 1 = the planet as a spinning pixel globe,
// 2 = the galaxy. The game keeps simulating underneath at every level, and made planets
// keep running at every level too.

import { W, H, clamp, lerp } from '../sim/util.js';
import { World } from '../world/terrain.js';
import { Globe, drawStars } from './globe.js';
import { Galaxy, ERA_LABEL, ERA_NAMES, CHANNELS, fmtPop, drawPlanetBolt } from './galaxy.js';
import { label } from '../eras/stone.js';

const LEVELS = ['Surface', 'Planet', 'Galaxy'];
const CENTER = { x: W / 2, y: H / 2 };
const BIG_R = 74;
const SUN = [-0.55, -0.42, 0.72];
const ERA_INDEX = { stone: 0, farming: 1, kingdoms: 1, industry: 2, space: 3 };

const ease = (t) => (t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2);
const smoothstep = (a, b, t) => { const x = clamp((t - a) / (b - a)); return x * x * (3 - 2 * x); };
const wrapAngle = (a) => Math.atan2(Math.sin(a), Math.cos(a));

export class Cosmos {
  constructor() {
    this.globe = new Globe();
    this.galaxy = null;
    this.z = 0;            // animated zoom, 0..2
    this.target = 0;       // requested level
    this.rot = 0;          // globe spin at level 1
    this.spinVel = 0.18;
    this.drag = null;
    this.pointers = new Map();
    this.pinch = null;
    this.focus = null;     // planet shown at level 1 (null = home)
    this.hover = null;
    this.sandbox = false;
    this.time = 0;
    this.flat = Object.assign(document.createElement('canvas'), { width: W, height: H });
    this.toast = null;
    this.wheelAcc = 0;
    this.wheelAt = 0;
    this.lastRun = null;
    this.onEvent = null;
    // East–west pan of the flat surface (world px). The map is a planet, so it wraps.
    this.panX = 0;
    this.panVel = 0;
    this.panned = Object.assign(document.createElement('canvas'), { width: W, height: H });
  }

  get active() { return this.z > 0.02 || this.target > 0; }
  get inGalaxy() { return this.z > 1.5; }
  minLevel() { return this.sandbox || (this.focus && !this.focus.home) ? 1 : 0; }

  reset() {
    this.galaxy = null; this.z = 0; this.target = 0; this.focus = null; this.hover = null; this.sandbox = false; this.drag = null; this.lastRun = null; this.panX = 0; this.panVel = 0;
  }

  ensureGalaxy(G, { autonomousHome = false } = {}) {
    if (this.galaxy) return this.galaxy;
    this.galaxy = new Galaxy();
    // A sandbox home runs on its own, on a mirror of the same world so the paused game's map is untouched.
    this.galaxy.addHome(autonomousHome ? new World(G.world.seed) : G.world, { autonomous: autonomousHome });
    // Three more worlds take shape from the player's first ten seconds of brain data.
    this.galaxy.addMindWorlds();
    this.galaxy.onEvent = (p, text, kind) => this.onEvent?.(p, text, kind);
    return this.galaxy;
  }

  zoomTo(level, G) {
    level = clamp(Math.round(level), this.minLevel(), 2);
    if (level === 0 && this.focus && !this.focus.home) { this.say('Only home can be played up close. Pick Home in the galaxy.'); return; }
    if (level >= 2) this.ensureGalaxy(G);
    if (level === 1 && this.target === 0) this.rot = this.faceRot();
    this.target = level;
  }
  zoomOut(G) { this.zoomTo(this.target + 1, G); }
  zoomIn(G) { this.zoomTo(this.target - 1, G); }

  say(text) { this.toast = { text, t: 2.6 }; }

  // The globe at level 1 faces whatever map column is in the middle of the (panned) surface.
  faceRot() { return Globe.faceRot(this.centerColumn()); }
  centerColumn() { return (((W / 2 - this.panX) % W) + W) % W; }

  // ---- planetary pan of the flat surface ----
  pan(dx) { this.panX = (((this.panX + dx) % W) + W) % W; }
  // Screen x (world px, 0..W) → map x, undoing the pan.
  mapX(screenX) { return (((screenX - this.panX) % W) + W) % W; }
  // Shift the finished surface frame by the pan, wrapping east–west.
  applyPan(low) {
    const off = Math.round(this.panX);
    if (!off) return;
    const pctx = this.panned.getContext('2d');
    pctx.clearRect(0, 0, W, H);
    pctx.drawImage(low, 0, 0);
    const lctx = low.getContext('2d');
    lctx.clearRect(0, 0, W, H);
    lctx.drawImage(this.panned, off, 0);
    lctx.drawImage(this.panned, off - W, 0);
  }
  // X offsets (world px) at which to repeat crisp overlays so they wrap with the map.
  overlayOffsets() { const o = Math.round(this.panX); return o ? [o, o - W] : [0]; }

  homeInfo(G) {
    const s = G.scene;
    if (!s) return null;
    const pop = s.living ? s.living().length : s.agents ? s.agents.length : s.citizens ? s.citizens.length + (s.engineers?.length || 0) + (s.passengers || 0) : s.people ? s.people.length : 0;
    return { eraIndex: ERA_INDEX[s.era] ?? 0, pop, canLaunch: !!(G.finished || s.phase === 'voyage' || s.phase === 'launch') };
  }

  update(dt, G) {
    this.time += dt;
    const speed = 1.5;
    if (this.z < this.target) this.z = Math.min(this.target, this.z + dt * speed);
    else if (this.z > this.target) this.z = Math.max(this.target, this.z - dt * speed);
    if (!this.drag) { this.rot += this.spinVel * dt; this.spinVel = lerp(this.spinVel, 0.18, 1 - Math.exp(-dt * 0.8)); }
    if (this.z <= 0.001 && this.target === 0) this.rot = this.faceRot();
    if (Math.abs(this.panVel) > 0.5) { this.pan(this.panVel * dt); this.panVel *= Math.exp(-dt * 6); } else this.panVel = 0;
    // The world you're looking at hears every channel at full strength and takes your blinks.
    if (this.galaxy) this.galaxy.attended = this.z > 0.5 && this.z < 1.5 ? (this.focus || this.galaxy.home) : null;
    const playDt = this.lastRun == null ? 0 : Math.max(0, G.runTime - this.lastRun);
    this.lastRun = G.runTime;
    if (this.galaxy && G.brain) this.galaxy.update(dt, G.brain, this.homeInfo(G), playDt);
    if (this.toast && (this.toast.t -= dt) <= 0) this.toast = null;
  }

  // Texture + look of the planet being shown large.
  focusLook(G) {
    const p = this.focus || this.galaxy?.home;
    if (!p || (p.home && !p.autonomous)) {
      const layer = G.scene?.layer || G.world.layers.stone;
      return { tex: this.globe.texture(layer, G.world.structures, G.scene?.era), atmosphere: [110, 175, 255], planet: this.galaxy?.home || null };
    }
    return { tex: this.galaxy.textureFor(p, this.globe, G.scene?.layer, G.world.structures), atmosphere: p.atmosphere, planet: p };
  }

  // Draw the cosmos into the low-res canvas, using what the scene just drew as the flat map.
  render(low, ctx, G) {
    if (this.z <= 0.001) return false;
    const fctx = this.flat.getContext('2d');
    fctx.clearRect(0, 0, W, H);
    fctx.drawImage(low, 0, 0);
    drawStars(ctx, this.time, this.time * 2);
    const look = this.focusLook(G);
    const z = this.z;
    if (z <= 1) {
      const e = ease(z);
      const fa = 1 - smoothstep(0.3, 0.75, z);
      if (fa > 0) {
        const s = 1 - 0.78 * e;
        ctx.globalAlpha = fa;
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(this.flat, Math.round(CENTER.x - W * s / 2), Math.round(CENTER.y - H * s / 2), Math.round(W * s), Math.round(H * s));
        ctx.globalAlpha = 1;
      }
      const face = this.faceRot();
      const rot = face + wrapAngle(this.rot - face) * smoothstep(0.55, 1, z);
      const bigR = lerp(30, BIG_R, e);
      if (look.tex) this.globe.draw(ctx, look.tex, CENTER.x, CENTER.y, bigR, { rot, alpha: smoothstep(0.2, 0.6, z), light: SUN, atmosphere: look.atmosphere });
      const lp = look.planet;
      if (lp && lp.flash > 0) drawPlanetBolt(ctx, CENTER.x, CENTER.y, bigR, lp.storms + 1, Math.min(1, lp.flash * 2) * smoothstep(0.5, 0.9, z));
      this.drawMoon(ctx, z);
    } else {
      const t = z - 1, e = ease(t);
      const gal = this.galaxy;
      const fp = look.planet || gal.home;
      gal.draw(ctx, this.globe, G.scene?.layer || G.world.layers.stone, G.world.structures, { alpha: smoothstep(0.15, 0.9, t), skip: fp, hover: this.hover });
      const q = gal.pos(fp);
      const toStar = [160 - q.x, 92 - q.y, 26];
      const n = Math.hypot(...toStar);
      const L = SUN.map((v, i) => lerp(v, toStar[i] / n, e));
      const rot = lerp(this.rot, gal.time * fp.spin + fp.phase, e);
      gal.drawPlanet(ctx, this.globe, fp, lerp(CENTER.x, q.x, e), lerp(CENTER.y, q.y, e), lerp(BIG_R, fp.size, e), G.scene?.layer || G.world.layers.stone, G.world.structures, 1, { rot, light: L, tilt: lerp(0.32, 0.3, e) });
    }
    return true;
  }

  // A tiny moon orbiting the big globe at level 1.
  drawMoon(ctx, z) {
    const a = smoothstep(0.6, 1, z);
    if (a <= 0 || this.z > 1.2) return;
    const ang = this.time * 0.5;
    const x = CENTER.x + Math.cos(ang) * 104, y = CENTER.y + Math.sin(ang) * 30;
    if (Math.sin(ang) < 0 && Math.abs(x - CENTER.x) < BIG_R) return; // behind the planet
    ctx.globalAlpha = a;
    ctx.fillStyle = '#b9b4c8'; ctx.beginPath(); ctx.arc(Math.round(x), Math.round(y), 3, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = '#8c879c'; ctx.fillRect(Math.round(x), Math.round(y) - 1, 1, 1); ctx.fillRect(Math.round(x) - 1, Math.round(y) + 1, 1, 1);
    ctx.globalAlpha = 1;
  }

  // Crisp text in world units.
  overlay(ctx, G) {
    const z = this.z;
    if (z > 0.6 && z < 1.4) {
      const a = 1 - Math.abs(z - 1) / 0.4;
      const p = this.focus;
      const info = this.homeInfo(G);
      const name = p && !p.home ? p.name : 'Home';
      const era = p && !p.home ? (p.pop > 0 ? ERA_LABEL[ERA_NAMES[p.eraIndex]] : 'barren') : ERA_LABEL[ERA_NAMES[info?.eraIndex ?? 0]];
      const pop = p && !p.home ? fmtPop(p.pop) : `${info?.pop ?? 0} people`;
      ctx.globalAlpha = clamp(a);
      label(ctx, CENTER.x, 14, `${name} · ${era} · ${pop}`, '#f2e8d5', 7);
      const ch = p && CHANNELS[p.channel];
      const lp = p || this.galaxy?.home;
      const isHome = !p || p.home;
      label(ctx, CENTER.x, 22, isHome ? 'your world · blink to strike it from orbit' : `you're here: every channel acts on it · ${p.effect || ''}`, ch && !isHome ? ch.color : '#a3a6c4', 5);
      if (lp) lp.labels.forEach((l, i) => { ctx.globalAlpha = clamp(a) * Math.min(1, l.t); label(ctx, (CENTER.x - BIG_R) / 2, CENTER.y - 24 + i * 9, l.text, l.color, 7); ctx.globalAlpha = clamp(a); });
      label(ctx, CENTER.x, H - 6, this.sandbox || (p && !p.home) ? 'drag to spin · scroll or Z to see the galaxy' : 'drag to spin · scroll or X to land · Z for the galaxy', '#a3a6c4', 5);
      ctx.globalAlpha = 1;
    }
    if (this.galaxy && z > 1.2) this.galaxy.drawOverlay(ctx, { alpha: smoothstep(1.3, 1.9, z), hover: this.hover });
    if (this.toast) { ctx.globalAlpha = Math.min(1, this.toast.t); label(ctx, CENTER.x, 30, this.toast.text, '#ffd166', 6); ctx.globalAlpha = 1; }
  }

  // ---- input (coordinates in world pixels) ----
  pointerDown(id, x, y) {
    this.pointers.set(id, { x, y });
    if (this.pointers.size === 2) {
      const [a, b] = [...this.pointers.values()];
      this.pinch = { d: Math.hypot(a.x - b.x, a.y - b.y) };
      this.drag = null;
      return;
    }
    this.drag = { x, y, sx: x, sy: y, moved: 0 };
  }

  pointerMove(id, x, y, G) {
    if (this.pointers.has(id)) this.pointers.set(id, { x, y });
    if (this.pinch && this.pointers.size === 2) {
      const [a, b] = [...this.pointers.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      if (d > this.pinch.d * 1.35) { this.zoomIn(G); this.pinch.d = d; }
      else if (d < this.pinch.d / 1.35) { this.zoomOut(G); this.pinch.d = d; }
      return;
    }
    if (this.drag) {
      const dx = x - this.drag.x;
      this.drag.moved += Math.abs(dx) + Math.abs(y - this.drag.y);
      this.drag.x = x; this.drag.y = y;
      if (this.z > 0.5 && this.z < 1.5) { this.rot -= dx / BIG_R; this.spinVel = -dx / BIG_R / 0.033; }
    }
    if (this.galaxy && this.z > 1.5) this.hover = this.galaxy.hit(x, y);
  }

  pointerUp(id, x, y, G) {
    this.pointers.delete(id);
    if (this.pointers.size < 2) this.pinch = null;
    const d = this.drag;
    this.drag = null;
    if (!d || d.moved > 4) return null;
    if (this.galaxy && this.z > 1.5) {
      const p = this.galaxy.hit(x, y);
      if (p && p.forming) { this.say(p.mindSlot && !this.galaxy.mindReady ? 'Still reading your mind. Keep playing.' : 'Still forming. Give it a second.'); return null; }
      if (p) { this.focusPlanet(p, G); return p; }
    }
    return null;
  }

  focusPlanet(p, G) {
    if (p.forming) { this.say('Still forming. Give it a second.'); return; }
    this.focus = p.home && !p.autonomous ? null : p;
    this.rot = this.galaxy ? this.galaxy.time * p.spin + p.phase : this.rot;
    this.target = 1;
    if (this.z < 1) this.z = 1;
  }

  wheel(deltaY, G) {
    const now = performance.now();
    this.wheelAcc += deltaY;
    if (now - this.wheelAt < 320) return;
    if (this.wheelAcc > 40) { this.zoomOut(G); this.wheelAt = now; this.wheelAcc = 0; }
    else if (this.wheelAcc < -40) { this.zoomIn(G); this.wheelAt = now; this.wheelAcc = 0; }
  }

  levelLabel() {
    const l = LEVELS[this.target];
    return this.target === 1 && this.focus && !this.focus.home ? `${this.focus.name}` : l;
  }
}
