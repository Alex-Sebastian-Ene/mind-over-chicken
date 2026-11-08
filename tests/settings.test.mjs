// Settings (src/ui/settings.js, #186): saved, validated, and the reduced-motion rule.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { loadSettings, saveSettings, reducedMotionOn, SETTINGS_KEY, SETTINGS_DEFAULTS } from '../src/ui/settings.js';
import { readCalibrationConfig, saveCalibrationConfig } from '../src/brain/calibration.js';

const memory = () => { const m = new Map(); return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, String(v)) }; };

test('settings round-trip and fall back to the defaults', () => {
  const s = memory();
  assert.deepEqual(loadSettings(s), { ...SETTINGS_DEFAULTS });
  saveSettings({ reducedMotion: true, cameraOnStart: true }, s);
  assert.deepEqual(loadSettings(s), { ...SETTINGS_DEFAULTS, reducedMotion: true, cameraOnStart: true });
  s.setItem(SETTINGS_KEY, '{"reducedMotion": "yes", "cameraOnStart": 1}');
  assert.deepEqual(loadSettings(s), { ...SETTINGS_DEFAULTS });
  s.setItem(SETTINGS_KEY, '{oops');
  assert.deepEqual(loadSettings(s), { ...SETTINGS_DEFAULTS });
  assert.deepEqual(loadSettings(null), { ...SETTINGS_DEFAULTS });
});

test('the camera starts with a game by default; an explicit opt-out is respected', () => {
  const s = memory();
  assert.equal(loadSettings(s).cameraOnStart, true);
  assert.equal(loadSettings(null).cameraOnStart, true);
  s.setItem(SETTINGS_KEY, '{"cameraOnStart":false}');
  assert.equal(loadSettings(s).cameraOnStart, false);
});

test('reduced motion follows the system until the player chooses', () => {
  assert.equal(reducedMotionOn({ reducedMotion: null }, true), true);
  assert.equal(reducedMotionOn({ reducedMotion: null }, false), false);
  assert.equal(reducedMotionOn({ reducedMotion: false }, true), false);
  assert.equal(reducedMotionOn({ reducedMotion: true }, false), true);
});

test('the calibration window chosen in Settings is what the next run reads', () => {
  const s = memory();
  saveCalibrationConfig({ seconds: 25, mode: 'sensor' }, s);
  const c = readCalibrationConfig('', s);
  assert.equal(c.seconds, 25);
  assert.equal(c.mode, 'sensor');
  assert.equal(readCalibrationConfig('?calib=5', s).seconds, 5, 'the URL still wins');
});

test('the top bar is EEG, 3D, AI God, Sound and Settings; Camera, Model and Debug are in Settings (#89)', () => {
  const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  const bar = html.slice(html.indexOf('<div class="bar-tools">'), html.indexOf('</header>'));
  for (const id of ['eegBtn', 'mode3dBtn', 'aiGodBtn', 'soundBtn', 'settingsBtn']) assert.match(bar, new RegExp(`id="${id}"`), id);
  for (const id of ['camBtn', 'modelBtn', 'dbgBtn']) assert.doesNotMatch(bar, new RegExp(`id="${id}"`), `${id} left the top bar`);
  const panel = html.slice(html.indexOf('id="settingsPanel"'), html.indexOf('</aside>', html.indexOf('id="settingsPanel"')));
  for (const id of ['camBtn', 'modelBtn', 'dbgBtn', 'setReducedMotion', 'setAimSens', 'setCalibSeconds', 'setCalibMode', 'setCameraOnStart', 'settingsReset']) assert.match(panel, new RegExp(`id="${id}"`), id);
});

test('the cursor size and high contrast are saved, and bad values fall back (#301)', () => {
  const s = memory();
  saveSettings({ ...SETTINGS_DEFAULTS, cursorSize: 'l', cursorContrast: true }, s);
  assert.deepEqual(loadSettings(s), { ...SETTINGS_DEFAULTS, cursorSize: 'l', cursorContrast: true });
  s.setItem(SETTINGS_KEY, '{"cursorSize": "xxl", "cursorContrast": "yes"}');
  assert.equal(loadSettings(s).cursorSize, 'm');
  assert.equal(loadSettings(s).cursorContrast, false);
});
