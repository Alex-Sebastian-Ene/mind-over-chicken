// camera (#33): loads Google's MediaPipe Tasks (Gesture Recognizer + Face Landmarker) on demand.
//
// Only downloads happen here: the runtime from jsDelivr (pinned version) and two model files
// from Google's model bucket. Inference runs in this tab (WebGL, or WebAssembly on the CPU);
// no camera frame, landmark or score is ever sent anywhere.

const VERSION = '0.10.14';
const CDN = `https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@${VERSION}`;
export const VISION_URLS = {
  bundle: `${CDN}/vision_bundle.mjs`,
  wasm: `${CDN}/wasm`,
  gesture: 'https://storage.googleapis.com/mediapipe-models/gesture_recognizer/gesture_recognizer/float16/1/gesture_recognizer.task',
  face: 'https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task',
};

const OPTIONS = {
  gesture: { baseOptions: { modelAssetPath: VISION_URLS.gesture }, runningMode: 'VIDEO', numHands: 1 },
  face: { baseOptions: { modelAssetPath: VISION_URLS.face }, runningMode: 'VIDEO', numFaces: 1, outputFaceBlendshapes: true },
};

let pending = null;

// Resolves to { gesture, face, delegates: {gesture, face}, timings: {runtime, gesture, face, total} (ms), rebuildOnCpu(name) }.
// Cached: a second call (camera toggled off and on) reuses the loaded models.
export function loadVision(onStatus = () => {}, { hands = true } = {}) {
  pending ??= load(onStatus, hands).catch((err) => { pending = null; throw err; });
  return pending;
}

// MediaPipe's WebAssembly prints TensorFlow Lite INFO lines on stderr, which the browser shows as
// console errors ("INFO: Created TensorFlow Lite XNNPACK delegate for CPU."). Re-route only lines
// starting with "INFO: " to console.info; every real error still goes to console.error.
function quietInfoLogs() {
  if (console.error.camQuiet) return;
  const error = console.error;
  const quiet = function (...args) {
    if (typeof args[0] === 'string' && args[0].startsWith('INFO: ')) return console.info(...args);
    return error.apply(this, args);
  };
  quiet.camQuiet = true;
  console.error = quiet;
}

async function load(onStatus, hands) {
  quietInfoLogs();
  const t0 = performance.now();
  const timings = {};
  onStatus('Loading MediaPipe…');
  const mp = await import(VISION_URLS.bundle);
  const fileset = await mp.FilesetResolver.forVisionTasks(VISION_URLS.wasm);
  timings.runtime = Math.round(performance.now() - t0);

  // One after the other: the tasks share one WebAssembly loader.
  let gesture = null;
  if (hands) {
    onStatus('Loading the hand model…');
    let t = performance.now();
    gesture = await create(mp.GestureRecognizer, fileset, OPTIONS.gesture);
    timings.gesture = Math.round(performance.now() - t);
  } else {
    timings.gesture = 0;
  }

  onStatus('Loading the face model…');
  let t = performance.now();
  const face = await create(mp.FaceLandmarker, fileset, OPTIONS.face);
  timings.face = Math.round(performance.now() - t);
  timings.total = Math.round(performance.now() - t0);

  const vision = {
    gesture: gesture?.task || null, face: face.task,
    delegates: { gesture: gesture?.delegate || null, face: face.delegate },
    timings,
    // A GPU delegate can build fine and still fail on the first frame (lost context, blocked
    // WebGL). The camera loop calls this once per task to fall back to the CPU.
    async rebuildOnCpu(name) {
      try { vision[name]?.close?.(); } catch {}
      if (!vision[name]) return null;
      const Task = name === 'gesture' ? mp.GestureRecognizer : mp.FaceLandmarker;
      vision[name] = await Task.createFromOptions(fileset, withDelegate(OPTIONS[name], 'CPU'));
      vision.delegates[name] = 'CPU';
      return vision[name];
    },
  };
  return vision;
}

function withDelegate(options, delegate) {
  return { ...options, baseOptions: { ...options.baseOptions, delegate } };
}

// GPU delegate first, CPU if the GPU path can't be built here.
async function create(Task, fileset, options) {
  try {
    return { task: await Task.createFromOptions(fileset, withDelegate(options, 'GPU')), delegate: 'GPU' };
  } catch (err) {
    console.warn('[camera] GPU delegate unavailable, using CPU:', err?.message || err);
    return { task: await Task.createFromOptions(fileset, withDelegate(options, 'CPU')), delegate: 'CPU' };
  }
}
