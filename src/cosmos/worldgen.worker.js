// Generates a World off the main thread so new planets never stall the game. With `vast`, the
// World is a galaxy planet: its 640×360 surface (#68) and the 320×180 capital cut from it, all
// generated in Rust (crates/moc-terrain via the engine wasm, #109).
import { World, initTerrain } from '../world/terrain.js';

const ready = initTerrain();

self.onmessage = async (e) => {
  const { id, seed, biome, vast, width, height } = e.data;
  try {
    await ready;
    const w = vast ? World.planet(seed, biome) : new World(seed, { biome, width, height });
    const msg = { id, seed: w.seed, biome: w.biome, width: w.width, height: w.height, h: w.h, type: w.type, baseType: w.baseType, owner: w.owner, rgba: w.rgba, sites: w.sites, continents: w.continents, stats: w.stats };
    const transfer = [w.h.buffer, w.type.buffer, w.baseType.buffer, w.owner.buffer, w.rgba.buffer];
    if (w.vast) {
      msg.vast = w.vast;
      transfer.push(w.vast.type.buffer, w.vast.rgba.buffer);
    }
    self.postMessage(msg, transfer);
  } catch (err) {
    self.postMessage({ id, error: String(err) });
  }
};
