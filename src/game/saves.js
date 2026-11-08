// Local game saves: several runs kept side by side in localStorage, so a demo can jump back to
// any era of any run. Pure and DOM-free (main.js hooks it up; the title screen lists it).
//
// One key holds every save, newest first:
//   moc.saves.v1 = [{ id, name, mode, sceneIndex, sceneLabel, era, seed, worldSize, stats, runTime,
//                     eraTimes, calibration, galaxy, brain, runNumber, createdAt, updatedAt }, ...]
// mode is 'story' (the eras), 'galaxy' or 'crowded' (the sandboxes); seed is the home world's;
// runTime is seconds played. src/game/checkpoint.js builds these from the running game.
// Every storage access is wrapped: a private window, a full quota or a corrupt value never
// throws into the game, it just behaves as "no saves".

export const SAVES_KEY = 'moc.saves.v1';
export const MAX_SAVES = 12;

function safeStorage() { try { return globalThis.localStorage ?? null; } catch { return null; } }

const isSave = (s) => !!s && typeof s === 'object' && typeof s.id === 'string' && s.id.length > 0;
const num = (v, d = 0) => (Number.isFinite(Number(v)) ? Number(v) : d);

function readAll(storage) {
  try {
    const raw = storage?.getItem(SAVES_KEY);
    if (!raw) return [];
    const list = JSON.parse(raw);
    return Array.isArray(list) ? list.filter(isSave) : [];
  } catch { return []; }
}

function writeAll(list, storage) {
  if (!storage) return false;
  try { storage.setItem(SAVES_KEY, JSON.stringify(list)); return true; } catch { return false; }
}

const newestFirst = (list) => list
  .map((s, i) => [s, i])
  .sort((a, b) => num(b[0].updatedAt) - num(a[0].updatedAt) || a[1] - b[1])
  .map(([s]) => s);

// Every save, newest first (by updatedAt; ties keep the most recently written first).
export function listSaves(storage = safeStorage()) {
  return newestFirst(readAll(storage));
}

export function getSave(id, storage = safeStorage()) {
  return readAll(storage).find((s) => s.id === id) ?? null;
}

// The run to offer as "Continue": the newest save, or null.
export function latestSave(storage = safeStorage()) {
  return listSaves(storage)[0] ?? null;
}

// Upsert by id. createdAt is kept from the first save of that id; updatedAt is now (or the
// run's own updatedAt when given, which tests use). Keeps at most MAX_SAVES, dropping the oldest.
// Returns the stored record, or null when nothing could be written.
export function saveRun(run, storage = safeStorage(), { max = MAX_SAVES } = {}) {
  if (!run || typeof run !== 'object') return null;
  const id = typeof run.id === 'string' && run.id ? run.id : newRunId();
  const all = readAll(storage);
  const prev = all.find((s) => s.id === id);
  const now = Date.now();
  const record = {
    ...(prev || {}),
    ...run,
    id,
    name: String(run.name ?? prev?.name ?? 'Run'),
    sceneIndex: Math.max(0, Math.floor(num(run.sceneIndex ?? prev?.sceneIndex, 0))),
    createdAt: num(prev?.createdAt ?? run.createdAt, now) || now,
    updatedAt: num(run.updatedAt, now) || now,
  };
  let next = newestFirst([record, ...all.filter((s) => s.id !== id)]).slice(0, Math.max(1, max));
  // A full quota: shed the oldest saves until the newest one fits.
  while (!writeAll(next, storage)) {
    if (next.length <= 1) return null;
    next = next.slice(0, -1);
  }
  return record;
}

export function deleteSave(id, storage = safeStorage()) {
  const all = readAll(storage);
  const next = all.filter((s) => s.id !== id);
  if (next.length === all.length) return false;
  return writeAll(next, storage);
}

export function clearSaves(storage = safeStorage()) {
  try { storage?.removeItem?.(SAVES_KEY); return true; } catch { return false; }
}

// A unique-enough id for a new run: time plus randomness, e.g. "run-lx3k9a-4f2b".
export function newRunId(now = Date.now(), rand = Math.random) {
  const r = Math.floor(rand() * 0x100000).toString(36).padStart(4, '0');
  return `run-${Math.floor(now).toString(36)}-${r}`;
}

// The number for the next run's name ("Run 3"): one more than the highest number saved.
export function nextRunNumber(storage = safeStorage()) {
  return readAll(storage).reduce((m, s) => Math.max(m, Math.floor(num(s.runNumber, 0))), 0) + 1;
}

// "Run 3 · Stone Age"
export function runName(runNumber, sceneLabel) {
  const head = `Run ${Math.max(1, Math.floor(num(runNumber, 1)))}`;
  return sceneLabel ? `${head} · ${sceneLabel}` : head;
}
