// Poor contact and missing head tracking (src/brain/brain.js, #175).
import test from 'node:test';
import assert from 'node:assert/strict';
import { Brain, QUALITY_UNUSABLE, QUALITY_USABLE } from '../src/brain/brain.js';

const bands = { delta: 0.2, theta: 0.2, alpha: 0.3, beta: 0.2, gamma: 0.1 };
const fakeBridge = () => ({ kind: 'bridge', update() {} });
const feed = (brain, n, frame) => { for (let i = 0; i < n; i++) { brain.time += 0.1; brain.ingest({ bands, blink: false, ...frame }); } };

test('quality-0 frames make the signal unusable, and it recovers with hysteresis', () => {
  const b = new Brain(fakeBridge());
  feed(b, 10, { quality: 1 });
  assert.equal(b.signalUnusable, false);
  feed(b, 20, { quality: 0 });
  assert.equal(b.signalUnusable, true);
  assert.ok(b.signalQuality < QUALITY_UNUSABLE);
  // Climbing back past the unusable line is not enough: it must pass the usable line.
  while (b.signalQuality < (QUALITY_UNUSABLE + QUALITY_USABLE) / 2) feed(b, 1, { quality: 1 });
  assert.equal(b.signalUnusable, true);
  feed(b, 20, { quality: 1 });
  assert.equal(b.signalUnusable, false);
});

test('the emulator is never unusable', () => {
  const b = new Brain(fakeBridge());
  feed(b, 30, { quality: 0, isSim: true });
  assert.equal(b.signalUnusable, false);
});

test('head tracking is lost without tilt (null orientation) and back with it', () => {
  const b = new Brain(fakeBridge());
  feed(b, 30, { quality: 1, tilt: null });
  assert.equal(b.headTracking, false, 'a live headset with no orientation cannot aim');
  feed(b, 1, { quality: 1, tilt: { x: 0.2, y: -0.1 } });
  assert.equal(b.headTracking, true);
  feed(b, 25, { quality: 1 });   // 2.5 s with no tilt
  assert.equal(b.headTracking, false);
});
