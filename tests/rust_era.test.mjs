// #194: only the playable eras run a Rust era sim; time skips and intro transitions don't.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { rustEraOf, rustEraTicks } from '../src/eras/rust-era.js';

test('a scene names its Rust era; a time skip has none', () => {
  assert.equal(rustEraOf({ rustEra: 0 }), 0);
  assert.equal(rustEraOf({ rustEra: 2 }), 2);
  assert.equal(rustEraOf({ era: 'farming' }), null);
  assert.equal(rustEraOf({ era: 'space' }), null);   // InfoAge reuses era 'space' but has no sim
  assert.equal(rustEraOf(null), null);
});

test('the era sim ticks only in a playable era outside its intro', () => {
  assert.equal(rustEraTicks({ rustEra: 1 }, false), true);
  assert.equal(rustEraTicks({ rustEra: 1 }, true), false);
  assert.equal(rustEraTicks({ era: 'farming' }, false), false);
});

test('Stone, Industry and Space declare their Rust era; the time skips do not', () => {
  const src = (f) => readFileSync(new URL(`../src/eras/${f}`, import.meta.url), 'utf8');
  assert.match(src('stone.js'), /this\.rustEra = 0;/);
  assert.match(src('industry.js'), /this\.rustEra = 1;/);
  assert.match(src('space.js'), /this\.rustEra = 2;/);
  assert.doesNotMatch(src('timeskip.js'), /rustEra/);
});
