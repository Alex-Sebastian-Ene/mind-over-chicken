// How far the 3D camera may pull back over the home world before the globe takes over. On the
// medium world (640×360) the hand-over is at 344, where the top-down view frames the 320×180
// window (main.js CAM3D.topDown). Bigger worlds let you see more of their surface first, up to
// twice as far on Huge (Alex: "for larger maps you should be able to zoom more before you go
// planetary"); smaller worlds keep the medium hand-over. The engine allows up to 900.
export const TOP_DOWN = 344;
export const MEDIUM_WIDTH = 640;
export const MAX_SURFACE_SCALE = 2;

export function surfaceMaxDistance(worldWidth = MEDIUM_WIDTH, topDown = TOP_DOWN) {
  const w = Number(worldWidth);
  const s = Number.isFinite(w) && w > 0 ? Math.min(MAX_SURFACE_SCALE, Math.max(1, w / MEDIUM_WIDTH)) : 1;
  return Math.round(topDown * s);
}
