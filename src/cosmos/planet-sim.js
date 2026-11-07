// Pure planet simulation: plain data in, plain data + events out. No DOM, no drawing.
// This is the reference for the Rust port — every rule a world follows lives here.
//
// A world hears ONE brain channel loudly (weight 1) and the others softly (OFF_CHANNEL):
//   calm  → growth and harvests      focus → research and era progress
//   pulse → goat-market volatility   blink → storms (each blink)
// The world the player is zoomed into ("attended") hears EVERY channel at weight 1, and a blink
// strikes it directly. The player's brain always applies; an optional AI god (p.god) only chooses
// the civilisation's effort split and never touches the player's input.
// `rng` is any () => [0,1) function, so a port or a test can make runs deterministic.

export const SIM = {
  ERA_COUNT: 4,                 // stone, farming, industry, space
  ERA_SECONDS: 15,              // seconds per era at research rate 1
  CAP: [60, 400, 3000, 12000],  // population cap per era
  MIN_POP: 3,
  OFF_CHANNEL: 0.25,            // how much a world hears the channels that aren't its own
  // growth (per second, logistic): (BASE + CALM*w_calm*calm - STRESS*w_pulse*stress) * (0.5 + alloc.growth)
  GROWTH_BASE: 0.03, GROWTH_CALM: 0.2, GROWTH_STRESS: 0.1,
  // research rate: clamp(BASE + FOCUS*w_focus*focus + CALM*calm - STRESS*w_pulse*stress, MIN, MAX) * (0.5 + alloc.research)
  RESEARCH_BASE: 0.45, RESEARCH_FOCUS: 1.5, RESEARCH_CALM: 0.2, RESEARCH_STRESS: 0.4, RESEARCH_MIN: 0.1, RESEARCH_MAX: 2.4,
  // goat market (log-price): dlnM = REVERT*ln(100/M)*dt + (SIGMA_BASE + SIGMA_PULSE*w_pulse*stress)*sqrt(dt)*N(0,1)
  MARKET_START: 100, MARKET_REVERT: 0.15, MARKET_SIGMA_BASE: 0.02, MARKET_SIGMA_PULSE: 0.22,
  MARKET_PEAK_DECAY: 0.05,      // peak relaxes toward price at this rate per second
  CRASH_RATIO: 0.78, CRASH_RESET: 0.92, BOOM_LEVEL: 128, BOOM_RESET: 115,
  CRASH_POP: 0.92, CRASH_PROGRESS: 0.2, BOOM_PROGRESS: 0.15,
  // channel events, checked every EVENT_MIN..EVENT_MAX seconds
  EVENT_MIN: 4, EVENT_MAX: 7,
  CALM_HIGH: 0.55, CALM_LOW: 0.12, CALM_LOW_CHANCE: 0.5, HARVEST_GAIN: 1.15, FAMINE_LOSS: 0.15,
  FOCUS_HIGH: 0.55, FOCUS_LOW: 0.1, FOCUS_LOW_CHANCE: 0.35, BREAKTHROUGH: 0.2, DARK_AGE: 0.15,
  FLAVOUR_CHANCE: 0.25,
  // blink storms
  STORM_CHANCE_OFF: 0.12, STORM_POP_OWN: 0.97, STORM_POP_OFF: 0.99, STORM_PROGRESS_OWN: 0.05, STORM_PROGRESS_OFF: 0.01,
  // space travel
  LAUNCH_EVERY: 7,              // launch charge needed; charge += dt * (0.6 + focus)
  LESSER_TARGET_CHANCE: 0.35,   // chance to reinforce a less advanced world when nothing is barren
  SHIP_MIN: 5, SHIP_MAX: 7, SETTLE_POP: 12, REINFORCE_POP: 8, REINFORCE_PROGRESS: 0.1,
  // stress = clamp(hrDelta / STRESS_BPM, 0, STRESS_MAX)
  STRESS_BPM: 20, STRESS_MAX: 1.5,
  // mind snapshot
  MIND_SECONDS: 10,
};

export const CHANNEL_KEYS = ['calm', 'focus', 'pulse', 'blink'];

// How strongly each channel showed up in the first MIND_SECONDS of play (comparable scales).
export const CHANNEL_SCORE = {
  calm: (m) => m.calm / 0.35,
  focus: (m) => m.focus / 0.3,
  pulse: (m) => Math.max(0, m.hrDelta ? m.hrDelta / 8 : (m.stress ?? 0) * 2.5),
  blink: (m) => m.blinks / 3,
};

// Effort split a world's civilisation uses when no AI god is steering it.
export const TEMPERAMENT = {
  calm: { growth: 0.55, research: 0.25, stability: 0.2 },
  focus: { growth: 0.2, research: 0.6, stability: 0.2 },
  pulse: { growth: 0.4, research: 0.5, stability: 0.1 },
  blink: { growth: 0.3, research: 0.4, stability: 0.3 },
  mixed: { growth: 0.4, research: 0.4, stability: 0.2 },
};

const FLAVOUR = [
  ['invented the spear', 'worships a rock', 'met a mammoth'],
  ['domesticated goats', 'invented cheese', 'elected a chicken'],
  ['opened a goat exchange', 'invented the strike', 'built a mill'],
  ['launched a satellite', 'found water on a moon', 'built a goat in orbit'],
];
const ERA_TITLES = ['Stone Age', 'Farming', 'Industry', 'Space Age'];

const clamp = (v, lo = 0, hi = 1) => (v < lo ? lo : v > hi ? hi : v);
const pickOf = (arr, rng) => arr[Math.floor(rng() * arr.length)];
function gaussOf(rng) {
  let u = 0, v = 0;
  while (u === 0) u = rng();
  while (v === 0) v = rng();
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}

// Plain planet state. Presentation fields (labels, textures) live elsewhere.
export function newPlanetState({ channel = 'mixed', pop = 0, eraIndex = 0 } = {}, rng = Math.random) {
  return {
    channel, pop, eraIndex, progress: 0,
    market: SIM.MARKET_START, marketPeak: SIM.MARKET_START, crashed: false, boomed: false,
    eventT: SIM.EVENT_MIN + rng() * (SIM.EVENT_MAX - SIM.EVENT_MIN),
    launchT: 0, storms: 0,
    god: null,          // AI-god seam: { act(observation) → { growth, research, stability } }
  };
}

// Brain readings every world sees this frame.
export function brainEffects({ calm, focus, hrDelta, blinksLast10s, panic }) {
  const pulseStress = hrDelta ? clamp(hrDelta / SIM.STRESS_BPM, 0, SIM.STRESS_MAX) : 0;
  const eegStress = Math.max(panic || 0, calm < 0.35 ? (0.35 - calm) * 1.5 : 0);
  const stress = Math.max(pulseStress, clamp(eegStress, 0, SIM.STRESS_MAX));
  return { calm, focus, hrDelta: hrDelta || 0, stress, blinkRate: blinksLast10s / 10 };
}

export function observe(p, fx) {
  return { era: p.eraIndex, pop: p.pop, cap: SIM.CAP[p.eraIndex], progress: p.progress, market: p.market, storms: p.storms, channel: p.channel, brain: fx };
}

function normalise(a) {
  const g = Math.max(0, a.growth ?? 0), r = Math.max(0, a.research ?? 0), s = Math.max(0, a.stability ?? 0);
  const t = g + r + s || 1;
  return { growth: g / t, research: r / t, stability: s / t };
}

// Advance one inhabited world by dt seconds. Mutates p, returns events:
// { kind: 'era'|'good'|'bad'|'flavour'|'launch-ready', text, cause? }
export function stepPlanet(p, dt, fx, rng = Math.random, attended = false) {
  const events = [];
  if (p.pop <= 0) return { events, growth: 0, research: 0 };
  const ch = p.channel || 'mixed';
  const w = (k) => (attended || ch === k ? 1 : SIM.OFF_CHANNEL);
  const alloc = normalise(p.god?.act?.(observe(p, fx)) || TEMPERAMENT[ch] || TEMPERAMENT.mixed);
  const cap = SIM.CAP[p.eraIndex];

  // Population: logistic growth driven by calm, dragged by stress.
  const growth = (SIM.GROWTH_BASE + SIM.GROWTH_CALM * w('calm') * fx.calm - SIM.GROWTH_STRESS * w('pulse') * fx.stress) * (0.5 + alloc.growth);
  p.pop = clamp(p.pop + p.pop * growth * (1 - p.pop / cap) * dt, SIM.MIN_POP, cap);

  // Research: focus speeds it up, stress slows it.
  const research = clamp(SIM.RESEARCH_BASE + SIM.RESEARCH_FOCUS * w('focus') * fx.focus + SIM.RESEARCH_CALM * fx.calm - SIM.RESEARCH_STRESS * w('pulse') * fx.stress, SIM.RESEARCH_MIN, SIM.RESEARCH_MAX) * (0.5 + alloc.research);
  p.progress += (dt / SIM.ERA_SECONDS) * research;

  // Goat market: a log-price random walk whose volatility is the player's pulse.
  const sigma = SIM.MARKET_SIGMA_BASE + SIM.MARKET_SIGMA_PULSE * w('pulse') * fx.stress;
  p.market *= Math.exp(SIM.MARKET_REVERT * Math.log(SIM.MARKET_START / p.market) * dt + sigma * Math.sqrt(dt) * gaussOf(rng));
  p.marketPeak = Math.max(p.market, p.marketPeak - (p.marketPeak - p.market) * SIM.MARKET_PEAK_DECAY * dt);

  if (p.progress >= 1 && p.eraIndex < SIM.ERA_COUNT - 1) {
    p.eraIndex++; p.progress = 0;
    events.push({ kind: 'era', text: ERA_TITLES[p.eraIndex] });
  }
  p.progress = Math.min(p.progress, 1);

  const steady = 1 - 0.5 * alloc.stability;   // stability softens setbacks
  p.eventT -= dt;
  if (p.eventT <= 0) {
    p.eventT = SIM.EVENT_MIN + rng() * (SIM.EVENT_MAX - SIM.EVENT_MIN);
    if (ch === 'calm' || ch === 'mixed') {
      if (fx.calm > SIM.CALM_HIGH) { p.pop = Math.min(cap, p.pop * SIM.HARVEST_GAIN); events.push({ kind: 'good', text: 'bumper harvest', cause: 'a calm god' }); }
      else if (fx.calm < SIM.CALM_LOW && rng() < SIM.CALM_LOW_CHANCE) { p.pop = Math.max(SIM.MIN_POP, p.pop * (1 - SIM.FAMINE_LOSS * steady)); events.push({ kind: 'bad', text: p.eraIndex === 0 ? 'famine' : 'poor harvest', cause: 'a restless god' }); }
    }
    if (ch === 'focus' || ch === 'mixed') {
      if (fx.focus > SIM.FOCUS_HIGH) { p.progress = Math.min(1, p.progress + SIM.BREAKTHROUGH); events.push({ kind: 'good', text: 'breakthrough', cause: 'your focus' }); }
      else if (fx.focus < SIM.FOCUS_LOW && rng() < SIM.FOCUS_LOW_CHANCE) { p.progress = Math.max(0, p.progress - SIM.DARK_AGE * steady); events.push({ kind: 'bad', text: 'dark age', cause: 'a wandering mind' }); }
    }
    if (rng() < SIM.FLAVOUR_CHANCE) events.push({ kind: 'flavour', text: pickOf(FLAVOUR[p.eraIndex], rng) });
  }
  if (ch === 'pulse' || ch === 'mixed') {
    if (!p.crashed && p.market < p.marketPeak * SIM.CRASH_RATIO) {
      p.crashed = true;
      p.pop = Math.max(SIM.MIN_POP, p.pop * (1 - (1 - SIM.CRASH_POP) * steady));
      p.progress = Math.max(0, p.progress - SIM.CRASH_PROGRESS * steady);
      const cause = fx.hrDelta ? `your pulse at ${fx.hrDelta >= 0 ? '+' : ''}${Math.round(fx.hrDelta)} bpm` : 'your tension';
      events.push({ kind: 'bad', text: 'goat crash', cause });
    } else if (!p.boomed && p.market > SIM.BOOM_LEVEL) {
      p.boomed = true;
      p.progress = Math.min(1, p.progress + SIM.BOOM_PROGRESS);
      const boomCause = fx.hrDelta ? 'a lively pulse' : 'a lively mind';
      events.push({ kind: 'good', text: 'goat boom', cause: boomCause });
    }
    if (p.market > p.marketPeak * SIM.CRASH_RESET) p.crashed = false;
    if (p.market < SIM.BOOM_RESET) p.boomed = false;
  }
  if (p.eraIndex === SIM.ERA_COUNT - 1) {
    p.launchT += dt * (0.6 + fx.focus);
    if (p.launchT >= SIM.LAUNCH_EVERY) { p.launchT = 0; events.push({ kind: 'launch-ready' }); }
  }
  return { events, growth, research };
}

// A player blink: a storm on blink worlds, a direct strike on the attended world,
// occasionally a stray bolt elsewhere.
export function blinkPlanet(p, rng = Math.random, attended = false) {
  if (p.pop <= 0) return null;
  const own = attended || p.channel === 'blink';
  if (!own && rng() > SIM.STORM_CHANCE_OFF) return null;
  p.storms++;
  p.pop = Math.max(SIM.MIN_POP, p.pop * (own ? SIM.STORM_POP_OWN : SIM.STORM_POP_OFF));
  p.progress = Math.min(1, p.progress + (own ? SIM.STORM_PROGRESS_OWN : SIM.STORM_PROGRESS_OFF));
  const text = attended ? 'lightning strike!' : own ? pickOf(['storm!', 'lightning!', 'thunderstorm'], rng) : 'stray bolt';
  return { kind: own ? 'storm' : 'flavour', text };
}

// A ship arriving: settle a barren world or reinforce an inhabited one.
export function arriveShip(to) {
  if (to.pop <= 0) { to.pop = SIM.SETTLE_POP; to.eraIndex = 0; to.progress = 0; return 'settled'; }
  to.pop += SIM.REINFORCE_POP;
  to.progress = Math.min(1, to.progress + SIM.REINFORCE_PROGRESS);
  return 'reinforced';
}

// Mind snapshot accumulator. Returns the channels ranked strongest-first once done.
export function newMind() { return { t: 0, calm: 0, focus: 0, hr: 0, stress: 0, blinks0: null, blinks: 0, hrDelta: 0, done: false, ranked: null }; }
export function stepMind(m, playDt, fx, blinksTotal) {
  if (m.done || playDt <= 0) return null;
  if (m.blinks0 == null) m.blinks0 = blinksTotal;
  m.t += playDt;
  m.calm += fx.calm * playDt; m.focus += fx.focus * playDt; m.hr += fx.hrDelta * playDt;
  m.stress = (m.stress || 0) + (fx.stress || 0) * playDt;
  if (m.t < SIM.MIND_SECONDS) return null;
  m.done = true;
  m.calm /= m.t; m.focus /= m.t; m.hrDelta = m.hr / m.t; m.stress /= m.t;
  m.blinks = blinksTotal - m.blinks0;
  m.ranked = CHANNEL_KEYS.map((ch) => ({ ch, s: CHANNEL_SCORE[ch](m) })).sort((a, b) => b.s - a.s);
  return m.ranked;
}
