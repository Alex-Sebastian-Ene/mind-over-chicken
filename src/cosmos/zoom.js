// The zoom state machine (#68), pure: no DOM, so Node tests can drive it.
//
//   level 0 = a world's SURFACE   level 1 = its GLOBE   level 2 = the GALAXY
//
// `focus` is the world being looked at: null means the hand-played home campaign, otherwise a
// galaxy planet (plain object). Any world with a surface ready (`p.surfaceReady`) can be landed
// on; the hand-played home always can (except in the galaxy sandbox, where home is autonomous
// and lands like any other world once its surface is ready). Worlds still forming can't be
// focused at all.
//
//   galaxy ──click a planet / zoom in──▶ its globe ──zoom in──▶ its surface
//   surface ──zoom out──▶ globe ──zoom out──▶ galaxy
//
// `z` animates toward `target` at ZOOM_SPEED levels per second; renderers read `z`.

export const LEVELS = ['Surface', 'Planet', 'Galaxy'];
export const ZOOM_SPEED = 1.5;

const clampLevel = (v, lo, hi) => (v < lo ? lo : v > hi ? hi : v);

export class ZoomState {
  constructor() {
    this.z = 0;          // animated zoom, 0..2
    this.target = 0;     // requested level
    this.focus = null;   // planet in view (null = the hand-played home)
    this.sandbox = false;
    this.homeRef = null; // the galaxy's home planet (Cosmos overrides homePlanet())
  }

  homePlanet() { return this.homeRef; }

  // The hand-played campaign world (not an autonomous sandbox home).
  isCampaign(p) { return !p || (p.home && !p.autonomous); }
  // The world in view as a planet object (the home planet when focus is null).
  world() { return this.focus || this.homePlanet(); }
  // Can this world be shown up close right now?
  canLand(p = this.focus) {
    if (this.isCampaign(p) && !this.sandbox) return true;
    const w = p || this.homePlanet();
    return !!(w && !w.forming && w.surfaceReady);
  }
  minLevel() { return this.canLand() ? 0 : 1; }

  get active() { return this.z > 0.02 || this.target > 0; }
  get inGalaxy() { return this.z > 1.5; }
  // What the surface level shows: 'campaign' (the home game), 'ai' (another world), or null.
  surfaceKind() {
    if (this.isCampaign(this.focus) && !this.sandbox) return 'campaign';
    return this.canLand() ? 'ai' : null;
  }
  // An AI world's surface is on screen (fully or zooming).
  get aiSurfaceInView() { return this.z < 1 && this.surfaceKind() === 'ai'; }
  // The world the player acts on: every brain channel at full weight, blinks strike it.
  // The campaign surface takes its blinks directly in the scene, so it isn't "attended" there.
  attended() {
    if (this.z >= 1.5) return null;
    const w = this.world();
    if (!w) return null;
    if (this.z <= 0.5 && this.isCampaign(this.focus) && !this.sandbox) return null;
    return w;
  }

  // Ask for a level. Returns { level, refused } where refused explains a clamp, if any.
  zoomTo(level) {
    const want = Math.round(level);
    const lv = clampLevel(want, this.minLevel(), 2);
    let refused = null;
    if (want < lv) refused = this.world()?.forming ? 'Still forming. Give it a second.' : 'This world has no surface to land on yet.';
    this.target = lv;
    return { level: lv, refused };
  }
  zoomIn() { return this.zoomTo(this.target - 1); }
  zoomOut() { return this.zoomTo(this.target + 1); }

  // Pick a world (from the galaxy or the panel): show its globe.
  focusPlanet(p) {
    if (!p) return { ok: false, refused: 'No such world.' };
    if (p.forming) return { ok: false, refused: 'Still forming. Give it a second.' };
    this.focus = this.isCampaign(p) ? null : p;
    this.target = 1;
    if (this.z < 1) this.z = 1;
    return { ok: true, refused: null };
  }

  // The G key (#43): the next world to visit, cycling through every landable AI world and then
  // back to the hand-played home. `planets` is the galaxy's list.
  nextWorld(planets) {
    const ai = planets.filter((p) => !this.isCampaign(p) && !p.forming && p.surfaceReady);
    const home = planets.find((p) => this.isCampaign(p));
    const ring = home && !this.sandbox ? [...ai, home] : ai;
    if (!ring.length) return null;
    const here = this.focus || (this.sandbox ? null : home);
    const i = ring.indexOf(here);
    return ring[(i + 1) % ring.length];
  }

  update(dt) {
    if (this.z < this.target) this.z = Math.min(this.target, this.z + dt * ZOOM_SPEED);
    else if (this.z > this.target) this.z = Math.max(this.target, this.z - dt * ZOOM_SPEED);
    // A world that loses its surface (say it is still forming) can't stay landed.
    if (this.target < this.minLevel()) this.target = this.minLevel();
  }

  label() {
    const w = this.focus;
    const campaign = this.isCampaign(w) && !this.sandbox;
    if (this.target === 2) return LEVELS[2];
    if (this.target === 1) return campaign || !w ? (this.sandbox && !w ? 'Home' : LEVELS[1]) : w.name;
    return campaign ? LEVELS[0] : `${(w || this.homePlanet())?.name || 'World'} · surface`;
  }
}
