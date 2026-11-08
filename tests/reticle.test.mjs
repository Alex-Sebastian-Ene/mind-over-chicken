// #301: one high-contrast aim cursor in every view, drawn once and last.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { reticleRects, RETICLE_HC, RETICLE_CORE } from '../src/render/reticle.js';

const extent = (rs) => {
  const xs = rs.flatMap((r) => [r[0], r[0] + r[2]]), ys = rs.flatMap((r) => [r[1], r[1] + r[3]]);
  return { w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys) };
};

test('a bright core on a dark outline, at least 16 screen px even small at 1x', () => {
  const r = reticleRects(100, 100, { f: 1, size: 's', reduced: true });
  const e = extent(r.edge);
  assert.ok(e.w >= 16 && e.h >= 16, `${e.w}×${e.h}`);
  assert.equal(r.colors.core, RETICLE_CORE);
  assert.ok(r.edge.length >= r.core.length, 'every core piece has an outline');
  // Outline drawn under the core: each core rect sits inside some edge rect.
  for (const c of r.core) assert.ok(r.edge.some((d) => d[0] <= c[0] && d[1] <= c[1] && d[0] + d[2] >= c[0] + c[2] && d[1] + d[3] >= c[1] + c[3]));
  assert.ok(r.core.every((c) => c[2] >= 1 && c[3] >= 1), 'never thinner than a screen pixel');
});

test('the centre dot sits on the exact aim point; sizes grow; high contrast is white on black', () => {
  const r = reticleRects(50, 60, { f: 2, reduced: true });
  assert.ok(r.core.some((c) => c[0] <= 50 && c[0] + c[2] >= 50 && c[1] <= 60 && c[1] + c[3] >= 60));
  const w = (size) => extent(reticleRects(0, 0, { f: 2, size, reduced: true }).edge).w;
  assert.ok(w('s') < w('m') && w('m') < w('l'));
  assert.deepEqual(reticleRects(0, 0, { contrast: true }).colors, RETICLE_HC);
});

test('the ring pulses a unit, except under reduced motion', () => {
  const key = (o) => JSON.stringify(reticleRects(0, 0, { f: 2, ...o }).core);
  assert.ok(new Set([0, 0.2, 0.4, 0.6].map((t) => key({ t }))).size > 1);
  assert.equal(new Set([0, 0.2, 0.4, 0.6].map((t) => key({ t, reduced: true }))).size, 1);
});

test('drawn once, last, by main.js; no scene or the globe draws its own; one cursor over the map', async () => {
  const read = (p) => readFile(new URL(`../${p}`, import.meta.url), 'utf8');
  const main = await read('src/main.js');
  assert.match(main, /drawAimReticle\(textCtx/);
  assert.ok(main.lastIndexOf('drawAimReticle(') > main.lastIndexOf('cosmos.overlay(textCtx'), 'after the globe/galaxy overlay');
  for (const f of ['src/eras/stone.js', 'src/eras/industry.js', 'src/eras/space.js', 'src/cosmos/surface.js', 'src/civ/sandbox.js', 'src/cosmos/cosmos.js']) {
    const src = await read(f);
    assert.ok(!/drawReticle\(ctx/.test(src), `${f} draws its own reticle`);
    assert.ok(!/add_billboard\(13,/.test(src), `${f} stands up a reticle billboard`);
  }
  const css = await read('styles.css');
  assert.match(css, /body\[data-reticle="1"\] #screen \{ cursor: none; \}/);
});
