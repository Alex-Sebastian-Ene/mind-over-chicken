// The WebGL HUD (objectives, clock, meters, advisor text) is drawn by the Rust shell inside
// Engine.game_tick, from the Rust era sim. That sim ran beside the JS scene on the old fixed
// 320×180 map, so the HUD said "Fire out · Tribe 10/15 · Continents 1/4" next to a lit fire on a
// 640×360 world, and the Rust advisor, completion and failure read state nobody could see (#191).
// The JS scene on screen is authoritative: each frame it hands its objectives and clock to Rust
// (Engine.game_mirror), and the Rust era sim stands still. See GameFlow::mirrored in
// crates/engine/src/eras/game.rs.

// A scene's objectives as the rows game_mirror takes: flags bit 1 done, 2 bad, 4 plain.
export function objectiveRows(scene) {
  const list = (typeof scene?.objectives === 'function' && scene.objectives()) || [];
  const labels = [], values = [];
  const flags = new Uint8Array(list.length);
  list.forEach((o, i) => {
    labels.push(String(o.label ?? ''));
    values.push(String(o.value ?? ''));
    flags[i] = (o.done ? 1 : 0) | (o.bad ? 2 : 0) | (o.plain ? 4 : 0);
  });
  return { labels, values, flags };
}

// Mirror the HUD scene into the Rust shell. False when the engine can't (no WebGL, an old pkg/).
export function mirrorHud(engine, runTime, scene) {
  if (typeof engine?.game_mirror !== 'function') return false;
  const r = objectiveRows(scene);
  engine.game_mirror(runTime || 0, r.labels, r.values, r.flags);
  return true;
}
