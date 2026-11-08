// Signal truth chip (src/ui/hud.js signalSummary, #85).
import test from 'node:test';
import assert from 'node:assert/strict';
import { signalSummary } from '../src/ui/hud.js';

const bridge = (o = {}) => ({ kind: 'bridge', connected: true, lastFrameAt: 1000, ...o });
const brain = (o = {}) => ({ signalQuality: 0.9, signalUnusable: false, headTracking: true, ...o });

test('no chip for the emulator', () => {
  assert.equal(signalSummary({ source: { kind: 'sim', connected: true }, brain: brain() }, 1500), null);
});

test('quality levels, head tracking and calibration state', () => {
  const calib = { done: false, remaining: 6.2 };
  let s = signalSummary({ source: bridge(), brain: brain(), calibration: calib }, 1500);
  assert.equal(s.level, 'good');
  assert.equal(s.text, 'Signal good 90% · calibrating 7 s');
  s = signalSummary({ source: bridge(), brain: brain({ signalQuality: 0.55, headTracking: false }), calibration: { done: true } }, 1500);
  assert.equal(s.level, 'fair');
  assert.equal(s.text, 'Signal fair 55% · no head aim · calibrated');
  assert.match(s.title, /mouse and arrow keys aim/);
  s = signalSummary({ source: bridge(), brain: brain({ signalQuality: 0.3, signalUnusable: true }) }, 1500);
  assert.equal(s.level, 'poor');
  assert.match(s.title, /era waits/);
});

test('a bridge that stopped sending says so', () => {
  const s = signalSummary({ source: bridge({ lastFrameAt: 1000 }), brain: brain() }, 9000);
  assert.deepEqual([s.level, s.text], ['poor', 'Signal: no data']);
});
