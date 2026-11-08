// Sidebar hotkeys (#211): E and C toggle the EEG and Camera sidebars, Shift+C re-centres the EEG
// head pose, ` stays a Camera alias, and none of them fire behind an overlay.
// Run: node --test tests/hotkeys.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { panelHotkey, overlayBlocksHotkeys, HOTKEY_OVERLAYS } from '../src/ui/hotkeys.js';

const key = (code, mods = {}) => ({ code, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, repeat: false, ...mods });

test('C toggles the Camera sidebar, Shift+C re-centres the headset, E toggles EEG (#211)', () => {
  assert.equal(panelHotkey(key('KeyC')), 'camera');
  assert.equal(panelHotkey(key('KeyC', { shiftKey: true })), 'eegCenter');
  assert.equal(panelHotkey(key('KeyE')), 'eeg');
  // C never re-centres; Shift+C never opens the camera.
  assert.notEqual(panelHotkey(key('KeyC')), 'eegCenter');
  assert.notEqual(panelHotkey(key('KeyC', { shiftKey: true })), 'camera');
  // Browser shortcuts and key repeat are left alone.
  for (const mods of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { repeat: true }]) {
    assert.equal(panelHotkey(key('KeyC', mods)), null);
    assert.equal(panelHotkey(key('KeyE', mods)), null);
  }
  assert.equal(panelHotkey(key('KeyM')), null);
});

function fakeDoc({ visible = [], wizard = false } = {}) {
  return {
    getElementById: (id) => (HOTKEY_OVERLAYS.includes(id) ? { hidden: !visible.includes(id) } : null),
    querySelector: (sel) => (sel === '.calib-wiz' ? { hidden: !wizard } : null),
  };
}

test('overlays hold the sidebar hotkeys (#211, #171-#173)', () => {
  assert.equal(overlayBlocksHotkeys(fakeDoc()), false);
  for (const id of ['titleScreen', 'planetSetup', 'failScreen', 'endScreen', 'banner']) {
    assert.equal(overlayBlocksHotkeys(fakeDoc({ visible: [id] })), true, id);
  }
  assert.equal(overlayBlocksHotkeys(fakeDoc({ wizard: true })), true, 'calibration wizard');
});

test('every label documents C / Shift+C (#211)', () => {
  const read = (f) => readFileSync(new URL(`../${f}`, import.meta.url), 'utf8');
  const html = read('index.html');
  assert.match(html, /id="camBtn"[^>]*title="[^"]*\(C\)"[^>]*>Camera <kbd>C<\/kbd>/);
  assert.match(html, /<kbd>C<\/kbd> Camera/, 'the Settings key list names C (the title screen has no hint now)');
  const cam = read('src/camera/camera.js');
  assert.doesNotMatch(cam, /Camera(?: on|…)? <kbd>`<\/kbd>/, 'dynamic button label uses C');
  assert.match(cam, /'Camera <kbd>C<\/kbd>'/);
  const eeg = read('src/ui/eeg-panel.js');
  assert.match(eeg, /press Shift\+C\)[^>]*>Center <kbd>Shift\+C<\/kbd>/);
  const main = read('src/main.js');
  assert.doesNotMatch(main, /e\.code === 'KeyC'\) \{ eegPanel\?\.calibrateCenter/, 'plain C no longer re-centres');
  assert.match(read('docs/demo.md'), /`Shift\+C`/);
});
