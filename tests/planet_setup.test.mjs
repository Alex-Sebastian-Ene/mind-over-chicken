// Planet setup: the four worlds shaped before the home campaign, and the PPO weighting their
// temperament applies. Run: node --test tests/planet_setup.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  BIOMES, DEFAULT_TRAITS, SETUP_COUNT, TRAIT_KEYS,
  actionPreview, clampTraits, defaultSetupPlanets, personalityLabel, rerollSetupPlanets,
  toPlanetOptions, traitWeights,
} from '../src/ai/personality.js';
import { weightedPolicy } from '../src/ai/policy.js';
import { AIGod } from '../src/ai/autopilot.js';
import { PlanetSetup } from '../src/ui/planet-setup.js';
import { Galaxy } from '../src/cosmos/galaxy.js';

const basePolicy = (probs = [0.4, 0.3, 0.2, 0.05, 0.05]) => ({
  obsSize: 1,
  probs: () => probs.slice(),
  selfCheck: () => 0,
});

test('the setup creates four distinct worlds with valid temperaments', () => {
  const planets = defaultSetupPlanets(undefined, () => 0.5);
  assert.equal(planets.length, SETUP_COUNT);
  assert.equal(new Set(planets.map((p) => p.name)).size, SETUP_COUNT, 'names are unique');
  for (const p of planets) {
    assert.ok(BIOMES.includes(p.biome), p.biome);
    for (const key of TRAIT_KEYS) assert.ok(p.traits[key] >= 0 && p.traits[key] <= 1, `${p.name} ${key}`);
  }
});

test('clampTraits keeps every trait in 0..1 and fills missing values', () => {
  const t = clampTraits({ friendliness: 2, aggressiveness: -1 });
  assert.equal(t.friendliness, 1);
  assert.equal(t.aggressiveness, 0);
  assert.equal(t.violence, DEFAULT_TRAITS.violence);
});

test('temperament leans the PPO actions the way its labels promise', () => {
  const peace = traitWeights({ friendliness: 0.95, aggressiveness: 0.05, violence: 0.05 });
  const war = traitWeights({ friendliness: 0.05, aggressiveness: 0.95, violence: 0.05 });
  const chaos = traitWeights({ friendliness: 0.5, aggressiveness: 0.5, violence: 0.95 });
  assert.ok(peace[3] > war[3], 'friendliness favours calm');
  assert.ok(war[1] > peace[1], 'aggressiveness favours striking the fire');
  assert.ok(war[2] > peace[2], 'aggressiveness favours the hunt');
  assert.ok(chaos[4] > peace[4], 'violence favours wild strikes');
  assert.ok(traitWeights(DEFAULT_TRAITS).every((v) => v > 0), 'no action is fully suppressed by default');
});

test('the PPO action preview is a normalised percentage split', () => {
  const preview = actionPreview(traitWeights({ friendliness: 0.7, aggressiveness: 0.3, violence: 0.2 }));
  assert.equal(preview.length, 5);
  const sum = preview.reduce((n, a) => n + a.pct, 0);
  assert.ok(Math.abs(sum - 100) <= 2, `percentages sum to ${sum}`);
  assert.ok(preview.every((a) => Number.isInteger(a.pct) && a.pct >= 0));
});

test('personalityLabel names the archetypes', () => {
  assert.equal(personalityLabel({ friendliness: 0.9, aggressiveness: 0.1, violence: 0.1 }), 'Peaceful');
  assert.equal(personalityLabel({ friendliness: 0.2, aggressiveness: 0.9, violence: 0.8 }), 'Warlike');
  assert.equal(personalityLabel({ friendliness: 0.4, aggressiveness: 0.4, violence: 0.9 }), 'Volatile');
});

test('toPlanetOptions carries traits and the derived PPO weights into the galaxy', () => {
  const opts = toPlanetOptions(defaultSetupPlanets(undefined, () => 0.5));
  assert.equal(opts.length, SETUP_COUNT);
  for (const o of opts) {
    assert.ok(o.setup);
    assert.equal(o.ppoWeights.length, 5);
    assert.deepEqual(o.traits, clampTraits(o.traits));
    assert.ok(BIOMES.includes(o.biome));
  }
});

test('reroll keeps the four worlds but changes their temperaments', () => {
  const first = defaultSetupPlanets(undefined, () => 0.1);
  let n = 0;
  const second = rerollSetupPlanets(first, () => ((n++ * 0.37) % 1));
  assert.deepEqual(second.map((p) => p.name), first.map((p) => p.name));
  assert.notDeepEqual(second.map((p) => p.traits), first.map((p) => p.traits));
});

test('weightedPolicy normalises and respects multipliers', () => {
  const p = weightedPolicy(basePolicy(), [1, 1, 1, 1, 1]);
  assert.deepEqual(p.probs([0]), [0.4, 0.3, 0.2, 0.05, 0.05]);
  const strike = weightedPolicy(basePolicy(), [0, 3, 0, 0, 0]);
  assert.deepEqual(strike.probs([0]), [0, 1, 0, 0, 0]);
  assert.equal(strike.act([0], { greedy: true }), 1);
});

test('weightedPolicy falls back to uniform when every action is suppressed', () => {
  const p = weightedPolicy(basePolicy(), [0, 0, 0, 0, 0]);
  const probs = p.probs([0]);
  assert.ok(probs.every((v) => Math.abs(v - 0.2) < 1e-12));
});

test('weightedPolicy samples only from actions with weight', () => {
  const p = weightedPolicy(basePolicy(), [1, 0, 0, 0, 0]);
  for (const r of [0, 0.2, 0.99]) assert.equal(p.act([0], { greedy: false, rng: () => r }), 0);
});

test('AIGod applies a planet temperament to the shared policy', () => {
  const god = new AIGod({ policy: basePolicy([0.4, 0.3, 0.2, 0.05, 0.05]), weights: [0, 0, 0, 0, 1] });
  assert.deepEqual(god.policy.probs([0]), [0, 0, 0, 0, 1]);
  assert.equal(god.policy.act([0], { greedy: true }), 4);
});

test('setup planets join the galaxy with their traits and weights intact', () => {
  const gal = new Galaxy();
  const made = gal.addSetupPlanets(toPlanetOptions(defaultSetupPlanets(undefined, () => 0.5)));
  assert.equal(made.length, SETUP_COUNT);
  for (const p of made) {
    assert.equal(p.setup, true);
    assert.equal(p.forming, true);
    assert.ok(p.traits && TRAIT_KEYS.every((k) => p.traits[k] >= 0 && p.traits[k] <= 1));
    assert.equal(p.ppoWeights.length, 5);
    assert.ok(p.ppoWeights.every((w) => w > 0));
  }
});

test('the setup controller works without a DOM (pure state path)', () => {
  const setup = new PlanetSetup(null);
  assert.equal(setup.result().length, SETUP_COUNT);
  setup.randomise();
  assert.equal(setup.result().length, SETUP_COUNT);
  setup.close();
});
