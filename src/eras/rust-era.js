// Which Rust era sim (crates/engine/src/eras: 0 Stone, 1 Industry, 2 Space) runs under a scene
// (#194). The playable eras say so with `rustEra`; the time skips (FarmingToKingdoms, InfoAge) have
// none, so no era sim restarts or ticks during them. None ticks during an intro transition either,
// since the era is paused until it ends.
export const rustEraOf = (scene) => (Number.isInteger(scene?.rustEra) ? scene.rustEra : null);
export const rustEraTicks = (scene, introActive) => rustEraOf(scene) !== null && !introActive;
