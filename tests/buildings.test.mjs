// #206: biome/era building variants. The selection is pure and deterministic, every generated
// kind has a 2D drawer and a 3D atlas cell, and the generated worlds stay stable for a seed.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import '../rl/headless.mjs';   // canvas + engine wasm for World.paint
import { World } from '../src/world/terrain.js';
import * as sprites from '../src/render/sprites.js';
import {
  BIOME_CATEGORIES, KIND_POOLS, STRUCTURE_KINDS, STRUCTURE_SPRITES,
  biomeAt, chooseStructureKind, isLitBuilding, isResidential, sampleBiome, structureHash, structureVariant,
} from '../src/world/buildings.js';

const ERAS = ['stone', 'farming', 'industry', 'space'];

test('structureVariant is deterministic and stays inside the kind’s range', () => {
  const o = { biome: 'coast', seed: 1234, x: 42, y: 77, era: 'stone' };
  const a = structureVariant('longhouse', o);
  assert.equal(a, structureVariant('longhouse', { ...o }));
  assert.ok(Number.isInteger(a) && a >= 0 && a < STRUCTURE_KINDS.longhouse.variants);
  // The hash actually varies with the seed/position instead of always landing on 0.
  const seen = new Set();
  for (let i = 0; i < 64; i++) {
    seen.add(structureVariant('longhouse', { ...o, seed: i, x: i * 11, y: i * 7 }));
  }
  assert.ok(seen.size > 1, `variants ${[...seen]}`);
  assert.equal(structureVariant('not-a-kind', o), 0);
  // structureHash is stable and unsigned.
  assert.equal(structureHash(1, 2, 3, 4), structureHash(1, 2, 3, 4));
  assert.ok(structureHash(1, 2, 3, 4) >= 0);
});

test('biomeAt and sampleBiome map coast, hills, forest, snow and plains', () => {
  assert.equal(biomeAt({}), 'plains');
  assert.equal(biomeAt({ forest: true }), 'forest');
  assert.equal(biomeAt({ hills: true }), 'hills');
  assert.equal(biomeAt({ coast: true }), 'coast');
  assert.equal(biomeAt({ snow: true }), 'snow');
  // Priority: snow beats coast, coast beats hills, hills beat forest.
  assert.equal(biomeAt({ snow: true, coast: true, hills: true, forest: true }), 'snow');
  assert.equal(biomeAt({ coast: true, hills: true, forest: true }), 'coast');
  assert.equal(biomeAt({ hills: true, forest: true }), 'hills');

  // A patch of one terrain tile around the sample point, grass elsewhere.
  const patch = (tile) => (x, y) => (Math.abs(x - 20) <= 5 && Math.abs(y - 20) <= 5 ? tile : 3);
  assert.equal(sampleBiome(patch(0), 20, 20), 'coast');    // sea -> coast
  assert.equal(sampleBiome(patch(5), 20, 20), 'hills');    // T.HILL
  assert.equal(sampleBiome(patch(6), 20, 20), 'hills');    // T.MOUNTAIN
  assert.equal(sampleBiome(patch(4), 20, 20), 'forest');   // T.FOREST
  assert.equal(sampleBiome(patch(7), 20, 20), 'snow');     // T.SNOW
  assert.equal(sampleBiome(patch(3), 20, 20), 'plains');   // T.GRASS
  assert.equal(sampleBiome(() => 3, 20, 20, 'ice'), 'snow');
  assert.deepEqual([...BIOME_CATEGORIES].sort(), ['coast', 'forest', 'hills', 'plains', 'snow']);
});

test('every biome/era pool picks a kind with a 2D drawer and an atlas sprite', () => {
  const seen = new Set();
  for (const era of ERAS) {
    assert.ok(KIND_POOLS[era], `${era} has pools`);
    for (const biome of BIOME_CATEGORIES) {
      const kinds = new Set();
      for (let i = 0; i < 80; i++) {
        const kind = chooseStructureKind(era, biome, 0x5eed, i * 7 + 3, i * 13 + 5, 'hut');
        seen.add(kind);
        kinds.add(kind);
        assert.ok(STRUCTURE_KINDS[kind], `${era}/${biome} produced unknown kind ${kind}`);
        assert.equal(typeof sprites.STRUCTURE_DRAWERS[kind], 'function', `${kind} has a 2D drawer`);
        assert.equal(typeof sprites[STRUCTURE_KINDS[kind].draw], 'function', `${kind} draw fn exported`);
        assert.equal(STRUCTURE_SPRITES[kind], STRUCTURE_KINDS[kind].sprite);
      }
      assert.ok(kinds.size > 1, `${era}/${biome} is non-uniform (${[...kinds]})`);
    }
  }
  // Every kind in the table — core and new — is drawable.
  for (const [kind, def] of Object.entries(STRUCTURE_KINDS)) {
    assert.equal(typeof sprites.STRUCTURE_DRAWERS[kind], 'function', `${kind}`);
    assert.equal(typeof sprites[def.draw], 'function', `${kind} -> ${def.draw}`);
  }
  // Across all pools the set is genuinely diverse, not a handful of huts.
  assert.ok(seen.size >= 18, `distinct kinds ${seen.size}: ${[...seen].sort().join(', ')}`);
  // Scenes that looked for a house still find homes/lights in a varied settlement.
  for (const kind of ['hut', 'house', 'longhouse', 'pueblo', 'stilthut', 'snowcabin', 'terraced', 'dome']) {
    assert.ok(isResidential(kind), `${kind} is a home`);
    assert.ok(STRUCTURE_KINDS[kind], `${kind} exists`);
  }
  assert.ok(isLitBuilding('clocktower') && isLitBuilding('warehouse') && isLitBuilding('commsdish'));
  assert.ok(!isResidential('factory') && !isLitBuilding('pad'));
});

test('variant selection is non-uniform over position and biome', () => {
  const seen = new Set();
  for (const era of ERAS) {
    for (const biome of BIOME_CATEGORIES) {
      for (let i = 0; i < 30; i++) {
        const kind = chooseStructureKind(era, biome, 17, i * 5, i * 9, 'hut');
        seen.add(`${kind}:${structureVariant(kind, { biome, seed: 17, x: i * 5, y: i * 9, era })}`);
      }
    }
  }
  assert.ok(seen.size >= 20, `distinct kind+variant ${seen.size}`);
  // Same seed and spot -> same kind and variant, whatever the order of calls.
  const a = [chooseStructureKind('industry', 'hills', 9, 100, 200, 'house'), structureVariant('windmill', { biome: 'hills', seed: 9, x: 100, y: 200, era: 'industry' })];
  const b = [chooseStructureKind('industry', 'hills', 9, 100, 200, 'house'), structureVariant('windmill', { biome: 'hills', seed: 9, x: 100, y: 200, era: 'industry' })];
  assert.deepEqual(a, b);
});

test('generated campaign structures are deterministic and use known kinds', () => {
  const w = new World(0x206b1d, { width: 320, height: 180 });
  const snap = (era) => {
    w.paint(era);
    return w.structures.map((s) => [s.kind, s.x, s.y, s.variant]);
  };
  for (const era of ERAS) {
    const a = snap(era);
    const b = snap(era);
    assert.deepEqual(a, b, `${era} is deterministic for a seed`);
    assert.ok(a.length > 0, `${era} placed structures`);
    for (const [kind, , , variant] of a) {
      assert.ok(STRUCTURE_KINDS[kind], `${era}: unknown kind ${kind}`);
      const n = STRUCTURE_KINDS[kind].variants || 1;
      assert.ok(variant >= 0 && variant < n, `${era}: ${kind} variant ${variant} in 0..${n - 1}`);
    }
  }
  // Industry and Space have production sites (factory/exchange/hangar/pad) plus varied housing.
  w.paint('industry');
  const industry = new Set(w.structures.map((s) => s.kind));
  assert.ok(industry.has('factory') && industry.has('exchange'), 'core industry sites stay');
  w.paint('space');
  const space = new Set(w.structures.map((s) => s.kind));
  assert.ok(space.has('hangar') && space.has('pad'), 'core space sites stay');
});

test('new atlas sprites have unique cells inside the 256 px / 32 px grid', () => {
  const src = readFileSync(new URL('../crates/engine/src/sprites.rs', import.meta.url), 'utf8');
  const consts = new Map();
  for (const m of src.matchAll(/pub const (SPRITE_[A-Z0-9_]+): u32 = (\d+);/g)) consts.set(m[1], Number(m[2]));
  const byValue = new Map([...consts].map(([name, value]) => [value, name]));
  const cells = new Map();
  let n = 0;
  for (const [kind, def] of Object.entries(STRUCTURE_KINDS)) {
    if (def.sprite < 16) continue;   // the core sprites keep their existing cells
    n++;
    const name = byValue.get(def.sprite);
    assert.ok(name, `${kind}: no Rust constant for sprite ${def.sprite}`);
    const arm = src.match(new RegExp(`${name}\\s*=>\\s*\\((\\d+),\\s*(\\d+),`));
    assert.ok(arm, `${kind}: no atlas arm for ${name}`);
    const col = Number(arm[1]), row = Number(arm[2]);
    assert.ok(col >= 0 && col < 8 && row >= 0 && row < 8, `${kind}: cell ${col},${row} outside the 8×8 grid`);
    const key = `${col},${row}`;
    assert.ok(!cells.has(key), `${kind}: cell ${key} already used by ${cells.get(key)}`);
    cells.set(key, kind);
  }
  assert.equal(n, 16, '16 new building sprites');
  assert.equal(cells.size, 16);
  assert.equal(consts.get('SPRITE_LONGHOUSE'), 16);
  assert.equal(consts.get('SPRITE_CARGO_DEPOT'), 31);
});
