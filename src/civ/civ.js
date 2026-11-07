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
  // The ages and conflict (#69).
  AGE_UP: 15, COLLAPSE: 16, ESCALATE: 17, DEESCALATE: 18, STRIFE: 19, CIVIL_WAR: 20, CRUSHED: 21,
  REVOLUTION: 22, VASSAL: 23, TRUCE: 24, CRISIS: 25, PLAGUE: 26, CAPTIVES: 27, BATTLE: 28,
  FESTIVAL: 29, COLONY: 30, INDEPENDENCE: 31, SUCCESSION: 32,
};
// News line tones (Rust `news::TONE_*`).
export const TONE = { info: 0, good: 1, bad: 2, warn: 3 };
export const CAST = { fireball: 1, spawn: 2, peace: 3, disaster: 4, bless: 5, rain: 6 };
export const SEED_SOURCE = ['the run seed', 'calibration', 'your mind', 'rebellion'];

export function loadCivModule() {
  compiled ??= fetch(WASM_URL)
    .then((r) => { if (!r.ok) throw new Error(`moc_civ.wasm: HTTP ${r.status}`); return r.arrayBuffer(); })
    .then((bytes) => WebAssembly.compile(bytes));
  return compiled;
}

// width/height 0 = the default planet (1280×640). staged: the civilisations wait to be
// chosen, placed and seeded by the player one at a time (#98).
// rivals: civilisations that arrive on their own at staggered times (#101).
export async function createCiv(seed, { width = 0, height = 0, staged = false, rivals = 0 } = {}) {
  const module = await loadCivModule();
  const instance = await WebAssembly.instantiate(module, {});
  return new Civ(instance.exports, seed, width, height, staged, rivals);
}

// The founding flow's phases (Rust `setup::Phase`).
export const SETUP = { off: 0, choose: 1, place: 2, seed: 3, done: 4 };

const css = (c) => `rgb(${c[0]},${c[1]},${c[2]})`;

export class Civ {
  constructor(ex, seed, width, height, staged, rivals) {
    this.ex = ex;
    this.seed = seed >>> 0;
    this.w = staged ? ex.civ_new_staged_with(this.seed, width, height, 4, rivals) : ex.civ_new_sized(this.seed, width, height);
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

  // UTF-8 text out of wasm memory.
  str(ptr, len) { return len ? this.decoder.decode(new Uint8Array(this.mem, ptr, len).slice()) : ''; }

  get mem() { return this.ex.memory.buffer; }

  // stress: 0..1, 0 at rest (the brain's `stress`, or the sandbox's fallback from calm and anger).
  setInputs(calm, focus, stress, anger) { this.ex.civ_set_inputs(this.w, calm || 0, focus || 0, stress || 0, anger || 0); }
  step(dt) { this.ex.civ_step(this.w, dt); }
  // Real seconds at the world's speed (×1, ×4, ×16), in steps of at most 0.1 s.
  advance(dt) { this.ex.civ_advance(this.w, dt); }
  cycleSpeed() { return this.ex.civ_cycle_speed(this.w); }
  get lastSteps() { return this.ex.civ_last_steps(this.w); }
  get speed() { return this.ex.civ_speed(this.w); }
  setJoy(j) { this.ex.civ_set_joy(this.w, j || 0); }

  // ---- words, written by the Rust side (news.rs) ----
  // News lines numbered from `from` up: {seq, kind, tone, log, priority, cooldown, x, y, text, logText, say, key}.
  news(from) {
    const ex = this.ex, w = this.w, next = ex.civ_news_next(w), out = [];
    for (let s = Math.max(from, ex.civ_news_first(w)); s < next; s++) {
      if (ex.civ_news_meta(w, s, 0) < 0) continue;
      const str = (k) => this.str(ex.civ_news_str_ptr(w, s, k), ex.civ_news_str_len(w, s, k));
      out.push({
        seq: s, kind: ex.civ_news_meta(w, s, 0), tone: ex.civ_news_meta(w, s, 1), log: ex.civ_news_meta(w, s, 2) > 0,
        priority: ex.civ_news_meta(w, s, 3), cooldown: ex.civ_news_meta(w, s, 4), x: ex.civ_news_meta(w, s, 5), y: ex.civ_news_meta(w, s, 6),
        text: str(0), logText: str(1), say: str(2), key: str(3),
      });
    }
    return { lines: out, next };
  }
  // The HUD rows: [{label, value, bad}], each people with its age.
  hudRows() {
    const len = this.ex.civ_hud_build(this.w);
    return this.str(this.ex.civ_text_buf_ptr(this.w), len).split('\n').filter(Boolean).map((r) => {
      const [label, value, bad] = r.split('\t');
      return { label, value, bad: bad === '1' };
    });
  }
  capitalLabel(f) { const len = this.ex.civ_label_build(this.w, f); return this.str(this.ex.civ_text_buf_ptr(this.w), len); }

  // ---- founding the civilisations one at a time (#98; the flow lives in Rust, setup.rs) ----
  get setupPhase() { return this.ex.civ_setup_phase(this.w); }
  setupHover(x, y) { return this.ex.civ_setup_hover(this.w, x, y) > 0; }
  setupClick(x, y) { return this.ex.civ_setup_click(this.w, x, y); }
  setupEnter() { return this.ex.civ_setup_enter(this.w); }
  setupNext() { this.ex.civ_setup_next(this.w); }
  setupBlink() { this.ex.civ_setup_blink(this.w); }
  // What to do now, then a line per civilisation.
  setupLines() { const len = this.ex.civ_setup_text_build(this.w); return this.str(this.ex.civ_text_buf_ptr(this.w), len).split('\n').filter(Boolean); }
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
  seedMind(calm, focus, stress, blinks) { return this.ex.civ_seed_player_mind(this.w, calm, focus, stress, blinks); }
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
      if (f[o] === 0 || f[o] === 2) continue;   // unused, or waiting to be founded (#98)
      const raw = names.subarray(i * this.nameLen, (i + 1) * this.nameLen);
      const end = raw.indexOf(0);
      const color = [f[o + 3], f[o + 4], f[o + 5]];
      out.push({
        id: i, alive: f[o] > 0, pop: f[o + 1], villages: f[o + 2], color, css: css(color),
        capital: f[o + 6] >= 0 ? { x: f[o + 6], y: f[o + 7] } : null,
        friendliness: f[o + 8], aggressiveness: f[o + 9], violence: f[o + 10], fertility: f[o + 11], cohesion: f[o + 12],
        parent: f[o + 13], wars: f[o + 14], raids: f[o + 15], food: f[o + 16], kills: f[o + 17], losses: f[o + 18],
        unrest: f[o + 19], seedSource: SEED_SOURCE[f[o + 20]] || '?', region: f[o + 21], isPlayer: f[o + 22] > 0, warriors: f[o + 23],
        age: f[o + 24], dark: f[o + 36], strife: f[o + 38],
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
      'births', 'deaths', 'starved', 'oldAge', 'killed', 'trades', 'fieldBuilds', 'civs', 'fires', 'rain', 'peakAgents', 'smitten', 'warriors',
      'ageUps', 'collapses', 'recoveries', 'strifes', 'civilWars', 'crushed', 'revolutions', 'escalations', 'battles', 'captives', 'vassals',
      'plagues', 'crises', 'festivals', 'brawls', 'routs', 'maxAge', 'speed', 'night', 'buildings'];
    return Object.fromEntries(keys.map((k, i) => [k, s[i]]));
  }

  warState(a, b) { return this.ex.civ_war_state(this.w, a, b); }
  tension(a, b) { return this.ex.civ_tension(this.w, a, b); }
  ownerAt(x, y) { return this.ex.civ_owner_at(this.w, x, y); }

  free() { if (this.w) { this.ex.civ_free(this.w); this.w = 0; } }
}
