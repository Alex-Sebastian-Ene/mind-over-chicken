// #201: the 3D sun rises and sets, nights are moonlit but readable, era darkness wins.
import test from 'node:test';
import assert from 'node:assert/strict';
import { dayNight, DAY_SECONDS, NIGHT_MAX } from '../src/render/daynight.js';

test('the run starts in daylight and the sun later sets', () => {
  const start = dayNight(0);
  assert.ok(start.elev > 0.4 && start.night === 0, `start ${JSON.stringify(start)}`);
  const elevs = Array.from({ length: 60 }, (_, i) => dayNight((i / 60) * DAY_SECONDS).elev);
  assert.ok(Math.min(...elevs) < -0.9 && Math.max(...elevs) > 0.9, 'a full day goes from noon to midnight');
});

test('night deepens after dusk but stays readable', () => {
  const nights = Array.from({ length: 120 }, (_, i) => dayNight((i / 120) * DAY_SECONDS).night);
  assert.equal(Math.max(...nights), NIGHT_MAX);
  assert.ok(nights.filter((n) => n === 0).length > 30, 'most of the day is plain daylight');
});

test("an era's own darkness wins over the clock", () => {
  assert.equal(dayNight(0, 0.6).night, 0.6);
  assert.equal(dayNight(DAY_SECONDS / 2, 0.9).night, 0.9);
});

test('the cycle repeats', () => {
  const a = dayNight(12), b = dayNight(12 + DAY_SECONDS);
  assert.ok(Math.abs(a.elev - b.elev) < 1e-9 && a.night === b.night);
});
