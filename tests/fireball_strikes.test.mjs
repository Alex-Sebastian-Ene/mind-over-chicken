// The fireball's slam does something: it strikes like lightning where it lands (lights the pit,
// roasts a giant chicken, sets a tree alight; in Industry it hits factories, pickets, the exchange).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('the Stone Age and Industry fireball impacts strike at the impact point', async () => {
  for (const f of ['stone', 'industry']) {
    const src = await readFile(new URL(`../src/eras/${f}.js`, import.meta.url), 'utf8');
    const i = src.indexOf('  impactFireball(');
    const body = src.slice(i, src.indexOf('\n  }\n', i));
    assert.match(body, /this\.lastBolt = -Infinity;/, f);
    assert.match(body, /this\.onBlink\((this\.)?G\.world\.aimOf\(x, y\)\)/, f);
  }
});
