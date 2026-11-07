// Calibration seed (#54, README §3.1 / §4.1).
//
// A configurable window at the start of play turns the player's signals into the seed for
// THEIR civilisation: friendliness, aggressiveness and violence. The other three
// civilisations always stay random. For the dummy run the player's seed is random too,
// but reproducible from a run seed, so a demo can be replayed exactly.
//
// Config (no code edits needed):
//   ?calib=<seconds>        window length, 1–120 (default 10; 10 s is a placeholder)
//   ?calib_mode=random      dummy run: traits from the run seed (default)
//   ?calib_mode=sensor      traits from the signals captured during the window
//   ?seed=<integer>         run seed, so a run (and a random setup) is reproducible
// The same keys can be saved in localStorage under "moc.calibration" as JSON.

import { mulberry32, clamp } from '../sim/util.js';

// Default window length. 10 s is a placeholder (README §3.1); ?calib=<seconds> overrides it.
export const CALIBRATION_SECONDS = 10;
export const CALIBRATION_DEFAULTS = Object.freeze({ seconds: CALIBRATION_SECONDS, mode: 'random' });
const STORAGE_KEY = 'moc.calibration';

export function readCalibrationConfig(search = globalThis.location?.search ?? '', storage = safeStorage()) {
  let saved = {};
  try { saved = JSON.parse(storage?.getItem(STORAGE_KEY) || '{}') || {}; } catch { saved = {}; }
  const q = new URLSearchParams(search);
  const seconds = Number(q.get('calib') ?? saved.seconds ?? CALIBRATION_DEFAULTS.seconds);
  const mode = (q.get('calib_mode') ?? saved.mode ?? CALIBRATION_DEFAULTS.mode) === 'sensor' ? 'sensor' : 'random';
  const seedRaw = q.get('seed') ?? saved.seed;
  const seed = seedRaw != null && seedRaw !== '' && Number.isFinite(Number(seedRaw)) ? Number(seedRaw) >>> 0 : (Math.random() * 2 ** 32) >>> 0;
  return {
    seconds: Number.isFinite(seconds) ? clamp(seconds, 1, 120) : CALIBRATION_DEFAULTS.seconds,
    mode,
    seed,
    seedFromConfig: seedRaw != null && seedRaw !== '',
  };
}

export function saveCalibrationConfig(config, storage = safeStorage()) {
  try { storage?.setItem(STORAGE_KEY, JSON.stringify({ seconds: config.seconds, mode: config.mode })); } catch { /* storage may be blocked */ }
}

function safeStorage() {
  try { return globalThis.localStorage ?? null; } catch { return null; }
}

// Random setup for the dummy run: reproducible from the run seed.
export function randomSeedTraits(seed) {
  const r = mulberry32(seed >>> 0);
  return { friendliness: round(r()), aggressiveness: round(r()), violence: round(r() * 0.8) };
}

// Sensor-to-trait formula (documented in docs/calibration.md). Inputs are window averages:
//   calm, focus           0..1 from the brain model
//   stress                0..1 brain.stress = max(agitation, face anger, 0.15·(1 - calm))
//   blinkRate             deliberate blinks per 10 s
//   joy, anger            0..1 from the face emotion manager (0 without a camera)
export function traitsFromFeatures(f) {
  const stress = clamp(f.stress ?? 0);
  const blinks = clamp((f.blinkRate ?? 0) / 6, 0, 1);
  const calm = clamp(f.calm ?? 0), focus = clamp(f.focus ?? 0);
  const joy = clamp(f.joy ?? 0), anger = clamp(f.anger ?? 0);
  return {
    friendliness: round(clamp(0.55 * calm + 0.35 * joy + 0.1 * (1 - anger))),
    aggressiveness: round(clamp(0.5 * stress + 0.35 * anger + 0.15 * focus)),
    violence: round(clamp(0.45 * anger + 0.35 * stress * (1 - calm) + 0.2 * blinks)),
  };
}

const round = (v) => Math.round(v * 1000) / 1000;

export class Calibration {
  constructor(config = readCalibrationConfig()) {
    this.config = config;
    this.t = 0;
    this.done = false;
    this.result = null;
    this.sum = { calm: 0, focus: 0, stress: 0, joy: 0, anger: 0, surprise: 0, sadness: 0, faceFrames: 0, n: 0 };
    this.blinksAtStart = null;
    this.listeners = [];
  }

  get seconds() { return this.config.seconds; }
  get remaining() { return Math.max(0, this.config.seconds - this.t); }
  onDone(fn) { this.listeners.push(fn); if (this.done) fn(this.result); }

  // Call with the seconds of real play this frame (0 while paused or on a menu).
  update(dt, brain, mood) {
    if (this.done || !(dt > 0)) return;
    if (this.blinksAtStart == null) this.blinksAtStart = brain?.stats?.blinks ?? 0;
    const s = this.sum;
    s.calm += (brain?.calm ?? 0) * dt;
    s.focus += (brain?.focus ?? 0) * dt;
    s.stress += (brain?.stress ?? 0) * dt;
    if (mood?.active && mood?.face) {
      s.joy += (mood.joy ?? 0) * dt; s.anger += (mood.anger ?? 0) * dt;
      s.surprise += (mood.surprise ?? 0) * dt; s.sadness += (mood.sadness ?? 0) * dt;
      s.faceFrames += dt;
    }
    s.n += dt;
    this.t += dt;
    if (this.t >= this.config.seconds) this.finish(brain);
  }

  finish(brain) {
    brain?.lockAgitationBaseline?.(this.config.seconds);   // the same window sets the resting agitation
    const s = this.sum, n = Math.max(1e-6, s.n), face = s.faceFrames > 0 ? s.faceFrames : 0;
    const blinks = Math.max(0, (brain?.stats?.blinks ?? 0) - (this.blinksAtStart ?? 0));
    const features = {
      calm: round(s.calm / n), focus: round(s.focus / n), stress: round(s.stress / n),
      blinkRate: round((blinks / n) * 10),
      joy: face ? round(s.joy / face) : 0, anger: face ? round(s.anger / face) : 0,
      surprise: face ? round(s.surprise / face) : 0, sadness: face ? round(s.sadness / face) : 0,
      faceSeen: face > 0,
    };
    const traits = this.config.mode === 'sensor' ? traitsFromFeatures(features) : randomSeedTraits(this.config.seed);
    this.result = { ...traits, source: this.config.mode, seconds: this.config.seconds, seed: this.config.seed, features };
    this.done = true;
    this.listeners.forEach((fn) => fn(this.result));
  }
}
