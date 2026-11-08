// The civ sandbox's planet sizes (#164) and the player's pick on the title screen (Alex: "the
// ability to change size"). DOM-free apart from initPlanetSizePicker, so tests can import it.

// The planet sizes a sandbox run can roll (Alex: different size planets).
export const PLANET_SIZES = [
  { name: 'small', w: 1280, h: 640, civs: 8 },
  { name: 'medium', w: 1920, h: 960, civs: 12 },
  { name: 'large', w: 2560, h: 1280, civs: 16 },
  { name: 'huge', w: 3072, h: 1536, civs: 20 },
];
// The player's pick on the title screen (Alex: "the ability to change size"): a named size, or
// null for a random one each run. Kept in localStorage; ?planet= in the URL still wins.
const PICK_KEY = 'moc.planetSize';
export function planetChoice(storage = globalThis.localStorage) {
  let v = null;
  try { v = storage?.getItem(PICK_KEY) ?? null; } catch {}
  return PLANET_SIZES.some((p) => p.name === v) ? v : null;
}
export function initPlanetSizePicker(root, storage = globalThis.localStorage) {
  if (!root) return;
  const buttons = [...root.querySelectorAll('[data-size]')];
  const show = () => {
    const pick = planetChoice(storage) || 'random';
    for (const b of buttons) b.setAttribute('aria-checked', String(b.dataset.size === pick));
  };
  for (const b of buttons) {
    b.addEventListener('click', () => {
      try { b.dataset.size === 'random' ? storage?.removeItem(PICK_KEY) : storage?.setItem(PICK_KEY, b.dataset.size); } catch {}
      show();
    });
  }
  show();
}

// The size for a run: a named size or WxH from the URL, else rolled from the seed.
export function planetSize(seed, arg = null) {
  const named = PLANET_SIZES.find((p) => p.name === arg);
  if (named) return { ...named };
  const m = /^(\d+)x(\d+)$/.exec(arg || '');
  if (m) {
    const [w, h] = [m[1], m[2]].map((v) => Math.max(320, Math.min(4096, Number(v))));
    // Peoples in proportion to the area, as the named sizes have them.
    return { name: `${w}x${h}`, w, h, civs: Math.max(4, Math.min(20, Math.round(8 * Math.sqrt((w * h) / (1280 * 640))))) };
  }
  let x = (seed ^ 0x9e3779b9) >>> 0;
  x = Math.imul(x ^ (x >>> 16), 0x45d9f3b) >>> 0;
  x = (x ^ (x >>> 16)) >>> 0;
  return { ...PLANET_SIZES[x % PLANET_SIZES.length] };
}

