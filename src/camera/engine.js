// camera debug (#67): where the camera models run.
//
//   worker (default)  MediaPipe runs in module Web Workers (vision.worker.js), one per model so hands
//                     and face run in parallel. The main thread only grabs a small frame
//                     (createImageBitmap with resize, ~320 px wide) and transfers it; at most one
//                     frame per worker is in flight, so nothing queues up and the game loop never
//                     waits for inference.
//   main (fallback)   the #59 path: the models run in this thread under a budget (at most one model
//                     per game frame, ~8–15 Hz hands, ≤5 Hz face). Used when workers, module workers,
//                     OffscreenCanvas or WebGL in a worker are unavailable, or a worker dies.
//
// Both report the same results to the handlers set with on():
//   gesture({ hand, name, score, aspect, at, latency, ms })   hand: 21 landmarks or null
//   face({ blendshapes, box, at, latency, ms })               blendshapes: null when no face
//   fail(err)    the worker path broke while running; the caller switches to the main thread
//   warn(text)   a model keeps failing
// latency is from grabbing the frame to having the result on the main thread; ms is the model call.
//
// ?camworker=0 forces the main-thread path, ?camworker=1 runs both models in one worker,
// ?camworker=2 (default) one worker per model.

import { loadVision } from './vision.js';
import { RateMeter, Ema } from './meter.js';

export const FRAME_WIDTH = { gesture: 320, face: 256 };
export const MAX_HZ = { gesture: 30, face: 24 };
// With one worker per model, the hands get the GPU and the face runs on the CPU (WebAssembly,
// XNNPACK). Every WebGL context of the page, the game's included, runs in the browser's one GPU
// process, so GPU models queue behind each other and behind the game. Measured on an i5-10300H
// laptop: GPU+GPU ~16 Hz each, GPU hands + CPU face ~22 Hz each. A GPU model that fails moves to the
// CPU, and so does every GPU model when the game drops frames (camera.js, GPU guard).
export const SPLIT_DELEGATES = { gesture: 'GPU', face: 'CPU' };

// ?camgpu=0 runs both worker models on the CPU, ?camgpu=2 both on the GPU; default SPLIT_DELEGATES.
export function workerDelegates(search = globalThis.location?.search || '') {
  const v = new URLSearchParams(search).get('camgpu');
  if (v === '0') return { gesture: 'CPU', face: 'CPU' };
  if (v === '2') return { gesture: 'GPU', face: 'GPU' };
  return { ...SPLIT_DELEGATES };
}
// Main-thread budget (#59): each model waits a multiple of its own measured cost.
const HAND_MS = { tracking: 66, searching: 125, costX: 3 };
const FACE_MS = { min: 200, costX: 6 };
const INIT_TIMEOUT_MS = 60000;
const STALL_MS = { first: 20000, later: 5000 };   // no answer to a frame for this long: the worker is stuck
const MAX_ERRORS = 30;

export function workerPreference(search = globalThis.location?.search || '') {
  const v = new URLSearchParams(search).get('camworker');
  return v === '0' || v === '1' || v === '2' ? Number(v) : 2;
}

// Why the worker path can't run here, or null when it can.
export function workerSupport(g = globalThis) {
  if (typeof g.Worker !== 'function') return 'no Web Workers';
  if (typeof g.createImageBitmap !== 'function') return 'no createImageBitmap';
  if (typeof g.OffscreenCanvas !== 'function') return 'no OffscreenCanvas';
  return null;
}

const newStat = () => ({ rate: new RateMeter(), ms: new Ema(0.2), latency: new Ema(0.2) });

class Engine {
  constructor(hands) {
    this.hands = hands;
    this.handlers = {};
    this.stats = { gesture: newStat(), face: newStat() };
    this.delegates = { gesture: null, face: null };
    this.timings = {};
    this.alive = true;
    this.note = '';
  }

  on(handlers) { this.handlers = handlers || {}; }

  record(name, at, ms, latency) {
    const s = this.stats[name];
    s.rate.tick(at);
    s.ms.add(ms);
    s.latency.add(latency);
  }

  resetStats() { for (const s of Object.values(this.stats)) { s.rate.reset(); s.ms.reset(); s.latency.reset(); } }

  // { mode, note, gesture: { hz, ms, latency, delegate }, face: {...}, timings }
  snapshot(now = performance.now()) {
    const one = (name) => {
      const s = this.stats[name];
      const r = (v) => (v == null ? null : Math.round(v * 10) / 10);
      return { hz: r(s.rate.hz(now)), ms: r(s.ms.value), latency: r(s.latency.value), delegate: this.delegates[name], count: s.rate.count };
    };
    return { mode: this.mode, workers: this.workers || 0, note: this.note, gesture: one('gesture'), face: one('face'), timings: { ...this.timings } };
  }
}

// ---------- worker path ----------
class WorkerEngine extends Engine {
  constructor(hands, split, delegates = workerDelegates()) {
    super(hands);
    this.mode = 'worker';
    const groups = !hands ? [['face']] : split ? [['gesture'], ['face']] : [['gesture', 'face']];
    this.lanes = groups.map((tasks) => ({
      tasks, delegate: tasks.length === 1 ? delegates[tasks[0]] : delegates.gesture,
      worker: null, ready: false, busy: false, sentAt: 0, seq: 0, aspect: 4 / 3, errors: 0,
    }));
    this.workers = this.lanes.length;
    this.lastRun = { gesture: -Infinity, face: -Infinity };
    this.seen = { gesture: null, face: null };
    this.frameSerial = 0;
    this.watched = null;
    this.captureErrors = 0;
  }

  async init(onStatus) {
    onStatus('Loading the camera models in a background worker…');
    const t0 = performance.now();
    await Promise.all(this.lanes.map((lane) => this.startLane(lane)));
    this.timings.total = Math.round(performance.now() - t0);
  }

  startLane(lane) {
    return new Promise((resolve, reject) => {
      let w;
      try {
        w = new Worker(new URL('./vision.worker.js', import.meta.url), { type: 'module', name: `camera-${lane.tasks.join('-')}` });
      } catch (err) { reject(err); return; }
      lane.worker = w;
      const timer = setTimeout(() => reject(new Error('worker start timed out')), INIT_TIMEOUT_MS);
      w.onmessage = (e) => {
        const m = e.data || {};
        if (m.type === 'result') this.onResult(lane, m);
        else if (m.type === 'ready') {
          clearTimeout(timer);
          lane.ready = true;
          Object.assign(this.delegates, m.delegates);
          for (const t of lane.tasks) this.timings[t] = m.timings?.[t];
          this.timings.runtime = Math.max(this.timings.runtime || 0, m.timings?.runtime || 0);
          resolve();
        } else if (m.type === 'error') { clearTimeout(timer); reject(new Error(m.message)); }
        else if (m.type === 'warn') console.warn('[camera worker]', m.message);
        else if (m.type === 'info') console.info('[camera worker]', m.message);
      };
      w.onerror = (e) => {
        e.preventDefault?.();
        clearTimeout(timer);
        const err = new Error(e.message || 'the worker failed to start');
        if (lane.ready) this.fail(err); else reject(err);
      };
      w.onmessageerror = () => this.fail(new Error('worker message could not be decoded'));
      w.postMessage({ type: 'init', tasks: lane.tasks, delegate: lane.delegate });
    });
  }

  // Count presented video frames exactly where the browser can tell us (requestVideoFrameCallback).
  // It pauses while the page isn't being rendered (e.g. a hidden embedded view), so then fall back
  // to the media clock.
  watch(video) {
    if (this.watched === video || typeof video.requestVideoFrameCallback !== 'function') return;
    this.watched = video;
    this.frameAt = -Infinity;
    const onFrame = () => {
      if (!this.alive || this.watched !== video) return;
      this.frameSerial++;
      this.frameAt = performance.now();
      video.requestVideoFrameCallback(onFrame);
    };
    video.requestVideoFrameCallback(onFrame);
  }

  frameKey(video) {
    return this.watched === video && performance.now() - this.frameAt < 250 ? `f${this.frameSerial}` : `t${video.currentTime}`;
  }

  // Called every game frame. Never waits: it only starts an async capture for idle workers.
  pump(video, now, { hands = this.hands } = {}) {
    if (!this.alive) return;
    this.watch(video);
    const key = this.frameKey(video);
    for (const lane of this.lanes) {
      if (!lane.ready) continue;
      if (lane.busy) {
        if (now - lane.sentAt > (lane.answered ? STALL_MS.later : STALL_MS.first)) this.fail(new Error('the camera worker stopped answering'));
        continue;
      }
      // A few ms of slack so a 30 fps camera isn't aliased down to 15 Hz by a 30 Hz cap.
      const run = lane.tasks.filter((t) => (t === 'face' || hands) && this.seen[t] !== key && now - this.lastRun[t] >= 1000 / MAX_HZ[t] - 6);
      if (!run.length) continue;
      const vw = video.videoWidth, vh = video.videoHeight;
      if (!vw || !vh) continue;
      lane.busy = true;
      lane.sentAt = now;
      lane.aspect = vw / vh;
      for (const t of run) { this.seen[t] = key; this.lastRun[t] = now; }
      const w = Math.min(vw, Math.max(...run.map((t) => FRAME_WIDTH[t])));
      const h = Math.round((w * vh) / vw);
      createImageBitmap(video, { resizeWidth: w, resizeHeight: h, resizeQuality: 'low' }).then((bitmap) => {
        if (!this.alive || !lane.worker) { bitmap.close(); lane.busy = false; return; }
        this.captureErrors = 0;
        lane.worker.postMessage({ type: 'frame', id: ++lane.seq, ts: now, bitmap, run }, [bitmap]);
      }, (err) => {
        lane.busy = false;
        if (++this.captureErrors > MAX_ERRORS) this.fail(err);
      });
    }
  }

  onResult(lane, m) {
    lane.busy = false;
    lane.answered = true;
    if (!this.alive) return;
    const at = performance.now();
    const latency = at - m.ts;
    Object.assign(this.delegates, m.delegates);
    if (m.errors) {
      if (++lane.errors === MAX_ERRORS) this.handlers.warn?.(`${lane.tasks.includes('gesture') ? 'Hand' : 'Face'} tracking is struggling on this device.`);
    } else lane.errors = 0;
    if (m.gesture) {
      this.record('gesture', at, m.ms.gesture, latency);
      this.handlers.gesture?.({ ...m.gesture, aspect: lane.aspect, at, latency, ms: m.ms.gesture });
    }
    if (m.face) {
      this.record('face', at, m.ms.face, latency);
      this.handlers.face?.({ ...m.face, at, latency, ms: m.ms.face });
    }
  }

  usesGpu() { return this.lanes.some((l) => l.ready) && Object.values(this.delegates).includes('GPU'); }

  // GPU guard (camera.js): move every GPU model to the CPU, e.g. when the game drops frames.
  moveToCpu(reason) {
    this.note = `models moved to the CPU: ${reason}`;
    console.info(`[camera] ${this.note}`);
    for (const lane of this.lanes) lane.worker?.postMessage({ type: 'cpu', reason });
  }

  fail(err) {
    if (!this.alive) return;
    this.close();
    this.handlers.fail?.(err);
  }

  close() {
    this.alive = false;
    for (const lane of this.lanes) {
      try { lane.worker?.terminate(); } catch {}
      lane.worker = null;
      lane.ready = false;
    }
  }
}

// ---------- main-thread fallback (#59) ----------
class MainEngine extends Engine {
  constructor(hands) {
    super(hands);
    this.mode = 'main';
    this.vision = null;
    this.seen = { gesture: -1, face: -1 };
    this.nextAt = { gesture: 0, face: 0 };
    this.cost = { gesture: 20, face: 15 };
    this.rebuilding = { gesture: false, face: false };
    this.errors = { gesture: 0, face: 0 };
  }

  async init(onStatus) {
    this.vision = await loadVision(onStatus, { hands: this.hands });
    this.delegates = this.vision.delegates;
    this.timings = this.vision.timings;
  }

  failed(name, err) {
    const vision = this.vision;
    this.errors[name]++;
    if (vision.delegates[name] === 'GPU' && !this.rebuilding[name]) {
      this.rebuilding[name] = true;
      console.warn(`[camera] ${name} model failed on the GPU, switching to CPU:`, err?.message || err);
      vision.rebuildOnCpu(name).catch((e) => console.warn(`[camera] ${name} CPU fallback failed:`, e?.message || e)).finally(() => { this.rebuilding[name] = false; });
    } else if (this.errors[name] === MAX_ERRORS) {
      console.warn(`[camera] ${name} model keeps failing:`, err?.message || err);
      this.handlers.warn?.(`${name === 'gesture' ? 'Hand' : 'Face'} tracking is struggling on this device.`);
    }
  }

  timed(name, fn) {
    const t0 = performance.now();
    let res = null;
    try { res = fn(); } catch (err) { this.failed(name, err); }
    const ms = performance.now() - t0;
    this.cost[name] += (ms - this.cost[name]) * 0.3;
    return { res, ms };
  }

  // At most one model per call, each budgeted against its own cost.
  pump(video, now, { hands = this.hands, tracking = false } = {}) {
    const v = this.vision;
    if (!v) return;
    const t = video.currentTime;
    const aspect = (video.videoWidth || 4) / (video.videoHeight || 3);
    if (hands && v.gesture && now >= this.nextAt.gesture && t !== this.seen.gesture && !this.rebuilding.gesture) {
      this.seen.gesture = t;
      const { res, ms } = this.timed('gesture', () => v.gesture.recognizeForVideo(video, now));
      this.nextAt.gesture = now + Math.max(tracking ? HAND_MS.tracking : HAND_MS.searching, this.cost.gesture * HAND_MS.costX);
      const at = performance.now();
      this.record('gesture', at, ms, at - now);
      const lms = res?.landmarks || [];
      const handsList = lms.map((lm, i) => {
        const g = res?.gestures?.[i]?.[0];
        const h = res?.handedness?.[i]?.[0];
        return {
          landmarks: lm,
          name: g?.categoryName || 'None',
          score: g?.score || 0,
          handedness: h?.categoryName || null,
          handednessScore: h?.score || 0,
        };
      });
      const top = res?.gestures?.[0]?.[0];
      this.handlers.gesture?.({ hands: handsList, hand: res?.landmarks?.[0] || null, name: top?.categoryName || 'None', score: top?.score || 0, aspect, at, latency: at - now, ms });
    } else if (v.face && now >= this.nextAt.face && t !== this.seen.face && !this.rebuilding.face) {
      this.seen.face = t;
      const { res, ms } = this.timed('face', () => v.face.detectForVideo(video, now));
      this.nextAt.face = now + Math.max(FACE_MS.min, this.cost.face * FACE_MS.costX);
      const at = performance.now();
      this.record('face', at, ms, at - now);
      const lm = res?.faceLandmarks?.[0];
      let box = null;
      if (lm?.length) {
        const xs = lm.map((p) => p.x), ys = lm.map((p) => p.y);
        box = { x0: Math.min(...xs), y0: Math.min(...ys), x1: Math.max(...xs), y1: Math.max(...ys) };
      }
      this.handlers.face?.({ blendshapes: res?.faceBlendshapes?.[0]?.categories || null, box, at, latency: at - now, ms });
    }
  }

  close() { this.alive = false; }   // the loaded models stay cached in vision.js
}

// Worker path if it can start here, else the main thread. `prefer`: 0 main, 1 one worker, 2 two.
export async function loadEngine({ hands = true, prefer = workerPreference(), onStatus = () => {} } = {}) {
  let note = prefer === 0 ? 'main thread requested (?camworker=0)' : '';
  if (prefer > 0) {
    const why = workerSupport();
    if (!why) {
      const eng = new WorkerEngine(hands, prefer === 2);
      try {
        await eng.init(onStatus);
        return eng;
      } catch (err) {
        eng.close();
        note = `worker unavailable: ${err?.message || err}`;
        console.warn('[camera] worker inference unavailable, using the main thread:', err?.message || err);
      }
    } else {
      note = `worker unavailable: ${why}`;
      console.warn(`[camera] worker inference unavailable (${why}), using the main thread`);
    }
  }
  const eng = new MainEngine(hands);
  eng.note = note;
  await eng.init(onStatus);
  return eng;
}
