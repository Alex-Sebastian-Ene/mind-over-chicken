// Epic #220 + #166: the space war presses your world as hard as the difficulty says.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { DIFFICULTY } from '../src/game/options.js';

test('the galaxy hands the difficulty\'s space pressure to the war (Easy 0.5, Normal 1, Hard 1.6)', async () => {
  const { SpaceWarView } = await import('../src/cosmos/space-war.js');
  const got = [];
  const view = Object.create(SpaceWarView.prototype);
  view.w = { war_set_player_pressure: (x) => got.push(x) };
  for (const id of ['easy', 'normal', 'hard']) view.setPlayerPressure(DIFFICULTY[id].space.pressure);
  assert.deepEqual(got, [0.5, 1, 1.6]);
  const cosmos = readFileSync(new URL('../src/cosmos/cosmos.js', import.meta.url), 'utf8');
  assert.match(cosmos, /this\.galaxy\.war\?\.setPlayerPressure\(G\.difficulty\?\.space\?\.pressure \?\? 1\)/);
});

test('war news about your world goes on the one news ticker', () => {
  const main = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.match(main, /kind === 'war'\)[^\n]*\n\s*G\?\.news\?\.push\(text, \{ tone, source: 'space' \}\)/);
});
