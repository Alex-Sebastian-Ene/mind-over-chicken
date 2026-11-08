import test from 'node:test';
import assert from 'node:assert/strict';
import { Advisor } from '../src/ui/advisor.js';

function fakeRoot() {
  const el = () => ({ textContent: '' });
  const nodes = { '[data-advisor-name]': el(), '[data-advisor-text]': el() };
  return {
    classList: { add() {}, remove() {} },
    dataset: {},
    querySelector: (s) => nodes[s] || { getContext: () => ({}) },
  };
}

test('advisor: same-key lines fire again after an era change (#184)', () => {
  const a = new Advisor(fakeRoot());
  assert.equal(a.say('intro', 'Stone intro', { cooldown: 999 }), true);
  assert.equal(a.say('intro', 'Stone intro again', { cooldown: 999 }), false);
  a.setEra('industry');
  assert.equal(a.say('intro', 'Industry intro', { cooldown: 999 }), true);
  a.setEra('space');
  assert.equal(a.say('intro', 'Space intro', { cooldown: 999 }), true);
  assert.equal(a.say('win', 'Space win', { cooldown: 999 }), true);
});
