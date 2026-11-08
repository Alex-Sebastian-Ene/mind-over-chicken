// #152: joy is a visible continuous buff, surprise a visible one-shot, and the HUD names both.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  JOY_START, JOY_FULL, SURPRISE_COOLDOWN, SURPRISE_SHOW, MOOD_EFFECTS,
  moodLive, joyBuff, surpriseReady, surpriseFires, surpriseEffect, sceneKind,
  joyCause, surpriseCause, moodChip, sparkleCount,
} from '../src/camera/mood-actions.js';
import { manualMood } from '../src/camera/manual-mood.js';
import { neutralMood } from '../src/camera/emotion.js';
import { FLASH_CAP, FLASH_CAP_REDUCED } from '../src/ui/motion.js';
import { Hud } from '../src/ui/hud.js';

const live = (m) => ({ ...m, active: true, source: m.source || 'camera' });
const read = (p) => readFileSync(new URL(`../${p}`, import.meta.url), 'utf8');

test('joy is a buff that ramps from JOY_START to JOY_FULL', () => {
  assert.equal(joyBuff(live({ joy: 0 })), 0);
  assert.equal(joyBuff(live({ joy: JOY_START })), 0, 'talking or a polite twitch stays below the start');
  assert.equal(joyBuff(live({ joy: JOY_FULL })), 1);
  assert.equal(joyBuff(live({ joy: 1 })), 1);
  const mid = joyBuff(live({ joy: (JOY_START + JOY_FULL) / 2 }));
  assert.ok(Math.abs(mid - 0.5) < 1e-9, `half way gives half the buff, got ${mid}`);
  let prev = -1;
  for (let j = 0; j <= 1.0001; j += 0.05) { const b = joyBuff(live({ joy: j })); assert.ok(b >= prev); prev = b; }
});

test('the manual slider: Happy and Mix give the full buff, Surprised none', () => {
  assert.equal(joyBuff(live(manualMood(0))), 1);
  assert.equal(joyBuff(live(manualMood(0.5))), 1);
  assert.equal(joyBuff(live(manualMood(1))), 0);
});

test('no effect when the camera is off', () => {
  const off = { ...neutralMood(), joy: 0.9, surpriseTriggered: true, active: false, source: 'none' };
  assert.equal(moodLive(off), false);
  assert.equal(joyBuff(off), 0);
  assert.equal(surpriseFires(off), false);
  assert.equal(joyBuff(null), 0);
  assert.equal(joyBuff(undefined), 0);
  assert.equal(moodChip(off, 'stone', 10), null, 'the HUD chip hides');
  // an onMood payload without `active` (older callers) counts as live
  assert.equal(joyBuff({ joy: 1 }), 1);
});

test('surprise fires only on the one-shot event, never while a face just stays surprised', () => {
  assert.equal(surpriseFires(live({ surprise: 1, dominant: 'surprise', reason: 'strong', surpriseTriggered: false })), false);
  assert.equal(surpriseFires(live({ surprise: 1, dominant: 'surprise', reason: 'change' })), false);
  assert.equal(surpriseFires(live({ surprise: 1, dominant: 'joy', reason: 'surprise', surpriseTriggered: true })), true,
    'a Mix (joy dominant) still fires the surprise');
});

test('one surprise per cooldown, whatever the source', () => {
  assert.equal(surpriseReady(0, null), true);
  assert.equal(surpriseReady(10, 9), false);
  assert.equal(surpriseReady(10, 10 - SURPRISE_COOLDOWN + 0.01), false);
  assert.equal(surpriseReady(10, 10 - SURPRISE_COOLDOWN), true);
  assert.equal(SURPRISE_COOLDOWN, 4, 'matches the camera tracker cooldown (emotion.js)');
});

test('the surprise effect is short and capped, and calmer under reduced motion', () => {
  const full = surpriseEffect(false), calm = surpriseEffect(true);
  assert.ok(full.flash > 0 && full.flash <= FLASH_CAP);
  assert.ok(calm.flash > 0 && calm.flash <= FLASH_CAP_REDUCED);
  assert.ok(full.shake > 0 && full.shake <= 2, 'a short shake (G.shake decays 2 per second)');
  assert.equal(calm.shake, 0);
  assert.ok(full.bolts > calm.bolts && calm.bolts >= 1);
});

test('every scene the issue lists has a kind with words for joy and surprise', () => {
  const scenes = [
    [{ id: 'stone', era: 'stone' }, 'stone'], [{ id: 'industry', era: 'industry' }, 'industry'],
    [{ id: 'space', era: 'space' }, 'space'], [{ id: 'timeskip1', era: 'farming' }, 'timeskip'],
    [{ id: 'timeskip2', era: 'space' }, 'timeskip'], [{ id: 'civ' }, 'civ'],
    [{ id: 'surface', era: 'surface' }, 'surface'],
  ];
  for (const [scene, kind] of scenes) {
    assert.equal(sceneKind(scene), kind, scene.id);
    const e = MOOD_EFFECTS[kind];
    assert.ok(e.tag && e.joy && e.surprise, kind);
  }
  assert.equal(sceneKind({ id: 'galaxy' }), null);
  assert.equal(sceneKind(null), null);
});

test('the cause lines name the emotion, its source and what it did here', () => {
  assert.match(joyCause('stone'), /^😊 Your smile: the tribe dances/);
  assert.match(joyCause('stone', 'manual'), /^😊 Manual joy:/);
  assert.match(surpriseCause('industry'), /^😮 Your surprised face: lightning hits the mill/);
  assert.match(surpriseCause('space', 'manual'), /^😮 Manual surprise:/);
  assert.match(joyCause(null), /😊 Your smile: the world brightens/, 'an unknown scene still names joy');
  assert.match(surpriseCause(null), /😮 Your surprised face: the sky flashes/);
});

test('the HUD chip shows the joy buff and the surprise cooldown', () => {
  const happy = live(manualMood(0));
  let c = moodChip(happy, 'stone', 100);
  assert.equal(c.tone, 'joy');
  assert.match(c.text, /😊 Joy 100% · tribe dances · 😮 Surprise ready/);
  assert.match(c.title, /Smile: the tribe dances/);

  c = moodChip({ ...live(neutralMood()) }, 'civ', 100);
  assert.equal(c.tone, 'idle');
  assert.match(c.text, /Smile for a buff/);

  const fired = { ...live(manualMood(1)), surpriseAt: 100 };
  c = moodChip(fired, 'space', 100 + SURPRISE_SHOW / 2);
  assert.equal(c.tone, 'surprise');
  assert.match(c.text, /😮 Surprise!/);
  c = moodChip(fired, 'space', 100 + SURPRISE_SHOW + 0.1);
  assert.match(c.text, /😮 Surprise in [1-4] s/);
  c = moodChip(fired, 'space', 100 + SURPRISE_COOLDOWN);
  assert.match(c.text, /😮 Surprise ready/);
});

test('sparkles scale with joy and stop without it', () => {
  assert.equal(sparkleCount(0, 1, 14, 0), 0);
  assert.equal(sparkleCount(1, 1, 14, 0.99), 14);
  let sum = 0;
  for (let i = 0; i < 1000; i++) sum += sparkleCount(0.5, 1 / 60, 12, (i + 0.5) / 1000);
  assert.ok(Math.abs(sum - 0.5 * 12 * 1000 / 60) <= 1, `about 6 a second at half joy, got ${sum * 60 / 1000}`);
});

test('Hud.showMood fills and hides the chip', () => {
  const el = { hidden: true, textContent: '', title: '', dataset: {} };
  const hud = { moodEl: el };
  Hud.prototype.showMood.call(hud, { mood: live(manualMood(0)), scene: { id: 'industry' } }, 5);
  assert.equal(el.hidden, false);
  assert.match(el.textContent, /markets steady/);
  assert.equal(el.dataset.tone, 'joy');
  Hud.prototype.showMood.call(hud, { mood: { ...neutralMood(), active: false, source: 'none' }, scene: { id: 'industry' } }, 5);
  assert.equal(el.hidden, true);
  Hud.prototype.showMood.call({ moodEl: null }, {}, 0);   // no chip in the page: no throw
  // the HUD's scene is a wrapper (cosmos.hudScene), so the kind the camera stamped wins
  Hud.prototype.showMood.call(hud, { mood: { ...live(manualMood(0)), kind: 'timeskip' }, scene: { era: 'space', label: 'x' } }, 5);
  assert.match(el.textContent, /the years glow/);
});

test('each listed scene reacts to the one-shot surprise and the continuous joy buff', () => {
  for (const f of ['src/eras/stone.js', 'src/eras/industry.js', 'src/eras/space.js', 'src/eras/timeskip.js', 'src/civ/sandbox.js', 'src/cosmos/surface.js']) {
    const src = read(f);
    assert.match(src, /from '\.\.\/camera\/mood-actions\.js'/, `${f} uses mood-actions`);
    assert.match(src, /surpriseFires\(mood\)/, `${f} fires its surprise on the event`);
    assert.doesNotMatch(src, /mood\.dominant === 'surprise'/, `${f} must not re-fire while a face stays surprised`);
  }
  for (const f of ['src/eras/stone.js', 'src/eras/industry.js', 'src/eras/space.js', 'src/eras/timeskip.js', 'src/civ/sandbox.js']) {
    assert.match(read(f), /joyBuff\(this\.G\.mood\)|joyBuff\(G\?\.mood\)|joyBuff\(this\.G\?\.mood\)/, `${f} reads the joy buff every frame`);
  }
  assert.match(read('src/cosmos/surface.js'), /sc\.G\.mood = /, 'the surface hands the mood to its tribe');
});

test('the camera gates surprise by the cooldown and uses the shared cause lines', () => {
  const cam = read('src/camera/camera.js');
  assert.match(cam, /surpriseReady\(nowSec, surpriseAt\)/);
  assert.match(cam, /surpriseCause\(kind, source\)/);
  assert.match(cam, /joyCause\(kind, source\)/);
  assert.match(cam, /surpriseAt, kind: sceneKind\(G\.scene\) \};/, 'G.mood carries the last surprise time and the scene kind for the HUD');
  assert.doesNotMatch(cam, /coach\?\.(open|setOpen)\(/, 'the coach is never auto-opened');
  assert.match(read('index.html'), /id="moodChip"/);
});
