// Mind over Chicken: boot, scene flow, render pipeline and input.

import { W, H, clamp } from './sim/util.js';
import { World } from './world/terrain.js';
import { SimSource, BridgeSource } from './brain/sources.js';
import { Brain } from './brain/brain.js';
import { Advisor } from './ui/advisor.js';
import { Hud } from './ui/hud.js';
import { Audio } from './audio.js';
import { StoneAge } from './eras/stone.js';
import { FarmingToKingdoms, InfoAge } from './eras/timeskip.js';
import { Industry } from './eras/industry.js';
import { SpaceAge } from './eras/space.js';
import { initCamera } from './camera/camera.js'; // camera (#33)

const SCENES = [() => new StoneAge(), () => new FarmingToKingdoms(), () => new Industry(), () => new InfoAge(), () => new SpaceAge()];
const $ = (id) => document.getElementById(id);

// ---------- canvases ----------
const screen = $('screen');
const main = screen.getContext('2d');
const low = Object.assign(document.createElement('canvas'), { width: W, height: H });
const lowCtx = low.getContext('2d');
const light = Object.assign(document.createElement('canvas'), { width: W, height: H });
const lightCtx = light.getContext('2d');
let K = 3;

function resize() {
  const rect = screen.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  K = Math.max(2, Math.round((rect.width * dpr) / W));
  if (screen.width !== W * K) { screen.width = W * K; screen.height = H * K; }
}
new ResizeObserver(resize).observe(screen);

// ---------- game state ----------
const audio = new Audio();
const advisor = new Advisor($('advisor'));
const hud = new Hud(document);
let G = null;
let fade = 0;           // 0..1 black overlay for transitions
let pendingScene = null;
let pendingTimer = 0;
let banner = null;      // {t}
let lastTs = 0;

function newGame(source, startIndex = 0) {
  if (G?.source && G.source !== source) G.source.disconnect();
  const brain = new Brain(source);
  G = {
    world: new World(),
    brain, source, advisor, hud, audio,
    scene: null, sceneIndex: startIndex, runTime: 0, shake: 0,
    stats: { lightning: 0, smitten: 0, births: 0, deaths: 0, recessions: 0, restarts: 0 },
    eraTimes: [], eraStart: 0, finished: false,
    complete(delay = 2) { pendingScene = G.sceneIndex + 1; pendingTimer = delay; },
    fail(reason) { showFail(reason); },
    finish(extra) { G.finished = true; G.extra = extra; setTimeout(showEnd, 2200); },
  };
  brain.onBlink((aim) => { hud.flashBlink(); if (!banner && G.scene?.onBlink) G.scene.onBlink(aim); });
  brain.onTrained = () => {};
  $('endScreen').hidden = true;
  $('failScreen').hidden = true;
  startScene(startIndex);
  try { window.claude?.hot?.snapshot?.(() => ({ sceneIndex: G?.sceneIndex ?? 0, kind: G?.source?.kind })); } catch {}
}

function startScene(i) {
  if (i >= SCENES.length) return;
  if (G.scene) G.eraTimes.push({ label: G.scene.label, seconds: G.runTime - G.eraStart });
  G.eraStart = G.runTime;
  G.sceneIndex = i;
  const scene = SCENES[i]();
  G.scene = scene;
  advisor.setEra(scene.era);
  scene.enter(G);
  hud.setScene(scene);
  document.body.dataset.era = scene.era;
  fade = 1;
  if (scene.intro) showBanner(scene, i);
  else banner = null;
}

function restartScene() {
  G.stats.restarts++;
  const i = G.sceneIndex;
  const scene = SCENES[i]();
  G.scene = scene;
  advisor.setEra(scene.era);
  scene.enter(G);
  hud.setScene(scene);
  fade = 1;
  if (scene.intro) showBanner(scene, i);
}

// ---------- overlays ----------
function showBanner(scene, i) {
  const el = $('banner');
  const n = ['I', 'II', 'III'][[0, 2, 4].indexOf(i)] || '';
  el.querySelector('[data-banner-era]').textContent = n ? `Era ${n}` : '';
  el.querySelector('[data-banner-title]').textContent = scene.intro.title;
  el.querySelector('[data-banner-goal]').textContent = scene.intro.goal;
  const list = el.querySelector('[data-banner-how]');
  list.replaceChildren(...scene.intro.how.map(([k, v]) => {
    const li = document.createElement('li');
    const b = document.createElement('b'); b.textContent = k;
    li.append(b, document.createTextNode(` ${v}`));
    return li;
  }));
  el.hidden = false;
  banner = { t: 0 };
  G.brain.setCue(null);
}

function dismissBanner() {
  if (!banner) return;
  banner = null;
  $('banner').hidden = true;
  // Re-arm the scene's calibration cue after the pause.
  if (G.scene.id === 'stone' || G.scene.id === 'industry') G.brain.setCue(G.scene.hold?.active || G.scene.ipo?.state === 'active' ? 'calm' : 'rest');
}

let failTimer = null;
function showFail(reason) {
  $('failReason').textContent = reason;
  $('failScreen').hidden = false;
  audio.bad();
  clearTimeout(failTimer);
  failTimer = setTimeout(retryEra, 5000);
}
function retryEra() {
  clearTimeout(failTimer);
  if ($('failScreen').hidden) return;
  $('failScreen').hidden = true;
  restartScene();
}

function showEnd() {
  const b = G.brain, s = G.stats, m = b.model, x = G.extra || {};
  G.eraTimes.push({ label: G.scene.label, seconds: G.runTime - G.eraStart });
  const t = Math.floor(G.runTime);
  const rows = [
    ['Run time', `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`],
    ['Era restarts', s.restarts],
    ['Lightning bolts', s.lightning],
    ['Villagers who found religion', s.smitten],
    ['Goat recessions caused', s.recessions],
    ['Passengers to the new world', `${x.passengers ?? 0} and one chicken`],
    ['Countdown scrubs', x.scrubs ?? 0],
    ['Seconds of calm', Math.round(b.stats.calmSeconds)],
    ['Seconds of focus', Math.round(b.stats.focusSeconds)],
    ['Peak heart rate', b.stats.peakHr ? `${Math.round(b.stats.peakHr)} bpm` : 'n/a'],
    ['Brain windows recorded', m.total()],
    ['Personal model accuracy', m.trained ? `${Math.round(m.accuracy * 100)}% (chance ${Math.round(m.chance * 100)}%)` : 'not trained'],
  ];
  $('endStats').replaceChildren(...rows.map(([k, v]) => {
    const d = document.createElement('div');
    const dt = document.createElement('dt'); dt.textContent = k;
    const dd = document.createElement('dd'); dd.textContent = v;
    d.append(dt, dd);
    return d;
  }));
  $('endScreen').hidden = false;
}

// ---------- render ----------
function render() {
  const scene = G?.scene;
  lowCtx.imageSmoothingEnabled = false;
  if (!scene) { main.fillStyle = '#0b0c16'; main.fillRect(0, 0, screen.width, screen.height); return; }
  lowCtx.setTransform(1, 0, 0, 1, 0, 0);
  scene.draw(lowCtx);
  const dark = scene.darkness || 0;
  if (dark > 0) {
    lightCtx.globalCompositeOperation = 'source-over';
    lightCtx.clearRect(0, 0, W, H);
    lightCtx.fillStyle = `rgba(6, 10, 32, ${dark})`;
    lightCtx.fillRect(0, 0, W, H);
    lightCtx.globalCompositeOperation = 'destination-out';
    for (const l of scene.lights()) {
      const g = lightCtx.createRadialGradient(l.x, l.y, 0, l.x, l.y, l.r);
      g.addColorStop(0, 'rgba(0,0,0,1)'); g.addColorStop(0.55, 'rgba(0,0,0,0.75)'); g.addColorStop(1, 'rgba(0,0,0,0)');
      lightCtx.fillStyle = g;
      lightCtx.beginPath(); lightCtx.arc(l.x, l.y, l.r, 0, Math.PI * 2); lightCtx.fill();
    }
    lightCtx.globalCompositeOperation = 'source-over';
    lowCtx.drawImage(light, 0, 0);
    // warm glow on top of fires
    lowCtx.globalCompositeOperation = 'lighter';
    for (const l of scene.lights()) {
      const g = lowCtx.createRadialGradient(l.x, l.y, 0, l.x, l.y, l.r * 0.6);
      g.addColorStop(0, 'rgba(255,140,50,0.22)'); g.addColorStop(1, 'rgba(255,140,50,0)');
      lowCtx.fillStyle = g; lowCtx.fillRect(l.x - l.r, l.y - l.r, l.r * 2, l.r * 2);
    }
    lowCtx.globalCompositeOperation = 'source-over';
  }
  scene.drawAfterLight?.(lowCtx);

  const sx = G.shake ? (Math.random() - 0.5) * G.shake * K : 0;
  const sy = G.shake ? (Math.random() - 0.5) * G.shake * K : 0;
  main.setTransform(1, 0, 0, 1, 0, 0);
  main.imageSmoothingEnabled = false;
  main.fillStyle = '#05060f';
  main.fillRect(0, 0, screen.width, screen.height);
  main.drawImage(low, sx, sy, W * K, H * K);
  main.setTransform(K, 0, 0, K, sx, sy);
  scene.drawOverlay?.(main);
  if (fade > 0) { main.setTransform(1, 0, 0, 1, 0, 0); main.fillStyle = `rgba(5,6,15,${fade})`; main.fillRect(0, 0, screen.width, screen.height); }
}

// ---------- loop ----------
// Driven by requestAnimationFrame, with a timer fallback for embedded views that throttle rAF.
let lastFrameAt = 0;
function frame(ts) {
  ts = performance.now();
  if (ts - lastFrameAt < 8) return;
  lastFrameAt = ts;
  const dt = Math.min(0.05, (ts - lastTs) / 1000 || 0.016);
  lastTs = ts;
  if (G) {
    G.brain.update(dt);
    if (banner) {
      banner.t += dt;
      if (banner.t > 6) dismissBanner();
    } else if ($('failScreen').hidden && $('endScreen').hidden) {
      G.runTime += dt;
      G.scene.update(dt);
    } else if (!$('endScreen').hidden) {
      G.scene.update(dt);
    }
    if (pendingScene != null) {
      pendingTimer -= dt;
      if (pendingTimer <= 0) {
        fade = Math.min(1, fade + dt * 3);
        if (fade >= 1) { const n = pendingScene; pendingScene = null; startScene(n); }
      }
    } else fade = Math.max(0, fade - dt * 1.6);
    G.shake = Math.max(0, (G.shake || 0) - dt * 2);
    advisor.update(dt);
    hud.update(dt, G);
  }
  render();
}
function rafLoop() { frame(); requestAnimationFrame(rafLoop); }
setInterval(() => { if (performance.now() - lastFrameAt > 45) frame(); }, 33);

// ---------- input ----------
function sim() { return G?.source?.kind === 'sim' ? G.source : null; }

function aimFromEvent(e) {
  const r = screen.getBoundingClientRect();
  return { x: clamp((e.clientX - r.left) / r.width), y: clamp((e.clientY - r.top) / r.height) };
}

screen.addEventListener('pointermove', (e) => {
  const s = sim(); if (!s) return;
  const a = aimFromEvent(e);
  s.setAim(a.x, a.y);
  G.brain.aim.x = a.x; G.brain.aim.y = a.y;
});
screen.addEventListener('pointerdown', (e) => {
  audio.unlock();
  if (banner) { dismissBanner(); return; }
  const s = sim(); if (!s) return;
  const a = aimFromEvent(e);
  s.setAim(a.x, a.y);
  G.brain.aim.x = a.x; G.brain.aim.y = a.y;
  s.blink();
});

const KEYS = { Space: 'calm', KeyF: 'focus', ShiftLeft: 'panic', ShiftRight: 'panic' };
window.addEventListener('keydown', (e) => {
  if (e.target.closest?.('input')) return;
  if (e.code === 'KeyM') { toggleModel(); return; }
  if (e.code === 'Enter') {
    if (banner) dismissBanner();
    else if (!$('failScreen').hidden) retryEra();
    else G?.scene?.skip?.();
    return;
  }
  const s = sim(); if (!s) return;
  if (KEYS[e.code]) { s.input[KEYS[e.code]] = true; e.preventDefault(); }
  if (e.code === 'KeyB' && !e.repeat) s.blink();
  const step = 0.04;
  if (e.code.startsWith('Arrow')) {
    const a = G.brain.aim;
    if (e.code === 'ArrowLeft') a.x = clamp(a.x - step);
    if (e.code === 'ArrowRight') a.x = clamp(a.x + step);
    if (e.code === 'ArrowUp') a.y = clamp(a.y - step);
    if (e.code === 'ArrowDown') a.y = clamp(a.y + step);
    s.setAim(a.x, a.y);
    e.preventDefault();
  }
});
window.addEventListener('keyup', (e) => {
  const s = sim(); if (!s) return;
  if (KEYS[e.code]) s.input[KEYS[e.code]] = false;
});
window.addEventListener('blur', () => { const s = sim(); if (s) Object.keys(s.input).forEach((k) => (s.input[k] = false)); });

// Toolbar tiles double as touch controls for the simulated brain.
document.querySelectorAll('[data-power]').forEach((tile) => {
  const power = tile.dataset.power;
  const map = { calm: 'calm', focus: 'focus', heart: 'panic' };
  tile.addEventListener('pointerdown', (e) => {
    audio.unlock();
    const s = sim(); if (!s) return;
    if (power === 'blink') { s.blink(); return; }
    s.input[map[power]] = true;
    tile.setPointerCapture?.(e.pointerId);
  });
  const up = () => { const s = sim(); if (s && map[power]) s.input[map[power]] = false; };
  tile.addEventListener('pointerup', up);
  tile.addEventListener('pointercancel', up);
  tile.addEventListener('lostpointercapture', up);
});

function toggleModel() {
  const p = $('modelPanel');
  p.hidden = !p.hidden;
  $('modelBtn').setAttribute('aria-pressed', String(!p.hidden));
}
$('modelBtn').addEventListener('click', toggleModel);
$('modelClose').addEventListener('click', toggleModel);
$('soundBtn').addEventListener('click', () => {
  audio.unlock();
  const on = audio.toggle();
  $('soundBtn').textContent = on ? 'Sound on' : 'Sound off';
  $('soundBtn').setAttribute('aria-pressed', String(on));
});
$('bannerGo').addEventListener('click', dismissBanner);
$('retryBtn').addEventListener('click', retryEra);
$('againBtn').addEventListener('click', () => { $('endScreen').hidden = true; newGame(new SimSource()); });
$('endModelBtn').addEventListener('click', toggleModel);

// ---------- title screen ----------
function begin(source) {
  audio.unlock();
  $('titleScreen').hidden = true;
  newGame(source);
}
$('playSim').addEventListener('click', () => begin(new SimSource()));
$('playBridge').addEventListener('click', async () => {
  const url = $('bridgeUrl').value.trim() || 'ws://localhost:8765';
  const status = $('bridgeStatus');
  status.textContent = `Connecting to ${url}…`;
  const src = new BridgeSource(url);
  try {
    await src.connect();
    status.textContent = 'Connected.';
    begin(src);
  } catch (err) {
    status.textContent = `${err.message}. Start the bridge with: python bridge/muse_bridge.py --board muse2 (or --board synthetic to test).`;
  }
});

// Debug handle for the console (and for automated playtests).
window.__moc = { get G() { return G; }, startScene: (i) => startScene(i) };

// camera (#33): opt-in webcam. Hand gestures cast spells (keys 1-7 without a webcam), the face sets
// the world's mood. It adds to the brain and keyboard inputs and never replaces them.
window.__moc.camera = initCamera({
  getG: () => G,
  canCast: () => !!G?.scene && !banner && !G.finished && $('titleScreen').hidden && $('failScreen').hidden && $('endScreen').hidden,
});

// ---------- boot ----------
function start(data = {}) {
  resize();
  lastTs = performance.now();
  requestAnimationFrame(rafLoop);
  if (data && data.kind === 'sim' && data.sceneIndex > 0) {
    $('titleScreen').hidden = true;
    newGame(new SimSource(), data.sceneIndex);
  }
}
document.fonts?.load('16px "Pixelify Sans"').finally?.(() => {});
if (window.claude?.hot?.ready) window.claude.hot.ready(start);
else start(window.claude?.hot?.data ?? {});
