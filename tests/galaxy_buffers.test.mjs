// galaxy buffers (#216): the Rust galaxy packs every planet and ship into one contiguous float
// buffer, and src/cosmos/galaxy.js reads it through a single cached Float32Array view over wasm
// memory rather than four per-entity getter calls per planet per frame.
// Run: node --test tests/galaxy_buffers.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as wasm from '../pkg/mind_over_chicken_engine.js';
import { Galaxy, setRustGalaxy } from '../src/cosmos/galaxy.js';

wasm.initSync({ module: readFileSync(new URL('../pkg/mind_over_chicken_engine_bg.wasm', import.meta.url)) });

const brain = { calm: 0.5, focus: 0.5, stress: 0.2, blinks: 0 };

test('RustGalaxy exposes one packed planet buffer with the documented layout (#216)', () => {
  const g = new wasm.RustGalaxy(4242);
  g.step(0.1, 0.5, 0.5, 0.2, 0, 0);

  const stride = g.planet_stride();
  assert.equal(stride, 9);
  assert.equal(g.planets_len(), g.planet_count() * stride);

  const memory = wasm.engine_memory();
  assert.ok(memory instanceof WebAssembly.Memory);
  const view = new Float32Array(memory.buffer, g.planets_ptr(), g.planets_len());
  assert.equal(view.length, g.planets_len());

  for (let i = 0; i < g.planet_count(); i++) {
    const o = i * stride;
    assert.equal(view[o], g.planet_pop(i), `planet ${i} pop`);
    assert.equal(view[o + 1], g.planet_era(i), `planet ${i} era`);
    assert.equal(view[o + 2], g.planet_progress(i), `planet ${i} progress`);
    assert.equal(view[o + 3], g.planet_market(i), `planet ${i} market`);
    assert.equal(view[o + 5], g.planet_is_alive(i) ? 1 : 0, `planet ${i} alive`);
    assert.equal(view[o + 6], g.planet_is_forming(i) ? 1 : 0, `planet ${i} forming`);
    assert.equal(view[o + 7], g.planet_pos_x(i), `planet ${i} x`);
    assert.equal(view[o + 8], g.planet_pos_y(i), `planet ${i} y`);
  }
});

test('RustGalaxy exposes a packed ship buffer (#216)', () => {
  const g = new wasm.RustGalaxy(12345);
  assert.equal(g.ship_stride(), 4);
  assert.equal(g.ships_len(), 0);

  // A calm, focused galaxy with three formed worlds launches colony ships. Run until one exists.
  for (let i = 0; i < 1000 && g.ships_len() === 0; i++) g.step(0.1, 0.9, 0.5, 0, 0, 0);
  const len = g.ships_len();
  assert.ok(len >= 4, 'a colony ship is in flight');

  const view = new Float32Array(wasm.engine_memory().buffer, g.ships_ptr(), len);
  assert.equal(view.length, len);
  for (let i = 0; i < len / 4; i++) {
    const o = i * 4;
    assert.equal(view[o], g.ship_from(i), `ship ${i} from`);
    assert.equal(view[o + 1], g.ship_to(i), `ship ${i} to`);
    assert.equal(view[o + 2], g.ship_t(i), `ship ${i} progress`);
    assert.ok(view[o + 3] > 0, `ship ${i} duration`);
  }
});

test('Galaxy reuses one Float32Array view over the Rust buffer (#216)', () => {
  setRustGalaxy(wasm.RustGalaxy, wasm.engine_memory);
  try {
    const gal = new Galaxy();
    gal.addHome({ biome: 'temperate', seed: 1 });
    gal.addMindWorlds();
    gal.update(0.1, brain, { eraIndex: 0, pop: 0, canLaunch: false });

    const view = gal.wasmPlanets();
    assert.ok(view instanceof Float32Array, 'a view is created');
    assert.equal(view.length, gal.wasm.planets_len());
    assert.equal(gal.wasmPlanets(), view, 'the same view is reused across reads');

    // The view is a live window, not a copy: stepping refreshes the numbers in place.
    gal.wasm.step(1, 0.5, 0.5, 0.2, 0, 0);
    const refreshed = gal.wasmPlanets();
    assert.equal(refreshed[0], gal.wasm.planet_pop(0), 'pop is read from the refreshed buffer');
  } finally {
    setRustGalaxy(null, null);
  }
});

test('Galaxy rebuilds the view if it is detached or stale (#216)', () => {
  setRustGalaxy(wasm.RustGalaxy, wasm.engine_memory);
  try {
    const gal = new Galaxy();
    gal.addHome({ biome: 'temperate', seed: 2 });
    gal.addMindWorlds();
    gal.update(0.1, brain, { eraIndex: 0, pop: 0, canLaunch: false });

    // Simulate a detached/stale view (wasm memory grew): the next read must not return it.
    gal.planetView = new Float32Array(1);
    const fresh = gal.wasmPlanets();
    assert.ok(fresh instanceof Float32Array);
    assert.notEqual(fresh.length, 1);
    assert.equal(fresh, gal.planetView);
    assert.equal(fresh.length, gal.wasm.planets_len());
  } finally {
    setRustGalaxy(null, null);
  }
});
