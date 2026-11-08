// civ (#230): seam helpers for the civ sandbox's torus planet (DOM-free, tested in
// tests/civ_seams.test.mjs).

// The pieces of the span [start, start + len) on a wrapping axis of length n (#230): one, or two
// where it crosses the seam, each [from, width] with 0 <= from < n and width > 0.
export function seamSpans(start, len, n) {
  const a = ((start % n) + n) % n, w = Math.min(len, n);
  return a + w > n ? [[a, n - a], [0, a + w - n]] : [[a, w]];
}

// The 3D relief is the 320×180 view, and the engine lays copies of it round itself (#230): fade
// a rim of the relief down to the sea so neighbouring copies meet at the waterline instead of
// as a cliff where the view's east edge abuts its own west edge.
export const RELIEF_RIM = 12;
export const RELIEF_SEA = 0.18;
export function fadeReliefRim(h, w, hgt, rim = RELIEF_RIM, sea = RELIEF_SEA) {
  for (let y = 0; y < hgt; y++) {
    const ey = Math.min(y, hgt - 1 - y);
    for (let x = 0; x < w; x++) {
      const e = Math.min(ey, x, w - 1 - x);
      if (e >= rim) { if (x < rim) x = w - rim - 1; continue; }
      const k = e / rim, s = k * k * (3 - 2 * k), i = y * w + x;
      h[i] = sea + (h[i] - sea) * s;
    }
  }
  return h;
}
