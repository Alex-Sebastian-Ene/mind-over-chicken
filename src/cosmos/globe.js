// Pixel-art globe: wraps a 320×180 era layer (treated as an equirectangular map) onto a
// sphere, ray-cast per pixel straight into the low-res world canvas so it stays pixel art.
// Geometry per (radius, tilt) is cached; per frame only the longitude offset and light change.

import { W, H, mulberry32 } from '../sim/util.js';

const TAU = Math.PI * 2;
const LIGHT_KINDS = { house: [255, 214, 120], tower: [255, 236, 160], factory: [255, 170, 90], exchange: [255, 226, 140], hangar: [200, 230, 255], pad: [255, 120, 100], hut: [255, 150, 60] };

export class Globe {
  constructor() {
    this.geo = new Map();
    this.tex = new WeakMap();
    this.scratch = new Map();
  }

  // Sphere geometry for a radius and axial tilt: normals, base longitude, latitude row, rim.
  geometry(r, tilt) {
    const key = `${r}|${tilt.toFixed(3)}`;
    let g = this.geo.get(key);
    if (g) return g;
    const pad = 3, half = r + pad, size = half * 2 + 1;
    const n = size * size;
    g = { r, size, half, mask: new Uint8Array(n), nx: new Float32Array(n), ny: new Float32Array(n), nz: new Float32Array(n), lon: new Float32Array(n), row: new Int16Array(n), rim: new Float32Array(n) };
    const ct = Math.cos(tilt), st = Math.sin(tilt);
    const rimW = Math.max(1.2, r * 0.06);
    for (let py = 0; py < size; py++) for (let px = 0; px < size; px++) {
      const i = py * size + px;
      const x = (px - half + 0.5) / r, y = (py - half + 0.5) / r;
      const d2 = x * x + y * y;
      if (d2 <= 1) {
        const z = Math.sqrt(1 - d2);
        // tilt around the screen x axis
        const y2 = y * ct - z * st, z2 = y * st + z * ct;
        const lat = Math.asin(Math.max(-1, Math.min(1, -y2)));
        g.mask[i] = 1;
        g.nx[i] = x; g.ny[i] = y; g.nz[i] = z;
        g.lon[i] = Math.atan2(x, z2);
        g.row[i] = Math.max(0, Math.min(H - 1, Math.floor((0.5 - lat / Math.PI) * H)));
      } else {
        const d = Math.sqrt(d2);
        const over = (d - 1) * r;
        if (over < rimW) { g.mask[i] = 2; g.rim[i] = 1 - over / rimW; g.nx[i] = x / d; g.ny[i] = y / d; }
      }
    }
    this.geo.set(key, g);
    if (this.geo.size > 40) this.geo.delete(this.geo.keys().next().value);
    return g;
  }

  // Texture from a layer canvas: pixels + polar ice + an emissive light map from structures.
  texture(layer, structures, era) {
    let t = this.tex.get(layer);
    if (t) return t;
    const src = layer.getContext('2d').getImageData(0, 0, W, H).data;
    const data = new Uint8ClampedArray(src);
    const rng = mulberry32(W * 7 + H);
    for (let x = 0; x < W; x++) {
      const capN = 7 + Math.floor(rng() * 4 + Math.sin(x / 9) * 2);
      const capS = 6 + Math.floor(rng() * 4 + Math.cos(x / 11) * 2);
      for (let y = 0; y < H; y++) {
        const edge = y < capN ? 1 - y / capN : y >= H - capS ? 1 - (H - 1 - y) / capS : 0;
        if (edge <= 0) continue;
        const k = Math.min(1, 0.35 + edge * 1.4);
        const i = (y * W + x) * 4;
        const shade = 228 + ((x * 13 + y * 7) % 3) * 8;
        data[i] = data[i] * (1 - k) + shade * k;
        data[i + 1] = data[i + 1] * (1 - k) + (shade + 4) * k;
        data[i + 2] = data[i + 2] * (1 - k) + 250 * k;
      }
    }
    const lights = new Uint8Array(W * H);
    const lightCol = new Uint8Array(W * H * 3);
    for (const s of structures || []) {
      const col = LIGHT_KINDS[s.kind];
      if (!col) continue;
      const rad = s.kind === 'factory' || s.kind === 'exchange' || s.kind === 'pad' ? 2 : 1;
      for (let dy = -rad; dy <= rad; dy++) for (let dx = -rad; dx <= rad; dx++) {
        const x = Math.round(s.x + dx), y = Math.round(s.y - 2 + dy);
        if (x < 0 || y < 0 || x >= W || y >= H) continue;
        const j = y * W + x;
        const v = 255 - (Math.abs(dx) + Math.abs(dy)) * 70;
        if (v > lights[j]) { lights[j] = v; lightCol[j * 3] = col[0]; lightCol[j * 3 + 1] = col[1]; lightCol[j * 3 + 2] = col[2]; }
      }
    }
    t = { data, lights, lightCol, era };
    this.tex.set(layer, t);
    return t;
  }

  // Draw a globe centred at (x, y) with radius r onto the low-res ctx.
  // opts: rot (radians), tilt, light [lx, ly, lz] (view space, toward the sun), alpha, atmosphere colour.
  draw(ctx, tex, x, y, r, opts = {}) {
    r = Math.max(3, Math.round(r));
    const tilt = opts.tilt ?? 0.32;
    const g = this.geometry(r, tilt);
    let L = opts.light || [-0.55, -0.42, 0.72];
    const ll = Math.hypot(L[0], L[1], L[2]) || 1;
    L = [L[0] / ll, L[1] / ll, L[2] / ll];
    const rot = opts.rot || 0;
    const atm = opts.atmosphere || [110, 175, 255];
    let sc = this.scratch.get(g.size);
    if (!sc) {
      const c = document.createElement('canvas'); c.width = g.size; c.height = g.size;
      sc = { c, ctx: c.getContext('2d'), img: c.getContext('2d').createImageData(g.size, g.size) };
      this.scratch.set(g.size, sc);
    }
    const out = sc.img.data;
    const { data, lights, lightCol } = tex;
    const n = g.size * g.size;
    for (let i = 0; i < n; i++) {
      const m = g.mask[i], o = i * 4;
      if (m === 0) { out[o + 3] = 0; continue; }
      if (m === 2) {
        const lit = Math.max(0, g.nx[i] * L[0] + g.ny[i] * L[1]) * 0.8 + 0.2;
        out[o] = atm[0]; out[o + 1] = atm[1]; out[o + 2] = atm[2];
        out[o + 3] = Math.round(255 * g.rim[i] * 0.75 * lit);
        continue;
      }
      let u = (g.lon[i] + rot) / TAU + 0.5;
      u -= Math.floor(u);
      const tx = Math.min(W - 1, Math.floor(u * W));
      const j = g.row[i] * W + tx;
      const s4 = j * 4;
      const lam = g.nx[i] * L[0] + g.ny[i] * L[1] + g.nz[i] * L[2];
      // Posterised light so the terminator reads as pixel-art bands.
      let shade = lam <= 0 ? 0 : Math.ceil(Math.min(1, lam * 1.25) * 6) / 6;
      shade = (0.14 + 0.92 * shade) * (0.82 + 0.18 * g.nz[i]);
      let rr = data[s4] * shade, gg = data[s4 + 1] * shade, bb = data[s4 + 2] * (shade + (lam <= 0.05 ? 0.08 : 0));
      if (lights[j] && lam < 0.16) {
        const k = (lights[j] / 255) * Math.min(1, (0.16 - lam) * 8);
        rr = rr * (1 - k) + lightCol[j * 3] * k;
        gg = gg * (1 - k) + lightCol[j * 3 + 1] * k;
        bb = bb * (1 - k) + lightCol[j * 3 + 2] * k;
      }
      out[o] = rr; out[o + 1] = gg; out[o + 2] = bb; out[o + 3] = 255;
    }
    sc.ctx.putImageData(sc.img, 0, 0);
    const a = opts.alpha ?? 1;
    if (a <= 0) return;
    ctx.globalAlpha = a;
    ctx.drawImage(sc.c, Math.round(x) - g.half, Math.round(y) - g.half);
    ctx.globalAlpha = 1;
  }

  // Longitude rotation that brings map column `px` to face the viewer.
  static faceRot(px) { return ((px / W) - 0.5) * TAU; }
}

// Static + twinkling starfield for every cosmos view.
const STARS = (() => { const r = mulberry32(4242); return Array.from({ length: 170 }, () => ({ x: Math.floor(r() * W), y: Math.floor(r() * H), b: r(), p: r() * 6 })); })();
export function drawStars(ctx, time, drift = 0) {
  ctx.fillStyle = '#05060f';
  ctx.fillRect(0, 0, W, H);
  for (const s of STARS) {
    const tw = 0.6 + 0.4 * Math.sin(time * (1 + s.b * 2) + s.p);
    const v = s.b > 0.85 ? 255 : s.b > 0.55 ? 170 : 95;
    ctx.fillStyle = `rgba(${v},${v},${Math.min(255, v + 30)},${tw})`;
    const x = ((s.x - drift * s.b) % W + W) % W;
    ctx.fillRect(Math.floor(x), s.y, 1, 1);
  }
}
