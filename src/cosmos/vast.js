// Vaster planet surfaces (#68). Pure: typed arrays in, typed arrays out, no DOM, so it runs in
// the worldgen worker and in Node tests.
//
// Every non-home world gets a VW × VH equirectangular surface (4× the area of the 320×180
// World), wrapping east–west, poles at the top and bottom. The AI god's own World (the one its
// Stone Age tribe lives on, src/ai/planet.js) is embedded unchanged as the CAPITAL region at
// TILE, so the tribe's paths and the drawn land agree pixel for pixel. Around it, more
// continents are generated with the same land rules and BIOMES palettes as src/world/terrain.js,
// and heights are blended across the capital's border so coastlines continue instead of
// stopping at a seam.
//
// terrain.js stays 320×180 for now (the eras and the Rust engine assume it). Once World takes a
// size, this generator becomes World(seed, { biome, width: VW, height: VH, continents }).

import { T, BIOMES } from '../world/terrain.js';
import { W, H, mulberry32 } from '../sim/util.js';

export const VW = W * 2;                         // 640
export const VH = H * 2;                         // 360
export const TILE = { x: 0, y: (VH - H) / 2 };   // capital region: west half, equator band
const BLEND = 14;                                // px over which the capital's coast blends out
const POLE = 12;                                 // rows of polar ice at the top and bottom
const LATTICE = 26;                              // noise cells across the map (so noise wraps at VW)

const smooth = (t) => t * t * (3 - 2 * t);
const clamp = (v, lo = 0, hi = 1) => (v < lo ? lo : v > hi ? hi : v);
const lerp = (a, b, t) => a + (b - a) * t;

export const wrapX = (x) => ((x % VW) + VW) % VW;
export const wrapDx = (dx) => dx - VW * Math.round(dx / VW);

// Integer hash → [0, 1).
function hash(ix, iy, s) {
  let h = (Math.imul(ix, 374761393) + Math.imul(iy, 668265263) + Math.imul(s, 1442695041)) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

// Fractal value noise whose x period is exactly VW, so the map wraps without a seam.
export function makeNoise(seed) {
  const cell = VW / LATTICE;
  return (x, y, oct = 4) => {
    let s = 0, a = 0.5, norm = 0, f = 1;
    for (let o = 0; o < oct; o++) {
      const period = LATTICE * f;
      const fx = (x / cell) * f, fy = (y / cell) * f;
      const xi = Math.floor(fx), yi = Math.floor(fy);
      const xf = smooth(fx - xi), yf = smooth(fy - yi);
      const x0 = ((xi % period) + period) % period, x1 = (x0 + 1) % period;
      const k = seed + o * 101;
      const v = lerp(lerp(hash(x0, yi, k), hash(x1, yi, k), xf), lerp(hash(x0, yi + 1, k), hash(x1, yi + 1, k), xf), yf);
      s += a * v; norm += a; a *= 0.5; f *= 2;
    }
    return s / norm;
  };
}

// Land type from height and moisture, the same thresholds as World.generate.
function typeFor(h, wet) {
  if (h < 0.24) return T.DEEP;
  if (h < 0.33) return T.SHALLOW;
  if (h < 0.37) return T.SAND;
  if (h < 0.66) return wet > 0.56 ? T.FOREST : T.GRASS;
  if (h < 0.76) return T.HILL;
  if (h < 0.9) return T.MOUNTAIN;
  return T.SNOW;
}

export const walkableType = (t) => t >= T.SAND && t !== T.MOUNTAIN && t !== T.SNOW;

// Continents outside the capital region: rejection-sampled ellipses that keep clear of it.
function placeContinents(rng, count) {
  const out = [];
  const tileBox = { x0: TILE.x - 10, x1: TILE.x + W + 10, y0: TILE.y - 8, y1: TILE.y + H + 8 };
  for (let tries = 0; out.length < count && tries < 400; tries++) {
    const polar = out.length >= count - 2;           // the last two sit in the high latitudes
    const rx = polar ? 40 + rng() * 30 : 42 + rng() * 38;
    const ry = polar ? 18 + rng() * 10 : 30 + rng() * 22;
    const cx = polar ? rng() * VW : W + 20 + rng() * (W - 40);
    const cy = polar ? (rng() < 0.5 ? POLE + 26 + rng() * 30 : VH - POLE - 26 - rng() * 30) : 50 + rng() * (VH - 100);
    // Keep clear of the capital region (with wrap) and of other continents.
    const nearX = Math.max(tileBox.x0, Math.min(tileBox.x1, cx));
    const dxT = Math.abs(wrapDx(cx - nearX)), dyT = Math.max(0, tileBox.y0 - cy, cy - tileBox.y1);
    const insideTile = cx >= tileBox.x0 && cx <= tileBox.x1 && cy >= tileBox.y0 && cy <= tileBox.y1;
    if (insideTile || (dxT < rx * 0.9 && dyT < ry * 0.9)) continue;
    if (out.some((c) => Math.abs(wrapDx(c.cx - cx)) < (c.rx + rx) * 0.85 && Math.abs(c.cy - cy) < (c.ry + ry) * 0.85)) continue;
    out.push({ id: `c${out.length + 3}`, cx, cy, rx, ry });
  }
  return out;
}

// Build the vast surface around a World. `world` needs h, baseType, sites, continents (a
// rehydrated or freshly generated World both do). Returns plain data + typed arrays.
export function buildVast(world, seed = world.seed) {
  const rng = mulberry32((seed ^ 0x51ed270b) >>> 0);
  const noise = makeNoise((seed * 7 + 13) | 0);
  const moist = makeNoise((seed * 11 + 29) | 0);
  const ice = makeNoise((seed * 3 + 5) | 0);
  const extra = placeContinents(rng, 5 + Math.floor(rng() * 2));
  const height = new Float32Array(VW * VH);
  const type = new Uint8Array(VW * VH);
  const tx0 = TILE.x, ty0 = TILE.y;

  for (let y = 0; y < VH; y++) {
    for (let x = 0; x < VW; x++) {
      const i = y * VW + x;
      const lx = wrapX(x - tx0), ly = y - ty0;
      if (lx < W && ly >= 0 && ly < H) {
        // The capital: the AI god's World, unchanged.
        height[i] = world.h[ly * W + lx];
        type[i] = world.baseType[ly * W + lx];
        continue;
      }
      let best = -9, second = -9;
      for (const c of extra) {
        const rx = wrapDx(x - c.cx) / c.rx, ry = (y - c.cy) / c.ry;
        const b = 1 - (rx * rx + ry * ry);
        if (b > best) { second = best; best = b; } else if (b > second) second = b;
      }
      let h = best * 0.56 + (noise(x, y) - 0.5) * 0.56 + 0.12;
      if (best - second < 0.22 && second > -1.2) h = Math.min(h, 0.2 + (best - second) * 0.4);
      // Blend into the capital's coast: nearest capital pixel's height, fading out over BLEND px.
      let ex = lx, dX = 0;
      if (lx >= W) { const de = lx - (W - 1), dw = VW - lx; ex = de <= dw ? W - 1 : 0; dX = Math.min(de, dw); }
      const ey = Math.min(H - 1, Math.max(0, ly));
      const dY = ly < 0 ? -ly : ly >= H ? ly - H + 1 : 0;
      const d = Math.hypot(dX, dY);
      if (d < BLEND) h = lerp(world.h[ey * W + ex], h, smooth(d / BLEND));
      height[i] = h;
      type[i] = typeFor(h, moist(x, y));
    }
  }

  // Polar ice: ragged caps over sea and land alike.
  for (let x = 0; x < VW; x++) {
    const capN = POLE + Math.floor((ice(x, 0, 3) - 0.5) * 10);
    const capS = POLE + Math.floor((ice(x, VH, 3) - 0.5) * 10);
    for (let y = 0; y < capN; y++) type[y * VW + x] = T.SNOW;
    for (let y = VH - capS; y < VH; y++) type[y * VW + x] = T.SNOW;
  }

  const v = { w: VW, h: VH, tile: { ...TILE }, height, type, continents: [], spots: [] };
  // Continents: the World's three (shifted), then the new ones.
  for (const c of world.continents) {
    v.continents.push({ id: c.id, cx: wrapX(c.cx + tx0), cy: c.cy + ty0, rx: c.rx, ry: c.ry, village: { x: wrapX(c.village.x + tx0), y: c.village.y + ty0 } });
  }
  for (const c of extra) {
    const village = nearestLand(v, Math.round(c.cx), Math.round(c.cy), 40);
    v.continents.push({ ...c, village });
  }

  // Settlement spots: the capital first (the tribe's own village), then every continent's
  // village, then a few more on the bigger landmasses; nearest to the capital first, which is
  // the order a growing civilisation spreads in.
  const cap = { x: wrapX(world.sites.village.x + tx0), y: world.sites.village.y + ty0 };
  const spots = [{ x: cap.x, y: cap.y, cont: 'home', capital: true }];
  const far = (x, y) => spots.every((s) => Math.hypot(wrapDx(s.x - x), s.y - y) > 26);
  for (const c of v.continents) {
    if (c.id === 'home' || !c.village) continue;
    if (walkableAt(v, c.village.x, c.village.y) && far(c.village.x, c.village.y)) spots.push({ x: c.village.x, y: c.village.y, cont: c.id });
  }
  for (const c of v.continents) {
    const extraSpots = c.rx * c.ry > 1800 ? 2 : 1;
    for (let k = 0; k < extraSpots; k++) {
      for (let tries = 0; tries < 30; tries++) {
        const a = rng() * Math.PI * 2, r = 0.35 + rng() * 0.4;
        const x = Math.round(wrapX(c.cx + Math.cos(a) * c.rx * r)), y = Math.round(c.cy + Math.sin(a) * c.ry * r);
        if (y < POLE + 6 || y > VH - POLE - 6) continue;
        const t = type[y * VW + x];
        if ((t === T.GRASS || t === T.SAND) && far(x, y)) { spots.push({ x, y, cont: c.id }); break; }
      }
    }
  }
  const d0 = (s) => Math.hypot(wrapDx(s.x - cap.x), s.y - cap.y);
  v.spots = [spots[0], ...spots.slice(1).sort((a, b) => d0(a) - d0(b))];
  return v;
}

export function typeAt(v, x, y) {
  x = wrapX(Math.round(x)); y = Math.round(y);
  if (y < 0 || y >= v.h) return T.SNOW;
  return v.type[y * v.w + x];
}
export const walkableAt = (v, x, y) => walkableType(typeAt(v, x, y));

export function nearestLand(v, x, y, maxR = 30) {
  for (let r = 0; r <= maxR; r++) {
    for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) {
      if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
      const t = typeAt(v, x + dx, y + dy);
      if (t === T.GRASS || t === T.SAND || t === T.HILL) return { x: wrapX(x + dx), y: y + dy };
    }
  }
  return null;
}

// Paint the vast surface (terrain only) into RGBA, the same look as World.paint: palette
// colours, relief shading, foam on the beaches and tree crowns on forest.
export function paintVast(v, biome = 'temperate', seed = 1) {
  const pal = BIOMES[biome] || BIOMES.temperate;
  const { w, h, type, height } = v;
  const out = new Uint8ClampedArray(w * h * 4);
  const rng = mulberry32((seed + 3) >>> 0);
  const tAt = (x, y) => (y < 0 || y >= h ? T.DEEP : type[y * w + wrapX(x)]);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x, t = type[i];
      const cols = pal[t] || pal[T.GRASS];
      let [r, g, b] = cols[Math.floor(rng() * cols.length)];
      if (t >= T.SAND && t !== T.ROAD && t !== T.PAVED && t !== T.FARM) {
        const dx = height[i] - height[y * w + wrapX(x - 1)];
        const dy = height[i] - (y > 0 ? height[i - w] : height[i]);
        const s = clamp(1 + (dx + dy) * 7, 0.78, 1.2);
        r *= s; g *= s; b *= s;
      }
      if (t === T.SHALLOW && (tAt(x + 1, y) === T.SAND || tAt(x - 1, y) === T.SAND || tAt(x, y + 1) === T.SAND || tAt(x, y - 1) === T.SAND)) [r, g, b] = pal.foam;
      const o = i * 4;
      out[o] = r; out[o + 1] = g; out[o + 2] = b; out[o + 3] = 255;
    }
  }
  // Tree crowns, as World.paint draws them.
  const trng = mulberry32((seed + 5) >>> 0);
  const hex = (s) => [parseInt(s.slice(1, 3), 16), parseInt(s.slice(3, 5), 16), parseInt(s.slice(5, 7), 16)];
  const tc = pal.trees.map(hex);
  const put = (x, y, c) => { if (y < 0) return; const o = (y * w + wrapX(x)) * 4; out[o] = c[0]; out[o + 1] = c[1]; out[o + 2] = c[2]; };
  for (let y = 1; y < h; y += 2) for (let x = (y >> 1) % 2; x < w; x += 2) {
    if (type[y * w + x] !== T.FOREST || trng() > 0.7) continue;
    put(x, y, tc[0]);
    put(x, y - 1, trng() > 0.5 ? tc[1] : tc[2]);
  }
  return out;
}

// Which spots are settled and how big each one is, from the world's tier and people.
// Pure, so the surface, the globe lights and tests agree. size: 0 camp … 3 city.
export const SPREAD = [1, 3, 6, 99];        // most settlements per tier
export const PEOPLE_PER_SITE = [0, 30, 120, 600];
export function settlementPlan(state, spots) {
  if (!state || state.pop <= 0 || !spots?.length) return [];
  const tier = state.tier ?? 0;
  const n = Math.max(1, Math.min(spots.length, SPREAD[tier], 1 + Math.floor(state.pop / Math.max(1, PEOPLE_PER_SITE[tier]))));
  const out = [];
  for (let k = 0; k < n; k++) {
    // The capital is the biggest; the rest shrink with distance, never below a camp.
    const size = Math.max(0, tier - (k === 0 ? 0 : 1 + Math.floor((k - 1) / 3)));
    out.push({ ...spots[k], size, index: k });
  }
  return out;
}
