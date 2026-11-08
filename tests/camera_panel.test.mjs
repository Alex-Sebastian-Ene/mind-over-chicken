// camera panel (#183): opening the Camera sidebar must not ask for the webcam.
// Run: node --test tests/camera_panel.test.mjs
//
// initCamera() runs against a tiny fake DOM, with navigator.mediaDevices.getUserMedia replaced by a
// counter that rejects (no real camera is touched). The toolbar button and the ` alias only open
// the sidebar; Start Camera is the one control that requests media.

import test from 'node:test';
import assert from 'node:assert/strict';

// ---------- a fake DOM just big enough for initCamera ----------
function fakeElement(tag = 'div') {
  const listeners = new Map();
  const cache = new Map();
  const ctx = new Proxy({}, { get: (t, k) => (k in t ? t[k] : () => ({})), set: (t, k, v) => { t[k] = v; return true; } });
  const el = {
    tagName: tag.toUpperCase(),
    hidden: false,
    disabled: false,
    textContent: '',
    innerHTML: '',
    width: 320,
    height: 240,
    clientWidth: 0,
    clientHeight: 0,
    readyState: 0,
    dataset: {},
    style: { setProperty() {} },
    attrs: {},
    classList: { toggle() {}, add() {}, remove() {}, contains: () => false },
    setAttribute(k, v) { this.attrs[k] = String(v); },
    getAttribute(k) { return this.attrs[k] ?? null; },
    addEventListener(type, fn) { if (!listeners.has(type)) listeners.set(type, []); listeners.get(type).push(fn); },
    dispatch(type, ev = {}) { for (const fn of listeners.get(type) || []) fn({ preventDefault() {}, ...ev }); },
    click() { this.dispatch('click'); },
    querySelector(sel) { if (!cache.has(sel)) cache.set(sel, fakeElement()); return cache.get(sel); },
    querySelectorAll() { return []; },
    insertAdjacentElement() {},
    getContext() { return ctx; },
    play: async () => {},
  };
  return el;
}

function installDom() {
  const byId = {
    camBtn: fakeElement('button'),
    camPanel: Object.assign(fakeElement('aside'), { hidden: true }),
    screen: fakeElement('canvas'),
    eegPanel: Object.assign(fakeElement('aside'), { hidden: true }),
    eegBtn: fakeElement('button'),
  };
  const winListeners = new Map();
  globalThis.document = {
    getElementById: (id) => byId[id] || null,   // no coachPanel: the coach is skipped
    createElement: (tag) => fakeElement(tag),
    body: { dataset: {} },
  };
  globalThis.window = {
    isSecureContext: true,
    devicePixelRatio: 1,
    addEventListener(type, fn) { if (!winListeners.has(type)) winListeners.set(type, []); winListeners.get(type).push(fn); },
    dispatch(type, ev) { for (const fn of winListeners.get(type) || []) fn(ev); },
    __moc: null,
  };
  globalThis.requestAnimationFrame = () => 0;
  const media = { calls: 0 };
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: {
      mediaDevices: {
        getUserMedia: async () => {
          media.calls++;
          throw Object.assign(new Error('denied in test'), { name: 'NotAllowedError' });
        },
      },
    },
  });
  return { byId, media };
}

test('opening the Camera sidebar requests no media; Start Camera does (#183)', async () => {
  const { byId, media } = installDom();
  const realSetInterval = globalThis.setInterval;
  globalThis.setInterval = () => 0;   // initCamera's fallback tick timer would keep Node alive
  let cam;
  try {
    const { initCamera } = await import('../src/camera/camera.js');
    cam = initCamera({ getG: () => null });
  } finally {
    globalThis.setInterval = realSetInterval;
  }
  assert.ok(cam, 'initCamera found its button and panel');
  const { camBtn, camPanel } = byId;

  // Toolbar button: opens the sidebar, no permission prompt.
  camBtn.click();
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(camPanel.hidden, false, 'the button opens the sidebar');
  assert.equal(camBtn.getAttribute('aria-pressed'), 'true');
  assert.equal(media.calls, 0, 'opening the sidebar must not call getUserMedia');
  assert.equal(cam.status, 'off');

  // Second click closes it, still no request.
  camBtn.click();
  assert.equal(camPanel.hidden, true);
  assert.equal(media.calls, 0);

  // The debug/` path uses togglePanel: also no request.
  cam.togglePanel(true);
  assert.equal(camPanel.hidden, false);
  assert.equal(media.calls, 0);

  // The opt-in note explains the prompt before it happens.
  assert.match(camPanel.innerHTML, /data-cam-optin[^>]*>The webcam stays off until you click <b>Start Camera<\/b>/, 'panel has an opt-in note');

  // Start Camera is the one place that asks.
  camPanel.querySelector('[data-cam-start]').click();
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(media.calls, 1, 'Start Camera calls getUserMedia once');
  assert.equal(cam.status, 'error', 'a denied prompt leaves the camera in the error state');
});

test('the panel template advertises Start Camera as the only opt-in (#183)', async () => {
  const { readFileSync } = await import('node:fs');
  const src = readFileSync(new URL('../src/camera/camera.js', import.meta.url), 'utf8');
  assert.match(src, /data-cam-optin/);
  assert.doesNotMatch(src, /if \(status === 'off'\) start\(\)/, 'the toolbar button must not start the camera');
  assert.doesNotMatch(src, /click Camera again/, 'errors point at Start Camera, not the toolbar button');
});
