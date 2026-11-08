// The era intro transition (src/ui/intro.js): timing, skip and input hold.
// Run: node --test tests/intro.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { EraIntro, INTRO, easeOutCubic } from '../src/ui/intro.js';

test('the intro runs for its length, then ends once and calls back', () => {
  let ended = 0;
  const intro = new EraIntro({ seconds: 3 }).start({ title: 'Stone Age', goal: 'Grow the tribe' }, () => ended++);
  assert.equal(intro.active, true);
  assert.equal(intro.update(1), true);
  assert.equal(intro.update(1.5), true);
  assert.equal(ended, 0);
  assert.equal(intro.update(0.6), false);
  assert.equal(intro.active, false);
  assert.equal(ended, 1);
  intro.update(5);
  intro.skip();
  assert.equal(ended, 1, 'onEnd runs exactly once');
});

test('Space, Enter or a click skip it; everything else is held until it ends', () => {
  let ended = 0;
  const intro = new EraIntro().start({ title: 'Industry' }, () => ended++);
  for (const code of ['KeyB', 'KeyF', 'ShiftLeft', 'KeyA', 'KeyZ', 'ArrowLeft']) assert.equal(intro.gate('key', code), 'hold', code);
  for (const kind of ['wheel', 'blink', 'cast']) assert.equal(intro.gate(kind), 'hold', kind);
  for (const code of INTRO.skipKeys) assert.equal(intro.gate('key', code), 'skip', code);
  assert.equal(intro.gate('pointer'), 'skip');
  assert.equal(ended, 0, 'gating alone does not end it');
  intro.skip();
  assert.equal(ended, 1);
  assert.equal(intro.active, false);
  // Once it has ended, inputs pass straight through.
  assert.equal(intro.gate('key', 'KeyB'), 'pass');
  assert.equal(intro.gate('pointer'), 'pass');
});

test('the camera eases onto the stage and the caption fades in and out', () => {
  const intro = new EraIntro({ seconds: 4 }).start({ title: 'Space Age', goal: 'Launch' });
  assert.equal(intro.away(), 1);
  assert.equal(intro.captionAlpha(), 0);
  intro.update(1);
  assert.ok(intro.away() > 0 && intro.away() < 1);
  assert.equal(intro.captionAlpha(), 1);
  let last = intro.away();
  for (let i = 0; i < 20; i++) { intro.update(0.14); assert.ok(intro.away() <= last); last = intro.away(); }
  intro.update(0.15);
  assert.ok(intro.captionAlpha() < 1, 'fading out near the end');
  intro.update(1);
  assert.equal(intro.away(), 0);
  assert.equal(intro.captionAlpha(), 0);
  assert.equal(easeOutCubic(0), 0);
  assert.equal(easeOutCubic(1), 1);
});

test('cancel drops it without the callback (Quit to Title, a new game)', () => {
  let ended = 0;
  const intro = new EraIntro().start({ title: 'Stone Age' }, () => ended++);
  intro.cancel();
  assert.equal(intro.active, false);
  intro.update(10);
  assert.equal(ended, 0);
});
