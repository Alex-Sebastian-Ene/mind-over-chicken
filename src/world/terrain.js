// One procedurally generated world that the civilisation lives on for the whole game. The land
// comes from Rust (crates/moc-terrain via the engine's RustWorldGen, #109): continents, ranges,
// rivers, biomes, the base colour layer, the campaign sites and the harbours. This class is a
// thin wrapper: it keeps the API the eras and pathfinding use (h, type, baseType, continents,
// sites, field, walkable, the era repaint) and is size-agnostic (world.width × world.height;
// the game's W × H by default, #129).

import { W, H, lerp, mulberry32, dist, wrap, wrapDelta } from '../sim/util.js';
import { drawHouse, drawHut, drawFactory, drawExchange, drawTower, drawHangar, drawPad } from '../render/sprites.js';
import * as wasm from '../../pkg/mind_over_chicken_engine.js';

export const T = { DEEP: 0, SHALLOW: 1, SAND: 2, GRASS: 3, FOREST: 4, HILL: 5, MOUNTAIN: 6, SNOW: 7, FARM: 8, ROAD: 9, PAVED: 10 };

const TEMPERATE = {
  [T.DEEP]: [[22, 44, 92], [25, 49, 99]],
  [T.SHALLOW]: [[40, 86, 146], [45, 94, 155]],
  [T.SAND]: [[214, 196, 136], [205, 186, 126]],
  [T.GRASS]: [[96, 156, 62], [88, 147, 57], [104, 163, 68]],
  [T.FOREST]: [[46, 98, 46], [58, 116, 52], [38, 84, 40]],
  [T.HILL]: [[124, 132, 74], [114, 122, 68]],
  [T.MOUNTAIN]: [[118, 110, 100], [104, 97, 89], [132, 124, 114]],
  [T.SNOW]: [[232, 232, 228], [216, 218, 222]],
  [T.FARM]: [[196, 170, 74], [120, 158, 60]],
  [T.ROAD]: [[150, 128, 98], [142, 121, 92]],
  [T.PAVED]: [[146, 146, 152], [136, 136, 144]],
  trees: ['#2c5a2a', '#6aa84f', '#4f8d3e'],
  foam: [120, 170, 205],
};
// Other planets reuse the same land rules with a different palette.
export const BIOMES = {
  temperate: TEMPERATE,
  desert: { ...TEMPERATE,
    [T.DEEP]: [[24, 70, 96], [27, 76, 102]], [T.SHALLOW]: [[44, 122, 138], [50, 130, 144]],
    [T.SAND]: [[232, 206, 140], [224, 198, 132]], [T.GRASS]: [[210, 172, 102], [200, 162, 94], [218, 180, 110]],
    [T.FOREST]: [[120, 150, 70], [104, 136, 60], [136, 160, 80]], [T.HILL]: [[184, 120, 72], [172, 110, 66]],
    [T.MOUNTAIN]: [[140, 78, 52], [126, 70, 46], [152, 88, 60]], [T.SNOW]: [[240, 222, 190], [230, 212, 180]],
    trees: ['#5e6f2c', '#9fbf4f', '#7d9a3a'], foam: [140, 200, 200] },
  ice: { ...TEMPERATE,
    [T.DEEP]: [[16, 30, 64], [18, 34, 70]], [T.SHALLOW]: [[52, 90, 130], [58, 98, 140]],
    [T.SAND]: [[176, 186, 196], [168, 178, 188]], [T.GRASS]: [[214, 228, 236], [204, 220, 230], [222, 234, 240]],
    [T.FOREST]: [[40, 84, 86], [48, 96, 98], [34, 74, 78]], [T.HILL]: [[170, 190, 206], [160, 180, 198]],
    [T.MOUNTAIN]: [[110, 124, 146], [98, 112, 134], [122, 136, 158]], [T.SNOW]: [[248, 250, 255], [236, 242, 250]],
    trees: ['#1f4a4c', '#5f9a96', '#3c7472'], foam: [190, 215, 235] },
  jungle: { ...TEMPERATE,
    [T.DEEP]: [[16, 70, 90], [18, 76, 96]], [T.SHALLOW]: [[34, 140, 150], [40, 150, 158]],
    [T.GRASS]: [[52, 140, 60], [46, 130, 54], [58, 150, 66]], [T.FOREST]: [[24, 84, 36], [30, 96, 42], [20, 72, 30]],
    [T.HILL]: [[78, 124, 58], [70, 114, 52]], trees: ['#174d22', '#4fa64a', '#2f7f36'], foam: [130, 210, 210] },
  alien: { ...TEMPERATE,
    [T.DEEP]: [[18, 60, 56], [20, 66, 60]], [T.SHALLOW]: [[36, 120, 104], [42, 130, 112]],
    [T.SAND]: [[210, 190, 226], [200, 180, 218]], [T.GRASS]: [[140, 92, 176], [128, 84, 164], [152, 102, 188]],
    [T.FOREST]: [[90, 44, 120], [104, 52, 136], [78, 38, 106]], [T.HILL]: [[110, 120, 170], [100, 110, 160]],
    [T.MOUNTAIN]: [[70, 58, 96], [62, 52, 88], [80, 66, 108]], [T.SNOW]: [[214, 255, 236], [200, 246, 226]],
    [T.FARM]: [[230, 140, 210], [150, 100, 190]], trees: ['#3e1d5c', '#d07ad8', '#9a52b8'], foam: [150, 230, 200] },
};

// The engine wasm must be initialised before a World is made: main.js does it at boot, the
// worldgen worker on start, and Node (tests, the RL env) with initTerrainSync(bytes).
let RustWorldGenClass = null;
export function setRustWorldGen(cls) {
  RustWorldGenClass = cls;
}
export function initTerrainSync(bytes) {
  wasm.initSync({ module: bytes });
}
export const initTerrain = () => wasm.default();
const Gen = () => RustWorldGenClass || wasm.RustWorldGen;
// The learned relief (#110) for worlds made by this wasm instance: the worldgen worker turns it
// on, the main thread never does. False (procedural relief, unchanged) without trained weights.
export function useLearnedRelief() {
  try { return Gen().use_learned_relief?.(true) ?? false; } catch { return false; }
}

export class World {
  // opts: { biome, width, height } (the game's W × H by default).
  constructor(seed = (Math.random() * 1e9) | 0, opts = {}) {
    this.seed = seed >>> 0;
    this.biome = opts.biome && BIOMES[opts.biome] ? opts.biome : 'temperate';
    this.palette = BIOMES[this.biome];
    this.layers = {};
    if (opts.gen === false) return;   // filled by World.planet / rehydration
    const g = Gen().home(this.seed, opts.width || W, opts.height || H, this.biome);
    this.fill(g);
    g.free();
  }

  // A galaxy planet: its 640×360 surface (world.vast) and, as the World, the 320×180 capital the
  // AI tribe lives on, cut from it at vast.tile (#68, #109).
  static planet(seed, biome = 'temperate') {
    const w = new World(seed, { biome, gen: false });
    const g = Gen().planet(w.seed, w.biome);
    w.fill(g);
    const s = JSON.parse(g.surface_json());
    w.vast = { w: g.surface_width(), h: g.surface_height(), tile: s.tile, type: g.surface_types(), height: g.surface_heights(), continents: s.continents, spots: s.spots, rgba: g.surface_rgba(), stats: JSON.parse(g.surface_stats_json()) };
    g.free();
    return w;
  }

  fill(g) {
    this.width = g.width();
    this.height = g.height();
    this.w = this.width;
    this.h = g.heights();
    this.type = g.types();
    this.baseType = this.type.slice();
    this.rgba = g.rgba();
    this.owner = g.owner();
    this.sites = JSON.parse(g.sites_json());
    this.continents = JSON.parse(g.continents_json());
    this.stats = { ...JSON.parse(g.stats_json()), ms: g.ms() };
  }

  // ---- the map is a torus (#111) of its own size (640×360 at home, #129) ----
  // Index of map pixel (x, y), wrapping both ways.
  idx(x, y) {
    const w = this.width, h = this.height;
    x = Math.round(x) % w; if (x < 0) x += w;
    y = Math.round(y) % h; if (y < 0) y += h;
    return y * w + x;
  }
  wrapX(x) { return wrap(x, this.width); }
  wrapY(y) { return wrap(y, this.height); }
  dx(a, b) { return wrapDelta(a, b, this.width); }
  dy(a, b) { return wrapDelta(a, b, this.height); }
  // Distance the short way round.
  dist(ax, ay, bx, by) { return Math.hypot(this.dx(ax, bx), this.dy(ay, by)); }
  // Aims (the brain's, the mouse's, the AI's) are 0..1 of the world: the map pixel under one,
  // and the aim at a map pixel.
  aimPoint(aim) { return { x: this.wrapX(Math.round(aim.x * this.width)), y: this.wrapY(Math.round(aim.y * this.height)) }; }
  aimOf(x, y) { return { x: this.wrapX(x) / this.width, y: this.wrapY(y) / this.height }; }

  get(x, y) { return this.type[this.idx(x, y)]; }
  walkable(x, y) { const t = this.get(x, y); return t >= T.SAND && t !== T.MOUNTAIN && t !== T.SNOW; }
  isWater(x, y) { return this.get(x, y) <= T.SHALLOW; }

  nearestWalkable(x, y, maxR = 60) {
    x = this.wrapX(Math.round(x)); y = this.wrapY(Math.round(y));
    if (this.walkable(x, y)) return { x, y };
    let fallback = null;
    for (let r = 1; r <= maxR; r++) {
      for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
        const px = this.wrapX(x + dx), py = this.wrapY(y + dy);
        if (this.walkable(px, py)) {
          if (this.get(px, py) !== T.FOREST) return { x: px, y: py };
          fallback ??= { x: px, y: py };
        }
      }
    }
    return fallback || (this.sites?.village ? { ...this.sites.village } : { x, y });
  }

  // Random walkable point in a ring around (x,y).
  randomLand(x, y, rMin, rMax, pred) {
    for (let i = 0; i < 80; i++) {
      const a = Math.random() * Math.PI * 2, r = rMin + Math.random() * (rMax - rMin);
      const px = this.wrapX(Math.round(x + Math.cos(a) * r)), py = this.wrapY(Math.round(y + Math.sin(a) * r));
      if (this.walkable(px, py) && (!pred || pred(px, py))) return { x: px, y: py };
    }
    const near = this.nearestWalkable(x, y);
    if (!pred || pred(near.x, near.y)) return near;
    if (this.sites?.village && (!pred || pred(this.sites.village.x, this.sites.village.y))) {
      return { ...this.sites.village };
    }
    return near;
  }

  // A walkable point in a loose cluster around (x, y): a free angle, a radius biased towards
  // the centre, and a little space from the structures already placed. Co-ordinates wrap.
  clusterSpot(x, y, spreadX, spreadY, pred, placed, rng = Math.random, innerX = 5, innerY = 3) {
    for (let i = 0; i < 30; i++) {
      const a = rng() * Math.PI * 2, r = Math.pow(rng(), 1.35);
      const px = this.wrapX(Math.round(x + Math.cos(a) * (innerX + r * (spreadX - innerX))));
      const py = this.wrapY(Math.round(y + Math.sin(a) * (innerY + r * (spreadY - innerY))));
      if (!this.walkable(px, py) || (pred && !pred(px, py))) continue;
      if (placed && placed.some((o) => Math.abs(this.dx(px, o.x)) < 3 && Math.abs(this.dy(py, o.y)) < 3)) continue;
      return { x: px, y: py };
    }
    return null;
  }

  // Era-specific land use. Called once per era before painting.
  shape(era) {
    this.type = this.baseType.slice();
    this.fields = new Map();
    this.structures = [];
    const v = this.sites.village, s = this.sites;
    const rng = mulberry32(this.seed + 17);
    const setT = (x, y, t) => { this.type[this.idx(x, y)] = t; };
    // Roads take the short way round the torus.
    const road = (a, b, w = 1) => {
      const ex = a.x + this.dx(b.x, a.x), ey = a.y + this.dy(b.y, a.y);
      const n = Math.max(1, Math.ceil(Math.hypot(ex - a.x, ey - a.y)));
      for (let i = 0; i <= n; i++) {
        const x = Math.round(lerp(a.x, ex, i / n)), y = Math.round(lerp(a.y, ey, i / n));
        for (let d = 0; d < w; d++) if (this.walkable(x, y + d)) setT(x, y + d, T.ROAD);
      }
    };
    const farms = (count, rMin, rMax) => {
      for (let i = 0; i < count; i++) {
        const c = this.randomLand(v.x, v.y, rMin, rMax, (x, y) => this.get(x, y) === T.GRASS);
        const fw = 6 + Math.floor(rng() * 6), fh = 4 + Math.floor(rng() * 4);
        for (let y = c.y; y < c.y + fh; y++) for (let x = c.x; x < c.x + fw; x++) if (this.get(x, y) === T.GRASS) setT(x, y, T.FARM);
      }
    };

    if (era === 'stone') {
      for (let i = 0; i < 4; i++) {
        const p = this.clusterSpot(v.x, v.y, 13, 8, null, this.structures, rng, 6, 4);
        if (p) this.structures.push({ kind: 'hut', ...p });
      }
    }
    if (era === 'farming') {
      farms(14, 18, 60);
      for (let i = 0; i < 10; i++) {
        const p = this.clusterSpot(v.x, v.y, 19, 12, null, this.structures, rng, 6, 4);
        if (p) this.structures.push({ kind: 'hut', ...p });
      }
    }
    // Settlements spread to the other continents from the farming age on.
    if (era !== 'stone') {
      for (const c of this.continents.slice(1)) {
        const n = era === 'farming' ? 5 : 8;
        for (let i = 0; i < n; i++) {
          const p = this.clusterSpot(c.village.x, c.village.y, 9, 6, null, this.structures, rng, 3, 2);
          if (!p) continue;
          if (era === 'farming') this.structures.push({ kind: 'hut', ...p });
          else if (era === 'space' && i % 3 === 0) this.structures.push({ kind: 'tower', ...p, h: 7 + (i % 4) });
          else this.structures.push({ kind: 'house', ...p, roof: era === 'space' ? '#5a6b8c' : '#8e3b2a' });
        }
        if (era !== 'farming' && c.harbor) road(c.village, c.harbor.land, 1);
      }
    }
    if (era === 'industry' || era === 'space') {
      farms(era === 'industry' ? 22 : 12, 34, 90);
      road(v, s.factory, 2); road(v, s.exchange, 1); road(v, s.pad, 1);
      road({ x: v.x - 30, y: v.y - 2 }, { x: v.x + 30, y: v.y - 2 });
      road({ x: v.x, y: v.y - 22 }, { x: v.x, y: v.y + 22 });
      for (let y = -2; y <= 2; y++) for (let x = -3; x <= 3; x++) setT(v.x + x, v.y + y, T.PAVED);
      const roofs = era === 'industry' ? ['#a4402f', '#8e3b2a', '#b5543a', '#6f4a3a'] : ['#5a6b8c', '#7f8fa8', '#a4402f', '#4d5d7a'];
      for (let i = 0; i < 48; i++) {
        const p = this.clusterSpot(v.x, v.y, 30, 20,
          (x, y) => dist(x, y, s.exchange.x, s.exchange.y) >= 9 && dist(x, y, s.factory.x, s.factory.y) >= 12,
          this.structures, rng, 9, 6);
        if (!p) continue;
        if (era === 'space' && i % 10 === 0) this.structures.push({ kind: 'tower', ...p, h: 8 + Math.floor(rng() * 7) });
        else this.structures.push({ kind: 'house', ...p, roof: roofs[i % roofs.length] });
      }
      this.structures.push({ kind: 'factory', x: s.factory.x, y: s.factory.y, smoke: era === 'industry' });
      this.structures.push({ kind: 'exchange', x: s.exchange.x, y: s.exchange.y });
    }
    if (era === 'space') {
      for (let y = -6; y <= 6; y++) for (let x = -8; x <= 8; x++) if (this.walkable(s.pad.x + x, s.pad.y + y)) setT(s.pad.x + x, s.pad.y + y, T.PAVED);
      this.structures.push({ kind: 'hangar', x: s.hangar.x, y: s.hangar.y });
      this.structures.push({ kind: 'pad', x: s.pad.x, y: s.pad.y });
    }
    this.structures.sort((a, b) => a.y - b.y);
  }

  // Bake terrain + static structures into pixel canvases (2D flat and 3D clean landscape). The
  // land's colours are the Rust base colour layer (#109: no per-tile random colour); Rust also
  // paints the era's land use (farms, roads, paving) over it.
  paint(era) {
    this.shape(era);
    this.layers3d ||= {};
    const W = this.width, H = this.height;

    // 1. Tree billboards for the 3D view, on forest tiles.
    this.trees = [];
    const trng = mulberry32(this.seed + 5);
    for (let y = 1; y < H; y += 2) for (let x = (y >> 1) % 2; x < W; x += 2) {
      if (this.type[y * W + x] !== T.FOREST || trng() > 0.7) continue;
      this.trees.push({ x, y, variant: trng() > 0.5 ? 1 : 0 });
    }

    // 2. Base landscape (also the 3D terrain texture: no flat structures).
    const c3d = document.createElement('canvas');
    c3d.width = W; c3d.height = H;
    const ctx3d = c3d.getContext('2d');
    const img = ctx3d.createImageData(W, H);
    img.data.set(wasm.paint_land_use(this.rgba, this.type, this.baseType, W, this.biome));
    ctx3d.putImageData(img, 0, 0);
    if (era === 'stone' || era === 'farming') {
      const m = this.sites.caveMouth;
      ctx3d.fillStyle = '#1a1714'; ctx3d.fillRect(m.x - 2, m.y - 1, 4, 3);
      ctx3d.fillStyle = '#2b2622'; ctx3d.fillRect(m.x - 3, m.y, 1, 2); ctx3d.fillRect(m.x + 2, m.y, 1, 2);
    }
    this.layers3d[era] = c3d;

    // 3. 2D / globe texture: the landscape with flat structures.
    const c2d = document.createElement('canvas');
    c2d.width = W; c2d.height = H;
    const ctx2d = c2d.getContext('2d');
    ctx2d.drawImage(c3d, 0, 0);
    for (const s of this.structures) {
      if (s.kind === 'hut') drawHut(ctx2d, s.x, s.y);
      if (s.kind === 'house') drawHouse(ctx2d, s.x, s.y, s.roof);
      if (s.kind === 'tower') drawTower(ctx2d, s.x, s.y, s.h);
      if (s.kind === 'factory') drawFactory(ctx2d, s.x, s.y);
      if (s.kind === 'exchange') drawExchange(ctx2d, s.x, s.y);
      if (s.kind === 'hangar') drawHangar(ctx2d, s.x, s.y);
      if (s.kind === 'pad') drawPad(ctx2d, s.x, s.y);
    }
    this.layers[era] = c2d;
    return c2d;
  }

  // Draw 3D trees and structures as upright billboards standing on the terrain. With a world
  // bigger than the view (#129), only those near the camera are sent: `near(x, y)`.
  draw3d(engine, near = null) {
    if (!engine) return;
    // 3D Trees across forest tiles
    if (this.trees) {
      for (const t of this.trees) {
        if (near && !near(t.x, t.y)) continue;
        engine.add_billboard(14, t.x, t.y, 1, 0, t.variant || 0);
      }
    }
    // 3D Structures
    if (this.structures) {
      for (const s of this.structures) {
        if (near && !near(s.x, s.y)) continue;
        switch (s.kind) {
          case 'hut':
            engine.add_billboard(5, s.x, s.y, 1, 0, 0);
            break;
          case 'house':
            engine.add_billboard(6, s.x, s.y, 1, 0, 0);
            break;
          case 'tower':
            engine.add_billboard(7, s.x, s.y, 1, 0, 0);
            break;
          case 'factory':
            engine.add_billboard(8, s.x, s.y, 1, 0, 0);
            break;
          case 'exchange':
            engine.add_billboard(9, s.x, s.y, 1, 0, 0);
            break;
          case 'hangar':
            engine.add_billboard(10, s.x, s.y, 1, 0, 0);
            break;
          case 'pad':
            engine.add_billboard(11, s.x, s.y, 1, 0, 0);
            break;
        }
      }
    }
  }

  typeAt(x, y) { return this.type[this.idx(x, y)]; }

  // Distance field (BFS over walkable tiles, in Rust) toward a target, cached on a 3 px grid so
  // moving targets like mammoths don't trigger a new search every frame.
  field(tx, ty) {
    const W = this.width, H = this.height;
    const qx = this.wrapX(Math.round(tx / 3) * 3), qy = this.wrapY(Math.round(ty / 3) * 3);
    const key = qy * W + qx;
    this.fields ||= new Map();
    let f = this.fields.get(key);
    if (f) { this.fields.delete(key); this.fields.set(key, f); return f; }
    const start = this.walkable(qx, qy) ? { x: qx, y: qy } : this.nearestWalkable(qx, qy, 12);
    f = wasm.walk_field(this.type, W, H, start.x, start.y, false);
    this.fields.set(key, f);
    if (this.fields.size > 64) this.fields.delete(this.fields.keys().next().value);
    return f;
  }

  // Same idea over water, for boats.
  seaField(tx, ty) {
    const W = this.width;
    const qx = this.wrapX(Math.round(tx)), qy = this.wrapY(Math.round(ty));
    const key = -(qy * W + qx) - 1;
    this.fields ||= new Map();
    let f = this.fields.get(key);
    if (f) return f;
    f = wasm.walk_field(this.type, W, this.height, qx, qy, true);
    f[qy * W + qx] = 0;
    this.fields.set(key, f);
    return f;
  }

  // Can you walk from the village to (x, y)?
  reachable(x, y) {
    const v = this.sites.village;
    return this.field(v.x, v.y)[this.idx(x, y)] >= 0;
  }

  // Animated water glints, drawn every frame on top of the baked layer.
  // A few glints that fade in and out at fixed spots, each on its own slow phase. They used to be
  // a fresh random scatter every half second, which read as arrays of white pixels popping up
  // on the sea (Alex). Same density per pixel of sea on any world size (#129).
  drawWater(ctx, time) {
    const rng = mulberry32(this.seed ^ 0x2545f491);
    const n = Math.round((24 * this.width * this.height) / (W * H));
    for (let i = 0; i < n; i++) {
      const x = Math.floor(rng() * this.width), y = Math.floor(rng() * this.height);
      const phase = rng() * Math.PI * 2, speed = 0.5 + rng() * 0.7;
      if (this.typeAt(x, y) > T.SHALLOW) continue;
      const a = Math.max(0, Math.sin(time * speed + phase)) * 0.3;
      if (a < 0.03) continue;
      ctx.fillStyle = `rgba(170, 205, 240, ${a.toFixed(3)})`;
      ctx.fillRect(x, y, 2, 1);
    }
  }
}
