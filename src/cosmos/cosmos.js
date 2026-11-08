// Zoom controller: 0 = a world's surface, 1 = the planet as a spinning pixel globe,
// 2 = the galaxy. The game keeps simulating underneath at every level, and made planets
// keep running at every level too.
//
// planets (#68): every world can be landed on. The home world's surface is the hand-played
// campaign; any other world's surface is its PlanetSurface (surface.js), shown and fed the
// player's inputs through viewScene()/castG()/hudScene(). The level logic itself is the pure
// ZoomState in zoom.js (tested in Node); this class adds drawing and pointer input.

import { W, H, clamp, lerp } from '../sim/util.js';
import { Globe, drawStars } from './globe.js';
import { Galaxy, ERA_LABEL, ERA_NAMES, CHANNELS, fmtPop, drawPlanetBolt } from './galaxy.js';
import { label } from '../eras/stone.js';
import { ZoomState } from './zoom.js';
import { tierName } from './planet-sim.js';
import { drawReticle } from '../render/sprites.js';
import { sideNeighbour } from './select.js';

const CENTER = { x: W / 2, y: H / 2 };
const BIG_R = 74;
const SUN = [-0.55, -0.42, 0.72];
const ERA_INDEX = { stone: 0, farming: 1, kingdoms: 1, industry: 2, space: 3 };

const ease = (t) => (t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2);
const smoothstep = (a, b, t) => { const x = clamp((t - a) / (b - a)); return x * x * (3 - 2 * x); };
const wrapAngle = (a) => Math.atan2(Math.sin(a), Math.cos(a));

// Continuous zoom input (#143): levels per wheel delta unit, and per e-fold of pinch spread.
const WHEEL_LEVELS = 0.004;   // a mouse notch is 0.4 of a level (Alex: easier zoom)
const PINCH_LEVELS = 3;

export class Cosmos extends ZoomState {
  constructor() {
    super();               // z, target, focus, sandbox (zoom.js)
    this.globe = new Globe();
    this.galaxy = null;
    this.rot = 0;          // globe spin at level 1
    this.spinVel = 0.18;
    this.drag = null;
    this.pointers = new Map();
    this.pinch = null;
    this.hover = null;
    this.time = 0;
    this.flat = Object.assign(document.createElement('canvas'), { width: W, height: H });
    this.toast = null;
    this.wheelAcc = 0;
    this.wheelAt = 0;
    this.lastRun = null;
    this.onEvent = null;
    // The home world is bigger than the screen (#129): a Rust ViewCamera (main.js hands it
    // over with setViewCamera) is the 320×180 window onto it, panning and wrapping both ways.
    this.view = null;
    this.panVel = 0;       // east–west pan speed (A/D)
    this.panVelY = 0;      // north–south pan speed (W/S)
    // A scene with its own camera (the civ sandbox) takes the pan instead: return true if handled.
    this.panSink = null;
    this.homeHuds = new WeakMap();
    this.castProxies = new WeakMap();
  }

  // ZoomState asks for the galaxy's home planet.
  homePlanet() { return this.galaxy?.home || null; }

  // The other world whose surface is on screen (or zooming), if any.
  get surfaceInView() { return this.aiSurfaceInView ? this.world()?.surface || null : null; }

  reset() {
    this.galaxy = null; this.z = 0; this.target = 0; this.focus = null; this.hover = null; this.sandbox = false; this.drag = null; this.lastRun = null; this.panVel = 0; this.panVelY = 0;
  }

  // ---- the home world's camera (#129) ----
  setViewCamera(cam) { this.view = cam; }
  // A new home world: size the camera to it and look at the village.
  attachWorld(world) {
    if (!this.view || !world) return;
    this.view.set_world(world.width || W, world.height || H);
    const v = world.sites?.village;
    if (v) this.view.set_center(v.x, v.y);
  }
  get worldW() { return this.view ? this.view.world_w() : W; }
  get worldH() { return this.view ? this.view.world_h() : H; }
  // Screen (view px) → home world px.
  toWorld(sx, sy) {
    if (!this.view || this.surfaceInView) return { x: sx, y: sy };
    return { x: this.view.to_world_x(sx), y: this.view.to_world_y(sy) };
  }
  // Point the home camera at a map spot (an era starts looking at its own stage).
  lookAt(x, y) { this.view?.set_center(x, y); }
  // The world point in the middle of the screen (scenes put screen-wide effects around it).
  viewCenter() { return this.view ? { x: this.view.center_x(), y: this.view.center_y() } : { x: W / 2, y: H / 2 }; }
  // Home world px → screen (view px) of the copy nearest the middle of the screen.
  toScreen(wx, wy) {
    if (!this.view || this.surfaceInView) return { x: wx, y: wy };
    return { x: this.view.to_screen_x(wx), y: this.view.to_screen_y(wy) };
  }

  ensureGalaxy(G, { autonomousHome = false } = {}) {
    if (this.galaxy) return this.galaxy;
    this.galaxy = new Galaxy();
    // A sandbox home runs on its own, on a mirror of the same world (built off-thread), so the paused game's map is untouched.
    this.galaxy.addHome(G.world, { autonomous: autonomousHome });
    // Three more worlds take shape from the player's first ten seconds of brain data.
    this.galaxy.addMindWorlds();
    this.galaxy.onEvent = (p, text, kind, tone) => this.onEvent?.(p, text, kind, tone);
    // The space war presses your world as hard as the difficulty says (#220: Easy 0.5, Hard 1.6).
    this.galaxy.war?.setPlayerPressure(G.difficulty?.space?.pressure ?? 1);
    return this.galaxy;
  }

  zoomTo(level, G) {
    if (Math.round(level) >= 2) this.ensureGalaxy(G);
    const fromSurface = this.target === 0;
    const r = super.zoomTo(level);
    if (r.refused) this.say(r.refused);
    if (r.level === 1 && fromSurface) this.rot = this.faceRot();
    return r;
  }
  zoomOut(G) { this.zoomTo(this.target + 1, G); }
  zoomIn(G) { this.zoomTo(this.target - 1, G); }

  say(text) { this.toast = { text, t: 2.6 }; }

  // The globe at level 1 faces whatever map column is in the middle of the (panned) surface.
  faceRot() { return Globe.faceRot(this.centerColumn()); }
  centerColumn() {
    const s = this.surfaceKind() === 'ai' ? this.world()?.surface : null;
    if (s) return s.centerColumn();
    return this.view ? (this.view.center_x() / this.worldW) * W : W / 2;
  }

  // ---- planetary pan of the flat surface ----
  // Home: the camera pans both ways over the bigger world and wraps (a torus, #111, #129).
  // Other worlds: their vast surface pans itself.
  pan(dx, dy = 0) {
    if (this.panSink?.(dx, dy)) return;
    const s = this.surfaceInView;
    if (s) { s.pan(dx, dy); return; }
    this.view?.pan(dx, dy);
  }
  // Screen x (view px) → map x. Other worlds' surfaces take screen aims.
  mapX(screenX) {
    if (this.surfaceInView || !this.view) return screenX;
    return this.view.to_world_x(screenX);
  }
  // Paint the camera's window of a world-sized canvas into the low-res view canvas, cut at the
  // seams so the map wraps both ways.
  blitView(worldCanvas, low) {
    const lctx = low.getContext('2d');
    lctx.imageSmoothingEnabled = false;
    if (!this.view) { lctx.drawImage(worldCanvas, 0, 0); return; }
    const r = this.view.blits();
    for (let i = 0; i < r.length; i += 6) lctx.drawImage(worldCanvas, r[i], r[i + 1], r[i + 2], r[i + 3], r[i + 4], r[i + 5], r[i + 2], r[i + 3]);
  }
  // The same, for a window `scale` times the view's size shrunk onto the view (a zoomed-out frame:
  // the era intro eases in from one).
  blitViewScaled(worldCanvas, low, scale) {
    if (!this.view?.blits_scaled) { this.blitView(worldCanvas, low); return; }
    const lctx = low.getContext('2d');
    lctx.imageSmoothingEnabled = false;
    const r = this.view.blits_scaled(scale);
    for (let i = 0; i < r.length; i += 8) lctx.drawImage(worldCanvas, r[i], r[i + 1], r[i + 2], r[i + 3], r[i + 4], r[i + 5], r[i + 6], r[i + 7]);
  }
  // [ox, oy] translations (world px → view px) at which to repeat world-space overlays and lights
  // so they wrap with the map: one per copy of the world reaching the screen (+margin).
  overlayOffsets(margin = 24) {
    if (this.surfaceInView || !this.view) return [[0, 0]];
    const c = this.view.copies(margin), out = [];
    for (let i = 0; i < c.length; i += 2) out.push([c[i], c[i + 1]]);
    return out;
  }

  // What the surface level shows and takes input: the home campaign's scene, or another world's surface.
  viewScene(G) { return (this.z < 1 && this.surfaceInView) || G?.scene || null; }

  // The game state the camera's casts and mood should reach (camera.js calls getG()): the real
  // G, but with `scene` pointing at the other world's surface while you're on it.
  castG(G) {
    if (!G || !this.surfaceInView || this.z > 0.5) return G;
    let px = this.castProxies.get(G);
    if (!px) {
      px = new Proxy(G, {
        get: (t, k) => (k === 'scene' && this.surfaceInView && this.z <= 0.5 ? this.surfaceInView : Reflect.get(t, k)),
        set: (t, k, v) => Reflect.set(t, k, v),
      });
      this.castProxies.set(G, px);
    }
    return px;
  }

  // What the HUD shows: another world you're on (AI god), the galaxy, or home (your world).
  hudScene(G, galaxyHud) {
    const w = this.world();
    if (this.z < 1.5 && w?.surface && this.surfaceKind() === 'ai') return w.surface;
    if (this.galaxy && (G.sandbox || this.inGalaxy)) return galaxyHud;
    const scene = G.scene;
    if (!scene) return galaxyHud;
    let h = this.homeHuds.get(scene);
    if (!h || h.base !== scene.label) {
      h = { base: scene.label, label: `${scene.label} · your world`, era: scene.era, powers: scene.powers, objectives: () => scene.objectives() };
      this.homeHuds.set(scene, h);
    }
    return h;
  }

  homeInfo(G) {
    const s = G.scene;
    if (!s) return null;
    // Scenes with people count them. The time-skip montages (Farming → Kingdoms, the Information
    // Age) have none: they report null so the galaxy keeps the count instead of "barren" (#105).
    const pop = s.living ? s.living().length : s.agents ? s.agents.length : s.citizens ? s.citizens.length + (s.engineers?.length || 0) + (s.passengers || 0) : s.people ? s.people.length : null;
    return { eraIndex: ERA_INDEX[s.era] ?? 0, pop, canLaunch: !!(G.finished || s.phase === 'voyage' || s.phase === 'launch') };
  }

  update(dt, G) {
    this.time += dt;
    super.update(dt);
    if (!this.drag) { this.rot += this.spinVel * dt; this.spinVel = lerp(this.spinVel, 0.18, 1 - Math.exp(-dt * 0.8)); }
    if (this.z <= 0.001 && this.target === 0) this.rot = this.faceRot();
    if (Math.abs(this.panVel) > 0.5) { this.pan(this.panVel * dt); this.panVel *= Math.exp(-dt * 6); } else this.panVel = 0;
    if (Math.abs(this.panVelY) > 0.5) { this.pan(0, this.panVelY * dt); this.panVelY *= Math.exp(-dt * 6); } else this.panVelY = 0;
    // The world you're looking at hears every channel at full strength and takes your blinks;
    // the one whose surface is on screen runs its tribe every frame.
    if (this.galaxy) {
      this.galaxy.attended = this.attended();
      this.galaxy.viewing = this.aiSurfaceInView ? this.world() : null;
    }
    const playDt = this.lastRun == null ? 0 : Math.max(0, G.runTime - this.lastRun);
    this.lastRun = G.runTime;
    if (this.galaxy && G.brain) this.galaxy.update(dt, G.brain, this.homeInfo(G), playDt, { anger: G.mood?.active ? G.mood.anger || 0 : 0 });
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
  render(low, ctx, G, { is3d = false } = {}) {
    if (this.z <= 0.001) return false;
    const fctx = this.flat.getContext('2d');
    fctx.clearRect(0, 0, W, H);
    fctx.drawImage(low, 0, 0);
    drawStars(ctx, this.time, this.time * 2);
    const look = this.focusLook(G);
    const z = this.z;
    if (z <= 1) {
      // #138: one continuous move. In 2D, the flat map shrinks and curls onto a sphere; in 3D,
      // the true 3D surface mesh is rendered underneath and blends directly with the planetary globe.
      const bigR = lerp(BIG_R * 1.25, BIG_R, smoothstep(0.35, 1, z));
      const fa = 1 - smoothstep(0.42, 0.66, z);
      if (!is3d && fa > 0) this.drawCurledMap(ctx, smoothstep(0, 0.6, z), fa, bigR, look.atmosphere);
      const face = this.faceRot();
      const rot = face + wrapAngle(this.rot - face) * smoothstep(0.55, 1, z);
      const globeAlpha = is3d ? smoothstep(0.12, 0.52, z) : smoothstep(0.4, 0.62, z);
      if (look.tex) this.globe.draw(ctx, look.tex, CENTER.x, CENTER.y, bigR, { rot, alpha: globeAlpha, light: SUN, atmosphere: look.atmosphere });
      const lp = look.planet;
      if (lp && lp.flash > 0) drawPlanetBolt(ctx, CENTER.x, CENTER.y, bigR, lp.storms + 1, Math.min(1, lp.flash * 2) * smoothstep(0.5, 0.9, z));
      this.drawMoon(ctx, z);
    } else {
      const t = z - 1, e = ease(t);
      const gal = this.galaxy;
      const fp = look.planet || gal.home;
      gal.draw(ctx, this.globe, G.scene?.layer || G.world.layers.stone, G.world.structures, { alpha: smoothstep(0.15, 0.9, t), skip: fp, hover: this.hover || this.selectedPlanet() });
      const q = gal.pos(fp);
      const toStar = [160 - q.x, 92 - q.y, 26];
      const n = Math.hypot(...toStar);
      const L = SUN.map((v, i) => lerp(v, toStar[i] / n, e));
      const rot = lerp(this.rot, gal.time * fp.spin + fp.phase, e);
      gal.drawPlanet(ctx, this.globe, fp, lerp(CENTER.x, q.x, e), lerp(CENTER.y, q.y, e), lerp(BIG_R, fp.size, e), G.scene?.layer || G.world.layers.stone, G.world.structures, 1, { rot, light: L, tilt: lerp(0.32, 0.3, e) });
    }
    return true;
  }

  // The flat surface wrapping onto a sphere (#138). As `curl` goes 0 -> 1 the view's columns
  // bend to longitudes and its rows to latitudes (up to ~77 degrees each way), its outline
  // narrows from the full screen to a circle of radius `R` (the globe's size at the hand-over)
  // and sun-side shading fades in, so the real globe can cross-fade in on top without a jump.
  // Two passes of strip copies (columns, then rows) keep it on the GPU path: a per-pixel
  // version needed a pixel read-back of the frame, which cost ~12 ms.
  drawCurledMap(ctx, curl, alpha, R, atmosphere = [110, 175, 255]) {
    if (!this.curlBuf) {
      const mk = () => { const cv = Object.assign(document.createElement('canvas'), { width: W, height: H }); const cx = cv.getContext('2d'); cx.imageSmoothingEnabled = false; return { cv, cx }; };
      this.curlBuf = { a: mk(), b: mk() };
    }
    const { a, b } = this.curlBuf;
    const EDGE = 1.35;                                  // latitude/longitude the map's edges reach
    const ang = EDGE * curl, sA = Math.sin(ang), bent = ang > 1e-3;
    const warp = (n) => (bent ? Math.asin(Math.max(-1, Math.min(1, n * sA))) / ang : n); // screen -> map, -1..1
    const rim = R * Math.sin(EDGE);
    const hw = lerp(W / 2, rim, curl), hh = lerp(H / 2, rim, curl);
    // Pass 1: columns. Destination column x (as -1..1 across the width) samples map column warp(x).
    a.cx.clearRect(0, 0, W, H);
    const CS = 2;
    for (let x = 0; x < W; x += CS) {
      const u0 = (warp((x / W) * 2 - 1) + 1) * 0.5 * W;
      const u1 = (warp(((x + CS) / W) * 2 - 1) + 1) * 0.5 * W;
      a.cx.drawImage(this.flat, u0, 0, Math.max(0.5, u1 - u0), H, x, 0, CS, H);
    }
    // Pass 2: rows. Destination row y (as -1..1 down the outline) samples row warp(y) of pass 1,
    // squeezed to that latitude's width.
    b.cx.clearRect(0, 0, W, H);
    const RS = 2;
    const top = Math.max(0, Math.floor(CENTER.y - hh)), bot = Math.min(H, Math.ceil(CENTER.y + hh));
    for (let y = top; y < bot; y += RS) {
      const n0 = (y - CENTER.y) / hh, n1 = (y + RS - CENTER.y) / hh;
      const nm = Math.max(-1, Math.min(1, (n0 + n1) / 2));
      const v0 = (warp(Math.max(-1, n0)) + 1) * 0.5 * H, v1 = (warp(Math.min(1, n1)) + 1) * 0.5 * H;
      const half = hw * Math.sqrt(Math.max(0, 1 - (nm * sA) ** 2)); // hw * cos(latitude)
      if (half < 0.5 || v1 - v0 <= 0) continue;
      b.cx.drawImage(a.cv, 0, v0, W, v1 - v0, Math.round(CENTER.x - half), y, Math.round(half * 2), RS);
    }
    // Sun-side shading: a radial falloff from the lit side, only where the map is drawn.
    const shade = 0.78 * curl;
    if (shade > 0.01) {
      const Ln = Math.hypot(SUN[0], SUN[1], SUN[2]);
      const gx = CENTER.x + (SUN[0] / Ln) * hw * 0.75, gy = CENTER.y + (SUN[1] / Ln) * hh * 0.75;
      const g = b.cx.createRadialGradient(gx, gy, 0, gx, gy, Math.max(hw, hh) * 2.1);
      g.addColorStop(0, `rgba(255,250,230,${0.12 * curl})`);
      g.addColorStop(0.35, 'rgba(0,0,0,0)');
      g.addColorStop(0.75, `rgba(2,4,18,${shade * 0.75})`);
      g.addColorStop(1, `rgba(2,4,18,${shade})`);
      b.cx.globalCompositeOperation = 'source-atop';
      b.cx.fillStyle = g;
      b.cx.fillRect(0, 0, W, H);
      b.cx.globalCompositeOperation = 'source-over';
    }
    ctx.globalAlpha = alpha;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(b.cv, 0, 0);
    // A thin atmosphere rim hugging the outline as it becomes round.
    const glow = smoothstep(0.3, 0.9, curl) * alpha;
    if (glow > 0.01) {
      ctx.strokeStyle = `rgb(${atmosphere[0]},${atmosphere[1]},${atmosphere[2]})`;
      ctx.lineWidth = 1;
      ctx.globalAlpha = glow * 0.7;
      ctx.beginPath(); ctx.ellipse(CENTER.x, CENTER.y, hw + 1, hh + 1, 0, 0, Math.PI * 2); ctx.stroke();
      ctx.globalAlpha = glow * 0.3;
      ctx.beginPath(); ctx.ellipse(CENTER.x, CENTER.y, hw + 2.5, hh + 2.5, 0, 0, Math.PI * 2); ctx.stroke();
    }
    ctx.globalAlpha = 1;
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
      const campaign = this.isCampaign(p) && !this.sandbox;
      const w = this.world();
      const name = campaign ? 'Home' : w?.name || 'Home';
      const era = !campaign && w ? (w.pop > 0 ? `${tierName(w)} · ${ERA_LABEL[ERA_NAMES[w.eraIndex]]}` : 'barren') : ERA_LABEL[ERA_NAMES[info?.eraIndex ?? 0]];
      const pop = !campaign && w ? fmtPop(w.pop) : `${this.galaxy?.home?.pop ?? info?.pop ?? 0} people`;
      ctx.globalAlpha = clamp(a);
      label(ctx, CENTER.x, 14, `${name} · ${era} · ${pop}`, '#f2e8d5', 7);
      const ch = !campaign && w ? CHANNELS[w.channel] : null;
      const lp = p || this.galaxy?.home;
      label(ctx, CENTER.x, 22, campaign ? 'your world · blink to strike it from orbit' : `AI god · you're here: every channel acts on it · ${w?.effect || ''}`, ch && w.channel !== 'mixed' ? ch.color : '#a3a6c4', 5);
      if (lp) lp.labels.forEach((l, i) => { ctx.globalAlpha = clamp(a) * Math.min(1, l.t); label(ctx, (CENTER.x - BIG_R) / 2, CENTER.y - 24 + i * 9, l.text, l.color, 7); ctx.globalAlpha = clamp(a); });
      label(ctx, CENTER.x, H - 6, this.canLand() ? 'drag to spin · scroll or X to land · Z for the galaxy' : 'drag to spin · scroll or Z to see the galaxy', '#a3a6c4', 5);
      ctx.globalAlpha = 1;
    }
    if (this.galaxy && z > 1.2) this.galaxy.drawOverlay(ctx, { alpha: smoothstep(1.3, 1.9, z), hover: this.hover || this.selectedPlanet() });
    if (this.galaxy && z > 1.6) {
      const sp = this.selectedPlanet();
      label(ctx, W / 2, H - 6, `◀ ▶ choose a world${sp ? ` (${sp.name})` : ''} · scroll in or Enter to land`, '#a3a6c4', 5);
    }
    if (this.toast) { ctx.globalAlpha = Math.min(1, this.toast.t); label(ctx, CENTER.x, 30, this.toast.text, '#ffd166', 6); ctx.globalAlpha = 1; }
    // The aim reticle (screen pointer here) is drawn once, on top, by main.js (#301).
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
      // Continuous (#143): spreading the fingers zooms in smoothly, about a level per 1.4x spread.
      if (d > 0 && this.pinch.d > 0) this.nudgeZoom(-Math.log(d / this.pinch.d) * PINCH_LEVELS, G);
      this.pinch.d = d;
      return;
    }
    if (this.drag) {
      const dx = x - this.drag.x;
      this.drag.moved += Math.abs(dx) + Math.abs(y - this.drag.y);
      this.drag.x = x; this.drag.y = y;
      this.dragRotate(dx);
    }
    if (this.galaxy && this.z > 1.5) this.hover = this.galaxy.hit(x, y);
  }

  pointerUp(id, x, y, G) {
    this.pointers.delete(id);
    if (this.pointers.size < 2) this.pinch = null;
    const d = this.drag;
    this.drag = null;
    if (!d || d.moved > 4) return null;
    return this.clickAt(x, y, G);
  }

  // A left click in the world (#284): select a planet in the galaxy or land on the globe.
  clickAt(x, y, G) {
    if (this.galaxy && this.z > 1.5) {
      const p = this.galaxy.hit(x, y);
      if (p && p.forming) { this.say(p.mindSlot && !this.galaxy.mindReady ? 'Still reading your mind. Keep playing.' : 'Still forming. Give it a second.'); return null; }
      if (p) { this.focusPlanet(p, G); return p; }
    } else if (this.z > 0.5 && this.z <= 1.5) {
      if (Math.hypot(x - CENTER.x, y - CENTER.y) <= BIG_R && this.canLand()) {
        this.zoomIn(G);
        return this.world();
      }
    }
    return null;
  }

  // Pointer drag and pinch drag share this so a rotated planet follows the hand the same way (#284).
  dragRotate(dxPixels) {
    if (!(this.z > 0.5 && this.z < 1.5)) return false;
    this.rot -= dxPixels / BIG_R;
    this.spinVel = -dxPixels / BIG_R / 0.033;
    return true;
  }

  // The galaxy's picker (Alex): Left/Right select the next planet to that side on screen; the
  // selected one is highlighted and is the world a zoom-in (wheel, X, pinch, Enter) lands on.
  selectSide(dir, G) {
    const gal = this.galaxy;
    if (!gal || !this.inGalaxy) return null;
    const items = gal.planets.map((p) => ({ p, x: gal.pos(p).x }));
    const p = sideNeighbour(items, this.selectedPlanet(), dir);
    if (!p) return null;
    this.selected = p;
    this.focus = this.isCampaign(p) ? null : p;
    return p;
  }
  // The planet the galaxy view is "hovering": the last one picked, else the one in focus, else home.
  selectedPlanet() { return this.selected || this.focus || this.galaxy?.home || null; }

  focusPlanet(p, G) {
    this.selected = p;
    const r = super.focusPlanet(p);
    if (r.refused) { this.say(r.refused); return; }
    this.rot = this.galaxy ? this.galaxy.time * p.spin + p.phase : this.rot;
    if (this.galaxy?.war && !this.isCampaign(p)) this.galaxy.war.target = p;   // #166: the orders' target
  }

  // Continuous (#143): every wheel delta moves the zoom (a mouse notch of 100 is 0.4 of a level;
  // a trackpad's small deltas glide), with no 320 ms gate. The two-hand pinch comes through here too.
  wheel(deltaY, G) {
    this.nudgeZoom(clamp(deltaY, -300, 300) * WHEEL_LEVELS, G);
  }

  nudgeZoom(dz, G) {
    if (this.z + dz >= 1.5) this.ensureGalaxy(G);
    const fromSurface = this.target === 0;
    const r = this.nudge(dz);
    if (!r) return;
    if (r.refused) this.say(r.refused);
    if (r.level === 1 && fromSurface) this.rot = this.faceRot();
  }

  levelLabel() { return this.label(); }

  // G (#43): land on the next world's surface (AI worlds in turn, then home again).
  visitNextWorld(G) {
    const gal = this.ensureGalaxy(G);
    const next = this.nextWorld(gal.planets);
    if (!next) { this.say('No other world to visit yet. Make a planet.'); return null; }
    this.focusPlanet(next, G);
    this.zoomTo(0, G);
    return next;
  }

  // The world the galaxy panel's details describe: the one hovered, else the one in view.
  detailPlanet() {
    if (!this.galaxy) return null;
    return this.hover || this.galaxy.attended || this.focus || this.galaxy.home;
  }
}
