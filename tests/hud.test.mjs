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

// #178: per era, every visible tile changes the scene; the rest are disabled or not yet unlocked.
test('power tiles per era: on, disabled or hidden', async () => {
  const { tileStates } = await import('../src/ui/hud.js');
  const { StoneAge } = await import('../src/eras/stone.js');
  const { Industry } = await import('../src/eras/industry.js');
  const { SpaceAge } = await import('../src/eras/space.js');
  const { FarmingToKingdoms, InfoAge } = await import('../src/eras/timeskip.js');
  const row = (s) => { const t = tileStates(s); return ['blink', 'calm', 'focus', 'stress'].map((k) => t[k]).join(' '); };
  //                                        blink   calm    focus   stress
  assert.equal(row(new StoneAge()),          'on on hidden hidden');
  assert.equal(row(new FarmingToKingdoms()), 'off off on hidden');
  assert.equal(row(new Industry()),          'off on off on');
  assert.equal(row(new InfoAge()),           'off off off off');
  assert.equal(row(new SpaceAge()),          'off on on on');
  // Outside the campaign (galaxy, sandboxes, other worlds) nothing is locked.
  assert.equal(row({ powers: ['blink', 'calm', 'focus', 'stress'] }), 'on on on on');
  assert.equal(row({ era: 'surface', powers: [] }), 'off off off off');
  // Every 'on' tile is a power the scene lists.
  for (const s of [new StoneAge(), new FarmingToKingdoms(), new Industry(), new InfoAge(), new SpaceAge()]) {
    const t = tileStates(s);
    for (const k of Object.keys(t)) assert.equal(t[k] === 'on', s.powers.includes(k), `${s.id} ${k}`);
  }
});

test('setScene disables and hides the tiles that do nothing', () => {
  const doc = fakeDoc();
  const hud = new Hud(doc);
  hud.setScene({ label: 'Era III · Industry', era: 'industry', powers: ['stress', 'calm'] });
  const tile = (k) => doc.querySelector(`[data-power="${k}"]`);
  assert.deepEqual(['blink', 'calm', 'focus', 'stress'].map((k) => [tile(k).disabled, tile(k).hidden]), [[true, false], [false, false], [true, false], [false, false]]);
  assert.equal(tile('calm').classList.contains('is-active'), true);
  hud.setScene({ label: 'Era I', era: 'stone', powers: ['blink', 'calm'] });
  assert.equal(tile('stress').hidden, true);
  assert.equal(tile('blink').disabled, false);
});
