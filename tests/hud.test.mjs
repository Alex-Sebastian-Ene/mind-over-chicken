// HUD (src/ui/hud.js) without a browser: a tiny fake document is enough for the class.
import test from 'node:test';
import assert from 'node:assert/strict';
import { Hud, setRustHudEngine } from '../src/ui/hud.js';

function fakeEl() {
  return {
    hidden: false, textContent: '', innerHTML: '', dataset: {},
    classList: { _s: new Set(), add(c) { this._s.add(c); }, remove(c) { this._s.delete(c); }, toggle(c, on) { if (on ?? !this._s.has(c)) this._s.add(c); else this._s.delete(c); }, contains(c) { return this._s.has(c); } },
    style: { setProperty() {} },
    children: [],
    querySelector(sel) { this._q ||= {}; return (this._q[sel] ||= sel === '.power-icon' ? { getContext: () => ({ clearRect() {}, fillRect() {}, set fillStyle(_) {} }) } : fakeEl()); },
    replaceChildren() {},
  };
}
function fakeDoc() {
  const byId = {};
  return {
    byId,
    getElementById: (id) => (byId[id] ||= fakeEl()),
    querySelector: (sel) => (byId[sel] ||= fakeEl()),
  };
}
const fakeBrain = () => ({
  calm: 0.4, focus: 0.2, stress: 0.1, cue: 'rest', personal: {}, genericCalm: 0.3, genericFocus: 0.2,
  stats: { blinks: 0 },
  threshold: () => 0.5,
  model: { count: () => 3, trained: false, accuracy: 0, chance: 0.33, trust: () => 0, trainCount: 0, lastTrainMs: 0, weights: () => [] },
});

test('the model panel renders in the WebGL build instead of throwing every frame (#173)', () => {
  const doc = fakeDoc();
  const hud = new Hud(doc);
  setRustHudEngine({}); // the Rust DOM shell is bound: it draws the meters, the panel is ours
  try {
    doc.getElementById('modelPanel').hidden = false;
    const G = { brain: fakeBrain(), runTime: 3, scene: null, calibration: null };
    assert.doesNotThrow(() => hud.update(0.016, G));
    assert.match(doc.getElementById('modelBody').innerHTML, /Rest windows/);
    doc.getElementById('modelPanel').hidden = true;
    doc.getElementById('modelBody').innerHTML = '';
    hud.update(0.016, G);
    assert.equal(doc.getElementById('modelBody').innerHTML, '', 'closed panel: nothing to draw');
  } finally {
    setRustHudEngine(null);
  }
});
