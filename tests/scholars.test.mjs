// #223: the Farming → Kingdoms scholars write inventions as you focus; no arithmetic.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { INVENTIONS, inkedLetters } from '../src/eras/timeskip.js';

test('three inventions, none of them sums', () => {
  assert.equal(INVENTIONS.length, 3);
  for (const i of INVENTIONS) assert.doesNotMatch(i.name, /[×÷+\-=\d]/);
  const src = readFileSync(new URL('../src/eras/timeskip.js', import.meta.url), 'utf8');
  assert.doesNotMatch(src, /makeProblems|Solve it in your head/);
});

test('focus inks the name faster; time alone writes half of it', () => {
  assert.equal(inkedLetters('Writing', 0, 0, 4.4), '');
  assert.equal(inkedLetters('Writing', 4.4, 0, 4.4), 'Writ');        // no focus: half by the end
  assert.equal(inkedLetters('Writing', 1, 1, 4.4), 'Writing');       // full focus: finished early
  assert.ok(inkedLetters('Law', 2.2, 0.6, 4.4).length >= 2);
});
