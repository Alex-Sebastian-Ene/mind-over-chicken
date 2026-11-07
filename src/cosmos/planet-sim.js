// Pure planet simulation: plain data in, plain data + events out. No DOM, no drawing.
// This is the reference for the Rust port (#24) — every rule a world follows lives here.
//
// A world hears ONE brain channel loudly (weight 1) and the others softly (OFF_CHANNEL):
//   calm  → growth and harvests      focus → research and era progress
//   stress → goat-market volatility  blink → storms (each blink)
// The world the player is zoomed into ("attended") hears EVERY channel at weight 1, and a blink
// strikes it directly. The player's brain always applies; an optional AI god (p.god) only chooses
// the civilisation's effort split and never touches the player's input.
// `rng` is any () => [0,1) function, so a port or a test can make runs deterministic.
//
// Progression is emergent (README §4: no research branches, nothing to unlock or pick). Nothing
// is chosen from a menu; a world grows when its stocks, people and mood allow it, and falls back
// when they don't (#68):
//   - Settlement tier (camp → village → town → city) rises when the next tier's entry conditions
//     (people, food, wood, stone, goats and stability) have all held for TIER_HOLD seconds.
//     Building spends the wood and stone. Each tier houses more people.
//   - Resources: food (gathered/farmed, plus milk from goats) is eaten every second; wood is
//     gathered and burned; stone is quarried; goats breed when people are fed. Stocks are capped
//     by storage, which grows with housing.
//   - Stability drifts toward a target that rises with the effort put into order and with calm,
//     and falls with hunger, overcrowding, internal conflict, the player's anger (camera mood)
//     and stress.
//   - Setbacks: famine (sustained hunger), plague (overcrowding), unrest (low stability burns
//     stores) and collapse (stability or people too low for the tier: drop one tier).
//   - Internal conflict: a world alone still fights itself. Tension from hunger, inequality,
//     instability, the player's anger and stress climbs the ladder feud → revolt → civil war (calm
//     and order bring it back down); a civil war ends with the rebels crushed, the world split
//     into two factions, or a tier collapsing.
//   - Eras stay fixed content levels; an era only begins once research is done AND the world has
//     the settlement to hold it, so eras and tiers pace each other without any branch to choose.
//   - Milestones are recorded once each; every world keeps a short history log.
//
// Progression thresholds live in three data tables shaped like data/ages.json (#70): TIERS,
// ERAS and CONFLICT, each entry with `enter: { all: { metric: value } }` read through
// metrics()/meets(). #69 can swap ERAS for the twenty ages in data/ages.json without touching
// the step functions.

import { stressFrom } from '../brain/brain.js';

// ---- progression tables (the one place thresholds live) ----

// Settlement tiers. `enter` must hold for SIM.TIER_HOLD seconds; `build` is spent on arrival.
export const TIERS = [
  { id: 'camp', name: 'Camp', housing: 40, enter: null, build: null },
  { id: 'village', name: 'Village', housing: 320, enter: { all: { population: 25, food: 30, wood: 25, stone: 0, wealth: 4, stability: 0.45 } }, build: { wood: 25, stone: 0 } },
  { id: 'town', name: 'Town', housing: 2600, enter: { all: { population: 220, food: 200, wood: 150, stone: 60, wealth: 30, stability: 0.5 } }, build: { wood: 150, stone: 60 } },
  { id: 'city', name: 'City', housing: 16000, enter: { all: { population: 1800, food: 1500, wood: 600, stone: 400, wealth: 150, stability: 0.55 } }, build: { wood: 600, stone: 400 } },
];

// Eras (fixed content levels). `cap` = most people the era's way of life supports; `enter` =
// research done and the settlement tier needed. #69 replaces this table with data/ages.json.
export const ERAS = [
  { key: 'stone', name: 'Stone Age', cap: 60, enter: null },
  { key: 'farming', name: 'Farming', cap: 400, enter: { all: { progress: 1, tier: 0 } } },
  { key: 'industry', name: 'Industry', cap: 3000, enter: { all: { progress: 1, tier: 1 } } },
  { key: 'space', name: 'Space Age', cap: 12000, enter: { all: { progress: 1, tier: 2 } } },
];

// Internal conflict ladder (data/ages.json → conflict.inside_one_civ). A level is entered when
// tension stays at or above `enter` for CONFLICT_HOLD seconds, and left when tension falls
// CONFLICT_CALM below it. `stability` is subtracted from the stability target, `output`
// multiplies food/wood/stone production, `steal` is the share of goats lost per second,
// `deaths` the share of people per second, `raid` the share of stores lost on entry.
export const CONFLICT = [
  { id: 'peace', name: 'Peace', enter: 0, stability: 0, output: 1, steal: 0, deaths: 0, raid: 0 },
  { id: 'feud', name: 'Clan feud', enter: 0.3, stability: 0.06, output: 0.95, steal: 0.01, deaths: 0, raid: 0 },
  { id: 'revolt', name: 'Revolt', enter: 0.6, stability: 0.15, output: 0.8, steal: 0.02, deaths: 0.002, raid: 0.15 },
  { id: 'civil_war', name: 'Civil war', enter: 0.9, stability: 0.3, output: 0.6, steal: 0.03, deaths: 0.006, raid: 0.2 },
];

export const SIM = {
  ERA_COUNT: ERAS.length,       // stone, farming, industry, space
  ERA_SECONDS: 15,              // seconds per era at research rate 1
  CAP: ERAS.map((e) => e.cap),  // population cap per era (also capped by housing, see popCap)
  MIN_POP: 3,
  OFF_CHANNEL: 0.25,            // how much a world hears the channels that aren't its own
  // growth (per second, logistic): (BASE + CALM*w_calm*calm - STRESS*w_stress*stress) * (0.5 + alloc.growth) * (1 - hunger)
  GROWTH_BASE: 0.03, GROWTH_CALM: 0.2, GROWTH_STRESS: 0.1,
  // research rate: clamp(BASE + FOCUS*w_focus*focus + CALM*calm - STRESS*w_stress*stress, MIN, MAX) * (0.5 + alloc.research) * (TIER_RESEARCH_BASE + TIER_RESEARCH*tier)
  RESEARCH_BASE: 0.45, RESEARCH_FOCUS: 1.5, RESEARCH_CALM: 0.2, RESEARCH_STRESS: 0.4, RESEARCH_MIN: 0.1, RESEARCH_MAX: 2.4,
  TIER_RESEARCH_BASE: 0.85, TIER_RESEARCH: 0.15,
  // goat market (log-price): dlnM = REVERT*ln(100/M)*dt + (SIGMA_BASE + SIGMA_STRESS*w_stress*stress)*sqrt(dt)*N(0,1)
  MARKET_START: 100, MARKET_REVERT: 0.15, MARKET_SIGMA_BASE: 0.02, MARKET_SIGMA_STRESS: 0.22,
  MARKET_PEAK_DECAY: 0.05,      // peak relaxes toward price at this rate per second
  CRASH_RATIO: 0.78, CRASH_RESET: 0.92, BOOM_LEVEL: 128, BOOM_RESET: 115,
  CRASH_POP: 0.92, CRASH_PROGRESS: 0.2, BOOM_PROGRESS: 0.15,
  CRASH_GOATS: 0.8, BOOM_GOATS: 1.1,   // a crash culls the herd, a boom grows it
  // channel events, checked every EVENT_MIN..EVENT_MAX seconds
  EVENT_MIN: 4, EVENT_MAX: 7,
  CALM_HIGH: 0.55, CALM_LOW: 0.12, CALM_LOW_CHANCE: 0.5, HARVEST_GAIN: 1.15, FAMINE_LOSS: 0.15,
  HARVEST_FOOD: 0.25,           // a bumper harvest adds this share of the food store (min 5); a poor one spoils it
  FOCUS_HIGH: 0.55, FOCUS_LOW: 0.1, FOCUS_LOW_CHANCE: 0.35, BREAKTHROUGH: 0.2, DARK_AGE: 0.15,
  FLAVOUR_CHANCE: 0.25,
  // blink storms
  STORM_CHANCE_OFF: 0.12, STORM_POP_OWN: 0.97, STORM_POP_OFF: 0.99, STORM_PROGRESS_OWN: 0.05, STORM_PROGRESS_OFF: 0.01,
  STORM_TENSION: 0.05,          // lightning on a world at war breaks a charge
  // space travel
  LAUNCH_EVERY: 7,              // launch charge needed; charge += dt * (0.6 + focus)
  LESSER_TARGET_CHANCE: 0.35,   // chance to reinforce a less advanced world when nothing is barren
  SHIP_MIN: 5, SHIP_MAX: 7, SETTLE_POP: 12, REINFORCE_POP: 8, REINFORCE_PROGRESS: 0.1,
  // mind snapshot
  MIND_SECONDS: 10,

  // ---- settlements, resources and stability (#68) ----
  TIER_HOLD: 3,                 // seconds every entry condition of the next tier must hold
  OVERCROWD_MAX: 1.3,           // people can pack into 130% of housing; growth stops there
  START_STOCK: { food: 20, wood: 5, stone: 0, goats: 4 },
  // food per second = pop * FOOD_YIELD[era] * biome.food * (0.6 + 0.8*alloc.growth) * (1 + HARVEST_CALM*w_calm*calm) * output / max(1, crowding)^2
  //                 + goats * GOAT_FOOD;   eaten: pop * FOOD_EAT
  FOOD_EAT: 0.05, FOOD_YIELD: [0.065, 0.085, 0.1, 0.11], HARVEST_CALM: 0.3, GOAT_FOOD: 0.04,
  DAY_SECONDS: 10,              // one day of the fast-forwarded world (for the food_days metric)
  // goats: logistic toward pop * GOAT_PER_HEAD at GOAT_BREED/s while people are fed;
  // hungry people eat them (GOAT_EATEN of the herd per second at full hunger)
  GOAT_BREED: 0.06, GOAT_PER_HEAD: 0.5, GOAT_EATEN: 0.15, GOAT_MEAL: 2,
  // wood/s = (pop * WOOD_RATE * biome.wood * (0.5 + alloc.growth)) * output - pop * WOOD_BURN
  WOOD_RATE: 0.03, WOOD_BURN: 0.006,
  // stone/s = pop * STONE_RATE * biome.stone * (0.3 + alloc.research) * output
  STONE_RATE: 0.012,
  STORE_PER_HOUSING: 4,         // each stock is capped at housing * this
  HUNGER_RATE: 2,               // hunger eases toward the share of food demand not met at this rate
  STARVE: 0.08,                 // people lost per second per unit of hunger above STARVE_FROM
  STARVE_FROM: 0.2,
  // stability target = BASE + ALLOC*alloc.stability + CALM*w_calm*calm - HUNGER*hunger
  //   - CROWD*max(0, crowding - 1) - ANGER*w_mood*anger - STRESS*w_stress*stress
  //   - CONFLICT[level].stability - SPLIT (while two factions share the world)
  // stability moves toward it at STAB_RATE/s
  STAB_START: 0.7, STAB_BASE: 0.62, STAB_ALLOC: 0.35, STAB_CALM: 0.15, STAB_HUNGER: 0.7, STAB_CROWD: 0.8,
  STAB_ANGER: 0.35, STAB_STRESS: 0.2, STAB_RATE: 0.35, STAB_SPLIT: 0.08,
  // setbacks (at most one every SETBACK_GAP seconds)
  SETBACK_GAP: 6,
  FAMINE_HUNGER: 0.45, FAMINE_SECONDS: 3, FAMINE_POP: 0.2, FAMINE_GOATS: 0.7, FAMINE_STABILITY: 0.12,
  PLAGUE_CROWD: 1.08, PLAGUE_CHANCE: 0.4, PLAGUE_POP: 0.22, PLAGUE_STABILITY: 0.1,   // rate/s = PLAGUE_CHANCE * (crowding - 1)
  UNREST: 0.3, UNREST_CHANCE: 0.25, UNREST_LOSS: 0.35, UNREST_PROGRESS: 0.1, UNREST_TENSION: 0.1, // rate/s while stability < UNREST
  // collapse (data/ages.json → collapse): stability below COLLAPSE, or people below ABANDON of
  // the tier's entry population, for COLLAPSE_SECONDS (4 s here: the galaxy runs fast-forward;
  // the agent sim in #69 uses the file's 30 s) drops one tier.
  COLLAPSE: 0.15, COLLAPSE_SECONDS: 4, ABANDON: 0.6, COLLAPSE_STABILITY: 0.25,
  RECOVERED: 0.6,               // stability that counts as having weathered a setback
  HISTORY: 8,                   // history entries kept per world
  // inequality (0..1, wealth gap): drifts at INEQ_RATE/s toward
  //   INEQ_BASE + INEQ_TIER*tier + INEQ_BOOM*max(0, market/100 - 1) - INEQ_ORDER*alloc.stability
  INEQ_START: 0.15, INEQ_BASE: 0.12, INEQ_TIER: 0.12, INEQ_BOOM: 0.6, INEQ_ORDER: 0.2, INEQ_RATE: 0.1,
  // tension (0..TENSION_MAX) changes by (pressure - relief) per second:
  //   pressure = HUNGER*hunger + INEQ*max(0, inequality - INEQ_FROM) + ANGER*w_mood*anger
  //            + STRESS*w_stress*stress + UNSTABLE*max(0, UNSTABLE_FROM - stability) + SPLIT (two factions)
  //   relief   = RELIEF + CALM*w_calm*calm + ORDER*alloc.stability
  T_HUNGER: 0.5, T_INEQ: 0.8, T_INEQ_FROM: 0.35, T_ANGER: 0.45, T_STRESS: 0.15, T_UNSTABLE: 0.4, T_UNSTABLE_FROM: 0.45,
  T_SPLIT: 0.04, T_RELIEF: 0.06, T_CALM: 0.12, T_ORDER: 0.08, TENSION_MAX: 1.2,
  CONFLICT_HOLD: 3,             // seconds at a level's tension before the conflict escalates
  CONFLICT_CALM: 0.15,          // hysteresis: de-escalate this far below a level's entry
  CIVIL_WAR_SECONDS: 8,         // a civil war is decided after this long
  // how a civil war ends: loyalists = stability + LOYAL_ORDER*alloc.stability + LOYAL_EQUAL*(1 - inequality)
  //                       rebels    = inequality + REBEL_TENSION*tension + REBEL_LUCK*rng()
  LOYAL_ORDER: 0.5, LOYAL_EQUAL: 0.3, REBEL_TENSION: 0.5, REBEL_LUCK: 0.4, CRUSH_MARGIN: 0.2,
  CRUSH_POP: 0.92, CRUSH_INEQ: 0.1, AFTER_WAR_TENSION: 0.4,   // grudges keep a feud going
  SPLIT_MIN: 0.3, SPLIT_MAX: 0.5, // share of the people who follow the rebels when the world splits
  REUNITE_STABILITY: 0.65, REUNITE_TENSION: 0.15, REUNITE_SECONDS: 10,
  PEACE_TENSION: 0.4,           // the peace cast lowers tension this much and steps the ladder down
  // the capital tribe (the AI god's Stone Age on the surface) reports back
  TRIBE_WIN_FOOD: 20, TRIBE_WIN_STABILITY: 0.1, TRIBE_WIN_PROGRESS: 0.1, TRIBE_LOSS_POP: 0.9, TRIBE_LOSS_STABILITY: 0.15,
};

export const RESOURCES = ['food', 'wood', 'stone', 'goats'];

// What each biome's land gives (multipliers on the rates above).
export const BIOME_YIELD = {
  temperate: { food: 1, wood: 1, stone: 1 },
  desert: { food: 0.75, wood: 0.5, stone: 1.3 },
  ice: { food: 0.7, wood: 0.8, stone: 1.1 },
  jungle: { food: 1.25, wood: 1.4, stone: 0.7 },
  alien: { food: 1, wood: 0.9, stone: 1.2 },
};

export const CHANNEL_KEYS = ['calm', 'focus', 'stress', 'blink'];

// How strongly each channel showed up in the first MIND_SECONDS of play (comparable scales).
export const CHANNEL_SCORE = {
  calm: (m) => m.calm / 0.35,
  focus: (m) => m.focus / 0.3,
  stress: (m) => (m.stress ?? 0) * 2.5,
  blink: (m) => m.blinks / 3,
};

// Effort split a world's civilisation uses when no AI god is steering it.
export const TEMPERAMENT = {
  calm: { growth: 0.55, research: 0.25, stability: 0.2 },
  focus: { growth: 0.2, research: 0.6, stability: 0.2 },
  stress: { growth: 0.4, research: 0.5, stability: 0.1 },
  blink: { growth: 0.3, research: 0.4, stability: 0.3 },
  mixed: { growth: 0.4, research: 0.4, stability: 0.2 },
};

// The PPO AI god (src/ai/autopilot.js) plays the capital tribe's Stone Age. Its latest move is
// read as the whole world's effort split through this table (the p.god seam), eased over time.
export const ACTION_SPLIT = {
  wait: { growth: 0.35, research: 0.45, stability: 0.2 },
  strike_fire: { growth: 0.45, research: 0.25, stability: 0.3 },
  strike_mammoth: { growth: 0.65, research: 0.15, stability: 0.2 },
  calm: { growth: 0.2, research: 0.2, stability: 0.6 },
  strike_random: { growth: 0.3, research: 0.45, stability: 0.25 },
};
export function easeSplit(split, action, dt, rate = 0.5) {
  const to = ACTION_SPLIT[action];
  if (!to) return split;
  const k = 1 - Math.exp(-rate * dt);
  for (const key of ['growth', 'research', 'stability']) split[key] += (to[key] - split[key]) * k;
  return split;
}

const FLAVOUR = [
  ['invented the spear', 'worships a rock', 'met a mammoth'],
  ['domesticated goats', 'invented cheese', 'elected a chicken'],
  ['opened a goat exchange', 'invented the strike', 'built a mill'],
  ['launched a satellite', 'found water on a moon', 'built a goat in orbit'],
];

// Milestones: recorded once per world, in the order reached.
export const MILESTONES = {
  village: 'First village', town: 'First town', city: 'First city',
  farming: 'Reached Farming', industry: 'Reached Industry', space: 'Reached the Space Age',
  pop100: '100 people', pop1000: '1,000 people', pop10000: '10,000 people',
  goats100: 'A hundred goats', rebuilt: 'Rebuilt after a collapse', weathered: 'Weathered hard times',
  launch: 'First ship launched', tribe: 'The tribe kept its fire', reunited: 'Reunited after a civil war',
};

const clamp = (v, lo = 0, hi = 1) => (v < lo ? lo : v > hi ? hi : v);
const pickOf = (arr, rng) => arr[Math.floor(rng() * arr.length)];
function gaussOf(rng) {
  let u = 0, v = 0;
  while (u === 0) u = rng();
  while (v === 0) v = rng();
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}

// Plain planet state. Presentation fields (labels, textures) live elsewhere.
export function newPlanetState({ channel = 'mixed', pop = 0, eraIndex = 0, biome = 'temperate' } = {}, rng = Math.random) {
  return {
    channel, pop, eraIndex, progress: 0, biome,
    market: SIM.MARKET_START, marketPeak: SIM.MARKET_START, crashed: false, boomed: false,
    eventT: SIM.EVENT_MIN + rng() * (SIM.EVENT_MAX - SIM.EVENT_MIN),
    launchT: 0, storms: 0,
    god: null,          // AI-god seam: { act(observation) → { growth, research, stability } }
    ...freshSettlement(),
  };
}

// Settlement, resource, stability and conflict state for a newly peopled world (#68).
export function freshSettlement() {
  return {
    tier: 0, ...SIM.START_STOCK, stability: SIM.STAB_START,
    hunger: 0, crowding: 0, tierHold: 0, failT: 0, famineT: 0, setbackT: 0,
    collapsed: false, hardTimes: false, age: 0,
    inequality: SIM.INEQ_START, tension: 0, conflict: 0, conflictHold: 0, warT: 0, factions: null, reuniteT: 0,
    milestones: [], history: [],
  };
}

// Brain readings every world sees this frame. `stress` is brain.stress (src/brain/brain.js):
// EEG agitation or face anger, about 0.15 at rest, 0..1. Without it, it is worked out from calm
// and anger with stressFrom. `anger` is the camera mood (0..1).
export function brainEffects({ calm, focus, stress, blinksLast10s, anger = 0 }) {
  return {
    calm, focus,
    stress: clamp(stress ?? stressFrom(calm, anger)),
    blinkRate: blinksLast10s / 10,
    anger: clamp(anger || 0),
  };
}

// Housing of the current tier, and the population the world can hold right now.
export function housing(p) { return TIERS[p.tier ?? 0].housing; }
export function popCap(p) { return Math.min(SIM.CAP[p.eraIndex], housing(p) * SIM.OVERCROWD_MAX); }
export function storage(p) { return housing(p) * SIM.STORE_PER_HOUSING; }
export const tierName = (p) => TIERS[p.tier ?? 0].name;
export const conflictName = (p) => CONFLICT[p.conflict ?? 0].name;

// The world's metrics, named as in data/ages.json where the file has them (population,
// food_days, stability, wealth, inequality) plus this sim's own (food, wood, stone, tier,
// progress, tension).
export function metrics(p) {
  const daily = Math.max(1e-6, p.pop * SIM.FOOD_EAT * SIM.DAY_SECONDS);
  return {
    population: p.pop, food: p.food, wood: p.wood, stone: p.stone, wealth: p.goats,
    food_days: p.food / daily, stability: p.stability, inequality: p.inequality ?? 0,
    tier: p.tier ?? 0, progress: p.progress, tension: p.tension ?? 0,
  };
}

// Does a metrics object satisfy an `enter` block ({ all: {...} } and/or { any: [{...}] })?
export function meets(m, enter) {
  if (!enter) return true;
  const ok = (cond) => Object.entries(cond).every(([k, v]) => (typeof v === 'boolean' ? !!m[k] === v : (m[k] ?? 0) >= v));
  if (enter.all && !ok(enter.all)) return false;
  if (enter.any && !enter.any.some(ok)) return false;
  return true;
}

export function observe(p, fx) {
  return {
    era: p.eraIndex, pop: p.pop, cap: popCap(p), progress: p.progress, market: p.market, storms: p.storms, channel: p.channel, brain: fx,
    tier: p.tier, food: p.food, wood: p.wood, stone: p.stone, goats: p.goats, stability: p.stability, hunger: p.hunger, crowding: p.crowding,
    inequality: p.inequality, tension: p.tension, conflict: p.conflict,
  };
}

function normalise(a) {
  const g = Math.max(0, a.growth ?? 0), r = Math.max(0, a.research ?? 0), s = Math.max(0, a.stability ?? 0);
  const t = g + r + s || 1;
  return { growth: g / t, research: r / t, stability: s / t };
}

// Record a line in the world's history (newest first).
export function logHistory(p, text, kind = 'event') {
  p.history ||= [];
  p.history.unshift({ t: Math.round(p.age || 0), text, kind });
  if (p.history.length > SIM.HISTORY) p.history.length = SIM.HISTORY;
}

// Mark a milestone once. Returns the event to emit, or null if it was already reached.
export function reachMilestone(p, id) {
  if (!id) return null;
  p.milestones ||= [];
  if (p.milestones.includes(id)) return null;
  p.milestones.push(id);
  const text = MILESTONES[id] || id;
  logHistory(p, text, 'milestone');
  return { kind: 'milestone', id, text };
}

// Does the world meet every entry condition of the next tier right now?
export function tierReady(p) {
  const next = TIERS[(p.tier ?? 0) + 1];
  return !!next && meets(metrics(p), next.enter);
}
// Can the next era begin (research done and the settlement to hold it)?
export function eraReady(p) {
  const next = ERAS[p.eraIndex + 1];
  return !!next && meets(metrics(p), next.enter);
}

// The stability the world drifts toward (exported so tests and the panel can explain it).
export function stabilityTarget(p, fx, alloc, w) {
  return SIM.STAB_BASE + SIM.STAB_ALLOC * alloc.stability + SIM.STAB_CALM * w('calm') * fx.calm
    - SIM.STAB_HUNGER * p.hunger - SIM.STAB_CROWD * Math.max(0, p.crowding - 1)
    - SIM.STAB_ANGER * w('mood') * (fx.anger || 0) - SIM.STAB_STRESS * w('stress') * fx.stress
    - CONFLICT[p.conflict ?? 0].stability - (p.factions ? SIM.STAB_SPLIT : 0);
}

// How fast tension builds (positive) or eases (negative) right now.
export function tensionRate(p, fx, alloc, w) {
  const pressure = SIM.T_HUNGER * p.hunger + SIM.T_INEQ * Math.max(0, p.inequality - SIM.T_INEQ_FROM)
    + SIM.T_ANGER * w('mood') * (fx.anger || 0) + SIM.T_STRESS * w('stress') * fx.stress
    + SIM.T_UNSTABLE * Math.max(0, SIM.T_UNSTABLE_FROM - p.stability) + (p.factions ? SIM.T_SPLIT : 0);
  const relief = SIM.T_RELIEF + SIM.T_CALM * w('calm') * fx.calm + SIM.T_ORDER * alloc.stability;
  return pressure - relief;
}

// Resources, hunger, stability, tiers and setbacks for one step. Mutates p, pushes events.
// Order: produce → eat → stability → setbacks → tier up.
export function stepSettlement(p, dt, fx, alloc, w, rng, events) {
  const by = BIOME_YIELD[p.biome] || BIOME_YIELD.temperate;
  const war = CONFLICT[p.conflict ?? 0];
  const tierBefore = p.tier;
  p.crowding = p.pop / housing(p);
  const store = storage(p);

  // ---- production (overcrowding and fighting both cut it) ----
  const crowdPenalty = 1 / Math.max(1, p.crowding) ** 2;
  const farmed = p.pop * SIM.FOOD_YIELD[p.eraIndex] * by.food * (0.6 + 0.8 * alloc.growth) * (1 + SIM.HARVEST_CALM * w('calm') * fx.calm) * crowdPenalty * war.output;
  p.food += (farmed + p.goats * SIM.GOAT_FOOD) * dt;
  p.wood = clamp(p.wood + (p.pop * SIM.WOOD_RATE * by.wood * (0.5 + alloc.growth) * war.output - p.pop * SIM.WOOD_BURN) * dt, 0, store);
  p.stone = clamp(p.stone + p.pop * SIM.STONE_RATE * by.stone * (0.3 + alloc.research) * war.output * dt, 0, store);

  // ---- eating: the share of demand not met becomes hunger ----
  const demand = p.pop * SIM.FOOD_EAT * dt;
  let shortfall = 0;
  if (p.food >= demand) p.food -= demand;
  else {
    shortfall = demand > 0 ? 1 - p.food / demand : 0;
    p.food = 0;
    // Hungry people eat the goats first.
    const eaten = Math.min(p.goats, p.goats * SIM.GOAT_EATEN * shortfall * dt);
    p.goats -= eaten;
    p.food += eaten * SIM.GOAT_MEAL;
  }
  p.food = Math.min(p.food, store);
  p.hunger = clamp(p.hunger + (shortfall - p.hunger) * (1 - Math.exp(-SIM.HUNGER_RATE * dt)));
  if (p.hunger > SIM.STARVE_FROM) p.pop = Math.max(SIM.MIN_POP, p.pop - p.pop * SIM.STARVE * (p.hunger - SIM.STARVE_FROM) * dt);

  // ---- goats breed while people are fed ----
  const goatCap = Math.max(2, p.pop * SIM.GOAT_PER_HEAD);
  if (p.hunger < 0.2) p.goats = Math.min(store, p.goats + p.goats * SIM.GOAT_BREED * (1 - p.goats / goatCap) * dt);
  p.goats = Math.max(0, p.goats);

  // ---- stability ----
  const target = clamp(stabilityTarget(p, fx, alloc, w));
  p.stability = clamp(p.stability + (target - p.stability) * (1 - Math.exp(-SIM.STAB_RATE * dt)));

  // ---- setbacks ----
  p.setbackT = Math.max(0, p.setbackT - dt);
  p.famineT = p.hunger > SIM.FAMINE_HUNGER ? p.famineT + dt : 0;
  const tier = TIERS[p.tier];
  const failing = p.tier > 0 && (p.stability < SIM.COLLAPSE || p.pop < tier.enter.all.population * SIM.ABANDON);
  p.failT = failing ? p.failT + dt : 0;
  if (p.setbackT <= 0) {
    let hit = null;
    if (p.failT >= SIM.COLLAPSE_SECONDS) {
      hit = collapseTier(p, 'collapse');
    } else if (p.famineT >= SIM.FAMINE_SECONDS) {
      p.pop = Math.max(SIM.MIN_POP, p.pop * (1 - SIM.FAMINE_POP));
      p.goats *= SIM.FAMINE_GOATS;
      p.stability = clamp(p.stability - SIM.FAMINE_STABILITY);
      p.famineT = 0;
      hit = { kind: 'setback', id: 'famine', text: 'famine', detail: 'the stores ran dry' };
    } else if (p.crowding > SIM.PLAGUE_CROWD && rng() < SIM.PLAGUE_CHANCE * (p.crowding - 1) * dt) {
      p.pop = Math.max(SIM.MIN_POP, p.pop * (1 - SIM.PLAGUE_POP));
      p.stability = clamp(p.stability - SIM.PLAGUE_STABILITY);
      hit = { kind: 'setback', id: 'plague', text: 'plague', detail: 'too many people, too close together' };
    } else if (p.stability < SIM.UNREST && rng() < SIM.UNREST_CHANCE * dt) {
      p.wood *= 1 - SIM.UNREST_LOSS; p.stone *= 1 - SIM.UNREST_LOSS; p.food *= 1 - SIM.UNREST_LOSS;
      p.progress = Math.max(0, p.progress - SIM.UNREST_PROGRESS);
      p.tension = Math.min(SIM.TENSION_MAX, p.tension + SIM.UNREST_TENSION);   // riots feed the feud
      hit = { kind: 'setback', id: 'unrest', text: 'unrest', detail: 'riots burned the stores' };
    }
    if (hit) setback(p, hit, events);
  }
  if (p.hardTimes && p.setbackT <= 0 && p.stability >= SIM.RECOVERED && p.hunger < 0.1) {
    p.hardTimes = false;
    const m = reachMilestone(p, 'weathered'); if (m) events.push(m);
  }

  // ---- growth to the next tier (only if nothing just knocked it down) ----
  if (p.tier === tierBefore && tierReady(p)) {
    p.tierHold += dt;
    if (p.tierHold >= SIM.TIER_HOLD) {
      const next = TIERS[p.tier + 1];
      p.wood -= next.build.wood; p.stone -= next.build.stone;
      p.tier++; p.tierHold = 0;
      logHistory(p, `Grew into a ${next.name.toLowerCase()}`, 'tier');
      events.push({ kind: 'tier', id: next.id, text: next.name.toLowerCase() });
      const m = reachMilestone(p, next.id); if (m) events.push(m);
      if (p.collapsed) { p.collapsed = false; const r = reachMilestone(p, 'rebuilt'); if (r) events.push(r); }
    }
  } else p.tierHold = 0;

  for (const [n, id] of [[100, 'pop100'], [1000, 'pop1000'], [10000, 'pop10000']]) if (p.pop >= n) { const m = reachMilestone(p, id); if (m) events.push(m); }
  if (p.goats >= 100) { const m = reachMilestone(p, 'goats100'); if (m) events.push(m); }
}

function setback(p, hit, events) {
  p.setbackT = SIM.SETBACK_GAP;
  p.hardTimes = true;
  logHistory(p, `${hit.text[0].toUpperCase()}${hit.text.slice(1)}: ${hit.detail}`, 'setback');
  events.push(hit);
}

// Drop one settlement tier (a Dark Age for this world). Returns the setback event.
function collapseTier(p, id) {
  const from = TIERS[p.tier].name;
  p.tier = Math.max(0, p.tier - 1);
  p.failT = 0; p.tierHold = 0; p.collapsed = true;
  p.stability = clamp(p.stability + SIM.COLLAPSE_STABILITY);   // the survivors regroup
  return { kind: 'setback', id, text: id === 'collapse' ? 'collapse' : 'civil war', detail: `${from} fell back to a ${TIERS[p.tier].name.toLowerCase()}` };
}

// Internal conflict for one step: inequality, tension, the feud → revolt → civil war ladder,
// how a civil war ends, and reunion of a split world. Mutates p, pushes events.
export function stepConflict(p, dt, fx, alloc, w, rng, events) {
  // Inequality: bigger settlements and goat booms widen the gap, effort put into order narrows it.
  const ineqTarget = clamp(SIM.INEQ_BASE + SIM.INEQ_TIER * p.tier + SIM.INEQ_BOOM * Math.max(0, p.market / SIM.MARKET_START - 1) - SIM.INEQ_ORDER * alloc.stability);
  p.inequality = clamp(p.inequality + (ineqTarget - p.inequality) * (1 - Math.exp(-SIM.INEQ_RATE * dt)));
  p.tension = clamp(p.tension + tensionRate(p, fx, alloc, w) * dt, 0, SIM.TENSION_MAX);

  const level = p.conflict;
  const war = CONFLICT[level];
  // Ongoing harm at this level.
  if (war.steal) p.goats = Math.max(0, p.goats * (1 - war.steal * dt));
  if (war.deaths) p.pop = Math.max(SIM.MIN_POP, p.pop * (1 - war.deaths * dt));

  // Climb or step down the ladder.
  const up = CONFLICT[level + 1];
  if (up && p.tension >= up.enter) {
    p.conflictHold += dt;
    if (p.conflictHold >= SIM.CONFLICT_HOLD) {
      p.conflict++; p.conflictHold = 0; p.warT = 0;
      if (up.raid) { p.food *= 1 - up.raid; p.wood *= 1 - up.raid; p.stone *= 1 - up.raid; }
      const cause = conflictCause(p, fx, w);
      logHistory(p, `${up.name}: ${cause}`, 'conflict');
      events.push({ kind: 'conflict', id: up.id, text: up.name.toLowerCase(), detail: cause });
    }
  } else p.conflictHold = 0;
  if (level > 0 && p.tension < war.enter - SIM.CONFLICT_CALM) {
    p.conflict--; p.conflictHold = 0; p.warT = 0;
    const text = p.conflict === 0 ? 'peace returned' : `the ${war.name.toLowerCase()} cooled to a ${CONFLICT[p.conflict].name.toLowerCase()}`;
    logHistory(p, `Truce: ${text}`, 'conflict');
    events.push({ kind: 'truce', id: CONFLICT[p.conflict].id, text: 'truce', detail: text });
  }

  // A civil war is decided after CIVIL_WAR_SECONDS.
  if (p.conflict === CONFLICT.length - 1) {
    p.warT += dt;
    if (p.warT >= SIM.CIVIL_WAR_SECONDS) {
      const loyal = p.stability + SIM.LOYAL_ORDER * alloc.stability + SIM.LOYAL_EQUAL * (1 - p.inequality);
      const rebels = p.inequality + SIM.REBEL_TENSION * p.tension + SIM.REBEL_LUCK * rng();
      let ev;
      if (loyal >= rebels + SIM.CRUSH_MARGIN) {
        p.pop = Math.max(SIM.MIN_POP, p.pop * SIM.CRUSH_POP);
        p.inequality = clamp(p.inequality + SIM.CRUSH_INEQ);
        ev = { kind: 'conflict', id: 'crushed', text: 'rebels crushed', detail: 'the loyalists won the civil war' };
        logHistory(p, 'Civil war: the rebels were crushed', 'conflict');
      } else if (!p.factions && rng() < 0.6) {
        const share = SIM.SPLIT_MIN + (SIM.SPLIT_MAX - SIM.SPLIT_MIN) * rng();
        p.factions = [{ id: 'loyalists', name: 'Loyalists', share: 1 - share }, { id: 'rebels', name: 'Rebels', share }];
        ev = { kind: 'conflict', id: 'split', text: 'the world split', detail: `rebels hold ${Math.round(share * 100)}% of the people` };
        logHistory(p, `Civil war split the world: rebels hold ${Math.round(share * 100)}%`, 'conflict');
      } else if (p.tier > 0) {
        ev = collapseTier(p, 'civil_war');
        logHistory(p, `Civil war: ${ev.detail}`, 'setback');
        p.hardTimes = true;
      } else {
        p.pop = Math.max(SIM.MIN_POP, p.pop * SIM.CRUSH_POP);
        ev = { kind: 'conflict', id: 'scattered', text: 'clans scattered', detail: 'the camp broke apart and regrouped' };
        logHistory(p, 'Civil war: the clans scattered and regrouped', 'conflict');
      }
      events.push(ev);
      p.conflict = 1; p.warT = 0; p.conflictHold = 0;
      p.tension = Math.min(p.tension, SIM.AFTER_WAR_TENSION);
    }
  }

  // A split world reunites once it is calm and orderly for a while.
  if (p.factions) {
    if (p.stability >= SIM.REUNITE_STABILITY && p.tension <= SIM.REUNITE_TENSION) {
      p.reuniteT += dt;
      if (p.reuniteT >= SIM.REUNITE_SECONDS) {
        p.factions = null; p.reuniteT = 0;
        events.push({ kind: 'truce', id: 'reunited', text: 'reunited', detail: 'the factions made peace' });
        const m = reachMilestone(p, 'reunited'); if (m) events.push(m);
      }
    } else p.reuniteT = 0;
  }
}

// The loudest reason tension is high, for the history log.
function conflictCause(p, fx, w) {
  const causes = [
    [SIM.T_HUNGER * p.hunger, 'hungry clans fought over food'],
    [SIM.T_INEQ * Math.max(0, p.inequality - SIM.T_INEQ_FROM), 'the poor rose against the rich'],
    [SIM.T_ANGER * w('mood') * (fx.anger || 0), "your anger stirred them up"],
    [SIM.T_STRESS * w('stress') * fx.stress, 'your stress set them on edge'],
    [SIM.T_UNSTABLE * Math.max(0, SIM.T_UNSTABLE_FROM - p.stability), 'nobody trusts the elders'],
    [p.factions ? SIM.T_SPLIT : 0, 'old grudges between the factions'],
  ];
  causes.sort((a, b) => b[0] - a[0]);
  return causes[0][0] > 0 ? causes[0][1] : 'old grudges';
}

// Advance one inhabited world by dt seconds. Mutates p, returns events:
// { kind: 'era'|'good'|'bad'|'flavour'|'launch-ready'|'tier'|'setback'|'milestone'|'conflict'|'truce', text, cause?, detail? }
export function stepPlanet(p, dt, fx, rng = Math.random, attended = false) {
  const events = [];
  if (p.pop <= 0) return { events, growth: 0, research: 0 };
  p.age = (p.age || 0) + dt;
  const ch = p.channel || 'mixed';
  const w = (k) => (attended || ch === k ? 1 : SIM.OFF_CHANNEL);
  const alloc = normalise(p.god?.act?.(observe(p, fx)) || TEMPERAMENT[ch] || TEMPERAMENT.mixed);

  // Settlements, resources, stability, setbacks and conflict first: they shape growth.
  stepSettlement(p, dt, fx, alloc, w, rng, events);
  stepConflict(p, dt, fx, alloc, w, rng, events);
  const cap = popCap(p);

  // Population: logistic growth driven by calm, dragged by stress and hunger, capped by housing.
  const growth = (SIM.GROWTH_BASE + SIM.GROWTH_CALM * w('calm') * fx.calm - SIM.GROWTH_STRESS * w('stress') * fx.stress) * (0.5 + alloc.growth) * (1 - p.hunger);
  p.pop = clamp(p.pop + p.pop * growth * (1 - p.pop / cap) * dt, SIM.MIN_POP, Math.max(cap, SIM.MIN_POP));

  // Research: focus speeds it up, stress slows it, bigger settlements think faster.
  const research = clamp(SIM.RESEARCH_BASE + SIM.RESEARCH_FOCUS * w('focus') * fx.focus + SIM.RESEARCH_CALM * fx.calm - SIM.RESEARCH_STRESS * w('stress') * fx.stress, SIM.RESEARCH_MIN, SIM.RESEARCH_MAX)
    * (0.5 + alloc.research) * (SIM.TIER_RESEARCH_BASE + SIM.TIER_RESEARCH * p.tier);
  p.progress = Math.min(1, p.progress + (dt / SIM.ERA_SECONDS) * research);

  // Goat market: a log-price random walk whose volatility is the player's stress.
  const sigma = SIM.MARKET_SIGMA_BASE + SIM.MARKET_SIGMA_STRESS * w('stress') * fx.stress;
  p.market *= Math.exp(SIM.MARKET_REVERT * Math.log(SIM.MARKET_START / p.market) * dt + sigma * Math.sqrt(dt) * gaussOf(rng));
  p.marketPeak = Math.max(p.market, p.marketPeak - (p.marketPeak - p.market) * SIM.MARKET_PEAK_DECAY * dt);

  // A new era begins when research is done AND the settlement can hold it (ERAS[].enter).
  if (eraReady(p)) {
    p.eraIndex++; p.progress = 0;
    const era = ERAS[p.eraIndex];
    events.push({ kind: 'era', text: era.name });
    const m = reachMilestone(p, era.key); if (m) events.push(m);
  }

  const steady = 1 - 0.5 * alloc.stability;   // stability softens setbacks
  p.eventT -= dt;
  if (p.eventT <= 0) {
    p.eventT = SIM.EVENT_MIN + rng() * (SIM.EVENT_MAX - SIM.EVENT_MIN);
    if (ch === 'calm' || ch === 'mixed') {
      if (fx.calm > SIM.CALM_HIGH) {
        p.pop = Math.min(cap, p.pop * SIM.HARVEST_GAIN);
        p.food = Math.min(storage(p), p.food + Math.max(5, p.food * SIM.HARVEST_FOOD));
        events.push({ kind: 'good', text: 'bumper harvest', cause: 'a calm god' });
      } else if (fx.calm < SIM.CALM_LOW && rng() < SIM.CALM_LOW_CHANCE) {
        p.pop = Math.max(SIM.MIN_POP, p.pop * (1 - SIM.FAMINE_LOSS * steady));
        p.food *= 1 - SIM.HARVEST_FOOD * steady;
        events.push({ kind: 'bad', text: p.eraIndex === 0 ? 'famine' : 'poor harvest', cause: 'a restless god' });
      }
    }
    if (ch === 'focus' || ch === 'mixed') {
      if (fx.focus > SIM.FOCUS_HIGH) { p.progress = Math.min(1, p.progress + SIM.BREAKTHROUGH); events.push({ kind: 'good', text: 'breakthrough', cause: 'your focus' }); }
      else if (fx.focus < SIM.FOCUS_LOW && rng() < SIM.FOCUS_LOW_CHANCE) { p.progress = Math.max(0, p.progress - SIM.DARK_AGE * steady); events.push({ kind: 'bad', text: 'dark age', cause: 'a wandering mind' }); }
    }
    if (rng() < SIM.FLAVOUR_CHANCE) events.push({ kind: 'flavour', text: pickOf(FLAVOUR[p.eraIndex], rng) });
  }
  if (ch === 'stress' || ch === 'mixed') {
    if (!p.crashed && p.market < p.marketPeak * SIM.CRASH_RATIO) {
      p.crashed = true;
      p.pop = Math.max(SIM.MIN_POP, p.pop * (1 - (1 - SIM.CRASH_POP) * steady));
      p.progress = Math.max(0, p.progress - SIM.CRASH_PROGRESS * steady);
      p.goats *= SIM.CRASH_GOATS;
      events.push({ kind: 'bad', text: 'goat crash', cause: `your stress at ${Math.round(fx.stress * 100)}%` });
    } else if (!p.boomed && p.market > SIM.BOOM_LEVEL) {
      p.boomed = true;
      p.progress = Math.min(1, p.progress + SIM.BOOM_PROGRESS);
      p.goats = Math.min(storage(p), p.goats * SIM.BOOM_GOATS);
      events.push({ kind: 'good', text: 'goat boom', cause: 'a lively mind' });
    }
    if (p.market > p.marketPeak * SIM.CRASH_RESET) p.crashed = false;
    if (p.market < SIM.BOOM_RESET) p.boomed = false;
  }
  if (p.eraIndex === SIM.ERA_COUNT - 1) {
    p.launchT += dt * (0.6 + fx.focus);
    if (p.launchT >= SIM.LAUNCH_EVERY) {
      p.launchT = 0;
      events.push({ kind: 'launch-ready' });
      const m = reachMilestone(p, 'launch'); if (m) events.push(m);
    }
  }
  return { events, growth, research };
}

// A player blink: a storm on blink worlds, a direct strike on the attended world,
// occasionally a stray bolt elsewhere. On a world at war, lightning breaks a charge.
export function blinkPlanet(p, rng = Math.random, attended = false) {
  if (p.pop <= 0) return null;
  const own = attended || p.channel === 'blink';
  if (!own && rng() > SIM.STORM_CHANCE_OFF) return null;
  p.storms++;
  p.pop = Math.max(SIM.MIN_POP, p.pop * (own ? SIM.STORM_POP_OWN : SIM.STORM_POP_OFF));
  p.progress = Math.min(1, p.progress + (own ? SIM.STORM_PROGRESS_OWN : SIM.STORM_PROGRESS_OFF));
  if (attended && (p.conflict ?? 0) >= 2) p.tension = Math.max(0, p.tension - SIM.STORM_TENSION);
  const text = attended ? 'lightning strike!' : own ? pickOf(['storm!', 'lightning!', 'thunderstorm'], rng) : 'stray bolt';
  return { kind: own ? 'storm' : 'flavour', text };
}

// A camera cast (#33) aimed at a world in view. Returns { text, good } for the label, or null.
export const CAST_EFFECT = {
  spawn: (p) => { p.food = Math.min(storage(p), p.food + 10 + p.pop * 0.5); return { text: 'food from the sky', good: true }; },
  rain: (p) => { p.food = Math.min(storage(p), p.food + 5 + p.pop * 0.3); p.hunger *= 0.5; p.stability = clamp(p.stability + 0.05); return { text: 'gentle rain', good: true }; },
  bless: (p) => { p.stability = clamp(p.stability + 0.15); return { text: 'blessed', good: true }; },
  peace: (p) => {
    p.stability = clamp(p.stability + 0.1); p.setbackT = Math.max(p.setbackT, 3);
    p.tension = Math.max(0, p.tension - SIM.PEACE_TENSION);
    if (p.conflict > 0) { p.conflict--; p.conflictHold = 0; p.warT = 0; logHistory(p, 'Your peace forced a truce', 'conflict'); }
    return { text: 'peace', good: true };
  },
  disaster: (p) => { p.stability = clamp(p.stability - 0.15); p.pop = Math.max(SIM.MIN_POP, p.pop * 0.95); p.wood *= 0.8; return { text: 'storm', good: false }; },
  fireball: (p) => { p.stability = clamp(p.stability - 0.05); p.pop = Math.max(SIM.MIN_POP, p.pop * 0.97); return { text: 'fireball', good: false }; },
};
export function castPlanet(p, spell) {
  if (!p || p.pop <= 0 || !CAST_EFFECT[spell]) return null;
  return CAST_EFFECT[spell](p);
}

// The capital tribe on the surface (the AI god's Stone Age, src/ai/planet.js) finished an episode.
export function tribeOutcome(p, outcome) {
  const events = [];
  if (p.pop <= 0) return events;
  if (outcome === 'win') {
    p.food = Math.min(storage(p), p.food + SIM.TRIBE_WIN_FOOD);
    p.stability = clamp(p.stability + SIM.TRIBE_WIN_STABILITY);
    p.progress = Math.min(1, p.progress + SIM.TRIBE_WIN_PROGRESS);
    const m = reachMilestone(p, 'tribe'); if (m) events.push(m);
    events.push({ kind: 'good', text: 'the tribe thrives', cause: 'its AI god' });
  } else if (outcome === 'lose') {
    p.pop = Math.max(SIM.MIN_POP, p.pop * SIM.TRIBE_LOSS_POP);
    p.stability = clamp(p.stability - SIM.TRIBE_LOSS_STABILITY);
    p.hardTimes = true;
    logHistory(p, 'The capital tribe perished; a new one gathered', 'setback');
    events.push({ kind: 'setback', id: 'tribe', text: 'the tribe perished', detail: 'a new one gathered at the fire' });
  }
  return events;
}

// A ship arriving: settle a barren world or reinforce an inhabited one.
export function arriveShip(to) {
  if (to.pop <= 0) {
    const history = to.history || [], milestones = to.milestones || [];
    Object.assign(to, freshSettlement(), { pop: SIM.SETTLE_POP, eraIndex: 0, progress: 0, history, milestones, age: to.age || 0 });
    logHistory(to, 'Settlers landed', 'event');
    return 'settled';
  }
  to.pop += SIM.REINFORCE_POP;
  to.progress = Math.min(1, to.progress + SIM.REINFORCE_PROGRESS);
  return 'reinforced';
}

// Mind snapshot accumulator. Returns the channels ranked strongest-first once done.
export function newMind() { return { t: 0, calm: 0, focus: 0, stress: 0, blinks0: null, blinks: 0, done: false, ranked: null }; }
export function stepMind(m, playDt, fx, blinksTotal) {
  if (m.done || playDt <= 0) return null;
  if (m.blinks0 == null) m.blinks0 = blinksTotal;
  m.t += playDt;
  m.calm += fx.calm * playDt; m.focus += fx.focus * playDt; m.stress += (fx.stress || 0) * playDt;
  if (m.t < SIM.MIND_SECONDS) return null;
  m.done = true;
  m.calm /= m.t; m.focus /= m.t; m.stress /= m.t;
  m.blinks = blinksTotal - m.blinks0;
  m.ranked = CHANNEL_KEYS.map((ch) => ({ ch, s: CHANNEL_SCORE[ch](m) })).sort((a, b) => b.s - a.s);
  return m.ranked;
}
