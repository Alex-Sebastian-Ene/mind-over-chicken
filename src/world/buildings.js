// #206: biome/era building variants.
//
// Pure, deterministic helpers shared by the world generator, the 2D sprite layer and the 3D
// billboard path. Nothing here touches simulation rules or the world RNG: every choice is a
// hash of the world seed, the building's map position, the biome and the era, so the same
// inputs always produce the same settlement.

// The five building biomes the world is classified into. `biomeAt` picks the first match in
// this order, so a snowy ridge wins over the coast it also touches.
export const BIOME_CATEGORIES = ['snow', 'coast', 'hills', 'forest', 'plains'];

// Terrain tile values mirrored from src/world/terrain.js `T`, so this module stays a leaf with
// no imports (terrain.js imports us). Keep in sync with T.
const T = { SHALLOW: 1, FOREST: 4, HILL: 5, MOUNTAIN: 6, SNOW: 7 };

// Every structure kind the world generator may place. `sprite` is the engine's atlas sprite id
// (crates/engine/src/sprites.rs); `draw` the 2D function in src/render/sprites.js. `variants`
// is the number of deterministic visual sub-variants (roofs, doors, trim).
export const STRUCTURE_KINDS = {
  // Existing core kinds.
  hut: { era: 'stone', sprite: 5, draw: 'drawHut', variants: 2 },
  house: { era: 'any', sprite: 6, draw: 'drawHouse', variants: 3 },
  tower: { era: 'any', sprite: 7, draw: 'drawTower', variants: 2 },
  factory: { era: 'industry', sprite: 8, draw: 'drawFactory', variants: 1 },
  exchange: { era: 'industry', sprite: 9, draw: 'drawExchange', variants: 1 },
  hangar: { era: 'space', sprite: 10, draw: 'drawHangar', variants: 1 },
  pad: { era: 'space', sprite: 11, draw: 'drawPad', variants: 1 },
  // Stone Age.
  longhouse: { era: 'stone', sprite: 16, draw: 'drawLonghouse', variants: 3 },
  pueblo: { era: 'stone', sprite: 17, draw: 'drawPueblo', variants: 3 },
  stilthut: { era: 'stone', sprite: 18, draw: 'drawStiltHut', variants: 2 },
  snowcabin: { era: 'stone', sprite: 19, draw: 'drawSnowCabin', variants: 2 },
  henge: { era: 'stone', sprite: 20, draw: 'drawHenge', variants: 2 },
  lodge: { era: 'stone', sprite: 21, draw: 'drawLodge', variants: 2 },
  // Industry.
  watermill: { era: 'industry', sprite: 22, draw: 'drawWatermill', variants: 2 },
  windmill: { era: 'industry', sprite: 23, draw: 'drawWindmill', variants: 2 },
  terraced: { era: 'industry', sprite: 24, draw: 'drawTerraced', variants: 3 },
  markethall: { era: 'industry', sprite: 25, draw: 'drawMarketHall', variants: 2 },
  clocktower: { era: 'industry', sprite: 26, draw: 'drawClockTower', variants: 2 },
  warehouse: { era: 'industry', sprite: 27, draw: 'drawWarehouse', variants: 2 },
  // Space.
  solar: { era: 'space', sprite: 28, draw: 'drawSolarArray', variants: 2 },
  dome: { era: 'space', sprite: 29, draw: 'drawDome', variants: 2 },
  commsdish: { era: 'space', sprite: 30, draw: 'drawCommsDish', variants: 2 },
  cargodepot: { era: 'space', sprite: 31, draw: 'drawCargoDepot', variants: 2 },
};

// kind → atlas sprite id, for the 3D billboard path.
export const STRUCTURE_SPRITES = Object.fromEntries(
  Object.entries(STRUCTURE_KINDS).map(([kind, def]) => [kind, def.sprite]),
);

// Biome-weighted pools per era. Repeated entries make a kind common; a pool always names the
// core kind as a fallback so existing worlds keep their huts and houses.
export const KIND_POOLS = {
  stone: {
    coast: ['stilthut', 'stilthut', 'hut', 'hut', 'longhouse'],
    hills: ['pueblo', 'pueblo', 'hut', 'hut', 'henge'],
    forest: ['lodge', 'lodge', 'longhouse', 'hut', 'hut'],
    snow: ['snowcabin', 'snowcabin', 'hut', 'longhouse'],
    plains: ['hut', 'hut', 'longhouse', 'pueblo', 'henge'],
  },
  farming: {
    coast: ['stilthut', 'longhouse', 'hut', 'house'],
    hills: ['pueblo', 'longhouse', 'house', 'hut'],
    forest: ['lodge', 'longhouse', 'house', 'hut'],
    snow: ['snowcabin', 'longhouse', 'house', 'hut'],
    plains: ['hut', 'longhouse', 'house', 'pueblo', 'lodge'],
  },
  industry: {
    coast: ['watermill', 'watermill', 'warehouse', 'terraced', 'house'],
    hills: ['windmill', 'windmill', 'terraced', 'clocktower', 'house'],
    forest: ['lodge', 'warehouse', 'terraced', 'house', 'house'],
    snow: ['warehouse', 'terraced', 'house', 'house'],
    plains: ['terraced', 'terraced', 'house', 'house', 'windmill', 'markethall', 'clocktower', 'warehouse'],
  },
  space: {
    coast: ['dome', 'solar', 'cargodepot', 'house', 'tower'],
    hills: ['solar', 'solar', 'commsdish', 'dome', 'tower'],
    forest: ['dome', 'house', 'solar', 'cargodepot'],
    snow: ['dome', 'dome', 'house', 'solar'],
    plains: ['solar', 'dome', 'commsdish', 'cargodepot', 'house', 'house', 'tower'],
  },
};

const ERA_SALT = { stone: 0x51, farming: 0x71, industry: 0x91, space: 0xb1 };
const BIOME_SALT = { plains: 0, coast: 1, hills: 2, forest: 3, snow: 4 };

// A small, fast integer hash. Pure: the same inputs always give the same 32-bit value, which
// keeps every generated settlement stable across reloads without touching the world RNG.
export function structureHash(seed = 0, x = 0, y = 0, salt = 0) {
  let h = (seed >>> 0) ^ Math.imul((x | 0) + 0x9e3779b9, 0x85ebca6b)
    ^ Math.imul((y | 0) + 0x7f4a7c15, 0xc2b2ae35) ^ (salt | 0);
  h ^= h >>> 16; h = Math.imul(h, 0x7feb352d);
  h ^= h >>> 15; h = Math.imul(h, 0x846ca68b);
  h ^= h >>> 16;
  return h >>> 0;
}

// Map a terrain sample (or a planet's climate name) to one of BIOME_CATEGORIES.
export function biomeAt({ coast = false, hills = false, forest = false, snow = false } = {}) {
  if (snow) return 'snow';
  if (coast) return 'coast';
  if (hills) return 'hills';
  if (forest) return 'forest';
  return 'plains';
}

// Classify the biome around (x, y) from a wrapped terrain lookup. `typeAt(px, py)` returns a
// terrain tile; a polar/ice planet is snow everywhere. Radius is in world pixels, sampled every
// other pixel so the cost stays tiny.
export function sampleBiome(typeAt, x, y, planetBiome = 'temperate', radius = 5) {
  if (planetBiome === 'ice') return 'snow';
  let coast = false, hills = false, forest = false, snow = false;
  for (let dy = -radius; dy <= radius; dy += 2) {
    for (let dx = -radius; dx <= radius; dx += 2) {
      const t = typeAt(Math.round(x + dx), Math.round(y + dy));
      if (t == null) continue;
      if (t <= T.SHALLOW) coast = true;
      else if (t === T.SNOW) snow = true;
      else if (t === T.HILL || t === T.MOUNTAIN) hills = true;
      else if (t === T.FOREST) forest = true;
    }
  }
  // Planet climates that have no tile marker of their own lean on their terrain sample.
  if (planetBiome === 'desert' && !coast && !hills) return 'plains';
  if (planetBiome === 'jungle' && forest) return 'forest';
  return biomeAt({ coast, hills, forest, snow });
}

// Kinds people live in. Scenes that used to look for `kind === 'house'` should use this, so a
// settlement full of variants still gives every agent a home without changing any rule.
const RESIDENTIAL = new Set(['hut', 'house', 'longhouse', 'pueblo', 'stilthut', 'snowcabin', 'terraced', 'dome']);
export const isResidential = (kind) => RESIDENTIAL.has(kind);

// Kinds that should glow at night (the InfoAge/Industry city lights), visual only.
const LIT = new Set([
  'house', 'tower', 'longhouse', 'pueblo', 'stilthut', 'snowcabin', 'terraced', 'markethall',
  'clocktower', 'warehouse', 'dome', 'commsdish', 'cargodepot', 'factory', 'exchange', 'hangar',
]);
export const isLitBuilding = (kind) => LIT.has(kind);

// Deterministically pick the kind for the next building at (x, y) in `era`/`biome`.
export function chooseStructureKind(era, biome, seed, x, y, fallback = 'hut') {
  const pool = KIND_POOLS[era]?.[biome] || KIND_POOLS[era]?.plains;
  if (!pool || !pool.length) return fallback;
  const salt = (ERA_SALT[era] ?? 0) + (BIOME_SALT[biome] ?? 0) * 0x1f;
  return pool[structureHash(seed, x, y, salt) % pool.length];
}

// Deterministic visual sub-variant for a kind (roof shade, door/window layout, trim).
export function structureVariant(kind, { biome = 'plains', seed = 0, x = 0, y = 0, era } = {}) {
  const def = STRUCTURE_KINDS[kind];
  if (!def) return 0;
  const n = Math.max(1, def.variants || 1);
  const salt = (def.sprite * 0x101) + (ERA_SALT[era ?? def.era] ?? 0) + (BIOME_SALT[biome] ?? 0);
  return structureHash(seed, x, y, salt) % n;
}
