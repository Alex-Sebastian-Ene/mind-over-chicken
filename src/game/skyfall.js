// Skyfall (#160): the seeded EEG showcase event. A catastrophe is telegraphed for 15–20 s; the
// player must hold Focus/Insight above the focus threshold to charge a shield before impact.
//
// This file is the pure state machine: countdown, sustained charge, threshold, input source,
// cooldown, outcomes and the per-era configuration. It never touches the DOM, particles or the
// advisor. The eras render it and apply its consequences (src/eras/stone.js, industry.js,
// space.js) and main.js keeps the first-run tutorial flag. All timing comes from mulberry32 and
// the run seed, so a replay sees the same schedule; the file uses no Math.random.

import { clamp, mulberry32 } from '../sim/util.js';

export const SKYFALL_PHASE = Object.freeze({ IDLE: 'idle', ACTIVE: 'active', AVERTED: 'averted', FAILED: 'failed' });

// Per-era flavours. `countdown` stays inside the issue's 15–20 s window; `chargeSeconds` is how
// long focus must be held *continuously* (the shield drains quickly when it slips). Damage
// numbers are deliberately capped so a failed event is a real loss, never a soft-lock.
export const SKYFALL_CONFIG = Object.freeze({
  stone: Object.freeze({
    era: 'stone',
    title: 'Skyfall',
    threat: 'Meteor',
    countdown: 18,
    chargeSeconds: 4,
    threshold: 0.5,
    drain: 2.2,
    firstDelay: [22, 36],
    cooldown: [90, 120],
    outcomeSeconds: 5,
    damage: Object.freeze({ people: 2, food: 3, fear: 0.45 }),
    visuals: Object.freeze({ meteor: '#ff9a2e', trail: '#ffd166', shield: '#9ec9ff', impact: '#e8452c' }),
    warning: 'My lord! A meteor is falling on the tribe! Hold Insight above the focus threshold to raise the fire-shield. Keep holding: a single tap will not charge it.',
    tutorial: 'Coach: press and HOLD F (or hold your focus) until the shield ring fills. The meteor only stops if you stay above the focus line.',
    averted: 'The shield holds! The sky burns orange, then clears. The tribe dances around the fire and credits you loudly.',
    failed: 'The meteor hits. The tribe loses people, food and nerve. The fire still burns: they can recover.',
  }),
  industry: Object.freeze({
    era: 'industry',
    title: 'Skyfall',
    threat: 'Unnatural storm',
    countdown: 17,
    chargeSeconds: 3.5,
    threshold: 0.52,
    drain: 2.1,
    firstDelay: [24, 40],
    cooldown: [95, 125],
    outcomeSeconds: 5,
    damage: Object.freeze({ price: 0.18, fear: 0.4, confidence: 0.2, unrest: 0.2 }),
    visuals: Object.freeze({ meteor: '#7fb2ff', trail: '#d7e8ff', shield: '#59d8c8', impact: '#4358a8' }),
    warning: 'Bro! An unnatural storm is about to hit the Exchange! Hold Insight above the focus threshold to charge the lightning rods, or the market panics. Hold it steady.',
    tutorial: 'Coach: press and HOLD F (or hold your focus) to keep the rods charged. Tapping once will not calm the market.',
    averted: 'The rods discharge into the storm and the sky clears. The market steadies; analysts call it "divine alpha".',
    failed: 'The storm hits the Exchange. Prices gap down, confidence cracks and the traders panic. The market can still recover.',
  }),
  space: Object.freeze({
    era: 'space',
    title: 'Skyfall',
    threat: 'Solar flare',
    countdown: 20,
    chargeSeconds: 4.5,
    threshold: 0.55,
    drain: 2.0,
    firstDelay: [26, 44],
    cooldown: [100, 135],
    outcomeSeconds: 5,
    damage: Object.freeze({ progress: 0.18, courage: 0.25, count: 2 }),
    visuals: Object.freeze({ meteor: '#ffcf4a', trail: '#fff4c2', shield: '#59c9ff', impact: '#ff5a72' }),
    warning: 'Flight Director to god: a solar flare is inbound! Hold Insight above the focus threshold to raise the launch shield. The whole launch window is at stake.',
    tutorial: 'Coach: press and HOLD F (or hold your focus) until the shield reaches full. A quick tap will not protect the pad.',
    averted: 'The launch shield holds against the flare. The pad survives and the crowd cheers. The chickens demand names on the rocket.',
    failed: 'The flare washes over the pad. Parts are lost, the crowd scatters and the countdown slips. The launch can still be saved.',
  }),
});

const ERA_SALT = Object.freeze({ stone: 0x51a7, industry: 0x1d57, space: 0x5ace });

export function skyfallConfig(era) {
  return SKYFALL_CONFIG[era] || SKYFALL_CONFIG.stone;
}

function saltOf(era) {
  return ERA_SALT[era] || ERA_SALT.stone;
}

function roundTenth(n) {
  return Math.round(n * 10) / 10;
}

function between(rng, range) {
  return range[0] + rng() * (range[1] - range[0]);
}

// Seeded schedule for one era/run. Exported so tests and replays can assert the same first
// warning and cooldown without playing the countdown out.
export function skyfallSchedule(seed, config = SKYFALL_CONFIG.stone) {
  const rng = mulberry32(((seed >>> 0) ^ saltOf(config.era) ^ 0xa5a5a5a5) >>> 0);
  return {
    firstDelay: roundTenth(between(rng, config.firstDelay)),
    cooldown: roundTenth(between(rng, config.cooldown)),
  };
}

export function sourceLabel(source) {
  if (source === 'keyboard') return 'Keyboard F';
  if (source === 'eeg') return 'EEG focus';
  return 'No focus input';
}

// One optional HUD objective row while the event is visible (scenes append it to their own rows).
export function skyfallObjective(skyfall) {
  const s = skyfall?.snapshot?.();
  if (!s || (!s.active && !s.outcome)) return [];
  if (s.active) {
    return [{ label: 'Skyfall', value: `${Math.ceil(s.timeLeft)}s · shield ${Math.round(s.progress * 100)}%`, bad: true }];
  }
  return [{ label: 'Skyfall', value: s.phase === 'averted' ? 'averted' : 'struck', done: s.phase === 'averted', bad: s.phase === 'failed' }];
}

// Build the state machine's input from the live game. The keyboard fallback is the same
// G.source.input.focus path as the Stone fire hold; the EEG path is brain.focus.
export function skyfallInput(G, config = SKYFALL_CONFIG.stone, { eligible = true } = {}) {
  const brain = G?.brain;
  let threshold = Number(config.threshold ?? 0.5);
  if (brain?.threshold) {
    try {
      const t = brain.threshold('focus', threshold);
      if (Number.isFinite(t)) threshold = t;
    } catch { /* keep the config threshold */ }
  }
  const keyboard = !!G?.source?.input?.focus;
  return {
    focus: Math.max(clamp(Number(brain?.focus) || 0), keyboard ? 1 : 0),
    threshold,
    keyboard,
    sourceKind: G?.source?.kind || 'sim',
    eligible,
  };
}

export class Skyfall {
  constructor({ era = 'stone', seed = 1, tutorial = true, config = null } = {}) {
    this.seed = (Number(seed) >>> 0) || 1;
    this.config = config ? { ...config } : skyfallConfig(era);
    this.tutorialDue = !!tutorial;
    this.rng = mulberry32(((this.seed ^ saltOf(this.config.era) ^ 0x1234abcd) >>> 0));
    const schedule = skyfallSchedule(this.seed, this.config);
    this.firstDelay = schedule.firstDelay;
    this.nextCooldown = schedule.cooldown;
    this.resetState();
  }

  resetState() {
    this.phase = SKYFALL_PHASE.IDLE;
    this.cooldownLeft = this.firstDelay;
    this.timeLeft = 0;
    this.charge = 0;
    this.held = 0;
    this.focus = 0;
    this.threshold = this.config.threshold;
    this.source = 'none';
    this.outcomeLeft = 0;
    this.events = 0;
    this.lastOutcome = null;
    this.startedAt = null;
  }

  // A scene change, retry or a new run starts the schedule from scratch. The tutorial can be
  // kept or consumed by the caller (G.skyfallTutorialSeen carries it across eras in main.js).
  reset({ seed = this.seed, tutorial = this.tutorialDue, config = this.config } = {}) {
    this.seed = (Number(seed) >>> 0) || 1;
    this.config = config ? { ...config } : this.config;
    this.tutorialDue = !!tutorial;
    this.rng = mulberry32(((this.seed ^ saltOf(this.config.era) ^ 0x1234abcd) >>> 0));
    const schedule = skyfallSchedule(this.seed, this.config);
    this.firstDelay = schedule.firstDelay;
    this.nextCooldown = schedule.cooldown;
    this.resetState();
    return this;
  }

  get active() { return this.phase === SKYFALL_PHASE.ACTIVE; }
  get outcome() { return this.phase === SKYFALL_PHASE.AVERTED || this.phase === SKYFALL_PHASE.FAILED ? this.phase : null; }

  startEvent() {
    if (this.phase !== SKYFALL_PHASE.IDLE || this.cooldownLeft > 0) return null;
    this.phase = SKYFALL_PHASE.ACTIVE;
    this.timeLeft = this.config.countdown;
    this.charge = 0;
    this.held = 0;
    this.focus = 0;
    this.threshold = this.config.threshold;
    this.source = 'none';
    this.outcomeLeft = 0;
    this.lastOutcome = null;
    this.events += 1;
    this.startedAt = this.events;
    const tutorial = this.tutorialDue;
    this.tutorialDue = false;
    return tutorial;
  }

  resolve(phase) {
    this.phase = phase;
    this.lastOutcome = phase;
    this.outcomeLeft = this.config.outcomeSeconds;
    this.cooldownLeft = this.nextCooldown;
  }

  // Stop an event without an outcome (a scene moved on, the launch started, the run ended).
  cancel() {
    if (this.phase === SKYFALL_PHASE.ACTIVE) {
      this.phase = SKYFALL_PHASE.IDLE;
      this.cooldownLeft = this.nextCooldown;
      this.timeLeft = 0;
      this.charge = 0;
      this.focus = 0;
      this.source = 'none';
    }
    return this;
  }

  snapshot(extra = {}) {
    const chargeTarget = Math.max(0.001, this.config.chargeSeconds);
    return {
      era: this.config.era,
      title: this.config.title,
      threat: this.config.threat,
      visuals: this.config.visuals,
      phase: this.phase,
      active: this.phase === SKYFALL_PHASE.ACTIVE,
      outcome: this.outcome,
      countdown: this.config.countdown,
      timeLeft: this.timeLeft,
      charge: this.charge,
      chargeTarget,
      progress: clamp(this.charge / chargeTarget),
      held: this.held,
      focus: clamp(this.focus || 0),
      threshold: this.threshold ?? this.config.threshold,
      source: this.source,
      sourceLabel: sourceLabel(this.source),
      tutorial: !!this.tutorialDue,
      cooldownLeft: Math.max(0, this.cooldownLeft),
      secondsUntilNext: this.phase === SKYFALL_PHASE.IDLE ? Math.max(0, this.cooldownLeft) : null,
      outcomeLeft: Math.max(0, this.outcomeLeft),
      events: this.events,
      lastOutcome: this.lastOutcome,
      started: false,
      averted: false,
      failed: false,
      paused: false,
      ...extra,
    };
  }

  // dt in seconds; input = { focus, threshold, keyboard, sourceKind, eligible }.
  // Returns this tick's snapshot with one-shot `started`, `averted` and `failed` flags.
  update(dt, input = {}) {
    const step = Number.isFinite(dt) && dt > 0 ? dt : 0;
    const out = this.snapshot();

    // Intros, fail/end screens, a paused signal or a finished scene must not consume a countdown.
    if (input.eligible === false) {
      out.paused = true;
      return out;
    }

    if (this.phase === SKYFALL_PHASE.IDLE) {
      if (this.cooldownLeft > 0) this.cooldownLeft = Math.max(0, this.cooldownLeft - step);
      if (this.cooldownLeft <= 0) {
        const tutorial = this.startEvent();
        if (tutorial !== null) {
          out.started = true;
          out.tutorial = tutorial;
          return Object.assign(out, this.snapshot({ started: true, tutorial }));
        }
      }
      return Object.assign(out, this.snapshot());
    }

    if (this.phase === SKYFALL_PHASE.ACTIVE) {
      const focus = clamp(Number(input.focus) || 0);
      let threshold = Number(input.threshold);
      if (!Number.isFinite(threshold)) threshold = this.config.threshold;
      this.focus = focus;
      this.threshold = threshold;
      this.source = input.keyboard ? 'keyboard' : input.sourceKind === 'bridge' ? 'eeg' : 'eeg';

      // Sustained focus charges; a tap adds almost nothing because the shield drains fast.
      if (focus >= threshold) {
        this.charge = Math.min(this.config.chargeSeconds, this.charge + step);
        this.held += step;
      } else {
        this.held = 0;
        this.charge = Math.max(0, this.charge - step * (this.config.drain ?? 2));
      }
      this.timeLeft = Math.max(0, this.timeLeft - step);

      out.averted = false;
      out.failed = false;
      if (this.charge >= this.config.chargeSeconds) {
        this.resolve(SKYFALL_PHASE.AVERTED);
        out.averted = true;
      } else if (this.timeLeft <= 0) {
        this.resolve(SKYFALL_PHASE.FAILED);
        out.failed = true;
      }
      return Object.assign(out, this.snapshot({ averted: out.averted, failed: out.failed }));
    }

    // AVERTED / FAILED: hold the outcome on screen for a few seconds, then start the cooldown.
    this.outcomeLeft = Math.max(0, this.outcomeLeft - step);
    if (this.outcomeLeft <= 0) {
      this.phase = SKYFALL_PHASE.IDLE;
      this.timeLeft = 0;
      this.charge = 0;
      this.held = 0;
      this.focus = 0;
      this.source = 'none';
    }
    return Object.assign(out, this.snapshot());
  }
}
