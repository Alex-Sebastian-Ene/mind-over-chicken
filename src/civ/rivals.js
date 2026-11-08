// Rival civilisations on your home world (#212, epic #220).
//
// The rivals are moc-civ peoples (crates/moc-civ/src/homeworld.rs, exports in home_ffi.rs) living
// on the home world's other continents, on the same map the eras play on: their terrain is built
// from the home World's tiles and heights. Your tribe is a mirrored faction inside their sim, so
// they can meet you, trade, raid, go to war and race you to space; the era scene stays the truth
// for your own people. What they do to you arrives as lines (ticker text, an advisor line, food,
// people hurt, a chicken-market shock) that applyRivalLine() turns into the scene's own numbers.
//
// Like civ.js, the module is plain `extern "C"` WebAssembly: views on memory.buffer are made
// fresh on every read, because any call may grow (and so replace) the buffer.
import { loadCivModule, TONE } from './civ.js';
import { drawHut, drawHouse, drawTower, drawPerson, drawBoat } from '../render/sprites.js';

// homeworld::he, the kinds of line.
export const RIVAL = { CONTACT: 1, TRADE: 2, RAID: 3, SACK: 4, WAR: 5, PEACE: 6, ALLY: 7, ARRIVE: 8, LANDING: 9, WORLD: 10, LAUNCH: 11, CLASH: 12 };
// The scenes that are time skips (FarmingToKingdoms, InfoAge): the rivals run fast there.
const SKIP_SCENES = new Set([1, 3]);
// The rival sim steps at 5 Hz; people are drawn between the last two steps.
export const STEP = 0.2;
const AGENT_FIELDS = 5, ROW_FIELDS = 16, MAX_FACTIONS = 24;

export async function createRivals(world, { seed = world.seed, council = 'ppo', rivals = 0, offset = 0 } = {}) {
  const module = await loadCivModule();
  const instance = await WebAssembly.instantiate(module, {});
  return new Rivals(instance.exports, world, { seed, council, rivals, offset });
}

const rgb = (r, g, b) => `rgb(${r | 0},${g | 0},${b | 0})`;
const wrap = (v, n) => ((v % n) + n) % n;
const wrapDelta = (a, b, n) => { let d = b - a; if (d > n / 2) d -= n; else if (d < -n / 2) d += n; return d; };

export class Rivals {
  constructor(ex, world, { seed = world.seed, council = 'ppo', rivals = 0, offset = 0 } = {}) {
    this.ex = ex;
    this.W = world.width;
    this.H = world.height;
    const n = this.W * this.H;
    const tiles = world.baseType || world.type;
    let p = ex.civ_home_buf(0, n);
    new Uint8Array(ex.memory.buffer, p, n).set(tiles.subarray ? tiles.subarray(0, n) : tiles.slice(0, n));
    p = ex.civ_home_buf(1, n);
    new Float32Array(ex.memory.buffer, p, n).set(world.h.subarray ? world.h.subarray(0, n) : Float32Array.from(world.h).subarray(0, n));
    const cs = world.continents || [];
    p = ex.civ_home_buf(2, cs.length * 7);
    const cv = new Float32Array(ex.memory.buffer, p, cs.length * 7);
    cs.forEach((c, k) => cv.set([c.cx, c.cy, c.rx, c.ry, c.village.x, c.village.y, c.area || 0], k * 7));
    this.h = ex.civ_home_new(seed >>> 0, this.W, this.H, cs.length, rivals, offset | 0);
    if (!this.h) throw new Error('rivals: the home world could not be built');
    this.ppo = ex.civ_home_ppo(this.h, council === 'rules' ? 0 : 1);
    this.mirror = ex.civ_home_mirror(this.h);
    this.total = ex.civ_home_rivals_total(this.h);
    this.seq = ex.civ_home_line_next(this.h);   // lines from founding are not news
    this.acc = 0;
    this.scene = -1;
    this.villages = new Float32Array(0);
    this.agents = new Float32Array(0);
    this.boats = new Float32Array(0);
    this.rows = new Float32Array(0);
    this.prev = new Map();   // agent id → [x, y] at the step before
    this.log = [];
    this.hover = null;
    this.names = new Map();
    this.perf = { samples: [], avg: 0, p95: 0 };
    this.territoryT = 0;
    this.snapshot();
  }

  free() {
    if (this.h) this.ex.civ_home_free(this.h);
    this.h = 0;
  }

  // The scene in play (0 Stone Age … 4 Space Age) sets the rivals' age range and pace.
  setScene(k) {
    if (!this.h || k === this.scene) return;
    this.scene = k;
    const ex = this.ex;
    ex.civ_home_set_stage(this.h, ex.civ_home_scene_ages(k, 0), ex.civ_home_scene_ages(k, 1), SKIP_SCENES.has(k) ? 1 : 0);
    ex.civ_home_catch_up(this.h);
  }

  setDifficulty(d) {
    if (this.h && d) this.ex.civ_home_set_difficulty(this.h, d.aggression ?? 1, d.raids ?? 1, d.damage ?? 1, d.growth ?? 1);
  }

  // Your people this step, from the era scene (see playerState).
  setPlayer(st) {
    const ex = this.ex;
    let p = ex.civ_home_buf(3, st.sites.length * 2);
    const sv = new Float32Array(ex.memory.buffer, p, st.sites.length * 2);
    st.sites.forEach((s, k) => { sv[k * 2] = s.x; sv[k * 2 + 1] = s.y; });
    p = ex.civ_home_buf(4, st.boats.length * 2);
    const bv = new Float32Array(ex.memory.buffer, p, st.boats.length * 2);
    st.boats.forEach((b, k) => { bv[k * 2] = b.x; bv[k * 2 + 1] = b.y; });
    ex.civ_home_set_player(this.h, st.sites.length, st.boats.length, st.pop | 0, st.food, st.launch, st.launched ? 1 : 0);
  }

  // Step the rivals; returns the new lines (see applyRivalLine).
  update(dt, st) {
    if (!this.h) return [];
    this.acc += dt;
    if (this.acc < STEP) return [];
    const real = this.acc;
    this.acc = 0;
    this.setPlayer(st);
    const t0 = performance.now();
    this.ex.civ_home_advance(this.h, real * this.ex.civ_home_speed(this.h));
    this.sample(performance.now() - t0);
    this.snapshot();
    return this.readLines();
  }

  sample(ms) {
    const s = this.perf.samples;
    s.push(ms);
    if (s.length > 300) s.shift();
    const sorted = [...s].sort((a, b) => a - b);
    this.perf.avg = s.reduce((a, b) => a + b, 0) / s.length;
    this.perf.p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))];
    this.perf.max = sorted[sorted.length - 1];
  }

  snapshot() {
    const ex = this.ex, h = this.h;
    // Where each person was, so the draw can move them smoothly between steps.
    this.prev.clear();
    const a = this.agents;
    for (let i = 0; i + AGENT_FIELDS <= a.length; i += this.agentStride || AGENT_FIELDS) this.prev.set(a[i + 3], [a[i], a[i + 1]]);
    const read = (ptr, count, stride) => (count && ptr ? new Float32Array(ex.memory.buffer, ptr, count * stride).slice() : new Float32Array(0));
    this.villageStride = ex.civ_home_village_stride();
    this.agentStride = ex.civ_home_agent_stride();
    this.boatStride = ex.civ_home_boat_stride();
    this.rowStride = ex.civ_home_rival_stride();
    this.villages = read(ex.civ_home_villages_ptr(h), ex.civ_home_village_count(h), this.villageStride);
    this.agents = read(ex.civ_home_agents_ptr(h), ex.civ_home_agent_count(h), this.agentStride);
    this.boats = read(ex.civ_home_boats_ptr(h), ex.civ_home_boat_count(h), this.boatStride);
    // One row per faction slot (world::MAX_FACTIONS of them).
    this.rows = read(ex.civ_home_rivals_ptr(h), MAX_FACTIONS, this.rowStride);
  }

  row(f) {
    const s = this.rowStride || ROW_FIELDS, o = f * s;
    if (o + s > this.rows.length) return null;
    const r = this.rows;
    return { state: r[o], pop: r[o + 1], villages: r[o + 2], age: r[o + 3], color: [r[o + 4], r[o + 5], r[o + 6]], stance: r[o + 7], tension: r[o + 8], war: r[o + 9], met: r[o + 10], capital: { x: r[o + 11], y: r[o + 12] }, launch: r[o + 13], land: r[o + 14], parent: r[o + 15] };
  }

  colour(f) {
    const r = this.row(f);
    return r ? rgb(...r.color) : '#c9c2b0';
  }

  text(len) {
    if (!len) return '';
    return new TextDecoder().decode(new Uint8Array(this.ex.memory.buffer, this.ex.civ_home_text_ptr(this.h), len).slice());
  }
  name(f) {
    if (!this.names.has(f)) this.names.set(f, this.text(this.ex.civ_home_name(this.h, f)));
    return this.names.get(f);
  }
  label(f) { return this.text(this.ex.civ_home_label(this.h, f)); }

  lineStr(seq, k) {
    const ex = this.ex, len = ex.civ_home_line_str_len(this.h, seq, k);
    if (!len) return '';
    return new TextDecoder().decode(new Uint8Array(ex.memory.buffer, ex.civ_home_line_str_ptr(this.h, seq, k), len).slice());
  }

  readLines() {
    const ex = this.ex, h = this.h;
    const next = ex.civ_home_line_next(h);
    const out = [];
    for (let seq = Math.max(this.seq, ex.civ_home_line_first(h)); seq < next; seq++) {
      const m = (k) => ex.civ_home_line_meta(h, seq, k);
      if (m(0) <= -1e8) continue;
      const line = {
        kind: m(0), tone: m(1), rival: m(2), x: m(3), y: m(4), food: m(5), hurt: m(6), market: m(7), n: m(8), priority: m(9), cooldown: m(10),
        text: this.lineStr(seq, 0), say: this.lineStr(seq, 1), key: this.lineStr(seq, 2),
      };
      out.push(line);
      if (line.kind !== RIVAL.WORLD || line.n === 2) {
        this.log.unshift({ text: line.text, tone: line.tone });
        if (this.log.length > 6) this.log.length = 6;
      }
    }
    this.seq = next;
    return out;
  }

  alive() { return this.h ? this.ex.civ_home_rivals_alive(this.h) : 0; }

  // The rivals' territory as an RGBA image `w`×`h`, one pixel per `step` tiles.
  territory(step) {
    const ex = this.ex;
    const w = ex.civ_home_territory(this.h, step);
    if (!w) return null;
    const len = ex.civ_home_territory_len(this.h);
    const h = Math.floor(len / 4 / w);
    return { w, h, data: new Uint8ClampedArray(ex.memory.buffer, ex.civ_home_territory_ptr(this.h), w * h * 4).slice() };
  }

  // The village under a world point (hover), or null.
  villageAt(x, y) {
    const v = this.villages, s = this.villageStride;
    let best = null, bestD = Infinity;
    for (let i = 0; i + s <= v.length; i += s) {
      const d = Math.hypot(wrapDelta(x, v[i], this.W), wrapDelta(y, v[i + 1], this.H));
      if (d < Math.max(8, v[i + 4] + 3) && d < bestD) { bestD = d; best = i; }
    }
    return best == null ? null : { x: v[best], y: v[best + 1], owner: v[best + 2] };
  }

  // People between the last two steps, wrap-aware: [x, y, faction, kind] each.
  people() {
    const a = this.agents, s = this.agentStride, k = Math.min(1, this.acc / STEP);
    const out = [];
    for (let i = 0; i + s <= a.length; i += s) {
      const p = this.prev.get(a[i + 3]);
      let x = a[i], y = a[i + 1];
      if (p) { x = wrap(p[0] + wrapDelta(p[0], x, this.W) * k, this.W); y = wrap(p[1] + wrapDelta(p[1], y, this.H) * k, this.H); }
      out.push([x, y, a[i + 2], a[i + 4], a[i + 3]]);
    }
    return out;
  }

  // Paint villages, fields, boats and (in 2D) people into the world canvas, at every wrap copy
  // that crosses an edge. flatPeople false in 3D: people stand up as billboards (draw3d).
  draw(ctx, { flatPeople = true, t = 0 } = {}) {
    if (!this.h) return;
    const W = this.W, H = this.H;
    const copies = (x, y, r, fn) => {
      fn(x, y);
      const ex = x < r ? W : x > W - r ? -W : 0, ey = y < r ? H : y > H - r ? -H : 0;
      if (ex) fn(x + ex, y);
      if (ey) fn(x, y + ey);
      if (ex && ey) fn(x + ex, y + ey);
    };
    const v = this.villages, s = this.villageStride;
    for (let i = 0; i + s <= v.length; i += s) {
      const [x, y, owner, pop, radius, capital, flags, age] = [v[i], v[i + 1], v[i + 2], v[i + 3], v[i + 4], v[i + 5], v[i + 6], v[i + 7]];
      if (owner === this.mirror) continue;
      const col = this.colour(owner);
      copies(x, y, radius + 10, (cx, cy) => drawVillage(ctx, cx, cy, { col, pop, radius, capital, flags, age, t }));
    }
    const b = this.boats, bs = this.boatStride;
    for (let i = 0; i + bs <= b.length; i += bs) {
      const col = this.colour(b[i + 2]);
      copies(b[i], b[i + 1], 8, (cx, cy) => drawBoat(ctx, cx, cy, { facing: b[i + 4] < 0 ? -1 : 1, sail: col }));
    }
    if (flatPeople) {
      for (const [x, y, f, kind, id] of this.people()) {
        if (kind === 2) continue;   // at sea: the boat shows them
        const col = this.colour(f);
        copies(x, y, 4, (cx, cy) => drawPerson(ctx, cx, cy, { shirt: col, hair: '#2a1d14', frame: ((t * 6) + id) | 0, pose: kind === 1 ? 'panic' : 'walk' }));
      }
    }
    // Names over the capitals, and the hovered village's full label.
    ctx.font = '5px "Pixelify Sans", monospace';
    ctx.textAlign = 'center';
    for (let i = 0; i + s <= v.length; i += s) {
      if (!v[i + 5] || v[i + 2] === this.mirror) continue;
      const f = v[i + 2];
      copies(v[i], v[i + 1], 30, (cx, cy) => textOn(ctx, cx, cy - v[i + 4] - 6, this.name(f), this.colour(f)));
    }
    if (this.hover) {
      const hv = this.villageAt(this.hover.x, this.hover.y);
      if (hv && hv.owner !== this.mirror) copies(hv.x, hv.y, 60, (cx, cy) => textOn(ctx, cx, cy + 9, this.label(hv.owner), '#f2e8d5'));
    }
    ctx.textAlign = 'left';
  }

  // In 3D the people stand up as billboards near the camera (at most `max`).
  draw3d(engine, { cx, cy, range = 110, max = 70, t = 0 } = {}) {
    if (!this.h || !engine?.add_billboard) return;
    let n = 0;
    for (const [x, y, , kind, id] of this.people()) {
      if (kind === 2) continue;
      const dx = wrapDelta(cx, x, this.W), dy = wrapDelta(cy, y, this.H);
      if (Math.abs(dx) > range || Math.abs(dy) > range * 0.7) continue;
      engine.add_billboard(0, cx + dx, cy + dy, (id & 1) ? 1 : -1, ((t * 6) + id) | 0, 0);
      if (++n >= max) break;
    }
  }
}

function textOn(ctx, x, y, text, col) {
  if (!text) return;
  ctx.fillStyle = 'rgba(8,10,24,0.55)';
  const w = ctx.measureText(text).width;
  ctx.fillRect(Math.round(x - w / 2 - 1), Math.round(y - 5), Math.ceil(w + 2), 6);
  ctx.fillStyle = col;
  ctx.fillText(text, Math.round(x), Math.round(y));
}

// A rival village: fields round it, huts → houses → towers by age, walls, the capital's banner.
function drawVillage(ctx, x, y, o) {
  x = Math.round(x); y = Math.round(y);
  const r = Math.max(4, Math.min(14, o.radius || 6));
  if (o.flags & 1) {
    ctx.fillStyle = 'rgba(196,170,82,0.55)';
    for (let k = 0; k < 4; k++) {
      const a = k * 1.7 + 0.4;
      ctx.fillRect(Math.round(x + Math.cos(a) * (r + 3)) - 2, Math.round(y + Math.sin(a) * (r * 0.6 + 2)) - 1, 5, 3);
    }
  }
  if (o.flags & 2) {
    ctx.fillStyle = '#8a8a8a';
    for (let k = 0; k < 16; k++) {
      const a = (k / 16) * Math.PI * 2;
      ctx.fillRect(Math.round(x + Math.cos(a) * (r + 1)), Math.round(y + Math.sin(a) * (r * 0.6 + 1)), 1, 1);
    }
  }
  const n = Math.max(1, Math.min(8, Math.round(o.pop / 3)));
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2 + 0.3;
    const hx = Math.round(x + Math.cos(a) * r * 0.7), hy = Math.round(y + Math.sin(a) * r * 0.45);
    if (o.age >= 13 && k % 3 === 0) drawTower(ctx, hx, hy, 6 + (k % 3) * 2);
    else if (o.age >= 4) drawHouse(ctx, hx, hy, o.col);
    else drawHut(ctx, hx, hy);
  }
  if (o.capital) {
    ctx.fillStyle = '#4a3a2a';
    ctx.fillRect(x, y - 9, 1, 9);
    ctx.fillStyle = o.col;
    ctx.fillRect(x + 1, y - 9, 4, 3);
  }
  if (o.flags & 8) {   // under siege
    ctx.fillStyle = (o.t * 4) % 1 < 0.5 ? 'rgba(232,69,44,0.8)' : 'rgba(255,154,46,0.6)';
    ctx.fillRect(x - 1, y - 3, 3, 2);
  }
  if (o.flags & 64) {  // just raided: smoke
    ctx.fillStyle = 'rgba(90,90,100,0.6)';
    ctx.fillRect(x - 1, y - 12 - Math.round((o.t * 4) % 4), 2, 2);
  }
}

// ---- the era scene → the rivals, and the rivals' lines → the era scene (pure; tested) ----

// Your people for the rivals' sim: settlements (camp first), boats, people, larder, launch.
// k is the scene (0 Stone Age … 4 Space Age).
export function playerState(scene, world, k) {
  const sites = [];
  const camp = scene?.fire && k === 0 ? scene.fire : world?.sites?.village;
  if (camp) sites.push({ x: camp.x, y: camp.y });
  if (k === 0) {
    for (const c of Object.values(scene?.colonies || {})) if (c && Number.isFinite(c.x)) sites.push({ x: c.x, y: c.y });
  } else {
    // From the farming age on your people live on every continent (terrain.js draws them there).
    for (const c of (world?.continents || []).slice(1)) sites.push({ x: c.village.x, y: c.village.y });
  }
  const boats = (scene?.boats || []).filter((b) => Number.isFinite(b?.x)).map((b) => ({ x: b.x, y: b.y }));
  const pop = typeof scene?.living === 'function' ? scene.living().length : (scene?.people?.length ?? 30);
  const food = Number.isFinite(scene?.food) ? scene.food : 4;
  const launch = k === 4 ? Math.max(0, Math.min(1, scene?.progress ?? 0)) : 0;
  const launched = k === 4 && (scene?.phase === 'launch' || scene?.phase === 'voyage');
  return { sites, boats, pop, food, launch, launched };
}

const MOOD = { [TONE.bad]: 'panic', [TONE.warn]: 'nervous', [TONE.good]: 'happy', [TONE.info]: 'nervous' };

// What a rival line does to the scene: food, people hurt, the chicken market; and the words.
// Returns what was applied (for tests and the log).
export function applyRivalLine(line, { scene, advisor, hud } = {}) {
  const done = { food: 0, hurt: 0, market: 0, said: false };
  if (line.food && Number.isFinite(scene?.food)) {
    const before = scene.food;
    scene.food = Math.max(0, scene.food + line.food);
    done.food = scene.food - before;
  }
  if (line.hurt > 0 && typeof scene?.living === 'function' && typeof scene.kill === 'function') {
    // The raid hits the people nearest where it landed, never the last few of the tribe.
    const alive = scene.living().filter((p) => !p.colony);
    const spare = Math.max(0, alive.length - 3);
    const victims = alive
      .map((p) => ({ p, d: Math.hypot((p.x ?? 0) - line.x, (p.y ?? 0) - line.y) }))
      .sort((a, b) => a.d - b.d)
      .slice(0, Math.min(line.hurt, spare));
    for (const { p } of victims) scene.kill(p);
    done.hurt = victims.length;
  }
  if (line.market && Number.isFinite(scene?.price)) {
    scene.price = Math.max(2, scene.price * (1 + line.market));
    if (Array.isArray(scene.headlines) && line.text) scene.headlines.push(line.text.toUpperCase());
    done.market = line.market;
  }
  if (line.say && advisor?.say) {
    advisor.say(line.key || `rival-${line.kind}`, line.say, { mood: MOOD[line.tone] || 'nervous', priority: line.priority || 2, cooldown: line.cooldown || 12 });
    done.said = true;
  }
  if (line.text && hud?.showCause && line.kind !== RIVAL.WORLD) {
    hud.showCause(line.text, { ms: 3600, tone: line.tone === TONE.bad ? 'bad' : line.tone === TONE.warn ? 'warn' : 'ok' });
  }
  return done;
}
