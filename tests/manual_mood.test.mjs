// #155: a manual Happy ↔ Mix ↔ Surprised mood, no camera needed.
import test from 'node:test';
import assert from 'node:assert/strict';
import { manualMood, surpriseRisingEdge } from '../src/camera/manual-mood.js';

test('the three stops', () => {
  const happy = manualMood(0), mix = manualMood(0.5), surprised = manualMood(1);
  assert.deepEqual([happy.joy, happy.surprise, happy.dominant], [1, 0, 'joy']);
  assert.deepEqual([mix.joy, mix.surprise, mix.dominant], [1, 1, 'joy']);   // the joy buff stays on in a mix
  assert.deepEqual([surprised.joy, surprised.surprise, surprised.dominant], [0, 1, 'surprise']);
  for (const m of [happy, mix, surprised]) assert.equal(m.source, 'manual');
});

test('surprise fires once per crossing into the surprised side', () => {
  assert.equal(surpriseRisingEdge(0, 0.5), true);      // Mix: joy stays on and surprise fires once
  assert.equal(surpriseRisingEdge(0.5, 1), false);     // already there: no repeat
  assert.equal(surpriseRisingEdge(0.2, 0.4), false);
  assert.equal(surpriseRisingEdge(0.4, 0.8), true);
});
