// What a save holds (src/game/saves.js), taken from the running game. Pure: main.js passes in G
// and the run, and gets back a plain JSON-safe record. A checkpoint is taken when a scene starts,
// so "continue" means: the same world (seed and size), the same era, the stats so far.

// "Era I · Stone Age" → "Stone Age", "Sandbox · A crowded planet" → "A crowded planet".
export function shortLabel(label) {
  const s = String(label ?? '');
  const i = s.indexOf(' · ');
  return i >= 0 ? s.slice(i + 3) : s;
}

const finite = (v) => (typeof v === 'number' && Number.isFinite(v));

// The run stats as plain numbers (lightning, smitten, births, ...).
export function statsSummary(stats) {
  const out = {};
  for (const [k, v] of Object.entries(stats || {})) if (finite(v)) out[k] = v;
  return out;
}

// The calibration window (#54): its config seed always, and its result once the window closed.
export function calibrationSummary(cal) {
  if (!cal) return null;
  const c = cal.config || {};
  return {
    config: { seconds: c.seconds ?? null, mode: c.mode ?? null, seed: finite(c.seed) ? c.seed : null },
    result: cal.result ? JSON.parse(JSON.stringify(cal.result)) : null,
  };
}

// The galaxy's worlds: home plus those born from the player's mind, by name and seed.
export function galaxySummary(galaxy) {
  const planets = Array.isArray(galaxy?.planets) ? galaxy.planets : [];
  return planets.map((p) => ({
    name: String(p.name ?? ''), seed: finite(p.seed) ? p.seed : null,
    channel: p.channel ?? null, biome: p.biome ?? null, home: !!p.home, forming: !!p.forming,
  }));
}

// The personal brain model (src/brain/model.js): its trained weights only, a few hundred
// numbers. The raw samples stay behind; a restored model predicts and keeps learning.
export function brainSnapshot(model) {
  if (!model?.trained || !Array.isArray(model.W)) return null;
  try {
    return JSON.parse(JSON.stringify({
      trained: true, classes: model.classes, W: model.W, b: model.b, mu: model.mu, sd: model.sd,
      accuracy: model.accuracy, chance: model.chance, trainCount: model.trainCount,
    }));
  } catch { return null; }
}

export function restoreBrain(model, snap) {
  if (!model || !snap?.trained || !Array.isArray(snap.W) || !Array.isArray(snap.classes)) return false;
  Object.assign(model, {
    trained: true, classes: snap.classes, W: snap.W, b: snap.b, mu: snap.mu, sd: snap.sd,
    accuracy: snap.accuracy ?? 0, chance: snap.chance ?? 0, trainCount: snap.trainCount ?? 1,
  });
  return true;
}

// The record saveRun() stores for this run right now.
//   run: { id, runNumber, mode: 'story' | 'galaxy' | 'crowded', createdAt?, calibration? }
//   extra: mode-specific fields (e.g. civSeed and planet for the crowded planet)
export function buildCheckpoint(G, run, extra = {}) {
  const scene = G?.scene;
  const sceneLabel = extra.sceneLabel ?? scene?.label ?? '';
  const world = G?.world;
  const calibration = calibrationSummary(G?.calibration);
  // A resumed run keeps the calibration result of its first window until a new one closes.
  if (calibration && !calibration.result && run?.calibration?.result) calibration.result = run.calibration.result;
  return {
    id: run.id,
    runNumber: run.runNumber,
    name: `Run ${run.runNumber} · ${shortLabel(sceneLabel) || 'Run'}`,
    mode: run.mode || 'story',
    sceneIndex: run.mode && run.mode !== 'story' ? 0 : (G?.sceneIndex ?? 0),
    sceneLabel,
    era: extra.era ?? scene?.era ?? null,
    seed: world?.seed ?? null,
    worldSize: world ? { width: world.width, height: world.height } : null,
    stats: statsSummary(G?.stats),
    runTime: finite(G?.runTime) ? Math.round(G.runTime * 10) / 10 : 0,
    eraTimes: Array.isArray(G?.eraTimes) ? G.eraTimes.map((e) => ({ label: e.label, seconds: Math.round((e.seconds || 0) * 10) / 10 })) : [],
    calibration,
    galaxy: galaxySummary(G?.camera?.galaxy),
    brain: brainSnapshot(G?.brain?.model),
    ...extra,
  };
}
