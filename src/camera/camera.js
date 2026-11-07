// camera (#33): opt-in webcam input.
//   Hands: MediaPipe Gesture Recognizer → seven casts (spells.js). Pointing up aims.
//          Deferred milestone: gated off by CAMERA_CASTS_ENABLED until gesture UI is in scope.
//   Face:  MediaPipe Face Landmarker blendshapes → smoothed mood → G.mood and scene.onMood().
//
// PRIVACY: the camera starts only when the player clicks "Camera". Frames go from the <video>
// element straight into MediaPipe, which runs inside this tab. Nothing is recorded, stored or
// uploaded: no frame, landmark or score leaves the device. Turning the camera off stops every
// track. The only network traffic is the one-time download of the runtime and the two models.
//
// The player stays in control: casts and moods are an extra channel on top of the headset and
// keyboard. They call scene hooks and move the aim the way the mouse does; they never change
// the brain's calm, focus or heart-rate signals and never block another input.

import { W, H, clamp, smooth } from '../sim/util.js';
import { SPELLS, SPELL_BY_ID, GESTURE_LABEL, spellForKeyCode, CAMERA_CASTS_ENABLED } from './spells.js';
import { CastController } from './casts.js';
import { EMOTIONS, EmotionTracker, neutralMood } from './emotion.js';
import { loadVision } from './vision.js';

const HAND_LINKS = [[0, 1], [1, 2], [2, 3], [3, 4], [0, 5], [5, 6], [6, 7], [7, 8], [5, 9], [9, 10], [10, 11], [11, 12],
  [9, 13], [13, 14], [14, 15], [15, 16], [13, 17], [17, 18], [18, 19], [19, 20], [0, 17]];
// Inference runs on the main thread, so it is budgeted: at most one model per game frame, and
// each model waits a multiple of its own measured cost before running again (~8–15 Hz for hands,
// ≤5 Hz for the face). That keeps the game near full speed while the camera is on.
const HAND_MS = { tracking: 66, searching: 125, costX: 3 };
const FACE_MS = { min: 200, costX: 6 };
const ADVISOR_GAP = 14;      // seconds between the advisor's mood remarks
const PING_MS = 900;         // how long a cast marker stays on the map

const MOOD_LINES = {
  joy: { mood: 'happy', lines: ['You are smiling, my lord. The people have started dancing. Badly, but with feeling.', 'A divine smile! Everyone down here feels lucky today.'] },
  anger: { mood: 'nervous', lines: ['That frown, my lord. The people can feel it. So can the goats.', 'Divine scowl detected. Tempers are rising down here.'] },
  surprise: { mood: 'panic', lines: ['You look startled! The sky took it personally.', 'Such a face! The heavens flashed in sympathy.'] },
  sadness: { mood: 'nervous', lines: ['Why the long face, my lord? The sun is hiding out of sympathy.', 'You seem sad. The whole world has gone a shade darker.'] },
};

const TEMPLATE = `
  <header class="cam-head">
    <span class="cam-live" data-cam-live><i aria-hidden="true"></i><b data-cam-state>Camera off</b></span>
    <button type="button" class="tool cam-close" data-cam-close title="Turn the camera off and close this panel">Close</button>
  </header>
  <p class="cam-msg" data-cam-msg role="status"></p>
  <div class="cam-body">
    <div class="cam-view">
      <video class="cam-video" playsinline muted></video>
      <canvas class="cam-hands" width="320" height="240" aria-hidden="true"></canvas>
    </div>
    <div class="cam-mood" aria-label="Mood read from your face">
      <p class="cam-mood-head">Mood <b data-cam-dominant>neutral</b></p>
      ${EMOTIONS.map((k) => `<div class="cam-mood-row" data-emotion="${k}"><span>${k}</span><span class="cam-bar"><span></span></span></div>`).join('')}
    </div>
  </div>
  <p class="cam-now" data-cam-now>Make a gesture, or press 1–7.</p>
  <div class="cam-spells" role="group" aria-label="Casts, keys 1 to 7">
    ${SPELLS.map((s) => `<button type="button" class="cam-spell" data-spell="${s.id}" aria-label="${s.name} (key ${s.key})" title="${s.name} · key ${s.key}. ${s.hint}">
      <svg class="cam-ring" viewBox="0 0 36 36" aria-hidden="true"><circle class="cam-ring-track" cx="18" cy="18" r="16"/><circle class="cam-ring-hold" cx="18" cy="18" r="16" pathLength="100"/><circle class="cam-ring-cd" cx="18" cy="18" r="16" pathLength="100"/></svg>
      <span class="cam-glyph" aria-hidden="true">${s.glyph}</span><kbd>${s.key}</kbd>
    </button>`).join('')}
  </div>
  <p class="cam-privacy">Frames never leave this device. Nothing is uploaded.</p>`;

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

export function initCamera({ getG, canCast = () => true, castsEnabled = CAMERA_CASTS_ENABLED } = {}) {
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
    state: $('[data-cam-state]'), msg: $('[data-cam-msg]'), now: $('[data-cam-now]'), dominant: $('[data-cam-dominant]'),
    bars: Object.fromEntries(EMOTIONS.map((k) => [k, $(`[data-emotion="${k}"] .cam-bar > span`)])),
    spells: Object.fromEntries(SPELLS.map((s) => [s.id, panel.querySelector(`[data-spell="${s.id}"]`)])),
  };

  // Cast markers and the hand cursor, drawn over the game canvas (under the era banner) so
  // they show in every era without touching the renderer.
  const fx = document.createElement('canvas');
  fx.className = 'cam-fx';
  fx.setAttribute('aria-hidden', 'true');
  screen?.insertAdjacentElement('afterend', fx);
  const fxCtx = fx.getContext('2d');
  let fxDirty = false;

  const casts = new CastController();
  const tracker = new EmotionTracker();
  const castFallback = castsEnabled ? ' Keys 1–7 still cast every spell.' : '';
  let status = 'off';          // off | starting | on | error
  let stream = null;
  let vision = null;
  let run = 0;                 // bumps on every start/stop so stale async work can tell
  let lastTick = 0;
  const seen = { gesture: -1, face: -1 };          // video.currentTime each model last saw
  const nextAt = { gesture: 0, face: 0 };           // earliest time each model may run again
  const lastAt = { gesture: 0, face: 0 };
  const cost = { gesture: 20, face: 15 };           // smoothed ms per call
  let gesture = { name: 'None', score: 0 };
  let hold = { held: null, progress: 0, aiming: false };
  let hand = null;
  let mood = neutralMood();
  let moodScene = null, moodSent = null, strongPending = false;
  let advisorAt = -Infinity;
  let lastCast = null;         // { id, at }
  let landmarkI = 0, landmarkScene = null;
  let uiAt = 0;
  const rebuilding = { gesture: false, face: false };
  const taskErrors = { gesture: 0, face: 0 };
  const pings = [];            // { x, y, at, glyph }

  // ---------- state & messages ----------
  function setStatus(next, msg = '') {
    status = next;
    const live = !!stream;
    btn.dataset.state = next;
    btn.dataset.live = live ? '1' : '0';
    btn.setAttribute('aria-pressed', String(next === 'on' || next === 'starting'));
    btn.textContent = live ? 'Camera on' : next === 'starting' ? 'Camera…' : 'Camera';
    panel.dataset.state = next;
    panel.dataset.live = live ? '1' : '0';
    ui.state.textContent = live ? (next === 'on' ? 'Camera on · tracking' : 'Camera on · loading') : next === 'error' ? 'Camera unavailable' : next === 'starting' ? 'Starting camera…' : 'Camera off';
    setMsg(msg);
  }
  function setMsg(msg) { ui.msg.textContent = msg; ui.msg.hidden = !msg; }

  // ---------- start / stop ----------
  async function start() {
    if (status === 'starting' || status === 'on') return;
    const id = ++run;
    panel.hidden = false;
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
      vision = await loadVision((m) => { if (id === run) setMsg(m); }, { hands: castsEnabled });
    } catch (err) {
      if (id !== run) return;
      teardown();
      setStatus('error', `Couldn't load the camera models (${err?.message || err}).`);
      return;
    }
    if (id !== run) return;
    seen.gesture = seen.face = -1;
    lastAt.gesture = lastAt.face = performance.now();
    setStatus('on');
  }

  function teardown() {
    stopTracks(stream);
    stream = null;
    video.srcObject = null;
    casts.reset();
    tracker.reset();
    hand = null;
    gesture = { name: 'None', score: 0 };
    hold = { held: null, progress: 0, aiming: false };
    mood = neutralMood();
    strongPending = false;
    handsCtx.clearRect(0, 0, hands.width, hands.height);
  }

  function stop() {
    run++;
    teardown();
    setStatus('off');
    panel.hidden = true;
  }

  btn.addEventListener('click', () => { if (status === 'on' || status === 'starting') stop(); else start(); });
  $('[data-cam-close]').addEventListener('click', stop);
  window.addEventListener('pagehide', () => { if (stream || status === 'starting') stop(); });

  // ---------- casting ----------
  function aimAt(a, rate, dt) {
    const G = getG();
    if (!G?.brain) return;
    const b = G.brain.aim;
    b.x = rate ? smooth(b.x, a.x, rate, dt) : a.x;
    b.y = rate ? smooth(b.y, a.y, rate, dt) : a.y;
    // Like the mouse: the emulated headset's head tilt follows, so the next frame doesn't pull it back.
    if (G.source?.kind === 'sim') G.source.setAim(b.x, b.y);
  }

  // Index fingertip (landmark 8). x is mirrored so the reticle moves the way your hand does.
  // The middle of the camera frame maps to the whole map: no need to reach the edges.
  function fingerAim(tip) {
    return { x: clamp((1 - tip.x - 0.15) / 0.7), y: clamp((tip.y - 0.12) / 0.6) };
  }

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

  // via: 'gesture' (already debounced and cooled down by the controller), 'key' or 'tap'.
  function cast(id, via, now = performance.now()) {
    if (!castsEnabled) return false;
    const G = getG();
    const scene = G?.scene;
    if (!scene || !SPELL_BY_ID[id] || !canCast()) return false;
    if (via !== 'gesture' && !casts.trigger(id, now)) return false;
    if (id === 'point' && via !== 'gesture') aimAt(nextLandmark(G));
    const aim = { x: G.brain.aim.x, y: G.brain.aim.y };
    try {
      // The fist reuses the existing lightning wherever an era has it.
      if (id === 'fireball' && scene.onBlink) { G.hud?.flashBlink?.(); scene.onBlink(aim); }
      else scene.onCast?.(id, aim);
    } catch (err) { console.error('[camera] cast failed:', err); }
    G.stats.casts = (G.stats.casts || 0) + 1;
    // Pointing by hand already shows the hand cursor; every other cast gets a toast and a marker.
    if (!(id === 'point' && via === 'gesture')) {
      lastCast = { id, at: now };
      pings.push({ ...aim, at: now, glyph: SPELL_BY_ID[id].glyph });
    }
    return true;
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

  // ---------- per video frame ----------
  function taskFailed(name, err) {
    taskErrors[name]++;
    if (vision.delegates[name] === 'GPU' && !rebuilding[name]) {
      rebuilding[name] = true;
      console.warn(`[camera] ${name} model failed on the GPU, switching to CPU:`, err?.message || err);
      vision.rebuildOnCpu(name).catch((e) => console.warn(`[camera] ${name} CPU fallback failed:`, e?.message || e)).finally(() => { rebuilding[name] = false; });
    } else if (taskErrors[name] === 30) {
      console.warn(`[camera] ${name} model keeps failing:`, err?.message || err);
      setMsg(`${name === 'gesture' ? 'Hand' : 'Face'} tracking is struggling on this device.${castFallback}`);
    }
  }

  // Runs one model on the current video frame, if one is due. Returns after at most one call.
  function infer(now) {
    const t = video.currentTime;
    if (castsEnabled && vision.gesture && now >= nextAt.gesture && t !== seen.gesture && !rebuilding.gesture) {
      seen.gesture = t;
      runGesture(now);
      nextAt.gesture = now + Math.max(hand ? HAND_MS.tracking : HAND_MS.searching, cost.gesture * HAND_MS.costX);
    } else if (now >= nextAt.face && t !== seen.face && !rebuilding.face) {
      seen.face = t;
      runFace(now);
      nextAt.face = now + Math.max(FACE_MS.min, cost.face * FACE_MS.costX);
    }
  }

  function timed(name, fn) {
    const t0 = performance.now();
    try { return fn(); } catch (err) { taskFailed(name, err); return null; } finally { cost[name] += (performance.now() - t0 - cost[name]) * 0.3; }
  }

  function runGesture(now) {
    if (!castsEnabled || !vision.gesture) return;
    const dt = clamp((now - lastAt.gesture) / 1000, 0, 0.25);
    lastAt.gesture = now;
    const res = timed('gesture', () => vision.gesture.recognizeForVideo(video, now));
    hand = res?.landmarks?.[0] || null;
    const top = res?.gestures?.[0]?.[0];
    gesture = { name: top?.categoryName || 'None', score: top?.score || 0 };
    if (canCast()) {
      hold = casts.update(gesture.name, gesture.score, now);
      if (hold.aiming && hand) aimAt(fingerAim(hand[8]), 14, dt);
      if (hold.cast) cast(hold.cast, 'gesture', now);
    } else hold = casts.update('None', 0, now);
    drawHands();
  }

  function runFace(now) {
    const dt = clamp((now - lastAt.face) / 1000, 0, 0.5);
    lastAt.face = now;
    const res = timed('face', () => vision.face.detectForVideo(video, now));
    const out = tracker.update(res?.faceBlendshapes?.[0]?.categories || null, dt);
    mood = out.mood;
    if (out.event === 'strong') strongPending = true;
  }

  function drawHands() {
    if (!castsEnabled) return;
    const w = video.videoWidth || 320, h = video.videoHeight || 240;
    if (hands.width !== w || hands.height !== h) { hands.width = w; hands.height = h; }
    handsCtx.clearRect(0, 0, w, h);
    if (!hand) return;
    const lw = Math.max(2, w / 160);
    handsCtx.lineWidth = lw;
    handsCtx.strokeStyle = hold.held ? '#ffcf4a' : 'rgba(89,216,200,0.9)';
    handsCtx.beginPath();
    for (const [a, b] of HAND_LINKS) { handsCtx.moveTo(hand[a].x * w, hand[a].y * h); handsCtx.lineTo(hand[b].x * w, hand[b].y * h); }
    handsCtx.stroke();
    handsCtx.fillStyle = '#f2e8d5';
    for (const p of hand) handsCtx.fillRect(p.x * w - lw, p.y * h - lw, lw * 2, lw * 2);
    if (hold.aiming) {
      handsCtx.strokeStyle = '#ff5a72';
      handsCtx.beginPath(); handsCtx.arc(hand[8].x * w, hand[8].y * h, lw * 4, 0, Math.PI * 2); handsCtx.stroke();
    }
  }

  // ---------- UI ----------
  function updatePanel(now) {
    if (!castsEnabled) {
      for (const k of EMOTIONS) ui.bars[k].style.width = `${Math.round(mood[k] * 100)}%`;
      const dom = !stream ? 'camera off' : !mood.face ? 'no face' : mood.calibrating ? 'reading your face…' : mood.dominant;
      if (ui.dominant.textContent !== dom) ui.dominant.textContent = dom;
      panel.dataset.mood = stream ? mood.dominant : 'off';
      return;
    }
    const fmt = (s) => `${s.glyph} ${s.name}`;
    let text;
    const recent = lastCast && now - lastCast.at < 1400 ? SPELL_BY_ID[lastCast.id] : null;
    if (recent) text = `Cast: ${fmt(recent)}!`;
    else if (stream && vision) {
      const g = GESTURE_LABEL[gesture.name] || gesture.name;
      if (hold.held) {
        const s = SPELL_BY_ID[hold.held];
        const cd = casts.cooldownLeft(s.id, now);
        text = cd > 0 ? `${g}: ${s.name} ready in ${(cd / 1000).toFixed(1)} s` : hold.aiming ? `${g}: aiming` : `${g}: hold for ${s.name}…`;
      } else text = hand ? `Hand: ${g}` : 'Show a hand to cast. Point up to aim.';
    } else text = 'Press 1–7 or tap a spell to cast.';
    if (ui.now.textContent !== text) ui.now.textContent = text;

    for (const s of SPELLS) {
      const el = ui.spells[s.id];
      const cd = casts.cooldownFrac(s.id, now);
      el.style.setProperty('--cd', cd.toFixed(3));
      el.style.setProperty('--hold', (hold.held === s.id ? hold.progress : 0).toFixed(3));
      el.classList.toggle('is-cooling', cd > 0);
      el.classList.toggle('is-held', hold.held === s.id);
    }
    for (const k of EMOTIONS) ui.bars[k].style.width = `${Math.round(mood[k] * 100)}%`;
    const dom = !stream ? 'camera off' : !mood.face ? 'no face' : mood.calibrating ? 'reading your face…' : mood.dominant;
    if (ui.dominant.textContent !== dom) ui.dominant.textContent = dom;
    panel.dataset.mood = stream ? mood.dominant : 'off';
  }

  function drawFx(now) {
    while (pings.length && now - pings[0].at > PING_MS) pings.shift();
    const G = getG();
    const cursor = stream && hold.aiming && hand && G?.brain;
    if (!pings.length && !cursor) {
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
    if (cursor) {
      const x = G.brain.aim.x * cw, y = G.brain.aim.y * ch;
      fxCtx.lineWidth = u;
      fxCtx.strokeStyle = 'rgba(89,216,200,0.95)';
      fxCtx.beginPath(); fxCtx.arc(x, y, 7 * u, 0, Math.PI * 2); fxCtx.stroke();
      fxCtx.beginPath(); fxCtx.moveTo(x - 10 * u, y); fxCtx.lineTo(x - 4 * u, y); fxCtx.moveTo(x + 4 * u, y); fxCtx.lineTo(x + 10 * u, y);
      fxCtx.moveTo(x, y - 10 * u); fxCtx.lineTo(x, y - 4 * u); fxCtx.moveTo(x, y + 4 * u); fxCtx.lineTo(x, y + 10 * u); fxCtx.stroke();
    }
    for (const p of pings) {
      const k = (now - p.at) / PING_MS;
      const x = p.x * cw, y = p.y * ch;
      fxCtx.globalAlpha = 1 - k;
      fxCtx.lineWidth = u;
      fxCtx.strokeStyle = '#ffcf4a';
      fxCtx.beginPath(); fxCtx.arc(x, y, (6 + 18 * k) * u, 0, Math.PI * 2); fxCtx.stroke();
      fxCtx.font = `${Math.round(10 * u)}px "Segoe UI Emoji", "Apple Color Emoji", "Noto Color Emoji", sans-serif`;
      fxCtx.textAlign = 'center';
      fxCtx.fillText(p.glyph, x, y - (12 + 10 * k) * u);
    }
    fxCtx.globalAlpha = 1;
  }

  // ---------- loop ----------
  // rAF, with a timer fallback for embedded views that throttle it (same as the game loop).
  function tick() {
    const now = performance.now();
    if (now - lastTick < 8) return;
    lastTick = now;
    if (stream && vision && status === 'on' && video.readyState >= 2) infer(now);
    const G = getG();
    if (G) {
      G.mood = { ...(stream ? mood : neutralMood()), active: !!stream };
      syncMood(G);
    }
    if (!panel.hidden && now - uiAt > 60) { uiAt = now; updatePanel(now); }
    drawFx(now);
  }
  function rafLoop() { tick(); requestAnimationFrame(rafLoop); }
  requestAnimationFrame(rafLoop);
  setInterval(() => { if (performance.now() - lastTick > 45) tick(); }, 33);

  setStatus('off');

  // Debug and test handle (window.__moc.camera).
  return {
    get status() { return status; },
    get mood() { return mood; },
    get vision() { return vision; },
    get cost() { return { ...cost }; },
    start, stop, cast,
    casts, tracker,
    // Download and initialise the models without a camera, e.g. to time them.
    async loadModels() { const v = await loadVision(undefined, { hands: castsEnabled }); return { timings: v.timings, delegates: v.delegates }; },
  };
}
