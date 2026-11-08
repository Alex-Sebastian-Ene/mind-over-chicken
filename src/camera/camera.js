// camera (#33, #67, #116–#123, #261): opt-in webcam input.
//   Aim:   the webcam does NOT track the head (#189). G.brain.aim comes from the EEG headset's head
//          pose while it streams, else the mouse (aim-source.js). Hands never overwrite it (#116, #120).
//   Hands: MediaPipe Hand Landmarker (numHands=2, #118, #327) + landmark classifier → movement:
//          1. One-hand pinch grabs & relocates living entities under the cursor (#120)
//          2. Two-hand pinch distance continuously zooms in/out & freezes on release (#119)
//          3. One deliberate cast (#291): fist → pull back → thrust + open palm = charged fireball.
//          4. Rain spell (#330, #340): open palm held in camera + Insight/Focus for two seconds.
//   Face:  MediaPipe Face Landmarker blendshapes → Joy (continuous) + Surprise (debounced event) (#117).
//   Coach: Collapsible bottom panel with procedural 21-landmark ghost hands (#123).
//
// PRIVACY: the camera starts only when the player clicks "Start Camera" in the sidebar (#183):
// the toolbar Camera button and the ` key only open the sidebar. Frames go from the <video>
// element to MediaPipe inside this tab (or its own workers). Nothing is recorded, stored or
// uploaded: no frame, landmark or score leaves the device. Turning the camera off stops every
// track. The only network traffic is the one-time download of the runtime and the two models.
//
// The player stays in control: hands never change the brain's calm or focus and never block
// another input. Face emotion does count toward brain state and game reactions.

import { clamp } from '../sim/util.js';
import { GESTURE_LABEL } from './gestures.js';
import { PinchMoveSmoother } from './pinch-move.js';
import { initMovementCalibration, loadPinchCalibration, savePinchCalibration } from './movement-calibration.js';
import { FireballController } from './fireball.js';
import { formation, drawFireballSprite, drawPixels, drawSegmentRing, ringSegments, FB_HOT } from '../render/fireball-sprite.js';
import { EMOTIONS, EmotionTracker, neutralMood, blendshapeMap } from './emotion.js';
import { manualMood, surpriseRisingEdge } from './manual-mood.js';
import { surpriseReady, sceneKind, joyCause, surpriseCause } from './mood-actions.js';   // #152
import { loadVision } from './vision.js';
import { loadEngine, workerPreference } from './engine.js';
import { handsEnabled, setHandsEnabled } from './hands-pref.js';
import { FingerController, MultiHandTracker, pinchMetrics } from './fingers.js';
import { initCoach } from './coach.js';
import { FrameMeter } from './meter.js';
import { drawHand } from './draw.js';
import { aimSource } from './aim-source.js';

const ADVISOR_GAP = 14;      // seconds between the advisor's mood remarks
const GPU_GUARD = { ratio: 0.85, minFps: 30, graceMs: 4000, everyMs: 1000 };

const MOOD_LINES = {
  joy: { mood: 'happy', lines: ['You are smiling, my lord. The people have started dancing. Badly, but with feeling.', 'A divine smile! Everyone down here feels lucky today.'] },
  surprise: { mood: 'panic', lines: ['You look startled! The sky took it personally.', 'Such a face! The heavens flashed in sympathy.'] },
};

const TEMPLATE = `
  <header class="eeg-head">
    <h2>Camera &amp; Hand Moves</h2>
    <button type="button" class="tool" data-cam-close title="Close Camera sidebar (C)">Close <kbd>C</kbd></button>
  </header>

  <section class="eeg-sec">
    <div class="eeg-status-row">
      <span class="eeg-dot" data-cam-dot data-ok="0"></span>
      <strong data-cam-state>Camera off</strong>
      <span class="eeg-qual" data-cam-mode>Idle</span>
    </div>
    <p class="eeg-msg" data-cam-msg role="status">Opt-in webcam for your face: your expression sets the mood. Hand moves are off unless you tick them below (pinch to grab a creature, move your hand to pan, two hands to pinch-zoom). The webcam does not track your head: the cursor follows the EEG headset's head pose, or the mouse. Frames never leave this device.</p>
    <div class="eeg-url-row">
      <button type="button" class="btn btn-primary btn-small" data-cam-start>Start Camera</button>
      <button type="button" class="btn btn-small" data-cam-stop disabled>Stop &amp; Clear</button>
    </div>
    <label class="cam-hands-toggle"><input type="checkbox" data-cam-hands> Hand moves (pinch to grab, two hands to zoom). Off by default: the face sets the mood either way.</label>
    <p class="cam-privacy" data-cam-optin>The webcam stays off until you click <b>Start Camera</b>; only then does the browser ask for permission. Frames never leave this device.</p>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Live Vision &amp; Two-Hand Tracking</h3>
      <span class="eeg-qual" data-cam-fps>-- fps</span>
    </div>
    <div class="cam-body">
      <div class="cam-view">
        <video class="cam-video" playsinline muted></video>
        <canvas class="cam-hands" width="320" height="240" aria-hidden="true"></canvas>
      </div>
      <div class="eeg-cursor-controls">
        <div class="eeg-imu-xyz" style="grid-template-columns:1fr">
          <div><span>Camera Frames</span><b data-cam-rate-video>--</b></div>
          <div><span>Hands Inference</span><b data-cam-rate-hands>--</b></div>
          <div><span>Face Inference</span><b data-cam-rate-face>--</b></div>
        </div>
        <div class="eeg-imu-mini">
          <span>Cursor: <b data-cam-aim>mouse</b></span>
        </div>
        <div class="eeg-imu-mini">
          <span>Hands: <b data-cam-gesture>--</b></span>
        </div>
        <div class="eeg-imu-mini">
          <span>Pinch / Zoom: <b data-cam-pinch>--</b></span>
        </div>
        <div class="eeg-url-row">
          <button type="button" class="btn btn-primary btn-small" data-calib-open>🎯 Pinch Movement Calibration</button>
        </div>
      </div>
    </div>
  </section>

  <section class="eeg-sec" aria-label="Charged fireball">
    <div class="eeg-sec-head">
      <h3>Charged Fireball</h3>
      <span class="eeg-qual" data-cam-fb-state>idle</span>
    </div>
    <div class="cam-fireball">
      <canvas class="cam-fb-orb" data-cam-fb-orb width="24" height="24" aria-hidden="true"></canvas>
      <span class="cam-fb-meter" data-cam-fb-meter><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i></span>
      <b data-cam-fb-pct>0%</b>
    </div>
    <p class="eeg-msg">Hold a fist, pull it back, then thrust forward and open your palm. Or hold <kbd>3</kbd> and release.</p>
    <p class="eeg-msg">Rain spell: hold an open palm to the camera and hold Insight/Focus for two seconds. No camera, no Rain.</p>
  </section>

  <section class="eeg-sec" aria-label="Mood read from your face">
    <div class="eeg-sec-head">
      <h3>Facial Mood (Joy &amp; Surprise)</h3>
      <span class="eeg-qual">Dominant: <b data-cam-dominant>neutral</b></span>
    </div>
    <div class="eeg-bands">
      ${EMOTIONS.map((k) => `
        <div class="eeg-band-row cam-mood-row" data-emotion="${k}">
          <span class="eeg-band-lbl" style="text-transform:capitalize">${k}</span>
          <span class="eeg-bar cam-bar"><span></span></span>
          <span class="eeg-band-val" data-emotion-val="${k}">0%</span>
        </div>`).join('')}
    </div>
    <div class="cam-manual-mood" aria-label="Manual mood (no camera needed, #155)">
      <label class="cam-manual-row"><input type="checkbox" data-cam-mood-off checked> No emotion</label>
      <label class="cam-manual-row">Happy <input type="range" min="0" max="100" value="0" step="1" data-cam-mood-slider aria-label="Manual mood: Happy to Mix to Surprised" disabled> Surprised</label>
      <span class="eeg-qual">Manual mood: <b data-cam-mood-manual>off</b> (overrides the camera while on)</span>
    </div>
    <p class="cam-privacy">Frames never leave this device. Nothing is uploaded.</p>
  </section>`;

function cameraError(err) {
  const name = err?.name || '';
  if (name === 'NotAllowedError' || name === 'PermissionDeniedError' || name === 'SecurityError')
    return 'Camera permission was denied. Allow it in the address bar and click Start Camera again.';
  if (name === 'NotFoundError' || name === 'DevicesNotFoundError' || name === 'OverconstrainedError')
    return 'No camera found.';
  if (name === 'NotReadableError' || name === 'TrackStartError' || name === 'AbortError')
    return 'The camera is busy in another app. Close it and click Start Camera again.';
  return `Camera unavailable (${name || err?.message || 'unknown error'}).`;
}

function stopTracks(stream) { stream?.getTracks().forEach((t) => t.stop()); }

export function initCamera({
  app = null,
  getG = () => window.__moc?.G,
  canCast = () => true,
  mapX = (x) => x,
  screenX = (x) => x,
  screenY = (y) => y,   // the home world is bigger than the screen (#129): aims map back through the camera
  onZoomDelta = null,
  onGrabMove = null,
  getContext = () => 'surface',
  onStatusChange = null,
} = {}) {
  const btn = document.getElementById('camBtn');
  const panel = document.getElementById('camPanel');
  const screen = document.getElementById('screen');
  if (!btn || !panel) return null;
  panel.innerHTML = TEMPLATE;
  const $ = (sel) => panel.querySelector(sel);
  const calibModal = document.createElement('div');
  calibModal.className = 'calib-wiz';
  calibModal.id = 'moveCalibModal';
  calibModal.hidden = true;
  calibModal.setAttribute('role', 'dialog');
  calibModal.setAttribute('aria-modal', 'true');
  calibModal.setAttribute('aria-label', 'Pinch and head calibration');
  calibModal.innerHTML = `
    <div class="calib-wiz-top">
      <div>
        <span class="calib-wiz-step" data-calib-status>idle</span>
        <h2>Pinch Movement Calibration</h2>
        <p data-calib-msg>Reach left, right, up and down with each hand. The wizard advances when you hit a target.</p>
      </div>
      <div class="calib-wiz-actions">
        <button type="button" class="btn btn-small" data-calib-close>Close <kbd>Esc</kbd></button>
      </div>
    </div>
    <div class="calib-wiz-stage"><canvas class="calib-wiz-canvas" width="960" height="500"></canvas></div>
    <div class="calib-wiz-bottom">
      <div class="eeg-url-row">
        <button type="button" class="btn btn-primary btn-small" data-calib-start>Calibrate</button>
        <button type="button" class="btn btn-small" data-calib-skip disabled>Skip</button>
        <button type="button" class="btn btn-small" data-calib-reset>Reset</button>
      </div>
    </div>`;
  if (document.body?.appendChild) document.body.appendChild(calibModal);
  const video = $('.cam-video');
  const hands = $('.cam-hands');
  const handsCtx = hands.getContext('2d');
  const ui = {
    dot: $('[data-cam-dot]'),
    state: $('[data-cam-state]'),
    mode: $('[data-cam-mode]'),
    msg: $('[data-cam-msg]'),
    startBtn: $('[data-cam-start]'),
    stopBtn: $('[data-cam-stop]'),
    fps: $('[data-cam-fps]'),
    rateVideo: $('[data-cam-rate-video]'),
    rateHands: $('[data-cam-rate-hands]'),
    handsToggle: $('[data-cam-hands]'),
    rateFace: $('[data-cam-rate-face]'),
    aim: $('[data-cam-aim]'),
    gesture: $('[data-cam-gesture]'),
    pinch: $('[data-cam-pinch]'),
    range: $('[data-cam-range]'),
    fbState: $('[data-cam-fb-state]'),
    fbOrb: $('[data-cam-fb-orb]'),
    fbMeter: $('[data-cam-fb-meter]'),
    fbPct: $('[data-cam-fb-pct]'),
    dominant: $('[data-cam-dominant]'),
    bars: Object.fromEntries(EMOTIONS.map((k) => [k, $(`[data-emotion="${k}"] .cam-bar > span`)])),
    vals: Object.fromEntries(EMOTIONS.map((k) => [k, $(`[data-emotion-val="${k}"]`)])),
  };

  // Overlay canvas for the grabbed-entity tether.
  const fx = document.createElement('canvas');
  fx.className = 'cam-fx';
  fx.setAttribute('aria-hidden', 'true');
  screen?.insertAdjacentElement('afterend', fx);
  const fxCtx = fx.getContext('2d');
  let fxDirty = false;

  const tracker = new EmotionTracker();
  const fingers = new FingerController({ headCursorOnly: true });
  const multiHand = new MultiHandTracker();
  const pinchSmoother = new PinchMoveSmoother();
  const fireball = new FireballController();
  let lastGestureAt = 0;
  let status = 'off';
  let pinchCal = loadPinchCalibration();
  let stream = null;
  let engine = null;
  let enginePending = null;
  let prefer = workerPreference();
  let run = 0;
  let lastTick = 0;
  let lastFaceAt = 0;
  let gesture = { name: 'None', score: 0 };
  let handsOn = handsEnabled();   // hand moves are opt-in (hands-pref.js)
  let lastHeadAim = null;
  let hand = null, handAspect = 4 / 3, faceBox = null;
  let finger = null;
  let multi = null;
  let grabbed = null;          // { label, kind, at } when holding a living entity (#120)
  let mood = neutralMood();
  let moodScene = null, moodSent = null, strongPending = false, surprisePending = false;
  let surpriseAt = null;       // seconds (performance.now) of the last surprise event (#152)
  // Manual mood (#155): off by default ("No emotion"); while on it replaces the camera's mood.
  let manualOn = false, manualValue = 0, manualPrev = null;
  let advisorAt = -Infinity;
  let uiAt = 0;
  const frames = new FrameMeter();
  let fpsBefore = 0;
  let guardAt = Infinity;

  const movementCal = initMovementCalibration({
    canvas: calibModal.querySelector('.calib-wiz-canvas'),
    statusEl: calibModal.querySelector('[data-calib-status]'),
    msgEl: calibModal.querySelector('[data-calib-msg]'),
    startBtn: calibModal.querySelector('[data-calib-start]'),
    skipBtn: calibModal.querySelector('[data-calib-skip]'),
    resetBtn: calibModal.querySelector('[data-calib-reset]'),
    getState: () => ({ live: !!stream && status === 'on', hands: multi, multi, grabbed, brain: getG()?.brain, video }),
    onChange: (cal) => { pinchCal = cal; savePinchCalibration(cal); },
    onComplete: () => {
      clearTimeout(closeTimer);
      closeTimer = setTimeout(() => {
        if (!movementCal?.active) { calibModal.hidden = true; }
      }, 2200);
    },
  });
  let closeTimer = 0;
  const openCalib = () => {
    calibModal.hidden = false;
    movementCal?.start?.();
  };
  const closeCalib = () => {
    clearTimeout(closeTimer);
    movementCal?.stop?.();
    calibModal.hidden = true;
  };
  $('[data-calib-open]')?.addEventListener('click', openCalib);
  calibModal.querySelector('[data-calib-close]')?.addEventListener('click', closeCalib);
  window.addEventListener('keydown', (e) => {
    if (e.code === 'Escape' && !calibModal.hidden) { e.preventDefault(); closeCalib(); }
  });

  const coach = initCoach({
    getContext,
  });

  // ---------- state & messages ----------
  function setStatus(next, msg = '') {
    status = next;
    const live = !!stream;
    btn.dataset.state = next;
    btn.dataset.live = live ? '1' : '0';
    btn.setAttribute('aria-pressed', String(!panel.hidden));
    btn.innerHTML = live ? 'Camera on <kbd>C</kbd>' : next === 'starting' ? 'Camera… <kbd>C</kbd>' : 'Camera <kbd>C</kbd>';
    panel.dataset.state = next;
    panel.dataset.live = live ? '1' : '0';
    if (ui.dot) ui.dot.dataset.ok = live ? '1' : next === 'starting' ? 'wait' : '0';
    if (ui.startBtn) {
      ui.startBtn.disabled = next === 'starting' || live;
      ui.startBtn.textContent = live ? 'Camera Running' : next === 'starting' ? 'Starting…' : 'Start Camera';
    }
    if (ui.stopBtn) ui.stopBtn.disabled = !live && next !== 'starting';
    ui.state.textContent = live ? (next === 'on' ? 'Camera on · tracking' : 'Camera on · loading') : next === 'error' ? 'Camera unavailable' : next === 'starting' ? 'Starting camera…' : 'Camera off';
    if (msg) setMsg(msg);
    else if (live && next === 'on') setMsg('Tracking hands and facial mood locally. The cursor comes from the headset or mouse, not the webcam.');
    onStatusChange?.({ status: next, live });
  }
  function setMsg(msg) {
    if (!ui.msg) return;
    ui.msg.textContent = msg || 'Opt-in webcam for hands and face. The cursor follows the EEG headset or the mouse.';
  }

  function togglePanel(force) {
    const open = force !== undefined ? force : panel.hidden;
    panel.hidden = !open;
    btn.setAttribute('aria-pressed', String(open));
    if (open) {
      for (const [pId, bId] of [['eegPanel', 'eegBtn'], ['modelPanel', 'modelBtn'], ['settingsPanel', 'settingsBtn'], ['dbgPanel', 'dbgBtn']]) {
        const p = document.getElementById(pId);
        const b = document.getElementById(bId);
        if (p && !p.hidden) {
          p.hidden = true;
          b?.setAttribute('aria-pressed', 'false');
        }
      }
    }
  }

  // ---------- inference engine ----------
  async function ensureEngine(onStatus) {
    if (engine?.alive) return engine;
    enginePending ??= loadEngine({ hands: true, prefer, onStatus }).finally(() => { enginePending = null; });
    const e = await enginePending;
    engine = e;
    e.on({ gesture: onGesture, face: onFace, warn: (m) => setMsg(m), fail: onEngineFail });
    return e;
  }

  function onEngineFail(err) {
    console.warn('[camera] worker inference stopped, switching to the main thread:', err?.message || err);
    engine = null;
    prefer = 0;
    const id = run;
    if (!stream) return;
    ensureEngine(() => {})
      .then((e) => { e.note = `worker stopped: ${err?.message || err}`; })
      .catch((e) => { if (id === run) { teardown(); setStatus('error', `Couldn't load the camera models (${e?.message || e}).`); } });
  }

  // ---------- start / stop ----------
  async function start() {
    if (status === 'starting' || status === 'on') return;
    const id = ++run;
    fpsBefore = frames.fps(performance.now());
    togglePanel(true);
    setStatus('starting', 'Asking for the camera…');
    if (!navigator.mediaDevices?.getUserMedia) {
      setStatus('error', window.isSecureContext === false
        ? 'The camera needs https or localhost.'
        : 'This browser has no camera access.');
      return;
    }
    let s;
    try {
      s = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'user', width: { ideal: 640 }, height: { ideal: 480 }, frameRate: { ideal: 60 } }, audio: false });   // 60 fps where the webcam can: hands track up to MAX_HZ
    } catch (err) {
      if (id === run) setStatus('error', cameraError(err));
      return;
    }
    if (id !== run) { stopTracks(s); return; }
    stream = s;
    video.srcObject = stream;
    try { await video.play(); } catch { /* autoplay of a muted inline video is allowed */ }
    setStatus('starting', 'Loading hand and face models…');
    try {
      await ensureEngine((m) => { if (id === run) setMsg(m); });
    } catch (err) {
      if (id !== run) return;
      teardown();
      setStatus('error', `Couldn't load the camera models (${err?.message || err}).`);
      return;
    }
    if (id !== run) return;
    engine.resetStats();
    lastFaceAt = performance.now();
    guardAt = lastFaceAt + GPU_GUARD.graceMs;
    setStatus('on');
  }

  // Hand moves switched off: let go of anything held and forget the hands.
  function dropHands() {
    const G = getG();
    if (G) {
      G.handInput = { active: false, palm: false, at: performance.now() };
      if (G.scene) G.scene.handInput = G.handInput;
    }
    if (grabbed && G?.scene?.onGrabEnd) {
      try { G.scene.onGrabEnd({ x: G.brain?.aim?.x ?? 0.5, y: G.brain?.aim?.y ?? 0.5, cancelled: true }); } catch {}
    }
    grabbed = null;
    fingers.reset();
    multiHand.reset();
    pinchSmoother.reset();
    hand = null;
    finger = null;
    multi = null;
    gesture = { name: 'None', score: 0 };
    handsCtx.clearRect(0, 0, hands.width, hands.height);
  }

  function teardown() {
    const G = getG();
    if (grabbed && G?.scene?.onGrabEnd) {
      try { G.scene.onGrabEnd({ x: G.brain?.aim?.x ?? 0.5, y: G.brain?.aim?.y ?? 0.5, cancelled: true }); } catch {}
    }
    grabbed = null;
    stopTracks(stream);
    stream = null;
    video.srcObject = null;
    tracker.reset();
    fingers.reset();
    multiHand.reset();
    hand = null;
    finger = null;
    multi = null;
    faceBox = null;
    gesture = { name: 'None', score: 0 };
    lastHeadAim = null;
    lastGestureAt = 0;
    pinchSmoother.reset();
    fireball.reset();
    if (G) {
      G.handInput = { active: false, palm: false, at: performance.now() };
      if (G.scene) G.scene.handInput = G.handInput;
    }
    movementCal?.stop?.();
    if (calibModal) calibModal.hidden = true;
    mood = neutralMood();
    strongPending = false;
    surprisePending = false;
    handsCtx.clearRect(0, 0, hands.width, hands.height);
  }

  function stop() {
    run++;
    teardown();
    setStatus('off', 'Camera stopped and cleared.');
  }

  // The toolbar button only opens and closes the sidebar (#183). getUserMedia runs from the
  // Start Camera button alone, so reading about the moves never triggers a permission prompt.
  btn.addEventListener('click', () => togglePanel());
  ui.startBtn?.addEventListener('click', () => start());
  if (ui.handsToggle) {
    ui.handsToggle.checked = handsOn;
    ui.handsToggle.addEventListener('change', () => {
      handsOn = setHandsEnabled(ui.handsToggle.checked);
      if (!handsOn) dropHands();
    });
  }
  ui.stopBtn?.addEventListener('click', () => stop());
  $('[data-cam-close]').addEventListener('click', () => togglePanel(false));
  window.addEventListener('pagehide', () => { if (stream || status === 'starting') stop(); });

  const reticleOnScreen = (G) => ({ x: clamp(screenX(G.brain.aim.x)), y: clamp(screenY(G.brain.aim.y)) });

  // ---------- living-entity grab (#120) ----------
  function tryGrabStart(G, now) {
    const scene = G?.scene;
    if (!scene || !G?.brain || !canCast()) return false;
    const aim = { x: G.brain.aim.x, y: G.brain.aim.y };
    if (typeof scene.onGrabStart === 'function') {
      const res = scene.onGrabStart(aim);
      if (res && res.ok) {
        grabbed = { label: res.label || 'creature', kind: res.kind || 'entity', at: now };
        coach?.flash(`Picked up ${grabbed.label} · move the cursor & release to drop`, 'ok', 2800);
        G.hud?.showCause?.(`🤏 Grabbed ${grabbed.label} — move the cursor & release to place`);
        return true;
      }
      const why = res?.reason || 'Move the cursor over a living creature to grab';
      coach?.flash(why, 'bad', 2000);
      return false;
    }
    return false;
  }

  function tryGrabEnd(G, now, cancelled = false) {
    if (!grabbed) return false;
    const scene = G?.scene;
    const label = grabbed.label;
    grabbed = null;
    if (scene && typeof scene.onGrabEnd === 'function' && G?.brain) {
      try {
        scene.onGrabEnd({ x: G.brain.aim.x, y: G.brain.aim.y, cancelled });
      } catch (err) {
        console.error('[camera] onGrabEnd failed:', err);
      }
    }
    if (!cancelled) {
      coach?.flash(`Dropped ${label}`, 'ok', 1600);
      G?.hud?.showCause?.(`🤏 Placed ${label}`);
    }
    return true;
  }

  // Manual mood controls (#155).
  {
    const off = panel.querySelector('[data-cam-mood-off]'), slider = panel.querySelector('[data-cam-mood-slider]');
    const out = panel.querySelector('[data-cam-mood-manual]');
    const show = () => {
      if (slider) slider.disabled = !manualOn;
      if (out) out.textContent = !manualOn ? 'off' : manualValue < 0.35 ? 'happy' : manualValue > 0.65 ? 'surprised' : 'mix';
    };
    off?.addEventListener('change', () => {
      manualOn = !off.checked; manualPrev = null;
      if (!manualOn) { mood = neutralMood(); surprisePending = false; }
      show();
    });
    slider?.addEventListener('input', () => { manualValue = Number(slider.value) / 100; show(); });
    show();
  }

  // ---------- mood (#117: Joy continuous + Surprise debounced event) ----------
  function syncMood(G) {
    const scene = G?.scene;
    if (!scene?.onMood || !canCast()) return;
    if (!stream && !manualOn && scene !== moodScene) {
      moodScene = scene;
      moodSent = 'neutral';
      strongPending = false;
      surprisePending = false;
      return;
    }
    // #152: one surprise per cooldown whatever the source (the slider can be wiggled across Mix).
    const nowSec = performance.now() / 1000;
    if (surprisePending && !surpriseReady(nowSec, surpriseAt)) surprisePending = false;
    let reason = null;
    if (surprisePending) reason = 'surprise';
    else if (scene !== moodScene || mood.dominant !== moodSent) reason = 'change';
    else if (strongPending) reason = 'strong';
    if (!reason) return;
    const changed = mood.dominant !== moodSent;
    const wasSurprise = surprisePending;
    moodScene = scene;
    moodSent = mood.dominant;
    strongPending = false;
    surprisePending = false;
    const source = manualOn ? 'manual' : 'camera';
    if (wasSurprise) surpriseAt = nowSec;
    try {
      scene.onMood({ ...mood, reason, surpriseTriggered: wasSurprise, active: manualOn || !!stream, source });
    } catch (err) {
      console.error('[camera] mood failed:', err);
    }
    // #152: the cause line names the emotion and what it did in this scene; the coach echoes it.
    const kind = sceneKind(scene);
    if (wasSurprise) {
      const text = surpriseCause(kind, source);
      G.hud?.showCause?.(text, { tone: 'surprise', ms: 3000 });
      coach?.flash(text, 'info', 2400);
      advisorReact(G, 'surprise');
    } else if (reason === 'change' && changed && mood.dominant === 'joy') {
      const text = joyCause(kind, source);
      G.hud?.showCause?.(text, { tone: 'joy', ms: 3000 });
      coach?.flash(text, 'info', 2400);
      advisorReact(G, 'joy');
    }
  }

  function advisorReact(G, dom = mood.dominant) {
    const t = performance.now() / 1000;
    const r = MOOD_LINES[dom];
    if (!r || !G.advisor || t - advisorAt < ADVISOR_GAP) return;
    const line = r.lines[Math.floor(Math.random() * r.lines.length)];
    if (G.advisor.say(`mood-${dom}`, line, { mood: r.mood, priority: 1, cooldown: 40 })) advisorAt = t;
  }

  // ---------- model results ----------
  function castFireball(fb, G) {
    const scene = G?.scene;
    const aim = G?.brain?.aim;
    if (!scene || !aim) return false;
    if (typeof scene.onFireball !== 'function') return false;
    try { scene.onFireball(aim, { color: fb.color, charge: fb.charge }); }
    catch (err) { console.error('[camera] onFireball failed:', err); return false; }
    G.stats.casts = (G.stats.casts || 0) + 1;
    coach?.flash(`🔥 Fireball ${Math.round(fb.charge * 100)}%`, 'ok', 1400);
    G.hud?.showCause?.(`🔥 Charged fireball ${Math.round(fb.charge * 100)}% · ${fb.color}`, { tone: 'ok' });
    return true;
  }

  function onGesture(r) {
    if (!stream || status !== 'on' || !handsOn) return;
    const now = performance.now();
    const gestureDt = Math.min(0.2, (now - (lastGestureAt || now)) / 1000) || 1 / 30;
    lastGestureAt = now;
    hand = r.hand || null;
    handAspect = r.aspect || 4 / 3;
    gesture = { name: r.name || 'None', score: r.score || 0 };
    const rawHands = Array.isArray(r.hands) && r.hands.length
      ? r.hands
      : (hand ? [{ landmarks: hand, name: gesture.name, score: gesture.score }] : []);

    const G = getG();
    const live = canCast() && !!G?.brain;
    // Rain spell (#330): the Stone Age reads this shared hand state. Open palm + Insight.
    const handSize = hand && hand.length >= 21 ? pinchMetrics(hand, handAspect) : null;
    const palm = !!hand && (
      (gesture.name === 'Open_Palm' && gesture.score >= 0.55)
      || (handSize && handSize.reach > 1.35)
    );
    if (G) {
      G.handInput = { active: true, palm, at: now };
      if (G.scene) G.scene.handInput = G.handInput;   // AI-world capital tribe shares the hand state
    }

    // Aim movement while pinched (#270): it is head pose or mouse — never the webcam.
    // Screen-space keeps the tilt independent of the world camera; screenX/Y map a world aim
    // back to the screen when screenAim is absent.
    const headNow = G?.brain?.screenAim
      || (G?.brain?.aim ? { x: screenX(G.brain.aim.x), y: screenY(G.brain.aim.y) } : null);
    const headDx = lastHeadAim && headNow ? headNow.x - lastHeadAim.x : 0;
    const headDy = lastHeadAim && headNow ? headNow.y - lastHeadAim.y : 0;
    lastHeadAim = headNow ? { x: headNow.x, y: headNow.y } : null;

    // Single-hand metrics preserved in headCursorOnly mode so hands NEVER move G.brain.aim (#116, #120)
    finger = fingers.update({
      hand,
      gesture,
      aim: live ? reticleOnScreen(G) : { x: 0.5, y: 0.55 },
      now,
      aspect: handAspect,
    });

    if (app && hand && hand.length >= 21) {
      try {
        const coords = new Float64Array(63);
        for (let i = 0; i < 21; i++) {
          coords[i * 3] = hand[i].x;
          coords[i * 3 + 1] = hand[i].y;
          coords[i * 3 + 2] = hand[i].z || 0;
        }
        app.on_hand_landmarks(coords, handAspect, now);
      } catch (err) {
        console.warn('RustApp on_hand_landmarks failed:', err);
      }
    }

    // Multi-hand tracker: 2-hand continuous zoom (#118, #119), 1-hand grab (#120)
    multi = multiHand.update({ hands: rawHands, now, aspect: handAspect });
    const fb = fireball.update({
      hand,
      gesture,
      mood,
      now,
      aspect: handAspect,
      pinching: !!(finger?.pinching || multi?.grab?.active),
    });
    if (fb.cast) castFireball(fb, G);
    if (multi.grab.started) pinchSmoother.reset();
    const pinchMove = pinchSmoother.filter({
      handDx: multi.grab.dx,
      handDy: multi.grab.dy,
      headDx,
      headDy,
    }, gestureDt);

    if (live) {
      // 1. Two-hand continuous pinch zoom (#119)
      if (multi.bothPinched && grabbed) {
        tryGrabEnd(G, now, true);
      }
      if (multi.zoomDelta && Math.abs(multi.zoomDelta) > 0 && onZoomDelta) {
        onZoomDelta(multi.zoomDelta, multi.zoomFactor);
      }

      // 2. One-hand pinch grab at the reticle (#120) & Globe/World movement
      if (multi.grab.started) {
        tryGrabStart(G, now);
        if (!grabbed && onGrabMove) {
          onGrabMove({ dx: 0, dy: 0, headDx: 0, headDy: 0, started: true, active: true, aim: G.brain.aim, grab: multi.grab });
        }
      } else if (multi.grab.active) {
        if (grabbed) {
          G.scene?.onGrabMove?.({ x: G.brain.aim.x, y: G.brain.aim.y });
        } else if (onGrabMove) {
          const sideCal = multi.grab.side === 'left' ? pinchCal.left : multi.grab.side === 'right' ? pinchCal.right : null;
          const hx = sideCal ? (sideCal.gainX ?? sideCal.gain ?? 1) : 1;
          const hy = sideCal ? (sideCal.gainY ?? sideCal.gain ?? 1) : 1;
          const headCal = pinchCal.head || {};
          const tx = headCal.gainX ?? headCal.gain ?? 1;
          const ty = headCal.gainY ?? headCal.gain ?? 1;
          onGrabMove({ dx: pinchMove.handDx * hx, dy: pinchMove.handDy * hy, headDx: pinchMove.headDx * tx, headDy: pinchMove.headDy * ty, started: false, active: true, aim: G.brain.aim, grab: multi.grab });
        }
      } else if (multi.grab.released || multi.grab.cancelled) {
        if (grabbed) {
          tryGrabEnd(G, now, !!multi.grab.cancelled);
        } else if (onGrabMove) {
          pinchSmoother.reset();
          onGrabMove({ dx: 0, dy: 0, headDx: 0, headDy: 0, released: true, cancelled: !!multi.grab.cancelled, active: false, aim: G.brain.aim, grab: multi.grab });
        }
      }
    }
    drawHands();
  }

  function onFace(r) {
    if (!stream || status !== 'on') return;
    const at = r.at ?? performance.now();
    const dt = clamp((at - lastFaceAt) / 1000, 0, 0.5);
    lastFaceAt = at;
    const out = tracker.update(r.blendshapes || null, dt);
    mood = out.mood;
    faceBox = r.box || null;
    if (out.event === 'strong') strongPending = true;
    if (out.surpriseTriggered) surprisePending = true;

    if (app && r.blendshapes) {
      try {
        const b = blendshapeMap(r.blendshapes);
        app.on_face_blendshapes(
          true,
          b.mouthSmileLeft || 0,
          b.mouthSmileRight || 0,
          b.browDownLeft || 0,
          b.browDownRight || 0,
          b.browInnerUp || 0,
          b.eyeWideLeft || 0,
          b.eyeWideRight || 0,
          b.jawOpen || 0,
          b.mouthFrownLeft || 0,
          b.mouthFrownRight || 0,
          dt
        );
      } catch (err) {
        console.warn('RustApp on_face_blendshapes failed:', err);
      }
    }
  }

  function drawHands() {
    const w = video.videoWidth || 320, h = video.videoHeight || 240;
    if (hands.width !== w || hands.height !== h) { hands.width = w; hands.height = h; }
    handsCtx.clearRect(0, 0, w, h);
    if (multi && (multi.left || multi.right)) {
      if (multi.left?.visible) {
        drawHand(handsCtx, multi.left.landmarks, w, h, {
          held: !!grabbed,
          pinching: !!multi.left.pinching,
          nearPinch: multi.left.ratio != null && multi.left.ratio < fingers.pinch.exit,
          side: 'left',
        });
      }
      if (multi.right?.visible) {
        drawHand(handsCtx, multi.right.landmarks, w, h, {
          held: !!grabbed,
          pinching: !!multi.right.pinching,
          nearPinch: multi.right.ratio != null && multi.right.ratio < fingers.pinch.exit,
          side: 'right',
        });
      }
    } else if (hand) {
      drawHand(handsCtx, hand, w, h, {
        held: !!grabbed,
        pinching: !!finger?.pinching,
        nearPinch: finger?.ratio != null && finger.ratio < fingers.pinch.exit,
      });
    }
  }

  // ---------- UI ----------
  function nowText(now) {
    if (fireball.state === 'charging' || fireball.state === 'primed') {
      return `🔥 Fireball ${Math.round(fireball.charge * 100)}% · ${fireball.state === 'primed' ? 'open your palm!' : 'pull back, then thrust'}`;
    }
    if (multi?.bothPinched) {
      return `👐 Two-hand zoom (${Math.round((multi.zoomFactor || 1) * 100)}%) · unpinch to freeze`;
    }
    if (grabbed) {
      return `🤏 Holding ${grabbed.label} · move the cursor & release pinch to drop`;
    }
    if (multi?.grab?.active) {
      return 'Pinch: move your hand to pan · head motion tilts the 3D view';
    }
    const aimBy = aimSource(getG(), now).short;
    if (!(stream && engine)) return `Aim with the ${aimBy}.`;
    const g = GESTURE_LABEL[gesture.name] || gesture.name;
    return hand ? `Hand: ${g} · ${aimBy} aims` : 'Show 1 hand to grab/move, or 2 hands to pinch-zoom.';
  }

  function fmtRate(r) {
    if (!r || !r.count) return 'waiting…';
    const parts = [`${Math.round(r.hz)} Hz`];
    if (r.ms != null) parts.push(`${Math.round(r.ms)} ms`);
    if (r.latency != null) parts.push(`${Math.round(r.latency)} ms lag`);
    return parts.join(' · ');
  }

  function updatePanel(now) {
    for (const k of EMOTIONS) {
      const pctStr = `${Math.round((mood[k] || 0) * 100)}%`;
      if (ui.bars[k]) ui.bars[k].style.width = pctStr;
      if (ui.vals[k]) ui.vals[k].textContent = pctStr;
    }
    const dom = !stream ? 'camera off' : !mood.face ? 'no face' : mood.calibrating ? 'reading your face…' : mood.dominant;
    if (ui.dominant.textContent !== dom) ui.dominant.textContent = dom;
    panel.dataset.mood = stream ? mood.dominant : 'off';

    const snap = engine ? engine.snapshot(now) : null;
    if (ui.mode) {
      if (stream && snap) {
        const where = snap.mode === 'worker' ? `worker ×${snap.workers}` : 'main thread';
        const dl = [...new Set([snap.gesture.delegate, snap.face.delegate].filter(Boolean))].join('/');
        ui.mode.textContent = `${where}${dl ? ` · ${dl}` : ''}`;
      } else {
        ui.mode.textContent = status === 'starting' ? 'Loading…' : 'Idle';
      }
    }
    if (ui.fps) ui.fps.textContent = `${Math.round(frames.fps(now))} fps`;
    if (ui.rateVideo) ui.rateVideo.textContent = stream && snap?.video?.count ? `${Math.round(snap.video.hz)} fps` : '--';
    if (ui.rateHands) ui.rateHands.textContent = stream && snap ? fmtRate(snap.gesture) : '--';
    if (ui.rateFace) ui.rateFace.textContent = stream && snap ? fmtRate(snap.face) : '--';

    const handCount = (multi?.left?.visible ? 1 : 0) + (multi?.right?.visible ? 1 : 0);
    if (ui.gesture) {
      if (!stream) {
        ui.gesture.textContent = 'camera off';
      } else if (handCount === 2) {
        const l = multi.left?.name ? GESTURE_LABEL[multi.left.name] || multi.left.name : 'hand';
        const r = multi.right?.name ? GESTURE_LABEL[multi.right.name] || multi.right.name : 'hand';
        ui.gesture.textContent = `2H · L: ${l} · R: ${r}`;
      } else if (handCount === 1) {
        const h = multi.left?.visible ? multi.left : multi.right;
        const name = h?.name ? GESTURE_LABEL[h.name] || h.name : GESTURE_LABEL[gesture.name] || gesture.name;
        ui.gesture.textContent = `1H · ${name}`;
      } else if (hand) {
        ui.gesture.textContent = `1H · ${GESTURE_LABEL[gesture.name] || gesture.name}`;
      } else {
        ui.gesture.textContent = 'no hand';
      }
    }
    if (ui.pinch) {
      if (multi?.bothPinched) {
        ui.pinch.textContent = `2H ZOOM ×${(multi.zoomFactor || 1).toFixed(2)}`;
      } else if (grabbed) {
        ui.pinch.textContent = `HOLDING ${grabbed.label}`;
      } else if (finger?.ratio != null) {
        ui.pinch.textContent = finger.pinching ? `PINCH (${finger.ratio.toFixed(2)})` : `open (${finger.ratio.toFixed(2)})`;
      } else {
        ui.pinch.textContent = '1H grab · 2H zoom';
      }
    }
    const aim = aimSource(getG(), now);
    if (ui.aim && ui.aim.textContent !== aim.label) ui.aim.textContent = aim.label;
    panel.dataset.aim = aim.id;
    if (ui.range) {
      ui.range.textContent = grabbed ? `carrying ${grabbed.label}` : multi?.bothPinched ? 'two-hand zoom active' : 'ready';
    }
    if (ui.fbState) {
      ui.fbState.textContent = fireball.state;
      drawPanelOrb(now);
      const lit = ringSegments(fireball.charge, 10);
      ui.fbMeter.querySelectorAll('i').forEach((el, i) => { el.style.background = i < lit ? fireball.color : ''; });
      ui.fbPct.textContent = `${Math.round(fireball.charge * 100)}%`;
    }
  }

  // The camera panel's fireball: the in-world pixel sprite, drawn 2x on a 24 px canvas.
  function drawPanelOrb(now) {
    const fbOrbCtx = ui.fbOrb?.getContext?.('2d');   // the same context every call; no TDZ at init
    if (!fbOrbCtx) return;
    fbOrbCtx.clearRect(0, 0, 24, 24);
    const charging = fireball.state === 'charging' || fireball.state === 'primed';
    if (!charging) {
      drawFireballSprite(fbOrbCtx, 12, 12, { r: 2, frame: 0, color: fireball.color, alpha: 0.55, tongues: false });
      return;
    }
    const f = formation(fireball.charge, now / 1000);
    drawPixels(fbOrbCtx, 12, 12, f.embers.map((e) => ({ x: e.x, y: e.y, c: e.hot ? 'hot' : 'tint' })), { hot: FB_HOT, tint: fireball.color });
    drawFireballSprite(fbOrbCtx, 12, 12, { r: f.r, frame: f.frame, color: fireball.color });
  }

  function drawFx(now) {
    const G = getG();
    const carrying = !!grabbed && !!G?.brain;
    const charging = fireball.state === 'charging' || fireball.state === 'primed';
    if (!carrying && !charging) {
      if (fxDirty) { fxCtx.clearRect(0, 0, fx.width, fx.height); fxDirty = false; }
      return;
    }
    const dpr = window.devicePixelRatio || 1;
    const cw = Math.round(fx.clientWidth * dpr), ch = Math.round(fx.clientHeight * dpr);
    if (!cw || !ch) return;
    if (fx.width !== cw || fx.height !== ch) { fx.width = cw; fx.height = ch; }
    fxCtx.clearRect(0, 0, cw, ch);
    fxDirty = true;
    const u = Math.max(1, cw / 320);
    const glyphAt = (glyph, x, y, size) => {
      fxCtx.font = `${Math.round(size)}px "Segoe UI Emoji", "Apple Color Emoji", "Noto Color Emoji", sans-serif`;
      fxCtx.textAlign = 'center';
      fxCtx.fillText(glyph, x, y);
    };
    const r = reticleOnScreen(G);
    const x = r.x * cw, y = r.y * ch;
    if (charging) {
      // Pixel-art charge-up at the cursor (#319): a segmented ring fills, embers spiral in and the
      // fireball grows from a spark; at full charge it strains, pulsing a pixel.
      const f = formation(fireball.charge, now / 1000);
      drawSegmentRing(fxCtx, x, y, 13, fireball.charge, fireball.color, { scale: u, n: 16 });
      drawPixels(fxCtx, x, y, f.embers.map((e) => ({ x: e.x, y: e.y, c: e.hot ? 'hot' : 'tint' })), { hot: FB_HOT, tint: fireball.color }, { scale: u });
      drawFireballSprite(fxCtx, x, y, { r: f.r, frame: f.frame, color: fireball.color, scale: u });
    }
    if (carrying) {
      drawSegmentRing(fxCtx, x, y, 8, 1, '#ff8a3d', { scale: u, n: 12 });
      glyphAt('🤏', x, y - 11 * u, 8 * u);
    }
    fxCtx.globalAlpha = 1;
  }

  // ---------- loop ----------
  function tick() {
    const now = performance.now();
    if (now - lastTick < 8) return;
    lastTick = now;
    frames.frame(now);
    if (stream && engine?.alive && status === 'on' && video.readyState >= 2) {
      engine.pump(video, now, { hands: handsOn, tracking: !!(hand || multi?.left || multi?.right) });
      if (now >= guardAt) guardFrames(now);
    }
    const G = getG();
    if (G) {
      if (manualOn) {
        mood = manualMood(manualValue);
        if (surpriseRisingEdge(manualPrev, manualValue)) surprisePending = true;   // once per crossing
        manualPrev = manualValue;
      }
      G.mood = { ...(manualOn || stream ? mood : neutralMood()), active: manualOn || !!stream, source: manualOn ? 'manual' : stream ? 'camera' : 'none', surpriseAt, kind: sceneKind(G.scene) };
      if (grabbed && G.brain) {
        G.scene?.onGrabMove?.({ x: G.brain.aim.x, y: G.brain.aim.y });
      }
      syncMood(G);
    }
    if (fireball.keyboard) {
      const fb = fireball.update({ mood, now, pinching: false });
      if (fb.cast) castFireball(fb, getG());
    }
    if (!panel.hidden && now - uiAt > 60) { uiAt = now; updatePanel(now); }
    coach?.update(now, {
      live: !!stream && status === 'on',
      leftVisible: !!multi?.left?.visible,
      rightVisible: !!multi?.right?.visible,
      bothPinched: !!multi?.bothPinched,
      grabActive: !!grabbed || !!multi?.grab?.active,
      fireball: fireball.state,
      pinchStrength: finger?.pinching ? 1 : 0,
      statusText: nowText(now),
    });
    if (!calibModal.hidden) movementCal?.update?.(now);
    drawFx(now);
  }
  function guardFrames(now) {
    guardAt = now + GPU_GUARD.everyMs;
    if (engine.mode !== 'worker' || !engine.usesGpu()) return;
    const fps = frames.fps(now);
    if (fpsBefore >= GPU_GUARD.minFps && fps < fpsBefore * GPU_GUARD.ratio) {
      engine.moveToCpu(`the game fell from ${Math.round(fpsBefore)} to ${Math.round(fps)} fps`);
      guardAt = Infinity;
    }
  }
  function rafLoop() { tick(); requestAnimationFrame(rafLoop); }
  requestAnimationFrame(rafLoop);
  setInterval(() => { if (performance.now() - lastTick > 45) tick(); }, 33);

  setStatus('off');

  function debugState(now = performance.now()) {
    return {
      status, live: !!stream, video,
      engine: engine ? engine.snapshot(now) : null,
      gesture: { ...gesture }, hand, handAspect, faceBox,
      hands: {
        left: multi?.left || null,
        right: multi?.right || null,
        bothPinched: !!multi?.bothPinched,
        zoomActive: !!multi?.zoomActive,
        distance: multi?.distance ?? null,
        pinchStart: multi?.pinchStart ?? null,
        zoomFactor: multi?.zoomFactor ?? 1,
      },
      grabbed: grabbed ? { ...grabbed } : null,
      finger: finger && { mode: finger.mode, ratio: finger.ratio, reach: finger.reach, pinching: finger.pinching, progress: finger.progress, suppress: finger.suppress, tip: finger.tip },
      pinch: { enter: fingers.pinch.enter, exit: fingers.pinch.exit },
      range: fingers.range.snapshot(),
      fireball: { state: fireball.state, charge: fireball.charge, color: fireball.color, keyboard: fireball.keyboard },
      mood: { ...mood },
    };
  }

  return {
    get status() { return status; },
    get live() { return !!stream; },
    get mood() { return mood; },
    get engine() { return engine; },
    get vision() { return engine?.vision || null; },
    get grabbed() { return grabbed; },
    start, stop, togglePanel,
    fireballKeyDown: () => fireball.keyboardDown(mood, performance.now()),
    fireballKeyUp: () => {
      const fb = fireball.keyboardUp(mood, performance.now());
      if (fb.cast) castFireball(fb, getG());
    },
    tracker, fingers, multiHand, coach, video,
    debugState,
    simulate: { gesture: onGesture, face: onFace },
    async setInferenceMode(mode) {
      prefer = mode;
      const old = engine;
      engine = null;
      old?.close();
      if (stream && status === 'on') { await ensureEngine(() => {}); engine.resetStats(); guardAt = performance.now() + GPU_GUARD.graceMs; }
      return engine?.snapshot().mode || null;
    },
    async loadModels() { const v = await loadVision(undefined, { hands: true }); return { timings: v.timings, delegates: v.delegates }; },
  };
}
