// Every cosmos.method(...) that main.js calls exists on Cosmos. A bad merge brought back
// cosmos.applyPan (removed in #129) and the 2D civ sandbox threw a TypeError every frame.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { Cosmos } from '../src/cosmos/cosmos.js';

test('main.js only calls cosmos methods that exist', async () => {
  const src = await readFile(new URL('../src/main.js', import.meta.url), 'utf8');
  const called = new Set([...src.matchAll(/\bcosmos\.(\w+)\(/g)].map((m) => m[1]));
  assert.ok(called.size > 3);
  const missing = [...called].filter((name) => typeof Cosmos.prototype[name] !== 'function');
  assert.deepEqual(missing, []);
});
