// The one game's New Game options (epic #220): world size and difficulty. DOM-free: storage and
// the query string are passed in, so tests (and the save system, #314) can use it as is.
//
// World size sets the home world's map (the view stays 320×180, panned by the camera) and how many
// rival peoples share it (#212). Difficulty is one table every system reads: the eras, the rivals
// and the space wars. Normal is today's game, number for number.

// Smallest to largest. Medium is #139's 640×360.
export const WORLD_SIZES = [
  { id: 'small', label: 'Small', w: 480, h: 270, rivals: 3 },
  { id: 'medium', label: 'Medium', w: 640, h: 360, rivals: 5 },
  { id: 'large', label: 'Large', w: 960, h: 540, rivals: 8 },
  { id: 'huge', label: 'Huge', w: 1280, h: 720, rivals: 12 },
];
export const SIZE_CHOICES = [...WORLD_SIZES.map((s) => s.id), 'random'];

export const DIFFICULTIES = ['easy', 'normal', 'hard'];
export const DIFFICULTY = {
  easy: {
    label: 'Easy',
    eras: {
      starvationGrace: 1.6,   // × the Stone Age's grace before hunger bites
      hunger: 0.6,            // × how fast people get hungry
      fireDecay: 0.6,         // × how fast the fire fades
      holdSeconds: 4,         // calm held this long feeds the fire
      calmThreshold: 0.45,    // the calm a hold needs (before calibration moves it)
      mammothDanger: 0.6,     // × giant chicken (mammoth) harm
      crashThreshold: 0.72,   // the goat index crashes below this share of its peak
      launchNeeds: 0.8,       // × the work a rocket needs
      hints: 1.5,             // × how often the advisor hints
    },
    rivals: { offset: -1, aggression: -0.15, raids: 0.5, damage: 0.5, growth: 0.85 },
    space: { pressure: 0.5 },
  },
  normal: {
    label: 'Normal',
    eras: { starvationGrace: 1, hunger: 1, fireDecay: 1, holdSeconds: 5, calmThreshold: 0.55, mammothDanger: 1, crashThreshold: 0.8, launchNeeds: 1, hints: 1 },
    rivals: { offset: 0, aggression: 0, raids: 1, damage: 1, growth: 1 },
    space: { pressure: 1 },
  },
  hard: {
    label: 'Hard',
    eras: { starvationGrace: 0.6, hunger: 1.35, fireDecay: 1.4, holdSeconds: 6, calmThreshold: 0.62, mammothDanger: 1.5, crashThreshold: 0.88, launchNeeds: 1.25, hints: 0.5 },
    rivals: { offset: 2, aggression: 0.25, raids: 1.6, damage: 1.5, growth: 1.2 },
    space: { pressure: 1.6 },
  },
};
// Which way each number moves as the game gets harder (+1 up, −1 down). Tests hold the table to it.
export const HARDER = {
  eras: { starvationGrace: -1, hunger: 1, fireDecay: 1, holdSeconds: 1, calmThreshold: 1, mammothDanger: 1, crashThreshold: 1, launchNeeds: 1, hints: -1 },
  rivals: { offset: 1, aggression: 1, raids: 1, damage: 1, growth: 1 },
  space: { pressure: 1 },
};

export const DEFAULT_OPTIONS = { worldSize: 'medium', difficulty: 'normal' };
const KEYS = { worldSize: 'moc.worldSize', difficulty: 'moc.difficulty' };

const get = (storage, k) => { try { return storage?.getItem?.(k) ?? null; } catch { return null; } };
const set = (storage, k, v) => { try { storage?.setItem?.(k, v); } catch { /* private mode: not kept */ } };

// The chosen options: a valid ?size= / ?difficulty= wins over what was stored; else the default.
export function readOptions({ storage = globalThis.localStorage, search = globalThis.location?.search ?? '' } = {}) {
  const q = new URLSearchParams(search);
  const first = (ok, ...vals) => vals.find((v) => ok.includes(v));
  return {
    worldSize: first(SIZE_CHOICES, q.get('size'), get(storage, KEYS.worldSize)) ?? DEFAULT_OPTIONS.worldSize,
    difficulty: first(DIFFICULTIES, q.get('difficulty'), get(storage, KEYS.difficulty)) ?? DEFAULT_OPTIONS.difficulty,
  };
}

export function saveOptions(opts, { storage = globalThis.localStorage } = {}) {
  if (SIZE_CHOICES.includes(opts?.worldSize)) set(storage, KEYS.worldSize, opts.worldSize);
  if (DIFFICULTIES.includes(opts?.difficulty)) set(storage, KEYS.difficulty, opts.difficulty);
}

// The concrete world size for a choice ('random' picks one from the seed).
export function worldSizeOf(choice, seed = 0) {
  if (choice === 'random') return WORLD_SIZES[(seed >>> 0) % WORLD_SIZES.length];
  return WORLD_SIZES.find((s) => s.id === choice) || WORLD_SIZES[1];
}

export function difficultyOf(id) { return DIFFICULTY[id] || DIFFICULTY.normal; }

// What an era reads: G.difficulty's era numbers, or Normal's when a run has none (tests, old saves).
export function eraTuning(G) { return (G?.difficulty || DIFFICULTY.normal).eras; }
