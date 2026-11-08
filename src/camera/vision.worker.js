// camera debug (#67): MediaPipe inference in a module Web Worker, so it never blocks the game.
//
// The main thread (engine.js) sends small video frames as transferred ImageBitmaps; this worker
// runs the Hand Landmarker (plus landmark classifier) and/or the Face Landmarker on them and posts back only landmarks,
// the top gesture and the 52 blendshape scores. Nothing leaves the device: the only network
// traffic is the one-time download of the pinned runtime and the two models, as on the main thread.
//
// Protocol
//   in   { type: 'init', tasks: ['gesture', 'face'], delegate?: 'GPU' | 'CPU' }   (GPU falls back to CPU)
//   out  { type: 'ready', delegates, timings } | { type: 'error', stage: 'init', message }
//   in   { type: 'frame', id, ts, bitmap, run: ['gesture', 'face'] }   (bitmap transferred, closed here)
//   out  { type: 'result', id, ts, ms: { gesture, face }, gesture?, face?, delegates }
//   in   { type: 'cpu', reason }   move any GPU task to the CPU
//   out  { type: 'warn' | 'info', message }

import { VISION_URLS, OPTIONS, withDelegate, quietInfoLogs } from './vision.js';
import { classifyHand } from './classify-hand.js';

quietInfoLogs();

// MediaPipe loads its WebAssembly glue with importScripts() when it runs in a worker, which module
// workers don't allow. Fetch the glue first and run the same script at global scope instead.
const scripts = new Map();
self.importScripts = (...urls) => {
  for (const url of urls) {
    const src = scripts.get(String(url));
    if (src == null) throw new Error(`[camera worker] script was not prefetched: ${url}`);
    (0, eval)(`${src}\n//# sourceURL=${url}`);
  }
};

const TASKS = { gesture: 'HandLandmarker', face: 'FaceLandmarker' };
let mp = null, fileset = null;
const tasks = {}, delegates = {}, lastTs = {}, rebuilding = {}, errors = {};

async function create(name, delegate) {
  return mp[TASKS[name]].createFromOptions(fileset, withDelegate(OPTIONS[name], delegate));
}

async function init(names, prefer = 'GPU') {
  const t0 = performance.now();
  const timings = {};
  mp = await import(VISION_URLS.bundle);
  fileset = await mp.FilesetResolver.forVisionTasks(VISION_URLS.wasm);
  const res = await fetch(fileset.wasmLoaderPath);
  if (!res.ok) throw new Error(`runtime download failed (${res.status})`);
  scripts.set(String(fileset.wasmLoaderPath), await res.text());
  timings.runtime = Math.round(performance.now() - t0);
  for (const name of names) {
    const t = performance.now();
    delegates[name] = prefer === 'CPU' ? 'CPU' : 'GPU';
    try {
      tasks[name] = await create(name, delegates[name]);
    } catch (err) {
      if (delegates[name] === 'CPU') throw err;
      self.postMessage({ type: 'warn', message: `${name}: GPU delegate unavailable in the worker, using CPU (${err?.message || err})` });
      tasks[name] = await create(name, 'CPU');
      delegates[name] = 'CPU';
    }
    errors[name] = 0;
    timings[name] = Math.round(performance.now() - t);
    // The first call compiles shaders and sizes buffers (seconds on some GPUs). Do it now, on a
    // blank frame, while the player still sees "Loading…", not on the first camera frame.
    const w0 = performance.now();
    warmUp(name);
    timings[`${name}WarmUp`] = Math.round(performance.now() - w0);
  }
  timings.total = Math.round(performance.now() - t0);
  return timings;
}

function warmUp(name) {
  const canvas = new OffscreenCanvas(320, 240);
  const g = canvas.getContext('2d');
  g.fillStyle = '#808080';
  g.fillRect(0, 0, 320, 240);
  try {
    if (name === 'gesture') tasks.gesture.detectForVideo(canvas, 1);
    else tasks.face.detectForVideo(canvas, 1);
    lastTs[name] = 1;
  } catch (err) {
    failed(name, err);
  }
}

// Rebuild a GPU task on the CPU: when it fails on a frame (a GPU delegate can build fine and still
// fail on the first frame), or when the main thread sees the game drop frames (GPU guard, engine.js).
function toCpu(name, why, level = 'warn') {
  if (delegates[name] !== 'GPU' || rebuilding[name] || !tasks[name]) return;
  rebuilding[name] = true;
  self.postMessage({ type: level, message: `${name}: moving to the CPU (${why})` });
  try { tasks[name].close?.(); } catch {}
  tasks[name] = null;
  create(name, 'CPU')
    .then((t) => { tasks[name] = t; delegates[name] = 'CPU'; errors[name] = 0; warmUp(name); })
    .catch((e) => self.postMessage({ type: 'warn', message: `${name} CPU fallback failed: ${e?.message || e}` }))
    .finally(() => { rebuilding[name] = false; });
}

function failed(name, err) {
  errors[name]++;
  if (delegates[name] === 'GPU') toCpu(name, `GPU failed: ${err?.message || err}`);
}

const round = (v) => Math.round(v * 10000) / 10000;

function runGesture(bitmap, ts) {
  const r = tasks.gesture.detectForVideo(bitmap, ts);
  const lms = r?.landmarks || [];
  const hands = lms.map((lm, i) => {
    const c = classifyHand(lm);
    const h = r?.handedness?.[i]?.[0];
    return {
      landmarks: lm.map((p) => ({ x: round(p.x), y: round(p.y), z: round(p.z) })),
      name: c.name,
      score: c.score,
      handedness: h?.categoryName || null,
      handednessScore: h?.score || 0,
    };
  });
  const first = hands[0] || null;
  return {
    hands,
    hand: first ? first.landmarks : null,
    name: first ? first.name : 'None',
    score: first ? first.score : 0,
    handedness: first ? first.handedness : null,
  };
}

function runFace(bitmap, ts) {
  const r = tasks.face.detectForVideo(bitmap, ts);
  const cats = r?.faceBlendshapes?.[0]?.categories;
  if (!cats) return { blendshapes: null, box: null };
  const blendshapes = {};
  for (const c of cats) blendshapes[c.categoryName] = round(c.score);
  let box = null;
  const lm = r.faceLandmarks?.[0];
  if (lm?.length) {
    let x0 = 1, y0 = 1, x1 = 0, y1 = 0;
    for (const p of lm) { if (p.x < x0) x0 = p.x; if (p.x > x1) x1 = p.x; if (p.y < y0) y0 = p.y; if (p.y > y1) y1 = p.y; }
    box = { x0: round(x0), y0: round(y0), x1: round(x1), y1: round(y1) };
  }
  return { blendshapes, box };
}

function frame({ id, ts, bitmap, run }) {
  const out = { type: 'result', id, ts, ms: {}, delegates: { ...delegates } };
  try {
    for (const name of run) {
      if (!tasks[name] || rebuilding[name]) continue;
      // VIDEO mode needs strictly increasing timestamps per task.
      const t = Math.max(Math.round(ts), (lastTs[name] ?? -1) + 1);
      lastTs[name] = t;
      const t0 = performance.now();
      try {
        out[name] = name === 'gesture' ? runGesture(bitmap, t) : runFace(bitmap, t);
        errors[name] = 0;
      } catch (err) {
        failed(name, err);
        out.errors = { ...out.errors, [name]: String(err?.message || err) };
      }
      out.ms[name] = Math.round((performance.now() - t0) * 10) / 10;
    }
  } finally {
    bitmap?.close?.();
  }
  self.postMessage(out);
}

self.onmessage = async (e) => {
  const m = e.data || {};
  if (m.type === 'frame') frame(m);
  else if (m.type === 'init') {
    try {
      if (typeof OffscreenCanvas === 'undefined') throw new Error('OffscreenCanvas is not available in workers here');
      const timings = await init(m.tasks || ['gesture', 'face'], m.delegate);
      self.postMessage({ type: 'ready', delegates: { ...delegates }, timings });
    } catch (err) {
      self.postMessage({ type: 'error', stage: 'init', message: String(err?.message || err) });
    }
  } else if (m.type === 'cpu') {
    for (const name of Object.keys(tasks)) toCpu(name, m.reason || 'requested', 'info');
  } else if (m.type === 'close') {
    for (const t of Object.values(tasks)) try { t?.close?.(); } catch {}
    self.close();
  }
};
