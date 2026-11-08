// A non-home world up close (#68): its vast surface (src/cosmos/vast.js), the settlements its
// civilisation has grown (from planet-sim's tier, era and people), and, at the capital, the
// tribe the PPO AI god plays (src/ai/planet.js + src/ai/autopilot.js).
//
// It behaves like a scene (draw, drawAfterLight, lights, drawOverlay, objectives, onBlink,
// onCast, onMood), so main.js renders it and routes the player's inputs to it exactly as it
// does for the home campaign. The player always acts on top of the AI: blinks strike where
// they aim (the tribe's own lightning inside the capital, a bolt anywhere else), casts and
// mood reach the world in view, and calm/focus reach the tribe through SharedBrain. The AI
// never touches the player's input; it only drives this simulated world.

import { W, H, clamp, rand, pick, mulberry32, Particles, floater } from '../sim/util.js';
import { T } from '../world/terrain.js';
import { label } from '../eras/stone.js';
import { SimPlanet } from '../ai/planet.js';
import { AIGod } from '../ai/autopilot.js';
import { drawHut, drawHouse, drawTower, drawFactory, drawPad, drawPerson, drawBolt, makeBolt, drawReticle } from '../render/sprites.js';
import { FireballFX } from '../render/fireball.js';
import { villagerLook, animate } from '../sim/agents.js';
import { VW, VH, wrapX, wrapDx, wrapY, wrapDy, typeAt, walkableAt, settlementPlan } from './vast.js';
import { TIERS, CONFLICT, castPlanet, tribeOutcome, easeSplit, tierName } from './planet-sim.js';

const ERA_KEYS = ['stone', 'farming', 'industry', 'space'];
const RESTART_AFTER = 3;                  // seconds the tribe dances (or lies still) before a new one gathers
// While the player watches a dead tribe, a new one doesn't gather in front of them (#297): the
// camera goes home after a short beat and the restart happens off-screen. This is the cap if
// they stay (they turned the return down by moving the camera).
const RESTART_IN_VIEW = 8;
const DARKNESS = [0.42, 0.36, 0.3, 0.26];  // night shading by era (lights carry the settlements)
const ROOFS = { farming: ['#8e3b2a', '#a4402f'], industry: ['#a4402f', '#8e3b2a', '#b5543a', '#6f4a3a'], space: ['#5a6b8c', '#7f8fa8', '#a4402f', '#4d5d7a'] };
const BUILDINGS = [3, 7, 14, 24];         // buildings per settlement size (camp … city)
const FARMS = [0, 3, 6, 9];               // farm plots per settlement size from Farming on
const WALKERS = [2, 4, 7, 10];            // villagers drawn per settlement size
const MAX_LIGHTS = 16;                    // the WebGL engine's light budget

const dummyCtx = () => new Proxy({
  drawImage() {},
  createImageData: (w = 1, h = 1) => ({ data: new Uint8ClampedArray(w * h * 4) }),
  putImageData() {},
  fillRect() {},
  clearRect() {},
  save() {},
  restore() {},
  translate() {},
  scale() {},
  beginPath() {},
  arc() {},
  fill() {},
  stroke() {},
  fillText() {},
  measureText: () => ({ width: 0 }),
}, {
  get: (target, prop) => (prop in target ? target[prop] : () => {}),
});

const makeCanvas = (w, h) => (typeof document !== 'undefined'
  ? Object.assign(document.createElement('canvas'), { width: w, height: h })
  : { width: w, height: h, getContext: dummyCtx });
let blank = null;
const blankLayer = () => (blank ||= makeCanvas(1, 1));
export const fmtNum = (n) => (n < 1000 ? `${Math.round(n)}` : `${(n / 1000).toFixed(n < 10000 ? 1 : 0)}k`);

export class PlanetSurface {
  constructor(p, world) {
    this.p = p;
    this.world = world;
    this.v = world.vast;
    this.id = 'surface';
    this.era = 'surface';
    this.powers = ['blink', 'calm', 'focus', 'stress'];
    this.base = makeCanvas(VW, VH);
    const bctx = this.base.getContext('2d');
    const img = bctx.createImageData(VW, VH);
    img.data.set(this.v.rgba);
    bctx.putImageData(img, 0, 0);
    this.v.rgba = null;                 // the canvas holds the pixels now
    this.built = makeCanvas(VW, VH);    // terrain + settlements, rebaked when they change
    this.tribeCanvas = makeCanvas(W, H);
    this.plan = []; this.planKey = ''; this.globeLayer = null; this.structs = []; this.bakeT = 0;
    const cap = this.v.spots[0];
    this.camX = wrapX(cap.x - W / 2);
    this.camY = clamp(cap.y - H / 2, 0, VH - H);
    this.sx = Math.round(this.camX); this.sy = Math.round(this.camY);
    this.t = 0; this.ownFlash = 0; this.bolts = []; this.lastBolt = -5; this.strikes = 0;
    this.particles = new Particles();
    this.fireballs = new FireballFX();
    this.walkers = new Map();
    this.sim = null; this.human = null; this.restartT = 0; this.acc = 0; this.episodes = 0; this.lastOutcome = null;
    this.split = { growth: 0.4, research: 0.4, stability: 0.2 };
    this.tribeVisible = false;
    this.rebake(true);
    p.surface = this;
    p.surfaceReady = true;
    if (p.pop > 0) this.ensureTribe();
  }

  get label() { return this.sim ? `${this.p.name} · AI god` : `${this.p.name} · nobody lives here yet`; }
  get darkness() { return DARKNESS[this.p.eraIndex] ?? 0.3; }
  get flash() { return Math.max(this.ownFlash, this.tribeVisible && this.sim ? this.sim.scene.flash * 0.6 : 0); }

  // The capital tribe and its AI god (PPO policy, cheap proxy brain). Its moves steer the whole
  // world's effort through the p.god seam in planet-sim.js.
  ensureTribe() {
    if (this.sim) return this.sim;
    // The surface paints the land itself, so the tribe's Stone Age only needs the land use
    // (shape), not a 320×180 painted layer: skip that paint on every (re)start.
    const w = this.world;
    w.paint = () => blankLayer();
    w.drawWater = () => {};
    w.layers3d = null;
    this.sim = new SimPlanet({ seed: w.seed, biome: w.biome, brain: 'proxy', world: w }).setGod(new AIGod());
    this.sim.scene.layer = blankLayer();   // the surface draws the land; the tribe draws itself
    if (this.sim.scene.G?.world) {
      this.sim.scene.G.world.drawWater = () => {};
      this.sim.scene.G.world.layers3d = null;
    }
    if (this.human) this.sim.attachHuman(this.human);
    this.p.god = { kind: 'ppo', act: () => this.split };
    return this.sim;
  }

  // The player's brain while this world is the one they are looking at (null when it isn't).
  setHuman(brain) {
    this.human = brain || null;
    if (this.sim && this.sim.human !== this.human) this.sim.attachHuman(this.human);
  }

  // Advance the tribe and its god. inView: also animate the villagers and the fighting.
  tick(dt, inView = false) {
    const events = [];
    this.t += dt;
    this.ownFlash = Math.max(0, this.ownFlash - dt);
    this.bolts = this.bolts.filter((b) => (b.life -= dt) > 0);
    for (const hit of this.fireballs.update(dt)) this.impactFireball(hit);
    this.particles.update(dt);
    const sp = this.sim;
    if (sp?.G) sp.G.handInput = this.handInput;   // Rain spell: the capital tribe needs the palm state
    if (sp) {
      if (sp.outcome) {
        // #297: say so the moment the tribe dies, so main.js can take the camera home.
        if (this.restartT === 0 && sp.outcome === 'lose') events.push({ kind: 'lost', text: 'the capital tribe fell', detail: sp.failReason || null });
        sp.advance(dt);   // the tribe keeps living (dancing, or not) for a moment
        if ((this.restartT += dt) >= (inView && sp.outcome === 'lose' ? RESTART_IN_VIEW : RESTART_AFTER)) {
          events.push(...tribeOutcome(this.p, sp.outcome));
          this.lastOutcome = sp.outcome;
          sp.restart();
          sp.scene.layer = blankLayer();
          this.restartT = 0;
          this.episodes++;
        }
      } else {
        sp.update(dt);    // the AI god decides (every 0.25 s of game time), then the tribe lives
        if (sp.god?.last) easeSplit(this.split, sp.god.last.name, dt);
      }
    }
    if (inView) { this.updateWalkers(dt); this.ambient(dt); }
    return events;
  }

  // Rebake the settlements when the plan changes; checked at most twice a second.
  maybeRebake(dt) {
    if ((this.bakeT -= dt) > 0) return false;
    this.bakeT = 0.5;
    return this.rebake();
  }

  rebake(force = false) {
    const p = this.p;
    const plan = settlementPlan(p, this.v.spots);
    const rebels = p.factions && plan.length > 1 ? Math.max(1, Math.round((plan.length - 1) * p.factions[1].share)) : 0;
    const key = `${p.eraIndex}|${plan.map((s) => s.size).join('')}|${rebels}`;
    if (!force && key === this.planKey) return false;
    this.planKey = key;
    plan.forEach((s, i) => { s.rebel = rebels > 0 && i > 0 && i >= plan.length - rebels; });
    this.plan = plan;
    const ctx = this.built.getContext('2d');
    ctx.drawImage(this.base, 0, 0);
    const era = ERA_KEYS[p.eraIndex] || 'stone';
    const structs = [];
    for (const s of plan) this.drawSettlement(ctx, s, era, structs);
    for (const [i] of this.walkers) if (!plan[i]) this.walkers.delete(i);
    // The globe's texture: the same map at 320×180, lights where the buildings are.
    const g = makeCanvas(W, H);
    const gctx = g.getContext('2d', { willReadFrequently: true });   // Globe.texture reads it back
    gctx.imageSmoothingEnabled = false;
    gctx.drawImage(this.built, 0, 0, VW, VH, 0, 0, W, H);
    this.globeLayer = g;
    this.structs = structs.map((s) => ({ kind: s.kind, x: s.x / 2, y: s.y / 2 }));
    this.placedStructs = structs;
    return true;
  }

  drawSettlement(ctx, s, era, structs) {
    const v = this.v, pal = this.world.palette;
    const rng = mulberry32((this.world.seed + s.index * 977 + 1) >>> 0);
    const size = s.size;
    const wrapDraw = (x, fn) => { fn(x); if (x < 24) fn(x + VW); if (x > VW - 24) fn(x - VW); };
    if (era !== 'stone') {
      const farm = pal[T.FARM];
      const plots = FARMS[size] + (s.capital ? 2 : 0);
      for (let k = 0; k < plots; k++) {
        const a = rng() * Math.PI * 2, r = 12 + size * 5 + rng() * (8 + size * 5);
        const fx = Math.round(s.x + Math.cos(a) * r), fy = Math.round(s.y + Math.sin(a) * r * 0.7);
        const fw = 5 + Math.floor(rng() * 6), fh = 3 + Math.floor(rng() * 4);
        for (let y = fy; y < fy + fh; y++) {
          const c = farm[(y >> 1) % 2];
          ctx.fillStyle = `rgb(${c[0]},${c[1]},${c[2]})`;
          for (let x = fx; x < fx + fw; x++) if (typeAt(v, x, y) === T.GRASS) wrapDraw(wrapX(x), (xx) => ctx.fillRect(xx, y, 1, 1));
        }
      }
    }
    if (size >= 2 && (era === 'industry' || era === 'space')) {
      ctx.fillStyle = 'rgb(146,146,152)';
      wrapDraw(wrapX(s.x - 3), (xx) => ctx.fillRect(xx, s.y - 2, 7, 5));
    }
    const placed = [];
    const n = BUILDINGS[size];
    // Scatter the buildings in a loose cluster instead of on tidy concentric rings: the angle
    // is free and the radius is biased towards the middle, so the village still reads as one
    // place while avoiding the ringed look. A little spacing stops the sprites piling up.
    const spreadX = (s.capital ? 15 : 9) + size * 3;
    const spreadY = spreadX * (s.capital ? 0.62 : 0.7);
    for (let i = 0; i < n; i++) {
      let x = 0, y = 0, clear = false;
      for (let tries = 0; tries < 14 && !clear; tries++) {
        const a = rng() * Math.PI * 2;
        const r = Math.pow(rng(), 1.35);     // usually near the centre, sometimes out at the edge
        const innerX = s.capital ? 6 : 4, innerY = innerX * (spreadY / spreadX);
        x = wrapX(Math.round(s.x + Math.cos(a) * (innerX + r * (spreadX - innerX))));
        y = Math.round(s.y + Math.sin(a) * (innerY + r * (spreadY - innerY)));
        clear = walkableAt(v, x, y)
          && !placed.some((b) => Math.abs(wrapDx(x - b.x)) < 4 && Math.abs(y - b.y) < 3);
      }
      if (!clear) continue;
      const kind = era === 'stone' ? 'hut' : era === 'farming' ? (size >= 2 && i % 3 === 0 ? 'house' : 'hut') : era === 'space' && size >= 2 && i % 3 === 0 ? 'tower' : 'house';
      placed.push({ kind, x, y, h: 7 + Math.floor(rng() * 7), roof: pick(ROOFS[era] || ROOFS.farming) });
    }
    if (size >= 2 && (era === 'industry' || era === 'space') && walkableAt(v, s.x - 14, s.y + 10)) placed.push({ kind: 'factory', x: wrapX(s.x - 14), y: s.y + 10 });
    if (s.capital && era === 'space' && size >= 2 && walkableAt(v, s.x + 18, s.y + 8)) placed.push({ kind: 'pad', x: wrapX(s.x + 18), y: s.y + 8 });
    placed.sort((a, b) => a.y - b.y);
    for (const b of placed) {
      wrapDraw(b.x, (x) => {
        if (b.kind === 'hut') drawHut(ctx, x, b.y);
        else if (b.kind === 'house') drawHouse(ctx, x, b.y, b.roof);
        else if (b.kind === 'tower') drawTower(ctx, x, b.y, b.h);
        else if (b.kind === 'factory') drawFactory(ctx, x, b.y);
        else if (b.kind === 'pad') drawPad(ctx, x, b.y);
      });
      structs.push(b);
    }
    // Two factions after a civil war: banners show who holds which settlement.
    if (this.p.factions) {
      const colour = s.rebel ? '#e8452c' : '#ffd166';
      wrapDraw(wrapX(s.x + 4), (x) => {
        ctx.fillStyle = '#5a3a22'; ctx.fillRect(x, s.y - 12, 1, 8);
        ctx.fillStyle = colour; ctx.fillRect(x + 1, s.y - 12, 3, 2);
      });
    }
  }

  // ---- villagers on the other settlements (the capital has the tribe) ----
  updateWalkers(dt) {
    const cx = this.camX + W / 2, cy = this.camY + H / 2;
    const era = ERA_KEYS[this.p.eraIndex] || 'stone';
    for (const s of this.plan) {
      if (s.capital) continue;
      if (Math.abs(wrapDx(s.x - cx)) > W || Math.abs(s.y - cy) > H) continue;
      let list = this.walkers.get(s.index);
      if (!list) { list = []; this.walkers.set(s.index, list); }
      const n = WALKERS[s.size];
      while (list.length < n) list.push(this.newWalker(s, era));
      if (list.length > n) list.length = n;
      for (const a of list) {
        animate(a, dt, 6);
        if (a.wait > 0) { a.wait -= dt; continue; }
        const dx = wrapDx(a.tx - a.x), dy = a.ty - a.y, d = Math.hypot(dx, dy);
        if (d < 1) { this.retarget(a, s); a.wait = rand(0.4, 2.2); continue; }
        const step = Math.min(d, 6 * dt);
        a.x = wrapX(a.x + (dx / d) * step); a.y += (dy / d) * step;
        a.facing = dx >= 0 ? 1 : -1;
      }
    }
  }

  newWalker(s, era) {
    const a = { x: s.x, y: s.y, tx: s.x, ty: s.y, frame: 0, ft: rand(0, 4), wait: rand(0, 1), facing: 1, ...villagerLook(era) };
    this.retarget(a, s);
    a.x = a.tx; a.y = a.ty;
    this.retarget(a, s);
    return a;
  }

  retarget(a, s) {
    const r = 6 + s.size * 6;
    for (let i = 0; i < 8; i++) {
      const x = wrapX(Math.round(s.x + rand(-r, r))), y = Math.round(s.y + rand(-r, r) * 0.7);
      if (walkableAt(this.v, x, y)) { a.tx = x; a.ty = y; return; }
    }
    a.tx = s.x; a.ty = s.y;
  }

  // Fighting you can see: clashes in the contested settlements during a revolt or civil war.
  ambient(dt) {
    const level = this.p.conflict ?? 0;
    if (level < 2 || !this.plan.length || Math.random() > dt * (level === 3 ? 5 : 2)) return;
    const s = pick(this.plan.filter((q) => q.rebel || !this.p.factions));
    if (!s) return;
    const x = s.x + rand(-10, 10), y = s.y + rand(-6, 6);
    this.particles.burst(6, () => ({ x, y, vx: rand(-14, 14), vy: rand(-18, -2), g: 40, life: 0.5, maxLife: 0.5, size: 1, color: pick(['#ffffff', '#e8452c', '#ffd166']) }));
    if (Math.random() < 0.3) this.particles.add({ x, y: y - 3, vx: rand(-2, 2), vy: -6, life: 2, maxLife: 2, size: 2, color: 'rgba(90,90,100,0.6)' });
  }

  // ---- camera: torus wraps east-west and clamps north-south bounds ----
  pan(dx, dy = 0) {
    this.camX = wrapX(this.camX - dx);
    this.camY = clamp(this.camY - dy, 0, VH - H);
  }
  // Globe column (0..W) facing the viewer when the middle of this view is in front.
  centerColumn() { return (wrapX(this.camX + W / 2) / VW) * W; }
  screenToWorld(aim) { return { x: wrapX(this.camX + aim.x * (W - 1)), y: wrapY(this.camY + aim.y * (H - 1)) }; }
  // World point → the capital tribe's 0..1 aim, or null outside the capital region.
  toTile(wx, wy) {
    const lx = wrapX(wx - this.v.tile.x), ly = wrapY(wy - this.v.tile.y);
    return lx <= W - 1 && ly <= H - 1 ? { x: lx / (W - 1), y: ly / (H - 1) } : null;
  }
  // Screen offsets at which the capital region shows (two across the date line).
  tileOffsets(sx, sy) {
    const y = this.v.tile.y - sy;
    if (y <= -H || y >= H) return [];
    const base = this.v.tile.x - sx;
    return [base, base + VW, base - VW].filter((x) => x > -W && x < W).map((x) => ({ x, y }));
  }
  // Translations that put world coordinates on screen (a second one across the date line).
  worldOffsets(sx) { return sx > VW - W ? [-sx, VW - sx] : [-sx]; }
  screenOf(x, y) { return { x: wrapX(x - this.sx), y: y - this.sy }; }

  getGlobeHeightmap() {
    if (!this.v?.height) return this.world?.h || null;
    if (!this._globeHeight) {
      this._globeHeight = new Float32Array(W * H);
      const vh = this.v.height;
      for (let y = 0; y < H; y++) {
        const vy = y * 2;
        const rowOffsetV = vy * VW;
        const rowOffsetVNext = Math.min(vy + 1, VH - 1) * VW;
        const rowOffsetSlice = y * W;
        for (let x = 0; x < W; x++) {
          const vx = x * 2;
          const vxNext = (vx + 1) % VW;
          const h00 = vh[rowOffsetV + vx];
          const h10 = vh[rowOffsetV + vxNext];
          const h01 = vh[rowOffsetVNext + vx];
          const h11 = vh[rowOffsetVNext + vxNext];
          this._globeHeight[rowOffsetSlice + x] = (h00 + h10 + h01 + h11) * 0.25;
        }
      }
    }
    return this._globeHeight;
  }

  getVisibleHeightmap() {
    return this.getGlobeHeightmap();
  }

  // ---- scene interface ----
  draw(ctx) {
    if (globalThis.__moc_render3d) {
      if (this.globeLayer) {
        ctx.drawImage(this.globeLayer, 0, 0);
      }
      this.tribeVisible = true;
      return;
    }
    const sx = (this.sx = Math.round(this.camX)), sy = (this.sy = Math.round(this.camY));
    const w1 = Math.min(W, VW - sx);
    ctx.drawImage(this.built, sx, sy, w1, H, 0, 0, w1, H);
    if (w1 < W) ctx.drawImage(this.built, 0, sy, W - w1, H, w1, 0, W - w1, H);
    this.drawGlints(ctx, sx, sy);
    for (const list of this.walkers.values()) {
      for (const a of list) {
        const q = this.screenOf(a.x, a.y);
        if (q.x > W + 3 || q.y < -6 || q.y > H + 2) continue;
        drawPerson(ctx, q.x, q.y, a);
      }
    }
    this.tribeVisible = false;
    if (this.sim) {
      const offs = this.tileOffsets(sx, sy);
      if (offs.length) {
        const tctx = this.tribeCanvas.getContext('2d');
        tctx.clearRect(0, 0, W, H);
        if (this.sim.scene) {
          this.sim.scene.layer = blankLayer();
          if (this.sim.scene.G?.world) {
            this.sim.scene.G.world.drawWater = () => {};
            this.sim.scene.G.world.layers3d = null;
          }
          this.sim.scene.draw(tctx);
          for (const o of offs) ctx.drawImage(this.tribeCanvas, o.x, o.y);
        }
        this.tribeVisible = true;
      }
    }
    for (const ox of this.worldOffsets(sx)) { ctx.save(); ctx.translate(ox, -sy); this.particles.draw(ctx); ctx.restore(); }
  }

  draw3d(engine, _dt) {
    const tx0 = (this.v?.tile?.x ?? 0) / 2, ty0 = (this.v?.tile?.y ?? 90) / 2;

    // 1. Point lights for lightning bolts, fires, and settlements
    if (this.sim) {
      const sc = this.sim.scene;
      if (sc.fire?.lit) {
        engine.add_3d_light(tx0 + sc.fire.x / 2, 10.0, ty0 + sc.fire.y / 2, 20.0 + sc.fire.heat * 22.0 + Math.sin(this.t * 13) * 1.2, 0.9);
      }
      if (sc.bolts) {
        for (const b of sc.bolts) {
          const e = b.pts[b.pts.length - 1];
          engine.add_3d_light(tx0 + e.x / 2, 12.0, ty0 + e.y / 2, 50.0, 2.2);
        }
      }
    }
    for (const b of this.bolts) {
      const e = b.pts[b.pts.length - 1];
      engine.add_3d_light(e.x / 2, 12.0, e.y / 2, 50.0, 2.2);
    }
    for (const s of this.plan) {
      engine.add_3d_light(s.x / 2, 8.0, s.y / 2, 14.0 + s.size * 6.0, 1.3);
    }

    // 2. Upright 3D structures (SPRITE_HUT=5, SPRITE_HOUSE=6, SPRITE_TOWER=7, SPRITE_FACTORY=8, SPRITE_PAD=11)
    if (this.placedStructs) {
      for (const b of this.placedStructs) {
        const spriteId = b.kind === 'hut' ? 5 : b.kind === 'house' ? 6 : b.kind === 'tower' ? 7 : b.kind === 'factory' ? 8 : b.kind === 'pad' ? 11 : 5;
        engine.add_billboard(spriteId, b.x / 2, b.y / 2, 1, 0, 0);
      }
    }

    // 3. Walkers (villagers moving between settlements)
    for (const list of this.walkers.values()) {
      for (const a of list) {
        engine.add_billboard(0, a.x / 2, a.y / 2, a.facing || 1, a.frame || 0, 0);
      }
    }

    // 4. Tribe inside capital (emerging around fire once lit)
    if (this.sim) {
      const sc = this.sim.scene;
      if (sc.fire) {
        engine.add_billboard(4, tx0 + sc.fire.x / 2, ty0 + sc.fire.y / 2, 1, Math.floor(this.t * 6) % 3, sc.fire.lit ? 0 : 1);
      }
      if (sc.mammoths) {
        for (const m of sc.mammoths) {
          engine.add_billboard(1, tx0 + m.x / 2, ty0 + m.y / 2, m.facing || 1, m.frame || 0, m.dead ? 1 : 0);
        }
      }
      if (sc.people && sc.fire?.lit) {
        for (const p of sc.people) {
          if (!p.alive && (p.deadT || 0) > 12) continue;
          const pose = !p.alive ? 3 : p.state === 'dance' ? 1 : p.state === 'cower' ? 2 : 0;
          engine.add_billboard(0, tx0 + p.x / 2, ty0 + p.y / 2, p.facing || 1, p.frame || 0, pose);
        }
      }
      if (sc.chicken) {
        const cx = tx0 + sc.chicken.x / 2, cy = ty0 + sc.chicken.y / 2;
        engine.add_billboard(3, cx, cy, sc.chicken.facing || 1, sc.chicken.frame || 0, 0);
      }
    }
  }

  drawGlints(ctx, sx, sy) {
    const rng = mulberry32((Math.floor(this.t * 2) * 7919) ^ this.world.seed);
    ctx.fillStyle = 'rgba(170, 205, 240, 0.55)';
    for (let i = 0; i < 50; i++) {
      const x = Math.floor(rng() * W), y = Math.floor(rng() * H);
      if (typeAt(this.v, sx + x, sy + y) <= T.SHALLOW) ctx.fillRect(x, y, 2, 1);
    }
  }

  drawAfterLight(ctx) {
    const sx = this.sx, sy = this.sy;
    if (this.sim && this.tribeVisible) {
      for (const o of this.tileOffsets(sx, sy)) {
        ctx.save(); ctx.translate(o.x, o.y);
        for (const b of this.sim.scene.bolts) drawBolt(ctx, b.pts, Math.min(1, b.life * 4));
        ctx.restore();
      }
    }
    for (const ox of this.worldOffsets(sx)) {
      ctx.save(); ctx.translate(ox, -sy);
      for (const b of this.bolts) drawBolt(ctx, b.pts, Math.min(1, b.life * 4));
      ctx.restore();
    }
    // The aim reticle is drawn once, on top, by main.js (#301).
    const f = this.flash;
    if (f > 0) { ctx.fillStyle = `rgba(220,230,255,${f * 0.9})`; ctx.fillRect(0, 0, W, H); }
  }

  lights() {
    const L = [];
    const sx = this.sx, sy = this.sy;
    if (this.sim && this.tribeVisible) {
      const sc = this.sim.scene;
      for (const o of this.tileOffsets(sx, sy)) {
        for (const b of sc.bolts) { const e = b.pts[b.pts.length - 1]; L.push({ x: o.x + e.x, y: o.y + e.y, r: 45 }); }
        if (sc.fire.lit) L.push({ x: o.x + sc.fire.x, y: o.y + sc.fire.y - 2, r: 16 + sc.fire.heat * 30 + Math.sin(this.t * 13) * 1.5 });
      }
    }
    for (const b of this.bolts) { const e = b.pts[b.pts.length - 1]; const q = this.screenOf(e.x, e.y); L.push({ x: q.x, y: q.y, r: 45 }); }
    const towns = [];
    for (const s of this.plan) {
      const q = this.screenOf(s.x, s.y);
      if (q.x > W + 20 && q.x < VW - 20) continue;
      if (q.y < -20 || q.y > H + 20) continue;
      towns.push({ x: q.x > W + 20 ? q.x - VW : q.x, y: q.y - 2, r: 12 + s.size * 7 + (s.capital ? 4 : 0) });
    }
    towns.sort((a, b) => b.r - a.r);
    return L.concat(towns).slice(0, MAX_LIGHTS);
  }

  drawOverlay(ctx) {
    const is3d = Boolean(ctx.__project3d || globalThis.__moc_render3d);
    if (is3d) {
      if (ctx.__project3d) {
        // 1. Settlements: project 3D terrain coordinates (s.x / 2, s.y / 2) to screen
        for (const s of this.plan) {
          if (s.size < 1 && !s.capital) continue;
          const pr = ctx.__project3d(s.x / 2, s.y / 2);
          if (!pr) continue;
          const px = pr[0];
          const py = pr[1] - 8 - s.size * 2;
          if (px > W - 4 || px < 4 || py < 12 || py > H) continue;
          const name = s.capital ? `capital ${TIERS[s.size].name.toLowerCase()}` : TIERS[s.size].name.toLowerCase();
          tag(ctx, px, py, s.rebel ? `${name} · rebels` : name, s.rebel ? '#ff8a9c' : '#f2e8d5', 4.5, 'center');
        }
        // 2. Planet surface floating text particles
        for (const pt of this.particles.list) {
          if (pt.shape !== 'text') continue;
          const pr = ctx.__project3d(pt.x / 2, pt.y / 2);
          if (!pr) continue;
          const t = clamp(pt.life / pt.maxLife);
          ctx.globalAlpha = t;
          ctx.font = `${pt.size}px "Pixelify Sans", monospace`;
          ctx.textAlign = 'center';
          ctx.fillStyle = 'rgba(10,10,20,0.85)';
          ctx.fillText(pt.text, pr[0] + 0.5, pr[1] + 0.5);
          ctx.fillStyle = pt.color;
          ctx.fillText(pt.text, pr[0], pr[1]);
          ctx.globalAlpha = 1;
        }
        // 3. Capital tribe floating text particles
        if (this.sim && this.tribeVisible) {
          const tx0 = (this.v?.tile?.x ?? 0) / 2, ty0 = (this.v?.tile?.y ?? 90) / 2;
          const pts = this.sim.scene?.particles?.list || [];
          for (const pt of pts) {
            if (pt.shape !== 'text') continue;
            const pr = ctx.__project3d(tx0 + pt.x / 2, ty0 + pt.y / 2);
            if (!pr) continue;
            const t = clamp(pt.life / pt.maxLife);
            ctx.globalAlpha = t;
            ctx.font = `${pt.size}px "Pixelify Sans", monospace`;
            ctx.textAlign = 'center';
            ctx.fillStyle = 'rgba(10,10,20,0.85)';
            ctx.fillText(pt.text, pr[0] + 0.5, pr[1] + 0.5);
            ctx.fillStyle = pt.color;
            ctx.fillText(pt.text, pr[0], pr[1]);
            ctx.globalAlpha = 1;
          }
        }
      }
      this.fireballs.draw(ctx, { project: (x, y) => [x, y] });
      return;
    }

    const sx = this.sx, sy = this.sy;
    this.fireballs.draw(ctx, { project: (x, y) => [x, y] });
    if (this.sim && this.tribeVisible) {
      for (const o of this.tileOffsets(sx, sy)) {
        ctx.save();
        ctx.translate(o.x, o.y);
        this.sim.scene.drawOverlay(ctx);
        ctx.restore();
      }
    }
    for (const s of this.plan) {
      if (s.size < 1 && !s.capital) continue;
      const q = this.screenOf(s.x, s.y);
      if (q.x > W - 4 || q.x < 4 || q.y < 18 || q.y > H) continue;
      const name = s.capital ? `capital ${TIERS[s.size].name.toLowerCase()}` : TIERS[s.size].name.toLowerCase();
      tag(ctx, q.x, q.y - 12 - s.size * 2, s.rebel ? `${name} · rebels` : name, s.rebel ? '#ff8a9c' : '#f2e8d5', 4.5, 'center');
    }
    for (const ox of this.worldOffsets(sx)) {
      ctx.save();
      ctx.translate(ox, -sy);
      this.particles.drawText(ctx);
      ctx.restore();
    }
  }

  drawScreenOverlay(ctx) {
    const p = this.p;
    const god = this.sim?.god;
    tag(ctx, 4, 7, `${p.name} · ${this.sim ? 'AI god' : 'unsettled'}`, '#ffcf4a', 6, 'left');
    const strife = p.conflict ? ` · ${CONFLICT[p.conflict].name.toLowerCase()}` : p.factions ? ' · two factions' : '';
    tag(ctx, 4, 13, `${god ? god.status() : 'waiting for settlers'} · ${tierName(p).toLowerCase()}${strife}`, '#a3a6c4', 4.5, 'left');
    tag(ctx, W / 2, H - 4, 'right-drag or A/D · W/S to explore · scroll or Z to leave', '#a3a6c4', 4.5, 'center');
  }

  objectives() {
    const p = this.p;
    const out = [
      { label: 'Tier', value: tierName(p), plain: true },
      { label: 'People', value: fmtNum(p.pop), plain: true },
      { label: 'Food', value: fmtNum(p.food), bad: p.hunger > 0.2, plain: p.hunger <= 0.2 },
      { label: 'Wood · stone', value: `${fmtNum(p.wood)} · ${fmtNum(p.stone)}`, plain: true },
      { label: 'Goats', value: fmtNum(p.goats), plain: true },
      { label: 'Stability', value: `${Math.round(p.stability * 100)}%`, bad: p.stability < 0.3, done: p.stability >= 0.6 },
    ];
    if (p.conflict) out.push({ label: 'Strife', value: CONFLICT[p.conflict].name, bad: true });
    return out;
  }

  // The player's blink (or the fist cast) at a screen aim.
  onBlink(aim) {
    const w = globalThis.__moc_render3d ? { x: aim.x * VW, y: aim.y * VH } : this.screenToWorld(aim);
    const t = this.toTile(w.x, w.y);
    const scene = this.sim?.scene;
    if (t && scene && !scene.done) {
      const before = scene.strikes;
      this.sim.humanBlink(t);              // the tribe's own lightning: fire pit, mammoths, people
      if (scene.strikes !== before) this.strikes++;
      return;
    }
    if (this.t - this.lastBolt < 0.5) return;
    this.lastBolt = this.t;
    this.strikes++;
    const pts = makeBolt(aim.x * (W - 1), aim.y * (H - 1)).map((q) => ({ x: q.x + this.camX, y: q.y + this.camY }));
    this.bolts.push({ pts, life: 0.35 });
    this.ownFlash = 0.3;
    this.particles.burst(12, () => ({ x: w.x, y: w.y, vx: rand(-30, 30), vy: rand(-40, 5), g: 60, life: 0.6, maxLife: 0.6, size: 1, color: pick(['#fff4c2', '#ffd166', '#ffffff']) }));
    const hit = this.plan.find((s) => Math.hypot(wrapDx(s.x - w.x), s.y - w.y) < 12);
    if (hit) floater(this.particles, w.x, w.y - 8, `lightning on the ${TIERS[hit.size].name.toLowerCase()}!`, '#ffe066', 6);
  }

  // Charged fireball (#291): launch a visible slam, then apply the impact when it lands (#315).
  onFireball(aim, opts = {}) {
    const w = this.screenToWorld(aim);
    const t = this.toTile(w.x, w.y);
    const color = opts.color || '#ff9a2e';
    const charge = opts.charge ?? 1;
    this.fireballs.launch(
      { x: aim.x * (W - 1), y: aim.y * (H - 1) },
      { color, charge, meta: { world: w, tile: t } },
    );
    const scene = this.sim?.scene;
    if (t && scene?.onFireball) scene.onFireball(t, opts);
  }

  impactFireball({ world, color = '#ff9a2e', charge = 1 }) {
    const w = world || { x: W / 2, y: H / 2 };
    const r = castPlanet(this.p, 'fireball');
    this.particles.burst(28 + Math.round(charge * 26), () => ({ x: w.x + rand(-22, 22), y: w.y + rand(-12, 12), vx: rand(-6, 6), vy: rand(-20, -3), life: rand(0.45, 0.95), maxLife: 0.95, size: 1, color }));
    this.particles.burst(10 + Math.round(charge * 8), () => ({ x: w.x + rand(-10, 10), y: w.y + rand(-2, 3), vx: rand(-8, 8), vy: rand(-12, -2), g: 20, life: rand(0.5, 0.9), maxLife: 0.9, size: 1, color: pick(['#c9c2b0', '#8a7a5a', '#5f5545']) }));
    if (r) floater(this.particles, w.x, w.y - 10, r.text, r.good ? '#9be38a' : '#ff8a9c', 7);
    this.ownFlash = Math.max(this.ownFlash, 0.38);
  }

  // A camera cast at a screen aim: the whole world feels it; inside the capital the tribe does too.
  onCast(spell, aim) {
    if (spell === 'point') return;
    const w = this.screenToWorld(aim);
    const t = this.toTile(w.x, w.y);
    const r = castPlanet(this.p, spell);
    const scene = this.sim?.scene;
    if (t && scene && !scene.done) scene.onCast?.(spell, t);
    if (r) floater(this.particles, w.x, w.y - 10, r.text, r.good ? '#9be38a' : '#ff8a9c', 7);
    if (spell === 'rain' || spell === 'disaster') {
      this.particles.burst(60, () => ({ x: w.x + rand(-60, 60), y: this.camY + rand(-10, H * 0.7), vx: -4, vy: rand(70, 100), life: rand(0.5, 1), maxLife: 1, size: 1, color: 'rgba(158,201,255,0.75)' }));
    }
    if (spell === 'disaster') {
      this.ownFlash = 0.5;
      for (let i = 0; i < 3; i++) {
        const pts = makeBolt(clamp(aim.x * (W - 1) + rand(-16, 16), 2, W - 3), clamp(aim.y * (H - 1) + rand(-8, 8), 4, H - 3)).map((q) => ({ x: q.x + this.camX, y: q.y + this.camY }));
        this.bolts.push({ pts, life: 0.35 + i * 0.12 });
      }
    }
    if (spell === 'bless' || spell === 'peace') {
      this.particles.burst(20, () => ({ x: w.x + rand(-20, 20), y: w.y + rand(-12, 12), vx: rand(-3, 3), vy: rand(-14, -4), life: 1.2, maxLife: 1.2, size: 1, color: spell === 'peace' ? '#9be38a' : pick(['#ffe066', '#fff4c2']) }));
    }
  }

  onMood(mood) { this.sim?.scene?.onMood?.(mood); }
}

// Crisp text in low-res units.
function tag(ctx, x, y, text, color, size, align = 'left') {
  ctx.font = `${size}px "Pixelify Sans", monospace`;
  ctx.textAlign = align;
  ctx.fillStyle = 'rgba(8,10,24,0.85)';
  ctx.fillText(text, x + 0.6, y + 0.6);
  ctx.fillStyle = color;
  ctx.fillText(text, x, y);
}
