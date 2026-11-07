// civ (#50): adapter for the warring-civilisations simulation in crates/moc-civ.
//
// The module is plain `extern "C"` WebAssembly (no wasm-bindgen; rebuild it with
// crates/moc-civ/build-wasm.sh). We keep the world handle as a number, call the exports, and
// read buffers through typed-array views on `memory.buffer`. Any call into the module may grow
// (and so replace) that buffer, so views are made fresh on every read and never kept.

const WASM_URL = new URL('./moc_civ.wasm', import.meta.url);
let compiled = null;

export const EVENT = {
  CONTACT: 1, WAR: 2, PEACE: 3, CAPTURE: 4, REBELLION: 5, WIPED: 6, FOUNDED: 7, RAID: 8,
  FAMINE: 9, SMITE: 10, TRADE: 11, CAPITAL: 12, RAZED: 13, CAST: 14,
};
export const CAST = { fireball: 1, spawn: 2, peace: 3, disaster: 4, bless: 5, rain: 6 };
export const SEED_SOURCE = ['the run seed', 'calibration', 'your mind', 'rebellion'];

export function loadCivModule() {
  compiled ??= fetch(WASM_URL)
    .then((r) => { if (!r.ok) throw new Error(`moc_civ.wasm: HTTP ${r.status}`); return r.arrayBuffer(); })
    .then((bytes) => WebAssembly.compile(bytes));
  return compiled;
}

// width/height 0 = the default planet (1280×640).
export async function createCiv(seed, { width = 0, height = 0 } = {}) {
  const module = await loadCivModule();
  const instance = await WebAssembly.instantiate(module, {});
  return new Civ(instance.exports, seed, width, height);
}

const css = (c) => `rgb(${c[0]},${c[1]},${c[2]})`;

export class Civ {
  constructor(ex, seed, width, height) {
    this.ex = ex;
    this.seed = seed >>> 0;
    this.w = ex.civ_new_sized(this.seed, width, height);
    this.pw = ex.civ_planet_w(this.w);
    this.ph = ex.civ_planet_h(this.w);
    this.slots = ex.civ_faction_slots();
    this.fstride = ex.civ_faction_stride();
    this.vstride = ex.civ_village_stride();
    this.estride = ex.civ_event_stride();
    this.nameLen = ex.civ_name_len();
    this.statsLen = ex.civ_stats_len();
    this.miniW = ex.civ_minimap_w();
    this.miniH = ex.civ_minimap_h();
    this.view = new ImageData(320, 180);
    this.mini = new ImageData(this.miniW, this.miniH);
    this.decoder = new TextDecoder();
  }

  get mem() { return this.ex.memory.buffer; }

  // stress 0..1. Until #69 renames it, moc-civ still takes "bpm over resting" (its pulse() is that / 25),
  // so stress goes in on that 0..25 scale.
  setInputs(calm, focus, stress, anger) { this.ex.civ_set_inputs(this.w, calm || 0, focus || 0, (stress || 0) * 25, anger || 0); }
  step(dt) { this.ex.civ_step(this.w, dt); }
  get time() { return this.ex.civ_time(this.w); }
  get agents() { return this.ex.civ_agent_count(this.w); }

  // The camera's 320×180 view as ImageData (copied out of wasm memory).
  render() {
    this.ex.civ_render_view(this.w);
    const ptr = this.ex.civ_view_ptr(this.w), len = this.ex.civ_view_len(this.w);
    this.view.data.set(new Uint8ClampedArray(this.mem, ptr, len));
    return this.view;
  }

  minimap() {
    this.ex.civ_render_minimap(this.w);
    const ptr = this.ex.civ_minimap_ptr(this.w);
    this.mini.data.set(new Uint8ClampedArray(this.mem, ptr, this.miniW * this.miniH * 4));
    return this.mini;
  }

  // ---- camera (planet pixels; the view's left edge wraps, its top is clamped) ----
  pan(dx, dy) { this.ex.civ_cam_pan(this.w, dx, dy); }
  centre(x, y) { this.ex.civ_cam_centre(this.w, x, y); }
  get camX() { return this.ex.civ_cam_x(this.w); }
  get camY() { return this.ex.civ_cam_y(this.w); }
  toScreen(x, y) {
    let sx = (((x - this.camX) % this.pw) + this.pw) % this.pw;
    if (sx > this.pw / 2) sx -= this.pw;
    if (sx < -320) sx += this.pw;
    return { x: sx, y: y - this.camY };
  }
  toPlanet(sx, sy) {
    return { x: ((this.camX + sx) % this.pw + this.pw) % this.pw, y: Math.min(this.ph - 1, Math.max(0, this.camY + sy)) };
  }

  // ---- the god's hand ----
  lightning(x, y) { return this.ex.civ_lightning(this.w, x, y); }
  cast(kind, x, y) { return this.ex.civ_cast(this.w, kind, x, y); }
  seedMind(calm, focus, stress, blinks) { return this.ex.civ_seed_player_mind(this.w, calm, focus, (stress || 0) * 25, blinks); }
  // Calibration hook (#54): friendliness, aggressiveness, violence for the player's civilisation.
  setPlayerTraits(friendliness, aggressiveness, violence) { return this.ex.civ_set_player_traits(this.w, friendliness, aggressiveness, violence); }

  // ---- reading the world ----
  factions() {
    const n = this.slots, s = this.fstride;
    const f = new Float32Array(this.mem, this.ex.civ_factions_ptr(this.w), n * s).slice();
    const names = new Uint8Array(this.mem, this.ex.civ_names_ptr(this.w), n * this.nameLen).slice();
    const out = [];
    for (let i = 0; i < n; i++) {
      const o = i * s;
      if (f[o] === 0) continue;
      const raw = names.subarray(i * this.nameLen, (i + 1) * this.nameLen);
      const end = raw.indexOf(0);
      const color = [f[o + 3], f[o + 4], f[o + 5]];
      out.push({
        id: i, alive: f[o] > 0, pop: f[o + 1], villages: f[o + 2], color, css: css(color),
        capital: f[o + 6] >= 0 ? { x: f[o + 6], y: f[o + 7] } : null,
        friendliness: f[o + 8], aggressiveness: f[o + 9], violence: f[o + 10], fertility: f[o + 11], cohesion: f[o + 12],
        parent: f[o + 13], wars: f[o + 14], raids: f[o + 15], food: f[o + 16], kills: f[o + 17], losses: f[o + 18],
        unrest: f[o + 19], seedSource: SEED_SOURCE[f[o + 20]] || '?', region: f[o + 21], isPlayer: f[o + 22] > 0, warriors: f[o + 23],
        name: this.decoder.decode(end < 0 ? raw : raw.subarray(0, end)),
        seed: this.ex.civ_faction_seed(this.w, i) >>> 0,
        start: { x: this.ex.civ_faction_start_x(this.w, i), y: this.ex.civ_faction_start_y(this.w, i) },
      });
    }
    return out;
  }

  villages() {
    const n = this.ex.civ_village_count(this.w), s = this.vstride;
    const v = new Float32Array(this.mem, this.ex.civ_villages_ptr(this.w), n * s);
    const out = [];
    for (let i = 0; i < n; i++) {
      const o = i * s;
      if (v[o + 7] === 0) continue;
      out.push({ id: i, x: v[o], y: v[o + 1], owner: v[o + 2], pop: v[o + 3], food: v[o + 4], unrest: v[o + 5], capital: v[o + 6] > 0, siege: v[o + 7] === 2 });
    }
    return out;
  }

  // New log entries since the last call (the log is cleared).
  events() {
    const n = this.ex.civ_events_len(this.w);
    if (!n) return [];
    const e = new Int32Array(this.mem, this.ex.civ_events_ptr(this.w), n * this.estride);
    const out = [];
    for (let i = 0; i < n; i++) {
      const o = i * this.estride;
      out.push({ kind: e[o], a: e[o + 1], b: e[o + 2], x: e[o + 3], y: e[o + 4], n: e[o + 5], t: e[o + 6] / 1000 });
    }
    this.ex.civ_events_clear(this.w);
    return out;
  }

  stats() {
    const s = new Float32Array(this.mem, this.ex.civ_stats_ptr(this.w), this.statsLen);
    const keys = ['time', 'agents', 'villages', 'wars', 'peaces', 'raids', 'captures', 'rebellions', 'contacts', 'wiped', 'founded',
      'births', 'deaths', 'starved', 'oldAge', 'killed', 'trades', 'fieldBuilds', 'civs', 'fires', 'rain', 'peakAgents', 'smitten', 'warriors'];
    return Object.fromEntries(keys.map((k, i) => [k, s[i]]));
  }

  warState(a, b) { return this.ex.civ_war_state(this.w, a, b); }
  tension(a, b) { return this.ex.civ_tension(this.w, a, b); }
  ownerAt(x, y) { return this.ex.civ_owner_at(this.w, x, y); }

  free() { if (this.w) { this.ex.civ_free(this.w); this.w = 0; } }
}
