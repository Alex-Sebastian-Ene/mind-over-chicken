// Reduced motion and photosensitivity (src/ui/motion.js, #90).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { reducedMotion, flashAlpha, shakeAmount, FLASH_CAP, FLASH_CAP_REDUCED } from '../src/ui/motion.js';

test('flashes are capped, softer under reduced motion', () => {
  assert.equal(flashAlpha(0.35), 0.35);
  assert.equal(flashAlpha(1), FLASH_CAP);
  assert.equal(flashAlpha(0.35, true), FLASH_CAP_REDUCED);
  assert.equal(flashAlpha(0.05, true), 0.05);
  assert.equal(flashAlpha(NaN), 0);
  assert.equal(flashAlpha(-1), 0);
});

test('no screen shake under reduced motion', () => {
  assert.equal(shakeAmount(10), 0.08);
  assert.equal(shakeAmount(10, true), 0);
  assert.equal(shakeAmount(0), 0);
  assert.equal(shakeAmount(undefined), 0);
});

test('the flag is body[data-reduced-motion="1"], and CSS stops animations under it', () => {
  assert.equal(reducedMotion({ body: { dataset: { reducedMotion: '1' } } }), true);
  assert.equal(reducedMotion({ body: { dataset: { reducedMotion: '0' } } }), false);
  assert.equal(reducedMotion(null), false);
  const css = readFileSync(new URL('../styles.css', import.meta.url), 'utf8');
  assert.match(css, /body\[data-reduced-motion="1"\] \*[^{]*\{[^}]*animation-duration/);
});

test('the render and camera tweens go through the guard (main.js)', () => {
  const src = readFileSync(new URL('../src/main.js', import.meta.url), 'utf8');
  assert.match(src, /flashAlpha\(\(scene\.flash/);
  assert.match(src, /shakeFor\(G\.shake, rm\)/);
  assert.match(src, /is_3d_mode\(\) && !reducedMotion\(\)/, 'no intro camera sweep');
});
