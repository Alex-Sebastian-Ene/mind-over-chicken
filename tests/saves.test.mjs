// Local game saves (src/game/saves.js): several runs, newest first, upsert by id, capped, and
// never throwing when storage is missing, full or corrupt.
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  SAVES_KEY, MAX_SAVES, listSaves, getSave, latestSave, saveRun, deleteSave, clearSaves,
  newRunId, nextRunNumber, runName,
} from '../src/game/saves.js';

const memory = () => {
  const m = new Map();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), _m: m };
};

test('saves are listed newest first and upserted by id', () => {
  const s = memory();
  assert.deepEqual(listSaves(s), []);
  saveRun({ id: 'a', name: 'Run 1 · Stone Age', sceneIndex: 0, sceneLabel: 'Stone Age', seed: 11, updatedAt: 100 }, s);
  saveRun({ id: 'b', name: 'Run 2 · Stone Age', sceneIndex: 0, sceneLabel: 'Stone Age', seed: 22, updatedAt: 200 }, s);
  assert.deepEqual(listSaves(s).map((x) => x.id), ['b', 'a']);
  // Saving run a again moves it to the top and keeps its createdAt.
  const first = getSave('a', s);
  saveRun({ id: 'a', name: 'Run 1 · Industry', sceneIndex: 2, sceneLabel: 'Industry', seed: 11, updatedAt: 300 }, s);
  const list = listSaves(s);
  assert.deepEqual(list.map((x) => x.id), ['a', 'b']);
  assert.equal(list[0].sceneIndex, 2);
  assert.equal(list[0].sceneLabel, 'Industry');
  assert.equal(list[0].name, 'Run 1 · Industry');
  assert.equal(list[0].createdAt, first.createdAt);
  assert.equal(list[0].updatedAt, 300);
  assert.equal(latestSave(s).id, 'a');
  assert.equal(JSON.parse(s.getItem(SAVES_KEY)).length, 2);
});

test('the save list is capped, dropping the oldest', () => {
  const s = memory();
  for (let i = 0; i < MAX_SAVES + 3; i++) saveRun({ id: `r${i}`, name: `Run ${i}`, updatedAt: 1000 + i }, s);
  const list = listSaves(s);
  assert.equal(list.length, MAX_SAVES);
  assert.equal(list[0].id, `r${MAX_SAVES + 2}`);
  assert.ok(!list.some((x) => x.id === 'r0' || x.id === 'r1' || x.id === 'r2'));
});

test('delete and clear', () => {
  const s = memory();
  saveRun({ id: 'a', updatedAt: 1 }, s);
  saveRun({ id: 'b', updatedAt: 2 }, s);
  assert.equal(deleteSave('a', s), true);
  assert.equal(deleteSave('nope', s), false);
  assert.deepEqual(listSaves(s).map((x) => x.id), ['b']);
  assert.equal(clearSaves(s), true);
  assert.deepEqual(listSaves(s), []);
});

test('missing, throwing or corrupt storage never throws', () => {
  const throwing = { getItem() { throw new Error('denied'); }, setItem() { throw new Error('denied'); }, removeItem() { throw new Error('denied'); } };
  assert.deepEqual(listSaves(null), []);
  assert.deepEqual(listSaves(throwing), []);
  assert.equal(saveRun({ id: 'a' }, null), null);
  assert.equal(saveRun({ id: 'a' }, throwing), null);
  assert.equal(deleteSave('a', throwing), false);
  assert.equal(clearSaves(throwing), false);
  assert.equal(latestSave(throwing), null);
  const s = memory();
  s.setItem(SAVES_KEY, '{oops');
  assert.deepEqual(listSaves(s), []);
  s.setItem(SAVES_KEY, JSON.stringify([{ id: 'ok', updatedAt: 1 }, null, 7, { name: 'no id' }]));
  assert.deepEqual(listSaves(s).map((x) => x.id), ['ok']);
  // A corrupt list is replaced cleanly by the next save.
  s.setItem(SAVES_KEY, 'not json');
  saveRun({ id: 'fresh', updatedAt: 5 }, s);
  assert.deepEqual(listSaves(s).map((x) => x.id), ['fresh']);
});

test('a full quota sheds the oldest saves so the newest still fits', () => {
  const s = memory();
  for (let i = 0; i < 5; i++) saveRun({ id: `r${i}`, updatedAt: i + 1 }, s);
  const realSet = s.setItem;
  s.setItem = (k, v) => { if (JSON.parse(v).length > 2) throw new Error('QuotaExceededError'); realSet(k, v); };
  const rec = saveRun({ id: 'big', updatedAt: 99 }, s);
  assert.equal(rec.id, 'big');
  assert.deepEqual(listSaves(s).map((x) => x.id), ['big', 'r4']);
});

test('a save without an id gets one, and sceneIndex is a whole number', () => {
  const s = memory();
  const rec = saveRun({ name: 'x', sceneIndex: '2.7' }, s);
  assert.match(rec.id, /^run-/);
  assert.equal(rec.sceneIndex, 2);
  assert.equal(saveRun(null, s), null);
});

test('run ids are unique-ish and run names count up', () => {
  assert.notEqual(newRunId(), newRunId());
  assert.equal(newRunId(36 ** 3, () => 0), 'run-1000-0000');
  const s = memory();
  assert.equal(nextRunNumber(s), 1);
  saveRun({ id: 'a', runNumber: 1 }, s);
  saveRun({ id: 'b', runNumber: 4 }, s);
  assert.equal(nextRunNumber(s), 5);
  assert.equal(runName(3, 'Stone Age'), 'Run 3 · Stone Age');
  assert.equal(runName(2), 'Run 2');
});
