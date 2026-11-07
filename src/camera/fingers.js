// camera (#67, #116, #118, #119, #120, #121): finger, two-hand, pinch-grab, continuous zoom & wave actions.
// Pure logic, no DOM, so Node can unit-test it.
//
//   OneEuroFilter / PointFilter  low-latency smoothing (Casiez et al., CHI 2012)
//   softDeadZone                 radial dead zone without edge jumps
//   HandRange                    learns reachable fingertip bounds
//   pinchMetrics / PinchDetector thumb tip (4) to index tip (8), relative to hand size, with hysteresis
//   FingerController             single-hand controller (supports both head-cursor-only grab mode and point/pinch)
//   WaveDetector                 quantizes horizontal/vertical open-hand wave over ~250 ms with ~400 ms cooldown (#121)
//   MultiHandTracker             stable two-hand identity (left/right), continuous two-hand pinch zoom (#118, #119),
//                                one-hand pinch grab (#120), and wave orbit (#121).

import { clamp } from '../sim/util.js';

const TAU = 2 * Math.PI;
const alpha = (cutoff, dt) => { const r = TAU * cutoff * dt; return r / (r + 1); };

// ---------- smoothing ----------
export class OneEuroFilter {
  constructor({ minCutoff = 1.2, beta = 8, dCutoff = 1, maxGapMs = 500 } = {}) {
    Object.assign(this, { minCutoff, beta, dCutoff, maxGapMs });
    this.reset();
  }

  reset() { this.x = null; this.dx = 0; this.t = null; }

  filter(value, tMs) {
    if (this.x == null || tMs - this.t > this.maxGapMs) { this.x = value; this.dx = 0; this.t = tMs; return value; }
    const dt = (tMs - this.t) / 1000;
    if (!(dt > 0)) return this.x;
    this.t = tMs;
    this.dx += alpha(this.dCutoff, dt) * ((value - this.x) / dt - this.dx);
    this.x += alpha(this.minCutoff + this.beta * Math.abs(this.dx), dt) * (value - this.x);
    return this.x;
  }
}

export class PointFilter {
  constructor(options) { this.fx = new OneEuroFilter(options); this.fy = new OneEuroFilter(options); }
  reset() { this.fx.reset(); this.fy.reset(); }
  filter(p, tMs) { return { x: this.fx.filter(p.x, tMs), y: this.fy.filter(p.y, tMs) }; }
}

export function softDeadZone(out, target, radius) {
  if (!out) return { x: target.x, y: target.y };
  const dx = target.x - out.x, dy = target.y - out.y, d = Math.hypot(dx, dy);
  if (d <= radius) return out;
  const k = (d - radius) / d;
  return { x: out.x + dx * k, y: out.y + dy * k };
}

// ---------- reachable range ----------
export const FIXED_RANGE = { x: [0.15, 0.85], y: [0.12, 0.72] };

export class HandRange {
  constructor({ learnMs = 3000, tauMs = 30000, minSpan = { x: 0.2, y: 0.16 }, margin = 0.08, fixed = FIXED_RANGE, maxStepMs = 200 } = {}) {
    Object.assign(this, { learnMs, tauMs, minSpan, margin, fixed, maxStepMs });
    this.reset();
  }

  reset() { this.lo = null; this.hi = null; this.seenMs = 0; this.lastT = null; }

  get progress() { return this.learnMs > 0 ? clamp(this.seenMs / this.learnMs) : 1; }
  get calibrated() { return this.seenMs >= this.learnMs; }

  update(p, tMs) {
    const x = clamp(p.x), y = clamp(p.y);
    const dt = this.lastT == null ? 0 : clamp(tMs - this.lastT, 0, this.maxStepMs);
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
function dist3(a, b, aspect) {
  return Math.hypot((a.x - b.x) * aspect, a.y - b.y, ((a.z || 0) - (b.z || 0)) * aspect);
}

export function pinchMetrics(hand, aspect = 4 / 3) {
  if (!hand || hand.length < 21) return null;
  const size = Math.max(dist3(hand[0], hand[9], aspect), 1.4 * dist3(hand[5], hand[17], aspect), 1e-6);
  return {
    ratio: dist3(hand[4], hand[8], aspect) / size,
    reach: dist3(hand[0], hand[8], aspect) / Math.max(dist3(hand[0], hand[5], aspect), 1e-6),
    point: { x: (hand[4].x + hand[8].x) / 2, y: (hand[4].y + hand[8].y) / 2 },
    wrist: { x: hand[0].x, y: hand[0].y },
    palm: { x: (hand[0].x + hand[9].x) / 2, y: (hand[0].y + hand[9].y) / 2 },
  };
}

export const PINCH = {
  enter: 0.3,
  exit: 0.45,
  enterMs: 50,
  exitMs: 60,
  lossMs: 300,
  minReach: 0.9,
};

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

// ---------- Wave / swipe detector (#121) ----------
// Detects a horizontal (or vertical) open-hand wave over ~250 ms with ~400 ms cooldown.
export class WaveDetector {
  constructor({ windowMs = 250, cooldownMs = 400, minDispX = 0.14, minDispY = 0.16, maxCrossRatio = 0.65 } = {}) {
    Object.assign(this, { windowMs, cooldownMs, minDispX, minDispY, maxCrossRatio });
    this.reset();
  }

  reset() {
    this.samples = [];
    this.cooldownUntil = -Infinity;
  }

  // hand: 21 landmarks, suppressed: true when pinching/zooming/fist.
  // Returns { wave: 'left' | 'right' | 'up' | 'down' | null, vx, vy }.
  update(hand, now, { suppressed = false, gestureName = 'None' } = {}) {
    if (!hand || hand.length < 21 || suppressed || gestureName === 'Closed_Fist') {
      if (!hand || suppressed) this.samples.length = 0;
      return { wave: null, vx: 0, vy: 0 };
    }
    // Check that fingers are generally open (index & middle extended)
    const palmLen = Math.hypot(hand[9].x - hand[0].x, hand[9].y - hand[0].y);
    const midExt = Math.hypot(hand[12].x - hand[0].x, hand[12].y - hand[0].y) / Math.max(palmLen, 1e-5);
    if (midExt < 1.35) {
      this.samples.length = 0;
      return { wave: null, vx: 0, vy: 0 };
    }
    // Mirrored x so moving hand right in front of webcam is +x
    const x = 1 - (hand[0].x + hand[9].x) * 0.5;
    const y = (hand[0].y + hand[9].y) * 0.5;
    this.samples.push({ x, y, t: now });
    while (this.samples.length && now - this.samples[0].t > this.windowMs) {
      this.samples.shift();
    }
    if (this.samples.length < 2 || now < this.cooldownUntil) {
      return { wave: null, vx: 0, vy: 0 };
    }
    const first = this.samples[0];
    const dt = (now - first.t) / 1000;
    if (dt < 0.06) return { wave: null, vx: 0, vy: 0 };
    const dx = x - first.x;
    const dy = y - first.y;
    const vx = dx / dt;
    const vy = dy / dt;
    const ax = Math.abs(dx), ay = Math.abs(dy);
    let wave = null;
    if (ax >= this.minDispX && ay <= ax * this.maxCrossRatio) {
      wave = dx > 0 ? 'right' : 'left';
    } else if (ay >= this.minDispY && ax <= ay * this.maxCrossRatio) {
      wave = dy > 0 ? 'down' : 'up';
    }
    if (wave) {
      this.cooldownUntil = now + this.cooldownMs;
      this.samples.length = 0;
    }
    return { wave, vx, vy };
  }
}

// ---------- Multi-hand plumbing + two-hand pinch zoom + one-hand grab (#118, #119, #120, #121) ----------
export class MultiHandTracker {
  constructor({
    lossMs = 300,
    grabCooldownMs = 400,
    zoomDeadZone = 0.015,
    pinch = {},
    wave = {},
  } = {}) {
    this.lossMs = lossMs;
    this.grabCooldownMs = grabCooldownMs;
    this.zoomDeadZone = zoomDeadZone;
    this.leftPinch = new PinchDetector({ ...pinch, lossMs });
    this.rightPinch = new PinchDetector({ ...pinch, lossMs });
    this.distFilter = new OneEuroFilter({ minCutoff: 1.5, beta: 6 });
    this.waveDetector = new WaveDetector(wave);
    this.reset();
  }

  reset() {
    this.leftPinch.reset();
    this.rightPinch.reset();
    this.distFilter.reset();
    this.waveDetector.reset();
    this.slots = { left: null, right: null };
    this.zoomActive = false;
    this.pinchStart = null;
    this.lastDistance = null;
    this.frozenZoom = 1;
    this.grabState = { active: false, side: null, cooldownUntil: -Infinity };
  }

  // Assigns detected hands to stable { left, right } slots (#118).
  // Handles 0, 1, or 2 hands, duplicate handedness labels, hand crossing, and brief 300ms dropouts.
  assignHands(rawHands, now, aspect = 4 / 3) {
    const candidates = (rawHands || [])
      .filter((h) => h && Array.isArray(h.landmarks) && h.landmarks.length >= 21)
      .slice(0, 2)
      .map((h) => {
        const m = pinchMetrics(h.landmarks, aspect);
        return {
          landmarks: h.landmarks,
          name: h.name || 'None',
          score: h.score || 0,
          handedness: h.handedness || null,
          wrist: m.wrist,
          point: m.point,
          ratio: m.ratio,
          reach: m.reach,
          at: now,
        };
      });

    // Drop duplicate detection of the exact same physical hand (wrists within 4% of frame)
    if (candidates.length === 2) {
      const d = Math.hypot(candidates[0].wrist.x - candidates[1].wrist.x, candidates[0].wrist.y - candidates[1].wrist.y);
      if (d < 0.04) candidates.pop();
    }

    const prevL = this.slots.left;
    const prevR = this.slots.right;
    let nextL = null, nextR = null;

    if (candidates.length === 2) {
      const [c0, c1] = candidates;
      // If both previous slots were recently seen, match by minimum total wrist movement so crossing hands keep identity!
      const haveBothPrev = prevL && prevR && (now - prevL.at <= this.lossMs) && (now - prevR.at <= this.lossMs);
      if (haveBothPrev) {
        const dDirect = Math.hypot(c0.wrist.x - prevL.wrist.x, c0.wrist.y - prevL.wrist.y)
          + Math.hypot(c1.wrist.x - prevR.wrist.x, c1.wrist.y - prevR.wrist.y);
        const dSwap = Math.hypot(c1.wrist.x - prevL.wrist.x, c1.wrist.y - prevL.wrist.y)
          + Math.hypot(c0.wrist.x - prevR.wrist.x, c0.wrist.y - prevR.wrist.y);
        if (dDirect <= dSwap) { nextL = c0; nextR = c1; }
        else { nextL = c1; nextR = c0; }
      } else if (c0.handedness && c1.handedness && c0.handedness !== c1.handedness) {
        nextL = c0.handedness === 'Left' ? c0 : c1;
        nextR = c0.handedness === 'Right' ? c0 : c1;
      } else {
        // Fallback: sort by x in frame
        if (c0.wrist.x <= c1.wrist.x) { nextL = c0; nextR = c1; }
        else { nextL = c1; nextR = c0; }
      }
    } else if (candidates.length === 1) {
      const c = candidates[0];
      const dL = prevL && (now - prevL.at <= this.lossMs) ? Math.hypot(c.wrist.x - prevL.wrist.x, c.wrist.y - prevL.wrist.y) : Infinity;
      const dR = prevR && (now - prevR.at <= this.lossMs) ? Math.hypot(c.wrist.x - prevR.wrist.x, c.wrist.y - prevR.wrist.y) : Infinity;
      if (dL < 0.18 || dR < 0.18) {
        if (dL <= dR) nextL = c;
        else nextR = c;
      } else if (c.handedness === 'Left') {
        nextL = c;
      } else if (c.handedness === 'Right') {
        nextR = c;
      } else if (c.wrist.x < 0.5) {
        nextL = c;
      } else {
        nextR = c;
      }
    }

    // Preserve recently lost hand slot metadata for grace window without jumping
    if (nextL) this.slots.left = { ...nextL, side: 'left', visible: true };
    else if (prevL && now - prevL.at <= this.lossMs) this.slots.left = { ...prevL, visible: false };
    else this.slots.left = null;

    if (nextR) this.slots.right = { ...nextR, side: 'right', visible: true };
    else if (prevR && now - prevR.at <= this.lossMs) this.slots.right = { ...prevR, visible: false };
    else this.slots.right = null;

    return { left: nextL, right: nextR };
  }

  // Updates multi-hand state, two-hand continuous zoom (#119), one-hand pinch grab (#120), and wave (#121).
  update({ hands = [], now = 0, aspect = 4 / 3 } = {}) {
    const { left: curL, right: curR } = this.assignHands(hands, now, aspect);

    const canStartL = !!curL && curL.reach >= PINCH.minReach && curL.name !== 'Closed_Fist';
    const canStartR = !!curR && curR.reach >= PINCH.minReach && curR.name !== 'Closed_Fist';

    const evL = this.leftPinch.update(curL ? curL.ratio : null, now, { canStart: canStartL });
    const evR = this.rightPinch.update(curR ? curR.ratio : null, now, { canStart: canStartR });

    const leftState = this.slots.left ? { ...this.slots.left, pinching: evL.active, pinchEvent: evL } : null;
    const rightState = this.slots.right ? { ...this.slots.right, pinching: evR.active, pinchEvent: evR } : null;

    const bothPinched = evL.active && evR.active;
    const bothVisible = !!(curL && curR);
    let distance = this.lastDistance;
    let zoomDelta = 0;      // multiplicative log step this frame (0 when frozen)
    let zoomFactor = this.frozenZoom;

    if (bothPinched) {
      // Cancel any active 1-hand grab if user enters 2-hand pinch zoom
      if (this.grabState.active) {
        this.grabState.active = false;
        this.grabState.side = null;
      }
      if (bothVisible) {
        const rawDist = Math.hypot(
          (curL.point.x - curR.point.x) * aspect,
          curL.point.y - curR.point.y
        );
        const filtered = this.distFilter.filter(rawDist, now);
        if (!this.zoomActive || this.pinchStart == null) {
          this.zoomActive = true;
          this.pinchStart = Math.max(filtered, 0.04);
          this.lastDistance = this.pinchStart;
          distance = this.pinchStart;
        } else {
          const diff = filtered - this.lastDistance;
          if (Math.abs(diff) > this.zoomDeadZone) {
            const eff = filtered - Math.sign(diff) * this.zoomDeadZone;
            const prev = Math.max(this.lastDistance, 0.04);
            const next = Math.max(eff, 0.04);
            zoomDelta = Math.log(next / prev);
            this.frozenZoom = clamp(this.frozenZoom * (next / prev), 0.25, 4.0);
            this.lastDistance = next;
          }
          distance = this.lastDistance;
        }
        zoomFactor = this.frozenZoom;
      } else {
        // One hand temporarily occluded (<300 ms): freeze at current zoom, do NOT jump!
        zoomDelta = 0;
        zoomFactor = this.frozenZoom;
      }
    } else {
      // Unpinched either hand or lost >300 ms: freeze at current zoom!
      if (this.zoomActive) {
        this.zoomActive = false;
        this.pinchStart = null;
        this.distFilter.reset();
      }
    }

    // One-hand pinch grab (#120) — suppressed while two-hand zoom is active or cooling down
    const grab = { active: false, started: false, released: false, cancelled: false, side: null };
    if (!bothPinched && !this.zoomActive) {
      const activeSide = evL.active ? 'left' : evR.active ? 'right' : null;
      const ev = activeSide === 'left' ? evL : activeSide === 'right' ? evR : (this.grabState.side === 'left' ? evL : evR);
      if (!this.grabState.active) {
        if (activeSide && (evL.started || evR.started) && now >= this.grabState.cooldownUntil) {
          this.grabState.active = true;
          this.grabState.side = activeSide;
          grab.active = true;
          grab.started = true;
          grab.side = activeSide;
        }
      } else {
        const s = this.grabState.side;
        const curEv = s === 'left' ? evL : evR;
        if (curEv.active) {
          grab.active = true;
          grab.side = s;
        } else if (curEv.released) {
          this.grabState.active = false;
          this.grabState.side = null;
          this.grabState.cooldownUntil = now + this.grabCooldownMs;
          grab.released = true;
          grab.side = s;
        } else if (curEv.cancelled) {
          this.grabState.active = false;
          this.grabState.side = null;
          grab.cancelled = true;
          grab.side = s;
        }
      }
    }

    // Wave / swipe (#121) — uses whichever visible hand is open, suppressed while pinching or zooming
    const primary = curR || curL;
    const anyPinching = evL.active || evR.active || this.leftPinch.closedAt != null || this.rightPinch.closedAt != null;
    const waveOut = this.waveDetector.update(primary?.landmarks || null, now, {
      suppressed: bothPinched || anyPinching || grab.active || grab.released,
      gestureName: primary?.name || 'None',
    });

    return {
      left: leftState,
      right: rightState,
      bothPinched,
      zoomActive: this.zoomActive,
      distance,
      pinchStart: this.pinchStart,
      zoomFactor,
      zoomDelta,
      grab,
      wave: waveOut.wave,
      waveVel: { vx: waveOut.vx, vy: waveOut.vy },
      suppressCasts: bothPinched || anyPinching || grab.active || grab.released || !!waveOut.wave,
    };
  }
}

// ---------- Single-hand controller (preserved for unit tests & precision fallback) ----------
export class FingerController {
  constructor({ aimScore = 0.45, pointGraceMs = 200, preciseGain = 0.35, deadZone = 0.004, minPinchMs = 150, refractoryMs = 300,
    fistScore = 0.5, headCursorOnly = false, filter = {}, pinchFilter = { minCutoff: 0.6, beta: 4 }, range = {}, pinch = {} } = {}) {
    if (RustFingerControllerClass) {
      try {
        this.wasm = new RustFingerControllerClass();
      } catch (e) {
        this.wasm = null;
      }
    }
    Object.assign(this, { aimScore, pointGraceMs, preciseGain, deadZone, minPinchMs, refractoryMs, fistScore, headCursorOnly });
    this.filter = new PointFilter(filter);
    this.pinchFilter = new PointFilter(pinchFilter);
    this.range = new HandRange(range);
    this.pinch = new PinchDetector(pinch);
    this.reset();
  }

  reset() {
    if (this.wasm) this.wasm.reset();
    this.filter.reset(); this.pinchFilter.reset(); this.range.reset(); this.pinch.reset();
    this.out = null;
    this.anchor = null;
    this.locked = null;
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
      if (ev.active) { out.mode = 'pinch'; out.pinching = true; out.aim = this.headCursorOnly ? null : this.locked; out.heldMs = ev.heldMs; out.progress = clamp(ev.heldMs / this.minPinchMs); }
      out.suppress = ev.active || now - this.releasedAt < this.refractoryMs;
      this.out = null;
      return (this.last = out);
    }

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
      if (!this.headCursorOnly && m.ratio < this.pinch.enter) {
        const h = this.range.map(pinchPoint, false);
        const target = {
          x: clamp(this.anchor.aim.x + (h.x - this.anchor.hand.x) * this.preciseGain),
          y: clamp(this.anchor.aim.y + (h.y - this.anchor.hand.y) * this.preciseGain),
        };
        this.locked = softDeadZone(this.locked, target, this.deadZone / 2);
      }
      out.aim = this.headCursorOnly ? null : this.locked;
      this.out = null;
    } else if (ev.released) {
      out.mode = 'release';
      if (ev.heldMs >= this.minPinchMs && this.locked) { out.cast = true; out.aim = this.headCursorOnly ? aim : this.locked; } else out.cancelled = true;
      this.releasedAt = now;
      this.anchor = null;
      this.pinchFilter.reset();
      this.out = null;
    } else if (!this.headCursorOnly) {
      if (gesture.name === 'Pointing_Up' && gesture.score >= this.aimScore) this.pointUntil = now + this.pointGraceMs;
      if (now <= this.pointUntil) {
        out.mode = 'point';
        this.out = softDeadZone(this.out, mapped, this.deadZone);
        out.aim = this.out;
      } else this.out = null;
    }
    out.suppress = ev.active || ev.released || this.pinch.closedAt != null || now - this.releasedAt < this.refractoryMs;
    return (this.last = out);
  }
}
