// One procedurally generated island that the civilisation lives on for the whole game.
// Each era repaints the same land: wild in the Stone Age, farmed and built up later.

import { W, H, clamp, lerp, mulberry32, dist } from '../sim/util.js';
import { drawHouse, drawHut, drawFactory, drawExchange, drawTower, drawHangar, drawPad } from '../render/sprites.js';

export const T = { DEEP: 0, SHALLOW: 1, SAND: 2, GRASS: 3, FOREST: 4, HILL: 5, MOUNTAIN: 6, SNOW: 7, FARM: 8, ROAD: 9, PAVED: 10 };

const PALETTE = {
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
  constructor(seed = (Math.random() * 1e9) | 0) {
    this.seed = seed;
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
    const cx = 160, cy = 92;
    this.sites = {
      village: { x: 150, y: 102 },
      pad: { x: 262, y: 104 },
    };
    const v = this.sites.village, p = this.sites.pad;
    const ridgeA = { x: cx - 62 + rng() * 10, y: cy - 34 + rng() * 8 };
    const ridgeB = { x: cx + 34 + rng() * 14, y: cy - 46 + rng() * 6 };
    this.sites.ridge = ridgeA;

    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const rx = (x - cx) / 138, ry = (y - cy) / 80;
        const r2 = rx * rx + ry * ry;
        let h = (1 - r2) * 0.56 + (noise(x / 26, y / 26) - 0.5) * 0.56 + 0.12;
        h += 0.3 * Math.exp(-((x - ridgeA.x) ** 2 + (y - ridgeA.y) ** 2) / (2 * 17 * 17));
        h += 0.2 * Math.exp(-((x - ridgeB.x) ** 2 + (y - ridgeB.y) ** 2) / (2 * 12 * 12));
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

  // Bake terrain + static structures into one pixel canvas for this era.
  paint(era) {
    this.shape(era);
    const c = document.createElement('canvas');
    c.width = W; c.height = H;
    const ctx = c.getContext('2d');
    const img = ctx.createImageData(W, H);
    const rng = mulberry32(this.seed + 3);
    for (let i = 0; i < W * H; i++) {
      const t = this.type[i];
      const x = i % W, y = (i / W) | 0;
      let col;
      if (t === T.FARM) col = PALETTE[T.FARM][(y >> 1) % 2];
      else { const pal = PALETTE[t]; col = pal[Math.floor(rng() * pal.length)]; }
      let [r, g, b] = col;
      // Gentle relief shading from the heightmap so hills read as hills.
      if (t >= T.SAND && t !== T.ROAD && t !== T.PAVED && t !== T.FARM) {
        const dx = this.h[i] - (x > 0 ? this.h[i - 1] : this.h[i]);
        const dy = this.h[i] - (y > 0 ? this.h[i - W] : this.h[i]);
        const shade = clamp(1 + (dx + dy) * 7, 0.78, 1.2);
        r *= shade; g *= shade; b *= shade;
      }
      // Foam where shallow water meets sand.
      if (t === T.SHALLOW && (this.typeAt(x + 1, y) === T.SAND || this.typeAt(x - 1, y) === T.SAND || this.typeAt(x, y + 1) === T.SAND || this.typeAt(x, y - 1) === T.SAND)) { r = 120; g = 170; b = 205; }
      img.data[i * 4] = r; img.data[i * 4 + 1] = g; img.data[i * 4 + 2] = b; img.data[i * 4 + 3] = 255;
    }
    ctx.putImageData(img, 0, 0);
    // Tree crowns on forest pixels.
    const trng = mulberry32(this.seed + 5);
    for (let y = 1; y < H; y += 2) for (let x = (y >> 1) % 2; x < W; x += 2) {
      if (this.type[y * W + x] !== T.FOREST || trng() > 0.7) continue;
      ctx.fillStyle = '#2c5a2a'; ctx.fillRect(x, y, 1, 1);
      ctx.fillStyle = trng() > 0.5 ? '#6aa84f' : '#4f8d3e'; ctx.fillRect(x, y - 1, 1, 1);
    }
    // Cave mouth.
    if (era === 'stone' || era === 'farming') {
      const m = this.sites.caveMouth;
      ctx.fillStyle = '#1a1714'; ctx.fillRect(m.x - 2, m.y - 1, 4, 3);
      ctx.fillStyle = '#2b2622'; ctx.fillRect(m.x - 3, m.y, 1, 2); ctx.fillRect(m.x + 2, m.y, 1, 2);
    }
    for (const s of this.structures) {
      if (s.kind === 'hut') drawHut(ctx, s.x, s.y);
      if (s.kind === 'house') drawHouse(ctx, s.x, s.y, s.roof);
      if (s.kind === 'tower') drawTower(ctx, s.x, s.y, s.h);
      if (s.kind === 'factory') drawFactory(ctx, s.x, s.y);
      if (s.kind === 'exchange') drawExchange(ctx, s.x, s.y);
      if (s.kind === 'hangar') drawHangar(ctx, s.x, s.y);
      if (s.kind === 'pad') drawPad(ctx, s.x, s.y);
    }
    this.layers[era] = c;
    return c;
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
