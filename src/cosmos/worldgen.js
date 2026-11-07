// Asynchronous world generation: a module worker builds the World (and, for galaxy planets, its
// vaster surface, #68) and the main thread rehydrates it. Falls back to building it here if
// workers aren't available.

import { World, BIOMES } from '../world/terrain.js';
import { buildVast, paintVast } from './vast.js';

let worker = null;
let workerBroken = false;
let nextId = 1;
const pending = new Map();

// Same work as the worker, on this thread (slow: only when workers fail).
function buildHere(seed, biome, vast) {
  const w = new World(seed, { biome });
  if (vast) {
    const v = buildVast(w, seed);
    w.vast = { w: v.w, h: v.h, tile: v.tile, type: v.type, continents: v.continents, spots: v.spots, rgba: paintVast(v, w.biome, seed) };
  }
  return w;
}

function getWorker() {
  if (worker || workerBroken) return worker;
  try {
    worker = new Worker(new URL('./worldgen.worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = (e) => {
      const job = pending.get(e.data.id);
      if (!job) return;
      pending.delete(e.data.id);
      if (e.data.error) job.resolve(buildHere(job.seed, job.biome, job.vast));
      else job.resolve(rehydrate(e.data));
    };
    worker.onerror = () => {
      workerBroken = true;
      worker = null;
      for (const [id, job] of pending) { pending.delete(id); job.resolve(buildHere(job.seed, job.biome, job.vast)); }
    };
  } catch {
    workerBroken = true;
    worker = null;
  }
  return worker;
}

function rehydrate(d) {
  const w = Object.create(World.prototype);
  Object.assign(w, {
    seed: d.seed, biome: d.biome, palette: BIOMES[d.biome] || BIOMES.temperate,
    h: d.h, type: d.type, baseType: d.baseType, owner: d.owner,
    sites: d.sites, continents: d.continents, layers: {},
  });
  if (d.vast) w.vast = d.vast;
  return w;
}

// opts.vast: also build the planet's vaster surface (world.vast).
export function generateWorld(seed, biome, { vast = false } = {}) {
  const wk = getWorker();
  if (!wk) return Promise.resolve(buildHere(seed, biome, vast));
  return new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, { resolve, seed, biome, vast });
    wk.postMessage({ id, seed, biome, vast });
  });
}
