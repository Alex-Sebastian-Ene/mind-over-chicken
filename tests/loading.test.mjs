// Loading screen state (src/ui/loading.js). Runs without a DOM: the module must not touch the
// document when there isn't one (Node tests import src/ai/* and src/cosmos/*, which use it).
import test from 'node:test';
import assert from 'node:assert/strict';
import { loading } from '../src/ui/loading.js';

const state = (id) => loading.snapshot().find((s) => s.id === id)?.state;

test('blocking and background steps go run -> done / fail', () => {
  loading.reset();
  loading.start('engine', 'Starting the game engine', { blocking: true });
  assert.equal(state('engine'), 'run');
  assert.equal(loading.snapshot()[0].blocking, true);
  loading.done('engine');
  assert.equal(state('engine'), 'done');
  loading.start('cam', 'Loading the camera models');
  loading.fail('cam', 'no camera');
  const cam = loading.snapshot().find((s) => s.id === 'cam');
  assert.deepEqual([cam.state, cam.note, cam.blocking], ['fail', 'no camera', false]);
});

test('track() follows the promise and returns it unchanged', async () => {
  loading.reset();
  const ok = loading.track('policy', 'Loading the AI god (PPO policy)', Promise.resolve(42));
  assert.equal(state('policy'), 'run');
  assert.equal(await ok, 42);
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(state('policy'), 'done');

  const bad = loading.track('world-1', 'Forming a planet', Promise.reject(new Error('worker died')), { group: 'worlds' });
  await assert.rejects(bad, /worker died/);
  await new Promise((r) => setTimeout(r, 0));
  const w = loading.snapshot().find((s) => s.id === 'world-1');
  assert.deepEqual([w.state, w.note], ['fail', 'worker died']);
});

test('unknown ids are ignored', () => {
  loading.reset();
  loading.done('nope');
  loading.fail('nope');
  assert.deepEqual(loading.snapshot(), []);
});
