// camera debug (#67): unit tests for the finger actions — One Euro smoothing, the dead zone, the
// per-player hand range, pinch detection and the pointing / pinch-and-release controller — plus the
// debug tab's rate meters. Synthetic MediaPipe hand landmarks, no browser.
// Run: node --test tests/camera_fingers.test.mjs   (also run by tests/test_camera_fingers.py)

import test from 'node:test';
import assert from 'node:assert/strict';
import { OneEuroFilter, PointFilter, softDeadZone, HandRange, FIXED_RANGE, pinchMetrics, PinchDetector, PINCH, FingerController, MultiHandTracker } from '../src/camera/fingers.js';
import { RateMeter, Ema, FrameMeter } from '../src/camera/meter.js';
import { workerPreference, workerSupport } from '../src/camera/engine.js';
import { COACH_MOVES, COACH_CARDS, buildGhostHand, initCoach } from '../src/camera/coach.js';

// ---------- helpers ----------
// Deterministic noise in [-1, 1].
function noise(seed = 1) {
  let s = seed >>> 0;
  return () => { s = (s * 1664525 + 1013904223) >>> 0; return (s / 4294967296) * 2 - 1; };
}
const std = (xs) => { const m = xs.reduce((a, b) => a + b, 0) / xs.length; return Math.sqrt(xs.reduce((a, b) => a + (b - m) ** 2, 0) / xs.length); };

// A right hand in "hand space": wrist at the origin, y towards the fingers, palm length (wrist →
// middle knuckle) = 1. Poses only move the thumb and finger joints that matter.
const BASE = [
  [0, 0],                                                   // 0 wrist
  [-0.25, 0.15], [-0.45, 0.35], [-0.6, 0.55], [-0.72, 0.75], // 1–4 thumb (open)
  [-0.3, 0.95], [-0.33, 1.35], [-0.35, 1.6], [-0.36, 1.8],   // 5–8 index (extended)
  [0, 1], [0, 1.45], [0, 1.72], [0, 1.95],                   // 9–12 middle
  [0.25, 0.95], [0.27, 1.35], [0.28, 1.6], [0.29, 1.8],      // 13–16 ring
  [0.47, 0.85], [0.52, 1.15], [0.55, 1.35], [0.57, 1.5],     // 17–20 pinky
];
const POSES = {
  open: {},
  point: { 4: [-0.05, 0.7], 10: [0, 1.2], 11: [0, 0.95], 12: [0, 0.8], 14: [0.27, 1.15], 15: [0.27, 0.95], 16: [0.26, 0.8], 18: [0.5, 1.0], 19: [0.5, 0.85], 20: [0.48, 0.75] },
  pinch: { 4: [-0.55, 1.0], 6: [-0.35, 1.3], 7: [-0.45, 1.25], 8: [-0.5, 1.1] },
  half: { 4: [-0.75, 0.75], 6: [-0.35, 1.3], 7: [-0.45, 1.25], 8: [-0.5, 1.1] },      // opening: between enter and exit
  release: { 4: [-0.8, 0.5], 6: [-0.35, 1.3], 7: [-0.45, 1.25], 8: [-0.5, 1.1] },
  fist: { 4: [-0.1, 0.75], 6: [-0.32, 1.15], 7: [-0.3, 0.8], 8: [-0.25, 0.6], 10: [0, 1.15], 11: [0, 0.8], 12: [0, 0.65], 14: [0.25, 1.1], 15: [0.25, 0.8], 16: [0.24, 0.65], 18: [0.48, 1.0], 19: [0.46, 0.8], 20: [0.44, 0.7] },
};
// Into MediaPipe frame coordinates: wrist at (cx, cy), palm length = `size` of the frame height.
function makeHand(pose = 'open', { cx = 0.5, cy = 0.75, size = 0.25, aspect = 4 / 3 } = {}) {
  const p = POSES[pose];
  return BASE.map((xy, i) => { const [hx, hy] = p[i] || xy; return { x: cx + (hx * size) / aspect, y: cy - hy * size, z: 0 }; });
}

// ---------- One Euro ----------
test('One Euro: first sample passes through; duplicates and out-of-order samples change nothing', () => {
  const f = new OneEuroFilter();
  assert.equal(f.filter(0.3, 0), 0.3);
  const a = f.filter(0.5, 33);
  assert.equal(f.filter(0.9, 33), a);
  assert.equal(f.filter(0.9, 20), a);
  // A long gap (hand lost) starts over at the new value instead of sliding across the map.
  assert.equal(f.filter(0.8, 2000), 0.8);
});

test('One Euro: a still hand\'s jitter is smoothed away', () => {
  const f = new OneEuroFilter();
  const rnd = noise(7);
  const raw = [], out = [];
  for (let i = 0; i < 300; i++) {
    const x = 0.5 + rnd() * 0.006;              // ±0.6% of the frame, typical landmark jitter
    raw.push(x);
    out.push(f.filter(x, i * 33.3));
  }
  const r = std(raw.slice(30)), o = std(out.slice(30));
  assert.ok(o < r * 0.45, `jitter ${o.toFixed(5)} vs raw ${r.toFixed(5)}`);
});

test('One Euro: a fast move is followed with little lag', () => {
  // A 0.4-wide sweep in 200 ms (2 frame-widths a second), then hold.
  const f = new OneEuroFilter();
  let t = 0, x = 0.3;
  for (; t < 500; t += 33.3) f.filter(x, t);
  for (let i = 1; i <= 6; i++) { t += 33.3; x = 0.3 + (0.4 * i) / 6; f.filter(x, t); }
  const lag = 0.7 - f.x;
  assert.ok(lag < 0.08, `lag at the end of the sweep ${lag.toFixed(3)}`);
  for (let i = 0; i < 5; i++) { t += 33.3; f.filter(0.7, t); }   // 166 ms later
  assert.ok(Math.abs(0.7 - f.x) < 0.01, `settled at ${f.x.toFixed(4)}`);
  // An exponential smoother tuned to the same jitter reduction lags far more.
  let ema = 0.3, tt = 0;
  for (; tt < 500; tt += 33.3) ema += (0.3 - ema) * 0.25;
  for (let i = 1; i <= 6; i++) ema += (0.3 + (0.4 * i) / 6 - ema) * 0.25;
  assert.ok(0.7 - ema > lag * 2, `ema lag ${(0.7 - ema).toFixed(3)} vs one euro ${lag.toFixed(3)}`);
});

test('PointFilter smooths x and y independently', () => {
  const f = new PointFilter();
  assert.deepEqual(f.filter({ x: 0.2, y: 0.8 }, 0), { x: 0.2, y: 0.8 });
  const p = f.filter({ x: 0.3, y: 0.8 }, 33);
  assert.ok(p.x > 0.2 && p.x < 0.3);
  assert.equal(p.y, 0.8);
});

// ---------- dead zone ----------
test('dead zone: small moves are ignored, larger ones trail by the radius without a jump', () => {
  const o = { x: 0.5, y: 0.5 };
  assert.equal(softDeadZone(o, { x: 0.503, y: 0.5 }, 0.004), o);
  const moved = softDeadZone(o, { x: 0.51, y: 0.5 }, 0.004);
  assert.ok(Math.abs(moved.x - 0.506) < 1e-9 && moved.y === 0.5);
  // Continuous at the edge of the zone: just past it moves just a little.
  const edge = softDeadZone(o, { x: 0.5041, y: 0.5 }, 0.004);
  assert.ok(edge.x - 0.5 < 0.0002);
  // Radial, not per axis.
  const diag = softDeadZone(o, { x: 0.503, y: 0.503 }, 0.004);
  assert.ok(diag !== o && Math.hypot(diag.x - 0.503, diag.y - 0.503) - 0.004 < 1e-9);
  assert.deepEqual(softDeadZone(null, { x: 0.1, y: 0.2 }, 0.004), { x: 0.1, y: 0.2 });
});

// ---------- auto range ----------
// The fingertip zig-zags over a box, touching every edge about once a second.
const tri = (p) => 1 - Math.abs(2 * (p - Math.floor(p)) - 1);
function sweep(range, box, ms, t0 = 0, step = 33.3) {
  let t = t0;
  for (let i = 0; t < t0 + ms; i++, t += step) {
    const x = box.x[0] + (box.x[1] - box.x[0]) * tri(i / 20);
    const y = box.y[0] + (box.y[1] - box.y[0]) * tri(i / 26);
    range.update({ x, y }, t);
  }
  return t;
}

test('range: fixed bounds until a hand is seen, then it learns this player\'s reach', () => {
  const r = new HandRange({ learnMs: 3000 });
  assert.deepEqual(r.bounds('x'), FIXED_RANGE.x);
  assert.deepEqual(r.map({ x: 0.5, y: 0.42 }), { x: 0.5, y: 0.5 });
  // This player sits far back: their fingertip only covers the middle of the frame.
  const box = { x: [0.4, 0.62], y: [0.35, 0.6] };
  sweep(r, box, 1500);
  assert.ok(r.progress > 0.4 && r.progress < 0.6 && !r.calibrated);
  sweep(r, box, 1600, 1500);
  assert.ok(r.calibrated);
  // Their comfortable edges now reach the map's edges (with a small margin to spare)...
  assert.equal(r.map({ x: 0.62, y: 0.6 }).x, 1);
  assert.equal(r.map({ x: 0.4, y: 0.35 }).y, 0);
  assert.ok(Math.abs(r.map({ x: 0.51, y: 0.475 }).x - 0.5) < 0.01);
  // ...which the old fixed bounds never did.
  const fixed = new HandRange({ learnMs: Infinity });
  assert.ok(fixed.map({ x: 0.62, y: 0.6 }).x < 0.7);
});

test('range: a still hand keeps a minimum span, so noise stays small', () => {
  const r = new HandRange({ learnMs: 1000 });
  for (let t = 0; t < 2000; t += 33) r.update({ x: 0.5, y: 0.5 }, t);
  const [x0, x1] = r.bounds('x');
  assert.ok(x1 - x0 >= r.minSpan.x * (1 - 2 * r.margin) - 1e-9, `span ${x1 - x0}`);
  const a = r.map({ x: 0.5, y: 0.5 }), b = r.map({ x: 0.503, y: 0.5 });
  assert.ok(b.x - a.x < 0.02, `0.003 of jitter moved the reticle ${b.x - a.x}`);
});

test('range: grows at once past an edge, and a stray extreme decays back', () => {
  const r = new HandRange({ learnMs: 1000, tauMs: 20000 });
  let t = sweep(r, { x: [0.4, 0.6], y: [0.4, 0.6] }, 1500);
  r.update({ x: 0.9, y: 0.5 }, (t += 33));           // one sample far right
  assert.ok(r.hi.x >= 0.9 - 1e-9);
  assert.equal(r.map({ x: 0.9, y: 0.5 }).x, 1);
  t = sweep(r, { x: [0.4, 0.6], y: [0.4, 0.6] }, 60000, t + 33);
  assert.ok(r.hi.x < 0.66, `hi.x decayed to ${r.hi.x}`);
  assert.ok(r.lo.x < 0.415, `lo.x ${r.lo.x}`);   // still reached about once a second
});

test('range: time without a hand doesn\'t count towards learning', () => {
  const r = new HandRange({ learnMs: 3000 });
  r.update({ x: 0.5, y: 0.5 }, 0);
  r.update({ x: 0.5, y: 0.5 }, 10000);   // 10 s gap: counts as at most one step
  assert.ok(r.seenMs <= 200 && !r.calibrated);
  r.reset();
  assert.equal(r.lo, null);
  assert.equal(r.progress, 0);
});

// ---------- pinch ----------
test('pinch metrics: touching tips read small, open and pointing hands large, a fist curls the index in', () => {
  const pinch = pinchMetrics(makeHand('pinch'));
  const open = pinchMetrics(makeHand('open'));
  const point = pinchMetrics(makeHand('point'));
  const fist = pinchMetrics(makeHand('fist'));
  assert.ok(pinch.ratio < PINCH.enter, `pinch ${pinch.ratio}`);
  assert.ok(open.ratio > PINCH.exit && point.ratio > PINCH.exit, `open ${open.ratio}, point ${point.ratio}`);
  assert.ok(pinch.reach >= PINCH.minReach, `pinch reach ${pinch.reach}`);
  assert.ok(fist.reach < PINCH.minReach, `fist reach ${fist.reach}`);
  assert.ok(fist.ratio < PINCH.exit, 'a fist brings thumb and index tips together too, hence the reach check');
  assert.equal(pinchMetrics(null), null);
  assert.equal(pinchMetrics(makeHand('open').slice(0, 10)), null);
});

test('pinch metrics: independent of hand size, position and the frame\'s aspect ratio', () => {
  const a = pinchMetrics(makeHand('half'));
  const far = pinchMetrics(makeHand('half', { size: 0.1, cx: 0.2, cy: 0.4 }));
  const wide = pinchMetrics(makeHand('half', { aspect: 16 / 9 }), 16 / 9);
  assert.ok(Math.abs(a.ratio - far.ratio) < 1e-9 && Math.abs(a.ratio - wide.ratio) < 1e-9);
  assert.ok(a.ratio > PINCH.enter && a.ratio < PINCH.exit, `half-open ${a.ratio}`);
});

test('pinch detector: debounced start, hysteresis, debounced release', () => {
  const d = new PinchDetector();
  assert.equal(d.update(0.2, 0).started, false);         // closed for 0 ms
  const s = d.update(0.2, 50);
  assert.equal(s.started, true);
  assert.equal(s.active, true);
  assert.equal(d.update(0.4, 100).active, true);         // between enter and exit: still pinching
  assert.equal(d.update(0.6, 130).active, true);         // open for 0 ms
  assert.equal(d.update(0.35, 160).active, true);        // a one-frame flicker doesn't release
  d.update(0.6, 200);
  const r = d.update(0.6, 260);
  assert.equal(r.released, true);
  assert.equal(r.active, false);
  assert.equal(r.heldMs, 210);
});

test('pinch detector: a lost hand cancels, a gated start never begins', () => {
  const d = new PinchDetector();
  d.update(0.1, 0); d.update(0.1, 60);
  assert.equal(d.active, true);
  assert.equal(d.update(null, 200).active, true);        // short dropout: hold on
  const c = d.update(null, 400);
  assert.equal(c.cancelled, true);
  assert.equal(c.released, false);
  const g = new PinchDetector();
  for (let t = 0; t < 300; t += 33) assert.equal(g.update(0.1, t, { canStart: false }).started, false);
});

// ---------- the controller ----------
const AIM = { x: 0.5, y: 0.5 };
function feed(c, frames, { aim = AIM, t0 = 0, step = 33.3 } = {}) {
  const outs = [];
  let t = t0;
  for (const f of frames) {
    for (let i = 0; i < (f.n || 1); i++) {
      const o = c.update({ hand: f.hand === undefined ? makeHand(f.pose, f.at) : f.hand, gesture: f.gesture || { name: 'None', score: 0 }, aim: f.aim || aim, now: t, aspect: 4 / 3 });
      outs.push(o);
      if (o.aim) aim = o.aim;
      t += step;
    }
  }
  return { outs, aim, t, last: outs[outs.length - 1] };
}
const fixedRange = { learnMs: Infinity };   // keep the old fixed bounds so expected positions are simple
const POINTING = { name: 'Pointing_Up', score: 0.8 };

test('controller: pointing aims at the mirrored, mapped fingertip', () => {
  const c = new FingerController({ range: fixedRange });
  // Fingertip of a hand whose wrist is at (0.3, 0.75): index tip x = 0.3 - 0.36*0.25/(4/3) = 0.2325 → mirrored 0.7675.
  const { last } = feed(c, [{ pose: 'point', at: { cx: 0.3 }, gesture: POINTING, n: 15 }]);
  assert.equal(last.mode, 'point');
  const tip = makeHand('point', { cx: 0.3 })[8];
  const want = { x: (1 - tip.x - 0.15) / 0.7, y: (tip.y - 0.12) / 0.6 };
  assert.ok(Math.abs(last.aim.x - want.x) < 0.01 && Math.abs(last.aim.y - want.y) < 0.01, JSON.stringify({ got: last.aim, want }));
  assert.equal(last.suppress, false);
});

test('controller: no gesture, no aiming; a one-frame recogniser dropout keeps aiming', () => {
  const c = new FingerController({ range: fixedRange });
  assert.equal(feed(c, [{ pose: 'open', n: 5 }]).last.aim, null);
  const r = feed(c, [{ pose: 'point', gesture: POINTING, n: 5 }, { pose: 'point', n: 1 }, { pose: 'point', gesture: POINTING, n: 1 }]);
  assert.ok(r.outs.every((o) => o.mode === 'point'));
  const gone = feed(c, [{ pose: 'point', n: 10 }], { t0: 1000 });
  assert.equal(gone.last.mode, 'none');
});

test('controller: pinch-and-hold aims precisely from the reticle; letting go casts there', () => {
  const c = new FingerController({ range: fixedRange });
  const start = { x: 0.3, y: 0.6 };
  // Pinch, then move the hand 0.14 of the frame to the left (= right on the mirrored screen).
  const hold = feed(c, [
    { pose: 'pinch', at: { cx: 0.5 }, n: 6 },
    ...Array.from({ length: 10 }, (_, i) => ({ pose: 'pinch', at: { cx: 0.5 - 0.014 * (i + 1) } })),
    { pose: 'pinch', at: { cx: 0.36 }, n: 10 },
  ], { aim: start });
  assert.ok(hold.outs.slice(3).every((o) => o.mode === 'pinch' && o.suppress), 'pinching suppresses gesture casts');
  // 0.14 of the frame is 0.2 of the fixed range; at 0.35 gain the reticle moves ~0.07.
  const moved = hold.aim.x - start.x;
  assert.ok(moved > 0.055 && moved < 0.075, `precise move ${moved.toFixed(4)}`);
  assert.ok(Math.abs(hold.aim.y - start.y) < 0.01);
  assert.equal(hold.last.progress, 1);
  // Opening the fingers (and drifting while doing it) doesn't move the target; the release casts.
  const rel = feed(c, [{ pose: 'half', at: { cx: 0.33 }, n: 2 }, { pose: 'release', at: { cx: 0.3 }, n: 3 }], { aim: hold.aim, t0: hold.t });
  const cast = rel.outs.find((o) => o.cast);
  assert.ok(cast, 'release casts');
  assert.deepEqual(cast.aim, hold.aim);
  assert.equal(cast.mode, 'release');
  assert.ok(rel.outs.every((o) => o.suppress), 'just released: still suppressed');
  // The refractory period ends.
  const after = feed(c, [{ pose: 'open', n: 12 }], { aim: hold.aim, t0: rel.t });
  assert.equal(after.last.suppress, false);
  assert.equal(after.outs.filter((o) => o.cast).length, 0);
});

test('controller: a quick tap or a lost hand ends a pinch without casting', () => {
  const tap = new FingerController({ range: fixedRange });
  const r = feed(tap, [{ pose: 'pinch', n: 3 }, { pose: 'release', n: 4 }]);
  assert.equal(r.outs.filter((o) => o.cast).length, 0);
  assert.ok(r.outs.some((o) => o.cancelled));
  const lost = new FingerController({ range: fixedRange });
  const l = feed(lost, [{ pose: 'pinch', n: 10 }, { hand: null, n: 12 }, { pose: 'release', n: 4 }]);
  assert.equal(l.outs.filter((o) => o.cast).length, 0);
  assert.ok(l.outs.some((o) => o.cancelled));
});

test('controller: a fist never starts a pinch, even with the thumb on the index', () => {
  const c = new FingerController({ range: fixedRange });
  const r = feed(c, [{ pose: 'fist', gesture: { name: 'Closed_Fist', score: 0.9 }, n: 20 }, { pose: 'open', n: 5 }]);
  assert.ok(r.outs.every((o) => !o.pinching && !o.cast));
  // The reach check alone is enough when the recogniser isn't sure it's a fist.
  const d = new FingerController({ range: fixedRange });
  const s = feed(d, [{ pose: 'fist', gesture: { name: 'None', score: 0 }, n: 20 }]);
  assert.ok(s.outs.every((o) => !o.pinching));
});

test('controller: the range learns from the fingertip while a hand is visible', () => {
  const c = new FingerController();
  feed(c, [{ pose: 'point', gesture: POINTING, n: 100 }]);   // 3.3 s
  assert.ok(c.range.calibrated);
  c.reset();
  assert.equal(c.range.calibrated, false);
});

// ---------- meters & engine choice ----------
test('rate meter, EMA and frame meter', () => {
  const m = new RateMeter(1000);
  for (let t = 0; t <= 1000; t += 40) m.tick(t);
  assert.ok(Math.abs(m.hz(1000) - 25) < 0.01);
  assert.equal(m.hz(5000), 0);
  const e = new Ema(0.5);
  assert.equal(e.add(10), 10);
  assert.equal(e.add(20), 15);
  const f = new FrameMeter({ longMs: 50 });
  for (let t = 0; t < 1000; t += 16.7) f.frame(t);
  f.frame(1100);
  assert.equal(f.long, 1);
  assert.ok(f.fps(1100) > 50);
});

test('engine: ?camworker= picks the inference path; missing worker features are reported', () => {
  assert.equal(workerPreference(''), 2);
  assert.equal(workerPreference('?camworker=0'), 0);
  assert.equal(workerPreference('?camworker=1&x=2'), 1);
  assert.equal(workerPreference('?camworker=yes'), 2);
  assert.equal(workerSupport({}), 'no Web Workers');
  assert.equal(workerSupport({ Worker: class {}, createImageBitmap() {} }), 'no OffscreenCanvas');
  assert.equal(workerSupport({ Worker: class {}, createImageBitmap() {}, OffscreenCanvas: class {} }), null);
});

// ---------- multi-hand tracking & continuous zoom (#118, #119) ----------
test('MultiHandTracker: tracks left and right hands stably across hand crossing and brief loss', () => {
  const m = new MultiHandTracker({ lossMs: 300 });
  const handL = makeHand('open', { cx: 0.3, cy: 0.7 });
  const handR = makeHand('open', { cx: 0.7, cy: 0.7 });

  // Initial detection: slot L and R
  const r1 = m.assignHands([{ landmarks: handL }, { landmarks: handR }], 0);
  assert.equal(r1.left.wrist.x, 0.3);
  assert.equal(r1.right.wrist.x, 0.7);

  // Small movement across steps so distance matching tracks crossing
  const crossL = makeHand('open', { cx: 0.45, cy: 0.7 });
  const crossR = makeHand('open', { cx: 0.55, cy: 0.7 });
  const r2 = m.assignHands([{ landmarks: crossL }, { landmarks: crossR }], 50);
  assert.ok(Math.abs(r2.left.wrist.x - 0.45) < 0.01, 'left hand followed movement');
  assert.ok(Math.abs(r2.right.wrist.x - 0.55) < 0.01, 'right hand followed movement');

  // Brief dropout under 300 ms preserves slot
  m.assignHands([], 100);
  assert.ok(m.slots.left && !m.slots.left.visible);
  assert.ok(m.slots.right && !m.slots.right.visible);
});

test('MultiHandTracker: two-hand pinch continuous zoom freezes on unpinch or hand loss (#119)', () => {
  const m = new MultiHandTracker({ pinch: { enterMs: 30 } });
  const pL1 = makeHand('pinch', { cx: 0.4, cy: 0.7 });
  const pR1 = makeHand('pinch', { cx: 0.6, cy: 0.7 });

  // Both pinch at distance 0.2
  let out;
  for (let t = 0; t <= 100; t += 25) {
    out = m.update({ hands: [{ landmarks: pL1 }, { landmarks: pR1 }], now: t });
  }
  assert.equal(out.bothPinched, true);
  assert.equal(out.zoomActive, true);

  // Spread hands apart significantly (from distance 0.2 to 0.5) over multiple frames
  const pL2 = makeHand('pinch', { cx: 0.25, cy: 0.7 });
  const pR2 = makeHand('pinch', { cx: 0.75, cy: 0.7 });
  for (let t = 125; t <= 300; t += 25) {
    out = m.update({ hands: [{ landmarks: pL2 }, { landmarks: pR2 }], now: t });
  }
  assert.ok(out.zoomFactor > 1, 'zoom factor increased');
  const freezeAt = out.zoomFactor;

  // Unpinch either hand: zoom freezes immediately without jumping back
  const openL = makeHand('open', { cx: 0.25, cy: 0.7 });
  // Pass open hand until pinch release debounces (60ms)
  let freezeZoom = null;
  for (let t = 325; t <= 450; t += 25) {
    out = m.update({ hands: [{ landmarks: openL }, { landmarks: pR2 }], now: t });
    if (!out.zoomActive && freezeZoom == null) freezeZoom = out.zoomFactor;
  }
  assert.equal(out.zoomActive, false);
  assert.equal(out.zoomDelta, 0);
  assert.equal(out.zoomFactor, freezeZoom);
});

// ---------- Coach ghost hand keyframes (#123) ----------
test('Coach: procedural 21-landmark ghost hands and keyframes exist for the movement moves', () => {
  assert.equal(COACH_MOVES.length, 2);
  assert.ok(!COACH_MOVES.some((m) => m.id === 'wave_orbit'));
  assert.equal(COACH_CARDS.length, COACH_MOVES.length);
  assert.ok(!COACH_CARDS.some((m) => m.id === 'palm_calm'));
  assert.ok(!COACH_CARDS.some((m) => m.id === 'fist_strike'));
  assert.deepEqual(COACH_CARDS.map((m) => m.id), ['pinch_grab', 'two_hand_zoom']);
  for (const move of COACH_MOVES) {
    const ghosts = move.sample(0.5);
    assert.ok(ghosts.length >= 1 && ghosts.length <= 2);
    for (const g of ghosts) {
      assert.equal(g.hand.length, 21);
      for (const pt of g.hand) {
        assert.ok(pt.x >= 0 && pt.x <= 1);
        assert.ok(pt.y >= 0 && pt.y <= 1);
      }
    }
  }
  const ghost = buildGhostHand('open', 'pinch', 0.5);
  assert.equal(ghost.length, 21);
});

test('Coach: initCoach tracks demonstrated moves and defaults to collapsed minimal state (#204)', () => {
  const panel = {
    innerHTML: '',
    classList: {
      tokens: new Set(['coach-panel', 'is-collapsed']),
      toggle(cls, force) {
        if (force === undefined) force = !this.tokens.has(cls);
        if (force) this.tokens.add(cls); else this.tokens.delete(cls);
        return force;
      },
      has(cls) { return this.tokens.has(cls); }
    },
    querySelector(sel) {
      return {
        hidden: false,
        dataset: {},
        style: {},
        classList: { toggle() {}, add() {}, remove() {} },
        setAttribute() {},
        addEventListener() {},
        getContext() { return {}; },
        textContent: '',
      };
    },
  };

  const oldDoc = globalThis.document;
  const oldStorage = globalThis.localStorage;
  const oldWin = globalThis.window;
  const store = new Map();
  store.set('moc.coachDemonstrated', JSON.stringify(['two_hand_zoom']));
  globalThis.localStorage = {
    getItem: (k) => store.get(k) ?? null,
    setItem: (k, v) => store.set(k, String(v)),
  };
  globalThis.document = {
    getElementById: (id) => (id === 'coachPanel' ? panel : null),
  };
  globalThis.window = {
    addEventListener() {},
  };

  try {
    const coach = initCoach();
    assert.ok(coach);
    assert.equal(coach.open, false, 'starts in collapsed/minimal state when moves demonstrated');
    assert.ok(coach.demonstrated.has('two_hand_zoom'));
    coach.markDemonstrated('pinch_grab');
    assert.ok(coach.demonstrated.has('pinch_grab'));
    coach.setOpen(true);
    assert.equal(coach.open, true);
    coach.setOpen(false);
    assert.equal(coach.open, false);
  } finally {
    globalThis.document = oldDoc;
    globalThis.localStorage = oldStorage;
    globalThis.window = oldWin;
  }
});
