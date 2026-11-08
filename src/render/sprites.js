// Hand-placed pixel sprites. Every function draws on the 320×180 world canvas,
// anchored at the sprite's feet (bottom centre), using whole pixels only.

const px = (ctx, x, y, c, w = 1, h = 1) => { ctx.fillStyle = c; ctx.fillRect(Math.round(x), Math.round(y), w, h); };

// ---------- static buildings (baked into the era layer) ----------

export function drawHut(ctx, x, y) {
  px(ctx, x - 2, y - 2, '#7a5530', 5, 2);      // walls
  px(ctx, x - 2, y - 3, '#c9a35a', 5, 1);      // straw
  px(ctx, x - 1, y - 4, '#d8b56a', 3, 1);
  px(ctx, x, y - 5, '#e2c27a');
  px(ctx, x, y - 1, '#2a1c10');                // door
}

export function drawHouse(ctx, x, y, roof = '#a4402f') {
  px(ctx, x - 2, y - 3, '#e3d3b4', 5, 3);
  px(ctx, x - 3, y - 4, roof, 7, 1);
  px(ctx, x - 2, y - 5, roof, 5, 1);
  px(ctx, x - 1, y - 6, roof, 3, 1);
  px(ctx, x, y - 2, '#4b3524', 1, 2);
  px(ctx, x - 2, y - 2, '#ffd36e');
}

export function drawTower(ctx, x, y, h = 10) {
  px(ctx, x - 2, y - h, '#6c7a92', 5, h);
  px(ctx, x - 2, y - h, '#8a97ad', 1, h);
  for (let i = 2; i < h - 1; i += 2) { px(ctx, x - 1, y - i - 1, (i * 7 + x) % 3 ? '#ffe39a' : '#3d4558'); px(ctx, x + 1, y - i - 1, (i * 5 + x) % 4 ? '#ffe39a' : '#3d4558'); }
  px(ctx, x, y - h - 2, '#c9ced8', 1, 2);
  px(ctx, x, y - h - 3, '#ff5a5a');
}

export function drawFactory(ctx, x, y) {
  px(ctx, x - 7, y - 6, '#8e3f2c', 14, 6);
  px(ctx, x - 7, y - 7, '#6b2f22', 14, 1);
  for (let i = -5; i <= 5; i += 3) px(ctx, x + i, y - 4, '#ffcf5c', 2, 1);
  px(ctx, x - 1, y - 2, '#3a2116', 2, 2);
  px(ctx, x - 5, y - 13, '#5d5a5a', 2, 7);
  px(ctx, x + 3, y - 11, '#5d5a5a', 2, 5);
  px(ctx, x - 5, y - 13, '#7b7777', 2, 1);
  px(ctx, x + 3, y - 11, '#7b7777', 2, 1);
}

export function drawExchange(ctx, x, y) {
  px(ctx, x - 6, y - 1, '#cfc8b8', 13, 1);
  px(ctx, x - 5, y - 6, '#efe9da', 11, 5);
  for (let i = -4; i <= 4; i += 2) px(ctx, x + i, y - 6, '#bdb5a2', 1, 5);
  px(ctx, x - 6, y - 7, '#d8d0bd', 13, 1);
  px(ctx, x - 4, y - 8, '#d8d0bd', 9, 1);
  px(ctx, x - 2, y - 9, '#d8d0bd', 5, 1);
  px(ctx, x, y - 8, '#f2c14e');                // the golden goat
}

export function drawHangar(ctx, x, y) {
  px(ctx, x - 7, y - 5, '#8a93a3', 15, 5);
  px(ctx, x - 6, y - 6, '#9ea7b6', 13, 1);
  px(ctx, x - 4, y - 7, '#9ea7b6', 9, 1);
  px(ctx, x - 3, y - 4, '#3b4252', 7, 4);
  px(ctx, x - 7, y - 5, '#b5bdcb', 1, 5);
}

export function drawPad(ctx, x, y) {
  px(ctx, x - 6, y - 1, '#5f6470', 13, 2);
  px(ctx, x - 5, y - 2, '#767c88', 11, 1);
  px(ctx, x - 3, y, '#f2c14e', 1, 1); px(ctx, x + 3, y, '#f2c14e', 1, 1);
}

// ---------- animated sprites ----------

// Villager. opts: shirt, skin, hair, frame (walk cycle), pose, carry (color), facing (-1|1)
// Clean pixel art with dark silhouette outline and readable clothing so people pop against grass and dirt.
export function drawPerson(ctx, x, y, o = {}) {
  x = Math.round(x); y = Math.round(y);
  const skin = o.skin || '#e0b48a', shirt = o.shirt || '#c0703a', hair = o.hair || '#3b2a1e';
  const pose = o.pose || 'walk';
  const facing = o.facing || 1;

  if (pose === 'dead') {
    // Fallen smitten/starved body with outline and charred clothing
    px(ctx, x - 3, y - 2, '#120d09', 7, 3); // outline
    px(ctx, x - 2, y - 1, shirt, 4, 1);     // charred torso
    px(ctx, x + 2, y - 1, skin, 2, 1);      // head
    px(ctx, x + 3, y - 2, hair, 1, 1);      // hair
    px(ctx, x - 3, y - 1, '#2b1d14', 1, 1); // feet/boots
    return;
  }

  const bob = (pose === 'dance' || pose === 'cheer') ? (o.frame % 2) : 0;
  const sy = y - bob;

  // Ground contact shadow
  px(ctx, x - 2, y, 'rgba(0,0,0,0.3)', 5, 1);

  // Legs and boots
  const legStep = (pose === 'walk' && (o.frame % 2 === 1)) ? 1 : 0;
  if (pose === 'walk') {
    px(ctx, x - 1, sy - 2 + legStep, '#2b1d14', 1, 2);
    px(ctx, x + 1, sy - 2 + (1 - legStep), '#2b1d14', 1, 2);
  } else {
    px(ctx, x - 1, sy - 2, '#2b1d14', 3, 2);
  }

  // Torso / shirt
  px(ctx, x - 1, sy - 5, shirt, 3, 3);
  // Belt & buckle
  px(ctx, x - 1, sy - 3, '#3a2414', 3, 1);
  px(ctx, x, sy - 3, '#d4af37', 1, 1);

  // Head and face
  px(ctx, x - 1, sy - 7, skin, 3, 2);
  // Eye pointing towards facing direction
  const eyeX = facing > 0 ? x + 1 : x - 1;
  px(ctx, eyeX, sy - 6, '#1a110a');
  // Hair crown and back
  px(ctx, x - 1, sy - 8, hair, 3, 1);
  const hairBackX = facing > 0 ? x - 1 : x + 1;
  px(ctx, hairBackX, sy - 7, hair, 1, 1);

  // Arms and poses
  if (pose === 'pray' || pose === 'dance' || pose === 'cheer') {
    px(ctx, x - 2, sy - 6, skin);
    px(ctx, x + 2, sy - 6, skin);
  } else if (pose === 'panic') {
    const flick = (o.frame % 2 === 1) ? -1 : 1;
    px(ctx, x - 2, sy - 6 + flick, skin);
    px(ctx, x + 2, sy - 6 - flick, skin);
  } else {
    // Walk swing
    px(ctx, x - 2, sy - 4 + (legStep ? 1 : 0), shirt);
    px(ctx, x + 2, sy - 4 + (legStep ? 0 : 1), shirt);
  }

  // Accessories
  if (o.hat) px(ctx, x - 2, sy - 8, o.hat, 5, 1);
  if (o.carry) px(ctx, x + facing * 2, sy - 4, o.carry, 2, 2);
  if (pose === 'sign') {
    px(ctx, x, sy - 7, '#7a5a3a', 1, 3);
    px(ctx, x - 2, sy - 10, '#f2ead8', 5, 3);
    px(ctx, x - 1, sy - 9, '#c0392b', 3, 1);
  }
  if (o.mark) px(ctx, x, sy - 8, o.mark);
}

export function drawMammoth(ctx, x, y, o = {}) {
  x = Math.round(x); y = Math.round(y);
  const f = o.facing || 1, fur = o.dead ? '#5a4030' : '#7b5536', dark = '#5e3f27';
  if (o.dead) {
    px(ctx, x - 3, y - 2, fur, 7, 2);
    px(ctx, x - 3, y - 3, '#8f6a48', 7, 1);
    return;
  }
  px(ctx, x - 3, y, 'rgba(0,0,0,0.25)', 7, 1);
  px(ctx, x - 3, y - 5, fur, 6, 3);
  px(ctx, x - 2, y - 6, fur, 4, 1);
  px(ctx, x + (f > 0 ? 2 : -4), y - 6, dark, 2, 3);           // head
  px(ctx, x + (f > 0 ? 4 : -5), y - 4, '#f4ecd8');             // tusk
  px(ctx, x + (f > 0 ? 3 : -4), y - 3, dark);                  // trunk
  const leg = o.frame % 2;
  px(ctx, x - 3 + leg, y - 2, dark, 1, 2); px(ctx, x + 1 - leg, y - 2, dark, 1, 2);
}

export function drawGoat(ctx, x, y, o = {}) {
  x = Math.round(x); y = Math.round(y);
  const f = o.facing || 1;
  px(ctx, x - 1, y - 2, '#efe8dc', 3, 1);
  px(ctx, x + (f > 0 ? 2 : -2), y - 3, '#cfc6b8');
  px(ctx, x - 1, y - 1, '#7a6a58'); px(ctx, x + 1, y - 1, '#7a6a58');
}

export function drawChicken(ctx, x, y, o = {}) {
  x = Math.round(x); y = Math.round(y);
  const f = o.facing || 1;
  const hop = o.frame % 4 === 0 ? 1 : 0;
  px(ctx, x - 1, y - 2 - hop, '#fbf6ea', 2, 2);
  px(ctx, x + (f > 0 ? 1 : -2), y - 3 - hop, '#fbf6ea');
  px(ctx, x + (f > 0 ? 1 : -2), y - 4 - hop, '#e53935');
  px(ctx, x + (f > 0 ? 2 : -3), y - 3 - hop, '#f6a623');
  if (o.halo) px(ctx, x - 1, y - 6 - hop, '#ffe066', 3, 1);
}

// Campfire. heat 0..1, t = time in seconds.
export function drawFire(ctx, x, y, heat, t) {
  x = Math.round(x); y = Math.round(y);
  if (heat <= 0) {
    // Hearth stones ring
    px(ctx, x - 4, y, '#505050', 9, 1);
    px(ctx, x - 3, y + 1, '#656565', 7, 1);
    px(ctx, x - 3, y - 1, '#444444', 7, 1);
    // Unlit firewood log stack
    px(ctx, x - 3, y, '#5a3a22', 7, 1); // bottom horizontal log
    px(ctx, x - 3, y, '#8d5b35');        // cut wood ends
    px(ctx, x + 3, y, '#8d5b35');
    px(ctx, x - 2, y - 1, '#6e4a2c', 5, 1); // diagonal crossed log
    px(ctx, x - 1, y - 2, '#7a5232', 3, 1); // top crossed log
    px(ctx, x, y - 3, '#9c683e');        // apex stick tip
    px(ctx, x - 1, y - 1, '#3e2413');    // notch shadows
    px(ctx, x + 1, y - 1, '#3e2413');
    return;
  }
  // Lit campfire
  px(ctx, x - 2, y - 1, '#5a3a22', 5, 1);
  px(ctx, x - 1, y - 2, '#6e4a2c', 3, 1);
  px(ctx, x - 3, y, '#6b6b6b'); px(ctx, x + 3, y, '#6b6b6b'); px(ctx, x - 2, y + 1, '#6b6b6b', 5, 1);
  const n = Math.ceil(heat * 6);
  for (let i = 0; i < n; i++) {
    const fx = x - 1 + ((i * 7 + Math.floor(t * 12)) % 3);
    const fh = 1 + Math.floor(heat * 4 * (0.5 + 0.5 * Math.sin(t * 15 + i * 1.7)));
    px(ctx, fx, y - 2 - fh, i % 3 === 0 ? '#ffe066' : (i % 3 === 1 ? '#ff9a2e' : '#e8452c'), 1, fh);
  }
  if (heat > 0.75) px(ctx, x, y - 7 - Math.floor((t * 6) % 3), '#ffd166');
}

// Rocket on the pad, built from the bottom up as progress goes 0 → 1.
export function drawRocket(ctx, x, y, progress = 1, t = 0, flame = 0) {
  x = Math.round(x); y = Math.round(y);
  const H = 22;
  const built = Math.floor(H * progress);
  for (let i = 0; i < built; i++) {
    const row = y - 2 - i;
    if (i < 3) { px(ctx, x - 3, row, '#c0392b'); px(ctx, x + 3, row, '#c0392b'); }   // fins
    if (i < H - 4) px(ctx, x - 2, row, i % 6 === 5 ? '#c7ccd6' : '#f4f6fa', 5, 1);
    else if (i < H - 2) px(ctx, x - 1, row, '#e04a3a', 3, 1);
    else px(ctx, x, row, '#e04a3a');
    if (i === 12) px(ctx, x, row, '#3e8ed0');                                          // window
    if (i === 8) px(ctx, x - 2, row, '#1f2a44', 5, 1);
  }
  if (progress < 1) {
    // scaffolding sparks at the build line
    if (Math.sin(t * 20) > 0) px(ctx, x - 3 + Math.floor((t * 9) % 7), y - 2 - built, '#ffe066');
  }
  if (flame > 0) {
    const fl = Math.floor(3 + flame * 6 + Math.sin(t * 40) * 2);
    for (let i = 0; i < fl; i++) {
      const w = Math.max(1, 3 - Math.floor(i / 3));
      px(ctx, x - Math.floor(w / 2), y - 1 + i, i < 2 ? '#fff4c2' : (i < 5 ? '#ffb02e' : '#e8452c'), w, 1);
    }
  }
}

export function drawGantry(ctx, x, y, h = 24) {
  for (let i = 0; i < h; i++) px(ctx, x, y - i, i % 3 === 0 ? '#c9a227' : '#8c7a3c');
  for (let i = 0; i < h; i += 4) px(ctx, x - 1, y - i, '#8c7a3c', 3, 1);
  px(ctx, x - 1, y - h, '#ff5a5a');
}

// Jagged lightning bolt from the sky to a target; returns the polyline.
export function makeBolt(tx, ty) {
  const pts = [{ x: tx + (Math.random() - 0.5) * 30, y: -4 }];
  const steps = 9;
  for (let i = 1; i < steps; i++) {
    const t = i / steps;
    pts.push({ x: pts[0].x + (tx - pts[0].x) * t + (Math.random() - 0.5) * 9, y: ty * t });
  }
  pts.push({ x: tx, y: ty });
  return pts;
}

export function drawBolt(ctx, pts, alpha = 1) {
  ctx.globalAlpha = alpha;
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i], b = pts[i + 1];
    const n = Math.max(Math.abs(b.x - a.x), Math.abs(b.y - a.y));
    for (let s = 0; s <= n; s++) {
      const x = a.x + (b.x - a.x) * (s / n), y = a.y + (b.y - a.y) * (s / n);
      px(ctx, x - 1, y, 'rgba(160,200,255,0.6)', 3, 1);
      px(ctx, x, y, '#ffffff');
    }
  }
  ctx.globalAlpha = 1;
}

// The god's aim reticle (head tilt or mouse).
export function drawReticle(ctx, x, y, t, color = '#ffe066') {
  x = Math.round(x); y = Math.round(y);
  const r = 3 + Math.round((Math.sin(t * 6) + 1) * 0.5);
  px(ctx, x - r - 1, y, color, 2, 1); px(ctx, x + r, y, color, 2, 1);
  px(ctx, x, y - r - 1, color, 1, 2); px(ctx, x, y + r, color, 1, 2);
  px(ctx, x, y, color);
}

// Small pixel portrait for the advisor, drawn on its own 16×16 canvas.
export function drawAdvisorPortrait(ctx, era, mood, t) {
  ctx.clearRect(0, 0, 16, 16);
  const bg = era === 'industry' ? '#3b2a20' : era === 'space' ? '#1d2840' : '#2a1d2e';
  px(ctx, 0, 0, bg, 16, 16);
  const shake = mood === 'panic' ? (Math.floor(t * 20) % 2) : 0;
  const ox = shake;
  const skin = '#d9a477';
  // head
  px(ctx, 5 + ox, 5, skin, 6, 6);
  px(ctx, 4 + ox, 7, skin, 1, 2); px(ctx, 11 + ox, 7, skin, 1, 2);
  // eyes (wide when nervous)
  const eye = mood === 'panic' || mood === 'nervous' ? '#ffffff' : '#2b1d14';
  px(ctx, 6 + ox, 7, eye, 1, mood === 'panic' ? 2 : 1); px(ctx, 9 + ox, 7, eye, 1, mood === 'panic' ? 2 : 1);
  if (mood === 'panic' || mood === 'nervous') { px(ctx, 6 + ox, 8, '#2b1d14'); px(ctx, 9 + ox, 8, '#2b1d14'); }
  // mouth
  if (mood === 'happy') { px(ctx, 6 + ox, 9, '#7a2e22', 4, 1); px(ctx, 7 + ox, 10, '#7a2e22', 2, 1); }
  else if (mood === 'panic') px(ctx, 7 + ox, 9, '#3a0f0a', 2, 2);
  else px(ctx, 7 + ox, 10, '#7a2e22', 2, 1);
  // sweat drop
  if (mood === 'nervous' || mood === 'panic') { const d = Math.floor(t * 4) % 3; px(ctx, 12 + ox, 5 + d, '#8fd3ff'); }
  if (era === 'stone' || era === 'farming') {
    // bone headdress and fur robe
    px(ctx, 4 + ox, 3, '#efe3c2', 8, 2); px(ctx, 5 + ox, 1, '#efe3c2', 1, 2); px(ctx, 10 + ox, 1, '#efe3c2', 1, 2); px(ctx, 7 + ox, 0, '#e8452c', 2, 3);
    px(ctx, 3, 11, '#7b5536', 10, 5); px(ctx, 7, 12, '#efe3c2', 2, 1);
  } else if (era === 'industry') {
    // slicked hair, suit, gold chain: the finance bro years
    px(ctx, 5 + ox, 3, '#1c1410', 6, 2); px(ctx, 4 + ox, 4, '#1c1410');
    px(ctx, 3, 11, '#26304a', 10, 5); px(ctx, 7, 11, '#f2ead8', 2, 3); px(ctx, 7, 12, '#c0392b', 2, 3);
    px(ctx, 5, 12, '#f2c14e'); px(ctx, 10, 12, '#f2c14e');
  } else {
    // mission headset
    px(ctx, 5 + ox, 3, '#4a3a2a', 6, 2);
    px(ctx, 3 + ox, 6, '#20242c', 2, 4); px(ctx, 4 + ox, 4, '#20242c', 8, 1); px(ctx, 11 + ox, 6, '#20242c', 1, 3);
    px(ctx, 2 + ox, 10, '#20242c', 3, 1);
    px(ctx, 3, 11, '#d9dde6', 10, 5); px(ctx, 5, 12, '#3e8ed0', 2, 2);
  }
}

// Boat: a raft in the Stone Age, a sailing ship later. Anchored at the waterline centre.
export function drawBoat(ctx, x, y, o = {}) {
  x = Math.round(x); y = Math.round(y);
  const f = o.facing || 1;
  if (o.kind === 'raft') {
    px(ctx, x - 2, y, '#7a5530', 5, 1);
    px(ctx, x - 2, y - 1, '#9a7040', 5, 1);
    px(ctx, x, y - 4, '#6b4a2c', 1, 3);
    px(ctx, x + f, y - 4, '#e8dcc0', 1, 2);
  } else {
    px(ctx, x - 3, y, '#5a3a22', 7, 1);
    px(ctx, x - 2, y + 1, '#4a2f1b', 5, 1);
    px(ctx, x - 3 + (f > 0 ? 6 : 0), y - 1, '#5a3a22');
    px(ctx, x, y - 6, '#4a3a2a', 1, 6);
    px(ctx, x - (f > 0 ? 2 : 0), y - 5, o.sail || '#f2ead8', 2, 3);
    px(ctx, x + (f > 0 ? 1 : -2), y - 5, o.sail || '#f2ead8', 2, 3);
    if (o.cargo) px(ctx, x - 2, y - 1, o.cargo, 2, 1);
  }
  // wake
  if (o.moving) px(ctx, x - 4 * f, y + 1, 'rgba(220,235,255,0.6)', 2, 1);
}
