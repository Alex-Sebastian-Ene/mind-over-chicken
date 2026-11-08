// Fireball projectile (#315): a short, readable arc that ends in a ground slam. Pure timing and
// drawing in the overlay pass; scenes own the impact effects (particles, shake, fear, audio).
//
// 2D: the shot starts above the target in world/screen pixels and accelerates down.
// 3D: the target is projected to screen, then the orb falls from above that point, so it reads
// as a slam instead of a decal painted on the terrain.

import { clamp, lerp } from '../sim/util.js';

export const FIREBALL_FX = {
  travel: 0.58,
  impact: 0.5,
  lift: 96,
  chargeLift: 64,
  screenLift: 42,
  screenChargeLift: 26,
  drift: 18,
};

export class FireballFX {
  constructor(options = {}) {
    this.opts = { ...FIREBALL_FX, ...options };
    this.list = [];
  }

  launch(target, { color = '#ff9a2e', charge = 1, from = null, duration = null, meta = null } = {}) {
    const c = clamp(Number(charge) || 0);
    const drift = (Math.random() - 0.5) * this.opts.drift;
    const start = from || { x: target.x + drift, y: target.y - (this.opts.lift + this.opts.chargeLift * c) };
    const shot = {
      x0: start.x, y0: start.y, x1: target.x, y1: target.y,
      t: 0, dur: duration ?? this.opts.travel, color, charge: c, drift, hit: false,
      ...(meta || {}),
    };
    this.list.push(shot);
    return shot;
  }

  // Advance the shots and return the ones that landed this frame (for the scene's impact hook).
  update(dt) {
    const hits = [];
    for (const s of this.list) {
      s.t += Math.max(0, Number(dt) || 0);
      if (!s.hit && s.t >= s.dur) {
        s.hit = true;
        hits.push({ ...s, x: s.x1, y: s.y1 });
      }
    }
    this.list = this.list.filter((s) => s.t < s.dur + this.opts.impact);
    return hits;
  }

  get active() { return this.list.length; }

  // Draw in the scene's overlay pass. `ctx.__project3d` is set for the 3D view by main.js.
  draw(ctx, { project = null } = {}) {
    if (!this.list.length) return;
    const proj = project || ctx.__project3d || null;
    for (const s of this.list) {
      const p = clamp(s.t / s.dur);
      const after = Math.max(0, s.t - s.dur);
      const target = proj ? proj(s.x1, s.y1) : [s.x1, s.y1];
      if (!target) continue;
      const [tx, ty] = target;

      if (s.t < s.dur) {
        for (let i = 5; i >= 1; i--) {
          const q = clamp(p - i * 0.06);
          const pos = this.position(s, q, proj, tx, ty);
          this.spark(ctx, pos.x, pos.y, s.color, 0.42 * (1 - i / 6));
        }
        const pos = this.position(s, p, proj, tx, ty);
        this.orb(ctx, pos.x, pos.y, s.color, s.charge, 1);
        continue;
      }

      if (after <= this.opts.impact) {
        const k = clamp(after / this.opts.impact);
        const r = 2 + k * (24 + s.charge * 22);
        ctx.save();
        // Ground flash: a short, bright pool at the impact point.
        ctx.globalAlpha = (1 - k) * 0.32;
        ctx.fillStyle = s.color;
        ctx.fillRect(Math.round(tx - r * 0.42), Math.round(ty - 2), Math.round(r * 0.84), 4);
        // Two expanding rings: the slam's shockwave, then a hot inner ring.
        ctx.globalAlpha = (1 - k) * 0.95;
        ctx.strokeStyle = s.color;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(Math.round(tx), Math.round(ty), Math.round(r), 0, Math.PI * 2);
        ctx.stroke();
        ctx.globalAlpha = (1 - k) * 0.65;
        ctx.strokeStyle = '#fff4c2';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(Math.round(tx), Math.round(ty), Math.round(r * 0.62), 0, Math.PI * 2);
        ctx.stroke();
        // A few embers thrown up from the crater.
        for (let i = 0; i < 5; i++) {
          const a = -Math.PI / 2 + (i - 2) * 0.5;
          const d = r * (0.35 + k * 0.55);
          this.spark(ctx, tx + Math.cos(a) * d, ty - Math.abs(Math.sin(a)) * d, i % 2 ? '#fff4c2' : s.color, (1 - k) * 0.9);
        }
        ctx.restore();
      }
    }
  }

  position(s, p, proj, tx, ty) {
    if (proj) {
      // Screen-space slam: a small drift, then a vertical drop onto the projected ground point.
      return {
        x: tx + s.drift * (1 - p),
        y: ty - (1 - p) ** 2 * (this.opts.screenLift + this.opts.screenChargeLift * s.charge),
      };
    }
    return {
      x: lerp(s.x0, s.x1, p),
      y: lerp(s.y0, s.y1, p * p),
    };
  }

  orb(ctx, x, y, color, charge, alpha = 1) {
    const r = Math.max(2, Math.round(2 + charge * 2.5));
    const px = Math.round(x), py = Math.round(y);
    ctx.save();
    ctx.globalAlpha = alpha;
    // Soft outer glow, dark outline, hot core, white centre. Pixel squares keep the art style.
    ctx.fillStyle = color;
    ctx.globalAlpha = alpha * 0.22;
    ctx.fillRect(px - r - 2, py - r - 2, r * 2 + 4, r * 2 + 4);
    ctx.globalAlpha = alpha * 0.9;
    ctx.fillStyle = 'rgba(8,10,24,0.7)';
    ctx.fillRect(px - r - 1, py - r - 1, r * 2 + 2, r * 2 + 2);
    ctx.fillStyle = color;
    ctx.fillRect(px - r, py - r, r * 2, r * 2);
    ctx.fillStyle = '#fff4c2';
    ctx.fillRect(px - Math.max(1, r - 1), py - Math.max(1, r - 1), Math.max(2, r), Math.max(2, r));
    ctx.restore();
  }

  spark(ctx, x, y, color, alpha = 1) {
    ctx.save();
    ctx.globalAlpha = clamp(alpha);
    ctx.fillStyle = color;
    ctx.fillRect(Math.round(x), Math.round(y), 1, 1);
    ctx.restore();
  }
}
