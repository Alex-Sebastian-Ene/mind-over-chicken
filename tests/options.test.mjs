// Epic #220: world size and difficulty for the one game's New Game flow.
import test from 'node:test';
import assert from 'node:assert/strict';
import { WORLD_SIZES, DIFFICULTY, DIFFICULTIES, HARDER, readOptions, saveOptions, worldSizeOf, difficultyOf, eraTuning } from '../src/game/options.js';

const memory = () => { const m = new Map(); return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, String(v)), m }; };

test('sizes grow, Medium is #139\'s 640×360, and rivals scale with the map', () => {
  assert.deepEqual(worldSizeOf('medium'), WORLD_SIZES[1]);
  assert.equal(worldSizeOf('medium').w, 640);
  for (let i = 1; i < WORLD_SIZES.length; i++) {
    assert.ok(WORLD_SIZES[i].w > WORLD_SIZES[i - 1].w && WORLD_SIZES[i].rivals > WORLD_SIZES[i - 1].rivals);
    assert.equal(WORLD_SIZES[i].w * 9, WORLD_SIZES[i].h * 16, 'every size keeps the 16:9 view shape');
  }
  assert.ok(WORLD_SIZES.includes(worldSizeOf('random', 12345)), 'random picks a real size');
  assert.equal(worldSizeOf('nonsense').id, 'medium');
});

test('the difficulty table is complete and every number moves the right way from Easy to Hard', () => {
  for (const id of DIFFICULTIES) {
    for (const [group, keys] of Object.entries(HARDER)) {
      for (const k of Object.keys(keys)) assert.ok(Number.isFinite(DIFFICULTY[id][group][k]), `${id}.${group}.${k}`);
      assert.deepEqual(Object.keys(DIFFICULTY[id][group]).sort(), Object.keys(keys).sort(), `${id}.${group} has exactly the table's keys`);
    }
  }
  for (const [group, keys] of Object.entries(HARDER)) {
    for (const [k, dir] of Object.entries(keys)) {
      const [e, n, h] = DIFFICULTIES.map((id) => DIFFICULTY[id][group][k] * dir);
      assert.ok(e < n && n < h, `${group}.${k} gets harder Easy → Normal → Hard`);
    }
  }
  // Normal is today's game: the numbers the eras used before the table.
  assert.deepEqual(DIFFICULTY.normal.eras, { starvationGrace: 1, hunger: 1, fireDecay: 1, holdSeconds: 5, calmThreshold: 0.55, mammothDanger: 1, crashThreshold: 0.8, launchNeeds: 1, hints: 1 });
  assert.deepEqual(DIFFICULTY.normal.rivals, { offset: 0, aggression: 0, raids: 1, damage: 1, growth: 1 });
  // The rivals' numbers stay inside what Rust accepts (homeworld::Difficulty::clamped).
  for (const id of DIFFICULTIES) {
    const r = DIFFICULTY[id].rivals;
    assert.ok(r.aggression >= -1 && r.aggression <= 1 && r.raids >= 0 && r.raids <= 4 && r.damage >= 0 && r.damage <= 4 && r.growth >= 0.25 && r.growth <= 3);
  }
});

test('options: stored choices, the query string wins, unknown values fall back', () => {
  const storage = memory();
  assert.deepEqual(readOptions({ storage, search: '' }), { worldSize: 'medium', difficulty: 'normal' });
  saveOptions({ worldSize: 'large', difficulty: 'hard' }, { storage });
  assert.deepEqual(readOptions({ storage, search: '' }), { worldSize: 'large', difficulty: 'hard' });
  assert.deepEqual(readOptions({ storage, search: '?difficulty=easy&size=small' }), { worldSize: 'small', difficulty: 'easy' });
  assert.deepEqual(readOptions({ storage, search: '?difficulty=godlike&size=planet' }), { worldSize: 'large', difficulty: 'hard' });
  saveOptions({ worldSize: 'gigantic', difficulty: 'hard' }, { storage });
  assert.equal(storage.m.get('moc.worldSize'), 'large', 'an unknown size is not stored');
  const broken = { getItem() { throw new Error('denied'); }, setItem() { throw new Error('denied'); } };
  assert.deepEqual(readOptions({ storage: broken, search: '' }), { worldSize: 'medium', difficulty: 'normal' });
  saveOptions({ worldSize: 'small' }, { storage: broken });
});

test('eras read Normal when a run has no difficulty', () => {
  assert.equal(eraTuning(null).holdSeconds, 5);
  assert.equal(eraTuning({ difficulty: difficultyOf('hard') }).holdSeconds, 6);
  assert.equal(difficultyOf('nope'), DIFFICULTY.normal);
});

test('the Stone Age takes its calm hold from the difficulty (5 s on Normal)', async () => {
  const { StoneAge } = await import('../src/eras/stone.js');
  const s = new StoneAge();
  s.G = { difficulty: DIFFICULTY.hard };
  assert.equal(s.holdSeconds(), 6);
  s.G = {};
  assert.equal(s.holdSeconds(), 5);
});

test('a save keeps the New Game options, so Continue plays the same size and difficulty', async () => {
  const { buildCheckpoint } = await import('../src/game/checkpoint.js');
  const cp = buildCheckpoint({ options: { worldSize: 'random', difficulty: 'hard', size: 'large' }, world: { seed: 9, width: 960, height: 540 } }, { id: 'r', runNumber: 1, mode: 'story' });
  assert.deepEqual(cp.options, { worldSize: 'random', difficulty: 'hard', size: 'large' });
  assert.deepEqual(cp.worldSize, { width: 960, height: 540 });
  assert.equal(buildCheckpoint({}, { id: 'r', runNumber: 1 }).options, null);
});

test('the New Game step offers every size and difficulty in the table', async () => {
  const { readFileSync } = await import('node:fs');
  const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  const view = html.slice(html.indexOf('data-title-view="new"'), html.indexOf('data-title-view="load"'));
  const values = (opt) => [...view.matchAll(new RegExp(`data-option="${opt}" data-value="([a-z]+)"`, 'g'))].map((m) => m[1]);
  assert.deepEqual(values('worldSize'), [...WORLD_SIZES.map((s) => s.id), 'random']);
  assert.deepEqual(values('difficulty'), DIFFICULTIES);
  assert.match(view, /id="newGameStart"/);
});
