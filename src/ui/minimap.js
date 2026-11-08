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
