// The Debug panel opens on its own (src/camera/debug.js, #187). It handed its toggle to the
// Camera panel, so the ` key and the (hidden) button opened Camera and Debug never showed.
import test from 'node:test';
import assert from 'node:assert/strict';

function fakeEl(id = '') {
  const handlers = {};
  const kids = {};
  return {
    id, hidden: true, textContent: '', innerHTML: '', dataset: {}, style: { setProperty() {} },
    attrs: {}, setAttribute(k, v) { this.attrs[k] = v; },
    addEventListener(t, fn) { (handlers[t] ||= []).push(fn); },
    click() { (handlers.click || []).forEach((fn) => fn({})); },
    querySelector(sel) { return (kids[sel] ||= fakeEl(sel)); },
    querySelectorAll() { return []; },
    getContext: () => new Proxy({}, { get: () => () => {} }),
    replaceChildren() {}, append() {},
    classList: { add() {}, remove() {}, toggle() {} },
    width: 160, height: 120,
  };
}

test('Debug opens its own panel, from the button and the ` key, and Camera and Debug close each other', async () => {
  const els = {};
  globalThis.document = { getElementById: (id) => (els[id] ||= fakeEl(id)), querySelector: () => null, createElement: () => fakeEl() };
  const keys = [];
  globalThis.window = { addEventListener: (t, fn) => { if (t === 'keydown') keys.push(fn); } };
  globalThis.requestAnimationFrame = () => 0;
  const intervals = [];
  const realSetInterval = globalThis.setInterval;
  globalThis.setInterval = (fn) => { intervals.push(fn); return 0; };
  try {
    const { initDebug, DEBUG_KEY } = await import('../src/camera/debug.js');
    const cam = els.camPanel = fakeEl('camPanel');
    cam.hidden = false;   // the Camera sidebar is open
    const camera = { togglePanel: (force) => { cam.hidden = !(force ?? cam.hidden); } };
    const dbg = initDebug({ getG: () => null, camera });
    assert.ok(dbg, 'initDebug found its button and panel');
    const panel = els.dbgPanel;

    els.dbgBtn.click();
    assert.equal(panel.hidden, false, 'the button opens the debug panel');
    assert.equal(cam.hidden, true, 'and closes the Camera panel');
    assert.equal(dbg.open, true);

    const press = () => keys.forEach((fn) => fn({ code: DEBUG_KEY, preventDefault() {}, target: null }));
    press();
    assert.equal(panel.hidden, true, 'the ` key closes it');
    press();
    assert.equal(panel.hidden, false, 'and opens it again');
    panel.hidden = true;   // Camera opened and closed it (camera.js togglePanel)
    press();
    assert.equal(panel.hidden, false, 'one press reopens it after Camera closed it');
  } finally {
    globalThis.setInterval = realSetInterval;
    delete globalThis.document; delete globalThis.window; delete globalThis.requestAnimationFrame;
  }
});
