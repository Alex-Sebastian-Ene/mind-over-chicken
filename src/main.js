// Mind over Chicken: boot, scene flow, render pipeline and input.

import { W, H, HOME_W, HOME_H, clamp, wrapDelta } from './sim/util.js';
import { World, setRustWorldGen } from './world/terrain.js';
import { SimSource, BridgeSource } from './brain/sources.js';
import { Brain, setRustBrain } from './brain/brain.js';
import { Advisor } from './ui/advisor.js';
import { Hud } from './ui/hud.js';
import { EegPanel } from './ui/eeg-panel.js';
import { Audio } from './audio.js';
import { StoneAge } from './eras/stone.js';
import { FarmingToKingdoms, InfoAge } from './eras/timeskip.js';
import { Industry } from './eras/industry.js';
import { SpaceAge } from './eras/space.js';
import initEngine, { Engine, RustImuTracker, RustBlinkClassifier, PointerGesture, ViewCamera, RustWorldGen, RustCastController, RustFingerController, RustEmotionTracker, RustGalaxy, RustAiPolicy, RustBrain, RustSimSource, RustCalibration } from '../pkg/mind_over_chicken_engine.js';
import { Cosmos } from './cosmos/cosmos.js';
import { ERA_LABEL, ERA_NAMES, CHANNELS, fmtPop, planetDot, ownerLabel, setRustGalaxy } from './cosmos/galaxy.js';
import { initCamera } from './camera/camera.js'; // camera (#33)
import { setRustCastController } from './camera/casts.js';
import { setRustFingerController } from './camera/fingers.js';
import { setRustEmotionTracker } from './camera/emotion.js';
import { setRustAiPolicy } from './ai/policy.js';
import { initDebug } from './camera/debug.js'; // camera debug (#67)
import { Calibration, readCalibrationConfig } from './brain/calibration.js'; // calibration (#54)
import { SIM } from './cosmos/planet-sim.js';
import { CivSandbox } from './civ/sandbox.js'; // civ (#50)
import { worldMeta, renderWorldDetail } from './cosmos/panel.js'; // planets (#68)
import { SimPlanet } from './ai/planet.js';
import { AIGod } from './ai/autopilot.js';

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
let viewCam = null;
let gesture = null; // Rust (input.rs): with the main button or a finger, a tap blinks and a drag pans
// Pan the map by a drag (world units), both axes, wherever you are: a scene with its own camera
// (the civ sandbox's big planet) takes it directly; otherwise the planet pans east–west (and
// north–south on vast worlds), and in 3D the camera also moves north–south over the map.
// 3D: the ground point you pressed on stays under the cursor (raycast in Rust), at any zoom/orbit.
let grab3d = null;
function groundAt(e) {
  const r = screen.getBoundingClientRect();
  const h = engine.screen_to_ground(e.clientX - r.left, e.clientY - r.top, r.width, r.height);
  return h && h[0] >= 0 ? { x: h[0], z: h[1] } : null;
}
function grabPan3d(e) {
  if (!grab3d) return false;
  // Over hills, moving the camera changes which slope the ray hits, so refine a few times until
  // the grabbed ground point is back under the cursor (each pass is one cheap Rust raycast).
  for (let pass = 0; pass < 3; pass++) {
    const g = groundAt(e);
    if (!g) break;
    // Wrap-aware difference on both axes (the world is a torus, #111), then move the one camera
    // (2D window and 3D target, #129) by it.
    const dx = wrapDelta(grab3d.x, g.x, cosmos.worldW), dz = wrapDelta(grab3d.z, g.z, cosmos.worldH);
    if (Math.abs(dx) < 0.05 && Math.abs(dz) < 0.05) break;
    cosmos.pan(-dx, -dz);
    // Apply now, not at the next frame: a fast mouse sends several moves per frame, and each one
    // must raycast against the camera it already moved.
    engine.set_camera_target(cosmos.view.center_x(), cosmos.view.center_y());
  }
  cam3dBase = null;
  return true;
}
function panMap(dx, dy) {
  cosmos.pan(dx, dy);   // both axes; the 3D target follows the 2D camera (#129)
  cam3dBase = null;
}

try {
  await initEngine();
  setRustWorldGen(RustWorldGen);
  setRustCastController(RustCastController);
  setRustFingerController(RustFingerController);
  setRustEmotionTracker(RustEmotionTracker);
  setRustGalaxy(RustGalaxy);
  setRustAiPolicy(RustAiPolicy);
  setRustBrain(RustBrain);
  gesture = new PointerGesture(6);
  viewCam = new ViewCamera(HOME_W, HOME_H, W, H); // the 320×180 window onto the home world (#129)
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
const jsAudio = new Audio();
const audio = {
  unlock() { if (engine) engine.audio_unlock(); else jsAudio.unlock(); },
  toggle() { return engine ? engine.audio_toggle() : jsAudio.toggle(); },
  thunder() { if (engine) engine.audio_play('thunder'); else jsAudio.thunder(); },
  ignite() { if (engine) engine.audio_play('ignite'); else jsAudio.ignite(); },
  blip() { if (engine) engine.audio_play('blip'); else jsAudio.blip(); },
  good() { if (engine) engine.audio_play('good'); else jsAudio.good(); },
  bad() { if (engine) engine.audio_play('bad'); else jsAudio.bad(); },
  pop() { if (engine) engine.audio_play('pop'); else jsAudio.pop(); },
  coin() { if (engine) engine.audio_play('coin'); else jsAudio.coin(); },
  bell() { if (engine) engine.audio_play('bell'); else jsAudio.bell(); },
  crash() { if (engine) engine.audio_play('crash'); else jsAudio.crash(); },
  tick() { if (engine) engine.audio_play('tick'); else jsAudio.tick(); },
  rumble(dur) { if (engine) engine.audio_play('rumble'); else jsAudio.rumble(dur); },
  fanfare() { if (engine) engine.audio_play('fanfare'); else jsAudio.fanfare(); },
};
const advisor = new Advisor($('advisor'));
const hud = new Hud(document);
const cosmos = new Cosmos();
cosmos.setViewCamera(viewCam);
// A scene with its own camera (the civ sandbox's planet) takes every pan: drag, keys, wheel.
cosmos.panSink = (dx, dy) => { if (G?.scene?.panBy && !aiPlanet) { G.scene.panBy(dx, dy); return true; } return false; };
// What the HUD shows while you look at the galaxy.
const galaxyHudScene = { label: 'Galaxy', powers: ['calm', 'focus', 'stress'], objectives: () => cosmos.galaxy?.objectives() || [] };
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

// A side panel that fails to start must not take the whole game (and every button) down.
let eegPanel = null;
try {
  eegPanel = new EegPanel(document, {
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
} catch (err) {
  console.error('EEG panel failed to start; the game runs without it:', err);
}
let fade = 0;           // 0..1 black overlay for transitions
let pendingScene = null;
let pendingTimer = 0;
let banner = null;      // {t}
let lastTs = 0;

// ---------- AI god (PPO) ----------
let aiPlanet = null;
function toggleAIGod() {
  if (aiPlanet) {
    aiPlanet.attachHuman(null);
    aiPlanet = null;
    const btn = $('aiGodBtn');
    if (btn) {
      btn.textContent = 'AI God: OFF';
      btn.setAttribute('aria-pressed', 'false');
    }
    advisor.say('ai-off', 'AI god disengaged. You have full command.', { mood: 'happy', priority: 2, cooldown: 1 });
  } else {
    aiPlanet = new SimPlanet({ seed: G?.world?.seed || 42, brain: 'proxy' }).setGod(new AIGod());
    if (G?.brain) aiPlanet.attachHuman(G.brain);
    const btn = $('aiGodBtn');
    if (btn) {
      btn.textContent = 'AI God: ON';
      btn.setAttribute('aria-pressed', 'true');
    }
    advisor.say('ai-on', 'AI god engaged! The PPO autopilot is steering this world. Stay calm to help it.', { mood: 'nervous', priority: 2, cooldown: 1 });
  }
}

function newGame(source, startIndex = 0) {
  if (previewSource && previewSource !== source) previewSource.disconnect();
  previewSource = null;
  previewBrain = null;
  if (G?.source && G.source !== source) G.source.disconnect();
  const brain = new Brain(source);
  if (aiPlanet) aiPlanet.attachHuman(brain);
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
    if (engine) engine.game_on_blink(aim.x * cosmos.worldW, aim.y * cosmos.worldH);
    cosmos.galaxy?.onBlink();   // storms on the blink world; a strike on the world you're zoomed into
    if (cosmos.galaxy?.attended) audio.thunder();
    if (aiPlanet) aiPlanet.humanBlink(aim);
    const view = cosmos.viewScene(G); // planets (#68): another world's surface when you're on it
    if (!banner && !cosmos.active && view?.onBlink) view.onBlink(aim);
  });
  // The galaxy exists from the start: home plus three worlds the first 10 s of play will shape.
  G.camera = cosmos;          // the eras ask it where the screen is (#129)
  cosmos.attachWorld(G.world); // the camera looks at the village
  cosmos.ensureGalaxy(G);
  brain.onTrained = () => {};
  $('endScreen').hidden = true;
  $('failScreen').hidden = true;
  if (engine && G.world) {
    engine.init_world(G.world.seed);
    // The 3D relief must be the world the simulation runs on (#71). Older pkg builds lack this.
    if (engine.set_heightmap) {
      try { engine.set_heightmap(G.world.h, G.world.width || 0, G.world.height || 0); } catch (err) { console.warn('set_heightmap failed:', err); }
    }
    // Around the 3D map: this world's deep sea, so the edges read as open ocean.
    const deep = G.world.palette?.[0]?.[0];
    if (deep && engine.set_backdrop) engine.set_backdrop(deep[0], deep[1], deep[2]);
    engine.set_era(startIndex);
    engine.game_start_era(Math.min(2, Math.floor(startIndex / 2)));
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
  if (engine) {
    engine.set_era(i);
    engine.game_start_era(Math.min(2, Math.floor(i / 2)));
  }
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
  if (engine) {
    engine.set_era(i);
    engine.game_restart_era();
  }
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
  banner = { t: 0, manual: (i === 0) };
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
  let heading = 'Collapse';
  let cause = 'The era has collapsed.';
  let nextStep = 'Try again with focused calm and steady aim.';

  if (typeof reason === 'string') {
    if (reason.includes(': ')) {
      const parts = reason.split(': ');
      cause = parts[0].trim();
      nextStep = parts.slice(1).join(': ').trim();
      heading = cause;
    } else {
      cause = reason;
      heading = reason;
      const lower = reason.toLowerCase();
      if (lower.includes('strike') || lower.includes('union')) {
        nextStep = 'Maintain calm when workers strike to settle tensions and resume production.';
      } else if (lower.includes('depression') || lower.includes('price')) {
        nextStep = 'Steady your breathing and focus to stimulate goat market confidence.';
      } else if (lower.includes('fire')) {
        nextStep = 'Stay calm when the flame begins to fade to rekindle warmth.';
      } else if (lower.includes('starv') || lower.includes('food')) {
        nextStep = 'Strike mammoths with lightning so hunters can gather meat for the tribe.';
      } else if (lower.includes('dark')) {
        nextStep = 'Blink directly on the fire pit to strike and kindle the flame.';
      }
    }
  } else if (reason && typeof reason === 'object') {
    heading = reason.heading || reason.title || 'Collapse';
    cause = reason.cause || reason.message || 'The tribe was overwhelmed.';
    nextStep = reason.nextStep || reason.advice || 'Retry the era.';
  }

  $('failHeading').textContent = heading;
  $('failCause').textContent = cause;
  $('failAdvice').textContent = nextStep;
  $('failScreen').hidden = false;
  $('biometricFeedbackAck').hidden = true;
  updateAssistUI();

  audio.bad();
  clearTimeout(failTimer);
  failTimer = null; // No silent auto-retry (#86)
}

function updateAssistUI() {
  const el = $('assistState');
  if (el) {
    el.textContent = G?.assistMode ? 'ON (Easier)' : 'Off';
    el.style.color = G?.assistMode ? '#70d6ff' : 'inherit';
  }
}

function toggleAssistMode() {
  if (!G) return;
  G.assistMode = !G.assistMode;
  if (engine?.set_assist_mode) engine.set_assist_mode(G.assistMode);
  updateAssistUI();
}

function quitToTitle() {
  clearTimeout(failTimer);
  failTimer = null;
  $('failScreen').hidden = true;
  $('banner').hidden = true;
  banner = null;
  G.scene = null;
  $('titleScreen').hidden = false;
}

function retryEra() {
  clearTimeout(failTimer);
  failTimer = null;
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
// The home world is bigger than the screen (#129). Its scenes (the eras, and the AI god's Stone
// Age) paint the whole map into `worldCanvas` in world pixels; the camera's 320×180 window is
// then cut out of it into `low` (2D), or the whole canvas drapes over the 3D relief. Anything
// fixed to the screen (charts, tickers, captions) goes through drawScreen / drawScreenOverlay.
const WORLD_SCENES = [StoneAge, FarmingToKingdoms, Industry, InfoAge, SpaceAge];
const worldCanvas = document.createElement('canvas');
const worldCtx = worldCanvas.getContext('2d');
function worldScene(scene) { return !!scene && scene !== cosmos.surfaceInView && WORLD_SCENES.some((c) => scene instanceof c) && !!scene.G?.world; }
function sizeWorldCanvas(world) {
  const w = world.width || W, h = world.height || H;
  if (worldCanvas.width !== w || worldCanvas.height !== h) { worldCanvas.width = w; worldCanvas.height = h; }
}
// Lights for the 2D shader are in screen pixels: repeat each at every wrap copy that reaches the view.
function addLights2d(scene, offsets) {
  for (const l of scene.lights()) {
    for (const [ox, oy] of offsets) {
      const x = l.x + ox, y = l.y + oy;
      if (x > -l.r && y > -l.r && x < W + l.r && y < H + l.r) engine.add_light(x, y, l.r, 1.0);
    }
  }
}
// Small map of the whole world with the camera's window on it (#129), in the screen's corner.
const MINI = { w: 64, h: 36, pad: 4 };
const miniCache = { layer: null, canvas: null };
function drawMinimap(ctx, scene) {
  const world = scene.G.world, layer = scene.layer || world.layers?.stone;
  if (!layer || !cosmos.view) return;
  if (miniCache.layer !== layer) {
    const c = miniCache.canvas || document.createElement('canvas');
    c.width = MINI.w; c.height = MINI.h;
    const cx = c.getContext('2d');
    cx.imageSmoothingEnabled = true;
    cx.drawImage(layer, 0, 0, MINI.w, MINI.h);
    miniCache.layer = layer; miniCache.canvas = c;
  }
  const x0 = W - MINI.w - MINI.pad, y0 = MINI.pad;   // top-right: the bottom is the coach's and the HUD's
  ctx.globalAlpha = 0.9;
  ctx.fillStyle = 'rgba(8,10,24,0.85)';
  ctx.fillRect(x0 - 1, y0 - 1, MINI.w + 2, MINI.h + 2);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(miniCache.canvas, x0, y0);
  // The camera's window, cut at the seams like the view itself.
  const sx = MINI.w / (world.width || W), sy = MINI.h / (world.height || H);
  const r = cosmos.view.blits();
  ctx.strokeStyle = '#ffe066';
  ctx.lineWidth = 0.6;
  for (let i = 0; i < r.length; i += 6) ctx.strokeRect(x0 + r[i] * sx, y0 + r[i + 1] * sy, r[i + 2] * sx, r[i + 3] * sy);
  const v = world.sites?.village;
  if (v) { ctx.fillStyle = '#ff9a2e'; ctx.fillRect(x0 + v.x * sx - 0.75, y0 + v.y * sy - 0.75, 1.5, 1.5); }
  ctx.globalAlpha = 1;
}

let perfSample = { n: 0, ms: 0 };
function render() {
  const t0 = performance.now();
  const scene = aiPlanet ? aiPlanet.scene : (G ? cosmos.viewScene(G) : null); // AI god autopilot, else the world in view (planets #68)
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
  lowCtx.clearRect(0, 0, W, H);
  const windowed = worldScene(scene);
  // 3D only on the surface itself: once the globe zoom starts, the flat 2D map takes over (the
  // 3D camera has already tipped over to that exact frame, so the hand-over is seamless).
  G.render3d = in3dSurface();
  if (G.render3d && cosmos.view) {
    // One camera for both views: the 3D target is the centre of the 2D window (#129).
    engine.set_camera_target(cosmos.view.center_x(), cosmos.view.center_y());
    stepCam3dTween();
  }
  // In 3D, scenes with draw3d stand their actors up as sprites, so they skip painting them flat.
  let ctx = lowCtx;
  if (windowed) {
    sizeWorldCanvas(scene.G.world);
    worldCtx.imageSmoothingEnabled = false;
    worldCtx.setTransform(1, 0, 0, 1, 0, 0);
    worldCtx.clearRect(0, 0, worldCanvas.width, worldCanvas.height);
    ctx = worldCtx;
  }
  scene.draw(ctx);
  scene.drawAfterLight?.(ctx);
  if (windowed && !G.render3d) cosmos.blitView(worldCanvas, low);
  if (!G.render3d) scene.drawScreen?.(lowCtx);
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
  // World-space lights and labels repeat at every copy of the world that reaches the screen, so
  // they follow the camera and wrap with the map. In 3D, labels are projected instead.
  const offsets = windowed && !G.render3d ? cosmos.overlayOffsets() : [[0, 0]];

  if (engine) {
    try {
      if (surface && G.render3d) {
        engine.upload_terrain_canvas(windowed ? worldCanvas : low); // the 3D terrain texture (#129)
        const near = windowed ? near3d(scene.G.world) : null;
        G.world?.draw3d?.(engine, near);
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
        if (surface && scene.lights) addLights2d(scene, offsets);
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
    if (G.render3d && engine && typeof engine.project_to_screen === 'function') {
      textCtx.__project3d = (wx, wz) => {
        const p = engine.project_to_screen(wx, wz, textLayer.width, textLayer.height);
        if (p[0] < -9000) return null;
        return [p[0] / K, p[1] / K];
      };
    } else {
      textCtx.__project3d = null;
    }
    for (const [ox, oy] of offsets) {
      textCtx.setTransform(K, 0, 0, K, ox * K, oy * K);
      scene.drawOverlay?.(textCtx);
    }
    textCtx.__project3d = null;
    textCtx.setTransform(K, 0, 0, K, 0, 0);
    if (G.render3d) scene.drawScreen?.(textCtx);   // 3D has no low-res frame to paint into
    scene.drawScreenOverlay?.(textCtx);
    if (windowed && !cosmic) drawMinimap(textCtx, scene);
  }
  textCtx.setTransform(K, 0, 0, K, 0, 0);
  textCtx.globalAlpha = visible;
  if (cosmic) cosmos.overlay(textCtx, G);
  textCtx.globalAlpha = 1;
  perfSample.n++; perfSample.ms += performance.now() - t0;
}

// 3D: only the trees and buildings near the camera target are sent as billboards (the world is
// four screens big, #129). Radius from the camera distance, generous for the tilted far edge.
function near3d(world) {
  const tx = engine.get_camera_target_x(), tz = engine.get_camera_target_z();
  const d = engine.get_camera_distance();
  const rx = d * 0.62 + 40, rz = d * 0.5 + 40;
  const ww = world.width, wh = world.height;
  return (x, y) => Math.abs(wrapDelta(x, tx, ww)) < rx && Math.abs(wrapDelta(y, tz, wh)) < rz;
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
    G.brain.update(dt, G.mood);   // the camera's face anger counts toward brain.stress
    if (engine) {
      const b = G.brain;
      engine.game_tick(dt, b.calm || 0, b.focus || 0, b.stress ?? 0, b.stats?.blinks || 0);
    }
    if (banner) {
      banner.t += dt;
      if (!banner.manual && banner.t > 8) dismissBanner();
    } else if (G.sandbox) {
      G.runTime += dt;   // the hand-played game stays paused in the sandbox
      G.calibration?.update(dt, G.brain, G.mood);
    } else if ($('failScreen').hidden && $('endScreen').hidden) {
      G.runTime += dt;
      G.calibration?.update(dt, G.brain, G.mood);
      if (aiPlanet) aiPlanet.update(dt);
      G.scene.update(dt);
    } else if (!$('endScreen').hidden) {
      if (aiPlanet) aiPlanet.update(dt);
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
    // planets (#68): the HUD names the world you're on: "your world" at home, "AI god" elsewhere.
    const wanted = cosmos.hudScene(G, galaxyHudScene);
    if (wanted !== hudScene) { hudScene = wanted; hud.setScene(wanted); }
    hud.update(dt, wanted === G.scene ? G : Object.assign(Object.create(G), { scene: wanted }));
    updateCosmosUi(dt);
  } else if (previewBrain) {
    previewBrain.update(dt);
    $('sourceStatus').textContent = previewSource.status;
    $('sourceStatus').dataset.ok = previewSource.connected ? '1' : '0';
  }
  eegPanel?.update();
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
      return { x: hit[0] / cosmos.worldW, y: hit[1] / cosmos.worldH };
    }
  }
  // 2D: screen → world through the camera (#129). Aims are 0..1 of the world, like the brain's.
  const a = aimFromEvent(e);
  if (cosmos.surfaceInView || !cosmos.view) return a;
  const p = cosmos.toWorld(a.x * W, a.y * H);
  return { x: p.x / cosmos.worldW, y: p.y / cosmos.worldH };
}
let panDrag = null;
let orbitDrag = null;
// ---------- 3D camera <-> globe hand-off ----------
// The globe zoom (cosmos) starts from the flat 2D map. So zooming the 3D camera out past its
// default distance tips it, smoothly, to look straight down, north-up, at the whole map: exactly
// the 2D frame. Only then does the globe take over. Zooming back in reverses it.
// topDown is where the 30 degree lens frames the 180-tile-high view at sea level (y = 7.92):
// 90 / tan(15 deg) + 7.92 = 344. The world is bigger (#129), but the camera target is the centre of
// the 2D window, so the top-down 3D frame is exactly the 2D view at hand-over.
const CAM3D = { blendFrom: 300, topDown: 344, topPitch: 1.55 };
let cam3dBase = null;   // the player's own angle when the tip-over began
let cam3dTween = null;  // a keyboard/button zoom-out animates the tip-over, then hands over
// 3D is the home world's relief; other worlds' surfaces (planets #68) stay 2D until they get their own mesh.
function in3dSurface() { return !!(engine && engine.is_3d_mode() && !cosmos.active && !((cosmos.z || 0) > 0) && (!G || !cosmos.viewScene || cosmos.viewScene(G) === G.scene) && !(G?.scene instanceof CivSandbox)); } // civ (#50) runs on its own planet: 2D
function applyCam3dBlend() {
  const d = engine.get_camera_distance();
  const t = clamp((d - CAM3D.blendFrom) / (CAM3D.topDown - CAM3D.blendFrom));
  if (t <= 0) {
    if (cam3dBase) {
      engine.orbit_camera(cam3dBase.yaw - engine.get_camera_yaw(), cam3dBase.pitch - engine.get_camera_pitch());
      cam3dBase = null;
    }
    return;
  }
  if (!cam3dBase) cam3dBase = { pitch: engine.get_camera_pitch(), yaw: engine.get_camera_yaw() };
  const e = t * t * (3 - 2 * t);
  const northUp = Math.round(cam3dBase.yaw / (2 * Math.PI)) * 2 * Math.PI;
  const yaw = cam3dBase.yaw + (northUp - cam3dBase.yaw) * e;
  const pitch = cam3dBase.pitch + (CAM3D.topPitch - cam3dBase.pitch) * e;
  engine.orbit_camera(yaw - engine.get_camera_yaw(), pitch - engine.get_camera_pitch());
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
  if (gesture?.owns(e.pointerId)) {
    const q = worldFromEvent(e);
    if (gesture.moved(e.pointerId, e.clientX, e.clientY, q.x, q.y)) {
      if (!grabPan3d(e)) panMap(gesture.pan_dx(), gesture.pan_dy());
      return;
    }
  }
  if (panDrag && panDrag.id === e.pointerId) {
    // East–west, plus north–south on the vast worlds (planets #68). In 3D the camera follows the pan.
    const q = worldFromEvent(e);
    panMap(q.x - panDrag.x, q.y - (panDrag.y ?? q.y));
    panDrag.x = q.x; panDrag.y = q.y;
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
  // Right-drag or Alt-drag orbits the camera, in 3D only. (Shift is the Stress key.)
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
    const q0 = worldFromEvent(e);
    panDrag = { id: e.pointerId, x: q0.x, y: q0.y };
    return;
  }
  // Right drag in 2D mode pans surface
  if (G && e.button === 2) {
    e.preventDefault();
    screen.setPointerCapture?.(e.pointerId);
    const q = worldFromEvent(e);
    panDrag = { id: e.pointerId, x: q.x, y: q.y };
    return;
  }
  const s = sim();
  if (gesture && G && e.button === 0) {
    try { screen.setPointerCapture?.(e.pointerId); } catch {}
    const q0 = worldFromEvent(e);
    gesture.down(e.pointerId, e.clientX, e.clientY, q0.x, q0.y);
    grab3d = in3dSurface() ? groundAt(e) : null;
    if (s) { const a = surfaceAim(e); s.setAim(a.x, a.y); G.brain.aim.x = a.x; G.brain.aim.y = a.y; }
    return; // the blink happens on release, if Rust says it was a tap
  }
  if (!s) return;
  const a = surfaceAim(e);
  s.setAim(a.x, a.y);
  G.brain.aim.x = a.x; G.brain.aim.y = a.y;
  s.blink();
});
const endPointer = (e) => {
  if (gesture?.owns(e.pointerId)) {
    const s = sim();
    if (gesture.up(e.pointerId, e.type === 'pointercancel') && s && G) {
      const a = surfaceAim(e);
      s.setAim(a.x, a.y);
      G.brain.aim.x = a.x; G.brain.aim.y = a.y;
      s.blink();
    }
    return;
  }
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

// Shift is the Stress demo key: full agitation on the emulated headset (input.panic).
const KEYS = { Space: 'calm', KeyF: 'focus', ShiftLeft: 'panic', ShiftRight: 'panic' };
window.addEventListener('keydown', (e) => {
  if (e.target.closest?.('input')) return;
  if (!$('failScreen').hidden) {
    if (e.code === 'Enter') { e.preventDefault(); retryEra(); return; }
    if (e.code === 'KeyA') { e.preventDefault(); toggleAssistMode(); return; }
    if (e.key === '?' || e.code === 'KeyH') {
      e.preventDefault();
      $('failScreen').hidden = true;
      if (G?.scene?.intro) showBanner(G.scene, G.sceneIndex);
      return;
    }
    if (e.code === 'Escape' || e.code === 'KeyQ') {
      e.preventDefault();
      quitToTitle();
      return;
    }
  }
  if (e.code === 'KeyM') { toggleModel(); return; }
  if (e.code === 'Escape' && eegPanel?.wiz?.active) { eegPanel?.closeWizard(); return; }
  if (e.code === 'KeyE') { eegPanel?.toggle(); return; }
  if (e.code === 'KeyC') { eegPanel?.calibrateCenter(); return; }
  if (e.code === 'KeyT') { toggle3D(); return; }
  if (e.code === 'KeyG') { toggleAIGod(); return; }
  if (G && (e.code === 'KeyZ' || e.code === 'Minus' || e.code === 'NumpadSubtract')) { zoomOutFromSurface(); return; }
  if (G && (e.code === 'KeyX' || e.code === 'Equal' || e.code === 'NumpadAdd')) { zoomInOnSurface(); return; }
  if (G && !cosmos.active && (e.code === 'KeyA' || e.code === 'KeyD')) { cosmos.panVel = e.code === 'KeyA' ? 140 : -140; return; }
  if (G && !cosmos.active && (e.code === 'KeyW' || e.code === 'KeyS')) { cosmos.panVelY = e.code === 'KeyW' ? 110 : -110; return; } // planets (#68); home pans north–south too (#129)
  if (G && e.code === 'KeyN' && !e.repeat) { if (cosmos.visitNextWorld(G)) audio.blip(); return; } // planets (#68, #43): visit the next AI world (G is the AI God toggle)
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
  const map = { calm: 'calm', focus: 'focus', stress: 'panic' };
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
  if (!p.hidden) {
    eegPanel?.toggle(false);
    window.__moc?.camera?.togglePanel?.(false);
  }
}
$('modelBtn').addEventListener('click', toggleModel);
$('modelClose').addEventListener('click', toggleModel);

function toggle3D() {
  if (!engine || !has3d) return;
  engine.set_3d_mode(!engine.is_3d_mode());
  cam3dBase = null; cam3dTween = null;
  sync3dButton();
  const on = engine.is_3d_mode();
  advisor.say('3d-toggle', on ? '3D view: drag to pan, right-drag to orbit, wheel to zoom.' : '2D classic view.', { mood: 'happy', priority: 2, cooldown: 1 });
}
$('mode3dBtn')?.addEventListener('click', toggle3D);
$('aiGodBtn')?.addEventListener('click', toggleAIGod);
$('soundBtn').addEventListener('click', () => {
  audio.unlock();
  const on = audio.toggle();
  $('soundBtn').textContent = on ? 'Sound on' : 'Sound off';
  $('soundBtn').setAttribute('aria-pressed', String(on));
});
$('bannerGo').addEventListener('click', dismissBanner);
$('retryBtn').addEventListener('click', retryEra);
$('assistBtn')?.addEventListener('click', toggleAssistMode);
$('showHowBtn')?.addEventListener('click', () => {
  $('failScreen').hidden = true;
  if (G?.scene?.intro) showBanner(G.scene, G.sceneIndex);
});
$('quitTitleBtn')?.addEventListener('click', quitToTitle);
$('biometricFeedbackBtn')?.addEventListener('click', () => {
  $('biometricFeedbackAck').hidden = false;
  if (G?.brain) {
    G.brain.toleranceBonus = (G.brain.toleranceBonus || 0) + 0.15;
    advisor?.say('assist', 'Tolerance widened. Take a deep breath; the gods are forgiving today.', { mood: 'happy', priority: 4, cooldown: 0 });
  }
});
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
    const tier = worldMeta(p); // planets (#68): settlement tier and strife
    return { p, era: p.forming ? 'Forming' : tier ? `${era} · ${tier}` : era, pop: `${mirrored ? `${p.pop} people` : fmtPop(p.pop)} · ${owner}`, prog: mirrored ? (p.eraIndex + 1) / 4 : p.pop > 0 ? p.progress : 0 };
  });
  renderWorldDetail($('worldDetail'), cosmos.detailPlanet()); // planets (#68): milestones and history
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
    ? `Read from your first 10 s: calm ${Math.round(m.calm * 100)}%, focus ${Math.round(m.focus * 100)}%, stress ${Math.round((m.stress || 0) * 100)}%, ${m.blinks} blinks.`
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
  advisor.say('galaxy', 'Behold, my lord: the galaxy. Make planets. Stay calm near them.', { mood: 'happy', priority: 3, cooldown: 999 });
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
// civ (#50): "Sandbox: four civilisations" runs its own scene (Rust sim in src/civ/) on the emulated headset.
$('playCiv').addEventListener('click', () => {
  begin(new SimSource());
  const scene = new CivSandbox({ cosmos });
  G.scene = scene;
  advisor.setEra(scene.era);
  scene.enter(G);
  hud.setScene(scene);
  document.body.dataset.era = scene.era;
  showBanner(scene, -1);
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

// camera (#33, #116–#123): opt-in webcam. Hands perform actions (pinch-grab, 2-hand zoom, palm, fist, wave)
// with coach visual demonstration. Face sets mood.
window.__moc.camera = initCamera({
  getG: () => cosmos.castG(G), // planets (#68): casts and mood reach the world you're on
  canCast: () => !!G?.scene && !banner && !G.finished && $('titleScreen').hidden && $('failScreen').hidden && $('endScreen').hidden,
  // camera debug (#67): the fingertip aims on the panned surface like the mouse does.
  // Aims are 0..1 of the world; the camera's overlay draws in 0..1 of the screen (#129).
  mapX: (x) => cosmos.mapX(x * W) / cosmos.worldW,
  screenX: (x) => (cosmos.surfaceInView ? x : cosmos.toScreen(x * cosmos.worldW, 0).x / W),
  screenY: (y) => (cosmos.surfaceInView ? y : cosmos.toScreen(0, y * cosmos.worldH).y / H),
  onZoomDelta: (delta) => {
    // Two-hand continuous pinch zoom (#119)
    try {
      cosmos.onWheel?.({ deltaY: -delta * 120, preventDefault() {} });
    } catch {}
  },
  onWave: (dir) => {
    // Open-hand wave orbits camera (#121)
    if (cosmos.mode === 'planet' || cosmos.mode === 'vast') {
      cosmos.pan(dir * 40, 0);
    }
  },
});
// camera debug (#67): Debug tab (button in the top bar, ` key; D pans the planet).
window.__moc.debug = initDebug({ getG: () => G, camera: window.__moc.camera });

// ---------- boot ----------
async function autoConnectBridge() {
  const active = G?.source || previewSource;
  if (active?.kind === 'bridge' && active.connected) return;
  const url = ($('bridgeUrl')?.value || '').trim() || 'ws://localhost:8765';
  const src = new BridgeSource(url);
  try {
    await src.connect();
    setGameSource(src);
    const status = $('bridgeStatus');
    if (status) status.textContent = 'Connected to Muse 2 bridge.';
  } catch {
    src.disconnect();
  }
}

function start(data = {}) {
  resize();
  lastTs = performance.now();
  requestAnimationFrame(rafLoop);
  if (data && data.kind === 'sim' && data.sceneIndex > 0) {
    $('titleScreen').hidden = true;
    newGame(new SimSource(), data.sceneIndex);
  }
  autoConnectBridge();
}
document.fonts?.load('16px "Pixelify Sans"').finally?.(() => {});
if (window.claude?.hot?.ready) window.claude.hot.ready(start);
else start(window.claude?.hot?.data ?? {});
