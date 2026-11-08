// The galaxy picker: Left/Right move to the next planet on that side of the screen, wrapping.
import test from 'node:test';
import assert from 'node:assert/strict';
import { sideNeighbour } from '../src/cosmos/select.js';

const A = { name: 'A' }, B = { name: 'B' }, C = { name: 'C' };
const items = [{ p: B, x: 160 }, { p: A, x: 40 }, { p: C, x: 250 }];

test('right and left follow screen order', () => {
  assert.equal(sideNeighbour(items, A, 1), B);
  assert.equal(sideNeighbour(items, B, 1), C);
  assert.equal(sideNeighbour(items, C, -1), B);
});

test('wraps at the ends and starts from an edge when nothing is selected', () => {
  assert.equal(sideNeighbour(items, C, 1), A);
  assert.equal(sideNeighbour(items, A, -1), C);
  assert.equal(sideNeighbour(items, null, 1), A);
  assert.equal(sideNeighbour(items, null, -1), C);
  assert.equal(sideNeighbour([], A, 1), null);
});
