// The fail screen fails a run once (src/ui/fail.js, #172). The Rust era sim keeps reporting its
// failure until the era restarts; the frame loop used to re-open the fail screen whenever it was
// hidden, so Controls (which hides it) bounced straight back and Retry stranded the new run.
import test from 'node:test';
import assert from 'node:assert/strict';
import { engineFailToShow } from '../src/ui/fail.js';

const failedEngine = (reason) => ({ is_game_era_failed: () => reason });

test('a failed engine era opens the fail screen once per run', () => {
  const engine = failedEngine('The tribe froze in the dark: blink on the fire pit.');
  const G = { finished: false };
  assert.match(engineFailToShow(engine, G), /froze/);
  G.failing = true;            // showFail() opened it
  // Controls hides the fail screen; the engine still reports the failure.
  assert.equal(engineFailToShow(engine, G), null);
  G.failing = false;           // Retry restarted the era
  assert.equal(engineFailToShow(failedEngine(null), G), null);
});

test('nothing to show without a run, after the ending, or without the engine API', () => {
  const engine = failedEngine('x');
  assert.equal(engineFailToShow(engine, null), null);
  assert.equal(engineFailToShow(engine, { finished: true }), null);
  assert.equal(engineFailToShow({}, {}), null);
  assert.equal(engineFailToShow(null, {}), null);
});
