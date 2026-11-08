// Fireball projectile (#315): a short, readable arc that ends in a ground slam. Pure timing and
// drawing in the overlay pass; scenes own the impact effects (particles, shake, fear, audio).
//
// 2D: the shot starts above the target in world/screen pixels and accelerates down.
// 3D: the target is projected to screen, then the orb falls from above that point, so it reads
// as a slam instead of a decal painted on the terrain.

import { clamp, lerp } from '../sim/util.js';
import { reducedMotion } from '../ui/motion.js';
import {
  drawFireballSprite, drawPixels, shockwavePixels, scorchPixels, shade,
  FB_CORE, FB_HOT, FB_SMOKE, FB_SCORCH, FB_DEBRIS,
} from './fireball-sprite.js';

// Pixel art (#319): the orb is the fireball sprite (spinning flame tongues, hot core, the emotion
// colour), the trail is embers and smoke, and the slam is staged: a short hold on the impact frame,
// then a stepped pixel shockwave on the ground, a fire burst, debris, dust and a scorch mark. Under
// reduced motion the slam is a static burst and scorch that fade. No smooth circles.

export const FIREBALL_FX = {
  travel: 0.58,
  impact: 1.0,
  hold: 0.1,         // s the impact frame holds before the burst spreads
  windup: 0.16,       // share of the flight spent pulling back before the drop
  windupLift: 6,      // px the orb rises while pulling back
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
  draw(ctx, { project = null, reduced = reducedMotion() } = {}) {
    if (!this.list.length) return;
    const proj = project || ctx.__project3d || null;
    for (const s of this.list) {
      const p = clamp(s.t / s.dur);
      const after = Math.max(0, s.t - s.dur);
      const target = proj ? proj(s.x1, s.y1) : [s.x1, s.y1];
      if (!target) continue;
      const [tx, ty] = target;
      const r = 2 + Math.round(s.charge * 2);
      if (s.t < s.dur) this.drawFlight(ctx, s, p, proj, tx, ty, r);
      else if (after <= this.opts.impact) this.drawImpact(ctx, s, after, tx, ty, r, reduced);
    }
  }

  drawFlight(ctx, s, p, proj, tx, ty, r) {
    const frame = Math.floor(s.t * 16) % 4;
    // Trail: embers in the charge colour and pale yellow, with a little grey smoke above them.
    for (let i = 6; i >= 1; i--) {
      const q = clamp(p - i * 0.045);
      const pos = this.position(s, q, proj, tx, ty);
      const fade = 1 - i / 7;
      drawPixels(ctx, pos.x, pos.y, [{ x: 0, y: 0, c: i % 2 ? 'tint' : 'hot' }], { tint: s.color, hot: FB_HOT }, { alpha: 0.85 * fade });
      if (i % 2 === 0) drawPixels(ctx, pos.x + (i % 4 ? 1 : -1), pos.y - 2 - i * 0.5, [{ x: 0, y: 0, c: 's' }], { s: FB_SMOKE }, { alpha: 0.4 * fade });
    }
    const pos = this.position(s, p, proj, tx, ty);
    drawFireballSprite(ctx, pos.x, pos.y, { r, frame, color: s.color });
  }

  drawImpact(ctx, s, after, tx, ty, r, reduced) {
    const o = this.opts;
    const k = clamp(after / o.impact);
    const frame = Math.floor(after * 14) % 4;
    const big = r + 4 + Math.round(s.charge * 4);
    // The scorch stays under everything and fades last.
    drawPixels(ctx, tx, ty + 1, scorchPixels(3 + Math.round(s.charge * 3)).map((q) => ({ ...q, c: 's' })), { s: FB_SCORCH }, { alpha: 0.55 * clamp((1 - k) * 2) });
    if (reduced) {
      // Static and readable: one burst that fades, no expanding ring or flying debris.
      drawFireballSprite(ctx, tx, ty - 2, { r: big - 1, frame: 0, color: s.color, alpha: 1 - k, tongues: false });
      return;
    }
    if (after < o.hold) {
      // Hold on the impact frame: the orb squashed into the ground, white-hot.
      drawFireballSprite(ctx, tx, ty - 1, { r: big, frame, color: s.color, tongues: false });
      return;
    }
    const k2 = clamp((after - o.hold) / (o.impact - o.hold));
    // A huge stepped pixel shockwave, with a white-hot inner blast.
    const R = 10 + k2 * (90 + s.charge * 100);
    const ring = shockwavePixels(R, k2).map((q) => ({ ...q, c: 'tint' }));
    drawPixels(ctx, tx, ty, ring, { tint: s.color }, { alpha: 1 - k2 });
    if (k2 < 0.7) drawPixels(ctx, tx, ty, shockwavePixels(R * 0.62, k2 + 0.3).map((q) => ({ ...q, c: 'hot' })), { hot: FB_HOT }, { alpha: 1 - k2 / 0.7 });
    if (k2 < 0.3) {
      drawPixels(ctx, tx, ty, shockwavePixels(R * 0.45, k2).map((q) => ({ ...q, c: 'core' })), { core: FB_CORE, hot: FB_HOT, tint: s.color }, { alpha: (1 - k2 / 0.3) * 0.9 });
    }
    // Fire burst rising and shrinking out of the crater.
    const burst = Math.max(1, Math.round(big * (1 - k2)));
    if (k2 < 0.85) drawFireballSprite(ctx, tx, ty - 2 - k2 * 7, { r: burst, frame, color: s.color, alpha: 1 - k2 * 0.6 });
    // Debris thrown up under gravity, and dust rolling out along the ground.
    const t = after - o.hold;
    const debris = [];
    for (let i = 0; i < 8; i++) {
      const vx = (i - 3.5) * (7 + s.charge * 4) + Math.sin(i * 7.3 + s.drift) * 3;
      const vy = 34 + (i % 3) * 10 + s.charge * 12;
      const y = -(vy * t - 70 * t * t);
      if (y > 1) continue;
      debris.push({ x: Math.round(vx * t), y: Math.round(Math.min(0, y)), c: `d${i % 3}` });
    }
    drawPixels(ctx, tx, ty - 1, debris, { d0: FB_DEBRIS[0], d1: FB_DEBRIS[1], d2: shade(s.color, 0.3) }, { alpha: 1 - k2 * 0.5 });
    const dust = [];
    for (let i = 0; i < 6; i++) {
      const side = i % 2 ? 1 : -1;
      dust.push({ x: Math.round(side * (3 + k2 * (10 + i * 3))), y: -Math.round(k2 * (i % 3)), c: 's' });
    }
    drawPixels(ctx, tx, ty, dust, { s: FB_DEBRIS[2] }, { alpha: 0.6 * (1 - k2) });
  }

  position(s, p, proj, tx, ty) {
    const o = this.opts;
    // A short wind-up (the pull back) before the drop, then an accelerating slam.
    const w = clamp(p / o.windup);
    const lift = o.windupLift * Math.sin(w * Math.PI / 2);
    const q = clamp((p - o.windup) / (1 - o.windup));
    if (proj) {
      // Screen-space slam: a small drift, then a vertical drop onto the projected ground point.
      return {
        x: tx + s.drift * (1 - q),
        y: ty - (1 - q) ** 2 * (o.screenLift + o.screenChargeLift * s.charge) - lift * (1 - q),
      };
    }
    return {
      x: lerp(s.x0, s.x1, q),
      y: lerp(s.y0 - lift, s.y1, q * q),
    };
  }
}
