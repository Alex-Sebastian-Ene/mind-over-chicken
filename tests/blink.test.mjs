// #145: blink-to-zap from every source, and every era that offers blink handles it.
// Blink is a power in the Stone Age, the civ sandbox, other worlds' surfaces and the galaxy; in
// Industry and Space its tile is off (#238) unless that era gives blink a job.
import test from 'node:test';
import assert from 'node:assert/strict';
import { Brain } from '../src/brain/brain.js';
import { StoneAge } from '../src/eras/stone.js';
import { Industry } from '../src/eras/industry.js';
import { SpaceAge } from '../src/eras/space.js';

const bands = { delta: 0.2, theta: 0.2, alpha: 0.2, beta: 0.2, gamma: 0.2 };

test('a blink from the headset or the emulator reaches the game once, debounced', () => {
  for (const isSim of [false, true]) {
    const b = new Brain({ update() {} });
    const hits = [];
    b.onBlink((aim) => hits.push({ ...aim }));
    b.time = 10;
    b.ingest({ bands, blink: true, isSim });
    b.time = 10.2;
    b.ingest({ bands, blink: true, isSim });   // the same blink's tail: ignored
    b.time = 11;
    b.ingest({ bands, blink: true, isSim });
    assert.equal(hits.length, 2, `isSim=${isSim}`);
    assert.ok(Number.isFinite(hits[0].x) && Number.isFinite(hits[0].y));
  }
});

test('every era that offers the blink power handles a blink', () => {
  for (const Scene of [StoneAge, Industry, SpaceAge]) {
    const s = new Scene();
    if (s.powers.includes('blink')) assert.equal(typeof s.onBlink, 'function', `${s.id} offers blink but ignores it`);
  }
  assert.ok(new StoneAge().powers.includes('blink'), 'the Stone Age is where lightning is learned');
});
