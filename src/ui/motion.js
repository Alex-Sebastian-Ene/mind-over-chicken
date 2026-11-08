// Reduced motion and photosensitivity (#90). The flag is body[data-reduced-motion="1"], set by
// Settings (#186) from the player's choice, or the system's prefers-reduced-motion until they
// choose. Under it: no screen shake, no camera sweeps (the era intro, the 3D zoom tween), CSS
// animations off (styles.css), and lightning and screen flashes barely brighten the screen.

export const FLASH_CAP = 0.6;           // a full-screen flash never goes brighter than this
export const FLASH_CAP_REDUCED = 0.15;  // and under reduced motion it is only a hint

export function reducedMotion(doc = globalThis.document) {
  return doc?.body?.dataset?.reducedMotion === '1';
}

// The full-screen flash alpha to draw (lightning, a launch, a crash), capped for photosensitivity.
export function flashAlpha(flash, reduced = false) {
  const f = Number.isFinite(flash) && flash > 0 ? flash : 0;
  return Math.min(f, reduced ? FLASH_CAP_REDUCED : FLASH_CAP);
}

// Screen-shake amplitude (screen units) for G.shake; none under reduced motion.
export function shakeAmount(shake, reduced = false) {
  return reduced || !(shake > 0) ? 0 : shake * 0.008;
}
