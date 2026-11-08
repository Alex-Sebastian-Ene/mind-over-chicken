// The civ sandbox in space (#166): civilisations at age 19+ (Rust: crates/moc-civ/src/space.rs)
// fly their fleets in the galaxy's war (Rust: crates/moc-cosmos/src/war.rs, through
// src/cosmos/space-war.js). Twice a second this glue registers each civilisation in space as a
// power living on the sandbox's planet, hands it the planets it may strike, flies its launches,
// and hands back what the galaxy did to it (loot, losses, colonies). War news about the
// civilisations goes to the sandbox's ticker. Every decision is made in Rust.
//
// ?spacedebug=N puts the first N living civilisations in space at once (for testing; the climb
// to age 19 takes a while even at ×16).

const SYNC_EVERY = 0.5;

export class CivSpaceLink {
  constructor(scene) {
    this.scene = scene;
    this.t = 0;
    this.power = new Map();   // civilisation id → { key, id } of its power in the galaxy
    const q = new URLSearchParams(location.search);
    this.debug = Math.max(0, Math.min(20, Number(q.get('spacedebug')) || 0));
    this.debugged = false;
  }

  // A civilisation's key in the galaxy: its slot and seed (slots are reused by new peoples).
  static key(f) { return (f.id * 1000003 + (f.seed % 1000003)) >>> 0; }

  // Test hook (and ?spacedebug): put the first n living civilisations in space now.
  debugSpace(n = 4) {
    const civ = this.scene.civ;
    if (!civ) return 0;
    let k = 0;
    for (const f of this.scene.factions) if (f.alive && k < n && civ.debugSpace(f.id, true)) k++;
    return k;
  }

  tick(dt) {
    const scene = this.scene, civ = scene.civ, gal = scene.cosmos?.galaxy, war = gal?.war;
    if (!civ || !war || (this.t -= dt) > 0) return;
    this.t = SYNC_EVERY;
    if (this.debug && !this.debugged && !scene.setupActive() && scene.factions.some((f) => f.alive)) this.debugged = this.debugSpace(this.debug) > 0;
    const home = gal.planets.findIndex((p) => p.home);
    if (home < 0) return;
    war.setShared(home);
    for (const s of civ.spaceInfo()) {
      const f = scene.byId.get(s.id);
      const had = this.power.get(s.id);
      const key = f ? CivSpaceLink.key(f) : had?.key;
      if (had && (had.key !== key || !s.active || !f?.alive)) {
        // Out of space (a Dark Age), gone, or another people in the slot: retire its power.
        war.upsertCiv(had.key, '', [0, 0, 0], home, s, false);
        this.power.delete(s.id);
      }
      if (!s.active || !f?.alive) continue;
      const id = war.upsertCiv(key, f.name, f.color, home, s, true);
      if (id < 0) continue;
      this.power.set(s.id, { key, id });
      civ.spaceResult(s.id, war.takeResults(id));
      civ.setSpaceTargets(s.id, war.targetsFor(id));
    }
    for (const l of civ.spaceLaunches()) {
      const p = this.power.get(l.civ);
      if (p) war.launchExternal(p.id, l.body, l.order, l.strength, l.motive);
    }
    for (const n of war.takeCivNews()) scene.news.unshift({ text: n.text, t: scene.t, tone: n.tone, seen: false });
  }
}
