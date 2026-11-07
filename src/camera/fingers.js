// camera debug (#67): finger actions. Pure logic, no DOM, so Node can unit-test it.
//
//   OneEuroFilter / PointFilter  low-latency smoothing (Casiez et al., CHI 2012): a slow hand gets
//                                a low cutoff, so jitter disappears; a fast hand gets a high one, so
//                                the reticle doesn't lag.
//   softDeadZone                 the reticle ignores movements smaller than a radius, and doesn't
//                                jump when it starts moving again.
//   HandRange                    learns how far this player's fingertip actually reaches over the
//                                first seconds (min/max extents, slow decay), instead of fixed bounds.
//   pinchMetrics / PinchDetector thumb tip (4) to index tip (8), relative to the hand's size, with
//                                hysteresis and debounce.
//   FingerController             puts them together: pointing aims, pinch-and-hold aims precisely,
//                                releasing the pinch confirms the armed cast.
//
// Landmarks are MediaPipe's: x and y normalised to the frame (0..1), z roughly in x's scale.

import { clamp } from '../sim/util.js';

const TAU = 2 * Math.PI;
const alpha = (cutoff, dt) => { const r = TAU * cutoff * dt; return r / (r + 1); };

// ---------- smoothing ----------
// One Euro filter on one value. Times are milliseconds; cutoffs are Hz; beta is per (unit / s).
export class OneEuroFilter {
  constructor({ minCutoff = 1.2, beta = 8, dCutoff = 1, maxGapMs = 500 } = {}) {
    Object.assign(this, { minCutoff, beta, dCutoff, maxGapMs });
    this.reset();
  }

  reset() { this.x = null; this.dx = 0; this.t = null; }

  filter(value, tMs) {
    if (this.x == null || tMs - this.t > this.maxGapMs) { this.x = value; this.dx = 0; this.t = tMs; return value; }
    const dt = (tMs - this.t) / 1000;
    if (!(dt > 0)) return this.x;          // a duplicate or out-of-order sample changes nothing
    this.t = tMs;
    this.dx += alpha(this.dCutoff, dt) * ((value - this.x) / dt - this.dx);
    this.x += alpha(this.minCutoff + this.beta * Math.abs(this.dx), dt) * (value - this.x);
    return this.x;
  }
}

// One Euro on a 2D point.
export class PointFilter {
  constructor(options) { this.fx = new OneEuroFilter(options); this.fy = new OneEuroFilter(options); }
  reset() { this.fx.reset(); this.fy.reset(); }
  filter(p, tMs) { return { x: this.fx.filter(p.x, tMs), y: this.fy.filter(p.y, tMs) }; }
}

// Radial soft dead zone: `out` only moves once `target` is more than `radius` away, and then
// trails it by exactly `radius`, so there is no jump at the edge of the zone.
export function softDeadZone(out, target, radius) {
  if (!out) return { x: target.x, y: target.y };
  const dx = target.x - out.x, dy = target.y - out.y, d = Math.hypot(dx, dy);
  if (d <= radius) return out;
  const k = (d - radius) / d;
  return { x: out.x + dx * k, y: out.y + dy * k };
}

// ---------- reachable range ----------
// Learns the box the fingertip actually covers. Until a hand has been seen for `learnMs`, the
// mapping blends from the old fixed bounds to the learned box. The box grows at once when the
// fingertip goes further, and after learning it slowly shrinks towards where the hand is
// (time constant `tauMs`), so one stray sample doesn't widen it for good. A minimum span keeps
// a still hand from turning tracking noise into big reticle moves, and an inner margin means the
// player reaches the map's edges without stretching to their limit.
export const FIXED_RANGE = { x: [0.15, 0.85], y: [0.12, 0.72] };

export class HandRange {
  constructor({ learnMs = 3000, tauMs = 30000, minSpan = { x: 0.2, y: 0.16 }, margin = 0.08, fixed = FIXED_RANGE, maxStepMs = 200 } = {}) {
    Object.assign(this, { learnMs, tauMs, minSpan, margin, fixed, maxStepMs });
    this.reset();
  }

  reset() { this.lo = null; this.hi = null; this.seenMs = 0; this.lastT = null; }

  get progress() { return this.learnMs > 0 ? clamp(this.seenMs / this.learnMs) : 1; }
  get calibrated() { return this.seenMs >= this.learnMs; }

  // p: fingertip in mirrored frame coordinates. Call it only while a hand is visible.
  update(p, tMs) {
    const x = clamp(p.x), y = clamp(p.y);
    const dt = this.lastT == null ? 0 : clamp(tMs - this.lastT, 0, this.maxStepMs); // a lost hand doesn't count
    this.lastT = tMs;
    if (!this.lo) { this.lo = { x, y }; this.hi = { x, y }; return; }
    const learned = this.calibrated;
    this.seenMs += dt;
    const k = learned && dt > 0 ? 1 - Math.exp(-dt / this.tauMs) : 0;
    for (const [a, v] of [['x', x], ['y', y]]) {
      this.lo[a] = Math.min(v, this.lo[a] + (v - this.lo[a]) * k);
      this.hi[a] = Math.max(v, this.hi[a] + (v - this.hi[a]) * k);
    }
  }

  // [lo, hi] actually used for one axis.
  bounds(axis) {
    const fixed = this.fixed[axis];
    if (!this.lo) return fixed.slice();
    let a = this.lo[axis], b = this.hi[axis];
    const min = this.minSpan[axis];
    if (b - a < min) { const c = (a + b) / 2; a = c - min / 2; b = c + min / 2; }
    const m = (b - a) * this.margin;
    a += m; b -= m;
    const w = this.progress;
    return [fixed[0] + (a - fixed[0]) * w, fixed[1] + (b - fixed[1]) * w];
  }

  // Fingertip → 0..1 on the map. clamped=false keeps going past the edges (used for deltas).
  map(p, clamped = true) {
    const [x0, x1] = this.bounds('x'), [y0, y1] = this.bounds('y');
    const x = (p.x - x0) / (x1 - x0), y = (p.y - y0) / (y1 - y0);
    return clamped ? { x: clamp(x), y: clamp(y) } : { x, y };
  }

  snapshot() {
    const r = (v) => Math.round(v * 1000) / 1000;
    const [x0, x1] = this.bounds('x'), [y0, y1] = this.bounds('y');
    return { x: [r(x0), r(x1)], y: [r(y0), r(y1)], progress: r(this.progress), calibrated: this.calibrated };
  }
}

// ---------- pinch ----------
// Distances in frame pixels divided by the frame height, so a 4:3 frame isn't skewed: x and z are
// scaled by the aspect ratio (MediaPipe's z uses roughly the same scale as x).
function dist3(a, b, aspect) {
  return Math.hypot((a.x - b.x) * aspect, a.y - b.y, ((a.z || 0) - (b.z || 0)) * aspect);
}

// ratio: thumb tip to index tip over the hand's size. Size is the larger of the palm's length
// (wrist → middle knuckle) and its scaled width (index → pinky knuckle), so tilting the hand one
// way doesn't shrink it. reach: wrist → index tip over wrist → index knuckle; a fist curls the index
// tip back into the palm (reach well under 1), a pinch keeps it out in front.
export function pinchMetrics(hand, aspect = 4 / 3) {
  if (!hand || hand.length < 21) return null;
  const size = Math.max(dist3(hand[0], hand[9], aspect), 1.4 * dist3(hand[5], hand[17], aspect), 1e-6);
  return {
    ratio: dist3(hand[4], hand[8], aspect) / size,
    reach: dist3(hand[0], hand[8], aspect) / Math.max(dist3(hand[0], hand[5], aspect), 1e-6),
    point: { x: (hand[4].x + hand[8].x) / 2, y: (hand[4].y + hand[8].y) / 2 },
  };
}

export const PINCH = {
  enter: 0.3,     // ratio below which the fingers count as touching
  exit: 0.45,     // ratio above which they count as apart again (hysteresis)
  enterMs: 50,    // closed this long (about two frames) before the pinch starts
  exitMs: 60,     // open this long before it is released
  lossMs: 300,    // hand lost this long while pinching: cancel, don't cast
  minReach: 0.9,  // index tip must be out in front of its knuckle (not curled into a fist)
};

// Hysteresis + debounce on the ratio. update(null) means no hand this frame.
// Returns { active, started, released, cancelled, heldMs }.
export class PinchDetector {
  constructor(options = {}) { Object.assign(this, PINCH, options); this.reset(); }

  reset() { this.active = false; this.since = 0; this.closedAt = null; this.openAt = null; this.lastSeen = -Infinity; }

  update(ratio, now, { canStart = true } = {}) {
    const ev = { active: false, started: false, released: false, cancelled: false, heldMs: 0 };
    if (ratio == null) {
      this.closedAt = this.openAt = null;
      if (this.active && now - this.lastSeen > this.lossMs) { this.active = false; ev.cancelled = true; ev.heldMs = now - this.since; }
    } else {
      this.lastSeen = now;
      if (!this.active) {
        if (ratio < this.enter && canStart) {
          this.closedAt ??= now;
          if (now - this.closedAt >= this.enterMs) { this.active = true; this.since = now; this.openAt = null; ev.started = true; }
        } else this.closedAt = null;
      } else if (ratio > this.exit) {
        this.openAt ??= now;
        if (now - this.openAt >= this.exitMs) { this.active = false; this.closedAt = null; ev.released = true; ev.heldMs = now - this.since; }
      } else this.openAt = null;
    }
    ev.active = this.active;
    if (ev.active) ev.heldMs = now - this.since;
    return ev;
  }
}

let RustFingerControllerClass = null;

export function setRustFingerController(cls) {
  RustFingerControllerClass = cls;
}

// ---------- everything together ----------
// update() takes one hand-tracking result and returns what the reticle and the casts should do:
//   mode      'none' | 'point' | 'pinch' | 'release'
//   aim       new reticle position (0..1 on the map) or null to leave it alone
//   cast      true on the frame a pinch is released after at least minPinchMs: cast the armed spell at `aim`
//   cancelled true when a pinch ended without casting (hand lost, or a too-short tap)
//   suppress  true while gesture casts must be ignored (pinching, about to pinch, just released)
//   progress  0..1 of the pinch's minimum hold, for the ring around the reticle
export class FingerController {
  constructor({ aimScore = 0.45, pointGraceMs = 200, preciseGain = 0.35, deadZone = 0.004, minPinchMs = 150, refractoryMs = 300,
    fistScore = 0.5, filter = {}, pinchFilter = { minCutoff: 0.6, beta: 4 }, range = {}, pinch = {} } = {}) {
    if (RustFingerControllerClass) {
      try {
        this.wasm = new RustFingerControllerClass();
      } catch (e) {
        this.wasm = null;
      }
    }
    Object.assign(this, { aimScore, pointGraceMs, preciseGain, deadZone, minPinchMs, refractoryMs, fistScore });
    this.filter = new PointFilter(filter);
    this.pinchFilter = new PointFilter(pinchFilter);
    this.range = new HandRange(range);
    this.pinch = new PinchDetector(pinch);
    this.reset();
  }

  reset() {
    if (this.wasm) this.wasm.reset();
    this.filter.reset(); this.pinchFilter.reset(); this.range.reset(); this.pinch.reset();
    this.out = null;            // reticle output while pointing (dead-zoned)
    this.anchor = null;         // { aim, hand } where the pinch started
    this.locked = null;         // reticle while pinching; frozen once the fingers start to open
    this.pointUntil = -Infinity;
    this.releasedAt = -Infinity;
    this.last = null;
  }

  update({ hand, gesture = { name: 'None', score: 0 }, aim = { x: 0.5, y: 0.55 }, now, aspect = 4 / 3 }) {
    if (this.wasm) {
      let coords = null;
      if (hand && hand.length >= 21) {
        coords = new Float64Array(63);
        for (let i = 0; i < 21; i++) {
          coords[i * 3] = hand[i].x;
          coords[i * 3 + 1] = hand[i].y;
          coords[i * 3 + 2] = hand[i].z || 0;
        }
      }
      const res = this.wasm.update(coords, gesture.name || 'None', gesture.score || 0, now, aspect);
      const mode = res[0];
      const resAim = res[1] ? { x: res[1][0], y: res[1][1] } : null;
      const cast = res[2];
      const cancelled = res[3];
      const suppress = res[4];
      const progress = res[5];
      return (this.last = {
        mode,
        aim: resAim,
        cast,
        cancelled,
        suppress,
        progress,
        pinching: mode === 'pinch',
        heldMs: mode === 'pinch' ? progress * this.minPinchMs : 0,
      });
    }

    const out = { mode: 'none', aim: null, cast: false, cancelled: false, suppress: false, progress: 0,
      tip: null, mapped: null, ratio: null, reach: null, pinching: false, heldMs: 0 };
    const m = pinchMetrics(hand, aspect);
    if (!m) {
      const ev = this.pinch.update(null, now);
      if (ev.cancelled) { out.cancelled = true; this.anchor = null; }
      if (ev.active) { out.mode = 'pinch'; out.pinching = true; out.aim = this.locked; out.heldMs = ev.heldMs; out.progress = clamp(ev.heldMs / this.minPinchMs); }
      out.suppress = ev.active || now - this.releasedAt < this.refractoryMs;
      this.out = null;
      return (this.last = out);
    }

    // Mirrored, so the reticle moves the way the hand does.
    const tip = this.filter.filter({ x: 1 - hand[8].x, y: hand[8].y }, now);
    this.range.update(tip, now);
    const mapped = this.range.map(tip);
    Object.assign(out, { tip, mapped, ratio: m.ratio, reach: m.reach });

    const fist = gesture.name === 'Closed_Fist' && gesture.score >= this.fistScore;
    const ev = this.pinch.update(m.ratio, now, { canStart: m.reach >= this.pinch.minReach && !fist });
    const pinchPoint = this.pinchFilter.filter({ x: 1 - m.point.x, y: m.point.y }, now);

    if (ev.started) {
      this.anchor = { aim: { x: aim.x, y: aim.y }, hand: this.range.map(pinchPoint, false) };
      this.locked = { x: aim.x, y: aim.y };
    }
    if (ev.active && this.anchor) {
      out.mode = 'pinch';
      out.pinching = true;
      out.heldMs = ev.heldMs;
      out.progress = clamp(ev.heldMs / this.minPinchMs);
      // Precise aim: the reticle moves a fraction of the hand's motion from where the pinch began.
      // Once the fingers start to open, it freezes, so letting go doesn't nudge the target.
      if (m.ratio < this.pinch.enter) {
        const h = this.range.map(pinchPoint, false);
        const target = {
          x: clamp(this.anchor.aim.x + (h.x - this.anchor.hand.x) * this.preciseGain),
          y: clamp(this.anchor.aim.y + (h.y - this.anchor.hand.y) * this.preciseGain),
        };
        this.locked = softDeadZone(this.locked, target, this.deadZone / 2);
      }
      out.aim = this.locked;
      this.out = null;
    } else if (ev.released) {
      out.mode = 'release';
      if (ev.heldMs >= this.minPinchMs && this.locked) { out.cast = true; out.aim = this.locked; } else out.cancelled = true;
      this.releasedAt = now;
      this.anchor = null;
      this.pinchFilter.reset();
      this.out = null;
    } else {
      if (gesture.name === 'Pointing_Up' && gesture.score >= this.aimScore) this.pointUntil = now + this.pointGraceMs;
      if (now <= this.pointUntil) {
        out.mode = 'point';
        this.out = softDeadZone(this.out, mapped, this.deadZone);
        out.aim = this.out;
      } else this.out = null;
    }
    // Fingers about to touch, touching, or just let go: a fist or a palm read from that hand
    // shape is a side effect of the pinch, not a cast.
    out.suppress = ev.active || ev.released || this.pinch.closedAt != null || now - this.releasedAt < this.refractoryMs;
    return (this.last = out);
  }
}
