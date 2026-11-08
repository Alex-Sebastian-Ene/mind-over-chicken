// Asynchronous world generation: a module worker builds the World (and, for galaxy planets, its
// vaster surface, #68) in Rust and the main thread rehydrates it. Falls back to building it here
// if workers aren't available.

import { World, BIOMES } from '../world/terrain.js';
import { loading } from '../ui/loading.js'; // loading screen

let worker = null;
let workerBroken = false;
let nextId = 1;
const pending = new Map();

// Same work as the worker, on this thread (slow: only when workers fail).
function buildHere(seed, biome, vast, width, height) {
  return vast ? World.planet(seed, biome) : new World(seed, { biome, width, height });
}

function getWorker() {
  if (worker || workerBroken) return worker;
  try {
    worker = new Worker(new URL('./worldgen.worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = (e) => {
      const job = pending.get(e.data.id);
      if (!job) return;
      pending.delete(e.data.id);
      if (e.data.error) job.resolve(buildHere(job.seed, job.biome, job.vast, job.width, job.height));
      else job.resolve(rehydrate(e.data));
    };
    worker.onerror = () => {
      workerBroken = true;
      worker = null;
      for (const [id, job] of pending) { pending.delete(id); job.resolve(buildHere(job.seed, job.biome, job.vast, job.width, job.height)); }
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
    width: d.width, height: d.height, w: d.width,
    h: d.h, type: d.type, baseType: d.baseType, owner: d.owner, rgba: d.rgba,
    sites: d.sites, continents: d.continents, stats: d.stats, layers: {},
  });
  if (d.vast) w.vast = d.vast;
  return w;
}

// opts.vast: a galaxy planet (its 640×360 surface as world.vast, the capital as the World);
// otherwise a home world of opts.width × opts.height (the game's size by default). Off the main
// thread, so even a big world never stalls a frame.
// opts.track: show it in the loading pill (the galaxy's forming queue shows its own step instead).
export function generateWorld(seed, biome, { vast = false, width, height, track = true } = {}) {
  const wk = getWorker();
  if (!wk) return Promise.resolve(buildHere(seed, biome, vast, width, height));
  const id = nextId++;
  const job = new Promise((resolve) => {
    pending.set(id, { resolve, seed, biome, vast, width, height });
    wk.postMessage({ id, seed, biome, vast, width, height });
  });
  if (!track) return job;
  // loading screen: the corner pill counts planets forming ("Forming 3 planets").
  return loading.track(`world-${id}`, 'Forming a planet', job, { group: 'worlds', plural: 'Forming {n} planets', doneText: 'Planet formed' });
}
