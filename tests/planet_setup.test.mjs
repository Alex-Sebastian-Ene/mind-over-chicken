// Planet setup: the four worlds shaped before the home campaign, and the PPO weighting their
// temperament applies. Run: node --test tests/planet_setup.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  BIOMES, DEFAULT_TRAITS, SETUP_COUNT, TRAIT_KEYS,
  actionPreview, clampTraits, personalityLabel, toPlanetOptions, traitWeights,
  MoodReading, READ_SECONDS, SETUP_NAMES, moodChannel, moodTraits, planetFromMood,
} from '../src/ai/personality.js';
import { weightedPolicy } from '../src/ai/policy.js';
import { AIGod } from '../src/ai/autopilot.js';
import { PlanetSetup } from '../src/ui/planet-setup.js';
import { Galaxy } from '../src/cosmos/galaxy.js';
import { Cosmos } from '../src/cosmos/cosmos.js';

const basePolicy = (probs = [0.4, 0.3, 0.2, 0.05, 0.05]) => ({
  obsSize: 1,
  probs: () => probs.slice(),
  selfCheck: () => 0,
});

// Four readings: calm, focus, stress and a flurry of blinks.
const MOODS = [
  { calm: 0.85, focus: 0.2, stress: 0.05, blinks: 0 },
  { calm: 0.2, focus: 0.8, stress: 0.1, blinks: 1 },
  { calm: 0.1, focus: 0.3, stress: 0.9, blinks: 0 },
  { calm: 0.2, focus: 0.2, stress: 0.1, blinks: 7 },
];
const fourWorlds = () => MOODS.map((m, i) => planetFromMood(m, i));

test('the setup creates four distinct worlds with valid temperaments', () => {
  const planets = fourWorlds();
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
  const opts = toPlanetOptions(fourWorlds());
  assert.equal(opts.length, SETUP_COUNT);
  for (const o of opts) {
    assert.ok(o.setup);
    assert.equal(o.ppoWeights.length, 5);
    assert.deepEqual(o.traits, clampTraits(o.traits));
    assert.ok(BIOMES.includes(o.biome));
  }
});

test('each world is read from a feeling: its channel, biome and temperament follow the mood', () => {
  const [calm, focus, stress, blink] = fourWorlds();
  assert.deepEqual([calm.channel, focus.channel, stress.channel, blink.channel], ['calm', 'focus', 'stress', 'blink']);
  assert.deepEqual([calm.biome, focus.biome, stress.biome, blink.biome], ['temperate', 'alien', 'desert', 'ice']);
  assert.deepEqual([calm, focus, stress, blink].map((p) => p.name), SETUP_NAMES);
  assert.ok(calm.traits.friendliness > stress.traits.friendliness, 'calm makes a friendlier world');
  assert.ok(stress.traits.violence > calm.traits.violence, 'stress makes a more violent world');
  assert.ok(focus.traits.aggressiveness > calm.traits.aggressiveness, 'focus makes a bolder world');
  assert.equal(personalityLabel(calm.traits), 'Peaceful');
  assert.ok(calm.rating > 80 && calm.rating <= 99, `rating ${calm.rating}`);
  assert.deepEqual(moodChannel({}), { channel: 'calm', strength: 0 });
  assert.deepEqual(moodTraits({ calm: 2, stress: -1 }), moodTraits({ calm: 1, stress: 0 }), 'readings are clamped');
});

test('a reading averages the brain over its window and counts its blinks', () => {
  const r = new MoodReading(2);
  const brain = { calm: 0, focus: 0.5, stress: 0, stats: { blinks: 10 } };
  for (let i = 0; i < 10; i++) r.add(0.1, { ...brain, calm: 1 });
  for (let i = 0; i < 10; i++) r.add(0.1, { ...brain, stats: { blinks: 13 } });
  assert.ok(r.done);
  assert.ok(Math.abs(r.mood.calm - 0.5) < 1e-9 && Math.abs(r.mood.focus - 0.5) < 1e-9, JSON.stringify(r.mood));
  assert.equal(r.mood.blinks, 3, 'only the blinks inside the window count');
  r.add(5, { calm: 1 });
  assert.ok(Math.abs(r.mood.calm - 0.5) < 1e-9, 'a finished reading stays put');
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
  const made = gal.addSetupPlanets(toPlanetOptions(fourWorlds()));
  assert.equal(made.length, SETUP_COUNT);
  for (const p of made) {
    assert.equal(p.setup, true);
    assert.equal(p.forming, true);
    assert.ok(p.traits && TRAIT_KEYS.every((k) => p.traits[k] >= 0 && p.traits[k] <= 1));
    assert.equal(p.ppoWeights.length, 5);
    assert.ok(p.ppoWeights.every((w) => w > 0));
  }
});

test('the setup reads one card at a time and only offers Begin after the fourth', () => {
  let begun = null;
  const mood = { calm: 0.9, focus: 0.1, stress: 0.05, stats: { blinks: 0 } };
  const setup = new PlanetSetup(null, { getBrain: () => mood, onBegin: (p) => { begun = p; } });
  setup.open();
  assert.equal(setup.phase, 'reading');
  setup.begin();
  assert.equal(begun, null, 'no Begin before four worlds');
  for (let i = 0; i < 2000 && !setup.isDone; i++) {
    setup.update(0.05);
    assert.ok(setup.result().length <= 4);
  }
  assert.ok(setup.isDone);
  assert.equal(setup.result().length, 4);
  assert.ok(setup.result().every((p) => p.channel === 'calm'), 'every world read from the calm brain');
  setup.begin();
  assert.equal(begun.length, 4);
  assert.ok(begun.every((p) => p.setup && p.ppoWeights.length === 5 && p.channel === 'calm'));
  assert.ok(READ_SECONDS * 4 < 2000 * 0.05);
});

test('setup worlds replace the mind worlds: no second reading, one sim', () => {
  const G = { world: { width: 640, height: 360, seed: 1 } };
  // ensureGalaxy only touches this.galaxy, so it runs on a bare object (Cosmos itself needs a DOM).
  const ensure = (opts) => Cosmos.prototype.ensureGalaxy.call({ galaxy: null }, G, opts);
  const gal = ensure({ setupPlanets: toPlanetOptions(fourWorlds()) });
  assert.equal(gal.planets.filter((p) => p.mindSlot).length, 0, 'no mind worlds to remake');
  assert.equal(gal.planets.filter((p) => p.setup).length, 4);
  assert.equal(gal.planets.length, 5, 'home and the four read worlds');
  assert.equal(gal.mind.done, true, 'the first ten seconds are not read again');
  assert.equal(gal.wasm, null, 'one simulation: the JS sim runs every world');
  assert.deepEqual(gal.planets.filter((p) => p.setup).map((p) => p.channel), ['calm', 'focus', 'stress', 'blink']);

  const plain = ensure();
  assert.equal(plain.planets.filter((p) => p.mindSlot).length, 3, 'a run without setup keeps the mind worlds');
});
