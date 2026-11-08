// Visible aim boundary (#294): every blink/click target is resolved from the active view at the
// moment of the action, then clamped to what that view can actually show. Pure functions, no DOM.
//
// The game has two aim spaces:
//   - the home campaign (2D map and 3D surface) uses 0..1 of the whole home world;
//   - the globe/galaxy, an AI world's surface and the civ sandbox use 0..1 of the screen.
// `aimViewKind` names the active space; the clamps keep the cursor and the strike on screen.

import { clamp, wrapDelta } from '../sim/util.js';

export const AIM_VIEW = Object.freeze({
  NONE: 'none',
  HOME_2D: 'home2d',
  HOME_3D: 'home3d',
  SURFACE: 'surface',
  CIV: 'civ',
  COSMOS: 'cosmos',
});

export const SCREEN_VIEWS = new Set([AIM_VIEW.SURFACE, AIM_VIEW.CIV, AIM_VIEW.COSMOS]);
export const HOME_VIEWS = new Set([AIM_VIEW.HOME_2D, AIM_VIEW.HOME_3D]);

const wrap = (v, size) => ((v % size) + size) % size;

// Which coordinate space the active view uses. `cosmos` means the globe/galaxy (z > 0.5), where
// the reticle is a screen pointer; an AI world's surface and the civ sandbox are screen views too.
export function aimViewKind({ hasScene = true, cosmos = false, surface = false, civ = false, is3d = false } = {}) {
  if (!hasScene) return AIM_VIEW.NONE;
  if (cosmos) return AIM_VIEW.COSMOS;
  if (surface) return AIM_VIEW.SURFACE;
  if (civ) return AIM_VIEW.CIV;
  if (is3d) return AIM_VIEW.HOME_3D;
  return AIM_VIEW.HOME_2D;
}

// Screen aim: 0..1 of the 320×180 viewport, with a small inset so the reticle is not cut in half
// at the edge. Returns `clamped` so callers can show edge feedback if they want it.
export function clampScreenAim(aim, { width = 320, height = 180, inset = 6 } = {}) {
  const ix = Math.min(0.45, Math.max(0, inset / Math.max(1, width)));
  const iy = Math.min(0.45, Math.max(0, inset / Math.max(1, height)));
  const x = clamp(Number(aim?.x) || 0, ix, 1 - ix);
  const y = clamp(Number(aim?.y) || 0, iy, 1 - iy);
  return { x, y, clamped: x !== aim?.x || y !== aim?.y };
}

// Home-world aim: 0..1 of the 640×360 world, clamped to the 320×180 camera window. The window
// wraps east-west (and around both seams for the home torus), so this projects world → screen,
// clamps the screen point, then projects back. `left`/`top` come from the Rust ViewCamera, which
// snaps them to whole pixels so the drawn reticle and the strike agree.
export function clampHomeAim(aim, { left = 0, top = 0, viewW = 320, viewH = 180, worldW = 640, worldH = 360, inset = 4 } = {}) {
  const wx = clamp(Number(aim?.x) || 0, 0, 1) * worldW;
  const wy = clamp(Number(aim?.y) || 0, 0, 1) * worldH;
  const midX = left + viewW / 2;
  const midY = top + viewH / 2;
  const sx = clamp(wrapDelta(wx, midX, worldW) + viewW / 2, inset, Math.max(inset, viewW - 1 - inset));
  const sy = clamp(wrapDelta(wy, midY, worldH) + viewH / 2, inset, Math.max(inset, viewH - 1 - inset));
  const vx = wrap(left + sx, worldW);
  const vy = wrap(top + sy, worldH);
  return {
    x: vx / worldW,
    y: vy / worldH,
    clamped: Math.abs(wrapDelta(vx, wx, worldW)) > 0.5 || Math.abs(wrapDelta(vy, wy, worldH)) > 0.5,
  };
}
