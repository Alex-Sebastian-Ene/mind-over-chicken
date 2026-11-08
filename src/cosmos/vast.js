// Vaster planet surfaces (#68): helpers for the 640×360 surfaces the galaxy's worlds have.
// The surfaces themselves come from Rust (crates/moc-terrain, `World.planet`, #109): a torus
// (it wraps east–west and north–south, #111), with the AI god's own World (the one its Stone
// Age tribe lives on, src/ai/planet.js) cut from it at TILE, so the tribe's paths and the drawn
// land agree pixel for pixel. Pure: no DOM, so it runs in the worldgen worker and in Node tests.

import { T } from '../world/terrain.js';
import { W, H } from '../sim/util.js';

export const VW = W * 2;                         // 640
export const VH = H * 2;                         // 360
export const TILE = { x: 0, y: (VH - H) / 2 };   // capital region: west half, equator band


export const wrapX = (x) => ((x % VW) + VW) % VW;
export const wrapDx = (dx) => dx - VW * Math.round(dx / VW);
export const wrapY = (y) => ((y % VH) + VH) % VH;
export const wrapDy = (dy) => dy - VH * Math.round(dy / VH);

export const walkableType = (t) => t >= T.SAND && t !== T.MOUNTAIN && t !== T.SNOW;

export function typeAt(v, x, y) {
  x = wrapX(Math.round(x)); y = wrapY(Math.round(y));
  return v.type[y * v.w + x];
}
export const walkableAt = (v, x, y) => walkableType(typeAt(v, x, y));

export function nearestLand(v, x, y, maxR = 30) {
  for (let r = 0; r <= maxR; r++) {
    for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) {
      if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
      const t = typeAt(v, x + dx, y + dy);
      if (t === T.GRASS || t === T.SAND || t === T.HILL) return { x: wrapX(x + dx), y: y + dy };
    }
  }
  return null;
}

// Which spots are settled and how big each one is, from the world's tier and people.
// Pure, so the surface, the globe lights and tests agree. size: 0 camp … 3 city.
export const SPREAD = [1, 3, 6, 99];        // most settlements per tier
export const PEOPLE_PER_SITE = [0, 30, 120, 600];
export function settlementPlan(state, spots) {
  if (!state || state.pop <= 0 || !spots?.length) return [];
  const tier = state.tier ?? 0;
  const n = Math.max(1, Math.min(spots.length, SPREAD[tier], 1 + Math.floor(state.pop / Math.max(1, PEOPLE_PER_SITE[tier]))));
  const out = [];
  for (let k = 0; k < n; k++) {
    // The capital is the biggest; the rest shrink with distance, never below a camp.
    const size = Math.max(0, tier - (k === 0 ? 0 : 1 + Math.floor((k - 1) / 3)));
    out.push({ ...spots[k], size, index: k });
  }
  return out;
}
