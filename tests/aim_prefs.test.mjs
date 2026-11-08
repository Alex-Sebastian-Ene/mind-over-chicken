// Head-aim settings survive a reload (src/ui/eeg-panel.js, #186).
import test from 'node:test';
import assert from 'node:assert/strict';
import { loadAimPrefs, saveAimPrefs, AIM_PREFS_KEY } from '../src/ui/eeg-panel.js';

const memory = () => { const m = new Map(); return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, String(v)), m }; };

test('aim settings round-trip, and bad values fall back to the defaults', () => {
  const s = memory();
  assert.deepEqual(loadAimPrefs(s), { sensitivity: 1, invertYaw: false, invertPitch: false });
  saveAimPrefs({ sensitivity: 1.7, invertYaw: true, invertPitch: false }, s);
  assert.deepEqual(loadAimPrefs(s), { sensitivity: 1.7, invertYaw: true, invertPitch: false });
  s.setItem(AIM_PREFS_KEY, '{"sensitivity": 9, "invertYaw": "yes"}');
  assert.deepEqual(loadAimPrefs(s), { sensitivity: 2.5, invertYaw: false, invertPitch: false }, 'clamped to the slider, strict booleans');
  s.setItem(AIM_PREFS_KEY, 'not json');
  assert.equal(loadAimPrefs(s).sensitivity, 1);
  assert.deepEqual(loadAimPrefs(null), { sensitivity: 1, invertYaw: false, invertPitch: false }, 'no storage (private mode)');
  assert.doesNotThrow(() => saveAimPrefs({ sensitivity: 1 }, { setItem() { throw new Error('quota'); } }));
});
