// Replayability (#91): seeds and share codes, daily/weekly challenges, modifiers, scores, unlocks,
// New Mind+ and the end-of-run chronicle. Everything is pure, so no browser is needed.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  MODIFIERS, MODIFIER_IDS, SCORE_CATEGORIES, SETTINGS_KEY, UNLOCKS_KEY,
  applyRunToUnlocks, buildChronicle, cameraEnabled, challengeFor, challengeKey,
  challengeDateLabel, chronicleFacts, combatEnabled, decodeShareCode, emptyProgress,
  encodeShareCode, hasModifier, mindPlusUnlocked, modifierEffect, nextDifficulty,
  parseReplaySearch, parseSeedText, readReplaySettings, readUnlocks, resolveRunConfig,
  runLabel, saveReplaySettings, scoreRun, twoHandZoomEnabled, validateShareCode,
} from '../src/game/replay.js';

const memory = () => {
  const m = new Map();
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    m,
  };
};

const baseRun = {
  seed: 0xdecafbad,
  worldSize: 'large',
  difficulty: 'hard',
  challenge: 'weekly',
  modifiers: ['fragile-fire', 'pacifist'],
  mindPlus: true,
};

test('share codes round-trip every field and reject a typo', () => {
  const code = encodeShareCode(baseRun);
  assert.match(code, /^MOC-[0-9A-HJKMNP-TV-Z]{5}-[0-9A-HJKMNP-TV-Z]{5}-[0-9A-HJKMNP-TV-Z]{2}$/);
  const back = decodeShareCode(code);
  assert.equal(back.seed, baseRun.seed);
  assert.equal(back.worldSize, 'large');
  assert.equal(back.difficulty, 'hard');
  assert.equal(back.challenge, 'weekly');
  assert.deepEqual(back.modifiers, ['fragile-fire', 'pacifist']);
  assert.equal(back.mindPlus, true);
  assert.equal(back.shareCode, code, 'the code is canonical');
  assert.deepEqual(decodeShareCode(code.toLowerCase()), back, 'lower case is accepted');

  const valid = validateShareCode(code);
  assert.equal(valid.ok, true);
  assert.equal(valid.run.seed, baseRun.seed);

  // One changed letter breaks the checksum; malformed input is rejected with a reason.
  const typo = code.slice(0, -1) + (code.endsWith('0') ? '1' : '0');
  assert.equal(decodeShareCode(typo), null);
  assert.equal(validateShareCode(typo).ok, false);
  assert.equal(validateShareCode('nonsense').ok, false);
  assert.equal(validateShareCode('MOC-00000-00000-00').ok, false);

  // Crockford aliases still read back the same code.
  assert.deepEqual(decodeShareCode(code.replace(/O/g, 'O').replace(/I/g, 'I')), back);
});

test('a pasted ?seed= is a number, a share code or short text', () => {
  assert.equal(parseSeedText('12345').seed, 12345);
  assert.equal(parseSeedText(' 42 ').seed, 42);
  assert.equal(parseSeedText('fire-chicken').seed, parseSeedText('fire-chicken').seed);
  assert.notEqual(parseSeedText('fire-chicken').seed, parseSeedText('fire-goat').seed);
  const code = encodeShareCode(baseRun);
  assert.equal(parseSeedText(code).seed, baseRun.seed);
  assert.equal(parseSeedText('MOC-AAAAA-AAAAA-AA'), null, 'a mistyped share code is not hashed into a run');
  assert.equal(parseReplaySearch(`?seed=${code}`).seed, baseRun.seed);
  assert.equal(parseReplaySearch('?seed=777&daily=1').seed, 777);
  assert.equal(parseReplaySearch('?code=' + code).worldSize, 'large');
});

test('resolveRunConfig keeps a shared run and applies New Mind+ only when unlocked', () => {
  const code = encodeShareCode({ ...baseRun, mindPlus: false });
  const out = resolveRunConfig({
    options: { worldSize: 'medium', difficulty: 'normal' },
    search: `?seed=${code}`,
    progress: emptyProgress(),
  });
  assert.equal(out.seed, baseRun.seed);
  assert.equal(out.worldSize, 'large');
  assert.equal(out.difficulty, 'hard');
  assert.equal(out.challenge, 'weekly');
  assert.deepEqual(out.modifiers, ['fragile-fire', 'pacifist']);
  assert.equal(out.shareCode, code);

  // The requested seed/modifiers are used when there is no query string.
  const requested = resolveRunConfig({
    options: { worldSize: 'small', difficulty: 'easy' },
    requested: { seed: 99, modifiers: ['blink-storm'], worldSize: 'huge', difficulty: 'normal' },
    progress: emptyProgress(),
  });
  assert.equal(requested.seed, 99);
  assert.equal(requested.worldSize, 'huge');
  assert.equal(requested.difficulty, 'normal');
  assert.deepEqual(requested.modifiers, ['blink-storm']);

  // New Mind+ is gated on the first ending, and bumps the difficulty one step when allowed.
  const locked = resolveRunConfig({ requested: { seed: 5, difficulty: 'normal', mindPlus: true }, progress: emptyProgress() });
  assert.equal(locked.mindPlus, false);
  assert.equal(locked.difficulty, 'normal');
  const unlocked = resolveRunConfig({ requested: { seed: 5, difficulty: 'normal', mindPlus: true }, progress: { endings: 1, unlocked: [] } });
  assert.equal(unlocked.mindPlus, true);
  assert.equal(unlocked.difficulty, 'hard');
  assert.equal(nextDifficulty('easy'), 'normal');
  assert.equal(nextDifficulty('hard'), 'hard');

  // A save keeps its own replay choices, even for a campaign started before this feature.
  const resumed = resolveRunConfig({
    options: { worldSize: 'medium', difficulty: 'normal' },
    resume: { seed: 123, options: { worldSize: 'small', difficulty: 'easy', size: 'small' } },
  });
  assert.equal(resumed.seed, 123);
  assert.equal(resumed.worldSize, 'small');
  assert.equal(resumed.difficulty, 'easy');
  assert.equal(resumed.modifiers.length, 0);
});

test('daily and weekly seeds come from the UTC date, deterministically and with rollover', () => {
  const day = Date.UTC(2026, 10, 8, 23, 30);
  const sameDay = Date.UTC(2026, 10, 8, 0, 5);
  const nextDay = Date.UTC(2026, 10, 9, 0, 5);
  assert.equal(challengeFor('daily', day).seed, challengeFor('daily', sameDay).seed);
  assert.notEqual(challengeFor('daily', day).seed, challengeFor('daily', nextDay).seed);
  assert.equal(challengeFor('daily', day).key, '2026-11-08');

  const monday = Date.UTC(2026, 10, 9, 12);
  const sunday = Date.UTC(2026, 10, 15, 12);
  const nextMonday = Date.UTC(2026, 10, 16, 12);
  assert.equal(challengeFor('weekly', monday).seed, challengeFor('weekly', sunday).seed);
  assert.notEqual(challengeFor('weekly', monday).seed, challengeFor('weekly', nextMonday).seed);
  assert.equal(challengeKey('weekly', monday), '2026-W46');
  assert.match(challengeDateLabel('daily', day), /\d/);
  assert.match(challengeDateLabel('weekly', monday), /\d/);
  assert.equal(challengeFor('none', day).seed, null);

  // No scheduling randomness: a date hash, never a dice roll.
  const src = readFileSync(new URL('../src/game/replay.js', import.meta.url), 'utf8');
  assert.doesNotMatch(src, /Math\.random\s*\(/);
  assert.match(src, /cryptoObj\?\.getRandomValues/);
});

test('the modifier table is complete and its effects are read by the systems', () => {
  assert.equal(MODIFIERS.length, 6);
  assert.deepEqual(MODIFIER_IDS, ['fragile-fire', 'volatile-market', 'blink-storm', 'no-camera', 'one-hand', 'pacifist']);
  for (const m of MODIFIERS) {
    assert.ok(m.label && m.blurb && m.readBy, m.id);
    assert.equal(typeof m.effects, 'object');
    assert.ok(Object.keys(m.effects).length > 0, m.id);
    assert.ok(m.score > 0, m.id);
  }
  const G = { replay: { modifiers: ['fragile-fire', 'no-camera', 'one-hand', 'pacifist'] } };
  assert.equal(hasModifier(G, 'fragile-fire'), true);
  assert.equal(hasModifier(G, 'blink-storm'), false);
  assert.ok(modifierEffect(G, 'fragile-fire', 'fireDecay', 1) > 1);
  assert.ok(modifierEffect(G, 'fragile-fire', 'hunger', 1) > 1);
  assert.equal(modifierEffect(G, 'blink-storm', 'boltCooldown', 0.5), 0.5);
  assert.equal(cameraEnabled(G), false);
  assert.equal(twoHandZoomEnabled(G), false);
  assert.equal(combatEnabled(G), false);
  const clean = { replay: { modifiers: [] } };
  assert.equal(cameraEnabled(clean), true);
  assert.equal(twoHandZoomEnabled(clean), true);
  assert.equal(combatEnabled(clean), true);

  // The hooks really exist where the table says they are read.
  const read = (p) => readFileSync(new URL(`../${p}`, import.meta.url), 'utf8');
  assert.match(read('src/eras/stone.js'), /modifierEffect\([\s\S]*'fragile-fire'/);
  assert.match(read('src/eras/stone.js'), /modifierEffect\([\s\S]*'blink-storm'/);
  assert.match(read('src/eras/industry.js'), /modifierEffect\([\s\S]*'volatile-market'/);
  assert.match(read('src/main.js'), /twoHandZoomEnabled/);
  assert.match(read('src/main.js'), /cameraEnabled/);
  assert.match(read('src/camera/camera.js'), /combatEnabled/);
});

test('scores have all six categories, are deterministic and move the right way', () => {
  const summary = (patch = {}) => ({
    finished: true,
    runTime: 600,
    stats: { lightning: 10, births: 10, deaths: 2, recessions: 1, restarts: 1, smitten: 0 },
    extra: { passengers: 8 },
    eraTimes: [{ seconds: 100 }, { seconds: 200 }, { seconds: 300 }],
    replay: { modifiers: [] },
    ...patch,
  });
  const base = scoreRun(summary());
  assert.deepEqual(base.categories.map((c) => c.id), SCORE_CATEGORIES.map((c) => c.id));
  assert.deepEqual(base.categories.map((c) => c.id), ['survival', 'prosperity', 'peace', 'chaos', 'speed', 'style']);
  for (const c of base.categories) assert.ok(c.score >= 0 && c.score <= 100, c.id);
  assert.ok(['S', 'A', 'B', 'C', 'D', 'E'].includes(base.rank));
  assert.deepEqual(scoreRun(summary()), base, 'the same run gives the same scores');

  const score = (patch) => scoreRun(summary(patch)).categoryMap;
  assert.ok(score({ runTime: 900 }).survival >= score({ runTime: 300 }).survival);
  assert.ok(score({ stats: { ...summary().stats, births: 30 } }).prosperity > score({ stats: { ...summary().stats, births: 3 } }).prosperity);
  assert.ok(score({ stats: { ...summary().stats, lightning: 42 } }).chaos > score({ stats: { ...summary().stats, lightning: 2 } }).chaos);
  assert.ok(score({ stats: { ...summary().stats, deaths: 0 } }).peace > score({ stats: { ...summary().stats, deaths: 20 } }).peace);
  assert.ok(score({ runTime: 300 }).speed > score({ runTime: 900 }).speed);
  assert.ok(score({ replay: { modifiers: ['fragile-fire', 'pacifist'] } }).style > score({ replay: { modifiers: [] } }).style);
  assert.equal(score({ finished: false, runTime: 600 }).speed, 0);
});

test('the chronicle uses only supplied real lines and returns 3–8 of them', () => {
  const events = Array.from({ length: 12 }, (_, i) => `Real event ${i + 1}.`);
  const lines = buildChronicle({ events });
  assert.ok(lines.length >= 3 && lines.length <= 8, `${lines.length} lines`);
  assert.equal(new Set(lines).size, lines.length);
  for (const line of lines) assert.ok(events.includes(line), line);
  assert.deepEqual(buildChronicle({ events: ['one', 'two', 'two', 'three'] }), ['one', 'two', 'three']);
  assert.deepEqual(buildChronicle({ events: [{ text: 'from an object' }] }), ['from an object']);

  // Facts are derived from the run's own numbers, and an empty run invents nothing.
  assert.deepEqual(chronicleFacts({ runTime: 0, stats: {}, extra: {}, eraTimes: [] }, null), []);
  const facts = chronicleFacts(summaryForFacts(), { rank: 'A', rankLabel: 'Feathered Legend' });
  assert.ok(facts.some((f) => /600 lightning|12 lightning/.test(f) || /lightning/.test(f)));
  assert.ok(facts.some((f) => /passengers and one chicken/.test(f)));
  const merged = buildChronicle({ events: ['Advisor line.'], facts: ['A fact.'], max: 8 });
  assert.deepEqual(merged, ['Advisor line.', 'A fact.']);
});

function summaryForFacts() {
  return {
    finished: true,
    runTime: 754,
    stats: { lightning: 12, births: 6, deaths: 1, recessions: 1 },
    extra: { passengers: 8 },
    eraTimes: [{ label: 'Era I · Stone Age', seconds: 100 }, { label: 'Era III · Space Age', seconds: 300 }],
    replay: { modifiers: ['fragile-fire'], mindPlus: true },
  };
}

test('unlocks persist, apply once per run and gate New Mind+', () => {
  const storage = memory();
  assert.equal(mindPlusUnlocked(readUnlocks({ storage })), false);
  const summary = {
    finished: true,
    runTime: 700,
    stats: { lightning: 25, births: 12, deaths: 1, recessions: 1, restarts: 0 },
    extra: { passengers: 8 },
    replay: { modifiers: [] },
    scores: { peace: 80, chaos: 70 },
    rank: 'S',
  };
  const first = applyRunToUnlocks(summary, { storage, runId: 'run-1' });
  assert.equal(first.alreadyApplied, false);
  assert.ok(first.newUnlocks.some((u) => u.id === 'new-mind-plus'));
  assert.ok(first.newUnlocks.some((u) => u.id === 'power-stormcaller'));
  assert.ok(first.newUnlocks.some((u) => u.id === 'ending-sacred-egg'));
  assert.equal(mindPlusUnlocked(first.progress), true);
  assert.equal(readUnlocks({ storage }).endings, 1);

  const again = applyRunToUnlocks(summary, { storage, runId: 'run-1' });
  assert.equal(again.alreadyApplied, true);
  assert.deepEqual(again.newUnlocks, []);
  assert.equal(readUnlocks({ storage }).endings, 1, 'the same run is not counted twice');

  const stored = JSON.parse(storage.getItem(UNLOCKS_KEY));
  assert.equal(stored.endings, 1);
  assert.ok(stored.unlocked.includes('trait-firekeeper'));

  // Corrupt storage is just an empty progress, never a throw.
  const broken = { getItem: () => '{not json', setItem() { throw new Error('denied'); } };
  assert.deepEqual(readUnlocks({ storage: broken }), emptyProgress());
  assert.equal(applyRunToUnlocks(summary, { storage: broken, runId: 'r' }).progress.endings, 1);
});

test('replay settings persist; a corrupt value falls back safely', () => {
  const storage = memory();
  saveReplaySettings({
    seed: 7, worldSize: 'huge', difficulty: 'hard', challenge: 'daily',
    challengeKey: '2026-11-08', modifiers: ['no-camera', 'nonsense'], mindPlus: true,
  }, { storage });
  const back = readReplaySettings({ storage });
  assert.equal(back.seed, 7);
  assert.equal(back.worldSize, 'huge');
  assert.equal(back.challenge, 'daily');
  assert.deepEqual(back.modifiers, ['no-camera']);
  assert.equal(storage.getItem(SETTINGS_KEY).includes('nonsense'), false);
  storage.setItem(SETTINGS_KEY, '{broken');
  assert.equal(readReplaySettings({ storage }), null);
});

test('the title and end screen are wired, and runLabel names the challenge and modifiers', () => {
  const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  for (const id of ['runSeedChip', 'runSeedInput', 'runSeedDaily', 'runSeedWeekly', 'runShareCode', 'copyRunCode', 'mindPlusToggle', 'endReplay', 'endScores', 'endChronicle', 'endMindPlus']) {
    assert.match(html, new RegExp(`id="${id}"`), id);
  }
  for (const id of MODIFIER_IDS) assert.match(html, new RegExp(`data-modifier="${id}"`), id);
  const main = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.match(main, /initReplayUi\(document\)/);
  assert.match(main, /resolveRunConfig\(/);
  assert.match(main, /buildChronicle\(/);
  assert.match(main, /applyRunToUnlocks\(/);

  const label = runLabel({ seed: 0xdecafbad, challenge: 'daily', modifiers: ['pacifist'], mindPlus: true });
  assert.match(label, /Seed DECAFBAD/);
  assert.match(label, /Daily/);
  assert.match(label, /Pacifist/);
  assert.match(label, /New Mind\+/);
});
