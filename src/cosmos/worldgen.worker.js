// Generates a World off the main thread (~150 ms each) so new planets never stall the game.
import { World } from '../world/terrain.js';

self.onmessage = (e) => {
  const { id, seed, biome } = e.data;
  try {
    const w = new World(seed, { biome });
    self.postMessage(
      { id, seed: w.seed, biome: w.biome, h: w.h, type: w.type, baseType: w.baseType, owner: w.owner, sites: w.sites, continents: w.continents },
      [w.h.buffer, w.type.buffer, w.baseType.buffer, w.owner.buffer],
    );
  } catch (err) {
    self.postMessage({ id, error: String(err) });
  }
};
