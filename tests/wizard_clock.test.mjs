// The calibration wizard's clock (src/ui/eeg-panel.js PausableClock, #176): it stands still while
// the headset isn't streaming, so a phase can't run out or finish on a dead stream.
import test from 'node:test';
import assert from 'node:assert/strict';
import { PausableClock } from '../src/ui/eeg-panel.js';

test('the wizard clock stops while the stream is lost and resumes where it was', () => {
  let real = 1000;
  const c = new PausableClock(() => real);
  real += 2000; assert.equal(c.tick(true), 3000);
  real += 5000; assert.equal(c.tick(false), 3000, 'five seconds without frames did not count');
  real += 5000; assert.equal(c.tick(false), 3000);
  assert.equal(c.now(), 3000);
  real += 1000; assert.equal(c.tick(true), 4000, 'back on the stream, the phase carries on');
});
