// Mind over Chicken: boot, scene flow, render pipeline and input.

import { W, H, clamp } from './sim/util.js';
import { World } from './world/terrain.js';
import { SimSource, BridgeSource } from './brain/sources.js';
import { Brain } from './brain/brain.js';
import { Advisor } from './ui/advisor.js';
import { Hud } from './ui/hud.js';
import { EegPanel } from './ui/eeg-panel.js';
import { Audio } from './audio.js';
import { StoneAge } from './eras/stone.js';
import { FarmingToKingdoms, InfoAge } from './eras/timeskip.js';
import { Industry } from './eras/industry.js';
import { SpaceAge } from './eras/space.js';
import initEngine, { Engine, RustImuTracker, RustBlinkClassifier } from '../pkg/mind_over_chicken_engine.js';
import { Cosmos } from './cosmos/cosmos.js';
import { ERA_LABEL, ERA_NAMES, CHANNELS, fmtPop, planetDot, ownerLabel } from './cosmos/galaxy.js';
import { initCamera } from './camera/camera.js'; // camera (#33)
import { Calibration, readCalibrationConfig } from './brain/calibration.js'; // calibration (#54)
import { SIM } from './cosmos/planet-sim.js';

// calibration (#54): the seed window is configurable (?calib=, ?calib_mode=, ?seed=); the galaxy's
// mind read uses the same window.
const calibConfig = readCalibrationConfig();
SIM.MIND_SECONDS = calibConfig.seconds;

const SCENES = [() => new StoneAge(), () => new FarmingToKingdoms(), () => new Industry(), () => new InfoAge(), () => new SpaceAge()];
const $ = (id) => document.getElementById(id);

// ---------- canvases & WebGL engine ----------
const screen = $('screen');
const textLayer = $('textLayer');
const textCtx = textLayer.getContext('2d');
const low = Object.assign(document.createElement('canvas'), { width: W, height: H });
const lowCtx = low.getContext('2d');
let K = 3;
let engine = null;

try {
  await initEngine();
  BridgeSource.setWasmClasses(RustImuTracker, RustBlinkClassifier);
  engine = new Engine('screen');
} catch (err) {
  console.error('Failed to initialize Rust WebGL engine:', err);
}
// Without WebGL, fall back to a plain 2D upscale so the game still shows.
const fallback2d = engine ? null : screen.getContext('2d');

// 3D world (#51): set_3d_mode only takes effect once the 3D renderer exists, so create it
// here and start in 3D. If the GPU can't do it, stay in 2D and hide the toggle.
let has3d = false;
if (engine) {
  try {
    engine.init_3d();
    engine.set_3d_mode(true);
    has3d = engine.is_3d_mode();
  } catch (err) {
    console.warn('3D renderer unavailable; staying in 2D:', err);
  }
}
function sync3dButton() {
  const btn = $('mode3dBtn');
  if (!btn) return;
  const on = !!engine && engine.is_3d_mode();
  btn.hidden = !has3d;
  btn.textContent = on ? '3D: ON' : '3D: OFF';
  btn.setAttribute('aria-pressed', String(on));
}
sync3dButton();

function resize() {
  const rect = screen.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  K = Math.max(2, Math.round((rect.width * dpr) / W));
  if (screen.width !== W * K) { screen.width = W * K; screen.height = H * K; }
  if (textLayer.width !== W * K) { textLayer.width = W * K; textLayer.height = H * K; }
}
new ResizeObserver(resize).observe(screen);

// ---------- game state ----------
const audio = new Audio();
const advisor = new Advisor($('advisor'));
const hud = new Hud(document);
const cosmos = new Cosmos();
// What the HUD shows while you look at the galaxy.
const galaxyHudScene = { label: 'Galaxy', powers: ['calm', 'focus', 'heart'], objectives: () => cosmos.galaxy?.objectives() || [] };
let hudScene = null;
let G = null;
let previewSource = null;
let previewBrain = null;

function setGameSource(nextSource) {
  if (G) {
    if (G.source && G.source !== nextSource) G.source.disconnect();
    G.source = nextSource;
    G.brain.source = nextSource;
    G.brain.resetTelemetry();
  } else {
    if (previewSource && previewSource !== nextSource) previewSource.disconnect();
    previewSource = nextSource;
    previewBrain = nextSource ? new Brain(nextSource) : null;
    if (previewBrain && nextSource?.kind === 'bridge') previewBrain.resetTelemetry();
  }
}

const eegPanel = new EegPanel(document, {
  getSource: () => G?.source || previewSource,
  getBrain: () => G?.brain || previewBrain,
  onConnectBridge: async (url) => {
    const src = new BridgeSource(url);
    await src.connect();
    setGameSource(src);
  },
  onDisconnectBridge: () => {
    setGameSource(new SimSource());
  },
});
let fade = 0;           // 0..1 black overlay for transitions
let pendingScene = null;
let pendingTimer = 0;
let banner = null;      // {t}
let lastTs = 0;

function newGame(source, startIndex = 0) {
  if (previewSource && previewSource !== source) previewSource.disconnect();
  previewSource = null;
  previewBrain = null;
  if (G?.source && G.source !== source) G.source.disconnect();
  const brain = new Brain(source);
  cosmos.reset();
  G = {
    world: new World(),
    brain, source, advisor, hud, audio,
    calibration: new Calibration(calibConfig), // calibration (#54)
    scene: null, sceneIndex: startIndex, runTime: 0, shake: 0,
    stats: { lightning: 0, smitten: 0, births: 0, deaths: 0, recessions: 0, restarts: 0 },
    eraTimes: [], eraStart: 0, finished: false,
    complete(delay = 2) { pendingScene = G.sceneIndex + 1; pendingTimer = delay; },
    fail(reason) { showFail(reason); },
    finish(extra) { G.finished = true; G.extra = extra; setTimeout(showEnd, 2200); },
  };
  brain.onBlink((aim) => {
    hud.flashBlink();
    cosmos.galaxy?.onBlink();   // storms on the blink world; a strike on the world you're zoomed into
    if (cosmos.galaxy?.attended) audio.thunder();
    if (!banner && !cosmos.active && G.scene?.onBlink) G.scene.onBlink(aim);
  });
  // The galaxy exists from the start: home plus three worlds the first 10 s of play will shape.
  cosmos.ensureGalaxy(G);
  brain.onTrained = () => {};
  $('endScreen').hidden = true;
  $('failScreen').hidden = true;
  if (engine && G.world) {
    engine.init_world(G.world.seed);
    // The 3D relief must be the world the simulation runs on (#71). Older pkg builds lack this.
    if (engine.set_heightmap) {
      try { engine.set_heightmap(G.world.h); } catch (err) { console.warn('set_heightmap failed:', err); }
    }
    // Around the 3D map: this world's deep sea, so the edges read as open ocean.
    const deep = G.world.palette?.[0]?.[0];
    if (deep && engine.set_backdrop) engine.set_backdrop(deep[0], deep[1], deep[2]);
    engine.set_era(startIndex);
  }
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
  if (engine) engine.set_era(i);
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
  if (engine) engine.set_era(i);
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
  textCtx.setTransform(1, 0, 0, 1, 0, 0);
  textCtx.clearRect(0, 0, textLayer.width, textLayer.height);
  if (!scene) {
    if (engine) {
      engine.begin_frame();
      engine.end_frame(0, 0, 0, 0, 0, screen.width, screen.height);
    }
    return;
  }
  lowCtx.imageSmoothingEnabled = false;
  lowCtx.setTransform(1, 0, 0, 1, 0, 0);
  // 3D only on the surface itself: once the globe zoom starts, the flat 2D map takes over (the
  // 3D camera has already tipped over to that exact frame, so the hand-over is seamless).
  G.render3d = in3dSurface();
  if (G.render3d) {
    // The 3D camera follows the same east–west pan as the 2D map (A/D, Shift+scroll, drag).
    engine.set_camera_target(cosmos.mapX(W / 2), engine.get_camera_target_z());
    stepCam3dTween();
  }
  // In 3D, scenes with draw3d stand their actors up as sprites, so they skip painting them flat.
  scene.draw(lowCtx);
  scene.drawAfterLight?.(lowCtx);
  if (!G.render3d) cosmos.applyPan(low);   // the surface is a planet: it wraps east–west (3D pans the camera instead)
  const cosmic = cosmos.render(low, lowCtx, G);

  // Zooming out fades the surface's night lighting and labels; the globe and galaxy
  // carry their own shading, so the night shader must not dim space.
  const z = cosmos.z || 0;
  const surface = !cosmic || z < 0.3;
  const k = cosmic ? Math.max(0, 1 - z / 0.3) : 1;
  const dark = (scene.darkness || 0) * k;
  const flash = (scene.flash || 0) * k;
  const shakeAmount = G.shake ? (G.shake * 0.008) : 0;
  const sx = shakeAmount ? (Math.random() - 0.5) * shakeAmount : 0;
  const sy = shakeAmount ? (Math.random() - 0.5) * shakeAmount : 0;
  // The surface is panned east–west after it is drawn, so lights (and labels) follow the
  // pan, with a second copy across the date line.
  const offsets = cosmos.overlayOffsets();

  if (engine) {
    try {
      if (surface && G.render3d) {
        engine.upload_canvas(low);
        if (scene.draw3d) {
          scene.draw3d(engine, 0.016);
        } else if (scene.lights) {
          for (const l of scene.lights()) {
            engine.add_3d_light(l.x, 10.0, l.y, l.r, 1.2);
          }
        }
        // Directional sun rotates with day/night
        const sunAngle = (G.runTime || 0) * 0.15;
        const sunX = Math.cos(sunAngle);
        const sunY = 0.85;
        const sunZ = Math.sin(sunAngle);
        engine.render_3d(0.016, sunX, sunY, sunZ, dark, flash, fade, sx, sy, screen.width, screen.height);
      } else {
        // begin_frame clears last frame's lights; without it they pile up every frame (#38).
        engine.begin_frame();
        engine.upload_canvas(low);
        if (surface && scene.lights) {
          for (const l of scene.lights()) {
            for (const off of offsets) engine.add_light(l.x + off, l.y, l.r, 1.0);
          }
        }
        engine.render_uploaded_canvas(dark, flash, fade, sx, sy, screen.width, screen.height);
      }
    } catch (e) {
      console.error('WebGL render error:', e);
    }
  } else if (fallback2d) {
    fallback2d.imageSmoothingEnabled = false;
    fallback2d.drawImage(low, 0, 0, screen.width, screen.height);
    if (dark > 0) { fallback2d.fillStyle = `rgba(6, 10, 32, ${dark})`; fallback2d.fillRect(0, 0, screen.width, screen.height); }
    if (fade > 0) { fallback2d.fillStyle = `rgba(5, 6, 15, ${fade})`; fallback2d.fillRect(0, 0, screen.width, screen.height); }
  }

  // Overlay (labels, calm ring, goat chart, countdown, galaxy labels) at full resolution,
  // after lighting.
  const visible = 1 - Math.min(1, fade);
  if (surface) {
    textCtx.globalAlpha = visible * k;
    for (const off of offsets) {
      textCtx.setTransform(K, 0, 0, K, off * K, 0);
      scene.drawOverlay?.(textCtx);
    }
  }
  textCtx.setTransform(K, 0, 0, K, 0, 0);
  textCtx.globalAlpha = visible;
  if (cosmic) cosmos.overlay(textCtx, G);
  textCtx.globalAlpha = 1;
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
    } else if (G.sandbox) {
      G.runTime += dt;   // the hand-played game stays paused in the sandbox
      G.calibration?.update(dt, G.brain, G.mood);
    } else if ($('failScreen').hidden && $('endScreen').hidden) {
      G.runTime += dt;
      G.calibration?.update(dt, G.brain, G.mood);
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
    cosmos.update(dt, G);
    advisor.update(dt);
    const showGalaxyHud = cosmos.galaxy && (G.sandbox || cosmos.inGalaxy);
    const wanted = showGalaxyHud ? galaxyHudScene : G.scene;
    if (wanted !== hudScene) { hudScene = wanted; hud.setScene(wanted); }
    hud.update(dt, showGalaxyHud ? Object.assign(Object.create(G), { scene: galaxyHudScene }) : G);
    updateCosmosUi(dt);
  } else if (previewBrain) {
    previewBrain.update(dt);
    $('sourceStatus').textContent = previewSource.status;
    $('sourceStatus').dataset.ok = previewSource.connected ? '1' : '0';
  }
  eegPanel.update();
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

function worldFromEvent(e) { const a = aimFromEvent(e); return { x: a.x * W, y: a.y * H }; }
// Aim on the surface: in 3D mode, raycast from screen to terrain; in 2D, screen → map position.
function surfaceAim(e) {
  if (engine && engine.is_3d_mode() && !cosmos.active) {
    const r = screen.getBoundingClientRect();
    const sx = e.clientX - r.left;
    const sy = e.clientY - r.top;
    const hit = engine.screen_to_ground(sx, sy, r.width, r.height);
    if (hit && hit[0] >= 0 && hit[1] >= 0) {
      return { x: clamp(hit[0] / W), y: clamp(hit[1] / H) };
    }
  }
  const a = aimFromEvent(e);
  return { x: cosmos.mapX(a.x * W) / W, y: a.y };
}
let panDrag = null;
let orbitDrag = null;
// ---------- 3D camera <-> globe hand-off ----------
// The globe zoom (cosmos) starts from the flat 2D map. So zooming the 3D camera out past its
// default distance tips it, smoothly, to look straight down, north-up, at the whole map: exactly
// the 2D frame. Only then does the globe take over. Zooming back in reverses it.
// topDown is where the 30 degree lens frames the 180-tile-high map at sea level (y = 7.92):
// 90 / tan(15 deg) + 7.92 = 344.
const CAM3D = { blendFrom: 300, topDown: 344, topPitch: 1.55 };
let cam3dBase = null;   // the player's own angle when the tip-over began
let cam3dTween = null;  // a keyboard/button zoom-out animates the tip-over, then hands over
function in3dSurface() { return !!(engine && engine.is_3d_mode() && !cosmos.active && !((cosmos.z || 0) > 0)); }
function applyCam3dBlend() {
  const d = engine.get_camera_distance();
  const t = clamp((d - CAM3D.blendFrom) / (CAM3D.topDown - CAM3D.blendFrom));
  if (t <= 0) {
    if (cam3dBase) {
      engine.orbit_camera(cam3dBase.yaw - engine.get_camera_yaw(), cam3dBase.pitch - engine.get_camera_pitch());
      engine.set_camera_target(engine.get_camera_target_x(), cam3dBase.z);
      cam3dBase = null;
    }
    return;
  }
  if (!cam3dBase) cam3dBase = { pitch: engine.get_camera_pitch(), yaw: engine.get_camera_yaw(), z: engine.get_camera_target_z() };
  const e = t * t * (3 - 2 * t);
  const northUp = Math.round(cam3dBase.yaw / (2 * Math.PI)) * 2 * Math.PI;
  const yaw = cam3dBase.yaw + (northUp - cam3dBase.yaw) * e;
  const pitch = cam3dBase.pitch + (CAM3D.topPitch - cam3dBase.pitch) * e;
  engine.orbit_camera(yaw - engine.get_camera_yaw(), pitch - engine.get_camera_pitch());
  engine.set_camera_target(engine.get_camera_target_x(), cam3dBase.z + (H / 2 - cam3dBase.z) * e);
}
function zoom3dBy(f) {
  const d = engine.get_camera_distance();
  engine.zoom_camera(Math.min(CAM3D.topDown, d * f) / d);
  applyCam3dBlend();
}
function zoomOutFromSurface() {
  if (!in3dSurface()) { cosmos.zoomOut(G); return; }
  cam3dTween = { from: engine.get_camera_distance(), t0: performance.now(), dur: 350 };
}
function zoomInOnSurface() {
  if (in3dSurface()) { cam3dTween = null; zoom3dBy(0.8); return; }
  cosmos.zoomIn(G);
}
function stepCam3dTween() {
  if (!cam3dTween) return;
  const k = clamp((performance.now() - cam3dTween.t0) / cam3dTween.dur);
  const d = engine.get_camera_distance();
  engine.zoom_camera((cam3dTween.from + (CAM3D.topDown - cam3dTween.from) * k) / d);
  applyCam3dBlend();
  if (k >= 1) { cam3dTween = null; cosmos.zoomOut(G); }
}

screen.addEventListener('pointermove', (e) => {
  if (G && cosmos.active) { const p = worldFromEvent(e); cosmos.pointerMove(e.pointerId, p.x, p.y, G); return; }
  if (orbitDrag && orbitDrag.id === e.pointerId) {
    const dx = e.clientX - orbitDrag.x;
    const dy = e.clientY - orbitDrag.y;
    orbitDrag.x = e.clientX;
    orbitDrag.y = e.clientY;
    if (engine && engine.is_3d_mode()) {
      engine.orbit_camera(dx * 0.008, dy * 0.008);
      cam3dBase = null;
    }
    return;
  }
  if (panDrag && panDrag.id === e.pointerId) {
    const x = worldFromEvent(e).x;
    const dx = x - panDrag.x;
    panDrag.x = x;
    cosmos.pan(dx);   // in 3D the camera follows this pan (see render)
    cam3dBase = null;
    return;
  }
  const s = sim(); if (!s) return;
  const a = surfaceAim(e);
  s.setAim(a.x, a.y);
  G.brain.aim.x = a.x; G.brain.aim.y = a.y;
});
screen.addEventListener('pointerdown', (e) => {
  audio.unlock();
  if (banner) { dismissBanner(); return; }
  if (G && cosmos.active) {
    const p = worldFromEvent(e);
    screen.setPointerCapture?.(e.pointerId);
    cosmos.pointerDown(e.pointerId, p.x, p.y);
    return;
  }
  // Right-drag or Alt-drag orbits the camera, in 3D only. (Shift is the racing-pulse key.)
  if (G && engine && engine.is_3d_mode() && !cosmos.active && (e.button === 2 || (e.button === 0 && e.altKey))) {
    e.preventDefault();
    screen.setPointerCapture?.(e.pointerId);
    orbitDrag = { id: e.pointerId, x: e.clientX, y: e.clientY };
    return;
  }
  // Middle click pans the camera
  if (G && e.button === 1) {
    e.preventDefault();
    screen.setPointerCapture?.(e.pointerId);
    panDrag = { id: e.pointerId, x: worldFromEvent(e).x };
    return;
  }
  // Right drag in 2D mode pans surface
  if (G && e.button === 2) {
    e.preventDefault();
    screen.setPointerCapture?.(e.pointerId);
    panDrag = { id: e.pointerId, x: worldFromEvent(e).x };
    return;
  }
  const s = sim(); if (!s) return;
  const a = surfaceAim(e);
  s.setAim(a.x, a.y);
  G.brain.aim.x = a.x; G.brain.aim.y = a.y;
  s.blink();
});
const endPointer = (e) => {
  if (orbitDrag && orbitDrag.id === e.pointerId) { orbitDrag = null; return; }
  if (panDrag && panDrag.id === e.pointerId) { panDrag = null; return; }
  if (!G || !cosmos.pointers.has(e.pointerId)) return;
  const p = worldFromEvent(e);
  const picked = cosmos.pointerUp(e.pointerId, p.x, p.y, G);
  if (picked) audio.blip();
};
screen.addEventListener('pointerup', endPointer);
screen.addEventListener('pointercancel', endPointer);
screen.addEventListener('wheel', (e) => {
  if (!G) return;
  e.preventDefault();
  // Horizontal scroll (or Shift+scroll) pans the surface; in 3D the camera follows the pan.
  const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey;
  if (horizontal && !cosmos.active) { cosmos.pan(-(e.shiftKey ? e.deltaY : e.deltaX) * 0.15); return; }
  if (in3dSurface()) {
    cam3dTween = null;
    // Already looking straight down at the whole map: the next notch hands over to the globe.
    if (e.deltaY > 0 && engine.get_camera_distance() >= CAM3D.topDown - 0.5) { cosmos.zoomOut(G); return; }
    zoom3dBy(e.deltaY > 0 ? 1.1 : 0.91);
    return;
  }
  cosmos.wheel(e.deltaY, G);
}, { passive: false });
screen.addEventListener('contextmenu', (e) => e.preventDefault());

const KEYS = { Space: 'calm', KeyF: 'focus', ShiftLeft: 'panic', ShiftRight: 'panic' };
window.addEventListener('keydown', (e) => {
  if (e.target.closest?.('input')) return;
  if (e.code === 'KeyM') { toggleModel(); return; }
  if (e.code === 'Escape' && eegPanel.wiz?.active) { eegPanel.closeWizard(); return; }
  if (e.code === 'KeyE') { eegPanel.toggle(); return; }
  if (e.code === 'KeyC') { eegPanel.calibrateCenter(); return; }
  if (e.code === 'KeyT') { toggle3D(); return; }
  if (G && (e.code === 'KeyZ' || e.code === 'Minus' || e.code === 'NumpadSubtract')) { zoomOutFromSurface(); return; }
  if (G && (e.code === 'KeyX' || e.code === 'Equal' || e.code === 'NumpadAdd')) { zoomInOnSurface(); return; }
  if (G && !cosmos.active && (e.code === 'KeyA' || e.code === 'KeyD')) { cosmos.panVel = e.code === 'KeyA' ? 140 : -140; return; }
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

function toggle3D() {
  if (!engine || !has3d) return;
  engine.set_3d_mode(!engine.is_3d_mode());
  cam3dBase = null; cam3dTween = null;
  sync3dButton();
  const on = engine.is_3d_mode();
  advisor.say('3d-toggle', on ? '3D view: right-drag to orbit, wheel to zoom.' : '2D classic view.', { mood: 'happy', priority: 2, cooldown: 1 });
}
$('mode3dBtn')?.addEventListener('click', toggle3D);
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

// ---------- zoom + galaxy UI ----------
let revealed = 0;
cosmos.onEvent = (p, text, kind) => {
  if (kind === 'born' && p.mindSlot) {
    revealed++;
    audio.good();
    if (revealed === 1) advisor.say('mindworlds', `My lord, your thoughts are making worlds. ${p.name} was born from your ${CHANNELS[p.channel].label.toLowerCase()}. Zoom out with Z to see them.`, { mood: 'happy', priority: 2, cooldown: 999 });
    if (revealed === 3) advisor.say('mindworlds3', 'Three worlds now, each listening to a different part of your mind. Steer them from here.', { mood: 'nervous', priority: 1, cooldown: 999 });
  } else if (kind === 'event' && cosmos.inGalaxy) {
    advisor.say(`gal-${text}`, `${p.name}: ${text}. That was you, my lord.`, { mood: /crash|famine|dark|poor/.test(text) ? 'panic' : 'happy', priority: 1, cooldown: 12 });
  }
};

let uiTimer = 0;
let lastListKey = '';
function updateCosmosUi(dt) {
  uiTimer -= dt;
  if (uiTimer > 0 || !G) return;
  uiTimer = 0.2;
  $('zoomLabel').textContent = cosmos.levelLabel();
  $('zoomIn').disabled = cosmos.target <= cosmos.minLevel();
  $('zoomOut').disabled = cosmos.target >= 2;
  const gal = cosmos.galaxy;
  const show = !!gal && (cosmos.target >= 1 || cosmos.z > 0.5 || G.sandbox);
  $('galaxyPanel').hidden = !show;
  if (!show) return;
  $('makePlanet').disabled = gal.full;
  const rows = gal.planets.map((p) => {
    const mirrored = p.home && !p.autonomous;
    const era = p.pop > 0 ? ERA_LABEL[ERA_NAMES[p.eraIndex]] : 'Barren';
    const owner = gal.attended === p ? `${ownerLabel(p)} · you're here` : ownerLabel(p);
    return { p, era: p.forming ? 'Forming' : era, pop: `${mirrored ? `${p.pop} people` : fmtPop(p.pop)} · ${owner}`, prog: mirrored ? (p.eraIndex + 1) / 4 : p.pop > 0 ? p.progress : 0 };
  });
  const key = rows.map((r) => `${r.p.name}|${r.era}|${r.pop}|${Math.round(r.prog * 20)}|${gal.attended === r.p}|${cosmos.target}|${r.p.effect}|${r.p.channel}`).join(';');
  if (key !== lastListKey) {
    lastListKey = key;
    $('planetList').replaceChildren(...rows.map(({ p, era, pop, prog }, i) => {
      const li = document.createElement('li');
      const b = document.createElement('button');
      b.type = 'button'; b.className = 'planet-row'; b.dataset.index = String(i);
      if (gal.attended === p) b.setAttribute('aria-current', 'true');
      const dot = document.createElement('span'); dot.className = 'planet-dot'; dot.style.background = planetDot(p);
      const name = document.createElement('span'); name.className = 'planet-name'; name.textContent = p.name;
      const meta = document.createElement('span'); meta.className = 'planet-meta'; meta.textContent = `${era} · ${pop}`;
      const bar = document.createElement('span'); bar.className = 'planet-bar'; bar.style.setProperty('--p', `${Math.round(prog * 100)}%`);
      const ch = CHANNELS[p.channel];
      const chip = document.createElement('span'); chip.className = 'planet-chip';
      if (ch && !p.home && p.channel !== 'mixed') { chip.textContent = ch.label; chip.style.color = ch.color; chip.style.borderColor = ch.color; }
      const fx = document.createElement('span'); fx.className = 'planet-effect'; fx.textContent = p.forming && p.mindSlot && !gal.mindReady ? 'Shaped by your first 10 seconds' : p.effect || '';
      b.append(dot, name, chip, meta, fx, bar);
      li.append(b);
      return li;
    }));
  }
  const m = gal.mind;
  $('mindLine').textContent = m.done
    ? `Read from your first 10 s: calm ${Math.round(m.calm * 100)}%, focus ${Math.round(m.focus * 100)}%, pulse ${m.hrDelta >= 0 ? '+' : ''}${Math.round(m.hrDelta)}, ${m.blinks} blinks.`
    : `Reading your mind: ${Math.max(0, Math.ceil(SIM.MIND_SECONDS - m.t))} s of play left.`;
  $('galaxyLog').replaceChildren(...gal.log.slice(0, 4).map((l) => { const li = document.createElement('li'); li.textContent = l.text; return li; }));
}

$('zoomIn').addEventListener('click', () => { if (G) { audio.unlock(); zoomInOnSurface(); } });
$('zoomOut').addEventListener('click', () => { if (G) { audio.unlock(); zoomOutFromSurface(); } });
$('makePlanet').addEventListener('click', () => {
  if (!G) return;
  audio.unlock();
  const gal = cosmos.ensureGalaxy(G);
  const p = gal.makePlanet();
  if (!p) return;
  audio.good();
  lastListKey = '';
  advisor.say('newplanet', `A new planet! I have named it ${p.name}. ${p.pop > 0 ? 'Someone already lives there. They seem nice.' : 'Nobody lives there yet. Send a ship.'}`, { mood: 'happy', priority: 2, cooldown: 6 });
});
$('planetList').addEventListener('click', (e) => {
  const b = e.target.closest('.planet-row');
  if (!b || !G || !cosmos.galaxy) return;
  const p = cosmos.galaxy.planets[Number(b.dataset.index)];
  if (p && p.forming) { cosmos.say('Still forming. Give it a second.'); return; }
  if (p) { cosmos.focusPlanet(p, G); audio.blip(); lastListKey = ''; }
});

function openGalaxy({ fromEnding = false } = {}) {
  const gal = cosmos.ensureGalaxy(G);
  if (fromEnding && !gal.planets.some((p) => p.name === 'New Coop')) {
    const pax = G.extra?.passengers ?? 8;
    gal.makePlanet({ name: 'New Coop', biome: 'alien', inhabited: true, eraIndex: 1, pop: pax + 1, size: 11 });
    gal.makePlanet({ inhabited: false });
  }
  cosmos.focus = null;
  if (cosmos.z < 1) cosmos.z = 1;
  cosmos.target = 2;
}
$('galaxyBtn').addEventListener('click', () => {
  $('endScreen').hidden = true;
  audio.unlock();
  openGalaxy({ fromEnding: true });
  advisor.say('galaxy', 'Behold, my lord: the galaxy. Make planets. Keep your pulse down near them.', { mood: 'happy', priority: 3, cooldown: 999 });
});

// ---------- title screen ----------
function begin(source) {
  audio.unlock();
  $('titleScreen').hidden = true;
  newGame(source);
}
$('playSim').addEventListener('click', () => begin(previewSource?.kind === 'bridge' && previewSource.connected ? previewSource : new SimSource()));
$('playSandbox').addEventListener('click', () => {
  begin(previewSource?.kind === 'bridge' && previewSource.connected ? previewSource : new SimSource());
  dismissBanner();
  G.sandbox = true;
  cosmos.sandbox = true;
  cosmos.galaxy = null;
  cosmos.ensureGalaxy(G, { autonomousHome: true });
  cosmos.z = 1; cosmos.target = 2;
  advisor.setEra('space');
  advisor.say('sandbox', 'Galaxy sandbox, my lord. Three worlds are forming from your thoughts. Make more planets and let them run.', { mood: 'happy', priority: 3, cooldown: 999 });
});
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
window.__moc = { get G() { return G; }, startScene: (i) => startScene(i), cosmos, get engine() { return engine; } };

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
