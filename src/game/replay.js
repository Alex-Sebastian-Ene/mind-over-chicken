// Replayability (#91): run seeds, share codes, daily/weekly challenges, challenge modifiers,
// run scores, unlocks and the end-of-run chronicle. Pure and DOM-free: main.js and the UI pass
// in plain data, so node tests can check every rule without a browser.
//
// A run is one world seed plus the New Game choices. The share code packs all of it into a short
// Crockford-style base32 string (seed, world size, difficulty, challenge kind, modifiers and the
// New Mind+ flag) with a checksum, so a typo is rejected instead of starting a different run.
// Nothing here uses Math.random: daily/weekly seeds are hashes of the UTC date, and a fresh seed
// comes from crypto.getRandomValues (or the clock).

import { SIZE_CHOICES, DIFFICULTIES } from './options.js';

export const REPLAY_VERSION = 1;
export const SETTINGS_KEY = 'moc.replay.v1';
export const UNLOCKS_KEY = 'moc.replay.unlocks.v1';

const SIZE_IDS = [...SIZE_CHOICES];
const KIND_INDEX = { none: 0, daily: 1, weekly: 2 };
const KIND_NAMES = ['none', 'daily', 'weekly'];

export const CHALLENGES = [
  { id: 'none', label: 'Free run' },
  { id: 'daily', label: 'Daily' },
  { id: 'weekly', label: 'Weekly' },
];

// ---------- challenge modifiers ----------
// The table is the single source of truth. Each effect is read by the system named in `readBy`
// (main.js or the era file), and `score` is the style bonus the modifier is worth.
export const MODIFIERS = [
  {
    id: 'fragile-fire', label: 'Fragile fire', short: 'Fragile fire',
    blurb: 'The flame fades faster and the tribe gets hungry sooner.',
    readBy: 'src/eras/stone.js', score: 8,
    effects: { fireDecay: 1.85, hunger: 1.3 },
  },
  {
    id: 'volatile-market', label: 'Volatile market', short: 'Volatile market',
    blurb: 'Chicken futures swing harder; recessions come sooner.',
    readBy: 'src/eras/industry.js', score: 8,
    effects: { volatility: 1.9, momentum: 1.5 },
  },
  {
    id: 'blink-storm', label: 'Blink storm', short: 'Blink storm',
    blurb: 'Lightning answers blinks almost twice as often.',
    readBy: 'src/eras/stone.js', score: 6,
    effects: { boltCooldown: 0.22 },
  },
  {
    id: 'no-camera', label: 'No camera', short: 'No camera',
    blurb: 'The webcam stays off: face mood and hand moves cannot help.',
    readBy: 'src/main.js', score: 10,
    effects: { camera: 0 },
  },
  {
    id: 'one-hand', label: 'One hand', short: 'One hand',
    blurb: 'Two-hand pinch zoom is disabled; one hand still pans.',
    readBy: 'src/main.js', score: 6,
    effects: { twoHandZoom: 0 },
  },
  {
    id: 'pacifist', label: 'Pacifist', short: 'Pacifist',
    blurb: 'Lightning and fireballs are off. Win without smiting.',
    readBy: 'src/main.js', score: 12,
    effects: { combat: 0 },
  },
];

export const MODIFIER_IDS = MODIFIERS.map((m) => m.id);
const MODIFIER_BY_ID = new Map(MODIFIERS.map((m) => [m.id, m]));

// ---------- small helpers ----------
const u32 = (v) => (Number.isFinite(Number(v)) ? Number(v) >>> 0 : 0);
const num = (v, d = 0) => (Number.isFinite(Number(v)) ? Number(v) : d);
const clamp100 = (v) => Math.max(0, Math.min(100, Math.round(v)));
const finiteSeed = (v) => Number.isFinite(Number(v)) && Number(v) >= 0 && Number(v) <= 0xffffffff;

// FNV-1a, 32-bit. Stable across engines: only integer ops.
export function hash32(text) {
  let h = 2166136261 >>> 0;
  const s = String(text ?? '');
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h >>> 0;
}

// A fresh seed without Math.random: crypto when the browser has it, else the clock.
export function randomSeed(now = Date.now(), cryptoObj = globalThis.crypto) {
  try {
    if (cryptoObj?.getRandomValues) {
      const a = new Uint32Array(1);
      cryptoObj.getRandomValues(a);
      return a[0] >>> 0;
    }
  } catch { /* no crypto here */ }
  return hash32(`moc:${Math.floor(num(now, Date.now()))}:${Math.floor(performance.now?.() ?? 0)}`);
}

// ---------- seed text ----------
export function normalizeModifiers(list) {
  const want = new Set(Array.isArray(list) ? list.map(String) : []);
  return MODIFIER_IDS.filter((id) => want.has(id));
}

export function hasModifier(source, id) {
  const list = source?.replay?.modifiers ?? source?.modifiers ?? [];
  return Array.isArray(list) && list.includes(id);
}

// The modifier's effect when active, otherwise `base`.
export function modifierEffect(source, id, key, base = 1) {
  if (!hasModifier(source, id)) return base;
  const fx = MODIFIER_BY_ID.get(id)?.effects || {};
  return Object.prototype.hasOwnProperty.call(fx, key) ? fx[key] : base;
}

export const cameraEnabled = (G) => !hasModifier(G, 'no-camera');
export const twoHandZoomEnabled = (G) => !hasModifier(G, 'one-hand');
export const combatEnabled = (G) => !hasModifier(G, 'pacifist');

// `?seed=` accepts a number, a share code or any short text (hashed). A share code wins.
export function parseSeedText(text) {
  const raw = String(text ?? '').trim();
  if (!raw) return null;
  const code = decodeShareCode(raw);
  if (code) return { seed: code.seed, code };
  // A mistyped MOC code must be rejected, not quietly hashed into a different run.
  if (/^MOC[\s-]/i.test(raw)) return null;
  if (/^\d+$/.test(raw) && finiteSeed(raw)) return { seed: u32(raw), code: null };
  return { seed: hash32(raw.toLowerCase()), code: null };
}

// ---------- UTC daily / weekly challenges ----------
function isoWeekKey(date) {
  const d = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()));
  const day = d.getUTCDay() || 7;             // Monday = 1, Sunday = 7
  d.setUTCDate(d.getUTCDate() + 4 - day);     // the Thursday of this ISO week
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
  const week = Math.ceil((((d - yearStart) / 86400000) + 1) / 7);
  return `${d.getUTCFullYear()}-W${String(week).padStart(2, '0')}`;
}

export function challengeKey(kind, date = Date.now()) {
  if (kind !== 'daily' && kind !== 'weekly') return '';
  const d = date instanceof Date ? date : new Date(date);
  if (!Number.isFinite(d.getTime())) return '';
  if (kind === 'daily') return d.toISOString().slice(0, 10);
  return isoWeekKey(d);
}

export function challengeFor(kind, date = Date.now()) {
  if (kind !== 'daily' && kind !== 'weekly') return { kind: 'none', key: '', seed: null };
  const key = challengeKey(kind, date);
  return { kind, key, seed: hash32(`moc:${kind}:${key}`) };
}

// The player's local calendar, deliberately different from the UTC key that sets the seed.
export function challengeDateLabel(kind, date = Date.now()) {
  const d = date instanceof Date ? date : new Date(date);
  if (!Number.isFinite(d.getTime())) return '';
  const fmt = (x) => x.toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'short' });
  if (kind === 'daily') return fmt(d);
  if (kind === 'weekly') {
    // The ISO week's Monday and Sunday, shown in the player's local calendar.
    const start = new Date(d);
    const day = (start.getUTCDay() + 6) % 7;
    start.setUTCDate(start.getUTCDate() - day);
    const end = new Date(start);
    end.setUTCDate(end.getUTCDate() + 6);
    return `${fmt(start)} – ${fmt(end)}`;
  }
  return '';
}

// ---------- share codes ----------
const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';   // Crockford base32: no I, L, O, U
const CHAR_INDEX = new Map([...ALPHABET].map((c, i) => [c, i]));
// Crockford aliases, so a code read aloud or typed from memory still works.
const ALIASES = { I: '1', L: '1', O: '0', U: 'V' };
const PAYLOAD_CHARS = 10;   // 49 bits in 10 chars
const CHECK_CHARS = 2;
const BITS = { version: 3, seed: 32, size: 3, difficulty: 2, kind: 2, modifiers: 6, mindPlus: 1 };
const OFFSET = (() => {
  const out = {};
  let at = 0;
  for (const [key, width] of Object.entries(BITS)) { out[key] = at; at += width; }
  return out;
})();

function cleanCode(text) {
  let s = String(text ?? '').trim().toUpperCase();
  s = s.replace(/^MOC[\s-]*/, '').replace(/[\s-]+/g, '');
  return [...s].map((c) => ALIASES[c] || c).join('');
}

function base32(value, chars) {
  let v = Math.floor(value);
  let out = '';
  for (let i = 0; i < chars; i++) { out = ALPHABET[v % 32] + out; v = Math.floor(v / 32); }
  return out;
}

function fromBase32(text) {
  let v = 0;
  for (const c of text) {
    const i = CHAR_INDEX.get(c);
    if (i == null) return null;
    v = v * 32 + i;
  }
  return v;
}

function checksumChars(payload) {
  const h = hash32(`moc-share:${payload}`);
  return base32(h % (32 ** CHECK_CHARS), CHECK_CHARS);
}

export function encodeShareCode(run = {}) {
  const size = Math.max(0, SIZE_IDS.indexOf(run.worldSize ?? 'medium'));
  const diff = Math.max(0, DIFFICULTIES.indexOf(run.difficulty ?? 'normal'));
  const kind = KIND_INDEX[run.challenge] ?? 0;
  const modifiers = normalizeModifiers(run.modifiers);
  let mask = 0;
  for (const id of modifiers) {
    const i = MODIFIER_IDS.indexOf(id);
    if (i >= 0) mask |= 1 << i;
  }
  const fields = {
    version: REPLAY_VERSION,
    seed: u32(run.seed),
    size,
    difficulty: diff,
    kind,
    modifiers: mask,
    mindPlus: run.mindPlus ? 1 : 0,
  };
  let value = 0;
  for (const [key, width] of Object.entries(BITS)) value += fields[key] * 2 ** OFFSET[key];
  const payload = base32(value, PAYLOAD_CHARS);
  const check = checksumChars(payload);
  return `MOC-${payload.slice(0, 5)}-${payload.slice(5)}-${check}`;
}

export function decodeShareCode(text) {
  const core = cleanCode(text);
  if (core.length !== PAYLOAD_CHARS + CHECK_CHARS) return null;
  const payload = core.slice(0, PAYLOAD_CHARS);
  const check = core.slice(PAYLOAD_CHARS);
  if (checksumChars(payload) !== check) return null;
  const value = fromBase32(payload);
  if (value == null) return null;
  const field = (key) => Math.floor(value / 2 ** OFFSET[key]) % 2 ** BITS[key];
  if (field('version') !== REPLAY_VERSION) return null;
  const size = SIZE_IDS[field('size')];
  const difficulty = DIFFICULTIES[field('difficulty')];
  const challenge = KIND_NAMES[field('kind')];
  if (!size || !difficulty || !challenge) return null;
  const mask = field('modifiers');
  const modifiers = MODIFIER_IDS.filter((_, i) => mask & (1 << i));
  if ((mask | 0) !== mask) return null;
  return {
    seed: field('seed') >>> 0,
    worldSize: size,
    difficulty,
    challenge,
    modifiers,
    mindPlus: field('mindPlus') === 1,
    shareCode: encodeShareCode({ seed: field('seed'), worldSize: size, difficulty, challenge, modifiers, mindPlus: field('mindPlus') === 1 }),
  };
}

// `{ ok, code?, run?, error? }` for the paste box and tests.
export function validateShareCode(text) {
  const raw = String(text ?? '').trim();
  if (!raw) return { ok: false, error: 'Enter a share code.' };
  if (!/^MOC[\s-]/i.test(raw)) return { ok: false, error: 'A share code starts with MOC-.' };
  const run = decodeShareCode(raw);
  if (!run) return { ok: false, error: 'That share code is not valid: check the letters and groups.' };
  return { ok: true, code: run.shareCode, run };
}

// ---------- query string and settings ----------
export function parseReplaySearch(search = '') {
  const q = new URLSearchParams(search);
  const codeText = q.get('code') || q.get('share');
  const seedText = q.get('seed');
  const code = codeText ? decodeShareCode(codeText) : (seedText ? decodeShareCode(seedText) : null);
  const parsedSeed = !code && seedText ? parseSeedText(seedText) : null;
  const challenge = q.get('daily') === '1' || q.get('daily') === 'true' ? 'daily'
    : q.get('weekly') === '1' || q.get('weekly') === 'true' ? 'weekly' : null;
  const mods = q.get('mods') ? q.get('mods').split(',') : null;
  const mindPlusFlag = q.get('mindplus') === '1' || q.get('mindplus') === 'true' || q.get('newmindplus') === '1';
  return {
    code,
    seed: code ? code.seed : parsedSeed?.seed ?? null,
    worldSize: code?.worldSize ?? (SIZE_IDS.includes(q.get('size')) ? q.get('size') : null),
    difficulty: code?.difficulty ?? (DIFFICULTIES.includes(q.get('difficulty')) ? q.get('difficulty') : null),
    challenge: code?.challenge ?? challenge,
    modifiers: code?.modifiers ?? (mods ? normalizeModifiers(mods) : null),
    mindPlus: code ? code.mindPlus : (q.has('mindplus') || q.has('newmindplus') ? mindPlusFlag : undefined),
  };
}

const safeStorage = (storage) => {
  try { return storage ?? globalThis.localStorage ?? null; } catch { return null; }
};

export function readReplaySettings({ storage = globalThis.localStorage } = {}) {
  const s = safeStorage(storage);
  try {
    const raw = s?.getItem?.(SETTINGS_KEY);
    const parsed = raw ? JSON.parse(raw) : null;
    if (!parsed || typeof parsed !== 'object') return null;
    return {
      seed: finiteSeed(parsed.seed) ? u32(parsed.seed) : null,
      worldSize: SIZE_IDS.includes(parsed.worldSize) ? parsed.worldSize : null,
      difficulty: DIFFICULTIES.includes(parsed.difficulty) ? parsed.difficulty : null,
      challenge: KIND_INDEX[parsed.challenge] != null ? parsed.challenge : 'none',
      challengeKey: typeof parsed.challengeKey === 'string' ? parsed.challengeKey : '',
      modifiers: normalizeModifiers(parsed.modifiers),
      mindPlus: !!parsed.mindPlus,
    };
  } catch { return null; }
}

export function saveReplaySettings(settings, { storage = globalThis.localStorage } = {}) {
  const s = safeStorage(storage);
  try {
    s?.setItem?.(SETTINGS_KEY, JSON.stringify({
      seed: finiteSeed(settings?.seed) ? u32(settings.seed) : null,
      worldSize: SIZE_IDS.includes(settings?.worldSize) ? settings.worldSize : null,
      difficulty: DIFFICULTIES.includes(settings?.difficulty) ? settings.difficulty : null,
      challenge: KIND_INDEX[settings?.challenge] != null ? settings.challenge : 'none',
      challengeKey: String(settings?.challengeKey ?? ''),
      modifiers: normalizeModifiers(settings?.modifiers),
      mindPlus: !!settings?.mindPlus,
    }));
    return true;
  } catch { return false; }
}

// ---------- New Mind+ ----------
export function nextDifficulty(id) {
  const i = Math.max(0, DIFFICULTIES.indexOf(id));
  return DIFFICULTIES[Math.min(DIFFICULTIES.length - 1, i + 1)];
}

export function mindPlusUnlocked(progress) {
  const p = normalizeProgress(progress);
  return p.endings > 0 || p.unlocked.includes('new-mind-plus');
}

// The resolved run: seed, options, challenge, modifiers and the effective difficulty.
// `resume` keeps the save's own run. `requested` is the New Game UI. The query string wins over
// both so a pasted ?seed= / ?code= always starts the shared run.
export function resolveRunConfig({
  options = {},
  resume = null,
  requested = null,
  search = '',
  now = Date.now(),
  progress = null,
} = {}) {
  if (resume) {
    const rr = resume.replay || {};
    const base = {
      seed: finiteSeed(resume.seed) ? u32(resume.seed) : (finiteSeed(rr.seed) ? u32(rr.seed) : randomSeed(now)),
      worldSize: SIZE_IDS.includes(rr.worldSize) ? rr.worldSize : (SIZE_IDS.includes(resume.options?.worldSize) ? resume.options.worldSize : (SIZE_IDS.includes(options.worldSize) ? options.worldSize : 'medium')),
      difficulty: DIFFICULTIES.includes(rr.difficulty) ? rr.difficulty : (DIFFICULTIES.includes(resume.options?.difficulty) ? resume.options.difficulty : (DIFFICULTIES.includes(options.difficulty) ? options.difficulty : 'normal')),
      challenge: KIND_INDEX[rr.challenge] != null ? rr.challenge : 'none',
      challengeKey: String(rr.challengeKey || ''),
      modifiers: normalizeModifiers(rr.modifiers),
      mindPlus: !!rr.mindPlus,
    };
    return finishConfig(base, now);
  }

  const query = parseReplaySearch(search);
  const req = requested && typeof requested === 'object' ? requested : {};
  const challenge = query.challenge || req.challenge || 'none';
  const fromChallenge = challenge === 'daily' || challenge === 'weekly' ? challengeFor(challenge, now) : null;
  let seed = query.seed;
  if (seed == null) seed = finiteSeed(req.seed) ? u32(req.seed) : null;
  if (seed == null) seed = fromChallenge?.seed ?? randomSeed(now);

  const worldSize = query.worldSize || req.worldSize || options.worldSize || 'medium';
  const difficulty = query.difficulty || req.difficulty || options.difficulty || 'normal';
  const modifiers = query.modifiers ?? req.modifiers ?? [];
  let mindPlus = query.mindPlus ?? !!req.mindPlus;
  if (progress != null && !mindPlusUnlocked(progress)) mindPlus = false;

  return finishConfig({
    seed,
    worldSize: SIZE_IDS.includes(worldSize) ? worldSize : 'medium',
    difficulty: DIFFICULTIES.includes(difficulty) ? difficulty : 'normal',
    challenge,
    challengeKey: fromChallenge?.key ?? String(req.challengeKey || ''),
    modifiers,
    mindPlus,
  }, now);
}

function finishConfig(base, now) {
  const baseDifficulty = DIFFICULTIES.includes(base.difficulty) ? base.difficulty : 'normal';
  const difficulty = base.mindPlus ? nextDifficulty(baseDifficulty) : baseDifficulty;
  const config = {
    seed: u32(base.seed),
    worldSize: SIZE_IDS.includes(base.worldSize) ? base.worldSize : 'medium',
    difficulty,
    baseDifficulty,
    challenge: KIND_INDEX[base.challenge] != null ? base.challenge : 'none',
    challengeKey: String(base.challengeKey || (base.challenge !== 'none' ? challengeKey(base.challenge, now) : '')),
    modifiers: normalizeModifiers(base.modifiers),
    mindPlus: !!base.mindPlus,
  };
  config.shareCode = encodeShareCode({
    seed: config.seed,
    worldSize: config.worldSize,
    difficulty: config.baseDifficulty,
    challenge: config.challenge,
    modifiers: config.modifiers,
    mindPlus: config.mindPlus,
  });
  config.label = runLabel(config);
  return config;
}

export function runLabel(replay) {
  const seed = (u32(replay?.seed)).toString(16).toUpperCase().padStart(8, '0');
  const bits = [`Seed ${seed}`];
  if (replay?.challenge === 'daily') bits.push('Daily');
  else if (replay?.challenge === 'weekly') bits.push('Weekly');
  const mods = normalizeModifiers(replay?.modifiers);
  if (mods.length) bits.push(mods.map((id) => MODIFIER_BY_ID.get(id)?.short || id).join(', '));
  if (replay?.mindPlus) bits.push('New Mind+');
  return bits.join(' · ');
}

// ---------- scores ----------
export const SCORE_CATEGORIES = [
  { id: 'survival', label: 'Survival', blurb: 'How long the run lasted, and what it cost.' },
  { id: 'prosperity', label: 'Prosperity', blurb: 'People, children and the world left behind.' },
  { id: 'peace', label: 'Peace', blurb: 'Few deaths, few recessions, few restarts.' },
  { id: 'chaos', label: 'Chaos', blurb: 'Lightning, crashes and divine mayhem.' },
  { id: 'speed', label: 'Speed', blurb: 'How quickly the ages fell.' },
  { id: 'style', label: 'Style', blurb: 'Modifiers, clean runs and New Mind+.' },
];

export const RANKS = [
  { rank: 'S', min: 85, label: 'Sacred Chicken' },
  { rank: 'A', min: 70, label: 'Feathered Legend' },
  { rank: 'B', min: 55, label: 'Coop Keeper' },
  { rank: 'C', min: 40, label: 'Humble Herder' },
  { rank: 'D', min: 25, label: 'Egg Apprentice' },
  { rank: 'E', min: 0, label: 'A Pebble' },
];

export function scoreRun(summary = {}) {
  const stats = summary.stats || {};
  const extra = summary.extra || {};
  const seconds = Math.max(0, num(summary.runTime));
  const deaths = Math.max(0, num(stats.deaths));
  const lightning = Math.max(0, num(stats.lightning));
  const recessions = Math.max(0, num(stats.recessions));
  const restarts = Math.max(0, num(stats.restarts));
  const births = Math.max(0, num(stats.births));
  const colonies = Math.max(0, num(stats.colonies));
  const passengers = Math.max(0, num(extra.passengers));
  const smitten = Math.max(0, num(stats.smitten));
  const eras = Array.isArray(summary.eraTimes) ? summary.eraTimes.length : 0;
  const mods = normalizeModifiers(summary.replay?.modifiers);
  const modifierStyle = mods.reduce((sum, id) => sum + (MODIFIER_BY_ID.get(id)?.score || 8), 0);
  const finished = !!summary.finished;

  const scores = {
    // 900 s is a strong full run; deaths cost a little.
    survival: clamp100(seconds / 9 - deaths * 2),
    prosperity: clamp100(births * 2 + colonies * 6 + passengers * 4),
    peace: clamp100(100 - deaths * 4 - recessions * 4 - restarts * 3 - smitten * 1),
    chaos: clamp100(lightning * 1.5 + recessions * 8 + restarts * 5 + smitten * 2),
    speed: finished ? clamp100(110 - seconds / 10) : 0,
    style: clamp100(modifierStyle + (restarts === 0 ? 15 : 0) + Math.min(20, eras * 4) + (summary.replay?.mindPlus ? 12 : 0)),
  };
  const total = clamp100(SCORE_CATEGORIES.reduce((sum, c) => sum + scores[c.id], 0) / SCORE_CATEGORIES.length);
  const rank = RANKS.find((r) => total >= r.min) || RANKS[RANKS.length - 1];
  return {
    categories: SCORE_CATEGORIES.map((c) => ({ ...c, score: scores[c.id], max: 100 })),
    categoryMap: scores,
    total,
    rank: rank.rank,
    rankLabel: rank.label,
  };
}

export function summarizeRun(G) {
  return {
    id: G?.run?.id ?? null,
    finished: !!G?.finished,
    runTime: num(G?.runTime),
    stats: { ...(G?.stats || {}) },
    extra: { ...(G?.extra || {}) },
    eraTimes: Array.isArray(G?.eraTimes) ? G.eraTimes.map((e) => ({ label: e?.label, seconds: num(e?.seconds) })) : [],
    replay: G?.replay ? { ...G.replay, modifiers: [...(G.replay.modifiers || [])] } : null,
    brain: {
      calmSeconds: num(G?.brain?.stats?.calmSeconds),
      focusSeconds: num(G?.brain?.stats?.focusSeconds),
      blinks: num(G?.brain?.stats?.blinks),
    },
  };
}

// ---------- chronicle ----------
// Only supplied facts are used: advisor/news lines from the run and numbers derived from its real
// stats. A short run still gets a chronicle once the caller passes enough real events.
export function chronicleFacts(summary = {}, result = null) {
  const stats = summary.stats || {};
  const extra = summary.extra || {};
  const out = [];
  const t = Math.floor(Math.max(0, num(summary.runTime)));
  if (t > 0) out.push(`The run lasted ${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}.`);
  const eras = Array.isArray(summary.eraTimes) ? summary.eraTimes.filter((e) => e?.seconds > 0) : [];
  if (eras.length) out.push(`${eras.length} ${eras.length === 1 ? 'age' : 'ages'} passed: ${eras.map((e) => String(e.label || '').replace(/^.* · /, '')).filter(Boolean).join(', ')}.`);
  if (num(stats.lightning) > 0) out.push(`${Math.floor(num(stats.lightning))} lightning bolts fell.`);
  if (num(stats.births) > 0) out.push(`${Math.floor(num(stats.births))} children were born.`);
  if (num(stats.deaths) > 0) out.push(`${Math.floor(num(stats.deaths))} villagers were lost.`);
  if (num(stats.recessions) > 0) out.push(`${Math.floor(num(stats.recessions))} chicken recessions shook the exchange.`);
  if (num(extra.passengers) > 0) out.push(`${Math.floor(num(extra.passengers))} passengers and one chicken reached the new world.`);
  const mods = normalizeModifiers(summary.replay?.modifiers);
  if (mods.length) out.push(`The run carried the ${mods.map((id) => MODIFIER_BY_ID.get(id)?.short || id).join(', ')} modifier${mods.length === 1 ? '' : 's'}.`);
  if (summary.replay?.mindPlus) out.push('This was a New Mind+ run: one difficulty harder.');
  if (result?.rank) out.push(`The chroniclers rank the run ${result.rank} (${result.rankLabel}).`);
  return out;
}

export function buildChronicle({ events = [], facts = [], max = 8 } = {}) {
  const raw = [
    ...(Array.isArray(events) ? events : []).map((e) => (typeof e === 'string' ? e : e?.text)),
    ...(Array.isArray(facts) ? facts : []),
  ];
  const seen = new Set();
  const lines = [];
  for (const line of raw) {
    const text = String(line ?? '').replace(/\s+/g, ' ').trim();
    if (!text || seen.has(text)) continue;
    seen.add(text);
    lines.push(text);
  }
  if (lines.length <= max) return lines;
  // Evenly spaced through the run, always keeping the first and the last: deterministic.
  const picked = [];
  for (let i = 0; i < max; i++) picked.push(lines[Math.round((i * (lines.length - 1)) / (max - 1))]);
  return [...new Set(picked)];
}

// ---------- unlocks ----------
export const UNLOCK_DEFS = [
  { id: 'new-mind-plus', kind: 'feature', label: 'New Mind+', hint: 'Finish your first run.', test: (s) => s.finished },
  { id: 'trait-firekeeper', kind: 'trait', label: 'Civ trait: Firekeeper', hint: 'Finish a run.', test: (s) => s.finished },
  { id: 'trait-steady-hand', kind: 'trait', label: 'Civ trait: Steady Hand', hint: 'Finish with 2 or fewer restarts.', test: (s) => s.finished && num(s.stats?.restarts) <= 2 },
  { id: 'power-stormcaller', kind: 'power', label: 'Power: Stormcaller', hint: '20 lightning bolts in one run.', test: (s) => num(s.stats?.lightning) >= 20 },
  { id: 'power-peacemaker', kind: 'power', label: 'Power: Peacemaker', hint: 'A peace score of 75 or more.', test: (s) => num(s.scores?.peace) >= 75 },
  { id: 'biome-jungle', kind: 'biome', label: 'Biome: Living Jungle', hint: 'A peace score of 65 or more.', test: (s) => num(s.scores?.peace) >= 65 },
  { id: 'biome-void', kind: 'biome', label: 'Biome: Voidglass', hint: 'A chaos score of 70 or more.', test: (s) => num(s.scores?.chaos) >= 70 },
  { id: 'advisor-prophet', kind: 'advisor-line', label: 'Advisor: Prophet Cluck', hint: 'Finish 3 runs.', test: (s) => num(s.endings) >= 3 },
  { id: 'advisor-economist', kind: 'advisor-line', label: 'Advisor: Egg Economist', hint: 'Cause a chicken recession.', test: (s) => num(s.stats?.recessions) >= 1 },
  { id: 'ending-sacred-egg', kind: 'chicken-ending', label: 'Ending: The Sacred Egg', hint: 'Land 8 or more passengers.', test: (s) => num(s.extra?.passengers) >= 8 },
  { id: 'ending-feathered-stars', kind: 'chicken-ending', label: 'Ending: Feathered Stars', hint: 'Reach an S rank.', test: (s) => s.rank === 'S' },
];
const UNLOCK_BY_ID = new Map(UNLOCK_DEFS.map((u) => [u.id, u]));

export function emptyProgress() {
  return { version: 1, runs: 0, endings: 0, unlocked: [], seen: [] };
}

export function normalizeProgress(progress) {
  const p = progress && typeof progress === 'object' ? progress : {};
  const unlocked = Array.isArray(p.unlocked) ? [...new Set(p.unlocked.filter((id) => UNLOCK_BY_ID.has(id)))] : [];
  return {
    version: 1,
    runs: Math.max(0, Math.floor(num(p.runs))),
    endings: Math.max(0, Math.floor(num(p.endings))),
    unlocked,
    seen: Array.isArray(p.seen) ? p.seen.slice(-20).map(String) : [],
  };
}

export function readUnlocks({ storage = globalThis.localStorage } = {}) {
  const s = safeStorage(storage);
  try {
    const raw = s?.getItem?.(UNLOCKS_KEY);
    return normalizeProgress(raw ? JSON.parse(raw) : null);
  } catch { return emptyProgress(); }
}

export function saveUnlocks(progress, { storage = globalThis.localStorage } = {}) {
  const s = safeStorage(storage);
  try {
    s?.setItem?.(UNLOCKS_KEY, JSON.stringify(normalizeProgress(progress)));
    return true;
  } catch { return false; }
}

// Apply a finished run once (pass runId so repeat calls cannot count it twice).
export function applyRunToUnlocks(summary = {}, { storage = globalThis.localStorage, runId = null } = {}) {
  const progress = readUnlocks({ storage });
  const id = runId ?? summary.id ?? null;
  if (id && progress.seen.includes(String(id))) {
    return { newUnlocks: [], progress, alreadyApplied: true };
  }
  if (id) progress.seen = [...progress.seen, String(id)].slice(-20);
  progress.runs += 1;
  if (summary.finished) progress.endings += 1;
  const ctx = {
    ...summary,
    scores: summary.scores || {},
    endings: progress.endings,
    runs: progress.runs,
  };
  const before = new Set(progress.unlocked);
  const newUnlocks = [];
  for (const def of UNLOCK_DEFS) {
    if (before.has(def.id) || !def.test || !def.test(ctx)) continue;
    progress.unlocked.push(def.id);
    newUnlocks.push(def);
  }
  saveUnlocks(progress, { storage });
  return { newUnlocks, progress, alreadyApplied: false };
}

export function unlockLabels(progress) {
  const p = normalizeProgress(progress);
  return p.unlocked.map((id) => UNLOCK_BY_ID.get(id)).filter(Boolean);
}

export function unlockListText(progress, { max = 4 } = {}) {
  const list = unlockLabels(progress);
  if (!list.length) return 'No unlocks yet: finish a run to begin.';
  const shown = list.slice(0, max).map((u) => u.label);
  return `Unlocked: ${shown.join(' · ')}${list.length > shown.length ? ` · +${list.length - shown.length}` : ''}`;
}
