// The frame driver (src/ui/loop.js, #171): Quit to Title once threw inside frame() before the next
// requestAnimationFrame was booked, so the rAF loop died for the rest of the session.
import test from 'node:test';
import assert from 'node:assert/strict';
import { frameLoop } from '../src/ui/loop.js';

function fakeRaf() {
  const queue = [];
  return { raf: (cb) => queue.push(cb), queue, step() { const cb = queue.shift(); cb?.(); } };
}

test('a frame that throws still books the next one', () => {
  const r = fakeRaf();
  let n = 0;
  const tick = frameLoop(() => { n++; if (n === 2) throw new Error('G.scene is null'); }, { raf: r.raf });
  tick();
  assert.equal(n, 1);
  assert.equal(r.queue.length, 1);
  assert.throws(() => r.step(), /G.scene is null/);
  assert.equal(r.queue.length, 1, 'the loop is still scheduled after the throw');
  r.step();
  r.step();
  assert.equal(n, 4, 'frames keep coming after the bad one');
});

test('stops when the JS loop hands over (RustApp)', () => {
  const r = fakeRaf();
  let running = true, n = 0;
  const tick = frameLoop(() => { n++; }, { raf: r.raf, running: () => running });
  tick();
  running = false;
  r.step();
  assert.equal(n, 1);
  assert.equal(r.queue.length, 0);
});
