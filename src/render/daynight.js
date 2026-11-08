// Day and night for the 3D world (#201). The sun climbs, crosses the sky and sets over a
// DAY_SECONDS day (the run starts mid-morning); after dusk the shaders' moonlit night takes over.
// Nights stay readable (night is capped below 1), and an era's own darkness (the Stone Age before
// its fire is lit) always wins, since it is part of the play, not the clock.
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

export const DAY_SECONDS = 150;
export const NIGHT_MAX = 0.75;

export function dayNight(t, eraDark = 0) {
  const phase = ((t || 0) / DAY_SECONDS) * Math.PI * 2 + 0.6;
  const elev = Math.sin(phase);                          // the sun's height: 1 noon, -1 midnight
  const sun = { x: Math.cos(phase), y: elev, z: 0.45 };   // an arc across the sky
  const night = Math.min(NIGHT_MAX, clamp01((0.15 - elev) * 1.6)); // dusk begins just above the horizon
  return { sun, elev, night: Math.max(clamp01(eraDark), night) };
}
