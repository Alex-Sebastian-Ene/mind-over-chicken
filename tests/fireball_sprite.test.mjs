// #319: the fireball is pixel art in every view: sprite frames, a charge-up that forms, a stepped
// pixel shockwave, segmented charge rings, and no smooth canvas circles anywhere in the effect.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { orbPixels, formation, shockwavePixels, scorchPixels, ringSegments, drawSegmentRing, shade } from '../src/render/fireball-sprite.js';
import { FireballFX } from '../src/render/fireball.js';

const ints = (ps) => ps.every((p) => Number.isInteger(p.x) && Number.isInteger(p.y));

test('the orb sprite is whole pixels: white core, hot ring, tint, dark edge, flame tongues that spin', () => {
  const f0 = orbPixels(4, 0), f1 = orbPixels(4, 1);
  assert.ok(ints(f0));
  assert.equal(f0.find((p) => p.x === 0 && p.y === 0).c, 'core');
  for (const c of ['core', 'hot', 'tint', 'edge']) assert.ok(f0.some((p) => p.c === c), c);
  const key = (ps) => ps.map((p) => `${p.x},${p.y},${p.c}`).sort().join(' ');
  assert.notEqual(key(f0), key(f1), 'frames differ: the tongues turn');
  assert.equal(orbPixels(4, 4), orbPixels(4, 0), 'four frames, cached');
  assert.ok(orbPixels(2, 0, { tongues: false }).length < f0.length);
  assert.equal(shade('#ff9a2e', 0.5), '#804d17');
});

test('charging forms the fireball: it grows with charge, embers spiral in, full charge strains', () => {
  assert.ok(formation(0, 1).r < formation(1, 1).r);
  const f = formation(0.5, 2);
  assert.equal(f.embers.length, 6);
  assert.ok(f.embers.every((e) => Number.isInteger(e.x) && Number.isInteger(e.y)));
  // An ember moves inward as its life advances.
  const d = (e) => Math.hypot(e.x, e.y);
  const a = formation(0.5, 0).embers[0], b = formation(0.5, 0.3).embers[0];
  assert.ok(b.life > a.life && d(b) <= d(a));
  const rs = new Set(Array.from({ length: 20 }, (_, i) => formation(1, i * 0.03).r));
  assert.ok(rs.size === 2, 'a full charge pulses a pixel bigger and smaller');
  assert.equal(new Set(Array.from({ length: 20 }, (_, i) => formation(0.5, i * 0.03).r)).size, 1);
});

test('the shockwave is a stepped, flattened ring that dithers as it fades', () => {
  const ring = shockwavePixels(12, 0);
  assert.ok(ints(ring));
  assert.equal(new Set(ring.map((p) => `${p.x},${p.y}`)).size, ring.length, 'no doubled pixels');
  assert.ok(Math.max(...ring.map((p) => Math.abs(p.y))) <= 6, 'lies on the ground (2:1)');
  assert.ok(shockwavePixels(12, 0.9).length < shockwavePixels(12, 0.5).length);
  assert.ok(shockwavePixels(12, 0.5).length < ring.length);
  assert.ok(ints(scorchPixels(5)) && scorchPixels(5).length > 10);
});

test('charge rings are segments: hard blocks, lit by charge', () => {
  assert.equal(ringSegments(0), 0);
  assert.equal(ringSegments(0.5), 8);
  assert.equal(ringSegments(1), 16);
  const rects = [];
  const ctx = { globalAlpha: 1, fillStyle: '', fillRect: (...a) => rects.push([...a, ctx.fillStyle]) };
  drawSegmentRing(ctx, 100, 100, 12, 0.25, '#ff0000', { scale: 3, n: 16 });
  assert.equal(rects.length, 16);
  assert.equal(rects.filter((r) => r[4] === '#ff0000').length, 4);
  assert.ok(rects.every((r) => r[0] % 3 === 0 && r[1] % 3 === 0), 'snapped to the game pixel grid');
});

function recorder() {
  const calls = new Set();
  const ctx = new Proxy({ globalAlpha: 1 }, {
    get(t, k) { if (k in t) return t[k]; if (typeof k !== 'string' || k.startsWith('__')) return undefined; return (...a) => { calls.add(k); }; },
    set(t, k, v) { t[k] = v; return true; },
  });
  return { ctx, calls };
}

test('the effect draws with pixels only: flight, impact hold, burst and the reduced-motion slam', () => {
  for (const reduced of [false, true]) {
    for (const project of [null, (x, y) => [x + 1, y - 1]]) {
      const fx = new FireballFX();
      fx.launch({ x: 100, y: 90 }, { color: '#4fc3ff', charge: 0.8 });
      const { ctx, calls } = recorder();
      for (let i = 0; i < 90; i++) { fx.update(1 / 60); fx.draw(ctx, { project, reduced }); }
      assert.ok(calls.has('fillRect'), 'drew something');
      for (const smooth of ['arc', 'stroke', 'ellipse', 'bezierCurveTo']) assert.ok(!calls.has(smooth), `${smooth} used (reduced=${reduced})`);
    }
  }
});

test('the shot winds up (rises) before it drops onto the target', () => {
  const fx = new FireballFX();
  const s = fx.launch({ x: 50, y: 120 }, { charge: 1, from: { x: 50, y: 20 } });
  const y0 = fx.position(s, 0, null, 50, 120).y;
  const yw = fx.position(s, fx.opts.windup, null, 50, 120).y;
  const y1 = fx.position(s, 1, null, 50, 120).y;
  assert.ok(yw < y0, 'pulls back up first');
  assert.equal(y1, 120);
});

test('no SLAM! text over the tutorial labels: the slam itself is the feedback', async () => {
  for (const f of ['eras/stone.js', 'eras/industry.js', 'eras/space.js', 'eras/timeskip.js', 'cosmos/surface.js']) {
    const src = await readFile(new URL(`../src/${f}`, import.meta.url), 'utf8');
    assert.ok(!src.includes('SLAM!'), f);
  }
  const cam = await readFile(new URL('../src/camera/camera.js', import.meta.url), 'utf8');
  assert.ok(!/fxCtx\.arc\(/.test(cam), 'the reticle charge ring is segmented pixels');
  assert.match(cam, /<canvas class="cam-fb-orb"/);
});

test('the hand-gesture coach stays closed until the player opens it', async () => {
  const { coachStartsOpen, COACH_OPEN_KEY } = await import('../src/camera/coach.js');
  const m = new Map();
  const store = { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, v) };
  assert.equal(coachStartsOpen(store), false, 'a first run starts closed');
  m.set('moc.coachOpen', '1');
  assert.equal(coachStartsOpen(store), false, 'the old auto-open flag no longer opens it');
  m.set(COACH_OPEN_KEY, '1');
  assert.equal(coachStartsOpen(store), true, 'a player who opened it gets it back');
  assert.equal(coachStartsOpen({ getItem() { throw new Error('blocked'); } }), false);
  const src = await (await import('node:fs/promises')).readFile(new URL('../src/camera/coach.js', import.meta.url), 'utf8');
  assert.match(src, /setOpen\(open, \{ remember: false \}\)/);
});
