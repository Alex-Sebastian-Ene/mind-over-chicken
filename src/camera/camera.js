// camera (#33): opt-in webcam input.
//   Hands: MediaPipe Gesture Recognizer → seven casts (spells.js). Pointing up aims with the index
//          fingertip; pinch-and-hold (thumb + index) aims precisely and letting go casts the armed
//          spell (fingers.js, #67). Enabled by CAMERA_CASTS_ENABLED (in scope per #55); gesture UI
//          navigation is deferred.
//   Face:  MediaPipe Face Landmarker blendshapes → smoothed mood → G.mood and scene.onMood().
//
// camera debug (#67): the models run in module Web Workers by default (engine.js), so inference
// never blocks the game loop; the #59 main-thread path with its budget is the fallback.
//
// PRIVACY: the camera starts only when the player clicks "Camera". Frames go from the <video>
// element to MediaPipe inside this tab (or its own workers). Nothing is recorded, stored or
// uploaded: no frame, landmark or score leaves the device. Turning the camera off stops every
// track. The only network traffic is the one-time download of the runtime and the two models.
//
// The player stays in control: casts and moods are an extra channel on top of the headset and
// keyboard. They call scene hooks and move the aim the way the mouse does; they never change
// the brain's calm, focus or heart-rate signals and never block another input.

import { W, H, clamp } from '../sim/util.js';
import { SPELLS, SPELL_BY_ID, GESTURE_LABEL, spellForKeyCode, CAMERA_CASTS_ENABLED } from './spells.js';
import { CastController } from './casts.js';
import { EMOTIONS, EmotionTracker, neutralMood } from './emotion.js';
import { loadVision } from './vision.js';
import { loadEngine, workerPreference } from './engine.js';
import { FingerController } from './fingers.js';
import { FrameMeter } from './meter.js';
import { drawHand } from './draw.js';

const ADVISOR_GAP = 14;      // seconds between the advisor's mood remarks
const PING_MS = 900;         // how long a cast marker stays on the map
const MISS_MS = 1400;        // how long "ready in …" stays up after a pinch on a cooling spell
// GPU guard (#67): GPU inference shares the browser's GPU process with the game's WebGL. If the game
// runs below this fraction of its frame rate from before the camera started, the worker models
// move to the CPU.
const GPU_GUARD = { ratio: 0.85, minFps: 30, graceMs: 4000, everyMs: 1000 };

const MOOD_LINES = {
  joy: { mood: 'happy', lines: ['You are smiling, my lord. The people have started dancing. Badly, but with feeling.', 'A divine smile! Everyone down here feels lucky today.'] },
  anger: { mood: 'nervous', lines: ['That frown, my lord. The people can feel it. So can the goats.', 'Divine scowl detected. Tempers are rising down here.'] },
  surprise: { mood: 'panic', lines: ['You look startled! The sky took it personally.', 'Such a face! The heavens flashed in sympathy.'] },
  sadness: { mood: 'nervous', lines: ['Why the long face, my lord? The sun is hiding out of sympathy.', 'You seem sad. The whole world has gone a shade darker.'] },
};

const TEMPLATE = `
  <header class="eeg-head">
    <h2>Camera &amp; Hand Casts</h2>
    <button type="button" class="tool" data-cam-close title="Close Camera sidebar (\`)">Close <kbd>\`</kbd></button>
  </header>

  <section class="eeg-sec">
    <div class="eeg-status-row">
      <span class="eeg-dot" data-cam-dot data-ok="0"></span>
      <strong data-cam-state>Camera off</strong>
      <span class="eeg-qual" data-cam-mode>Idle</span>
    </div>
    <p class="eeg-msg" data-cam-msg role="status">Opt-in webcam: cast spells with hand gestures and set world mood with your face. Frames never leave this device.</p>
    <div class="eeg-url-row">
      <button type="button" class="btn btn-primary btn-small" data-cam-start>Start Camera</button>
      <button type="button" class="btn btn-small" data-cam-stop disabled>Stop Camera</button>
    </div>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Live Vision &amp; Hand Landmarks</h3>
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
          <span>Gesture: <b data-cam-gesture>--</b></span>
        </div>
        <div class="eeg-imu-mini">
          <span>Pinch: <b data-cam-pinch>--</b></span>
        </div>
      </div>
    </div>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Hand Spells (Keys 1–7)</h3>
      <span class="eeg-qual" data-cam-last>Ready</span>
    </div>
    <p class="eeg-msg" data-cam-now>Make a gesture, or press 1–7.</p>
    <div class="cam-spells" role="group" aria-label="Casts, keys 1 to 7">
      ${SPELLS.map((s) => `<button type="button" class="cam-spell" data-spell="${s.id}" aria-label="${s.name} (key ${s.key})" title="${s.name} · key ${s.key}. ${s.hint}">
        <svg class="cam-ring" viewBox="0 0 36 36" aria-hidden="true"><circle class="cam-ring-track" cx="18" cy="18" r="16"/><circle class="cam-ring-hold" cx="18" cy="18" r="16" pathLength="100"/><circle class="cam-ring-cd" cx="18" cy="18" r="16" pathLength="100"/></svg>
        <span class="cam-glyph" aria-hidden="true">${s.glyph}</span><kbd>${s.key}</kbd>
      </button>`).join('')}
    </div>
    <div class="eeg-imu-mini">
      <span>Hand Range: <b data-cam-range>learning…</b></span>
    </div>
  </section>

  <section class="eeg-sec" aria-label="Mood read from your face">
    <div class="eeg-sec-head">
      <h3>Facial Mood &amp; Expression</h3>
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
  const fallback = castsEnabled ? ' Keys 1–7 still cast every spell.' : '';
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

// mapX / screenX convert the reticle's x between the visible screen and the (panned, wrapping)
// planet surface, as the mouse does; both are identity when main.js doesn't pass them.
export function initCamera({ getG, canCast = () => true, castsEnabled = CAMERA_CASTS_ENABLED, mapX = (x) => x, screenX = (x) => x } = {}) {
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
    $('.cam-now').textContent = 'Face mood is active. Hand casts are deferred.';
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

  // Cast markers, the hand cursor and hold rings, drawn over the game canvas (under the era
  // banner) so they show in every era without touching the renderer.
  const fx = document.createElement('canvas');
  fx.className = 'cam-fx';
  fx.setAttribute('aria-hidden', 'true');
  screen?.insertAdjacentElement('afterend', fx);
  const fxCtx = fx.getContext('2d');
  let fxDirty = false;

  const casts = new CastController();
  const tracker = new EmotionTracker();
  const fingers = new FingerController();
  const castFallback = castsEnabled ? ' Keys 1–7 still cast every spell.' : '';
  let status = 'off';          // off | starting | on | error
  let stream = null;
  let engine = null;           // engine.js: worker or main-thread inference, kept across camera toggles
  let enginePending = null;
  let prefer = workerPreference();
  let run = 0;                 // bumps on every start/stop so stale async work can tell
  let lastTick = 0;
  let lastFaceAt = 0;
  let gesture = { name: 'None', score: 0 };
  let hold = { held: null, progress: 0, aiming: false };
  let hand = null, handAspect = 4 / 3, faceBox = null;
  let finger = null;           // last FingerController output
  let armed = 'fireball';      // what letting go of a pinch casts: the last aimed spell
  let mood = neutralMood();
  let moodScene = null, moodSent = null, strongPending = false;
  let advisorAt = -Infinity;
  let lastCast = null;         // { id, at } for the toast and marker
  let miss = null;             // { id, at } a pinch released while that spell was cooling down
  const history = [];          // last casts for the debug tab: { id, via, at }
  let landmarkI = 0, landmarkScene = null;
  let uiAt = 0;
  const pings = [];            // { x, y, at, glyph } in map coordinates
  const frames = new FrameMeter();
  let fpsBefore = 0;           // game frame rate when the player turned the camera on
  let guardAt = Infinity;

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
    else if (live && next === 'on') setMsg('Tracking hands and facial mood locally.');
  }
  function setMsg(msg) {
    if (!ui.msg) return;
    ui.msg.textContent = msg || 'Opt-in webcam: cast spells with hand gestures and set world mood with your face.';
  }

  function togglePanel(force) {
    const open = force !== undefined ? force : panel.hidden;
    panel.hidden = !open;
    btn.setAttribute('aria-pressed', String(open));
    if (open) {
      const eegP = document.getElementById('eegPanel');
      const eegB = document.getElementById('eegBtn');
      if (eegP && !eegP.hidden) {
        eegP.hidden = true;
        eegB?.setAttribute('aria-pressed', 'false');
      }
      const modP = document.getElementById('modelPanel');
      const modB = document.getElementById('modelBtn');
      if (modP && !modP.hidden) {
        modP.hidden = true;
        modB?.setAttribute('aria-pressed', 'false');
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

  // A worker died while running: carry on with the main-thread path.
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
    if (id !== run) { stopTracks(s); return; } // turned off while the prompt was open
    stream = s;
    video.srcObject = stream;
    try { await video.play(); } catch { /* autoplay of a muted inline video is allowed; ignore */ }
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
    stopTracks(stream);
    stream = null;
    video.srcObject = null;
    casts.reset();
    tracker.reset();
    fingers.reset();
    hand = null;
    finger = null;
    faceBox = null;
    gesture = { name: 'None', score: 0 };
    hold = { held: null, progress: 0, aiming: false };
    mood = neutralMood();
    strongPending = false;
    handsCtx.clearRect(0, 0, hands.width, hands.height);
  }

  function stop() {
    run++;
    teardown();
    setStatus('off', 'Camera stopped. Keys 1–7 still cast every spell.');
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

  // ---------- aiming ----------
  // In map coordinates (key 1's landmarks).
  function setAim(m) {
    const G = getG();
    if (!G?.brain) return;
    const b = G.brain.aim;
    b.x = clamp(m.x); b.y = clamp(m.y);
    // Like the mouse: the emulated headset's head tilt follows, so the next frame doesn't pull it back.
    if (G.source?.kind === 'sim') G.source.setAim(b.x, b.y);
  }
  // In screen coordinates (the finger), converted to the panned surface like a mouse position.
  const aimAtScreen = (s) => setAim({ x: mapX(clamp(s.x)), y: s.y });
  const reticleOnScreen = (G) => ({ x: clamp(screenX(G.brain.aim.x)), y: G.brain.aim.y });

  function landmarks(G) {
    const sc = G.scene, s = G.world?.sites || {};
    const pts = [];
    if (sc.id === 'stone') {
      if (sc.fire) pts.push(sc.fire);
      for (const m of sc.mammoths || []) if (m.alive) pts.push({ x: m.x, y: m.y - 2 });
    } else if (sc.id === 'industry') pts.push(s.exchange, s.factory, s.picket);
    else if (sc.id === 'space') pts.push(s.pad, s.hangar, s.crowd);
    else pts.push(s.village, s.cave);
    return pts.filter(Boolean);
  }

  function nextLandmark(G) {
    const pts = landmarks(G);
    if (!pts.length) return { ...G.brain.aim };
    if (landmarkScene !== G.scene) { landmarkScene = G.scene; landmarkI = 0; }
    const p = pts[landmarkI++ % pts.length];
    return { x: clamp(p.x / (W - 1)), y: clamp(p.y / (H - 1)) };
  }

  // ---------- casting ----------
  // via: 'gesture' (already debounced and cooled down by the controller), 'pinch', 'key' or 'tap'.
  function cast(id, via, now = performance.now()) {
    if (!castsEnabled) return false;
    const G = getG();
    const scene = G?.scene;
    if (!scene || !SPELL_BY_ID[id] || !canCast()) return false;
    if (via !== 'gesture' && !casts.trigger(id, now)) return false;
    if (id === 'point' && via !== 'gesture') setAim(nextLandmark(G));
    const aim = { x: G.brain.aim.x, y: G.brain.aim.y };
    try {
      // The fist reuses the existing lightning wherever an era has it.
      if (id === 'fireball' && scene.onBlink) { G.hud?.flashBlink?.(); scene.onBlink(aim); }
      else scene.onCast?.(id, aim);
    } catch (err) { console.error('[camera] cast failed:', err); }
    G.stats.casts = (G.stats.casts || 0) + 1;
    if (SPELL_BY_ID[id].aimed) armed = id;
    history.unshift({ id, via, at: now });
    history.length = Math.min(history.length, 5);
    // Pointing by hand already shows the hand cursor; every other cast gets a toast and a marker.
    if (!(id === 'point' && via === 'gesture')) {
      lastCast = { id, at: now };
      pings.push({ ...aim, at: now, glyph: SPELL_BY_ID[id].glyph });
    }
    return true;
  }

  // Letting go of a pinch: the armed spell lands on the precisely placed reticle.
  function castArmed(now) {
    if (!cast(armed, 'pinch', now)) miss = { id: armed, at: now };
  }

  window.addEventListener('keydown', (e) => {
    if (!castsEnabled || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
    const spell = spellForKeyCode(e.code);
    if (!spell) return;
    if (cast(spell.id, 'key')) e.preventDefault();
  });
  for (const s of SPELLS) ui.spells[s.id].addEventListener('click', () => cast(s.id, 'tap'));

  // ---------- mood ----------
  // Level-triggered: whenever the scene or the dominant mood differs from what the scene last
  // heard, tell it. So a scene that was paused (banner, retry screen) or freshly started still
  // ends up in the right mood, and turning the camera off returns it to neutral.
  function syncMood(G) {
    const scene = G?.scene;
    if (!scene?.onMood || !canCast()) return;
    // Camera off and nothing to undo: a fresh scene is already neutral, so leave it alone.
    if (!stream && scene !== moodScene) { moodScene = scene; moodSent = 'neutral'; strongPending = false; return; }
    let reason = null;
    if (scene !== moodScene || mood.dominant !== moodSent) reason = 'change';
    else if (strongPending) reason = 'strong';
    if (!reason) return;
    const changed = mood.dominant !== moodSent;
    moodScene = scene; moodSent = mood.dominant; strongPending = false;
    try { scene.onMood({ ...mood, reason }); } catch (err) { console.error('[camera] mood failed:', err); }
    if (reason === 'change' && changed && mood.dominant !== 'neutral') advisorReact(G);
  }

  function advisorReact(G) {
    const t = performance.now() / 1000;
    const r = MOOD_LINES[mood.dominant];
    if (!r || !G.advisor || t - advisorAt < ADVISOR_GAP) return;
    const line = r.lines[Math.floor(Math.random() * r.lines.length)];
    if (G.advisor.say(`mood-${mood.dominant}`, line, { mood: r.mood, priority: 1, cooldown: 40 })) advisorAt = t;
  }

  // ---------- model results ----------
  // r: { hand: 21 landmarks | null, name, score, aspect }
  function onGesture(r) {
    if (!stream || status !== 'on' || !castsEnabled) return;
    const now = performance.now();
    hand = r.hand || null;
    handAspect = r.aspect || 4 / 3;
    gesture = { name: r.name || 'None', score: r.score || 0 };
    const G = getG();
    const live = canCast() && !!G?.brain;
    finger = fingers.update({ hand, gesture, aim: live ? reticleOnScreen(G) : { x: 0.5, y: 0.55 }, now, aspect: handAspect });
    if (live) {
      // While pinching (or about to), a fist or palm read from that hand shape is not a cast.
      hold = finger.suppress ? casts.update('None', 0, now) : casts.update(gesture.name, gesture.score, now);
      if (finger.aim) aimAtScreen(finger.aim);
      if (finger.cast) castArmed(now);
      else if (hold.cast) cast(hold.cast, 'gesture', now);
    } else hold = casts.update('None', 0, now);
    drawHands();
  }

  // r: { blendshapes: categories or {name: score} | null, box }
  function onFace(r) {
    if (!stream || status !== 'on') return;
    const at = r.at ?? performance.now();
    const dt = clamp((at - lastFaceAt) / 1000, 0, 0.5);
    lastFaceAt = at;
    const out = tracker.update(r.blendshapes || null, dt);
    mood = out.mood;
    faceBox = r.box || null;
    if (out.event === 'strong') strongPending = true;
  }

  const handState = () => ({ held: !!hold.held, aiming: finger?.mode === 'point', pinching: !!finger?.pinching, nearPinch: finger?.ratio != null && finger.ratio < fingers.pinch.exit });

  function drawHands() {
    if (!castsEnabled) return;
    const w = video.videoWidth || 320, h = video.videoHeight || 240;
    if (hands.width !== w || hands.height !== h) { hands.width = w; hands.height = h; }
    handsCtx.clearRect(0, 0, w, h);
    drawHand(handsCtx, hand, w, h, handState());
  }

  // ---------- UI ----------
  function nowText(now) {
    const fmt = (s) => `${s.glyph} ${s.name}`;
    // A pinch let go on a cooling spell says when it's ready, even over the last cast's toast.
    if (miss && now - miss.at < MISS_MS && (!lastCast || miss.at > lastCast.at)) {
      const s = SPELL_BY_ID[miss.id];
      return `${fmt(s)} ready in ${(casts.cooldownLeft(s.id, now) / 1000).toFixed(1)} s`;
    }
    const recent = lastCast && now - lastCast.at < 1400 ? SPELL_BY_ID[lastCast.id] : null;
    if (recent) return `Cast: ${fmt(recent)}!`;
    if (!(stream && engine)) return 'Press 1–7 or tap a spell to cast.';
    const g = GESTURE_LABEL[gesture.name] || gesture.name;
    if (finger?.pinching) {
      const s = SPELL_BY_ID[armed];
      const cd = casts.cooldownLeft(armed, now);
      return cd > 0 ? `Pinch: aiming · ${s.name} ready in ${(cd / 1000).toFixed(1)} s` : `Pinch: aiming · let go for ${fmt(s)}`;
    }
    if (hold.held) {
      const s = SPELL_BY_ID[hold.held];
      const cd = casts.cooldownLeft(s.id, now);
      return cd > 0 ? `${g}: ${s.name} ready in ${(cd / 1000).toFixed(1)} s` : finger?.mode === 'point' ? `${g}: aiming` : `${g}: hold for ${s.name}…`;
    }
    if (finger?.mode === 'point') return `${g}: aiming`;
    return hand ? `Hand: ${g}` : 'Show a hand. Point to aim, pinch to aim precisely.';
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

    if (ui.gesture) {
      ui.gesture.textContent = !stream ? 'camera off' : !hand ? 'no hand' : `${GESTURE_LABEL[gesture.name] || gesture.name} (${Math.round(gesture.score * 100)}%)${finger?.mode === 'point' ? ' · aiming' : ''}`;
    }
    if (ui.pinch) {
      const armedSpell = SPELL_BY_ID[armed || 'fireball'];
      ui.pinch.textContent = finger?.ratio == null
        ? `casts ${armedSpell.glyph} ${armedSpell.name}`
        : finger.pinching
          ? `HELD (${finger.ratio.toFixed(2)}) -> ${armedSpell.glyph}`
          : `open (${finger.ratio.toFixed(2)}) · ${armedSpell.glyph}`;
    }
    if (ui.range) {
      const rg = fingers.range.snapshot();
      ui.range.textContent = `${rg.calibrated ? 'learned' : `learning ${Math.round((rg.progress || 0) * 100)}%`} · X:${rg.x[0].toFixed(2)}–${rg.x[1].toFixed(2)} Y:${rg.y[0].toFixed(2)}–${rg.y[1].toFixed(2)}`;
    }
    if (ui.last) {
      const last = history[0];
      ui.last.textContent = last ? `${SPELL_BY_ID[last.id].glyph} ${SPELL_BY_ID[last.id].name} (${((now - last.at) / 1000).toFixed(1)}s ago)` : 'Ready';
    }

    if (!castsEnabled) return;
    const text = nowText(now);
    if (ui.now.textContent !== text) ui.now.textContent = text;
    for (const s of SPELLS) {
      const el = ui.spells[s.id];
      const cd = casts.cooldownFrac(s.id, now);
      el.style.setProperty('--cd', cd.toFixed(3));
      el.style.setProperty('--hold', (hold.held === s.id ? hold.progress : 0).toFixed(3));
      el.classList.toggle('is-cooling', cd > 0);
      el.classList.toggle('is-held', hold.held === s.id);
    }
  }

  // The finger cursor, gesture-hold and pinch rings at the reticle, and cast markers.
  function drawFx(now) {
    while (pings.length && now - pings[0].at > PING_MS) pings.shift();
    const G = getG();
    const tracking = !!(stream && hand && G?.brain && canCast());
    const mode = tracking ? finger?.mode : null;
    const cursor = mode === 'point' || mode === 'pinch';
    const holding = tracking && hold.held && hold.held !== 'point' && hold.progress > 0;
    if (!pings.length && !cursor && !holding) {
      if (fxDirty) { fxCtx.clearRect(0, 0, fx.width, fx.height); fxDirty = false; }
      return;
    }
    const dpr = window.devicePixelRatio || 1;
    const cw = Math.round(fx.clientWidth * dpr), ch = Math.round(fx.clientHeight * dpr);
    if (!cw || !ch) return;
    if (fx.width !== cw || fx.height !== ch) { fx.width = cw; fx.height = ch; }
    fxCtx.clearRect(0, 0, cw, ch);
    fxDirty = true;
    const u = Math.max(1, cw / 320);   // one world pixel
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
    if (cursor || holding) {
      const r = reticleOnScreen(G);
      const x = r.x * cw, y = r.y * ch;
      if (mode === 'pinch') {
        // Precise aim: a small, tight cursor, the pinch's hold ring and the armed spell.
        ring(x, y, 5 * u, 1, 'rgba(242,232,213,0.35)');
        ring(x, y, 5 * u, finger.progress, '#ff8a3d', 1.5 * u);
        fxCtx.fillStyle = '#ff8a3d';
        fxCtx.fillRect(x - u / 2, y - u / 2, u, u);
        fxCtx.globalAlpha = casts.cooldownLeft(armed, now) > 0 ? 0.45 : 1;
        glyphAt(SPELL_BY_ID[armed].glyph, x + 9 * u, y - 6 * u, 7 * u);
        fxCtx.globalAlpha = 1;
      } else if (cursor) {
        fxCtx.lineWidth = u;
        fxCtx.strokeStyle = 'rgba(89,216,200,0.95)';
        fxCtx.beginPath(); fxCtx.arc(x, y, 7 * u, 0, Math.PI * 2); fxCtx.stroke();
        fxCtx.beginPath(); fxCtx.moveTo(x - 10 * u, y); fxCtx.lineTo(x - 4 * u, y); fxCtx.moveTo(x + 4 * u, y); fxCtx.lineTo(x + 10 * u, y);
        fxCtx.moveTo(x, y - 10 * u); fxCtx.lineTo(x, y - 4 * u); fxCtx.moveTo(x, y + 4 * u); fxCtx.lineTo(x, y + 10 * u); fxCtx.stroke();
      }
      if (holding) {
        // Gesture hold: fills over ~300 ms, then the cast fires. A cooling spell shows a dim ring.
        const s = SPELL_BY_ID[hold.held];
        const cooling = casts.cooldownLeft(s.id, now) > 0;
        ring(x, y, 11 * u, 1, 'rgba(242,232,213,0.25)');
        ring(x, y, 11 * u, cooling ? 1 - casts.cooldownFrac(s.id, now) : hold.progress, cooling ? 'rgba(255,207,74,0.5)' : '#59d8c8', 2 * u);
        fxCtx.globalAlpha = cooling ? 0.5 : 1;
        glyphAt(s.glyph, x, y - 14 * u, 8 * u);
        fxCtx.globalAlpha = 1;
      }
    }
    for (const p of pings) {
      const k = (now - p.at) / PING_MS;
      const x = screenX(p.x) * cw, y = p.y * ch;
      fxCtx.globalAlpha = 1 - k;
      fxCtx.lineWidth = u;
      fxCtx.strokeStyle = '#ffcf4a';
      fxCtx.beginPath(); fxCtx.arc(x, y, (6 + 18 * k) * u, 0, Math.PI * 2); fxCtx.stroke();
      glyphAt(p.glyph, x, y - (12 + 10 * k) * u, 10 * u);
    }
    fxCtx.globalAlpha = 1;
  }

  // ---------- loop ----------
  // rAF, with a timer fallback for embedded views that throttle it (same as the game loop).
  function tick() {
    const now = performance.now();
    if (now - lastTick < 8) return;
    lastTick = now;
    frames.frame(now);
    if (stream && engine?.alive && status === 'on' && video.readyState >= 2) {
      engine.pump(video, now, { hands: castsEnabled, tracking: !!hand });
      if (now >= guardAt) guardFrames(now);
    }
    const G = getG();
    if (G) {
      G.mood = { ...(stream ? mood : neutralMood()), active: !!stream };
      syncMood(G);
    }
    if (!panel.hidden && now - uiAt > 60) { uiAt = now; updatePanel(now); }
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

  // Everything the debug tab shows, as plain data.
  function debugState(now = performance.now()) {
    return {
      status, live: !!stream, castsEnabled, video,
      engine: engine ? engine.snapshot(now) : null,
      gesture: { ...gesture }, hold: { ...hold }, hand, handAspect, faceBox,
      finger: finger && { mode: finger.mode, ratio: finger.ratio, reach: finger.reach, pinching: finger.pinching, progress: finger.progress, suppress: finger.suppress, tip: finger.tip },
      pinch: { enter: fingers.pinch.enter, exit: fingers.pinch.exit },
      range: fingers.range.snapshot(),
      armed,
      cooldowns: SPELLS.map((s) => ({ id: s.id, glyph: s.glyph, name: s.name, key: s.key, left: casts.cooldownLeft(s.id, now), frac: casts.cooldownFrac(s.id, now) })),
      history: history.slice(),
      mood: { ...mood },
    };
  }

  // Debug and test handle (window.__moc.camera).
  return {
    get status() { return status; },
    get mood() { return mood; },
    get engine() { return engine; },
    get vision() { return engine?.vision || null; },
    get armed() { return armed; },
    start, stop, cast, togglePanel,
    casts, tracker, fingers, video,
    debugState,
    // Feed model results in directly, as the engine does (stubbed outputs for tests).
    simulate: { gesture: onGesture, face: onFace },
    // Where inference runs: 0 main thread, 1 one worker, 2 a worker per model (default). For comparing.
    async setInferenceMode(mode) {
      prefer = mode;
      const old = engine;
      engine = null;
      old?.close();
      if (stream && status === 'on') { await ensureEngine(() => {}); engine.resetStats(); guardAt = performance.now() + GPU_GUARD.graceMs; }
      return engine?.snapshot().mode || null;
    },
    // Download and initialise the main-thread models without a camera, e.g. to time them.
    async loadModels() { const v = await loadVision(undefined, { hands: castsEnabled }); return { timings: v.timings, delegates: v.delegates }; },
  };
}
