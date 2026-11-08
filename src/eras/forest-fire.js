// Forest fires burn out by themselves. A burning tree lasts about 10 s and tries to light a
// neighbour at most twice; each generation is less likely to spread (falloff) and none spreads
// past maxGen, so a fire from a lightning strike dies down in about half a minute even in thick
// forest. Rain still puts it out at once. Before this, every burning tree lit the nearest tree
// every couple of seconds, so a dense forest could burn indefinitely. Pure so it can be tested.
export const FOREST_FIRE = { life: 10, spreads: 2, falloff: 0.6, maxGen: 4 };

// A newly burning tree, `gen` steps from the strike that started the fire.
export function forestFire(x, y, gen = 0, rnd = Math.random) {
  return { x, y, heat: 0.9, t: 0, gen, spreads: 0, spread: 0.9 + rnd() * 0.7 };
}

// Advance one burning tree by dt; true when it should light a neighbouring tree now.
export function forestFireStep(f, dt, rnd = Math.random) {
  f.t += dt;
  f.heat = Math.max(0, 0.9 * (1 - f.t / FOREST_FIRE.life));
  f.spread -= dt;
  if (f.heat <= 0 || f.spread > 0 || f.spreads >= FOREST_FIRE.spreads || f.gen >= FOREST_FIRE.maxGen) return false;
  f.spread = 1.4 + rnd() * 0.8;
  f.spreads++;
  return rnd() < FOREST_FIRE.falloff ** f.gen;
}
