// No heart rate anywhere on the headset input path (#153). Heart rate left the game in #99; these
// tests keep it from coming back through the wasm brain's ingest or the emulated headset's frames.
// Run: node --test tests/no_heart_rate.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as engine from '../pkg/mind_over_chicken_engine.js';
import { Brain, setRustBrain } from '../src/brain/brain.js';
import { SimSource, setRustSimSource } from '../src/brain/sources.js';

const BANDS = { delta: 0.25, theta: 0.2, alpha: 0.2, beta: 0.2, gamma: 0.15 };
const HR_KEYS = ['hr', 'heartRate', 'restHr', 'ppg', 'bpm'];

function withClasses(brainCls, simCls, fn) {
  setRustBrain(brainCls);
  setRustSimSource(simCls);
  try {
    return fn();
  } finally {
    setRustBrain(null);
    setRustSimSource(null);
  }
}

test('Brain passes RustBrain bands, blink, tilt, motion and quality only, never a heart rate', () => {
  const calls = [];
  class FakeRustBrain {
    ingest(...args) { calls.push(args); return false; }
  }
  withClasses(FakeRustBrain, null, () => {
    const b = new Brain({ update() {} });
    // A legacy frame that still has an hr field: it must not reach the wasm brain.
    b.ingest({ bands: BANDS, blink: true, hr: 95, tilt: { x: 0.4, y: -0.2 }, motion: 0.3, quality: 0.8 });
  });
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0], [0.25, 0.2, 0.2, 0.2, 0.15, true, 0.4, -0.2, 0.3, 0.8]);
  assert.equal(calls[0].includes(95), false, 'the heart rate reached RustBrain.ingest');
});

test('the wasm emulated headset frame has 10 numbers and no heart-rate slot', () => {
  // RustSimSource.step layout: delta, theta, alpha, beta, gamma, blink, tilt x, tilt y, motion, quality.
  class FakeRustSimSource {
    set_input() {}
    blink() {}
    step() { return Float32Array.from([0.3, 0.2, 0.2, 0.2, 0.1, 1, 0.5, -0.5, 0.25, 0.75]); }
  }
  const frames = [];
  withClasses(null, FakeRustSimSource, () => {
    const src = new SimSource();
    assert.ok(src.wasm, 'the fake wasm sim is used');
    src.update(0.35, (f) => frames.push(f));
  });
  assert.ok(frames.length >= 3);
  for (const f of frames) {
    for (const k of HR_KEYS) assert.equal(k in f, false, k);
    assert.equal(f.blink, true);
    assert.equal(f.motion, 0.25);
    assert.equal(f.quality, 0.75);
    assert.ok(Math.abs(f.bands.alpha - 0.2) < 1e-6);
  }
});

test('the real wasm engine: no heart-rate argument or slot, and the brain meters still move', () => {
  engine.initSync({ module: readFileSync(new URL('../pkg/mind_over_chicken_engine_bg.wasm', import.meta.url)) });
  const { RustBrain, RustSimSource } = engine;
  assert.equal(RustBrain.prototype.ingest.length, 10, 'RustBrain.ingest(bands x5, blink, tilt x/y, motion, quality)');
  const raw = new RustSimSource(153);
  const out = raw.step(0.1);
  assert.equal(out.length, 10, `RustSimSource.step: ${Array.from(out)}`);
  assert.ok(Array.from(out).every((v) => Math.abs(v) <= 1), `no pulse-sized value: ${Array.from(out)}`);
  raw.free();

  const warn = console.warn;
  const warnings = [];
  console.warn = (...a) => warnings.push(a.join(' '));
  try {
    withClasses(RustBrain, RustSimSource, () => {
      const src = new SimSource();
      const b = new Brain(src);
      assert.ok(src.wasm && b.wasm, 'both wasm classes load');
      const frames = [];
      const argCounts = new Set();
      const ingest = b.ingest.bind(b);
      b.ingest = (f) => { frames.push(f); ingest(f); };
      const wasmIngest = b.wasm.ingest.bind(b.wasm);
      b.wasm.ingest = (...a) => { argCounts.add(a.length); return wasmIngest(...a); };

      src.input.calm = true;
      for (let i = 0; i < 300; i++) b.update(0.05);
      assert.ok(frames.length > 100, `frames ${frames.length}`);
      for (const f of frames) for (const k of HR_KEYS) assert.equal(k in f, false, k);
      assert.deepEqual([...argCounts], [10]);
      assert.ok(b.wasm, 'the wasm brain stayed on (no ingest/update failure)');
      assert.ok(b.calm > 0.3, `calm meter rises with the calm key: ${b.calm}`);
      const calm = b.calm;

      src.input.calm = false;
      src.input.panic = true;
      for (let i = 0; i < 200; i++) b.update(0.05);
      assert.ok(b.calm < calm, `calm meter falls under the Stress key: ${b.calm} vs ${calm}`);
    });
  } finally {
    console.warn = warn;
  }
  assert.deepEqual(warnings, []);
});
