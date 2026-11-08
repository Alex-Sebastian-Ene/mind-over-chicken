// Older building types overlap into later eras: an Industry town keeps some of its old huts and
// a Space-age town keeps some Industry houses, so progress reads as growth, not a replacement.
// Pure: the era's mix of building types, and a pick from it for a 0..1 roll. Used by the home
// village (src/world/terrain.js) and other worlds' settlements (src/cosmos/surface.js).

export const ERA_ORDER = ['stone', 'farming', 'industry', 'space'];

// Roof colours by the era a house belongs to (an Industry house keeps its red roof in Space).
export const ROOFS = {
  farming: ['#8e3b2a', '#a4402f'],
  industry: ['#a4402f', '#8e3b2a', '#b5543a', '#6f4a3a'],
  space: ['#5a6b8c', '#7f8fa8', '#4d5d7a'],
};

// Each era's town: { kind, from, share }. `from` is the era the building type belongs to, so a
// `from` older than the era is a holdover. Shares sum to 1.
const MIX = {
  stone: [{ kind: 'hut', from: 'stone', share: 1 }],
  farming: [{ kind: 'hut', from: 'farming', share: 1 }],
  industry: [
    { kind: 'house', from: 'industry', share: 0.75 },
    { kind: 'hut', from: 'farming', share: 0.25 },     // the old huts are still standing
  ],
  space: [
    { kind: 'house', from: 'space', share: 0.45 },
    { kind: 'tower', from: 'space', share: 0.15 },
    { kind: 'house', from: 'industry', share: 0.28 },  // red-roofed Industry terraces
    { kind: 'hut', from: 'farming', share: 0.12 },     // and a few huts nobody will knock down
  ],
};

export function buildingMix(era) {
  return (MIX[era] || MIX.stone).map((b) => ({ ...b }));
}

// The share of an era's buildings that belong to older eras.
export function holdoverShare(era) {
  const at = ERA_ORDER.indexOf(era);
  return buildingMix(era).reduce((s, b) => s + (ERA_ORDER.indexOf(b.from) < at ? b.share : 0), 0);
}

// Pick a building type for a roll in [0, 1): { kind, from }.
export function pickBuilding(era, roll) {
  const mix = MIX[era] || MIX.stone;
  let acc = 0;
  for (const b of mix) {
    acc += b.share;
    if (roll < acc) return { kind: b.kind, from: b.from };
  }
  const last = mix[mix.length - 1];
  return { kind: last.kind, from: last.from };
}

// A roof for a house of the era it belongs to, from a 0..1 roll.
export function roofFor(from, roll) {
  const list = ROOFS[from] || ROOFS.farming;
  return list[Math.min(list.length - 1, Math.floor(roll * list.length))];
}
