// The aim cursor (#301): one high-contrast reticle for every view, drawn last on the
// full-resolution text layer, so night, weather and flashes never wash it out and 3D never shrinks
// or tints it. A bright warm core (2 units) on a dark outline (1 unit), four arms with a gap round
// a small centre dot, and a thin ring that pulses a unit (static under reduced motion). `f` is the
// unit in screen pixels (main.js uses half a game pixel), so it stays crisp and scales with the
// display. Size S/M/L and high contrast come from Settings.

export const RETICLE_SIZES = { s: 0.75, m: 1, l: 1.4 };
export const RETICLE_CORE = '#ffd166';
export const RETICLE_EDGE = 'rgba(11,12,22,0.88)';
export const RETICLE_HC = { core: '#ffffff', edge: '#000000' };

// The rectangles [x, y, w, h] to fill, outline first, then core, around the aim point (x, y).
export function reticleRects(x, y, { f = 2, size = 'm', contrast = false, t = 0, reduced = false } = {}) {
  f = Math.max(1, Math.round(f));
  const s = RETICLE_SIZES[size] ?? 1;
  const cx = Math.round(x), cy = Math.round(y);
  const arm = Math.max(4, Math.round(9 * s)) * f, gap = Math.max(2, Math.round(3 * s)) * f;
  const th = 2 * f;                                   // core thickness
  const pulse = reduced ? 0 : Math.round(Math.sin(t * Math.PI * 2 * 1.2));
  const R = (Math.round(11 * s) + pulse) * f;
  const core = [], edge = [];
  const box = (rx, ry, w, h) => { core.push([rx, ry, w, h]); edge.push([rx - f, ry - f, w + 2 * f, h + 2 * f]); };
  const h = th / 2;
  box(cx - h, cy - gap - arm, th, arm);               // up
  box(cx - h, cy + gap, th, arm);                     // down
  box(cx - gap - arm, cy - h, arm, th);               // left
  box(cx + gap, cy - h, arm, th);                     // right
  box(cx - h, cy - h, th, th);                        // centre dot, on the exact aim point
  // A thin stepped ring: one-unit dots (with their outline) round the arms.
  const n = Math.max(16, Math.round((2 * Math.PI * R) / (2 * f)));
  const seen = new Set();
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const px = cx + Math.round((Math.cos(a) * R) / f) * f, py = cy + Math.round((Math.sin(a) * R) / f) * f;
    // Leave gaps where the arms cross the ring, so the cross reads first.
    if (Math.abs(px - cx) < th + f || Math.abs(py - cy) < th + f) continue;
    const id = `${px},${py}`;
    if (seen.has(id)) continue;
    seen.add(id);
    core.push([px, py, f, f]);
    edge.push([px - f, py - f, 3 * f, 3 * f]);
  }
  return { core, edge, colors: contrast ? RETICLE_HC : { core: RETICLE_CORE, edge: RETICLE_EDGE } };
}

export function drawAimReticle(ctx, x, y, opts = {}) {
  const { core, edge, colors } = reticleRects(x, y, opts);
  ctx.fillStyle = colors.edge;
  for (const r of edge) ctx.fillRect(r[0], r[1], r[2], r[3]);
  ctx.fillStyle = colors.core;
  for (const r of core) ctx.fillRect(r[0], r[1], r[2], r[3]);
}
