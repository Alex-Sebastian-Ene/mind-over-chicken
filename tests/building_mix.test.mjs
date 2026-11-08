// Older building types overlap into later eras: an Industry town keeps some old huts, a
// Space-age town keeps some Industry houses. Run: node --test tests/building_mix.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { ERA_ORDER, ROOFS, buildingMix, holdoverShare, pickBuilding, roofFor } from '../src/world/building-mix.js';

test('every era mix sums to 1 and only uses its own or older building types', () => {
  for (const era of ERA_ORDER) {
    const mix = buildingMix(era);
    const total = mix.reduce((s, b) => s + b.share, 0);
    assert.ok(Math.abs(total - 1) < 1e-9, `${era}: shares sum to ${total}`);
    for (const b of mix) assert.ok(ERA_ORDER.indexOf(b.from) <= ERA_ORDER.indexOf(era), `${era} has a ${b.from} ${b.kind}`);
  }
});

test('Stone and Farming are all huts; Industry keeps old huts; Space keeps Industry houses and huts', () => {
  assert.equal(holdoverShare('stone'), 0);
  assert.deepEqual(buildingMix('farming').map((b) => b.kind), ['hut']);
  const industry = buildingMix('industry');
  assert.ok(industry.some((b) => b.kind === 'hut' && b.from === 'farming'));
  assert.ok(holdoverShare('industry') >= 0.15 && holdoverShare('industry') <= 0.35, `industry holdovers ${holdoverShare('industry')}`);
  const space = buildingMix('space');
  assert.ok(space.some((b) => b.kind === 'house' && b.from === 'industry'));
  assert.ok(space.some((b) => b.kind === 'hut'));
  // New buildings still dominate: growth, not a museum.
  for (const era of ['industry', 'space']) assert.ok(holdoverShare(era) < 0.5, `${era} holdovers ${holdoverShare(era)}`);
});

test('pickBuilding follows the shares across the roll and is deterministic', () => {
  const counts = {};
  const N = 1000;
  for (let i = 0; i < N; i++) {
    const b = pickBuilding('industry', (i + 0.5) / N);
    counts[`${b.from}:${b.kind}`] = (counts[`${b.from}:${b.kind}`] || 0) + 1;
  }
  assert.equal(counts['industry:house'], 750);
  assert.equal(counts['farming:hut'], 250);
  assert.deepEqual(pickBuilding('space', 0.99), pickBuilding('space', 0.99));
  assert.deepEqual(pickBuilding('space', 0.9999999), { kind: 'hut', from: 'farming' });
  assert.deepEqual(pickBuilding('nowhere', 0.5), { kind: 'hut', from: 'stone' });
});

test('an old house keeps the roof of its own era', () => {
  for (const r of [0, 0.3, 0.7, 0.999]) {
    assert.ok(ROOFS.industry.includes(roofFor('industry', r)));
    assert.ok(ROOFS.space.includes(roofFor('space', r)));
  }
  assert.ok(!ROOFS.space.some((c) => ROOFS.industry.includes(c)), 'space and industry roofs are told apart');
});
