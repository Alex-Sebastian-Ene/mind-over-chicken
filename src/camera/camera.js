// camera (#33, #67, #116–#123): opt-in webcam input.
//   Aim:   the webcam does NOT track the head (#189). G.brain.aim comes from the EEG headset's head
//          pose while it streams, else the mouse (aim-source.js). Hands never overwrite it (#116, #120).
//   Hands: MediaPipe Gesture Recognizer (numHands=2, #118) →
//          1. One-hand pinch grabs & relocates living entities under the cursor (#120)
//          2. Two-hand pinch distance continuously zooms in/out & freezes on release (#119)
//          3. Three core moves (#122): Pinch (1), Open Palm / Calm (2), Closed Fist / Strike (3)
//   Face:  MediaPipe Face Landmarker blendshapes → Joy (continuous) + Surprise (debounced event) (#117).
//   Coach: Collapsible bottom panel with procedural 21-landmark ghost hands (#123).
//
// PRIVACY: the camera starts only when the player clicks "Camera". Frames go from the <video>
// element to MediaPipe inside this tab (or its own workers). Nothing is recorded, stored or
// uploaded: no frame, landmark or score leaves the device. Turning the camera off stops every
// track. The only network traffic is the one-time download of the runtime and the two models.
//
// The player stays in control: casts and moods are an extra channel on top of the headset and
// keyboard. They call scene hooks and move the aim the way the mouse does; they never change
// the brain's calm or focus and never block another input. Face emotion does count toward
// brain state and game reactions.

import { clamp } from '../sim/util.js';
import { SPELLS, SPELL_BY_ID, GESTURE_LABEL, spellForKeyCode, CAMERA_CASTS_ENABLED } from './spells.js';
import { CastController } from './casts.js';
import { EMOTIONS, EmotionTracker, neutralMood, blendshapeMap } from './emotion.js';
import { loadVision } from './vision.js';
import { loadEngine, workerPreference } from './engine.js';
import { FingerController, MultiHandTracker } from './fingers.js';
import { initCoach } from './coach.js';
import { FrameMeter } from './meter.js';
import { drawHand } from './draw.js';
import { aimSource } from './aim-source.js';

const ADVISOR_GAP = 14;      // seconds between the advisor's mood remarks
const PING_MS = 900;         // how long a cast marker stays on the map
const MISS_MS = 1400;        // how long "ready in …" stays up after a cooling cast
const GPU_GUARD = { ratio: 0.85, minFps: 30, graceMs: 4000, everyMs: 1000 };

const MOOD_LINES = {
  joy: { mood: 'happy', lines: ['You are smiling, my lord. The people have started dancing. Badly, but with feeling.', 'A divine smile! Everyone down here feels lucky today.'] },
  surprise: { mood: 'panic', lines: ['You look startled! The sky took it personally.', 'Such a face! The heavens flashed in sympathy.'] },
};

const TEMPLATE = `
  <header class="eeg-head">
    <h2>Camera &amp; Hand Moves</h2>
    <button type="button" class="tool" data-cam-close title="Close Camera sidebar (\`)">Close <kbd>\`</kbd></button>
  </header>

  <section class="eeg-sec">
    <div class="eeg-status-row">
      <span class="eeg-dot" data-cam-dot data-ok="0"></span>
      <strong data-cam-state>Camera off</strong>
      <span class="eeg-qual" data-cam-mode>Idle</span>
    </div>
    <p class="eeg-msg" data-cam-msg role="status">Opt-in webcam for hands and face: pinch-grab creatures, two-hand pinch-zoom, palm to calm, fist to strike; your expression sets the mood. The webcam does not track your head: the cursor follows the EEG headset's head pose, or the mouse. Frames never leave this device.</p>
    <div class="eeg-url-row">
      <button type="button" class="btn btn-primary btn-small" data-cam-start>Start Camera</button>
      <button type="button" class="btn btn-small" data-cam-stop disabled>Stop &amp; Clear</button>
    </div>
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
      </div>
    </div>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Core Hand Moves (Keys 1–3)</h3>
      <span class="eeg-qual" data-cam-last>Ready</span>
    </div>
    <p class="eeg-msg" data-cam-now>Aim with the headset or mouse. Pinch=Grab (1), Palm=Calm (2), Fist=Strike (3).</p>
    <div class="cam-spells" role="group" aria-label="Core moves, keys 1 to 3">
      ${SPELLS.map((s) => `<button type="button" class="cam-spell" data-spell="${s.id}" aria-label="${s.name} (key ${s.key})" title="${s.name} · key ${s.key}. ${s.hint}">
        <svg class="cam-ring" viewBox="0 0 36 36" aria-hidden="true"><circle class="cam-ring-track" cx="18" cy="18" r="16"/><circle class="cam-ring-hold" cx="18" cy="18" r="16" pathLength="100"/><circle class="cam-ring-cd" cx="18" cy="18" r="16" pathLength="100"/></svg>
        <span class="cam-glyph" aria-hidden="true">${s.glyph}</span><kbd>${s.key}</kbd>
      </button>`).join('')}
    </div>
    <div class="eeg-imu-mini">
      <span>Action State: <b data-cam-range>ready</b></span>
    </div>
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
    <p class="cam-privacy">Frames never leave this device. Nothing is uploaded.</p>
  </section>`;

function cameraError(err, castsEnabled) {
  const fallback = castsEnabled ? ' Keys 1–3 still trigger every move.' : '';
  const name = err?.name || '';
  if (name === 'NotAllowedError' || name === 'PermissionDeniedError' || name === 'SecurityError')
    return `Camera permission was denied. Allow it in the address bar and click Camera again.${fallback}`;
  if (name === 'NotFoundError' || name === 'DevicesNotFoundError' || name === 'OverconstrainedError')
    return `No camera found.${fallback}`;
  if (name === 'NotReadableError' || name === 'TrackStartError' || name === 'AbortError')
    return `The camera is busy in another app. Close it and click Camera again.${fallback}`;
  return `Camera unavailable (${name || err?.message || 'unknown error'}).${fallback}`;
}

function stopTracks(stream) { stream?.getTracks().forEach((t) => t.stop()); }

export function initCamera({
  app = null,
  getG = () => window.__moc?.G,
  canCast = () => true,
  castsEnabled = CAMERA_CASTS_ENABLED,
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
  const spellRow = $('.cam-spells');
  if (!castsEnabled) {
    spellRow.style.display = 'none';
    panel.dataset.casts = 'off';
    $('[data-cam-now]').textContent = 'Face mood is active. Hand casts are deferred.';
  }
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
    rateHands: $('[data-cam-rate-hands]'),
    rateFace: $('[data-cam-rate-face]'),
    aim: $('[data-cam-aim]'),
    gesture: $('[data-cam-gesture]'),
    pinch: $('[data-cam-pinch]'),
    range: $('[data-cam-range]'),
    last: $('[data-cam-last]'),
    now: $('[data-cam-now]'),
    dominant: $('[data-cam-dominant]'),
    bars: Object.fromEntries(EMOTIONS.map((k) => [k, $(`[data-emotion="${k}"] .cam-bar > span`)])),
    vals: Object.fromEntries(EMOTIONS.map((k) => [k, $(`[data-emotion-val="${k}"]`)])),
    spells: Object.fromEntries(SPELLS.map((s) => [s.id, panel.querySelector(`[data-spell="${s.id}"]`)])),
  };

  // Overlay canvas for hold rings, grabbed entity tether, and cast pings.
  const fx = document.createElement('canvas');
  fx.className = 'cam-fx';
  fx.setAttribute('aria-hidden', 'true');
  screen?.insertAdjacentElement('afterend', fx);
  const fxCtx = fx.getContext('2d');
  let fxDirty = false;

  const casts = new CastController();
  const tracker = new EmotionTracker();
  const fingers = new FingerController({ headCursorOnly: true });
  const multiHand = new MultiHandTracker();
  const castFallback = castsEnabled ? ' Keys 1–3 still trigger every move.' : '';
  let status = 'off';          // off | starting | on | error
  let stream = null;
  let engine = null;
  let enginePending = null;
  let prefer = workerPreference();
  let run = 0;
  let lastTick = 0;
  let lastFaceAt = 0;
  let gesture = { name: 'None', score: 0 };
  let hold = { held: null, progress: 0, aiming: false };
  let hand = null, handAspect = 4 / 3, faceBox = null;
  let finger = null;
  let multi = null;
  let grabbed = null;          // { label, kind, at } when holding a living entity (#120)
  let armed = 'fist';
  let mood = neutralMood();
  let moodScene = null, moodSent = null, strongPending = false, surprisePending = false;
  let advisorAt = -Infinity;
  let lastCast = null;
  let miss = null;
  const history = [];
  let uiAt = 0;
  const pings = [];
  const frames = new FrameMeter();
  let fpsBefore = 0;
  let guardAt = Infinity;

  const coach = initCoach({
    getContext,
    onCastMove: (id) => cast(id, 'tap'),
  });

  // ---------- state & messages ----------
  function setStatus(next, msg = '') {
    status = next;
    const live = !!stream;
    btn.dataset.state = next;
    btn.dataset.live = live ? '1' : '0';
    btn.setAttribute('aria-pressed', String(!panel.hidden));
    btn.innerHTML = live ? 'Camera on <kbd>`</kbd>' : next === 'starting' ? 'Camera… <kbd>`</kbd>' : 'Camera <kbd>`</kbd>';
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
      for (const [pId, bId] of [['eegPanel', 'eegBtn'], ['modelPanel', 'modelBtn'], ['settingsPanel', 'settingsBtn']]) {
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
    enginePending ??= loadEngine({ hands: castsEnabled, prefer, onStatus }).finally(() => { enginePending = null; });
    const e = await enginePending;
    engine = e;
    e.on({ gesture: onGesture, face: onFace, warn: (m) => setMsg(m + castFallback), fail: onEngineFail });
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
        ? `The camera needs https or localhost.${castFallback}`
        : `This browser has no camera access.${castFallback}`);
      return;
    }
    let s;
    try {
      s = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'user', width: { ideal: 640 }, height: { ideal: 480 } }, audio: false });
    } catch (err) {
      if (id === run) setStatus('error', cameraError(err, castsEnabled));
      return;
    }
    if (id !== run) { stopTracks(s); return; }
    stream = s;
    video.srcObject = stream;
    try { await video.play(); } catch { /* autoplay of a muted inline video is allowed */ }
    setStatus('starting', castsEnabled ? 'Loading hand and face models…' : 'Loading face model…');
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

  function teardown() {
    const G = getG();
    if (grabbed && G?.scene?.onGrabEnd) {
      try { G.scene.onGrabEnd({ x: G.brain?.aim?.x ?? 0.5, y: G.brain?.aim?.y ?? 0.5, cancelled: true }); } catch {}
    }
    grabbed = null;
    stopTracks(stream);
    stream = null;
    video.srcObject = null;
    casts.reset();
    tracker.reset();
    fingers.reset();
    multiHand.reset();
    hand = null;
    finger = null;
    multi = null;
    faceBox = null;
    gesture = { name: 'None', score: 0 };
    hold = { held: null, progress: 0, aiming: false };
    mood = neutralMood();
    strongPending = false;
    surprisePending = false;
    handsCtx.clearRect(0, 0, hands.width, hands.height);
  }

  function stop() {
    run++;
    teardown();
    setStatus('off', 'Camera stopped and cleared. Keys 1–3 still trigger every move.');
  }

  btn.addEventListener('click', () => {
    if (panel.hidden) {
      togglePanel(true);
      if (status === 'off') start();
    } else {
      togglePanel(false);
    }
  });
  ui.startBtn?.addEventListener('click', () => start());
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
        history.unshift({ id: 'pinch', via: 'pinch', at: now });
        history.length = Math.min(history.length, 5);
        lastCast = { id: 'pinch', at: now };
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
      if (G?.brain) pings.push({ x: G.brain.aim.x, y: G.brain.aim.y, at: now, glyph: '🤏' });
    }
    return true;
  }

  // ---------- casting (#122) ----------
  function cast(id, via, now = performance.now()) {
    if (!castsEnabled) return false;
    const G = getG();
    const scene = G?.scene;
    if (!scene || !SPELL_BY_ID[id] || !canCast()) return false;

    // Key 1 or Pinch button toggles living-entity grab at the reticle (#120, #122)
    if (id === 'pinch') {
      if (grabbed) {
        return tryGrabEnd(G, now, false);
      }
      if (tryGrabStart(G, now)) return true;
      // Fallback if scene doesn't implement onGrabStart or nothing under cursor
      if (via !== 'gesture' && !casts.trigger(id, now)) return false;
      const aim = { x: G.brain.aim.x, y: G.brain.aim.y };
      scene.onCast?.('pinch', aim);
      return true;
    }

    if (via !== 'gesture' && !casts.trigger(id, now)) return false;
    const aim = { x: G.brain.aim.x, y: G.brain.aim.y };
    try {
      if ((id === 'fist' || id === 'fireball') && scene.onBlink && !scene.onCast) {
        G.hud?.flashBlink?.();
        scene.onBlink(aim);
      } else {
        scene.onCast?.(id, aim);
      }
      if (G.engine?.game_on_cast) {
        G.engine.game_on_cast(id, aim.x * 320, aim.y * 180);
      }
    } catch (err) { console.error('[camera] cast failed:', err); }
    G.stats.casts = (G.stats.casts || 0) + 1;
    if (SPELL_BY_ID[id].aimed) armed = id;
    history.unshift({ id, via, at: now });
    history.length = Math.min(history.length, 5);
    lastCast = { id, at: now };
    pings.push({ ...aim, at: now, glyph: SPELL_BY_ID[id].glyph });
    coach?.flash(`${SPELL_BY_ID[id].glyph} ${SPELL_BY_ID[id].name}!`, 'ok', 1600);
    if (id === 'palm') G.hud?.showCause?.('🖐️ Your open palm spread calm across the land');
    else if (id === 'fist') G.hud?.showCause?.('✊ Your closed fist called down a strike');
    return true;
  }

  window.addEventListener('keydown', (e) => {
    if (!castsEnabled || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
    const spell = spellForKeyCode(e.code);
    if (!spell) return;
    if (cast(spell.id, 'key')) e.preventDefault();
  });
  for (const s of SPELLS) ui.spells[s.id]?.addEventListener('click', () => cast(s.id, 'tap'));

  // ---------- mood (#117: Joy continuous + Surprise debounced event) ----------
  function syncMood(G) {
    const scene = G?.scene;
    if (!scene?.onMood || !canCast()) return;
    if (!stream && scene !== moodScene) {
      moodScene = scene;
      moodSent = 'neutral';
      strongPending = false;
      surprisePending = false;
      return;
    }
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
    try {
      scene.onMood({ ...mood, reason, surpriseTriggered: wasSurprise });
    } catch (err) {
      console.error('[camera] mood failed:', err);
    }
    if (wasSurprise) {
      G.hud?.showCause?.('😮 Your surprised expression startled the sky!');
      advisorReact(G, 'surprise');
    } else if (reason === 'change' && changed && mood.dominant === 'joy') {
      G.hud?.showCause?.('😊 Your smile boosted morale (+joy)');
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
  function onGesture(r) {
    if (!stream || status !== 'on' || !castsEnabled) return;
    const now = performance.now();
    hand = r.hand || null;
    handAspect = r.aspect || 4 / 3;
    gesture = { name: r.name || 'None', score: r.score || 0 };
    const rawHands = Array.isArray(r.hands) && r.hands.length
      ? r.hands
      : (hand ? [{ landmarks: hand, name: gesture.name, score: gesture.score }] : []);

    const G = getG();
    const live = canCast() && !!G?.brain;

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
          onGrabMove({ dx: 0, dy: 0, started: true, active: true, aim: G.brain.aim, grab: multi.grab });
        }
      } else if (multi.grab.active) {
        if (grabbed) {
          G.scene?.onGrabMove?.({ x: G.brain.aim.x, y: G.brain.aim.y });
        } else if (onGrabMove) {
          onGrabMove({ dx: multi.grab.dx, dy: multi.grab.dy, started: false, active: true, aim: G.brain.aim, grab: multi.grab });
        }
      } else if (multi.grab.released || multi.grab.cancelled) {
        if (grabbed) {
          tryGrabEnd(G, now, !!multi.grab.cancelled);
        } else if (onGrabMove) {
          onGrabMove({ dx: 0, dy: 0, released: true, cancelled: !!multi.grab.cancelled, active: false, aim: G.brain.aim, grab: multi.grab });
        }
      }

      // 3. Palm (Calm) & Fist (Strike) gesture holds (#122) — suppressed while pinching or grabbing
      const suppressCast = multi.suppressCasts || finger.suppress;
      hold = suppressCast ? casts.update('None', 0, now) : casts.update(gesture.name, gesture.score, now);
      if (hold.cast) cast(hold.cast, 'gesture', now);
    } else {
      hold = casts.update('None', 0, now);
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
    if (!castsEnabled) return;
    const w = video.videoWidth || 320, h = video.videoHeight || 240;
    if (hands.width !== w || hands.height !== h) { hands.width = w; hands.height = h; }
    handsCtx.clearRect(0, 0, w, h);
    if (multi && (multi.left || multi.right)) {
      if (multi.left?.visible) {
        drawHand(handsCtx, multi.left.landmarks, w, h, {
          held: !!hold.held,
          pinching: !!multi.left.pinching,
          nearPinch: multi.left.ratio != null && multi.left.ratio < fingers.pinch.exit,
          side: 'left',
        });
      }
      if (multi.right?.visible) {
        drawHand(handsCtx, multi.right.landmarks, w, h, {
          held: !!hold.held,
          pinching: !!multi.right.pinching,
          nearPinch: multi.right.ratio != null && multi.right.ratio < fingers.pinch.exit,
          side: 'right',
        });
      }
    } else if (hand) {
      drawHand(handsCtx, hand, w, h, {
        held: !!hold.held,
        pinching: !!finger?.pinching,
        nearPinch: finger?.ratio != null && finger.ratio < fingers.pinch.exit,
      });
    }
  }

  // ---------- UI ----------
  function nowText(now) {
    const fmt = (s) => `${s.glyph} ${s.name}`;
    if (miss && now - miss.at < MISS_MS && (!lastCast || miss.at > lastCast.at)) {
      const s = SPELL_BY_ID[miss.id];
      return `${fmt(s)} ready in ${(casts.cooldownLeft(s.id, now) / 1000).toFixed(1)} s`;
    }
    if (multi?.bothPinched) {
      return `👐 Two-hand zoom (${Math.round((multi.zoomFactor || 1) * 100)}%) · unpinch to freeze`;
    }
    if (grabbed) {
      return `🤏 Holding ${grabbed.label} · move the cursor & release pinch to drop`;
    }
    const recent = lastCast && now - lastCast.at < 1400 ? SPELL_BY_ID[lastCast.id] : null;
    if (recent) return `Move: ${fmt(recent)}!`;
    const aimBy = aimSource(getG(), now).short;
    if (!(stream && engine)) return `Aim with the ${aimBy}. Press 1 (Grab), 2 (Calm), or 3 (Strike).`;
    const g = GESTURE_LABEL[gesture.name] || gesture.name;
    if (hold.held) {
      const s = SPELL_BY_ID[hold.held];
      const cd = casts.cooldownLeft(s.id, now);
      return cd > 0 ? `${g}: ${s.name} ready in ${(cd / 1000).toFixed(1)} s` : `${g}: hold for ${s.name}…`;
    }
    return hand ? `Hand: ${g} · ${aimBy} aims` : 'Show 1 hand to grab/calm/strike, or 2 hands to pinch-zoom.';
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
    if (ui.last) {
      const last = history[0];
      ui.last.textContent = last && SPELL_BY_ID[last.id] ? `${SPELL_BY_ID[last.id].glyph} ${SPELL_BY_ID[last.id].name} (${((now - last.at) / 1000).toFixed(1)}s ago)` : 'Ready';
    }

    if (!castsEnabled) return;
    const text = nowText(now);
    if (ui.now.textContent !== text) ui.now.textContent = text;
    for (const s of SPELLS) {
      const el = ui.spells[s.id];
      if (!el) continue;
      const cd = casts.cooldownFrac(s.id, now);
      const isHeld = hold.held === s.id || (s.id === 'pinch' && !!grabbed);
      el.style.setProperty('--cd', cd.toFixed(3));
      el.style.setProperty('--hold', (isHeld ? (grabbed ? 1 : hold.progress) : 0).toFixed(3));
      el.classList.toggle('is-cooling', cd > 0);
      el.classList.toggle('is-held', isHeld);
    }
  }

  function drawFx(now) {
    while (pings.length && now - pings[0].at > PING_MS) pings.shift();
    const G = getG();
    const tracking = !!(stream && (hand || multi?.left || multi?.right) && G?.brain && canCast());
    const holding = tracking && hold.held && hold.progress > 0;
    const carrying = !!grabbed && !!G?.brain;
    if (!pings.length && !holding && !carrying) {
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
    const ring = (x, y, r, frac, color, width = u) => {
      fxCtx.lineWidth = width;
      fxCtx.strokeStyle = color;
      fxCtx.beginPath(); fxCtx.arc(x, y, r, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * frac); fxCtx.stroke();
    };
    const glyphAt = (glyph, x, y, size) => {
      fxCtx.font = `${Math.round(size)}px "Segoe UI Emoji", "Apple Color Emoji", "Noto Color Emoji", sans-serif`;
      fxCtx.textAlign = 'center';
      fxCtx.fillText(glyph, x, y);
    };
    if ((holding || carrying) && G?.brain) {
      const r = reticleOnScreen(G);
      const x = r.x * cw, y = r.y * ch;
      if (carrying) {
        ring(x, y, 8 * u, 1, '#ff8a3d', 2 * u);
        glyphAt('🤏', x, y - 11 * u, 8 * u);
      }
      if (holding) {
        const s = SPELL_BY_ID[hold.held];
        if (s) {
          const cooling = casts.cooldownLeft(s.id, now) > 0;
          ring(x, y, 11 * u, 1, 'rgba(242,232,213,0.25)');
          ring(x, y, 11 * u, cooling ? 1 - casts.cooldownFrac(s.id, now) : hold.progress, cooling ? 'rgba(255,207,74,0.5)' : '#59d8c8', 2 * u);
          fxCtx.globalAlpha = cooling ? 0.5 : 1;
          glyphAt(s.glyph, x, y - 14 * u, 8 * u);
          fxCtx.globalAlpha = 1;
        }
      }
    }
    for (const p of pings) {
      const k = (now - p.at) / PING_MS;
      const x = screenX(p.x) * cw, y = screenY(p.y) * ch;
      fxCtx.globalAlpha = 1 - k;
      fxCtx.lineWidth = u;
      fxCtx.strokeStyle = '#ffcf4a';
      fxCtx.beginPath(); fxCtx.arc(x, y, (6 + 18 * k) * u, 0, Math.PI * 2); fxCtx.stroke();
      glyphAt(p.glyph, x, y - (12 + 10 * k) * u, 10 * u);
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
      engine.pump(video, now, { hands: castsEnabled, tracking: !!(hand || multi?.left || multi?.right) });
      if (now >= guardAt) guardFrames(now);
    }
    const G = getG();
    if (G) {
      G.mood = { ...(stream ? mood : neutralMood()), active: !!stream };
      if (grabbed && G.brain) {
        G.scene?.onGrabMove?.({ x: G.brain.aim.x, y: G.brain.aim.y });
      }
      syncMood(G);
    }
    if (!panel.hidden && now - uiAt > 60) { uiAt = now; updatePanel(now); }
    coach?.update(now, {
      live: !!stream && status === 'on',
      leftVisible: !!multi?.left?.visible,
      rightVisible: !!multi?.right?.visible,
      bothPinched: !!multi?.bothPinched,
      grabActive: !!grabbed || !!multi?.grab?.active,
      heldMove: hold.held,
      holdProgress: hold.progress,
      pinchStrength: finger?.pinching ? 1 : 0,
      statusText: nowText(now),
    });
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
      status, live: !!stream, castsEnabled, video,
      engine: engine ? engine.snapshot(now) : null,
      gesture: { ...gesture }, hold: { ...hold }, hand, handAspect, faceBox,
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
      armed,
      cooldowns: SPELLS.map((s) => ({ id: s.id, glyph: s.glyph, name: s.name, key: s.key, left: casts.cooldownLeft(s.id, now), frac: casts.cooldownFrac(s.id, now) })),
      history: history.slice(),
      mood: { ...mood },
    };
  }

  return {
    get status() { return status; },
    get live() { return !!stream; },
    get mood() { return mood; },
    get engine() { return engine; },
    get vision() { return engine?.vision || null; },
    get armed() { return armed; },
    get grabbed() { return grabbed; },
    start, stop, cast, togglePanel,
    casts, tracker, fingers, multiHand, coach, video,
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
    async loadModels() { const v = await loadVision(undefined, { hands: castsEnabled }); return { timings: v.timings, delegates: v.delegates }; },
  };
}
