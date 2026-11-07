// Shared agent plumbing: movement that respects the island, animation clocks, villager looks.

import { pick, rand, W as VIEW_W, H as VIEW_H, wrap, wrapDelta } from './util.js';

// The map is a torus (#111) of the world's own size (640×360 at home, #129): steer the short
// way round and keep positions inside the map.
const sizeOf = (world) => [world?.width || VIEW_W, world?.height || VIEW_H];

// Warm, bright hides and dyes so people stand out clearly on grass and dirt.
const SHIRTS = ['#c0703a', '#d68c45', '#b5523a', '#d9a85b', '#a8613a'];
const HAIR = ['#2b1d14', '#3b2a1e', '#5a3b22', '#1c1410', '#8a5a32'];
const SKIN = ['#e0b48a', '#c99470', '#a8754f', '#f0c9a0', '#8d5a3b'];

let rustVillagerLookFn = null;
let rustAnimateAgentFn = null;

export function setRustAgentHelpers(villagerLookFn, animateAgentFn) {
  rustVillagerLookFn = villagerLookFn;
  rustAnimateAgentFn = animateAgentFn;
}

export function villagerLook(era = 'stone') {
  if (rustVillagerLookFn) {
    try {
      const arr = rustVillagerLookFn(era, Math.floor(Math.random() * 100000));
      return { shirt: arr[0], hair: arr[1], skin: arr[2] };
    } catch {}
  }
  const shirts = era === 'industry' ? ['#5a74a8', '#9a5a40', '#4f8a66', '#a8473f', '#7a8296']
    : era === 'space' ? ['#d9dde6', '#3e8ed0', '#e0a43a', '#c0392b', '#6c7a92'] : SHIRTS;
  return { shirt: pick(shirts), hair: pick(HAIR), skin: pick(SKIN) };
}

// Walk toward a target. Far away: follow the island's distance field around water and
// mountains. Close up: steer straight in, sliding round small obstacles. True on arrival.
// Invariant: sprites must never stand or walk on water.
export function walk(a, tx, ty, speed, dt, world) {
  // If displaced into water, immediately snap back to nearest walkable land
  if (world && world.isWater && (world.isWater(a.x, a.y) || !world.walkable(a.x, a.y))) {
    const safe = world.nearestWalkable(a.x, a.y, 40);
    a.x = safe.x;
    a.y = safe.y;
  }
  // If destination is in water, clamp target to nearest walkable land so sprites don't wade in
  if (world && world.isWater && (world.isWater(tx, ty) || !world.walkable(tx, ty))) {
    const safe = world.nearestWalkable(tx, ty, 40);
    tx = safe.x;
    ty = safe.y;
  }

  const [MW, MH] = sizeOf(world);
  a.x = wrap(a.x, MW); a.y = wrap(a.y, MH);
  const dx = wrapDelta(tx, a.x, MW), dy = wrapDelta(ty, a.y, MH);
  tx = a.x + dx; ty = a.y + dy;
  const d = Math.hypot(dx, dy);
  if (d < 1.2) return true;
  // Target sits on rock or water and we're pressed up against it: close enough.
  if (d < 6 && (a.stuck || 0) > 0.8) { a.stuck = 0; return true; }
  const step = Math.min(d, speed * dt);
  if (d > 7 && world && world.field) {
    const f = world.field(wrap(tx, MW), wrap(ty, MH));
    const cx = Math.round(a.x), cy = Math.round(a.y);
    const at = (x, y) => f[wrap(y, MH) * MW + wrap(x, MW)];
    const here = at(cx, cy);
    if (here > 0) {
      let best = here, bx = cx, by = cy;
      for (let oy = -1; oy <= 1; oy++) for (let ox = -1; ox <= 1; ox++) {
        const v = at(cx + ox, cy + oy);
        if (v >= 0 && v < best) { best = v; bx = cx + ox; by = cy + oy; }
      }
      if (bx !== cx || by !== cy) {
        const mx = bx - a.x, my = by - a.y, md = Math.hypot(mx, my) || 1;
        const nx = a.x + (mx / md) * step, ny = a.y + (my / md) * step;
        // Strictly prevent taking a step into water
        if (world.walkable(nx, ny) && (!world.isWater || !world.isWater(nx, ny))) {
          a.x = wrap(nx, MW); a.y = wrap(ny, MH);
          a.facing = mx >= 0 ? 1 : -1;
          a.stuck = 0;
          return false;
        }
      }
    }
  }
  const base = Math.atan2(dy, dx);
  const tries = [0, 0.6, -0.6, 1.2, -1.2, 1.9, -1.9];
  for (const off of tries) {
    const ang = base + off * (a.side || 1);
    const nx = a.x + Math.cos(ang) * step, ny = a.y + Math.sin(ang) * step;
    if (world.walkable(nx, ny) && (!world.isWater || !world.isWater(nx, ny))) {
      a.x = wrap(nx, MW); a.y = wrap(ny, MH);
      a.facing = Math.cos(ang) >= 0 ? 1 : -1;
      return false;
    }
  }
  a.side = -(a.side || 1); // stuck: try the other way round next time
  a.stuck = (a.stuck || 0) + dt;
  return false;
}

export function animate(a, dt, rate = 6) {
  if (rustAnimateAgentFn) {
    try {
      const res = rustAnimateAgentFn(a.ft || 0, dt, rate);
      a.ft = res[0];
      a.frame = Math.floor(res[1]);
      return;
    } catch {}
  }
  a.ft = (a.ft || 0) + dt * rate;
  a.frame = Math.floor(a.ft);
}

export function wanderTarget(world, cx, cy, rMin, rMax) {
  return world.randomLand(cx, cy, rMin, rMax);
}

export function jitter(n = 1) { return rand(-n, n); }

// Sail across water toward a dock, following the sea distance field. True on arrival.
export function sail(b, tx, ty, speed, dt, world) {
  const [MW, MH] = sizeOf(world);
  b.x = wrap(b.x, MW); b.y = wrap(b.y, MH);
  let mx = wrapDelta(tx, b.x, MW), my = wrapDelta(ty, b.y, MH);
  const d = Math.hypot(mx, my);
  if (d < 1.5) return true;
  const step = Math.min(d, speed * dt);
  const f = world.seaField(tx, ty);
  const cx = Math.round(b.x), cy = Math.round(b.y);
  const at = (x, y) => f[wrap(y, MH) * MW + wrap(x, MW)];
  const here = at(cx, cy);
  if (here > 0) {
    let best = here, bx = cx, by = cy;
    for (let oy = -1; oy <= 1; oy++) for (let ox = -1; ox <= 1; ox++) {
      const v = at(cx + ox, cy + oy);
      if (v >= 0 && v < best) { best = v; bx = cx + ox; by = cy + oy; }
    }
    mx = bx - b.x; my = by - b.y;
  }
  const md = Math.hypot(mx, my) || 1;
  b.x = wrap(b.x + (mx / md) * step, MW); b.y = wrap(b.y + (my / md) * step, MH);
  b.facing = mx >= 0 ? 1 : -1;
  return false;
}
