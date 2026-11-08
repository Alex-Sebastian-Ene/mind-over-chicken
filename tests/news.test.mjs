// Epic #220: one news ticker for the one game, fed by the goat market and the rival peoples.
import test from 'node:test';
import assert from 'node:assert/strict';
import { News, TONE_COLOUR } from '../src/ui/news.js';
import { bootModeOf } from '../src/game/options.js';

test('news: newest last, repeats dropped, a short backlog, old news leaves the tape', () => {
  const n = new News({ max: 3, keep: 10, dedupe: 5 });
  assert.equal(n.active, false);
  assert.ok(n.push('A'));
  assert.equal(n.push('A'), false, 'the same line twice in a row is one item');
  n.push('B', { tone: 2 });
  assert.equal(n.items[1].tone, 'bad', 'civ TONE numbers map to names');
  n.push('C'); n.push('D');
  assert.deepEqual(n.items.map((i) => i.text), ['B', 'C', 'D'], 'at most `max` items');
  n.update(6);
  assert.ok(n.push('B'), 'after the dedupe window a line may repeat');
  n.update(5);
  assert.deepEqual(n.items.map((i) => i.text), ['B'], 'news older than `keep` leaves');
  assert.equal(n.push('   '), false);
});

test("a scene's headlines channel writes to the one ticker and reads its own lines back", () => {
  const n = new News();
  const goats = n.channel('market', ['EXCHANGE OPENS'], 'market');
  n.push('A rival people settles to the east', { source: 'rivals' });
  goats.push('GOAT INDEX -20%');
  assert.deepEqual(n.items.map((i) => i.source), ['market', 'rivals', 'market']);
  assert.deepEqual(goats.slice(-5), ['EXCHANGE OPENS', 'GOAT INDEX -20%']);
  assert.equal(goats.length, 2);
  assert.equal(n.items[0].tone, 'market');
});

test('the tape draws each line in its tone, and hides when quiet', () => {
  const calls = [];
  const ctx = {
    save() {}, restore() {}, fillRect: (...a) => calls.push(['rect', ...a]), measureText: (t) => ({ width: t.length * 3 }),
    fillText: (t, x) => calls.push(['text', t, x]), set fillStyle(v) { calls.push(['style', v]); }, font: '', textAlign: '',
  };
  const n = new News();
  n.draw(ctx, 320, 180);
  assert.equal(calls.length, 0, 'nothing to say: no tape');
  n.push('Raid!', { tone: 'bad' });
  n.draw(ctx, 320, 180);
  assert.ok(calls.some((c) => c[0] === 'rect' && c[2] === 172), 'a strip along the bottom');
  assert.equal(n.x, 320, 'it starts at the right edge');
  n.update(1);
  assert.equal(n.x, 280, 'and scrolls left');
  n.draw(ctx, 320, 180);
  assert.ok(calls.some((c) => c[0] === 'style' && c[1] === TONE_COLOUR.bad), 'in its tone');
  assert.ok(calls.some((c) => c[0] === 'text' && c[1] === 'Raid!' && c[2] === 280));
});

test('old modes stay reachable by URL: ?mode=civ, ?mode=galaxy, ?mode=story', () => {
  assert.equal(bootModeOf('?mode=civ'), 'crowded');
  assert.equal(bootModeOf('?mode=crowded'), 'crowded');
  assert.equal(bootModeOf('?mode=galaxy'), 'galaxy');
  assert.equal(bootModeOf('?mode=story&size=huge'), 'story');
  assert.equal(bootModeOf('?mode=nope'), null);
  assert.equal(bootModeOf(''), null);
});
