// Planet temperaments for the AI god's PPO policy.
//
// The Stone Age PPO policy (rl/, src/ai/policy.js) picks one of five actions every decision:
// wait, strike the fire, hunt a mammoth, stay calm, or strike at random. Its exported weights
// are fixed, but a world can lean on them: the setup screen gives each new planet a
// temperament (friendliness, aggressiveness, violence), and traitWeights() turns that
// temperament into multipliers on the policy's action probabilities. AIGod wraps the shared
// policy with those multipliers (weightedPolicy in policy.js), so every world keeps the same
// trained brain but expresses a different personality.
//
// This file is pure: no DOM, no wasm, so the setup screen and the Node tests share it.

export const TRAITS = [
  {
    key: 'friendliness',
    label: 'Friendliness',
    short: 'mercy',
    hint: 'Calm and caution. Friendly worlds settle disputes and keep their fire tended.',
  },
  {
    key: 'aggressiveness',
    label: 'Aggressiveness',
    short: 'fervour',
    hint: 'Lightning and the hunt. Aggressive worlds strike first and grow fast.',
  },
  {
    key: 'violence',
    label: 'Violence',
    short: 'recklessness',
    hint: 'Wild strikes. Violent worlds take chances, and their luck often runs out.',
  },
];

export const TRAIT_KEYS = TRAITS.map((t) => t.key);

export const DEFAULT_TRAITS = Object.freeze({
  friendliness: 0.5,
  aggressiveness: 0.5,
  violence: 0.35,
});

// The PPO action order, mirrored from src/ai/observe.js ACTIONS. Kept here as data so the
// personality maths stays independent of the simulation.
export const ACTION_LABELS = ['Wait', 'Strike fire', 'Hunt', 'Stay calm', 'Wild strike'];
export const ACTION_NAMES = ['wait', 'strike_fire', 'strike_mammoth', 'calm', 'strike_random'];

export const BIOMES = ['temperate', 'jungle', 'desert', 'ice', 'alien'];
export const BIOME_LABELS = {
  temperate: 'Temperate',
  jungle: 'Jungle',
  desert: 'Desert',
  ice: 'Ice',
  alien: 'Alien',
};
export const BIOME_COLORS = {
  temperate: '#6cbf84',
  jungle: '#59d8a0',
  desert: '#d9a05b',
  ice: '#9fc7f0',
  alien: '#b58cff',
  forming: '#8a7f8f',
};

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const round3 = (v) => Math.round(v * 1000) / 1000;

export function clampTrait(value, fallback = 0.5) {
  const n = Number(value);
  return Number.isFinite(n) ? clamp01(n) : fallback;
}

export function clampTraits(traits = {}) {
  const out = {};
  for (const key of TRAIT_KEYS) out[key] = round3(clampTrait(traits[key], DEFAULT_TRAITS[key]));
  return out;
}

// Action multipliers. They are relative, not probabilities: policy.js normalises them after
// multiplying the trained policy's own probabilities, so the PPO's judgement still matters.
export function traitWeights(traits = {}) {
  const t = clampTraits(traits);
  const f = t.friendliness, a = t.aggressiveness, v = t.violence;
  return [
    0.55 + 0.85 * f + 0.45 * (1 - a), // wait: patient, friendly worlds wait their turn
    0.30 + 1.90 * a + 0.45 * v,       // strike_fire: relight the pit, or start a fight
    0.40 + 1.70 * a + 0.35 * v,       // strike_mammoth: the hunt, the clearest show of force
    0.35 + 2.00 * f + 0.35 * (1 - a), // calm: mercy and stability
    0.10 + 0.80 * a + 2.10 * v,       // strike_random: recklessness, not strategy
  ];
}

// Normalise multipliers into a preview of the god's likely choices (percentages).
export function actionPreview(weights = traitWeights(DEFAULT_TRAITS)) {
  const w = weights.map((v) => Math.max(0, Number(v) || 0));
  const total = w.reduce((a, b) => a + b, 0) || 1;
  return w.map((value, i) => ({
    name: ACTION_NAMES[i],
    label: ACTION_LABELS[i],
    weight: round3(value),
    pct: Math.round((value / total) * 100),
  }));
}

// A short name for the temperament, shown on the card and in the galaxy panel.
export function personalityLabel(traits = {}) {
  const t = clampTraits(traits);
  const { friendliness: f, aggressiveness: a, violence: v } = t;
  if (f >= 0.65 && a <= 0.4) return 'Peaceful';
  if (a >= 0.65 && v >= 0.55) return 'Warlike';
  if (v >= 0.65) return 'Volatile';
  if (a >= 0.65) return 'Bold';
  if (f >= 0.65) return 'Gentle';
  if (a <= 0.3 && v <= 0.35) return 'Cautious';
  return 'Balanced';
}

// The four worlds the player shapes before the home planet. Names and biomes are fixed so the
// player can tell them apart; the starting temperaments are archetypes rather than all-random.
export const SETUP_NAMES = ['Henhaven', 'Cluckturn', 'Roostia', 'Yolkos'];
const SETUP_ARCHETYPES = [
  { friendliness: 0.72, aggressiveness: 0.28, violence: 0.18 },
  { friendliness: 0.38, aggressiveness: 0.72, violence: 0.46 },
  { friendliness: 0.5, aggressiveness: 0.42, violence: 0.66 },
  { friendliness: 0.62, aggressiveness: 0.48, violence: 0.3 },
];
const SETUP_BIOMES = ['temperate', 'desert', 'jungle', 'ice'];

export const SETUP_COUNT = 4;

// Fresh setup state. `rng` only jitters the archetypes, so a run stays recognisable while two
// openings do not feel identical.
export function defaultSetupPlanets(count = SETUP_COUNT, rng = Math.random) {
  const n = Math.max(1, Math.min(count, SETUP_NAMES.length));
  return Array.from({ length: n }, (_, i) => {
    const base = SETUP_ARCHETYPES[i % SETUP_ARCHETYPES.length];
    const jitter = () => (rng() - 0.5) * 0.16;
    return {
      id: `setup-${i}`,
      name: SETUP_NAMES[i],
      biome: SETUP_BIOMES[i % SETUP_BIOMES.length],
      size: 9 + (i % 3),
      traits: clampTraits({
        friendliness: base.friendliness + jitter(),
        aggressiveness: base.aggressiveness + jitter(),
        violence: base.violence + jitter(),
      }),
    };
  });
}

// Re-roll temperaments (and biomes) without losing the four worlds' identities.
export function rerollSetupPlanets(planets, rng = Math.random) {
  return planets.map((p, i) => ({
    ...p,
    biome: BIOMES[Math.floor(rng() * BIOMES.length)] || p.biome,
    traits: clampTraits({
      friendliness: rng(),
      aggressiveness: rng(),
      violence: rng(),
    }),
  }));
}

// The options Galaxy.makePlanet needs. traitWeights is attached here so every consumer computes
// the personality the same way.
export function toPlanetOptions(planets = []) {
  return planets.map((p, i) => {
    const traits = clampTraits(p.traits);
    return {
      name: p.name || SETUP_NAMES[i] || `World ${i + 1}`,
      biome: BIOMES.includes(p.biome) ? p.biome : 'temperate',
      size: Number.isFinite(p.size) ? p.size : 9,
      traits,
      ppoWeights: traitWeights(traits),
      setup: true,
    };
  });
}
