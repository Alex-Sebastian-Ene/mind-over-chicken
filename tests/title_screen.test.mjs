// The simple title screen: a short menu, no prose, and the helpers behind Continue and Load.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { MAIN_MENU, menuState, saveLabel, stepIndex, timeAgo } from '../src/ui/title.js';

const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
const title = html.slice(html.indexOf('id="titleScreen"'), html.indexOf('</section>', html.indexOf('id="titleScreen"')));
const view = (name) => {
  const at = title.indexOf(`data-title-view="${name}"`);
  assert.ok(at > 0, `${name} view`);
  const next = title.indexOf('data-title-view=', at + 1);
  return title.slice(at, next > 0 ? next : undefined);
};
const text = (s) => s.replace(/<!--[\s\S]*?-->/g, '').replace(/<kbd>[^<]*<\/kbd>/g, '').replace(/<[^>]+>/g, ' ').replace(/&\w+;/g, ' ').replace(/\s+/g, ' ').trim();
const buttons = (s) => [...s.matchAll(/<button\b[^>]*>([\s\S]*?)<\/button>/g)].map((m) => text(m[1].replace(/<span[\s\S]*?<\/span>/g, '')));

test('the main menu is exactly Continue, New Game, Load, Sections, Calibrate, Options', () => {
  assert.deepEqual(MAIN_MENU, ['Continue', 'New Game', 'Load', 'Sections', 'Calibrate', 'Options']);
  assert.deepEqual(buttons(view('main')), MAIN_MENU);
  // Continue and Load start hidden: they appear once a run is saved.
  for (const k of ['continue', 'load']) assert.match(view('main'), new RegExp(`data-title-menu="${k}"[^>]*\\bhidden\\b`), k);
  // New Game opens its step (world size, difficulty, #220); Options opens the Settings panel.
  assert.match(view('main'), /id="playSim"[^>]*data-title-menu="new"[^>]*data-title-open="new"/);
  assert.match(view('main'), /id="titleSettings"[^>]*data-title-menu="options"/);
});

test('no long prose: no era list, no hints, a short tagline', () => {
  assert.doesNotMatch(title, /class="eras"|class="hint"|class="lede"|class="judges"|<details/);
  assert.doesNotMatch(title, /placeholder|emulated headset|No headset or camera needed/i);
  const tagline = /<p class="tagline">([^<]*)<\/p>/.exec(title);
  assert.ok(tagline, 'tagline');
  assert.ok(tagline[1].split(/\s+/).length <= 8, 'tagline is at most 8 words');
  for (const [, p] of title.matchAll(/<p\b[^>]*>([\s\S]*?)<\/p>/g)) assert.ok(text(p).split(' ').length <= 8, `short line: ${text(p)}`);
  assert.ok(text(title).split(' ').length <= 110, `the whole title screen is short (${text(title).split(' ').length} words)`);
});

test('Sections jumps to every era, the galaxy and the crowded planet with its size picker', () => {
  const sections = view('sections');
  assert.deepEqual(buttons(sections).slice(0, 7), ['Stone Age', 'Farming → Kingdoms', 'Industry', 'Information Age', 'Space Age', 'Galaxy', 'Crowded planet']);
  assert.deepEqual([...sections.matchAll(/data-section="(\d+)"/g)].map((m) => Number(m[1])), [0, 1, 2, 3, 4]);
  // One era button per scene in main.js.
  const scenes = /const SCENES = \[([^\n]*)\];/.exec(readFileSync(new URL('../src/main.js', import.meta.url), 'utf8'));
  assert.equal(scenes[1].split('=>').length - 1, 5);
  assert.match(sections, /id="playSandbox"[^>]*data-section="galaxy"/);
  assert.match(sections, /id="playCiv"[^>]*data-section="crowded"/);
  assert.match(sections, /id="planetSizePick"[^>]*role="radiogroup"/);
});

test('Calibrate holds the bridge address, Connect, its status and the wizard button', () => {
  const cal = view('calibrate');
  for (const id of ['bridgeUrl', 'playBridge', 'bridgeStatus']) assert.match(cal, new RegExp(`id="${id}"`), id);
  assert.match(cal, /data-title-calibrate[^>]*disabled/);
  assert.match(cal, /aria-live="polite"/);
});

test('every sub-view has a Back button', () => {
  for (const v of ['load', 'sections', 'calibrate']) assert.match(view(v), /data-title-back/, v);
});

test('Continue and Load appear only with saves', () => {
  assert.deepEqual(menuState([]), { continue: false, load: false });
  assert.deepEqual(menuState(null), { continue: false, load: false });
  assert.deepEqual(menuState([{ id: 'a' }]), { continue: true, load: true });
});

test('a save reads as its run, mode, section and age', () => {
  const now = Date.UTC(2026, 10, 8, 12);
  assert.deepEqual(saveLabel({ id: 'a', name: 'Run 3 · Stone Age', runNumber: 3, mode: 'story', sceneLabel: 'Era II · Industry', updatedAt: now - 5 * 60e3 }, now),
    { name: 'Run 3', where: 'Era II · Industry', meta: 'Story · Era II · Industry · 5 min ago' });
  // A sandbox's section repeats the mode ("Sandbox · Galaxy"): shown once.
  assert.deepEqual(saveLabel({ id: 'b', runNumber: 2, mode: 'galaxy', sceneLabel: 'Sandbox · Galaxy', updatedAt: now - 2 * 3600e3 }, now),
    { name: 'Run 2', where: 'Galaxy', meta: 'Galaxy · 2 h ago' });
  assert.equal(saveLabel({ id: 'e', runNumber: 4, mode: 'crowded', sceneLabel: 'Sandbox · A crowded planet' }, now).meta, 'Crowded planet');
  assert.equal(saveLabel({ id: 'c', name: 'Run 7 · Space Age', mode: 'crowded' }, now).name, 'Run 7');
  assert.equal(saveLabel({ id: 'd' }, now).name, 'Run');
});

test('time ago', () => {
  const now = 1_800_000_000_000;
  assert.equal(timeAgo(now - 10e3, now), 'just now');
  assert.equal(timeAgo(now - 3 * 60e3, now), '3 min ago');
  assert.equal(timeAgo(now - 5 * 3600e3, now), '5 h ago');
  assert.equal(timeAgo(now - 86400e3, now), '1 day ago');
  assert.equal(timeAgo(now - 3 * 86400e3, now), '3 days ago');
  assert.equal(timeAgo(undefined, now), '');
});

test('arrow keys wrap around the menu', () => {
  assert.equal(stepIndex(0, 4, 1), 1);
  assert.equal(stepIndex(3, 4, 1), 0);
  assert.equal(stepIndex(0, 4, -1), 3);
  assert.equal(stepIndex(-1, 4, 1), 0);
  assert.equal(stepIndex(-1, 4, -1), 3);
  assert.equal(stepIndex(0, 0, 1), -1);
});

test('main.js wires the menu: era sections start at their scene, Connect stays on the title', () => {
  const main = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.match(main, /initTitle\(document,/);
  assert.match(main, /onSection: \(i\) => begin\(playSource\(\), \{ startIndex: i \}\)/);
  assert.match(main, /onContinue: \(save\) => window\.__moc\?\.continueRun\?\.\(save\)/);
  assert.match(main, /startWizard\('both'\)/);
  const connect = main.slice(main.indexOf("$('playBridge').addEventListener"), main.indexOf('initTitle(document,'));
  assert.doesNotMatch(connect, /begin\(/, 'Connect no longer starts a game');
  assert.match(connect, /setGameSource\(src\)/);
});
