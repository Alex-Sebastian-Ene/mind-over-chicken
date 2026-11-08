// The home world's minimap window in 3D (release QA). In 2D the camera shows a fixed 320×180
// window, but the 3D camera zooms, tilts and orbits, so its window is the patch of ground the
// screen's corners look at: a trapezoid that shrinks as you zoom in and turns as you orbit.
//
// `groundAt(sx, sy)` raycasts a screen point (0..sw, 0..sh) to the ground: [x, z] in world px,
// wrapped into the world, or null / negative when the ray misses (sky above a tilted horizon).
// Returns the four corners (top-left, top-right, bottom-right, bottom-left) unwrapped around the
// camera target (tx, tz) so the shape never tears at a seam, or null when the ground is not seen.
export function footprint3d(groundAt, { tx, tz, ww, wh, sw, sh }) {
  const unwrap = (v, c, n) => { let d = (v - c) % n; if (d > n / 2) d -= n; else if (d < -n / 2) d += n; return c + d; };
  const hit = (sx, sy) => {
    const g = groundAt(sx, sy);
    if (!g || !(g[0] >= 0) || !(g[1] >= 0)) return null;
    return [unwrap(g[0], tx, ww), unwrap(g[1], tz, wh)];
  };
  // A top corner over the horizon: walk down its side until the ray meets the ground.
  const corner = (sx, fromTop) => {
    for (let k = 0; k <= 10; k++) {
      const p = hit(sx, fromTop ? (sh * k) / 20 : sh - (sh * k) / 20);
      if (p) return p;
    }
    return null;
  };
  const pts = [corner(0, true), corner(sw, true), corner(sw, false), corner(0, false)];
  return pts.every(Boolean) ? pts : null;
}


// A minimap-sized, single-copy camera window is only useful when the four ground corners are
// real points on this world and form a normal quad. At a low pitch the top corners can miss the
// ground entirely; walking down to the first hit then lands on the wrapped side of the world,
// and drawing all wrap copies made a giant self-intersecting shape. Reject those footprints so
// the caller can fall back to the stable 2D window box.
export function minimapWindow3d(groundAt, opts) {
  const pts = footprint3d(groundAt, opts);
  if (!pts) return null;
  const { ww, wh } = opts;
  const eps = 0.5;   // the Rust camera snaps window edges to whole pixels
  if (!pts.every(([x, y]) => Number.isFinite(x) && Number.isFinite(y) && x >= -eps && x <= ww + eps && y >= -eps && y <= wh + eps)) return null;
  const [a, b, c, d] = pts;
  const cross = (p, q, r) => Math.sign((q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]));
  const crosses = (p, q, r, s) => cross(p, q, r) * cross(p, q, s) < 0 && cross(r, s, p) * cross(r, s, q) < 0;
  if (crosses(a, b, c, d) || crosses(b, c, d, a)) return null;
  return pts;
}

// The 2D camera window as one clamped box, a stable fallback at low pitch or when the 3D ground
// quad runs off the map. `vw`/`vh` are the home view's world-pixel size (320×180).
export function viewWindow(cx, cy, ww, wh, vw = 320, vh = 180) {
  const w = Math.min(vw, ww), h = Math.min(vh, wh);
  const x = Math.min(Math.max(cx - w / 2, 0), Math.max(0, ww - w));
  const y = Math.min(Math.max(cy - h / 2, 0), Math.max(0, wh - h));
  return { x, y, w, h };
}
