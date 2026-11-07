// Asynchronous world generation: a module worker builds the World and the main thread
// rehydrates it. Falls back to building it here if workers aren't available.

import { World, BIOMES } from '../world/terrain.js';

let worker = null;
let workerBroken = false;
let nextId = 1;
const pending = new Map();

function getWorker() {
  if (worker || workerBroken) return worker;
  try {
    worker = new Worker(new URL('./worldgen.worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = (e) => {
      const job = pending.get(e.data.id);
      if (!job) return;
      pending.delete(e.data.id);
      if (e.data.error) job.resolve(new World(job.seed, { biome: job.biome }));
      else job.resolve(rehydrate(e.data));
    };
    worker.onerror = () => {
      workerBroken = true;
      worker = null;
      for (const [id, job] of pending) { pending.delete(id); job.resolve(new World(job.seed, { biome: job.biome })); }
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
  return w;
}

export function generateWorld(seed, biome) {
  const wk = getWorker();
  if (!wk) return Promise.resolve(new World(seed, { biome }));
  return new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, { resolve, seed, biome });
    wk.postMessage({ id, seed, biome });
  });
}
