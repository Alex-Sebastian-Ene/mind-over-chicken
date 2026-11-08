// Hand moves are off by default: the real setup is the EEG headset plus the face for mood.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { HANDS_KEY, handsEnabled, setHandsEnabled } from '../src/camera/hands-pref.js';

const memory = () => { const m = new Map(); return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)) }; };

test('hand moves start off and the choice is remembered', () => {
  const store = memory();
  assert.equal(handsEnabled(store), false);
  assert.equal(setHandsEnabled(true, store), true);
  assert.equal(store.getItem(HANDS_KEY), '1');
  assert.equal(handsEnabled(store), true);
  setHandsEnabled(false, store);
  assert.equal(handsEnabled(store), false);
});

test('no storage (private window, blocked site data) means hands off, without throwing', () => {
  const broken = { getItem() { throw new Error('denied'); }, setItem() { throw new Error('denied'); } };
  assert.equal(handsEnabled(broken), false);
  assert.equal(handsEnabled(undefined), false);
  assert.equal(setHandsEnabled(true, broken), true);
});

test('the camera only runs the hand model and reads gestures when hand moves are on', async () => {
  const src = await readFile(new URL('../src/camera/camera.js', import.meta.url), 'utf8');
  assert.match(src, /engine\.pump\(video, now, \{ hands: handsOn,/);
  assert.match(src, /function onGesture\(r\) \{\n\s+if \(!stream \|\| status !== 'on' \|\| !handsOn\) return;/);
  assert.match(src, /data-cam-hands/);
});
