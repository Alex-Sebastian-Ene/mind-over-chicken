// One procedurally generated world that the civilisation lives on for the whole game: a home
// continent and two smaller ones across the sea. Each era repaints the same land: wild in the
// Stone Age, farmed and built up later, with settlements spreading to the other continents.

import { W, H, clamp, lerp, mulberry32, dist } from '../sim/util.js';
import { drawHouse, drawHut, drawFactory, drawExchange, drawTower, drawHangar, drawPad } from '../render/sprites.js';

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

function makeNoise(rng) {
  const G = 33;
  const grid = new Float32Array(G * G).map(() => rng());
  const at = (x, y) => grid[(((y % G) + G) % G) * G + (((x % G) + G) % G)];
  const sm = (t) => t * t * (3 - 2 * t);
  const value = (x, y) => {
    const xi = Math.floor(x), yi = Math.floor(y), xf = sm(x - xi), yf = sm(y - yi);
    return lerp(lerp(at(xi, yi), at(xi + 1, yi), xf), lerp(at(xi, yi + 1), at(xi + 1, yi + 1), xf), yf);
  };
  return (x, y, oct = 4) => {
    let s = 0, a = 0.5, f = 1, norm = 0;
    for (let i = 0; i < oct; i++) { s += a * value(x * f, y * f); norm += a; a *= 0.5; f *= 2; }
    return s / norm;
  };
}

export class World {
  constructor(seed = (Math.random() * 1e9) | 0, opts = {}) {
    this.seed = seed;
    this.biome = opts.biome && BIOMES[opts.biome] ? opts.biome : 'temperate';
    this.palette = BIOMES[this.biome];
    this.h = new Float32Array(W * H);
    this.type = new Uint8Array(W * H);
    this.baseType = null;
    this.layers = {};
    this.generate();
  }

  generate() {
    const rng = mulberry32(this.seed);
    const noise = makeNoise(rng);
    const moist = makeNoise(mulberry32(this.seed ^ 0x9e3779b9));
    // The home continent fills the left two thirds; two smaller continents sit across the sea.
    const conts = [
      { id: 'home', cx: 124, cy: 96, rx: 98, ry: 66 },
      { id: 'north', cx: 268 + rng() * 10, cy: 48 + rng() * 8, rx: 40 + rng() * 6, ry: 30 + rng() * 4 },
      { id: 'south', cx: 266 + rng() * 10, cy: 138 + rng() * 6, rx: 38 + rng() * 6, ry: 26 + rng() * 4 },
    ];
    this.continents = conts;
    const cx = conts[0].cx, cy = conts[0].cy;
    this.sites = {
      village: { x: 116, y: 100 },
      pad: { x: 198, y: 104 },
    };
    const v = this.sites.village, p = this.sites.pad;
    const ridgeA = { x: cx - 48 + rng() * 10, y: cy - 30 + rng() * 8 };
    const ridgeB = { x: cx + 36 + rng() * 10, y: cy - 38 + rng() * 6 };
    this.sites.ridge = ridgeA;
    this.owner = new Uint8Array(W * H);

    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        let best = -9, second = -9, owner = 0;
        conts.forEach((c, i) => {
          const rx = (x - c.cx) / c.rx, ry = (y - c.cy) / c.ry;
          const b = 1 - (rx * rx + ry * ry);
          if (b > best) { second = best; best = b; owner = i; } else if (b > second) second = b;
        });
        this.owner[y * W + x] = owner;
        let h = best * 0.56 + (noise(x / 24, y / 24) - 0.5) * 0.56 + 0.12;
        // Keep a strait of open water between neighbouring continents.
        if (best - second < 0.22 && second > -1.2) h = Math.min(h, 0.2 + (best - second) * 0.4);
        h += 0.3 * Math.exp(-((x - ridgeA.x) ** 2 + (y - ridgeA.y) ** 2) / (2 * 16 * 16));
        h += 0.2 * Math.exp(-((x - ridgeB.x) ** 2 + (y - ridgeB.y) ** 2) / (2 * 11 * 11));
        // Flatten a meadow for the village and a coastal plain for the launch pad.
        const dv = dist(x, y, v.x, v.y);
        if (dv < 30) h = lerp(0.5, h, clamp((dv - 18) / 12));
        const dp = dist(x, y, p.x, p.y);
        if (dp < 18) h = lerp(0.47, h, clamp((dp - 10) / 8));
        // A walkable corridor from village to pad.
        const t = clamp(((x - v.x) * (p.x - v.x) + (y - v.y) * (p.y - v.y)) / ((p.x - v.x) ** 2 + (p.y - v.y) ** 2));
        const lx = v.x + (p.x - v.x) * t, ly = v.y + (p.y - v.y) * t + Math.sin(t * 9 + this.seed) * 5;
        const dl = dist(x, y, lx, ly);
        if (dl < 4) h = clamp(h, 0.42, 0.66);
        // Small settlement meadows on the other continents.
        for (let i = 1; i < conts.length; i++) {
          const dc = dist(x, y, conts[i].cx, conts[i].cy);
          if (dc < 12) h = lerp(0.48, h, clamp((dc - 6) / 6));
        }
        this.h[y * W + x] = h;

        let ty;
        if (h < 0.24) ty = T.DEEP;
        else if (h < 0.33) ty = T.SHALLOW;
        else if (h < 0.37) ty = T.SAND;
        else if (h < 0.66) ty = moist(x / 18, y / 18) > 0.56 && dv > 26 && dp > 16 && dl > 4 ? T.FOREST : T.GRASS;
        else if (h < 0.76) ty = T.HILL;
        else if (h < 0.9) ty = T.MOUNTAIN;
        else ty = T.SNOW;
        this.type[y * W + x] = ty;
      }
    }
    this.baseType = this.type.slice();

    // Cave: the first hill/mountain pixel walking from the village toward the main ridge.
    const steps = 200;
    this.sites.cave = { x: v.x - 20, y: v.y - 20 };
    for (let i = 0; i <= steps; i++) {
      const x = Math.round(lerp(v.x, ridgeA.x, i / steps)), y = Math.round(lerp(v.y, ridgeA.y, i / steps));
      if (this.get(x, y) >= T.HILL) {
        const back = Math.max(0, i - 3);
        this.sites.cave = { x: Math.round(lerp(v.x, ridgeA.x, back / steps)), y: Math.round(lerp(v.y, ridgeA.y, back / steps)) };
        this.sites.caveMouth = { x, y };
        break;
      }
    }
    this.sites.caveMouth ||= { ...this.sites.cave };
    this.sites.fire = { x: v.x, y: v.y };
    this.sites.factory = this.nearestWalkable(v.x - 46, v.y + 26);
    this.sites.exchange = this.nearestWalkable(v.x + 18, v.y - 10);
    this.sites.picket = this.nearestWalkable(this.sites.factory.x + 2, this.sites.factory.y + 9);
    this.sites.hangar = this.nearestWalkable(p.x - 26, p.y + 10);
    this.sites.crowd = this.nearestWalkable(p.x - 30, p.y - 10);

    // Other continents: a village spot and a harbour on the coast facing home.
    for (const c of this.continents) {
      c.village = c.id === 'home' ? { ...v } : this.nearestWalkable(c.cx, c.cy);
    }
    const home = this.continents[0];
    for (const c of this.continents.slice(1)) {
      c.harbor = this.coastToward(c.village, home.village, (x, y) => this.owner[y * W + x] !== 0);
      c.homeHarbor = this.coastToward(home.village, c.village, (x, y) => this.owner[y * W + x] === 0);
    }
  }

  // Walk from `from` toward `to`: the last walkable pixel before the sea is the quay,
  // the first water pixel is the dock where boats tie up.
  coastToward(from, to, pred) {
    const n = Math.ceil(dist(from.x, from.y, to.x, to.y));
    let last = { ...from };
    for (let i = 0; i <= n; i++) {
      const x = Math.round(lerp(from.x, to.x, i / n)), y = Math.round(lerp(from.y, to.y, i / n));
      if (this.isWater(x, y)) return { land: last, dock: { x, y } };
      if (this.walkable(x, y) && (!pred || pred(x, y))) last = { x, y };
    }
    return { land: last, dock: this.nearestWater(last.x, last.y) };
  }

  nearestWater(x, y, maxR = 30) {
    for (let r = 1; r <= maxR; r++) for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) {
      if (Math.max(Math.abs(dx), Math.abs(dy)) === r && this.isWater(x + dx, y + dy)) return { x: x + dx, y: y + dy };
    }
    return { x, y };
  }

  get(x, y) {
    x = Math.round(x); y = Math.round(y);
    if (x < 0 || y < 0 || x >= W || y >= H) return T.DEEP;
    return this.type[y * W + x];
  }
  walkable(x, y) { const t = this.get(x, y); return t >= T.SAND && t !== T.MOUNTAIN && t !== T.SNOW; }
  isWater(x, y) { return this.get(x, y) <= T.SHALLOW; }

  nearestWalkable(x, y, maxR = 60) {
    x = Math.round(x); y = Math.round(y);
    for (let r = 0; r <= maxR; r++) {
      for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
        if (this.walkable(x + dx, y + dy) && this.get(x + dx, y + dy) !== T.FOREST) return { x: x + dx, y: y + dy };
      }
    }
    return { x, y };
  }

  // Random walkable point in a ring around (x,y).
  randomLand(x, y, rMin, rMax, pred) {
    for (let i = 0; i < 80; i++) {
      const a = Math.random() * Math.PI * 2, r = rMin + Math.random() * (rMax - rMin);
      const px = Math.round(x + Math.cos(a) * r), py = Math.round(y + Math.sin(a) * r);
      if (this.walkable(px, py) && (!pred || pred(px, py))) return { x: px, y: py };
    }
    return this.nearestWalkable(x, y);
  }

  // Era-specific land use. Called once per era before painting.
  shape(era) {
    this.type = this.baseType.slice();
    this.fields = new Map();
    this.structures = [];
    const v = this.sites.village, s = this.sites;
    const rng = mulberry32(this.seed + 17);
    const setT = (x, y, t) => { if (x >= 0 && y >= 0 && x < W && y < H) this.type[y * W + x] = t; };
    const road = (a, b, w = 1) => {
      const n = Math.ceil(dist(a.x, a.y, b.x, b.y));
      for (let i = 0; i <= n; i++) {
        const x = Math.round(lerp(a.x, b.x, i / n)), y = Math.round(lerp(a.y, b.y, i / n));
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
        const a = (i / 4) * Math.PI * 2 + 0.5;
        this.structures.push({ kind: 'hut', x: Math.round(v.x + Math.cos(a) * 11), y: Math.round(v.y + Math.sin(a) * 7) });
      }
    }
    if (era === 'farming') {
      farms(14, 18, 60);
      for (let i = 0; i < 10; i++) {
        const a = (i / 10) * Math.PI * 2;
        this.structures.push({ kind: 'hut', x: Math.round(v.x + Math.cos(a) * (12 + (i % 2) * 6)), y: Math.round(v.y + Math.sin(a) * (8 + (i % 2) * 4)) });
      }
    }
    // Settlements spread to the other continents from the farming age on.
    if (era !== 'stone') {
      for (const c of this.continents.slice(1)) {
        const n = era === 'farming' ? 5 : 8;
        for (let i = 0; i < n; i++) {
          const a = (i / n) * Math.PI * 2;
          const x = Math.round(c.village.x + Math.cos(a) * 7), y = Math.round(c.village.y + Math.sin(a) * 5);
          if (!this.walkable(x, y)) continue;
          if (era === 'farming') this.structures.push({ kind: 'hut', x, y });
          else if (era === 'space' && i % 3 === 0) this.structures.push({ kind: 'tower', x, y, h: 7 + (i % 4) });
          else this.structures.push({ kind: 'house', x, y, roof: era === 'space' ? '#5a6b8c' : '#8e3b2a' });
        }
        if (era !== 'farming') road(c.village, c.harbor.land, 1);
      }
    }
    if (era === 'industry' || era === 'space') {
      farms(era === 'industry' ? 22 : 12, 34, 90);
      road(v, s.factory, 2); road(v, s.exchange, 1); road(v, s.pad, 1);
      road({ x: v.x - 30, y: v.y - 2 }, { x: v.x + 30, y: v.y - 2 });
      road({ x: v.x, y: v.y - 22 }, { x: v.x, y: v.y + 22 });
      for (let y = -2; y <= 2; y++) for (let x = -3; x <= 3; x++) setT(v.x + x, v.y + y, T.PAVED);
      const roofs = era === 'industry' ? ['#a4402f', '#8e3b2a', '#b5543a', '#6f4a3a'] : ['#5a6b8c', '#7f8fa8', '#a4402f', '#4d5d7a'];
      let placed = 0;
      for (let ring = 0; ring < 3; ring++) {
        const n = 10 + ring * 6;
        for (let i = 0; i < n; i++) {
          const a = (i / n) * Math.PI * 2 + ring * 0.3;
          const x = Math.round(v.x + Math.cos(a) * (12 + ring * 9)), y = Math.round(v.y + Math.sin(a) * (8 + ring * 6));
          if (!this.walkable(x, y) || dist(x, y, s.exchange.x, s.exchange.y) < 9 || dist(x, y, s.factory.x, s.factory.y) < 12) continue;
          if (era === 'space' && ring === 0 && i % 2 === 0) this.structures.push({ kind: 'tower', x, y, h: 8 + Math.floor(rng() * 7) });
          else this.structures.push({ kind: 'house', x, y, roof: roofs[(i + ring) % roofs.length] });
          placed++;
        }
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

  // Bake terrain + static structures into pixel canvases (2D flat and 3D clean landscape).
  paint(era) {
    this.shape(era);
    this.layers3d ||= {};

    // 1. Collect 3D tree coordinates for forest tiles
    this.trees = [];
    const trng = mulberry32(this.seed + 5);
    for (let y = 1; y < H; y += 2) for (let x = (y >> 1) % 2; x < W; x += 2) {
      if (this.type[y * W + x] !== T.FOREST || trng() > 0.7) continue;
      this.trees.push({ x, y, variant: trng() > 0.5 ? 1 : 0 });
    }

    // 2. Base landscape texture (used for 3D terrain mesh upload: no flat structure or tree decals)
    const c3d = document.createElement('canvas');
    c3d.width = W; c3d.height = H;
    const ctx3d = c3d.getContext('2d');
    const img = ctx3d.createImageData(W, H);
    const rng = mulberry32(this.seed + 3);
    for (let i = 0; i < W * H; i++) {
      const t = this.type[i];
      const x = i % W, y = (i / W) | 0;
      let col;
      if (t === T.FARM) col = this.palette[T.FARM][(y >> 1) % 2];
      else { const pal = this.palette[t]; col = pal[Math.floor(rng() * pal.length)]; }
      let [r, g, b] = col;
      // Gentle relief shading from the heightmap so hills read as hills.
      if (t >= T.SAND && t !== T.ROAD && t !== T.PAVED && t !== T.FARM) {
        const dx = this.h[i] - (x > 0 ? this.h[i - 1] : this.h[i]);
        const dy = this.h[i] - (y > 0 ? this.h[i - W] : this.h[i]);
        const shade = clamp(1 + (dx + dy) * 7, 0.78, 1.2);
        r *= shade; g *= shade; b *= shade;
      }
      // Foam where shallow water meets sand.
      if (t === T.SHALLOW && (this.typeAt(x + 1, y) === T.SAND || this.typeAt(x - 1, y) === T.SAND || this.typeAt(x, y + 1) === T.SAND || this.typeAt(x, y - 1) === T.SAND)) { [r, g, b] = this.palette.foam; }
      img.data[i * 4] = r; img.data[i * 4 + 1] = g; img.data[i * 4 + 2] = b; img.data[i * 4 + 3] = 255;
    }
    ctx3d.putImageData(img, 0, 0);

    // Cave mouth.
    if (era === 'stone' || era === 'farming') {
      const m = this.sites.caveMouth;
      ctx3d.fillStyle = '#1a1714'; ctx3d.fillRect(m.x - 2, m.y - 1, 4, 3);
      ctx3d.fillStyle = '#2b2622'; ctx3d.fillRect(m.x - 3, m.y, 1, 2); ctx3d.fillRect(m.x + 2, m.y, 1, 2);
    }
    this.layers3d[era] = c3d;

    // 3. 2D fallback / globe texture (with flat painted trees and structures)
    const c2d = document.createElement('canvas');
    c2d.width = W; c2d.height = H;
    const ctx2d = c2d.getContext('2d');
    ctx2d.drawImage(c3d, 0, 0);

    // Flat tree crowns for 2D mode
    const trng2 = mulberry32(this.seed + 5);
    for (let y = 1; y < H; y += 2) for (let x = (y >> 1) % 2; x < W; x += 2) {
      if (this.type[y * W + x] !== T.FOREST || trng2() > 0.7) continue;
      const tc = this.palette.trees;
      ctx2d.fillStyle = tc[0]; ctx2d.fillRect(x, y, 1, 1);
      ctx2d.fillStyle = trng2() > 0.5 ? tc[1] : tc[2]; ctx2d.fillRect(x, y - 1, 1, 1);
    }

    // Flat structures for 2D mode
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

  // Draw 3D trees and structures as upright billboards standing on the terrain
  draw3d(engine) {
    if (!engine) return;
    // 3D Trees across forest tiles
    if (this.trees) {
      for (const t of this.trees) {
        engine.add_billboard(14, t.x, t.y, 1, 0, t.variant || 0);
      }
    }
    // 3D Structures
    if (this.structures) {
      for (const s of this.structures) {
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

  typeAt(x, y) { if (x < 0 || y < 0 || x >= W || y >= H) return T.DEEP; return this.type[y * W + x]; }

  // Distance field (BFS over walkable pixels) toward a target, cached on a 3 px grid so
  // moving targets like mammoths don't trigger a new search every frame.
  field(tx, ty) {
    const qx = Math.max(0, Math.min(W - 1, Math.round(tx / 3) * 3)), qy = Math.max(0, Math.min(H - 1, Math.round(ty / 3) * 3));
    const key = qy * W + qx;
    this.fields ||= new Map();
    let f = this.fields.get(key);
    if (f) { this.fields.delete(key); this.fields.set(key, f); return f; }
    f = new Int16Array(W * H).fill(-1);
    const start = this.walkable(qx, qy) ? { x: qx, y: qy } : this.nearestWalkable(qx, qy, 12);
    const queue = new Int32Array(W * H);
    let head = 0, tail = 0;
    const s0 = start.y * W + start.x;
    f[s0] = 0; queue[tail++] = s0;
    while (head < tail) {
      const i = queue[head++];
      const x = i % W, y = (i / W) | 0, d = f[i] + 1;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        if (!dx && !dy) continue;
        const nx = x + dx, ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= W || ny >= H) continue;
        const j = ny * W + nx;
        if (f[j] !== -1) continue;
        const t = this.type[j];
        if (t < T.SAND || t === T.MOUNTAIN || t === T.SNOW) continue;
        f[j] = d; queue[tail++] = j;
      }
    }
    this.fields.set(key, f);
    if (this.fields.size > 64) this.fields.delete(this.fields.keys().next().value);
    return f;
  }

  // Same idea over water, for boats.
  seaField(tx, ty) {
    const qx = Math.round(tx), qy = Math.round(ty);
    const key = -(qy * W + qx) - 1;
    this.fields ||= new Map();
    let f = this.fields.get(key);
    if (f) return f;
    f = new Int16Array(W * H).fill(-1);
    const queue = new Int32Array(W * H);
    let head = 0, tail = 0;
    const s0 = qy * W + qx;
    f[s0] = 0; queue[tail++] = s0;
    while (head < tail) {
      const i = queue[head++];
      const x = i % W, y = (i / W) | 0, d = f[i] + 1;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        if (!dx && !dy) continue;
        const nx = x + dx, ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= W || ny >= H) continue;
        const j = ny * W + nx;
        if (f[j] !== -1 || this.type[j] > T.SHALLOW) continue;
        f[j] = d; queue[tail++] = j;
      }
    }
    this.fields.set(key, f);
    return f;
  }

  // Can you walk from the village to (x, y)?
  reachable(x, y) {
    const v = this.sites.village;
    return this.field(v.x, v.y)[Math.round(y) * W + Math.round(x)] >= 0;
  }

  // Animated water glints, drawn every frame on top of the baked layer.
  drawWater(ctx, time) {
    const rng = mulberry32((Math.floor(time * 2) * 7919) ^ this.seed);
    ctx.fillStyle = 'rgba(170, 205, 240, 0.55)';
    for (let i = 0; i < 70; i++) {
      const x = Math.floor(rng() * W), y = Math.floor(rng() * H);
      if (this.typeAt(x, y) <= T.SHALLOW) ctx.fillRect(x, y, 2, 1);
    }
  }
}
