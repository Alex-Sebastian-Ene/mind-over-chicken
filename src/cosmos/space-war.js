// The interplanetary war on the galaxy view (#166). The war is Rust (crates/moc-cosmos/src/war.rs,
// reached through RustGalaxy's war_* methods; buffer strides in crates/moc-cosmos/src/host.rs).
// This file only copies each planet's numbers in, applies what the war did back to the planets,
// draws fleets and battles, and builds the small orders panel. The words (news, HUD rows, the
// orders line, planet-list notes) are written in Rust.

import { arriveShip } from './planet-sim.js';
import { label } from '../eras/stone.js';

// Strides (moc_cosmos::host).
const BODY_IN = 9, FLEET = 8, BATTLE = 6, BODY_OUT = 8, POWER = 18, FX = 5;
// How each brain channel's world leans (0 dove … 1 hawk).
const TEMPER = { stress: 0.75, calm: 0.25, focus: 0.45, blink: 0.55, mixed: 0.5 };
// Fleet trails by order: raid, bombardment, invasion.
const TRAIL = ['#ffd166', '#ff5a72', '#d58cff'];
export const ORDERS = ['Raid', 'Bombard', 'Invade'];
const GLYPH = ['raid ', 'bomb ', 'invade '];
const EMPTY = new Float32Array(0);
const PLAYER_COLOUR = '#ffd166';
// Labels on a planet after a battle there: [order][won].
const BATTLE_LABEL = [
  [{ text: 'raid repelled', color: '#9be38a' }, { text: 'raided!', color: '#ffd166' }],
  [{ text: 'shields held', color: '#9be38a' }, { text: 'bombarded!', color: '#ff5a72' }],
  [{ text: 'landings repelled', color: '#9be38a' }, { text: 'invaded!', color: '#d58cff' }],
];

export class SpaceWarView {
  constructor(galaxy, wasm) {
    this.g = galaxy;
    this.w = wasm;
    this.buf = new Float32Array(BODY_IN * 16);
    this.names = [];
    this.fleets = EMPTY;
    this.battles = EMPTY;
    this.bodies = EMPTY;
    this.powers = EMPTY;
    this.powerNames = [];
    this.seenBattle = 0;
    this.newsSeq = 0;
    this.uiT = 0;
    this.playerHome = undefined;
    this.shared = -1;      // the civ sandbox's planet (stage 4), −1 none
    this.target = null;    // the world the player's orders go to
    this.hud = [];
    this.meta = [];
    this.civNews = [];     // war news about civ sandbox civilisations, for the sandbox's ticker
  }

  // Difficulty seam: how hard the AI powers press the player's world (×how often they launch at
  // it, ×how hard those fleets hit). 1 is Normal; Easy about 0.5, Hard about 1.6. Rust clamps it.
  setPlayerPressure(x) { this.w.war_set_player_pressure?.(Number(x) || 1); }

  // The civ sandbox's planet: its civilisations, not the player, live there (#166 stage 4).
  setShared(i) {
    if (this.shared === i) return;
    this.shared = i;
    this.w.war_set_shared(i, true);
  }

  update(dt, fx) {
    const g = this.g, w = this.w, n = g.planets.length;
    if (this.buf.length < n * BODY_IN) this.buf = new Float32Array(n * BODY_IN * 2);
    const b = this.buf;
    for (let i = 0; i < n; i++) {
      const p = g.planets[i], o = i * BODY_IN;
      if (this.names[i] !== p.name) { this.names[i] = p.name; w.war_name(i, p.name); }
      const hand = p.home && !p.autonomous;
      b[o] = p.alive && !p.forming ? 1 : 0;
      b[o + 1] = Math.max(0, p.pop || 0);
      b[o + 2] = p.eraIndex || 0;
      b[o + 3] = p.tier || 0;
      b[o + 4] = p.progress || 0;
      b[o + 5] = (p.food || 0) + (p.wood || 0) + (p.stone || 0) + (p.goats || 0);
      b[o + 6] = p.stability ?? 0.7;
      b[o + 7] = TEMPER[p.channel] ?? 0.5;
      b[o + 8] = hand ? 1 : 0;
    }
    // The campaign's hand-played home is the player's; the sandboxes have no player world.
    const home = this.shared >= 0 ? -1 : g.planets.findIndex((p) => p.home && !p.autonomous);
    if (home !== this.playerHome) { this.playerHome = home; w.war_set_player_home(home); }
    w.war_sync(b.subarray(0, n * BODY_IN));
    w.war_step(dt, fx?.calm || 0, fx?.focus || 0, fx?.stress || 0);
    this.applyFx(w.war_take_fx());
    this.fleets = w.war_fleets();
    this.battles = w.war_battles();
    this.labelBattles();
    if ((this.uiT -= dt) <= 0) {
      this.uiT = 0.25;
      this.bodies = w.war_bodies();
      this.powers = w.war_powers();
      for (let p = this.powerNames.length; p < this.powers.length / POWER; p++) this.powerNames[p] = w.war_power_name(p);
      this.hud = parseRows(w.war_hud());
      this.meta = g.planets.map((_, i) => w.war_planet_meta(i));
      this.readNews();
      if (this.target && (this.target.home && !this.target.autonomous)) this.target = null;
    }
  }

  // People, progress and stores lost; planets taken; colonists landed.
  applyFx(fx) {
    const g = this.g;
    for (let i = 0; i < g.planets.length && (i + 1) * FX <= fx.length; i++) {
      const o = i * FX, p = g.planets[i];
      const [popMul, progLoss, stockMul, by, settled] = [fx[o], fx[o + 1], fx[o + 2], fx[o + 3], fx[o + 4]];
      if (popMul === 1 && progLoss === 0 && stockMul === 1 && by < 0 && !settled) continue;
      if (p.home && !p.autonomous) { p.flash = 0.6; continue; }   // the campaign's scene isn't ours to hurt
      if (settled && p.pop <= 0) { arriveShip(p); p.surface?.ensureTribe?.(); }
      if (p.pop > 0) p.pop = Math.max(3, p.pop * popMul);
      p.progress = Math.max(0, (p.progress || 0) - progLoss);
      for (const k of ['food', 'wood', 'stone', 'goats']) if (typeof p[k] === 'number') p[k] *= stockMul;
      if (by >= 0) {
        p.ruler = by === this.w.war_player() ? 'you' : this.powerName(by);
        g.pushLabel(p, { text: `taken by ${p.ruler}`, t: 4, color: '#d58cff' });
      }
      p.flash = Math.max(p.flash || 0, 0.4);
    }
  }

  labelBattles() {
    const bt = this.battles, g = this.g;
    for (let o = 0; o + BATTLE <= bt.length; o += BATTLE) {
      const id = bt[o + 5];
      if (id <= this.seenBattle) continue;
      this.seenBattle = id;
      const p = g.planets[bt[o]];
      if (p) g.pushLabel(p, { ...BATTLE_LABEL[bt[o + 2]][bt[o + 3] ? 1 : 0], t: 2.6 });
    }
  }

  // [seq, tone, about the player, about a civ sandbox civilisation, text] per line.
  readNews() {
    const a = this.w.war_news(this.newsSeq), g = this.g;
    for (let k = 0; k + 4 < a.length; k += 5) {
      this.newsSeq = a[k] + 1;
      g.note(null, a[k + 4]);
      if (a[k + 2]) g.onEvent?.(g.home, a[k + 4], 'war', a[k + 1]);
      if (a[k + 3]) { this.civNews.push({ text: a[k + 4], tone: a[k + 1] }); if (this.civNews.length > 8) this.civNews.shift(); }
    }
  }

  // ---- civ sandbox civilisations in space (#166 stage 4; src/civ/space-link.js drives these) ----
  // s: a civilisation's space program as civ.spaceInfo() reads it. Returns its power id (−1 none).
  upsertCiv(key, name, color, home, s, alive = true) {
    return this.w.war_upsert_civ(key >>> 0, name, color[0], color[1], color[2], home, s.production, s.knowledge, s.resources, s.fleet, s.shields, s.defences, s.garrison, s.tier, s.temper, alive);
  }
  targetsFor(id) { return this.w.war_targets_for(id); }
  launchExternal(id, body, order, strength, motive) { return this.w.war_launch_external(id, body, order, strength, motive); }
  takeResults(id) { return this.w.war_take_results(id); }
  takeCivNews() { const q = this.civNews; this.civNews = []; return q; }

  powerName(p) { return this.powerNames[p] || (this.powerNames[p] = this.w.war_power_name(p)); }

  // A power's colour: the player's gold, a civilisation's own, a world's atmosphere.
  colour(p) {
    const P = this.powers, o = p * POWER;
    if (o + POWER > P.length) return '#ffffff';
    if (P[o + 1] === 0) return PLAYER_COLOUR;
    if (P[o + 1] === 2) return `rgb(${P[o + 15]},${P[o + 16]},${P[o + 17]})`;
    const a = this.g.planets[P[o + 11]]?.atmosphere;
    return a ? `rgb(${a[0]},${a[1]},${a[2]})` : '#ffffff';
  }

  // The player's power, as numbers: { space, fleet, cap } (null without one).
  player() {
    const id = this.w.war_player();
    const o = id * POWER;
    if (id < 0 || o + POWER > this.powers.length) return null;
    return { id, space: this.powers[o + 3] > 0, fleet: this.powers[o + 4], cap: this.powers[o + 5] };
  }

  // Order an attack on a planet: null if the fleet is away, else why not.
  order(planet, kind) {
    const i = this.g.planets.indexOf(planet);
    if (i < 0) return 'No such world.';
    const r = this.w.war_order(i, kind);
    this.uiT = 0;
    return r || null;
  }

  targetLine(planet) {
    const i = this.g.planets.indexOf(planet);
    return i < 0 ? '' : this.w.war_target_line(i);
  }

  planetMeta(planet) { return this.meta[this.g.planets.indexOf(planet)] || ''; }

  // ---- drawing (world pixels; called from Galaxy.draw after the colony ships) ----
  draw(ctx, alpha = 1) {
    const g = this.g, F = this.fleets, B = this.battles, bodies = this.bodies;
    ctx.globalAlpha = alpha;
    // Shields: a faint ring that thickens with them.
    for (let i = 0; i < g.planets.length && (i + 1) * BODY_OUT <= bodies.length; i++) {
      const o = i * BODY_OUT, cap = bodies[o + 4];
      if (cap <= 0.5) continue;
      const p = g.planets[i], q = g.pos(p), r = p.size + 2;
      ctx.globalAlpha = alpha * (0.15 + 0.35 * Math.min(1, bodies[o + 1] / cap));
      ctx.fillStyle = '#7fe8ff';
      for (let k = 0; k < 32; k += 2) { const a = (k / 32) * Math.PI * 2 + g.time * 0.3; ctx.fillRect(Math.round(q.x + Math.cos(a) * r), Math.round(q.y + Math.sin(a) * r * 0.9), 1, 1); }
      // A colony flies its ruler's colour.
      const ruler = bodies[o];
      if (ruler >= 0 && this.powers.length >= (ruler + 1) * POWER && this.powers[ruler * POWER + 2] !== i) {
        ctx.globalAlpha = alpha;
        ctx.fillStyle = this.colour(ruler);
        ctx.fillRect(Math.round(q.x + p.size * 0.7), Math.round(q.y - p.size * 0.9), 2, 2);
      }
    }
    // Fleets: a head in the owner's colour, a trail in the order's.
    for (let o = 0; o + FLEET <= F.length; o += FLEET) {
      const from = g.planets[F[o]], to = g.planets[F[o + 1]];
      if (!from || !to) continue;
      const s = F[o + 3] === 2 || F[o + 5] > 60 ? 3 : 2;   // invasions and big fleets draw bigger
      for (let k = 9; k >= 0; k--) {
        const sp = curve(g.pos(from), g.pos(to), Math.max(0, F[o + 2] - k * 0.012), F[o + 6]);
        ctx.globalAlpha = alpha * (k === 0 ? 1 : 0.9 - k * 0.08);
        ctx.fillStyle = k === 0 ? this.colour(F[o + 4]) : TRAIL[F[o + 3]] || '#ffffff';
        const z = k === 0 ? s : 1, x = Math.round(sp.x - (z - 1) / 2), y = Math.round(sp.y - (z - 1) / 2);
        if (k === 0) {   // a dark rim so the ship reads against the star and the planets
          ctx.fillStyle = 'rgba(8,10,24,0.85)';
          ctx.fillRect(x - 1, y - 1, z + 2, z + 2);
          ctx.fillStyle = this.colour(F[o + 4]);
        }
        ctx.fillRect(x, y, z, z);
      }
    }
    // Battles: a flash, a ring and sparks at the planet.
    for (let o = 0; o + BATTLE <= B.length; o += BATTLE) {
      const p = g.planets[B[o]];
      if (!p) continue;
      const q = g.pos(p), age = B[o + 1], id = B[o + 5];
      const col = B[o + 3] ? TRAIL[B[o + 2]] : '#7fe8ff';
      ctx.globalAlpha = alpha * Math.max(0, 1 - age);
      ctx.strokeStyle = col;
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.arc(Math.round(q.x), Math.round(q.y), p.size + 2 + age * 8, 0, Math.PI * 2); ctx.stroke();
      ctx.fillStyle = col;
      for (let k = 0; k < 10; k++) {
        const h = ((id * 2654435761 + k * 40503) >>> 0) / 4294967296;
        const a = h * Math.PI * 2, rr = p.size * (0.4 + 0.9 * ((h * 7) % 1)) + age * 5;
        if (((g.time * 12 + k) | 0) % 3 === 0) continue;
        ctx.fillRect(Math.round(q.x + Math.cos(a) * rr), Math.round(q.y + Math.sin(a) * rr * 0.8), 1, 1);
      }
    }
    ctx.globalAlpha = 1;
  }

  // Crisp labels on the fleets (world units, the galaxy's overlay pass): the order and its strength.
  drawOverlay(ctx, alpha = 1) {
    const g = this.g, F = this.fleets;
    ctx.globalAlpha = alpha;
    for (let o = 0; o + FLEET <= F.length; o += FLEET) {
      const from = g.planets[F[o]], to = g.planets[F[o + 1]];
      if (!from || !to) continue;
      if (F[o + 2] < 0.15 || F[o + 2] > 0.85) continue;   // no labels piling up on the planets
      const sp = curve(g.pos(from), g.pos(to), F[o + 2], F[o + 6]);
      label(ctx, sp.x, sp.y - 4, `${GLYPH[F[o + 3]]}${Math.round(F[o + 5])}`, TRAIL[F[o + 3]], 4);
    }
    ctx.globalAlpha = 1;
  }

  // The HUD rows for the player's power ([] without one).
  objectives() { return this.hud; }
}

// The flight path: a bent quadratic curve from a to b (as the colony ships fly).
function curve(a, b, t, bend) {
  const mx = (a.x + b.x) / 2 - (b.y - a.y) * bend, my = (a.y + b.y) / 2 + (b.x - a.x) * bend - 18;
  const u = 1 - t;
  return { x: u * u * a.x + 2 * u * t * mx + t * t * b.x, y: u * u * a.y + 2 * u * t * my + t * t * b.y };
}

// "label\tvalue\tbad" lines (Rust) → HUD rows.
function parseRows(text) {
  return text.split('\n').filter(Boolean).map((r) => {
    const [label, value, bad] = r.split('\t');
    return { label, value, plain: bad !== '1', bad: bad === '1' };
  });
}

// The orders panel in the galaxy panel: shown once the player's world is in space. Click a world
// to target it (cosmos.focusPlanet sets war.target), then raid, bombard or invade.
export function renderWarOrders(root, galaxy) {
  if (!root) return;
  const war = galaxy?.war;
  const me = war?.player();
  const show = !!(me && me.space);
  root.hidden = !show;
  if (!show) return;
  const t = war.target;
  const line = t ? war.targetLine(t) : `Your fleet ${Math.round(me.fleet)}/${Math.round(me.cap)}. Click a world to target it.`;
  let p = root.querySelector('.war-line');
  if (!p) {
    p = document.createElement('p');
    p.className = 'war-line';
    const row = document.createElement('div');
    row.className = 'war-btns';
    ORDERS.forEach((name, k) => {
      const b = document.createElement('button');
      b.type = 'button'; b.className = 'btn btn-small war-order'; b.dataset.order = String(k); b.textContent = name;
      row.append(b);
    });
    root.replaceChildren(p, row);
  }
  if (p.textContent !== line) p.textContent = line;
  const ready = !!t && me.fleet >= 4;
  for (const b of root.querySelectorAll('.war-order')) b.disabled = !ready;
}
