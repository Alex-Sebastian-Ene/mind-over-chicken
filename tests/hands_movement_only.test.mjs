// Hands are for movement only (#261, #168): no hand shape and no key casts a spell; the only hand
// moves are one-hand pinch (grab / move) and two-hand pinch (zoom), and the coach shows exactly those.
// Regression guard for the cast path removed in #268.
// Run: node --test tests/hands_movement_only.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { COACH_MOVES, COACH_CARDS, buildGhostHand } from '../src/camera/coach.js';
import { MultiHandTracker } from '../src/camera/fingers.js';

const HAND_MOVES = ['pinch_grab', 'two_hand_zoom'];

test('the coach shows exactly Pinch Grab and Two-Hand Zoom, each backed by a tracker action (#261, #168)', () => {
  assert.deepEqual(COACH_MOVES.map((m) => m.id), HAND_MOVES);
  assert.deepEqual(COACH_CARDS.map((m) => m.id), HAND_MOVES);
  assert.deepEqual(COACH_CARDS.map((m) => m.title), ['Pinch Grab', 'Two-Hand Zoom']);
  for (const m of COACH_CARDS) assert.doesNotMatch(`${m.instruction} ${m.key}`, /\b[1-7]\b|cast|spell|fireball|calm/i, m.id);
});

test('no spell table, cast controller or cast call is left in src/camera (#261)', () => {
  const dir = new URL('../src/camera/', import.meta.url);
  assert.equal(existsSync(new URL('spells.js', dir)), false);
  assert.equal(existsSync(new URL('casts.js', dir)), false);
  for (const f of readdirSync(dir).filter((n) => n.endsWith('.js'))) {
    const src = readFileSync(new URL(f, dir), 'utf8');
    for (const re of [/\.onCast\??\.?\(/, /game_on_cast/, /CastController/, /spellForKeyCode/]) {
      assert.doesNotMatch(src, re, `${f} still has ${re}`);
    }
  }
  const main = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.doesNotMatch(main, /RustCastController|setRustCastController/);
});

// ---------- runtime: keys 1-9 on a live camera module cast nothing ----------
function fakeElement(tag = 'div') {
  const listeners = new Map();
  const cache = new Map();
  const ctx = new Proxy({}, { get: (t, k) => (k in t ? t[k] : () => ({})), set: (t, k, v) => { t[k] = v; return true; } });
  return {
    tagName: tag.toUpperCase(), hidden: false, disabled: false, checked: false, value: '0', textContent: '', innerHTML: '',
    width: 320, height: 240, clientWidth: 0, clientHeight: 0, readyState: 0,
    dataset: {}, style: { setProperty() {} }, attrs: {},
    classList: { toggle() {}, add() {}, remove() {}, contains: () => false },
    setAttribute(k, v) { this.attrs[k] = String(v); },
    getAttribute(k) { return this.attrs[k] ?? null; },
    addEventListener(type, fn) { if (!listeners.has(type)) listeners.set(type, []); listeners.get(type).push(fn); },
    removeEventListener() {},
    querySelector(sel) { if (!cache.has(sel)) cache.set(sel, fakeElement()); return cache.get(sel); },
    querySelectorAll() { return []; },
    insertAdjacentElement() {}, appendChild() {}, append() {}, remove() {},
    getContext() { return ctx; },
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 0, height: 0 }),
    play: async () => {},
  };
}

test('keys 1-9 and numpad 1-9 cast nothing on a live scene (#261)', async () => {
  const byId = { camBtn: fakeElement('button'), camPanel: Object.assign(fakeElement('aside'), { hidden: true }), screen: fakeElement('canvas') };
  const winListeners = new Map();
  globalThis.document = { getElementById: (id) => byId[id] || null, createElement: (t) => fakeElement(t), body: { dataset: {} }, querySelector: () => null, addEventListener() {} };
  globalThis.window = {
    isSecureContext: true, devicePixelRatio: 1, __moc: null,
    addEventListener(type, fn) { if (!winListeners.has(type)) winListeners.set(type, []); winListeners.get(type).push(fn); },
    removeEventListener() {},
  };
  globalThis.requestAnimationFrame = () => 0;
  const calls = [];
  const scene = {
    onCast: (...a) => calls.push(['onCast', ...a]),
    onBlink: (...a) => calls.push(['onBlink', ...a]),
    onGrabStart: (...a) => { calls.push(['onGrabStart', ...a]); return { ok: false }; },
  };
  const G = { scene, brain: { aim: { x: 0.5, y: 0.5 } }, stats: {}, hud: { showCause() {}, flashBlink() {} }, engine: { game_on_cast: (...a) => calls.push(['game_on_cast', ...a]) } };
  const realSetInterval = globalThis.setInterval;
  globalThis.setInterval = () => 0;
  let cam;
  try {
    const { initCamera } = await import('../src/camera/camera.js');
    cam = initCamera({ getG: () => G, canCast: () => true });
  } finally {
    globalThis.setInterval = realSetInterval;
  }
  assert.ok(cam, 'initCamera ran');
  assert.equal(typeof cam.cast, 'undefined', 'the camera exposes no cast()');
  for (const code of [...Array.from({ length: 9 }, (_, i) => `Digit${i + 1}`), ...Array.from({ length: 9 }, (_, i) => `Numpad${i + 1}`)]) {
    for (const type of ['keydown', 'keyup']) {
      for (const fn of winListeners.get(type) || []) fn({ code, key: code.slice(-1), repeat: false, preventDefault() {}, target: null });
    }
  }
  assert.deepEqual(calls, [], 'no key reached a scene cast, blink or the Rust era cast');
  assert.equal(G.stats.casts, undefined);
});

// ---------- the hand tracker: only pinches move anything ----------
test('an open palm or a fist held 2 s never starts a grab or a zoom; a pinch does (#261)', () => {
  const run = (poseA, poseB = poseA, two = false) => {
    const t = new MultiHandTracker();
    const out = [];
    for (let i = 0; i < 60; i++) {
      const left = { landmarks: buildGhostHand(poseA, poseB, 1, { cx: 0.3, cy: 0.7, aspect: 4 / 3 }), name: poseA === 'fist' ? 'Closed_Fist' : 'Open_Palm', score: 0.9, handedness: 'Left' };
      const right = { landmarks: buildGhostHand(poseA, poseB, 1, { cx: 0.7, cy: 0.7, aspect: 4 / 3, mirrorX: true }), name: 'Open_Palm', score: 0.9, handedness: 'Right' };
      out.push(t.update({ hands: two ? [left, right] : [left], now: i * 33, aspect: 4 / 3 }));
    }
    return out;
  };
  for (const pose of ['open', 'fist']) {
    for (const two of [false, true]) {
      const out = run(pose, pose, two);
      assert.ok(out.every((o) => !o.grab.started && !o.grab.active && !o.bothPinched && !o.zoomActive), `${pose}${two ? ' ×2' : ''} moved something`);
      assert.ok(out.every((o) => !('cast' in o)), 'the tracker has no cast output');
    }
  }
  const pinch = run('pinch');
  assert.ok(pinch.some((o) => o.grab.started), 'a one-hand pinch still starts a grab / map move');
  const zoom = run('pinch', 'pinch', true);
  assert.ok(zoom.some((o) => o.bothPinched), 'two pinched hands still zoom');
});
