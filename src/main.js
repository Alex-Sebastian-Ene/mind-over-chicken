// Mind over Chicken: boot, scene flow, render pipeline and input.

import { W, H, clamp } from './sim/util.js';
import { World, setRustWorldGen } from './world/terrain.js';
import { SimSource, BridgeSource, setRustSimSource } from './brain/sources.js';
import { Brain, setRustBrain } from './brain/brain.js';
import { Advisor, setRustAdvisorEngine } from './ui/advisor.js';
import { Hud, setRustHudEngine } from './ui/hud.js';
import { EegPanel } from './ui/eeg-panel.js';
import { Audio } from './audio.js';
import { StoneAge } from './eras/stone.js';
import { FarmingToKingdoms, InfoAge } from './eras/timeskip.js';
import { Industry } from './eras/industry.js';
import { SpaceAge } from './eras/space.js';
import initEngine, {
  Engine,
  RustApp,
  RustImuTracker,
  RustBlinkClassifier,
  PointerGesture,
  RustWorldGen,
  RustCastController,
  RustFingerController,
  RustEmotionTracker,
  RustGalaxy,
  RustAiPolicy,
  RustBrain,
  RustSimSource,
  RustCalibration,
  rust_villager_look,
  rust_animate_agent,
} from '../pkg/mind_over_chicken_engine.js';
import { Cosmos } from './cosmos/cosmos.js';
import { ERA_LABEL, ERA_NAMES, CHANNELS, fmtPop, planetDot, ownerLabel, setRustGalaxy } from './cosmos/galaxy.js';
import { initCamera } from './camera/camera.js'; // camera (#33)
import { initCameraAdapter } from './camera/adapter.js';
import { setRustCastController } from './camera/casts.js';
import { setRustFingerController } from './camera/fingers.js';
import { setRustEmotionTracker } from './camera/emotion.js';
import { setRustAiPolicy } from './ai/policy.js';
import { setRustAgentHelpers } from './sim/agents.js';
import { initDebug } from './camera/debug.js'; // camera debug (#67)
import { Calibration, readCalibrationConfig, setRustCalibration } from './brain/calibration.js'; // calibration (#54)
import { SIM } from './cosmos/planet-sim.js';
import { CivSandbox } from './civ/sandbox.js'; // civ (#50)
import { worldMeta, renderWorldDetail } from './cosmos/panel.js'; // planets (#68)
import { SimPlanet } from './ai/planet.js';
import { AIGod } from './ai/autopilot.js';
import { loading } from './ui/loading.js'; // loading screen

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
    // Wrap-aware east–west difference, then move the camera by it (x is owned by the planet pan).
    let dx = grab3d.x - g.x; dx = ((dx + W * 1.5) % W) - W / 2;
    const dz = grab3d.z - g.z;
    if (Math.abs(dx) < 0.05 && Math.abs(dz) < 0.05) break;
    if (dx) cosmos.pan(-dx);
    // Apply now, not at the next frame: a fast mouse sends several moves per frame, and each one
    // must raycast against the camera it already moved.
    engine.set_camera_target(cosmos.mapX(W / 2), clamp(engine.get_camera_target_z() + dz, 0, H - 1));
  }
  cam3dBase = null;
  return true;
}
function panMap(dx, dy) {
  if (G?.scene?.panBy) { G.scene.panBy(dx, dy); return; }
  cosmos.pan(dx, dy);
  if (dy && in3dSurface()) {
    engine.set_camera_target(engine.get_camera_target_x(), clamp(engine.get_camera_target_z() - dy, 0, H - 1));
  }
  cam3dBase = null;
}

try {
  loading.start('engine', 'Starting the game engine', { blocking: true }); // loading screen
  await initEngine();
  setRustWorldGen(RustWorldGen);
  setRustCastController(RustCastController);
  setRustFingerController(RustFingerController);
  setRustEmotionTracker(RustEmotionTracker);
  setRustGalaxy(RustGalaxy);
  setRustAiPolicy(RustAiPolicy);
  setRustBrain(RustBrain);
  setRustSimSource(RustSimSource);
  setRustCalibration(RustCalibration);
  setRustAgentHelpers(rust_villager_look, rust_animate_agent);
  gesture = new PointerGesture(6);
  BridgeSource.setWasmClasses(RustImuTracker, RustBlinkClassifier);
  engine = new Engine('screen');
  try {
    engine.bind_dom_shell();
    setRustAdvisorEngine(engine);
    setRustHudEngine(engine);
  } catch (err) {
    console.warn('Rust DOM shell binding failed:', err);
  }
  loading.done('engine');
} catch (err) {
  console.error('Failed to initialize Rust WebGL engine:', err);
  loading.fail('engine', 'no WebGL here: using the 2D fallback');
}

export async function startRustApp() {
  await initEngine();
  const app = new RustApp('screen');
  initCameraAdapter(app);
  app.start();
  window.__moc = { app };
  return app;
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
function resetAIGod() {
  if (!aiPlanet) return;
  aiPlanet.attachHuman(null);
  aiPlanet = null;
  const btn = $('aiGodBtn');
  if (btn) {
    btn.textContent = 'AI God: OFF';
    btn.setAttribute('aria-pressed', 'false');
  }
}
function toggleAIGod() {
  if (!G || !$('titleScreen').hidden) return;
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
  resetAIGod();
  const brain = new Brain(source);
  if (aiPlanet) aiPlanet.attachHuman(brain);
  cosmos.reset();
  G = {
    world: new World(),
    brain, source, advisor, hud, audio, engine,
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
    if (engine) engine.game_on_blink(aim.x * W, aim.y * H);
    cosmos.galaxy?.onBlink();   // storms on the blink world; a strike on the world you're zoomed into
    if (cosmos.galaxy?.attended) audio.thunder();
    if (aiPlanet) aiPlanet.humanBlink(aim);
    const view = cosmos.viewScene(G); // planets (#68): another world's surface when you're on it
    if (!banner && view?.onBlink) view.onBlink(aim);
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
  const go = $('bannerGo');
  if (go) go.innerHTML = 'Begin <kbd>Enter</kbd>';
  el.hidden = false;
  banner = { t: 0, manual: (i === 0) };
  G.brain.setCue(null);
}

function showControlsFromFail() {
  if (!G?.scene?.intro) return;
  $('failScreen').hidden = true;
  showBanner(G.scene, G.sceneIndex);
  banner.manual = true;
  banner.fromFail = true;
  const go = $('bannerGo');
  if (go) go.innerHTML = 'Back <kbd>Esc</kbd>';
}

function dismissBanner() {
  if (!banner) return;
  const fromFail = banner.fromFail;
  banner = null;
  $('banner').hidden = true;
  if (fromFail) { $('failScreen').hidden = false; return; }
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
  $('endScreen').hidden = true;
  banner = null;
  pendingScene = null;
  resetAIGod();
  const keep = G?.source?.kind === 'bridge' && G.source.connected ? G.source : null;
  if (G?.source && G.source !== keep) G.source.disconnect();
  G = null;
  hudScene = null;
  fade = 0;
  setGameSource(keep);
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
function render() {
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
  // 3D only on the surface itself: once the globe zoom starts, the flat 2D map takes over (the
  // 3D camera has already tipped over to that exact frame, so the hand-over is seamless).
  G.render3d = in3dSurface();
  globalThis.__moc_render3d = G.render3d;
  if (G.render3d) {
    // The 3D camera follows the same east–west pan as the 2D map (A/D, Shift+scroll, drag).
    if (scene && scene !== G.scene && typeof scene.centerColumn === 'function') {
      engine.set_camera_target(W / 2, H / 2);
    } else {
      engine.set_camera_target(cosmos.mapX(W / 2), engine.get_camera_target_z());
    }
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
        sync3dWorldMesh(scene);
        engine.begin_frame();
        engine.upload_canvas(low);
        if (scene === G.scene) {
          G.world?.draw3d?.(engine);
          if (typeof engine.draw_era_3d === 'function' && !G.sandbox && G.scene?.era) {
            engine.draw_era_3d();
          } else if (scene.draw3d) {
            scene.draw3d(engine, 0.016);
          }
        } else if (scene?.draw3d) {
          scene.draw3d(engine, 0.016);
        } else if (scene?.lights) {
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
    if (G.render3d && engine && typeof engine.project_to_screen === 'function') {
      textCtx.__project3d = (wx, wz) => {
        const p = engine.project_to_screen(wx, wz, textLayer.width, textLayer.height);
        if (p[0] < -9000) return null;
        return [p[0] / K, p[1] / K];
      };
    } else {
      textCtx.__project3d = null;
    }
    for (const off of offsets) {
      textCtx.setTransform(K, 0, 0, K, off * K, 0);
      scene.drawOverlay?.(textCtx);
    }
    textCtx.__project3d = null;
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
    G.brain.update(dt, G.mood);   // the camera's face anger counts toward brain.stress
    if (engine) {
      const b = G.brain;
      engine.game_tick(dt, b.calm || 0, b.focus || 0, b.stress ?? 0, b.stats?.blinks || 0);
      if (engine.is_game_era_complete?.() && !pendingScene && !G.finished) {
        G.complete(1.5);
      }
      if (engine.is_game_era_failed?.() && $('failScreen').hidden && !G.finished) {
        const failReason = engine.is_game_era_failed();
        if (failReason) {
          showFail(failReason);
        }
      }
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
      G.scene?.update(dt);
    } else if (!$('endScreen').hidden) {
      if (aiPlanet) aiPlanet.update(dt);
      G.scene?.update(dt);
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

function isEegLive() {
  const src = G?.source;
  return !!(src && src.kind === 'bridge' && src.connected && src.streaming && src.lastFrameAt > 0 && (performance.now() - src.lastFrameAt < 3000));
}

function aimFromEvent(e) {
  const r = screen.getBoundingClientRect();
  return { x: clamp((e.clientX - r.left) / r.width), y: clamp((e.clientY - r.top) / r.height) };
}

function worldFromEvent(e) { const a = aimFromEvent(e); return { x: a.x * W, y: a.y * H }; }
// Aim on the surface: in 3D mode, raycast from screen to terrain; in 2D, screen → map position.
function surfaceAim(e) {
  if (cosmos.active) return aimFromEvent(e);
  if (engine && engine.is_3d_mode()) {
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

function followCursorAim(e) {
  if (!G?.brain || isEegLive()) return;
  const a = surfaceAim(e);
  G.brain.aim.x = a.x;
  G.brain.aim.y = a.y;
  const s = sim();
  if (s) s.setAim(a.x, a.y);
  G.scene?.setAim?.(a.x, a.y);
}
window.addEventListener('pointermove', followCursorAim, { passive: true });
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
let current3dWorldKey = '';
function sync3dWorldMesh(scene) {
  if (!engine) return;
  const isOther = scene && scene !== G?.scene && typeof scene.getVisibleHeightmap === 'function';
  const world = isOther ? scene.world : G?.world;
  if (!world) return;
  const qx = Math.round((scene.sx || 0) / 2) * 2;
  const qy = Math.round((scene.sy || 0) / 2) * 2;
  const key = isOther ? `${world.seed}_${qx}_${qy}` : `${world.seed}`;
  if (key === current3dWorldKey) return;
  current3dWorldKey = key;

  const h = isOther ? scene.getVisibleHeightmap() : world.h;
  if (h && engine.set_heightmap) {
    try { engine.set_heightmap(h); } catch (e) { console.warn('set_heightmap failed:', e); }
  }
  const deep = world.palette?.[0]?.[0];
  if (deep && engine.set_backdrop) {
    engine.set_backdrop(deep[0], deep[1], deep[2]);
  }
}

// 3D world: enabled whenever looking at any planet's surface at z <= 0.
function in3dSurface() {
  return !!(engine && engine.is_3d_mode() && !cosmos.active && !((cosmos.z || 0) > 0));
}
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
  followCursorAim(e);
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
function triggerAimClick(e) {
  if (!G) return;
  const a = surfaceAim(e);
  G.brain.aim.x = a.x;
  G.brain.aim.y = a.y;
  const s = sim();
  if (s) {
    s.setAim(a.x, a.y);
  }
  if (!isEegLive()) {
    G.brain.stats.blinks++;
    G.brain.blinkListeners.forEach((fn) => fn(a));
  }
}

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
  if (gesture && G && e.button === 0) {
    try { screen.setPointerCapture?.(e.pointerId); } catch {}
    const q0 = worldFromEvent(e);
    gesture.down(e.pointerId, e.clientX, e.clientY, q0.x, q0.y);
    grab3d = in3dSurface() ? groundAt(e) : null;
    const a = surfaceAim(e);
    G.brain.aim.x = a.x; G.brain.aim.y = a.y;
    const s = sim();
    if (s) s.setAim(a.x, a.y);
    return; // the blink happens on release, if Rust says it was a tap
  }
  triggerAimClick(e);
});
const endPointer = (e) => {
  if (gesture?.owns(e.pointerId)) {
    if (gesture.up(e.pointerId, e.type === 'pointercancel') && G) {
      triggerAimClick(e);
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
      showControlsFromFail();
      return;
    }
    if (e.code === 'Escape' || e.code === 'KeyQ') {
      e.preventDefault();
      quitToTitle();
      return;
    }
  }
  if (e.code === 'Escape' && eegPanel?.wiz?.active) { eegPanel?.closeWizard(); return; }
  if (eegPanel?.wiz?.active) return;
  if (banner?.fromFail) {
    if (e.code === 'Escape' || e.code === 'Enter' || e.code === 'Backspace') { e.preventDefault(); dismissBanner(); }
    return;
  }
  const overlay = !G || !$('titleScreen').hidden || !$('failScreen').hidden || !$('endScreen').hidden;
  if (overlay) return;
  if (banner && e.code !== 'Enter') return;
  if (e.code === 'KeyM') { toggleModel(); return; }
  if (e.code === 'KeyE') { eegPanel?.toggle(); return; }
  if (e.code === 'KeyC') { eegPanel?.calibrateCenter(); return; }
  if (e.code === 'KeyT') { toggle3D(); return; }
  if (e.code === 'KeyG') { toggleAIGod(); return; }
  if (G && (e.code === 'KeyZ' || e.code === 'Minus' || e.code === 'NumpadSubtract')) { zoomOutFromSurface(); return; }
  if (G && (e.code === 'KeyX' || e.code === 'Equal' || e.code === 'NumpadAdd')) { zoomInOnSurface(); return; }
  if (G && !cosmos.active && (e.code === 'KeyA' || e.code === 'KeyD')) { cosmos.panVel = e.code === 'KeyA' ? 140 : -140; return; }
  if (G && !cosmos.active && cosmos.surfaceInView && (e.code === 'KeyW' || e.code === 'KeyS')) { cosmos.panVelY = e.code === 'KeyW' ? 110 : -110; return; } // planets (#68)
  if (G && e.code === 'KeyN' && !e.repeat) { cam3dBase = null; cam3dTween = null; if (cosmos.visitNextWorld(G)) audio.blip(); return; } // planets (#68, #43): visit the next AI world (G is the AI God toggle)
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
$('showHowBtn')?.addEventListener('click', showControlsFromFail);
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
  $('zoomIn').disabled = in3dSurface() ? (engine?.get_camera_distance?.() <= 14) : (cosmos.target <= cosmos.minLevel());
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
  if (p) { cam3dBase = null; cam3dTween = null; cosmos.focusPlanet(p, G); audio.blip(); lastListKey = ''; }
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

// camera (#33, #116–#123): opt-in webcam. Hands perform actions (pinch-grab, 2-hand zoom, palm, fist)
// with coach visual demonstration. Face sets mood.
window.__moc.camera = initCamera({
  getG: () => cosmos.castG(G), // planets (#68): casts and mood reach the world you're on
  canCast: () => !!G?.scene && !banner && !G.finished && $('titleScreen').hidden && $('failScreen').hidden && $('endScreen').hidden,
  // camera debug (#67): the fingertip aims on the panned surface like the mouse does.
  mapX: (x) => cosmos.mapX(x * W) / W,
  screenX: (x) => ((((x * W + cosmos.panX) % W) + W) % W) / W,
  onZoomDelta: (delta, factor) => {
    // Two-hand continuous pinch zoom: delta > 0 is zoom in (hands spread), delta < 0 is zoom out (hands together)
    if (!G) return;
    if (in3dSurface()) {
      cam3dTween = null;
      if (delta < 0 && engine.get_camera_distance() >= CAM3D.topDown - 0.5) {
        cosmos.zoomOut(G);
        return;
      }
      const step = 1 - Math.max(-0.25, Math.min(0.25, delta * 2.5));
      zoom3dBy(step);
      return;
    }
    // Cosmos zoom (surface <-> globe <-> galaxy): wheel negative zooms in, positive zooms out
    cosmos.wheel(-delta * 750, G);
  },
  onGrabMove: ({ dx, dy, active, started, released, aim }) => {
    // One-hand pinch grab & move the globe or world view
    if (!G) return;
    if (cosmos.active || cosmos.z > 0.4) {
      if (active) {
        // In normalized hand space (-1..1). Moving hand right rotates globe right.
        const spin = -dx * 3.5;
        cosmos.rot += spin;
        cosmos.spinVel = spin / 0.033;
      }
      return;
    }
    if (in3dSurface()) {
      if (active && (Math.abs(dx) > 0.001 || Math.abs(dy) > 0.001)) {
        engine.orbit_camera(-dx * 3.5, dy * 3.5);
        cam3dBase = null;
      }
      return;
    }
    if (active && (Math.abs(dx) > 0.001 || Math.abs(dy) > 0.001)) {
      panMap(-dx * W * 1.5, -dy * H * 1.5);
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
