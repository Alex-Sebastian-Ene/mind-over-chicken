// Charged fireball (#291): the one deliberate hand cast. Pure state machine and colour mapping,
// no DOM, so Node can test it:
//
//   idle → (fist held 180 ms) → charging → (pull back / time) → primed
//   primed → (palm opens within 350 ms) → released (one fireball)
//   anything → pinch / hand lost / fist relaxed / palm without thrust → cancelled
//
// Depth uses pinchMetrics().size (wrist→middle-MCP, aspect-corrected), never raw z alone.

import { clamp } from '../sim/util.js';
import { pinchMetrics } from './fingers.js';
import { SURPRISE_FULL, surpriseNormalized } from './emotion.js';

export const FIREBALL = {
  chargeHoldMs: 180,
  releaseWindowMs: 350,
  cooldownMs: 1000,
  cancelCooldownMs: 450,
  minCharge: 0.15,
  pullFull: 0.35,
  thrustVelocity: 1.2,
  fistScore: 0.55,
  palmScore: 0.55,
  chargeTime: 2.5,
  colorTau: 0.2,
};

const EMBER = [255, 154, 46];
const HAPPY = [255, 59, 48];
const SURPRISED = [77, 163, 255];

const clamp01 = (v) => clamp(Number(v) || 0);
const mix = (a, b, t) => a.map((v, i) => Math.round(v + (b[i] - v) * clamp01(t)));
const hex = (c) => `#${c.map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0')).join('')}`;
const rgb = (h) => {
  const m = /^#?([0-9a-f]{6})$/i.exec(h || '');
  return m ? [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16)) : EMBER;
};

// Joy 0..1 → red, surprise normalised by its practical 0.2 ceiling → blue, neutral → ember.
export function fireballColor(mood = {}) {
  const joy = clamp01(mood.joy);
  const surprise = clamp01(surpriseNormalized(mood.surprise));
  const total = joy + surprise;
  if (total < 0.05) return hex(EMBER);
  const wj = joy / total, ws = surprise / total;
  const hue = mix(HAPPY, SURPRISED, ws);          // joy alone red, surprise alone blue
  const strength = clamp01(total * 1.5);          // low emotion stays ember
  return hex(mix(EMBER, hue, strength));
}

export class FireballController {
  constructor(options = {}) {
    this.opts = { ...FIREBALL, ...options };
    this.reset();
  }

  reset() {
    this.state = 'idle';
    this.charge = 0;
    this.color = hex(EMBER);
    this.cast = false;
    this.cancelled = false;
    this.fizzle = false;
    this.lastNow = 0;
    this.fistSince = 0;
    this.neutralSize = 0;
    this.lastSize = 0;
    this.chargeSince = 0;
    this.primedAt = 0;
    this.cooldownUntil = 0;
    this.keyboard = false;
  }

  update({ hand = null, gesture = {}, mood = {}, now = performance.now(), aspect = 4 / 3, pinching = false } = {}) {
    const o = this.opts;
    const dt = this.lastNow ? clamp((now - this.lastNow) / 1000, 0, 0.2) : 0.016;
    this.lastNow = now;
    this.cast = false;
    this.cancelled = false;
    this.fizzle = false;
    this.color = hex(mix(rgb(this.color), rgb(fireballColor(mood)), 1 - Math.exp(-dt / o.colorTau)));

    if (pinching) { this.cancel(now); return this.snapshot(); }
    if (this.keyboard) { this.updateKeyboardCharge(now); return this.snapshot(); }
    if (now < this.cooldownUntil) { this.state = 'cooldown'; this.charge = Math.max(0, this.charge - dt * 3); return this.snapshot(); }
    if (this.state === 'cooldown') this.state = 'idle';

    const m = hand && hand.length >= 21 ? pinchMetrics(hand, aspect) : null;
    if (!m) { if (this.state === 'charging' || this.state === 'primed') this.cancel(now); return this.snapshot(m, 0, false); }
    const size = m.size;
    const fist = gesture?.name === 'Closed_Fist' && (gesture.score || 0) >= o.fistScore;
    const palm = (gesture?.name === 'Open_Palm' && (gesture.score || 0) >= o.palmScore)
      || (m.reach > 1.35 && !fist);
    const thrust = this.lastSize ? (size - this.lastSize) / Math.max(dt, 0.001) / Math.max(this.neutralSize || size, 1e-6) : 0;
    const snapshot = (mm = m, th = thrust, p = palm) => this.snapshot(mm, th, p);

    if (this.state === 'idle') {
      if (!fist) { this.fistSince = 0; return snapshot(); }
      if (!this.fistSince) this.fistSince = now;
      if (now - this.fistSince >= o.chargeHoldMs) {
        this.state = 'charging';
        this.charge = 0;
        this.neutralSize = size;
        this.lastSize = size;
        this.chargeSince = now;
      }
      return snapshot();
    }

    if (this.state === 'charging') {
      if (!fist || palm) { this.cancel(now); return snapshot(); }
      const pull = clamp01((this.neutralSize - size) / Math.max(this.neutralSize * o.pullFull, 1e-6));
      this.charge = clamp01(this.charge + dt / o.chargeTime * (0.25 + 0.75 * pull));
      this.lastSize = size;
      if (thrust >= o.thrustVelocity && this.charge >= o.minCharge) {
        this.state = 'primed';
        this.primedAt = now;
      }
      return snapshot(m, thrust, palm);
    }

    if (this.state === 'primed') {
      if (!hand) { this.cancel(now); return snapshot(); }
      if (palm) {
        if (this.charge >= o.minCharge) {
          this.cast = true;
          this.state = 'cooldown';
          this.cooldownUntil = now + o.cooldownMs;
        } else {
          this.cancelled = true;
          this.fizzle = true;
          this.state = 'cooldown';
          this.cooldownUntil = now + o.cancelCooldownMs;
        }
        return snapshot(m, thrust, palm);
      }
      if (!fist) { this.cancel(now); return snapshot(); }
      this.lastSize = size;
      if (now - this.primedAt > o.releaseWindowMs) this.state = 'charging';
      return snapshot(m, thrust, palm);
    }

    return snapshot(m, thrust, palm);
  }

  snapshot(m = null, thrust = 0, palm = false) {
    return {
      state: this.state,
      charge: this.charge,
      color: this.color,
      cast: this.cast,
      cancelled: this.cancelled,
      fizzle: this.fizzle,
      size: m?.size ?? null,
      thrust,
      palm,
    };
  }

  cancel(now) {
    if (this.state === 'charging' || this.state === 'primed') {
      this.cancelled = true;
      this.fizzle = true;
    }
    this.state = 'cooldown';
    this.charge = 0;
    this.fistSince = 0;
    this.primedAt = 0;
    this.cooldownUntil = now + this.opts.cancelCooldownMs;
  }

  keyboardDown(mood = {}, now = performance.now()) {
    if (now < this.cooldownUntil) return;
    this.keyboard = true;
    this.state = 'charging';
    this.charge = 0;
    this.chargeSince = now;
    this.color = fireballColor(mood);
  }

  updateKeyboardCharge(now) {
    if (!this.keyboard) return;
    this.charge = clamp01((now - this.chargeSince) / (this.opts.chargeTime * 1000));
    if (now < this.cooldownUntil) { this.keyboard = false; this.state = 'cooldown'; }
  }

  keyboardUp(mood = {}, now = performance.now()) {
    if (!this.keyboard) return this.snapshot();
    this.keyboard = false;
    this.cast = this.charge >= this.opts.minCharge;
    if (this.cast) {
      this.state = 'cooldown';
      this.cooldownUntil = now + this.opts.cooldownMs;
    } else {
      this.cancelled = true;
      this.fizzle = true;
      this.state = 'cooldown';
      this.cooldownUntil = now + this.opts.cancelCooldownMs;
    }
    return this.snapshot();
  }
}

export { SURPRISE_FULL };
