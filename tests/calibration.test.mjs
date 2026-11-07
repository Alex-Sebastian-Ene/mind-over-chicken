// Calibration seed (#54): config parsing, reproducible random setup, the sensor formula,
// and the window itself. Run: node --test tests/calibration.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Calibration, readCalibrationConfig, randomSeedTraits, traitsFromFeatures, CALIBRATION_DEFAULTS } from '../src/brain/calibration.js';

const noStorage = { getItem: () => null, setItem: () => {} };

test('defaults: 10 s placeholder window, random dummy setup', () => {
  const c = readCalibrationConfig('', noStorage);
  assert.equal(c.seconds, CALIBRATION_DEFAULTS.seconds);
  assert.equal(c.mode, 'random');
  assert.equal(c.seedFromConfig, false);
});

test('window, mode and seed are configurable without code edits', () => {
  const c = readCalibrationConfig('?calib=4&calib_mode=sensor&seed=42', noStorage);
  assert.deepEqual([c.seconds, c.mode, c.seed, c.seedFromConfig], [4, 'sensor', 42, true]);
  assert.equal(readCalibrationConfig('?calib=999', noStorage).seconds, 120);
  assert.equal(readCalibrationConfig('?calib=abc', noStorage).seconds, 10);
  const saved = { getItem: () => JSON.stringify({ seconds: 6, mode: 'sensor' }) };
  assert.deepEqual([readCalibrationConfig('', saved).seconds, readCalibrationConfig('', saved).mode], [6, 'sensor']);
});

test('random setup is reproducible from the run seed and differs between seeds', () => {
  assert.deepEqual(randomSeedTraits(1234), randomSeedTraits(1234));
  assert.notDeepEqual(randomSeedTraits(1234), randomSeedTraits(1235));
  for (const v of Object.values(randomSeedTraits(99))) assert.ok(v >= 0 && v <= 1);
});

test('sensor formula: calm and joy read friendly; stress and anger read aggressive and violent', () => {
  const serene = traitsFromFeatures({ calm: 0.9, focus: 0.2, stress: 0.1, blinkRate: 1, joy: 0.8, anger: 0 });
  const furious = traitsFromFeatures({ calm: 0.05, focus: 0.6, stress: 0.95, blinkRate: 8, joy: 0, anger: 0.9 });
  assert.ok(serene.friendliness > 0.7 && serene.aggressiveness < 0.15 && serene.violence < 0.1, JSON.stringify(serene));
  assert.ok(furious.friendliness < 0.15 && furious.aggressiveness > 0.7 && furious.violence > 0.7, JSON.stringify(furious));
});

test('the window collects features for its configured length, then seeds once', () => {
  const brain = { calm: 0.8, focus: 0.3, stress: 0.2, stats: { blinks: 0 } };
  const cal = new Calibration({ seconds: 3, mode: 'sensor', seed: 7 });
  let seeded = 0;
  cal.onDone(() => seeded++);
  for (let i = 0; i < 2.9 / 0.1; i++) { cal.update(0.1, brain, null); brain.stats.blinks += i % 10 === 0 ? 1 : 0; }
  assert.equal(cal.done, false);
  cal.update(0.0, brain, null);            // paused frames don't count
  assert.equal(cal.done, false);
  cal.update(0.2, brain, null);
  assert.equal(cal.done, true);
  assert.equal(seeded, 1);
  const r = cal.result;
  assert.equal(r.source, 'sensor');
  assert.equal(r.seconds, 3);
  assert.ok(Math.abs(r.features.calm - 0.8) < 1e-6);
  assert.ok(Math.abs(r.features.stress - 0.2) < 1e-6);
  assert.equal('hrDelta' in r.features, false);
  assert.ok(r.features.blinkRate > 0);
  assert.ok(r.friendliness > r.aggressiveness);
  cal.update(1, brain, null);              // no re-seeding after done
  assert.equal(seeded, 1);
});

test('random mode ignores the signals and uses the run seed', () => {
  const cal = new Calibration({ seconds: 1, mode: 'random', seed: 2024 });
  cal.update(1, { calm: 0, focus: 1, stress: 1, stats: { blinks: 0 } }, null);
  const { friendliness, aggressiveness, violence } = cal.result;
  assert.deepEqual({ friendliness, aggressiveness, violence }, randomSeedTraits(2024));
  assert.equal(cal.result.source, 'random');
});
