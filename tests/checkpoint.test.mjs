// Local saves, game side (src/game/checkpoint.js + the hooks in src/main.js): what a checkpoint
// holds, that it survives the save list, and that main.js takes one at every era start.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildCheckpoint, shortLabel, brainSnapshot, restoreBrain, galaxySummary, calibrationSummary } from '../src/game/checkpoint.js';
import { saveRun, listSaves, nextRunNumber } from '../src/game/saves.js';
import { PersonalModel } from '../src/brain/model.js';

const memory = () => { const m = new Map(); return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) }; };

function fakeG(overrides = {}) {
  const model = new PersonalModel();
  Object.assign(model, { trained: true, classes: ['rest', 'calm'], W: [[0.1, 0.2], [0.3, 0.4]], b: [0, 1], mu: [1, 2], sd: [1, 1], accuracy: 0.8, chance: 0.5, trainCount: 3 });
  return {
    world: { seed: 123456, width: 640, height: 360, h: new Float32Array(4) },
    scene: { label: 'Era II · Industry', era: 'industry', G: {} },
    sceneIndex: 2,
    runTime: 95.37,
    stats: { lightning: 4, smitten: 1, births: 30, deaths: 2, recessions: 0, restarts: 1, junk: 'x' },
    eraTimes: [{ label: 'Era I · Stone Age', seconds: 60.04 }, { label: 'Time skip · Farming → Kingdoms', seconds: 12 }],
    calibration: { config: { seconds: 10, mode: 'random', seed: 777, seedFromConfig: false }, result: { calm: 0.4, source: 'random', seed: 777 } },
    camera: { galaxy: { planets: [{ name: 'Home', seed: 123456, home: true, biome: 'temperate', channel: 'mixed', world: { big: true } }, { name: 'Stillwater', seed: 42, channel: 'calm', biome: 'temperate' }] } },
    brain: { model },
    ...overrides,
  };
}

test('a checkpoint has the save shape: world, era, stats, run time, calibration, galaxy, brain', () => {
  const cp = buildCheckpoint(fakeG(), { id: 'run-x', runNumber: 3, mode: 'story' });
  assert.equal(cp.id, 'run-x');
  assert.equal(cp.name, 'Run 3 · Industry');
  assert.equal(cp.mode, 'story');
  assert.equal(cp.sceneIndex, 2);
  assert.equal(cp.sceneLabel, 'Era II · Industry');
  assert.equal(cp.era, 'industry');
  assert.equal(cp.seed, 123456);
  assert.deepEqual(cp.worldSize, { width: 640, height: 360 });
  assert.deepEqual(cp.stats, { lightning: 4, smitten: 1, births: 30, deaths: 2, recessions: 0, restarts: 1 });
  assert.equal(cp.runTime, 95.4);
  assert.deepEqual(cp.eraTimes.map((e) => e.seconds), [60, 12]);
  assert.deepEqual(cp.calibration.config, { seconds: 10, mode: 'random', seed: 777 });
  assert.equal(cp.calibration.result.seed, 777);
  assert.deepEqual(cp.galaxy.map((p) => p.name), ['Home', 'Stillwater']);
  assert.equal(cp.galaxy[0].home, true);
  assert.equal(cp.galaxy[1].seed, 42);
  assert.deepEqual(cp.brain.classes, ['rest', 'calm']);
  // Plain JSON, and it survives the save list as is.
  const s = memory();
  saveRun(cp, s);
  const back = listSaves(s)[0];
  for (const k of ['name', 'sceneIndex', 'seed', 'worldSize', 'stats', 'runTime', 'calibration', 'galaxy', 'brain']) assert.deepEqual(back[k], JSON.parse(JSON.stringify(cp[k])), k);
  assert.equal(nextRunNumber(s), 4);
});

test('sandbox checkpoints carry their mode and seed, and sit at scene 0', () => {
  const G = fakeG({ scene: { label: 'Sandbox · A crowded planet', era: 'kingdoms' } });
  const civ = buildCheckpoint(G, { id: 'c', runNumber: 1, mode: 'crowded' }, { civSeed: 99, planet: 'large' });
  assert.equal(civ.mode, 'crowded');
  assert.equal(civ.sceneIndex, 0);
  assert.equal(civ.civSeed, 99);
  assert.equal(civ.planet, 'large');
  assert.equal(civ.name, 'Run 1 · A crowded planet');
  const gal = buildCheckpoint(fakeG(), { id: 'g', runNumber: 2, mode: 'galaxy' }, { sceneLabel: 'Sandbox · Galaxy', era: 'space' });
  assert.equal(gal.name, 'Run 2 · Galaxy');
  assert.equal(gal.sceneLabel, 'Sandbox · Galaxy');
  assert.equal(gal.era, 'space');
  assert.equal(gal.seed, 123456);
});

test('a resumed run keeps its first calibration result until a new window closes', () => {
  const G = fakeG({ calibration: { config: { seconds: 10, mode: 'random', seed: 777 }, result: null } });
  const cp = buildCheckpoint(G, { id: 'r', runNumber: 1, mode: 'story', calibration: { result: { calm: 0.4, seed: 777 } } });
  assert.deepEqual(cp.calibration.result, { calm: 0.4, seed: 777 });
  assert.equal(calibrationSummary(null), null);
});

test('the brain model round-trips its weights; an untrained model saves nothing', () => {
  assert.equal(brainSnapshot(new PersonalModel()), null);
  const snap = brainSnapshot(fakeG().brain.model);
  const fresh = new PersonalModel();
  assert.equal(restoreBrain(fresh, JSON.parse(JSON.stringify(snap))), true);
  assert.equal(fresh.trained, true);
  assert.deepEqual(fresh.W, [[0.1, 0.2], [0.3, 0.4]]);
  assert.equal(fresh.has('calm'), true);
  assert.equal(restoreBrain(new PersonalModel(), null), false);
});

test('labels and galaxy summaries', () => {
  assert.equal(shortLabel('Era I · Stone Age'), 'Stone Age');
  assert.equal(shortLabel('Time skip · Farming → Kingdoms'), 'Farming → Kingdoms');
  assert.equal(shortLabel('Plain'), 'Plain');
  assert.deepEqual(galaxySummary(null), []);
});

test('main.js checkpoints every era start, resumes saves and exposes them (#local saves)', () => {
  const src = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
  const startScene = src.slice(src.indexOf('function startScene('), src.indexOf('function autosave('));
  assert.match(startScene, /G\.run\?\.mode === 'story'\) autosave\(\)/);
  const newGame = src.slice(src.indexOf('function newGame('), src.indexOf('function startScene('));
  // A resume keeps its world: its seed, its size and its New Game options (#220).
  assert.match(newGame, /const seed = Number\.isFinite\(resume\?\.seed\) \? resume\.seed >>> 0/);
  assert.match(newGame, /resume\?\.worldSize\?\.width \? \{ w: resume\.worldSize\.width, h: resume\.worldSize\.height/);
  assert.match(newGame, /\.\.\.\(resume\?\.options \|\| \{\}\)/);
  assert.match(newGame, /new World\(seed, \{ width: size\.w \|\| HOME_W, height: size\.h \|\| HOME_H \}\)/);
  assert.match(newGame, /G\.run = \{ id: resume\?\.id \|\| newRunId\(\)/);
  assert.match(newGame, /restoreBrain\(brain\.model, resume\.brain\)/);
  assert.match(src, /window\.__moc\.continueRun = /);
  assert.match(src, /window\.__moc\.saves = \{/);
  assert.match(src, /function startGalaxySandbox\(source, resume = null\)/);
  assert.match(src, /function startCivSandbox\(source, resume = null\)/);
  const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  const panel = html.slice(html.indexOf('id="settingsPanel"'), html.indexOf('</aside>', html.indexOf('id="settingsPanel"')));
  assert.match(panel, /id="setDeleteSaves"/);
});
