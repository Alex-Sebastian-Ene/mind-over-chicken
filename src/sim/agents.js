// Shared agent plumbing: movement that respects the island, animation clocks, villager looks.

import { pick, rand } from './util.js';

// Warm, bright hides and dyes so people stand out on grass and dirt (they used to be dirt-brown).
const SHIRTS = ['#c0703a', '#d68c45', '#b5523a', '#d9a85b', '#a8613a'];
const HAIR = ['#2b1d14', '#3b2a1e', '#5a3b22', '#1c1410', '#8a5a32'];
const SKIN = ['#e0b48a', '#c99470', '#a8754f', '#f0c9a0', '#8d5a3b'];

export function villagerLook(era = 'stone') {
  const shirts = era === 'industry' ? ['#5a74a8', '#9a5a40', '#4f8a66', '#a8473f', '#7a8296']
    : era === 'space' ? ['#d9dde6', '#3e8ed0', '#e0a43a', '#c0392b', '#6c7a92'] : SHIRTS;
  return { shirt: pick(shirts), hair: pick(HAIR), skin: pick(SKIN) };
}

// Walk toward a target. Far away: follow the island's distance field around water and
// mountains. Close up: steer straight in, sliding round small obstacles. True on arrival.
export function walk(a, tx, ty, speed, dt, world) {
  const dx = tx - a.x, dy = ty - a.y;
  const d = Math.hypot(dx, dy);
  if (d < 1.2) return true;
  // Target sits on rock or water and we're pressed up against it: close enough.
  if (d < 6 && (a.stuck || 0) > 0.8) { a.stuck = 0; return true; }
  const step = Math.min(d, speed * dt);
  if (d > 7 && world.field) {
    const f = world.field(tx, ty);
    const W = 320;
    const cx = Math.round(a.x), cy = Math.round(a.y);
    const here = f[cy * W + cx];
    if (here > 0) {
      let best = here, bx = cx, by = cy;
      for (let oy = -1; oy <= 1; oy++) for (let ox = -1; ox <= 1; ox++) {
        const v = f[(cy + oy) * W + (cx + ox)];
        if (v >= 0 && v < best) { best = v; bx = cx + ox; by = cy + oy; }
      }
      if (bx !== cx || by !== cy) {
        const mx = bx - a.x, my = by - a.y, md = Math.hypot(mx, my) || 1;
        a.x += (mx / md) * step; a.y += (my / md) * step;
        a.facing = mx >= 0 ? 1 : -1;
        a.stuck = 0;
        return false;
      }
    }
  }
  const base = Math.atan2(dy, dx);
  const tries = [0, 0.6, -0.6, 1.2, -1.2, 1.9, -1.9];
  for (const off of tries) {
    const ang = base + off * (a.side || 1);
    const nx = a.x + Math.cos(ang) * step, ny = a.y + Math.sin(ang) * step;
    if (world.walkable(nx, ny)) {
      a.x = nx; a.y = ny;
      a.facing = Math.cos(ang) >= 0 ? 1 : -1;
      return false;
    }
  }
  a.side = -(a.side || 1); // stuck: try the other way round next time
  a.stuck = (a.stuck || 0) + dt;
  return false;
}

export function animate(a, dt, rate = 6) {
  a.ft = (a.ft || 0) + dt * rate;
  a.frame = Math.floor(a.ft);
}

export function wanderTarget(world, cx, cy, rMin, rMax) {
  return world.randomLand(cx, cy, rMin, rMax);
}

export function jitter(n = 1) { return rand(-n, n); }

// Sail across water toward a dock, following the sea distance field. True on arrival.
export function sail(b, tx, ty, speed, dt, world) {
  const d = Math.hypot(tx - b.x, ty - b.y);
  if (d < 1.5) return true;
  const step = Math.min(d, speed * dt);
  const f = world.seaField(tx, ty);
  const W = 320;
  const cx = Math.round(b.x), cy = Math.round(b.y);
  const here = f[cy * W + cx];
  let mx = tx - b.x, my = ty - b.y;
  if (here > 0) {
    let best = here, bx = cx, by = cy;
    for (let oy = -1; oy <= 1; oy++) for (let ox = -1; ox <= 1; ox++) {
      const v = f[(cy + oy) * W + (cx + ox)];
      if (v >= 0 && v < best) { best = v; bx = cx + ox; by = cy + oy; }
    }
    mx = bx - b.x; my = by - b.y;
  }
  const md = Math.hypot(mx, my) || 1;
  b.x += (mx / md) * step; b.y += (my / md) * step;
  b.facing = mx >= 0 ? 1 : -1;
  return false;
}
