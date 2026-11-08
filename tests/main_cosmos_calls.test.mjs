// main.js drives the zoom through `cosmos.<method>(…)`. A merge once brought back a call to
// cosmos.applyPan, which #129 had removed, and every 2D frame of the civ sandbox and the other
// worlds threw "cosmos.applyPan is not a function" (release QA). Every method main.js calls on
// cosmos must exist on Cosmos.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const main = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
const cosmosSrc = readFileSync(new URL('../src/cosmos/cosmos.js', import.meta.url), 'utf8')
  + readFileSync(new URL('../src/cosmos/zoom.js', import.meta.url), 'utf8');

test('every cosmos method main.js calls is defined', () => {
  const called = new Set([...main.matchAll(/\bcosmos\??\.([A-Za-z_]\w*)\??\.?\(/g)].map((m) => m[1]));
  assert.ok(called.size > 10, 'found the calls');
  const defined = (name) => new RegExp(String.raw`^\s+(?:async\s+|get\s+|static\s+)?${name}\s*\(`, 'm').test(cosmosSrc)
    || new RegExp(String.raw`this\.${name}\s*=`).test(cosmosSrc);
  const missing = [...called].filter((name) => !defined(name));
  assert.deepEqual(missing, []);
});
