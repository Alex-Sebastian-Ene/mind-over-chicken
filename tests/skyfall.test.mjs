// Skyfall showcase event (#160): seeded schedule, countdown, sustained focus charge, input source,
// one-shot outcomes/cooldown, per-era config and clean reset. The state machine is pure, so the
// tests drive it directly with simulated focus and the keyboard-fallback input shape.
//
// Run: node --test tests/skyfall.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  Skyfall, SKYFALL_CONFIG, skyfallConfig, skyfallSchedule, skyfallInput, sourceLabel, skyfallObjective,
} from '../src/game/skyfall.js';

const TICK = 0.1;

function fastConfig(era = 'stone', extra = {}) {
  return { ...skyfallConfig(era), firstDelay: [0, 0], cooldown: [12, 12], ...extra };
}

function start(sf, input = { focus: 0, threshold: 0.5, keyboard: false, sourceKind: 'sim' }) {
  let ev = sf.update(TICK, input);
  assert.equal(ev.started, true, 'the event starts on the first eligible tick');
  return ev;
}

function run(sf, input, seconds = 30) {
  let ev = sf.snapshot();
  for (let i = 0; i < Math.round(seconds / TICK); i++) ev = sf.update(TICK, input);
  return ev;
}

test('holding focus above the threshold charges the shield and averts the event', () => {
  const sf = new Skyfall({ era: 'stone', seed: 7, tutorial: false, config: fastConfig() });
  start(sf);
  const holding = { focus: 0.9, threshold: 0.5, keyboard: false, sourceKind: 'bridge' };
  let ev = sf.snapshot();
  for (let i = 0; i < 100 && !ev.averted; i++) ev = sf.update(TICK, holding);
  assert.equal(ev.averted, true);
  assert.equal(sf.phase, 'averted');
  assert.match(ev.sourceLabel, /EEG/);
  assert.equal(ev.timeLeft > 0, true, 'the shield filled before impact');
  assert.equal(ev.progress, 1);
});

test('ignoring the event lets the countdown expire and strikes the era', () => {
  const sf = new Skyfall({ era: 'stone', seed: 8, tutorial: false, config: fastConfig() });
  start(sf);
  const idle = { focus: 0, threshold: 0.5, keyboard: false, sourceKind: 'sim' };
  let ev = sf.snapshot();
  for (let i = 0; i < Math.round((skyfallConfig('stone').countdown + 1) / TICK); i++) {
    ev = sf.update(TICK, idle);
    if (ev.failed) break;
  }
  assert.equal(ev.failed, true);
  assert.equal(sf.phase, 'failed');
  assert.equal(ev.timeLeft, 0);
  assert.equal(ev.charge, 0);
});

test('a tap does not count as sustained focus', () => {
  const sf = new Skyfall({ era: 'stone', seed: 9, tutorial: false, config: fastConfig() });
  start(sf);
  // Tap one frame, release for nine: the shield drains much faster than a tap can fill it.
  for (let i = 0; i < 15; i++) {
    sf.update(TICK, { focus: 1, threshold: 0.5, keyboard: true, sourceKind: 'sim' });
    for (let j = 0; j < 9; j++) sf.update(TICK, { focus: 0, threshold: 0.5, keyboard: false, sourceKind: 'sim' });
  }
  const ev = sf.snapshot();
  assert.equal(ev.averted, false, 'tapping never completes the ritual');
  assert.ok(ev.charge < 0.2, `tapping left only ${ev.charge}s of charge`);
  assert.equal(sf.phase, 'active', 'the countdown is still running when the taps stop');
});

test('the keyboard fallback uses the same input.focus path as the fire hold', () => {
  const G = { brain: { focus: 0, threshold: (kind, base) => base }, source: { kind: 'sim', input: { focus: true } } };
  const input = skyfallInput(G, skyfallConfig('stone'));
  assert.equal(input.focus, 1);
  assert.equal(input.keyboard, true);
  assert.equal(input.sourceKind, 'sim');

  const sf = new Skyfall({ era: 'stone', seed: 10, tutorial: false, config: fastConfig() });
  start(sf);
  let ev = sf.snapshot();
  for (let i = 0; i < 100 && !ev.averted; i++) ev = sf.update(TICK, input);
  assert.equal(ev.averted, true);
  assert.equal(ev.source, 'keyboard');
  assert.equal(ev.sourceLabel, 'Keyboard F');

  G.source.input.focus = false;
  const released = skyfallInput(G, skyfallConfig('stone'));
  assert.equal(released.focus, 0);
  assert.equal(released.keyboard, false);
});

test('the live HUD names EEG focus vs the keyboard fallback', () => {
  assert.equal(sourceLabel('keyboard'), 'Keyboard F');
  assert.equal(sourceLabel('eeg'), 'EEG focus');
  assert.equal(sourceLabel('none'), 'No focus input');

  const eeg = new Skyfall({ era: 'space', seed: 11, tutorial: false, config: fastConfig('space') });
  start(eeg);
  const ev = eeg.update(TICK, { focus: 0.8, threshold: 0.5, keyboard: false, sourceKind: 'bridge' });
  assert.equal(ev.source, 'eeg');
  assert.equal(ev.sourceLabel, 'EEG focus');

  const keyboard = new Skyfall({ era: 'space', seed: 11, tutorial: false, config: fastConfig('space') });
  start(keyboard);
  const kev = keyboard.update(TICK, { focus: 1, threshold: 0.5, keyboard: true, sourceKind: 'sim' });
  assert.equal(kev.source, 'keyboard');
  assert.equal(kev.sourceLabel, 'Keyboard F');
});

test('one event at a time, then a seeded cooldown before the next', () => {
  const cfg = fastConfig('industry', { outcomeSeconds: 0.4 });
  const sf = new Skyfall({ era: 'industry', seed: 12, tutorial: false, config: cfg });
  start(sf);
  let ev = sf.snapshot();
  for (let i = 0; i < 100 && !ev.averted; i++) ev = sf.update(TICK, { focus: 1, threshold: 0.5, keyboard: true, sourceKind: 'sim' });
  assert.equal(ev.averted, true);
  assert.equal(sf.events, 1);

  // The outcome is still on screen; no new event may start yet.
  for (let i = 0; i < 3; i++) {
    const during = sf.update(TICK, { focus: 1, threshold: 0.5, keyboard: true, sourceKind: 'sim' });
    assert.equal(during.started, false);
  }
  run(sf, { eligible: true, focus: 0 }, 1);
  assert.equal(sf.phase, 'idle');
  assert.ok(sf.cooldownLeft > 0);
  assert.equal(sf.update(TICK, { focus: 0 }).started, false);
  let again = sf.snapshot();
  for (let i = 0; i < Math.round((cfg.cooldown[0] + 1) / TICK); i++) {
    again = sf.update(TICK, { focus: 0 });
    if (again.started) break;
  }
  assert.equal(again.started, true, 'the next event starts after the cooldown');
  assert.equal(sf.events, 2);
});

test('each era has its own theatrics, countdown and consequence budget', () => {
  const threats = new Set();
  for (const era of ['stone', 'industry', 'space']) {
    const cfg = skyfallConfig(era);
    assert.equal(cfg.era, era);
    assert.ok(cfg.countdown >= 15 && cfg.countdown <= 20, `${era} countdown is in the 15–20 s window`);
    assert.ok(cfg.chargeSeconds > 0 && cfg.chargeSeconds < cfg.countdown);
    assert.ok(cfg.threshold > 0 && cfg.threshold < 1);
    assert.ok(cfg.firstDelay[0] < cfg.firstDelay[1] && cfg.cooldown[0] < cfg.cooldown[1]);
    assert.ok(cfg.damage && Object.keys(cfg.damage).length, `${era} has a real consequence`);
    assert.ok(cfg.visuals?.meteor && cfg.visuals?.shield);
    assert.ok(cfg.tutorial && cfg.averted && cfg.failed);
    threats.add(cfg.threat);
  }
  assert.equal(threats.size, 3, 'meteor, storm and solar flare are distinct');
});

test('reset on a scene change/retry clears the event and its schedule', () => {
  const sf = new Skyfall({ era: 'stone', seed: 13, tutorial: false, config: fastConfig() });
  start(sf);
  sf.update(TICK, { focus: 1, threshold: 0.5, keyboard: true, sourceKind: 'sim' });
  assert.equal(sf.active, true);
  const fresh = sf.reset({ seed: 99, tutorial: true });
  assert.equal(fresh, sf);
  assert.equal(sf.phase, 'idle');
  assert.equal(sf.timeLeft, 0);
  assert.equal(sf.charge, 0);
  assert.equal(sf.events, 0);
  assert.equal(sf.cooldownLeft, sf.firstDelay);
  assert.equal(sf.tutorialDue, true);
  const ev = sf.update(TICK, { focus: 0 });
  assert.equal(ev.started, true, 'a fresh scene can run the event again');
});

test('the schedule is deterministic from the run seed', () => {
  const a = skyfallSchedule(42, SKYFALL_CONFIG.stone);
  const b = skyfallSchedule(42, SKYFALL_CONFIG.stone);
  assert.deepEqual(a, b);
  const others = [1, 2, 3, 4, 5].map((s) => skyfallSchedule(s, SKYFALL_CONFIG.stone));
  assert.ok(others.some((s) => s.firstDelay !== a.firstDelay || s.cooldown !== a.cooldown), 'different seeds move the schedule');

  const first = new Skyfall({ era: 'stone', seed: 42, tutorial: false });
  const second = new Skyfall({ era: 'stone', seed: 42, tutorial: false });
  const ticksToStart = (sf) => {
    for (let i = 0; i < 1000; i++) if (sf.update(TICK, { focus: 0 }).started) return i;
    return -1;
  };
  const aTicks = ticksToStart(first);
  const bTicks = ticksToStart(second);
  assert.equal(aTicks, bTicks);
  assert.ok(aTicks > 0);
});

test('nothing triggers or advances while the scene is ineligible', () => {
  const sf = new Skyfall({ era: 'stone', seed: 14, tutorial: false, config: fastConfig() });
  const before = sf.cooldownLeft;
  for (let i = 0; i < 50; i++) {
    const ev = sf.update(TICK, { eligible: false, focus: 1, threshold: 0.5, keyboard: true });
    assert.equal(ev.started, false);
    assert.equal(ev.paused, true);
  }
  assert.equal(sf.phase, 'idle');
  assert.equal(sf.cooldownLeft, before, 'an intro/end or paused signal does not consume the schedule');

  start(sf);
  sf.update(TICK, { focus: 1, threshold: 0.5, keyboard: true, sourceKind: 'sim' });
  const frozen = sf.snapshot();
  const paused = sf.update(5, { eligible: false, focus: 0, threshold: 0.5 });
  assert.equal(paused.timeLeft, frozen.timeLeft);
  assert.equal(paused.charge, frozen.charge);
  assert.equal(sf.phase, 'active');
});

test('the active event contributes a clear HUD objective', () => {
  const sf = new Skyfall({ era: 'stone', seed: 15, tutorial: false, config: fastConfig() });
  assert.deepEqual(skyfallObjective(sf), []);
  start(sf);
  sf.update(TICK, { focus: 1, threshold: 0.5, keyboard: true, sourceKind: 'sim' });
  const row = skyfallObjective(sf)[0];
  assert.equal(row.label, 'Skyfall');
  assert.match(row.value, /shield/);
  assert.equal(row.bad, true);
});

test('all three eras drive the shared event from their updates and draw the HUD', async () => {
  for (const era of ['stone', 'industry', 'space']) {
    const src = await readFile(new URL(`../src/eras/${era}.js`, import.meta.url), 'utf8');
    assert.match(src, /new Skyfall\(\{ era: this\.era/, `${era} creates the seeded event`);
    assert.match(src, /updateSkyfall\(dt\)/, `${era} steps it`);
    assert.match(src, /drawSkyfallHud\(ctx, this\.skyfall\?\.snapshot\(\)/, `${era} draws the source/focus HUD`);
    assert.match(src, /skyfall-warn/, `${era} telegraphs with the advisor`);
    assert.match(src, /skyfall-averted/, `${era} explains the averted outcome`);
    assert.match(src, /skyfall-failed/, `${era} explains the failed outcome`);
  }
});
