// The civ sandbox's planet size: the player's pick (title screen), the URL, or random (Alex).
import test from 'node:test';
import assert from 'node:assert/strict';
import { PLANET_SIZES, planetSize, planetChoice } from '../src/civ/planet-size.js';

const store = (v) => ({ getItem: () => v, setItem() {}, removeItem() {} });

test('a stored pick is used; anything else means random', () => {
  for (const p of PLANET_SIZES) assert.equal(planetChoice(store(p.name)), p.name);
  assert.equal(planetChoice(store(null)), null);
  assert.equal(planetChoice(store('gigantic')), null);
  assert.equal(planetChoice({ getItem() { throw new Error('blocked'); } }), null);
});

test('a picked size gives that planet and its peoples', () => {
  const huge = planetSize(123, 'huge');
  assert.deepEqual([huge.w, huge.h, huge.civs], [3072, 1536, 20]);
  const small = planetSize(123, 'small');
  assert.deepEqual([small.w, small.h, small.civs], [1280, 640, 8]);
});

test('random is one of the four sizes and varies with the seed', () => {
  const names = new Set();
  for (let s = 1; s < 200; s++) {
    const p = planetSize(s, null);
    assert.ok(PLANET_SIZES.some((q) => q.name === p.name));
    names.add(p.name);
  }
  assert.equal(names.size, PLANET_SIZES.length);
});
