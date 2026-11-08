// Pixel-art fireball (#319): one sprite set for the charge-up, the flight and the slam, drawn as
// whole game pixels so it matches the world. FireballFX draws it in the overlay pass (projected to
// the screen in 3D, so 2D and 3D share the same art), and the camera panel and the reticle draw the
// same pixels scaled up. No smooth circles anywhere: every shape is a set of integer pixels.

export const FB_CORE = '#ffffff';
export const FB_HOT = '#fff4c2';
export const FB_SMOKE = '#5a5560';
export const FB_SCORCH = '#1d1410';
export const FB_DEBRIS = ['#3b2a1e', '#6b4a2e', '#8a7a5a'];

// Mix a #rrggbb colour toward black by k (0..1): the sprite's dark edge in the charge colour.
export function shade(hex, k = 0.5) {
  const m = /^#?([0-9a-f]{6})$/i.exec(String(hex || ''));
  if (!m) return '#4a1a08';
  const n = parseInt(m[1], 16);
  const ch = (v) => Math.round(v * (1 - k)).toString(16).padStart(2, '0');
  return `#${ch((n >> 16) & 255)}${ch((n >> 8) & 255)}${ch(n & 255)}`;
}

// The orb's pixels for radius r (1..8) and animation frame (0..3), centred on 0,0. Each pixel says
// which ring it belongs to: 'core' (white), 'hot' (pale yellow), 'tint' (the emotion colour) or
// 'edge' (dark tint). With tongues, three flame licks orbit the rim, a quarter turn per frame.
const ORB_CACHE = new Map();
export function orbPixels(r, frame = 0, { tongues = true } = {}) {
  r = Math.max(1, Math.min(8, Math.round(r)));
  frame = ((Math.floor(frame) % 4) + 4) % 4;
  const key = `${r}:${frame}:${tongues ? 1 : 0}`;
  if (ORB_CACHE.has(key)) return ORB_CACHE.get(key);
  const out = [];
  for (let y = -r - 1; y <= r + 1; y++) {
    for (let x = -r - 1; x <= r + 1; x++) {
      const d = Math.hypot(x, y);
      if (d <= r * 0.34 + 0.2) out.push({ x, y, c: 'core' });
      else if (d <= r * 0.68 + 0.2) out.push({ x, y, c: 'hot' });
      else if (d <= r + 0.25) out.push({ x, y, c: 'tint' });
      else if (d <= r + 0.95 && r >= 2) out.push({ x, y, c: 'edge' });
    }
  }
  if (tongues) {
    const seen = new Set(out.map((p) => `${p.x},${p.y}`));
    for (let k = 0; k < 3; k++) {
      const a = frame * (Math.PI / 2) + (k * Math.PI * 2) / 3 - Math.PI / 2;
      for (const [rr, c] of [[r + 1, 'tint'], [r + 2, 'edge']]) {
        const x = Math.round(Math.cos(a) * rr), y = Math.round(Math.sin(a) * rr);
        const id = `${x},${y}`;
        const at = out.findIndex((p) => p.x === x && p.y === y);
        if (at >= 0) out[at] = { x, y, c };
        else if (!seen.has(id)) { out.push({ x, y, c }); seen.add(id); }
      }
    }
  }
  ORB_CACHE.set(key, out);
  return out;
}

// Draw a pixel list at (x, y): `scale` screen pixels per game pixel, colours from the palette.
export function drawPixels(ctx, x, y, pixels, palette, { scale = 1, alpha = 1 } = {}) {
  if (!(alpha > 0)) return;
  const s = Math.max(1, Math.round(scale));
  const ox = Math.round(x / s) * s, oy = Math.round(y / s) * s;
  const prev = ctx.globalAlpha;
  ctx.globalAlpha = prev * Math.min(1, alpha);
  for (const p of pixels) {
    const col = palette[p.c] ?? palette.tint;
    if (!col) continue;
    ctx.fillStyle = col;
    ctx.fillRect(ox + p.x * s, oy + p.y * s, s, s);
  }
  ctx.globalAlpha = prev;
}

export function fireballPalette(color) {
  return { core: FB_CORE, hot: FB_HOT, tint: color, edge: shade(color, 0.55) };
}

export function drawFireballSprite(ctx, x, y, { r = 3, frame = 0, color = '#ff9a2e', scale = 1, alpha = 1, tongues = true } = {}) {
  drawPixels(ctx, x, y, orbPixels(r, frame, { tongues }), fireballPalette(color), { scale, alpha });
}

// The charge-up (formation) at charge 0..1 and time t (s): the orb grows from a spark (r 1) to a
// full fireball (r 4); embers spiral in toward it, faster as it charges; at full charge it strains,
// pulsing a pixel bigger and smaller. Ember positions are in game pixels around the orb.
export function formation(charge, t = 0) {
  const c = Math.min(1, Math.max(0, Number(charge) || 0));
  const full = c >= 0.95;
  const pulse = full && Math.sin(t * 18) > 0 ? 1 : 0;
  const r = 1 + Math.round(c * 3) + pulse;
  const n = 6;
  const reach = 6 + c * 5;
  const spin = 3 + c * 6;
  const embers = [];
  for (let i = 0; i < n; i++) {
    const life = (t * (0.9 + c * 1.4) + i / n) % 1;      // 0 at the rim, 1 at the orb
    const d = r + 1 + (reach - r - 1) * (1 - life);
    const a = t * spin + (i * Math.PI * 2) / n + life * 2.2;
    embers.push({ x: Math.round(Math.cos(a) * d), y: Math.round(Math.sin(a) * d), hot: i % 2 === 0, life });
  }
  return { r, pulse, full, embers, frame: Math.floor(t * (8 + c * 10)) % 4 };
}

// A stepped ground shockwave of radius R (game pixels), flattened 2:1 because it lies on the
// ground. As it fades (k 0..1) it dithers: every pixel, then every other, then one in four.
export function shockwavePixels(R, k = 0) {
  R = Math.max(1, Math.round(R));
  const step = k > 0.75 ? 4 : k > 0.45 ? 2 : 1;
  const seen = new Set();
  const out = [];
  const n = Math.max(8, Math.round(R * 6.3));
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const x = Math.round(Math.cos(a) * R), y = Math.round(Math.sin(a) * R * 0.5);
    const id = `${x},${y}`;
    if (seen.has(id)) continue;
    seen.add(id);
    out.push({ x, y });
  }
  return out.filter((_, i) => i % step === 0);
}

// A flattened scorch mark of radius R: filled, dithered at the rim.
export function scorchPixels(R) {
  R = Math.max(1, Math.round(R));
  const out = [];
  for (let y = -Math.ceil(R / 2); y <= Math.ceil(R / 2); y++) {
    for (let x = -R; x <= R; x++) {
      const d = Math.hypot(x / R, y / (R * 0.5));
      if (d <= 0.7 || (d <= 1 && (x + y) % 2 === 0)) out.push({ x, y });
    }
  }
  return out;
}

// How many of n chunky segments a charge fills.
export function ringSegments(charge, n = 16) {
  return Math.max(0, Math.min(n, Math.floor((Math.min(1, Math.max(0, Number(charge) || 0)) + 1e-9) * n)));
}

// A segmented pixel ring (the charge meter and the grab ring): n hard-edged blocks round a circle of
// radius R game pixels; the first ringSegments(charge) are lit, the rest faint.
export function drawSegmentRing(ctx, x, y, R, charge, color, { scale = 1, n = 16, base = 'rgba(242,232,213,0.28)', block = 2 } = {}) {
  const s = Math.max(1, Math.round(scale));
  const lit = ringSegments(charge, n);
  const prev = ctx.globalAlpha;
  for (let i = 0; i < n; i++) {
    const a = -Math.PI / 2 + ((i + 0.5) / n) * Math.PI * 2;
    const px = Math.round(x / s + Math.cos(a) * R - block / 2) * s;
    const py = Math.round(y / s + Math.sin(a) * R - block / 2) * s;
    ctx.fillStyle = i < lit ? color : base;
    ctx.fillRect(px, py, block * s, block * s);
  }
  ctx.globalAlpha = prev;
}
