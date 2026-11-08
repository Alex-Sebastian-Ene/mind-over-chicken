// The fail screen's state (#172). The Rust era sim runs beside the JS scene and keeps reporting
// its failure until the era restarts. The frame loop used to re-open the fail screen whenever it
// was hidden, so opening Controls from it brought it straight back on top, and Retry then left
// the new run paused behind the controls card. A run fails once: G.failing is set when the fail
// screen opens and cleared when the era (re)starts, so hiding the screen never re-fails it.

// The engine's fail reason to show now, or null.
export function engineFailToShow(engine, G) {
  if (!G || G.finished || G.failing) return null;
  return engine?.is_game_era_failed?.() || null;
}
