// The tribe stays visible when the campfire goes out. Rendering used to be gated on
// `this.fire.lit`, so every person vanished the moment the flame died (in both the flat and
// the 3D-sprite passes) even though the simulation kept them walking around.
import test from 'node:test';
import assert from 'node:assert/strict';
import '../rl/headless.mjs';   // canvas stand-in (World.paint) and the engine wasm
import { SimPlanet } from '../src/ai/planet.js';

function recordingCtx() {
  const rects = [];
  const target = {
    fillRect: (...args) => rects.push(args),
    drawImage: () => {},
    measureText: () => ({ width: 0 }),
  };
  const ctx = new Proxy(target, {
    get: (t, k) => (k in t ? t[k] : (() => {})),
    set: (t, k, v) => { t[k] = v; return true; },
  });
  return { ctx, rects };
}

function sceneWithLonePerson() {
  const s = new SimPlanet({ seed: 7, brain: 'proxy' }).scene;
  s.mammoths = [];
  s.colonies = {};
  s.forest = { fires: [] };
  s.chicken = { x: 4, y: 4, frame: 0, facing: 1 };
  s.fire = { x: 8, y: 8, heat: 0, lit: false, everLit: true };
  s.people = [{ x: 100, y: 100, alive: true, state: 'walk', frame: 0, facing: 1, halo: false }];
  return s;
}

test('the flat pass paints the tribe with the fire out', () => {
  const { ctx, rects } = recordingCtx();
  sceneWithLonePerson().draw(ctx);
  const person = rects.some(([x, y]) => x >= 98 && x <= 102 && y >= 92 && y <= 100);
  assert.ok(person, 'villager pixels are drawn even though the campfire is unlit');
});

test('the 3D-sprite pass stands the tribe up with the fire out', () => {
  const billboards = [];
  const engine = { add_3d_light: () => {}, add_billboard: (...args) => billboards.push(args) };
  sceneWithLonePerson().draw3d(engine, 1 / 60);
  const person = billboards.some(([kind, x, y]) => kind === 0 && x === 100 && y === 100);
  assert.ok(person, 'the person billboard is still added when the campfire is unlit');
});
