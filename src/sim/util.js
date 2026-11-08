// Small math, randomness and particle helpers shared by every era.

// The world is drawn at a low pixel resolution and scaled up with nearest-neighbour
// filtering, so every sprite is honest pixel art.
export const W = 320;   // world width in pixels
export const H = 180;   // world height in pixels

// The map is a torus (#111): wrap a coordinate into [0, size), and the shorter signed way
// round from b to a.
export const wrap = (v, size) => { const r = v % size; return r < 0 ? r + size : r; };
export const wrapDelta = (a, b, size) => { let d = (a - b) % size; if (d > size / 2) d -= size; else if (d < -size / 2) d += size; return d; };

export const clamp = (v, lo = 0, hi = 1) => (v < lo ? lo : v > hi ? hi : v);
export const lerp = (a, b, t) => a + (b - a) * t;
export const smooth = (current, target, rate, dt) => current + (target - current) * (1 - Math.exp(-rate * dt));
export const dist = (ax, ay, bx, by) => Math.hypot(ax - bx, ay - by);
export const sigmoid = (x) => 1 / (1 + Math.exp(-x));
export const rand = (lo = 0, hi = 1) => lo + Math.random() * (hi - lo);
export const randInt = (lo, hi) => Math.floor(rand(lo, hi + 1));
export const pick = (arr) => arr[Math.floor(Math.random() * arr.length)];
export const chance = (p) => Math.random() < p;

// Gaussian noise (Box-Muller)
export function gauss() {
  let u = 0, v = 0;
  while (u === 0) u = Math.random();
  while (v === 0) v = Math.random();
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}

// Seeded RNG, used for stable scenery so hills don't jitter between frames.
export function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Move (x,y) toward (tx,ty) at speed; returns true on arrival.
export function stepToward(obj, tx, ty, speed, dt) {
  const dx = tx - obj.x, dy = ty - obj.y;
  const d = Math.hypot(dx, dy);
  if (d < 2) { obj.x = tx; obj.y = ty; obj.vx = 0; return true; }
  const s = Math.min(d, speed * dt);
  obj.x += (dx / d) * s;
  obj.y += (dy / d) * s;
  obj.vx = dx / d;
  return false;
}

export function median(arr) {
  if (!arr.length) return 0;
  const s = [...arr].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

// Tiny particle system. Each era owns one.
export class Particles {
  constructor() { this.list = []; }
  add(p) {
    this.list.push({ life: 1, maxLife: 1, vx: 0, vy: 0, g: 0, drag: 0, size: 3, grow: 0, color: '#fff', alpha: 1, shape: 'circle', ...p });
    if (this.list.length > 900) this.list.splice(0, this.list.length - 900);
  }
  burst(n, fn) { for (let i = 0; i < n; i++) this.add(fn(i)); }
  update(dt) {
    const out = [];
    for (const p of this.list) {
      p.life -= dt;
      if (p.life <= 0) continue;
      p.vy += p.g * dt;
      p.vx *= 1 - p.drag * dt;
      p.vy *= 1 - p.drag * dt;
      p.x += p.vx * dt;
      p.y += p.vy * dt;
      p.size = Math.max(0.1, p.size + p.grow * dt);
      out.push(p);
    }
    this.list = out;
  }
  // Pixel pass: every particle is a crisp square on the low-res world canvas.
  draw(ctx) {
    for (const p of this.list) {
      if (p.shape === 'text') continue;
      const t = clamp(p.life / p.maxLife);
      ctx.globalAlpha = p.alpha * (p.fade === false ? 1 : t);
      ctx.fillStyle = typeof p.color === 'function' ? p.color(t) : p.color;
      const s = Math.max(1, Math.round(p.size));
      ctx.fillRect(Math.round(p.x - s / 2), Math.round(p.y - s / 2), s, s);
    }
    ctx.globalAlpha = 1;
  }
  // Overlay pass: floating text, drawn crisp on the full-resolution canvas (world units).
  drawText(ctx) {
    ctx.textAlign = 'center';
    for (const p of this.list) {
      if (p.shape !== 'text') continue;
      const t = clamp(p.life / p.maxLife);
      ctx.globalAlpha = t;
      ctx.font = `${p.size}px "Pixelify Sans", monospace`;
      let x = p.x, y = p.y;
      if (ctx.__project3d) {
        const pr = ctx.__project3d(x, y);
        if (pr) { x = pr[0]; y = pr[1]; }
      }
      ctx.fillStyle = 'rgba(10,10,20,0.85)';
      ctx.fillText(p.text, x + 0.5, y + 0.5);
      ctx.fillStyle = p.color;
      ctx.fillText(p.text, x, y);
    }
    ctx.globalAlpha = 1;
  }
}

// Floating text, used for "+1 baby", "-12%" and other little popups.
export function floater(particles, x, y, text, color = '#f2e8d5', size = 6) {
  particles.add({ x, y, vy: -9, life: 1.8, maxLife: 1.8, shape: 'text', text, color, size });
}
