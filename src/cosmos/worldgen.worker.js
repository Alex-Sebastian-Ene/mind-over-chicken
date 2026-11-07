// Generates a World off the main thread (~150 ms each) so new planets never stall the game.
// With `vast`, it also builds and paints the planet's vaster surface around it (#68, ~250 ms).
import { World } from '../world/terrain.js';
import { buildVast, paintVast } from './vast.js';

self.onmessage = (e) => {
  const { id, seed, biome, vast } = e.data;
  try {
    const w = new World(seed, { biome });
    const msg = { id, seed: w.seed, biome: w.biome, h: w.h, type: w.type, baseType: w.baseType, owner: w.owner, sites: w.sites, continents: w.continents };
    const transfer = [w.h.buffer, w.type.buffer, w.baseType.buffer, w.owner.buffer];
    if (vast) {
      const v = buildVast(w, seed);
      const rgba = paintVast(v, w.biome, seed);
      msg.vast = { w: v.w, h: v.h, tile: v.tile, type: v.type, continents: v.continents, spots: v.spots, rgba };
      transfer.push(v.type.buffer, rgba.buffer);
    }
    self.postMessage(msg, transfer);
  } catch (err) {
    self.postMessage({ id, error: String(err) });
  }
};
