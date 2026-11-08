// People invincibility toggle (#321): blocked deaths leave the tribe alive and stats unchanged.
import test from 'node:test';
import assert from 'node:assert/strict';
import { StoneAge } from '../src/eras/stone.js';

test('invincible people cannot be killed', () => {
  const s = Object.create(StoneAge.prototype);
  s.G = { invincible: true, stats: { deaths: 0 } };
  const p = { alive: true, state: 'walk', starve: 5 };
  s.kill(p);
  assert.equal(p.alive, true);
  assert.equal(p.state, 'idle');
  assert.equal(p.starve, 0);
  assert.equal(s.G.stats.deaths, 0);

  s.G.invincible = false;
  s.kill(p);
  assert.equal(p.alive, false);
  assert.equal(p.state, 'dead');
  assert.equal(s.G.stats.deaths, 1);
});
