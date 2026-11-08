// camera (#33, #122, #150, #159): gesture → cast debounce, cooldowns and the fist-to-open fireball,
// run against BOTH controllers: the JS one (src/camera/casts.js) and the wasm RustCastController the
// browser actually runs (crates/engine/src/camera/casts.rs, loaded from pkg/). The browser used to
// run the old seven-cast wasm table while Node tested the JS one, so keys 2 and 3 were dead in the
// game and a held fist or palm cast legacy spells, with every Node test green.
// Run: node --test tests/camera_casts.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as wasm from '../pkg/mind_over_chicken_engine.js';
import { CastController, setRustCastController } from '../src/camera/casts.js';
import { SPELL_BY_ID } from '../src/camera/spells.js';

wasm.initSync({ module: readFileSync(new URL('../pkg/mind_over_chicken_engine_bg.wasm', import.meta.url)) });

const BACKENDS = {
  js: () => { setRustCastController(null); try { return new CastController(); } finally { setRustCastController(null); } },
  wasm: () => { setRustCastController(wasm.RustCastController); try { return new CastController(); } finally { setRustCastController(null); } },
};

// Feed a gesture at 30 fps over [t0, t0 + ms]; return the casts that fired and the last update.
function feed(c, gesture, t0, ms, score = 0.9, step = 1000 / 30) {
  const casts = [];
  let last;
  for (let t = t0; t <= t0 + ms; t += step) {
    last = c.update(gesture, score, t);
    if (last.cast) casts.push({ id: last.cast, t });
  }
  return { casts, last };
}

for (const [name, make] of Object.entries(BACKENDS)) {
  test(`[${name}] the controller runs the expected backend`, () => {
    const c = make();
    assert.equal(!!c.wasm, name === 'wasm');
  });

  test(`[${name}] gestures map to the three core moves; legacy gestures cast nothing`, () => {
    const c = make();
    assert.equal(c.spellFor('Open_Palm'), 'palm');
    assert.equal(c.spellFor('Closed_Fist'), 'fireball');
    for (const g of ['Pointing_Up', 'Victory', 'Thumb_Up', 'Thumb_Down', 'ILoveYou', 'None']) {
      assert.equal(c.spellFor(g) ?? null, null, g);
      assert.equal(feed(make(), g, 0, 3000).casts.length, 0, `${g} held 3 s`);
    }
  });

  test(`[${name}] keys 2 and 3 cast (they were dead on the old wasm table)`, () => {
    const c = make();
    assert.equal(c.trigger('palm', 0), true);
    assert.equal(c.trigger('fireball', 0), true);
    assert.equal(c.trigger('fist', 0), false, 'no Strike move (#159)');
    assert.equal(c.trigger('spawn', 0), false);
  });

  test(`[${name}] a held fist charges but never casts (#150, #159)`, () => {
    const c = make();
    const { casts, last } = feed(c, 'Closed_Fist', 0, 20000);
    assert.equal(casts.length, 0);
    assert.equal(last.held, 'fireball');
    assert.equal(last.progress, 1);
    assert.equal(last.armed, 'fireball');
  });

  test(`[${name}] fist then open hand releases exactly one fireball, and no Calm from the same hand`, () => {
    const c = make();
    feed(c, 'Closed_Fist', 0, 500);
    c.update('None', 0, 533);           // transition frames between the poses
    c.update('None', 0, 566);
    const { casts } = feed(c, 'Open_Palm', 600, 2400);
    assert.deepEqual(casts.map((x) => x.id), ['fireball']);
    assert.equal(casts[0].t, 600);
    // Leave the palm, then a fresh palm hold calms as usual.
    feed(c, 'None', 3033, 400);
    assert.deepEqual(feed(c, 'Open_Palm', 3466, 600).casts.map((x) => x.id), ['palm']);
  });

  test(`[${name}] a held palm alone calms once and never fires`, () => {
    const c = make();
    const { casts } = feed(c, 'Open_Palm', 0, 20000);
    assert.deepEqual(casts.map((x) => x.id), ['palm']);
    assert.ok(casts[0].t >= 300 && casts[0].t < 340);
  });

  test(`[${name}] a too-short fist or a too-late open hand does not fire`, () => {
    const c = make();
    feed(c, 'Closed_Fist', 0, 200);
    assert.ok(feed(c, 'Open_Palm', 233, 1500).casts.every((x) => x.id === 'palm'));
    const d = make();
    feed(d, 'Closed_Fist', 0, 500);
    feed(d, 'None', 533, 900);
    const late = d.update('Open_Palm', 0.9, 1466);
    assert.equal(late.cast ?? null, null);
    assert.equal(late.armed ?? null, null);
  });

  test(`[${name}] the fireball cooldown eats a release instead of queueing it`, () => {
    const c = make();
    feed(c, 'Closed_Fist', 0, 400);
    assert.equal(c.update('Open_Palm', 0.9, 433).cast, 'fireball');
    feed(c, 'None', 466, 250);
    feed(c, 'Closed_Fist', 733, 500);
    assert.equal(feed(c, 'Open_Palm', 1266, 700).casts.length, 0);
    assert.ok(c.cooldownLeft('fireball', 1266) > 0);
    // After the cooldown, fist then open fires again.
    feed(c, 'None', 2000, 300);
    feed(c, 'Closed_Fist', 2400, 400);
    assert.equal(c.update('Open_Palm', 0.9, 2833).cast, 'fireball');
  });

  test(`[${name}] low-confidence gestures are ignored`, () => {
    const c = make();
    assert.equal(feed(c, 'Open_Palm', 0, 1000, 0.3).casts.length, 0);
    feed(c, 'Closed_Fist', 1100, 500);
    assert.equal(c.update('Open_Palm', 0.3, 1633).cast ?? null, null);
  });

  test(`[${name}] short tracking dropouts do not break a hold, longer ones do`, () => {
    const c = make();
    c.update('Open_Palm', 0.9, 0);
    c.update('Open_Palm', 0.9, 150);
    c.update('None', 0, 200);
    assert.equal(c.update('Open_Palm', 0.9, 310).cast, 'palm');
    const d = make();
    d.update('Open_Palm', 0.9, 0);
    d.update('Open_Palm', 0.9, 150);
    d.update('None', 0, 200);
    d.update('None', 0, 400);
    assert.equal(d.update('Open_Palm', 0.9, 420).cast ?? null, null);
    assert.equal(d.update('Open_Palm', 0.9, 720).cast, 'palm');
  });

  test(`[${name}] keyboard casts share the cooldowns`, () => {
    const c = make();
    const cd = SPELL_BY_ID.palm.cooldown;
    assert.equal(c.trigger('palm', 0), true);
    assert.equal(c.trigger('palm', 1000), false);
    assert.ok(Math.abs(c.cooldownFrac('palm', 0) - 1) < 1e-9);
    assert.ok(c.cooldownFrac('palm', cd / 2) > 0.49);
    assert.equal(c.cooldownLeft('palm', cd), 0);
    assert.equal(c.trigger('palm', cd), true);
    c.trigger('palm', 0);
    assert.equal(feed(c, 'Open_Palm', 10, 1000).casts.length, 0);
    assert.equal(c.trigger('nonsense', 0), false);
    assert.equal(c.trigger('fireball', 0), true);
    assert.equal(c.trigger('fireball', SPELL_BY_ID.fireball.cooldown - 1), false);
  });
}

test('the JS spell table and the wasm table agree on cooldowns', () => {
  const c = BACKENDS.wasm();
  for (const s of Object.values(SPELL_BY_ID)) {
    assert.equal(c.trigger(s.id, 0), true, s.id);
    assert.ok(Math.abs(c.cooldownLeft(s.id, 0) - s.cooldown) < 1e-6, `${s.id} cooldown`);
  }
});
