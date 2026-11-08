// camera (#189): the webcam has no head-pose model, so no camera copy may claim it tracks the head,
// and the panel names the real aim source (EEG headset / emulated / mouse).
// Run: node --test tests/camera_aim_source.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { aimSource, AIM_SOURCES, HEADSET_STALE_MS } from '../src/camera/aim-source.js';

test('aim source follows the brain source, not the webcam (#189)', () => {
  const now = 100_000;
  assert.equal(aimSource(null, now), AIM_SOURCES.none);
  assert.equal(aimSource({ source: { kind: 'sim' } }, now).id, 'emulated');

  const bridge = { kind: 'bridge', connected: true, streaming: true, lastFrameAt: now - 100 };
  assert.equal(aimSource({ source: bridge }, now).id, 'headset');
  assert.match(aimSource({ source: bridge }, now).label, /EEG headset/);

  // Connected but silent for longer than main.js's isEegLive window: the mouse aims again.
  assert.equal(aimSource({ source: { ...bridge, lastFrameAt: now - HEADSET_STALE_MS - 1 } }, now).id, 'mouse');
  assert.equal(aimSource({ source: { ...bridge, streaming: false } }, now).id, 'mouse');
  assert.equal(aimSource({ source: { ...bridge, connected: false } }, now).id, 'mouse');
  assert.equal(aimSource({ source: { ...bridge, lastFrameAt: 0 } }, now).id, 'mouse');
});

test('camera, coach and spell copy never claim webcam head tracking (#189)', () => {
  const claims = /head cursor|head aims|move (your )?head|MOVE HEAD|sole aim/i;
  for (const f of ['camera.js', 'coach.js', 'spells.js', 'debug.js']) {
    const src = readFileSync(new URL(`../src/camera/${f}`, import.meta.url), 'utf8');
    const hit = src.split('\n').find((line) => claims.test(line));
    assert.equal(hit, undefined, `${f} still claims head tracking: ${hit?.trim()}`);
  }
  const cam = readFileSync(new URL('../src/camera/camera.js', import.meta.url), 'utf8');
  assert.match(cam, /data-cam-aim/, 'the camera panel shows the aim source');
  assert.match(cam, /does not track your head/, 'the panel says the webcam does not track the head');
});
