// camera (faster tracking, Alex): the worker scheduler feeds hands and face as fast as the camera and
// the models allow, up to MAX_HZ. Simulated: a fake camera presenting frames (requestVideoFrameCallback),
// fake workers that answer after a fixed inference time, and the game loop pumping at 60 fps. No
// MediaPipe, no webcam.
// Run: node --test tests/camera_rates.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import { WorkerEngine, MAX_HZ } from '../src/camera/engine.js';
import { PINCH_MOVE_TAU, lowPass } from '../src/camera/pinch-move.js';

// Run the scheduler for `seconds` of simulated time; return { gesture, face, video } in Hz plus the
// mean lag per channel: from the camera presenting the frame to its result reaching the game.
export async function simulate(Engine, { cameraFps = 30, inferMs = { gesture: 15, face: 20 }, seconds = 3, gameFps = 60 } = {}) {
  let clock = 0;
  const realNow = performance.now;
  performance.now = () => clock;
  const realBitmap = globalThis.createImageBitmap;
  globalThis.createImageBitmap = async () => ({ close() {} });
  const events = [];   // { at, fn }
  const at = (t, fn) => events.push({ at: t, fn });
  const counts = { gesture: 0, face: 0 }, lag = { gesture: 0, face: 0 };
  const warm = 500;
  let presentedAt = 0;   // when the camera presented its newest frame
  try {
    const eng = new Engine(true, true, { gesture: 'GPU', face: 'CPU' });
    eng.on({
      gesture: () => { if (clock >= warm) counts.gesture++; },
      face: () => { if (clock >= warm) counts.face++; },
    });
    for (const lane of eng.lanes) {
      lane.ready = true;
      lane.worker = {
        postMessage(m) {
          if (m.type !== 'frame') return;
          const ms = Math.max(...m.run.map((t) => inferMs[t]));
          const shot = presentedAt;
          at(clock + ms, () => {
            if (clock >= warm) for (const t of m.run) lag[t] += clock - shot;
            eng.onResult(lane, {
            type: 'result', id: m.id, ts: m.ts, delegates: {},
            ms: Object.fromEntries(m.run.map((t) => [t, inferMs[t]])),
            ...(m.run.includes('gesture') ? { gesture: { hands: [], hand: null, name: 'None', score: 0 } } : {}),
            ...(m.run.includes('face') ? { face: { blendshapes: null, box: null } } : {}),
            });
          });
        },
        terminate() {},
      };
    }
    let rvfc = null;
    const video = { videoWidth: 640, videoHeight: 480, currentTime: 0, requestVideoFrameCallback(cb) { rvfc = cb; } };
    for (let t = 0; t < seconds * 1000 + warm; t += 1000 / cameraFps) at(t, () => { presentedAt = clock; video.currentTime = t / 1000; const cb = rvfc; rvfc = null; cb?.(); });
    for (let t = 5; t < seconds * 1000 + warm; t += 1000 / gameFps) at(t, () => eng.pump(video, clock, { hands: true }));
    while (events.length) {
      events.sort((a, b) => a.at - b.at);
      const e = events.shift();
      clock = e.at;
      e.fn();
      for (let i = 0; i < 4; i++) await Promise.resolve();   // let createImageBitmap resolve and post
    }
    const hz = (n) => Math.round((n / seconds) * 10) / 10;
    const snap = eng.snapshot(clock);
    return {
      gesture: hz(counts.gesture), face: hz(counts.face), video: snap.video?.hz ?? null,
      lag: { gesture: Math.round(lag.gesture / Math.max(1, counts.gesture)), face: Math.round(lag.face / Math.max(1, counts.face)) },
    };
  } finally {
    performance.now = realNow;
    globalThis.createImageBitmap = realBitmap;
  }
}

test('a 30 fps camera: hands and face both run on every camera frame', async () => {
  const r = await simulate(WorkerEngine, { cameraFps: 30, inferMs: { gesture: 15, face: 20 } });
  assert.ok(r.gesture >= 29, `hands ${r.gesture} Hz`);
  assert.ok(r.face >= 29, `face ${r.face} Hz`);
  assert.ok(r.video >= 29 && r.video <= 31, `camera ${r.video} fps`);
  // Each frame is grabbed when the camera presents it, so the lag is about one inference.
  assert.ok(r.lag.gesture <= 16, `hand lag ${r.lag.gesture} ms`);
  assert.ok(r.lag.face <= 21, `face lag ${r.lag.face} ms`);
});

test('a 60 fps camera: hands follow it, the face stays at its 30 Hz cap', async () => {
  const r = await simulate(WorkerEngine, { cameraFps: 60, inferMs: { gesture: 12, face: 20 } });
  assert.ok(r.gesture >= 55 && r.gesture <= MAX_HZ.gesture + 1, `hands ${r.gesture} Hz`);
  assert.ok(r.face >= 25 && r.face <= MAX_HZ.face + 1, `face ${r.face} Hz`);
});

test('a slow model is limited by its own time, never queues frames', async () => {
  const r = await simulate(WorkerEngine, { cameraFps: 30, inferMs: { gesture: 45, face: 70 } });
  // One frame in flight per worker: at most one result per inference time, and close to it.
  assert.ok(r.gesture <= 1000 / 45 + 0.5 && r.gesture >= 14, `hands ${r.gesture} Hz`);
  assert.ok(r.face <= 1000 / 70 + 0.5 && r.face >= 9, `face ${r.face} Hz`);
});

test('pinch-move smoothing settles faster than before (tau 70 ms → 45 ms)', () => {
  assert.ok(PINCH_MOVE_TAU <= 0.05);
  // 90% of a step after 100 ms of 30 Hz frames.
  let v = 0;
  for (let i = 0; i < 3; i++) v = lowPass(v, 1, 1 / 30);
  assert.ok(v >= 0.85, `step response ${v.toFixed(2)} after 100 ms`);
});
