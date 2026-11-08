// Mind over Chicken: boot, scene flow, render pipeline and input.

import { W, H, HOME_W, HOME_H, clamp, lerp, wrapDelta } from './sim/util.js';

const smoothstep = (a, b, t) => { const x = clamp((t - a) / (b - a)); return x * x * (3 - 2 * x); };
import { World, setRustWorldGen } from './world/terrain.js';
import { SimSource, BridgeSource, liveBridge, setRustSimSource } from './brain/sources.js';
import { Brain, setRustBrain } from './brain/brain.js';
import { Advisor, setRustAdvisorEngine } from './ui/advisor.js';
import { EraIntro } from './ui/intro.js'; // era intro transition (no click-to-dismiss card)
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
  ViewCamera, // the 320×180 window onto the bigger home world (#129)
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
import { CivSandbox, initPlanetSizePicker } from './civ/sandbox.js'; // civ (#50)
import { worldMeta, renderWorldDetail } from './cosmos/panel.js'; // planets (#68)
import { SimPlanet } from './ai/planet.js';
import { AIGod } from './ai/autopilot.js';
import { observe, decodeAction, scriptedAction, BOLT_COOLDOWN } from './ai/observe.js';
import { loading } from './ui/loading.js'; // loading screen
import { panelHotkey } from './ui/hotkeys.js'; // E / C / Shift+C sidebars (#211)
import { engineFailToShow } from './ui/fail.js';
import { frameLoop } from './ui/loop.js';
import { dayNight } from './render/daynight.js';
import { rustEraOf, rustEraTicks } from './eras/rust-era.js';
import { mirrorHud } from './ui/hud-mirror.js';

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
const cosmosCanvas = Object.assign(document.createElement('canvas'), { width: W, height: H });
const cosmosCtx = cosmosCanvas.getContext('2d');
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
  return keepGroundUnder(e, grab3d);
}
// Move the camera so the ground point `anchor` sits under the pointer again: a drag (grab-pan)
// and a zoom toward the cursor both use it. Returns false where the scene has its own camera.
function keepGroundUnder(e, anchor) {
  const scene = cosmos?.viewScene?.(G) || G?.scene;
  if (scene instanceof CivSandbox || scene?.civ || (scene && typeof scene.centerColumn === 'function')) {
    return false;
  }
  for (let pass = 0; pass < 3; pass++) {
    const g = groundAt(e);
    if (!g) break;
    if (worldScene(cosmos.viewScene(G)) && cosmos.view) {
      // The home world (#129): a wrap-aware difference on both axes (a torus, #111), then move
      // the one camera (2D window and 3D target) by it.
      const dx = wrapDelta(anchor.x, g.x, cosmos.worldW), dz = wrapDelta(anchor.z, g.z, cosmos.worldH);
      if (Math.abs(dx) < 0.05 && Math.abs(dz) < 0.05) break;
      cosmos.pan(-dx, -dz);
      // Apply now, not at the next frame: a fast mouse sends several moves per frame, and each
      // one must raycast against the camera it already moved.
      engine.set_camera_target(cosmos.view.center_x(), cosmos.view.center_y());
      continue;
    }
    // Other worlds' 320×180 relief (#192): pan east–west, move the target north–south.
    let dx = anchor.x - g.x; dx = ((dx + W * 1.5) % W) - W / 2;
    const dz = anchor.z - g.z;
    if (Math.abs(dx) < 0.05 && Math.abs(dz) < 0.05) break;
    if (dx) cosmos.pan(-dx);
    engine.set_camera_target(engine.get_camera_target_x(), clamp(engine.get_camera_target_z() + dz, 0, H - 1));
  }
  cam3dBase = null;
  return true;
}
function panMap(dx, dy) {
  // Both axes. The home world's 3D target follows the 2D camera (#129); a scene with its own
  // camera (the civ sandbox) takes the pan through cosmos.panSink.
  const scene = cosmos?.viewScene?.(G) || G?.scene;
  cosmos.pan(dx, dy);
  if (dy && in3dSurface() && !worldScene(scene) && !G?.scene?.panBy && (!scene || typeof scene.centerColumn !== 'function')) {
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
  viewCam = new ViewCamera(HOME_W, HOME_H, W, H); // the 320×180 window onto the home world (#129)
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

let jsLoopRunning = true;
export async function startRustApp() {
  jsLoopRunning = false;
  await initEngine();
  const app = new RustApp('screen');
  initCameraAdapter(app);
  app.start();
  window.__moc = { app };
  return app;
}
if (typeof window !== 'undefined') {
  window.startRustApp = startRustApp;
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
cosmos.panSink = (dx, dy) => { if (G?.scene?.panBy) { G.scene.panBy(dx, dy); return true; } return false; };
// What the HUD shows while you look at the galaxy.
// Blinks raise storms on the galaxy's blink world, so Lightning is a power there too (#178).
const galaxyHudScene = { label: 'Galaxy', powers: ['blink', 'calm', 'focus', 'stress'], objectives: () => cosmos.galaxy?.objectives() || [] };
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
    try {
      await src.connect();
      await src.waitForFrame();   // swap only to a headset that is sending EEG (#174)
    } catch (err) {
      src.disconnect();           // no reconnect loop behind a failed attempt
      throw err;
    }
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
let banner = null;      // {t}: the controls card, now only reopened from the fail screen
// Each era opens with a short camera move onto its stage, its title and goal fading over it,
// inputs held until it ends (Space, Enter or a click skip it). The sim is paused meanwhile.
const intro = new EraIntro();
let introCam = null;    // 3D: { from: {dist, pitch}, to: {dist, pitch} } for the ease-in
let lastTs = 0;

// ---------- AI god (PPO / Autopilot) ----------
let aiGod = null;
let aiGodTimer = 0;
function resetAIGod() {
  if (!aiGod) return;
  aiGod = null;
  const s = sim();
  if (s) { s.input.calm = false; s.input.focus = false; s.input.panic = false; }
  const btn = $('aiGodBtn');
  if (btn) {
    btn.textContent = 'AI God: OFF';
    btn.title = 'Toggle AI God autopilot (G)';
    btn.setAttribute('aria-pressed', 'false');
  }
}
function toggleAIGod() {
  if (!G || !$('titleScreen').hidden) return;
  if (aiGod) {
    resetAIGod();
    advisor.say('ai-off', 'AI god disengaged. You have full command.', { mood: 'happy', priority: 2, cooldown: 1 });
  } else {
    // The trained (PPO) policy loads in the background; if it can't, the player is told that a
    // scripted god is steering, on the button and by the advisor (#181).
    const god = new AIGod({
      onStatus: (mode, err) => {
        if (aiGod !== god) return;   // toggled off meanwhile
        const b = $('aiGodBtn');
        if (b) {
          b.textContent = mode === 'ppo' ? 'AI God: ON' : 'AI God: ON (scripted)';
          b.title = mode === 'ppo' ? 'AI God: the trained PPO policy is steering (G)' : `AI God: the trained policy did not load (${err?.message || err}), a scripted god is steering (G)`;
        }
        if (mode === 'scripted') advisor.say('ai-scripted', 'The trained AI god would not load, my lord. A simple scripted god is steering instead.', { mood: 'panic', priority: 3, cooldown: 1 });
      },
    });
    aiGod = god;
    aiGodTimer = 0;
    const btn = $('aiGodBtn');
    if (btn) {
      btn.textContent = god.mode === 'scripted' ? 'AI God: ON (scripted)' : 'AI God: ON';
      btn.setAttribute('aria-pressed', 'true');
    }
    advisor.say('ai-on', 'AI god engaged! The PPO autopilot is steering this world. Stay calm to help it.', { mood: 'nervous', priority: 2, cooldown: 1 });
  }
}

function updateAIGodAutopilot(dt) {
  if (!aiGod || !G || !$('failScreen').hidden || !$('endScreen').hidden) return;
  const scene = cosmos?.viewScene?.(G) || G?.scene;
  if (!scene) return;
  const s = sim();

  if (scene.id === 'stone') {
    aiGodTimer -= dt;
    if (aiGodTimer <= 0) {
      aiGodTimer = 0.25;
      const obs = observe(scene, G, { lastBolt: G.aiLastBolt ?? -Infinity });
      const action = aiGod.policy ? aiGod.policy.act(obs, { greedy: true }) : scriptedAction(obs);
      const { aim, calm } = decodeAction(scene, action);
      if (s) s.input.calm = calm;
      if (aim && scene.t - (G.aiLastBolt || -Infinity) >= BOLT_COOLDOWN) {
        G.aiLastBolt = scene.t;
        if (s) s.setAim(aim.x, aim.y);
        scene.onBlink(aim);
      }
    }
  } else if (scene.id === 'industry') {
    if (s) {
      const needCalm = scene.ipo?.state === 'active' || (scene.arousal && scene.arousal() > 0.4) || scene.strike;
      s.input.calm = !!needCalm;
      s.input.focus = !needCalm;
    }
  } else if (scene.era === 'space') {
    if (s) {
      if (scene.phase === 'build') {
        s.input.focus = true;
        s.input.calm = false;
      } else if (scene.phase === 'board') {
        s.input.focus = false;
        s.input.calm = true;
      } else if (scene.phase === 'countdown') {
        s.input.focus = true;
        s.input.calm = true;
        s.input.panic = false;
      }
    }
  } else if (scene instanceof CivSandbox || scene?.civ) {
    if (s) {
      s.input.calm = true;
      s.input.focus = false;
    }
  }
}

function newGame(source, startIndex = 0) {
  if (previewSource && previewSource !== source) previewSource.disconnect();
  previewSource = null;
  previewBrain = null;
  if (G?.source && G.source !== source) G.source.disconnect();
  resetAIGod();
  const brain = new Brain(source);
  cosmos.reset();
  intro.cancel(); introCam = null; delete document.body.dataset.intro;
  G = {
    world: new World(undefined, { width: HOME_W, height: HOME_H }), // the bigger home world (#129)
    brain, source, advisor, hud, audio, engine,
    calibration: new Calibration(calibConfig), // calibration (#54)
    scene: null, sceneIndex: startIndex, runTime: 0, shake: 0,
    stats: { lightning: 0, smitten: 0, births: 0, deaths: 0, recessions: 0, restarts: 0 },
    eraTimes: [], eraStart: 0, finished: false,
    complete(delay = 2) {
      if (G.sceneIndex >= SCENES.length - 1) return;
      pendingScene = G.sceneIndex + 1;
      pendingTimer = delay;
    },
    fail(reason) { showFail(reason); },
    finish(extra) { G.finished = true; G.extra = extra; setTimeout(showEnd, 2200); },
    // The intro transition for the scene in play (#137), e.g. a new stage inside one scene.
    startTransition(caption) { startTransition(G.scene, caption); },
  };
  // Headset aims are screen positions: map them through the home camera (#129), raycasting in 3D.
  brain.screenToAim = (x, y) => {
    if (cosmos.surfaceInView || !cosmos.view || cosmos.active) return { x, y };
    if (in3dSurface()) {
      const r = screen.getBoundingClientRect();
      const hit = engine.screen_to_ground(x * r.width, y * r.height, r.width, r.height);
      if (hit && hit[0] >= 0) return { x: hit[0] / cosmos.worldW, y: hit[1] / cosmos.worldH };
    }
    const p = cosmos.toWorld(x * W, y * H);
    return { x: p.x / cosmos.worldW, y: p.y / cosmos.worldH };
  };
  brain.onBlink((aim) => {
    hud.flashBlink();
    if (engine) engine.game_on_blink(aim.x * cosmos.worldW, aim.y * cosmos.worldH);
    cosmos.galaxy?.onBlink();   // storms on the blink world; a strike on the world you're zoomed into
    if (cosmos.galaxy?.attended) audio.thunder();
    const view = cosmos.viewScene(G); // planets (#68): another world's surface when you're on it
    if (!banner && !intro.active && view?.onBlink) view.onBlink(aim);
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
    engine.set_era(startIndex);   // startScene() starts the scene's Rust era sim, if it has one (#194)
  }
  startScene(startIndex);
  try { window.claude?.hot?.snapshot?.(() => ({ sceneIndex: G?.sceneIndex ?? 0, kind: G?.source?.kind })); } catch {}
}

function startScene(i) {
  if (i >= SCENES.length) {
    pendingScene = null;
    fade = 0;
    return;
  }
  if (G.scene) G.eraTimes.push({ label: G.scene.label, seconds: G.runTime - G.eraStart });
  G.eraStart = G.runTime;
  G.sceneIndex = i;
  const scene = SCENES[i]();
  G.scene = scene;
  if (engine) {
    engine.set_era(i);
    // Only the playable eras have a Rust sim; a time skip must not restart the last one (#194).
    if (rustEraOf(scene) !== null) engine.game_start_era(rustEraOf(scene));
  }
  advisor.setEra(scene.era);
  scene.enter(G);
  lookAtStage(scene);
  hud.setScene(scene);
  document.body.dataset.era = scene.era;
  fade = 1;
  G.failing = false;
  banner = null;
  $('banner').hidden = true;
  if (scene.intro) startIntro(scene, i);
  else intro.cancel();
}

// Each era opens with the camera on its stage: the village, the exchange, the launch crowd (#129).
function lookAtStage(scene) {
  const s = G?.world?.sites;
  if (!s || !WORLD_SCENES.some((c) => scene instanceof c)) return;
  const f = scene.era === 'industry' ? s.exchange : scene.era === 'space' ? s.crowd : s.village;
  if (f) cosmos.lookAt(f.x, f.y);
}

function restartScene() {
  G.stats.restarts++;
  const i = G.sceneIndex;
  const scene = SCENES[i]();
  G.scene = scene;
  if (engine) {
    engine.set_era(i);
    if (rustEraOf(scene) !== null) engine.game_restart_era();
  }
  advisor.setEra(scene.era);
  scene.enter(G);
  lookAtStage(scene);
  hud.setScene(scene);
  fade = 1;
  // A fresh attempt: no fail pending, and no controls card left over from the fail screen (#172).
  G.failing = false;
  banner = null;
  $('banner').hidden = true;
  if (scene.intro) startIntro(scene, i);
}

// ---------- era intro transition ----------
// A short move onto the era's stage instead of a card: 3D eases from high and tilted down to the
// play pose, 2D zooms in from a wider frame (ViewCamera.blits_scaled). The caption is the era
// title and its one-line goal; the how-to lines move to the advisor once the stage is reached.
const INTRO_FROM = { dist: 560, pitch: 1.42, zoom2d: 2.2 };
function startIntro(scene, i) {
  const n = ['I', 'II', 'III'][[0, 2, 4].indexOf(i)] || '';
  startTransition(scene, { eyebrow: n ? `Era ${n}` : (scene.label || ''), title: scene.intro.title, goal: scene.intro.goal, how: scene.intro.how });
}
// The transition API (#137): any scene (an era, a progression stage, the civ sandbox) opens with
// it. caption: { eyebrow, title, goal, how: [[key, text], ...] }; the how-to lines go to the
// advisor when it ends. Also on G.startTransition(caption) for the scene in play (a new stage
// inside one scene) and __moc.startTransition.
function startTransition(scene, caption = {}) {
  if (!G || !scene) return;
  intro.start({ eyebrow: caption.eyebrow || '', title: caption.title || scene.label || '', goal: caption.goal || '', how: caption.how || [] }, () => endIntro(scene));
  document.body.dataset.intro = 'on';
  G.brain.setCue(null);
  introCam = null;
  if (engine && engine.is_3d_mode()) {
    // Land on the player's own angle, or the default diorama pose if they were looking straight down.
    const d = engine.get_camera_distance(), pitch = engine.get_camera_pitch();
    const to = d >= CAM3D.blendFrom ? { dist: 300, pitch: 1.05 } : { dist: d, pitch };
    introCam = { from: { dist: Math.max(to.dist, INTRO_FROM.dist), pitch: Math.max(to.pitch, INTRO_FROM.pitch) }, to };
    cam3dBase = null; cam3dTween = null;
    applyIntroCamera();
  }
}
function applyIntroCamera() {
  if (!introCam || !engine) return;
  const a = intro.away();
  const { from, to } = introCam;
  const dist = to.dist + (from.dist - to.dist) * a;
  const pitch = to.pitch + (from.pitch - to.pitch) * a;
  engine.zoom_camera(dist / engine.get_camera_distance());
  engine.orbit_camera(0, pitch - engine.get_camera_pitch());
}
function endIntro(scene) {
  applyIntroCamera();
  const caption = intro.caption;
  introCam = null;
  delete document.body.dataset.intro;
  if (!G || G.scene !== scene) return;
  // Re-arm the scene's calibration cue, as the card's Begin did.
  if (scene.id === 'stone' || scene.id === 'industry') G.brain.setCue(scene.hold?.active || scene.ipo?.state === 'active' ? 'calm' : 'rest');
  // The how-to lines, now the advisor's first words on the stage.
  const how = (caption?.how || []).map(([k, v]) => `${k}: ${v}`).join(' ');
  if (how) advisor.say(`howto-${scene.era || scene.id}`, how, { mood: 'happy', priority: 3, cooldown: 999 });
}
// The caption over the move: eyebrow, title and goal, fading in and out (view units).
function drawIntroCaption(ctx) {
  const a = intro.captionAlpha();
  const c = intro.caption;
  if (a <= 0 || !c) return;
  ctx.save();
  ctx.globalAlpha = a;
  ctx.textAlign = 'center';
  const line = (text, y, size, color) => {
    if (!text) return;
    ctx.font = `${size}px "Pixelify Sans", monospace`;
    ctx.fillStyle = 'rgba(8,10,24,0.85)';
    ctx.fillText(text, W / 2 + 0.7, y + 0.7);
    ctx.fillStyle = color;
    ctx.fillText(text, W / 2, y);
  };
  line(c.eyebrow, 46, 6, '#ff9a2e');
  line(c.title, 66, 18, '#f2e8d5');
  line(c.goal, 80, 6.5, '#ffd166');
  ctx.font = '5px "Pixelify Sans", monospace';
  ctx.fillStyle = 'rgba(242,232,213,0.55)';
  ctx.fillText('Space · Enter · click to skip', W / 2, H - 14);
  ctx.restore();
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
  if (G) {
    if (G.failing) return;   // one fail per run: the JS scene and the Rust era sim can both report it (#172)
    G.failing = true;
  }
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
        nextStep = 'Strike giant chickens with lightning so hunters can gather meat for the tribe.';
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
  const how = $('showHowBtn');
  if (how) how.hidden = !G?.scene?.intro;   // no controls card to show for a scene without one (#172)
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
  intro.cancel(); introCam = null; delete document.body.dataset.intro;
  pendingScene = null;
  resetAIGod();
  const keep = liveBridge(G?.source);
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
    ['Headset', G.source?.kind === 'bridge' ? `Muse via the bridge (${G.source.connected ? 'connected' : 'disconnected'})` : 'Emulated headset (keyboard)'],
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
  const x0 = MINI.pad, y0 = MINI.pad;   // top-left: the goat chart is top-right, the coach and HUD at the bottom
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

// Frame cost for playtests (#129): render ms and whole-frame ms, read and reset via __moc.perf().
let perfSample = { n: 0, ms: 0, frames: 0, frameMs: 0, maxFrameMs: 0 };
function render() {
  const t0 = performance.now();
  const scene = G ? cosmos.viewScene(G) : null;
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
  // In 3D mode, the 3D map is kept active continuously between orbit and surface (continuous zoom down from globe).
  G.render3d = in3dSurface();
  globalThis.__moc_render3d = G.render3d;
  const z = cosmos?.z || 0;
  if (G.render3d) {
    if (windowed && cosmos.view) {
      // One camera for both views: the 3D target is the centre of the 2D window (#129).
      engine.set_camera_target(cosmos.view.center_x(), cosmos.view.center_y());
    } else if (scene && typeof scene.centerColumn === 'function') {
      // Other worlds and the civ sandbox (#192): their own camera over a 320×180 relief.
      const cx = scene.centerColumn();
      const vh = scene.v?.h ?? (scene.civ ? H : 360);
      const cz = scene.civ ? (H / 2) : clamp(((scene.camY ?? 0) + H / 2) / vh * H, 10, H - 10);
      engine.set_camera_target(cx, cz);
    }
    if (z > 0.001 && z < 1.0) {
      // Continuous camera zoom between orbit and ground
      const k = clamp(z / 0.75);
      const groundDist = CAM3D.blendFrom;
      const targetDist = groundDist + (CAM3D.topDown - groundDist) * k;
      const targetPitch = 1.05 + (CAM3D.topPitch - 1.05) * k;
      if (typeof engine.set_camera_distance === 'function') {
        engine.set_camera_distance(targetDist);
        engine.set_camera_pitch(targetPitch);
      }
    } else {
      stepCam3dTween();
    }
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
  if (windowed && !G.render3d) {
    const zoom = intro.active ? 1 + (INTRO_FROM.zoom2d - 1) * intro.away() : 1;
    if (zoom > 1.001) cosmos.blitViewScaled(worldCanvas, low, zoom);   // the intro's wider frame
    else cosmos.blitView(worldCanvas, low);
  }
  if (!G.render3d) scene.drawScreen?.(lowCtx);
  if (!windowed && !G.render3d && scene?.phase !== 'voyage') cosmos.applyPan(low);

  let cosmic = false;
  if (!G.render3d || z >= 1.0) {
    cosmic = cosmos.render(low, lowCtx, G, { is3d: false });
  } else if (z > 0.001) {
    cosmosCtx.imageSmoothingEnabled = false;
    cosmosCtx.clearRect(0, 0, W, H);
    cosmic = cosmos.render(low, cosmosCtx, G, { is3d: true });
  }

  // Zooming out fades the surface's night lighting and labels; the globe and galaxy
  // carry their own shading, so the night shader must not dim space.
  const surface = !cosmic || z < 1.0;
  const k = cosmic ? Math.max(0, 1 - z / 0.3) : 1;
  const dark = scene.phase === 'voyage' ? 0 : (scene.darkness || 0) * k;
  const flash = (scene.flash || 0) * k;
  const shakeAmount = G.shake ? (G.shake * 0.008) : 0;
  const sx = shakeAmount ? (Math.random() - 0.5) * shakeAmount : 0;
  const sy = shakeAmount ? (Math.random() - 0.5) * shakeAmount : 0;
  // World-space lights and labels repeat at every copy of the world that reaches the screen, so
  // they follow the camera and wrap with the map. In 3D, labels are projected instead.
  const offsets = windowed && !G.render3d ? cosmos.overlayOffsets() : [[0, 0]];

  if (engine) {
    try {
      if (G.render3d && z < 1.0) {
        sync3dWorldMesh(scene);
        engine.begin_frame();
        // The 3D terrain texture: the home world's world-sized canvas (#129), or the view canvas
        // for other worlds' 320×180 relief (#192).
        engine.upload_terrain_canvas(windowed ? worldCanvas : low);
        if (scene === G.scene && !G.sandbox && !(scene instanceof CivSandbox) && !scene?.civ && scene?.era) {
          G.world?.draw3d?.(engine, windowed ? near3d(scene.G.world) : null);
          if (scene.draw3d) {
            scene.draw3d(engine, 0.016);
          } else if (typeof engine.draw_era_3d === 'function') {
            engine.draw_era_3d();
          }
        } else if (scene?.draw3d) {
          scene.draw3d(engine, 0.016);
        } else if (scene?.lights) {
          for (const l of scene.lights()) {
            engine.add_3d_light(l.x, 10.0, l.y, l.r, 1.2);
          }
        }
        // Day and night (#201): the sun rises and sets; after dusk the moonlit night shading takes
        // over. An era's own darkness (dark) still wins. Off the surface the night must not dim space.
        const dn = dayNight(G.runTime || 0, dark);
        const night3d = z > 0.001 || scene.phase === 'voyage' ? dark : dn.night;
        engine.render_3d(0.016, dn.sun.x, dn.sun.y, dn.sun.z, night3d, flash, fade, sx, sy, screen.width, screen.height);

        // Continuous overlay blend between 3D planet view and 3D map view
        if (z > 0.001) {
          const overlayAlpha = smoothstep(0.04, 0.70, z);
          if (overlayAlpha > 0.001 && typeof engine.render_overlay_canvas === 'function') {
            engine.render_overlay_canvas(cosmosCanvas, overlayAlpha, screen.width, screen.height);
          }
        }
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
    for (const [ox, oy] of intro.active ? [] : offsets) {   // map labels wait for the stage
      textCtx.setTransform(K, 0, 0, K, ox * K, oy * K);
      scene.drawOverlay?.(textCtx);
    }
    textCtx.__project3d = null;
    textCtx.setTransform(K, 0, 0, K, 0, 0);
    if (G.render3d) scene.drawScreen?.(textCtx);   // 3D has no low-res frame to paint into
    if (!intro.active) scene.drawScreenOverlay?.(textCtx);   // the stage's own HUD waits for the intro
    if (windowed && !cosmic && scene.phase !== 'voyage' && !intro.active) drawMinimap(textCtx, scene);   // not over the voyage's starfield
    if (intro.active) drawIntroCaption(textCtx);
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
    signalNotices();
    // The Rust shell draws the WebGL HUD (objectives, clock, meters, advisor text) in game_tick.
    // It shows the JS scene on screen (#191): mirrorHud hands over its objectives and clock, and
    // the Rust era sim stands still, so it runs every frame, time skips and intros included.
    // Without the mirror (an old pkg/), the Rust era sim ticks only in a playable era (#194).
    const mirrored = engine && mirrorHud(engine, G.runTime, cosmos.hudScene(G, galaxyHudScene));
    if (engine && (mirrored || rustEraTicks(G.scene, intro.active))) {
      const b = G.brain;
      engine.game_tick(dt, b.calm || 0, b.focus || 0, b.stress ?? 0, b.stats?.blinks || 0);
      // Under the mirror these never fire: the JS scene calls G.complete / G.fail itself.
      if (engine.is_game_era_complete?.() && !pendingScene && !G.finished && G.sceneIndex < SCENES.length - 1 && G.scene?.era !== 'space') {
        G.complete(1.5);
      }
      // Once per failed run, not whenever the fail screen is hidden (Controls hides it, #172).
      const failReason = engineFailToShow(engine, G);
      if (failReason) showFail(failReason);
    }
    if (intro.active) {
      intro.update(dt);   // the sim stays paused: the era starts exactly as enter() left it
      applyIntroCamera();
    } else if (banner) {
      banner.t += dt;
      if (!banner.manual && banner.t > 8) dismissBanner();
    } else if (G.sandbox) {
      G.runTime += dt;   // the hand-played game stays paused in the sandbox
      G.calibration?.update(dt, G.brain, G.mood);
    } else if (G.signalPaused && $('failScreen').hidden && $('endScreen').hidden) {
      // #175: poor contact. The era waits (no scoring on garbage calm/focus) and resumes by itself.
    } else if ($('failScreen').hidden && $('endScreen').hidden) {
      G.runTime += dt;
      G.calibration?.update(dt, G.brain, G.mood);
      if (aiGod) updateAIGodAutopilot(dt);
      G.scene?.update(dt);
    } else if (!$('endScreen').hidden) {
      G.scene?.update(dt);
    }
    if (G.scene?.phase === 'launch' || G.scene?.phase === 'voyage') {
      fade = 0;
      pendingScene = null;
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
  const fms = performance.now() - ts;
  perfSample.frames++; perfSample.frameMs += fms; perfSample.maxFrameMs = Math.max(perfSample.maxFrameMs, fms);
}
const rafLoop = frameLoop(frame, { running: () => jsLoopRunning }); // survives a throwing frame (#171)
setInterval(() => { if (performance.now() - lastFrameAt > 45) frame(); }, 33);

// ---------- input ----------
function sim() { return G?.source?.kind === 'sim' ? G.source : null; }

function isEegLive() {
  const src = G?.source;
  return !!(src && src.kind === 'bridge' && src.connected && src.streaming && src.lastFrameAt > 0 && (performance.now() - src.lastFrameAt < 3000));
}

// The headset aims only while it is sending head tilt (#175): a live Muse with no orientation
// used to freeze the cursor, because a live headset turned the mouse aim off.
function headAimLive() { return isEegLive() && !!G?.brain?.headTracking; }

// #175: say once per run when the headset can't aim, and while its contact is unusable.
function signalNotices() {
  const b = G?.brain;
  if (!b) return;
  // Only a live headset can be unusable; if it stops streaming the pause lifts (no stuck era).
  if (G.source?.kind !== 'bridge' || !isEegLive()) { G.signalPaused = false; return; }
  if (!b.headTracking && b.stats.framesSeen > 20 && !G.headNoticeShown) {
    G.headNoticeShown = true;
    advisor.say('head-lost', 'Head tracking unavailable, my lord: aim with the mouse or the arrow keys.', { mood: 'nervous', priority: 3, cooldown: 30 });
    hud.showCause?.('Head tracking unavailable: aim with the mouse or the arrow keys', { ms: 5000, tone: 'wait' });
  }
  if (b.signalUnusable && !G.signalPaused) {
    G.signalPaused = true;
    advisor.say('signal-bad', 'I cannot read your mind, my lord: the headset has poor contact. Settle the band on your forehead and behind your ears. The era waits for you.', { mood: 'panic', priority: 4, cooldown: 10 });
  } else if (!b.signalUnusable && G.signalPaused) {
    G.signalPaused = false;
    advisor.say('signal-ok', 'There you are. The signal is back; the era resumes.', { mood: 'happy', priority: 3, cooldown: 5 });
  }
  if (G.signalPaused) hud.showCause?.(`Headset signal unusable (quality ${Math.round(b.signalQuality * 100)}%): the era is paused`, { ms: 600, tone: 'wait' });
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
      return { x: hit[0] / cosmos.worldW, y: hit[1] / cosmos.worldH };
    }
  }
  // 2D: screen → world through the camera (#129). Aims are 0..1 of the world, like the brain's.
  const a = aimFromEvent(e);
  if (cosmos.surfaceInView || !cosmos.view) return a;
  const p = cosmos.toWorld(a.x * W, a.y * H);
  return { x: p.x / cosmos.worldW, y: p.y / cosmos.worldH };
}

function followCursorAim(e) {
  if (!G?.brain || headAimLive()) return;
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
// topDown is where the 30 degree lens frames the 180-tile-high view at sea level (y = 7.92):
// 90 / tan(15 deg) + 7.92 = 344. The world is bigger (#129), but the camera target is the centre of
// the 2D window, so the top-down 3D frame is exactly the 2D view at hand-over.
const CAM3D = { blendFrom: 300, topDown: 344, topPitch: 1.55 };
let cam3dBase = null;   // the player's own angle when the tip-over began
let cam3dTween = null;  // a keyboard/button zoom-out animates the tip-over, then hands over
let current3dWorldKey = '';
function sync3dWorldMesh(scene) {
  if (!engine || !scene) return;
  if (scene instanceof CivSandbox || scene?.civ) {
    const key = `civ_${scene.seed || 0}_${Math.round(scene.civ?.camX || 0)}_${Math.round(scene.civ?.camY || 0)}`;
    if (key === current3dWorldKey) return;
    current3dWorldKey = key;
    const h = scene.getVisibleHeightmap?.();
    if (h && engine.set_heightmap) {
      try { engine.set_heightmap(h); } catch (e) { console.warn('set_heightmap failed:', e); }
    }
    if (engine.set_backdrop) {
      engine.set_backdrop(24, 52, 98);
    }
    return;
  }
  if (typeof scene.getGlobeHeightmap === 'function' || typeof scene.getVisibleHeightmap === 'function') {
    const sId = scene.p?.name || scene.p?.id || scene.seed || scene.id || 'other';
    const key = `other_${sId}`;
    if (key === current3dWorldKey) return;
    current3dWorldKey = key;

    const h = scene.getGlobeHeightmap?.() || scene.getVisibleHeightmap?.();
    if (h && engine.set_heightmap) {
      try { engine.set_heightmap(h); } catch (e) { console.warn('set_heightmap failed:', e); }
    }
    const world = scene.world || G?.world;
    const deep = world?.palette?.[0]?.[0] || [11, 16, 32];
    if (engine.set_backdrop) {
      engine.set_backdrop(deep[0], deep[1], deep[2]);
    }
    return;
  }

  const world = G?.world;
  if (!world) return;
  const key = `home_${world.seed}`;
  if (key === current3dWorldKey) return;
  current3dWorldKey = key;

  if (world.h && engine.set_heightmap) {
    try { engine.set_heightmap(world.h, world.width || 0, world.height || 0); } catch (e) { console.warn('set_heightmap failed:', e); }
  }
  const deep = world.palette?.[0]?.[0];
  if (deep && engine.set_backdrop) {
    engine.set_backdrop(deep[0], deep[1], deep[2]);
  }
}

// 3D world: enabled whenever looking at any planet's surface (including continuous zoom down from globe).
function in3dSurface() {
  const scene = cosmos?.viewScene?.(G) || G?.scene;
  if (scene?.phase === 'voyage') return false;
  return Boolean(
    engine &&
    engine.is_3d_mode() &&
    (!cosmos || !cosmos.active || (cosmos.z || 0) < 1.0)
  );
}
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
  const scene = cosmos?.viewScene?.(G) || G?.scene;
  if (scene instanceof CivSandbox || scene?.civ) {
    scene.onWheel?.(100);
    return;
  }
  if (!in3dSurface()) { cosmos.zoomOut(G); return; }
  const d = engine.get_camera_distance();
  if (d < CAM3D.topDown - 4) {
    cam3dTween = { from: d, t0: performance.now(), dur: 300 };
  } else {
    cosmos.zoomOut(G);
  }
}
function zoomInOnSurface() {
  const scene = cosmos?.viewScene?.(G) || G?.scene;
  if (scene instanceof CivSandbox || scene?.civ) {
    scene.onWheel?.(-100);
    return;
  }
  if (in3dSurface() && (cosmos?.z || 0) <= 0.01) {
    cam3dTween = null;
    zoom3dBy(0.8);
    return;
  }
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
  if (intro.gate('pointer') !== 'pass') return;   // inputs are held during the era intro
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
  if (intro.gate('pointer') === 'skip') { intro.skip(); return; }   // a click skips the era intro
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
  if (intro.gate('wheel') !== 'pass') return;
  // Horizontal scroll (or Shift+scroll) pans the surface; in 3D the camera follows the pan.
  const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey;
  if (horizontal && !cosmos.active) { cosmos.pan(-(e.shiftKey ? e.deltaY : e.deltaX) * 0.15); return; }
  // In the galaxy a sideways scroll moves the picker between planets (Alex).
  if (horizontal && cosmos.inGalaxy) {
    galaxyScroll += e.shiftKey ? e.deltaY : e.deltaX;
    if (Math.abs(galaxyScroll) > 80) { if (cosmos.selectSide(Math.sign(galaxyScroll), G)) audio.blip(); galaxyScroll = 0; }
    return;
  }
  if (in3dSurface() && (cosmos?.z || 0) <= 0.01) {
    cam3dTween = null;
    const scene = cosmos?.viewScene?.(G) || G?.scene;
    // Already looking straight down at the whole map: scrolling on carries on into the globe,
    // continuously (#143).
    if (e.deltaY > 0 && engine.get_camera_distance() >= CAM3D.topDown - 0.5 && !(scene instanceof CivSandbox) && !scene?.civ) {
      cosmos.wheel(wheelAmount(e), G);
      return;
    }
    // In proportion to the scroll (#143), and easy to do (Alex: "the zoom is too difficult"):
    // a mouse notch is about ×1.27, a trackpad pinch counts triple. The ground under the cursor
    // stays under it, like a map app.
    const anchor = groundAt(e);
    zoom3dBy(Math.exp(wheelAmount(e) * 0.0024));
    if (anchor) keepGroundUnder(e, anchor);
    return;
  }
  const scene = cosmos?.viewScene?.(G) || G?.scene;
  if (scene instanceof CivSandbox || scene?.civ) {
    scene.onWheel?.(wheelAmount(e));
    return;
  }
  cosmos.wheel(wheelAmount(e), G);
}, { passive: false });
// How far one wheel event zooms, in pixel-like units: a mouse notch is about 100, a trackpad
// pinch (sent as Ctrl+wheel with small deltas) counts triple, line-mode wheels are scaled up.
let galaxyScroll = 0;   // sideways scroll gathered towards the next planet pick
function wheelAmount(e) {
  const lines = e.deltaMode === 1 ? 33 : e.deltaMode === 2 ? 400 : 1;
  return clamp(e.deltaY * lines * (e.ctrlKey ? 3 : 1), -300, 300);
}
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
  // The era intro: Space or Enter skip it, every other game key waits for the stage.
  const gate = intro.gate('key', e.code);
  if (gate === 'skip') { e.preventDefault(); intro.skip(); return; }
  if (gate === 'hold') return;
  if (banner && e.code !== 'Enter') return;
  if (e.code === 'KeyM') { toggleModel(); return; }
  // E / C toggle the EEG / Camera sidebars; Shift+C re-centres the EEG head pose (#211).
  const panelKey = panelHotkey(e);   // ` stays a Camera alias in camera/debug.js
  if (panelKey === 'eeg') { eegPanel?.toggle(); return; }
  if (panelKey === 'camera') { e.preventDefault(); window.__moc?.camera?.togglePanel(); return; }
  if (panelKey === 'eegCenter') { e.preventDefault(); eegPanel?.calibrateCenter(); return; }
  if (e.code === 'KeyT') { toggle3D(); return; }
  if (e.code === 'KeyG') { toggleAIGod(); return; }
  if (G && (e.code === 'KeyZ' || e.code === 'Minus' || e.code === 'NumpadSubtract')) { zoomOutFromSurface(); return; }
  if (G && (e.code === 'KeyX' || e.code === 'Equal' || e.code === 'NumpadAdd')) { zoomInOnSurface(); return; }
  // The galaxy picker (Alex): Left/Right or A/D move between planets; Enter lands on the chosen one.
  if (G && cosmos.inGalaxy && ['ArrowLeft', 'ArrowRight', 'KeyA', 'KeyD'].includes(e.code)) {
    e.preventDefault();
    if (!e.repeat && cosmos.selectSide(e.code === 'ArrowLeft' || e.code === 'KeyA' ? -1 : 1, G)) audio.blip();
    return;
  }
  if (G && !cosmos.active && (e.code === 'KeyA' || e.code === 'KeyD')) { cosmos.panVel = e.code === 'KeyA' ? 140 : -140; return; }
  if (G && !cosmos.active && (e.code === 'KeyW' || e.code === 'KeyS')) { cosmos.panVelY = e.code === 'KeyW' ? 110 : -110; return; } // planets (#68); home pans north–south too (#129)
  if (G && e.code === 'KeyN' && !e.repeat) { cam3dBase = null; cam3dTween = null; if (cosmos.visitNextWorld(G)) audio.blip(); return; } // planets (#68, #43): visit the next AI world (G is the AI God toggle)
  if (e.code === 'Enter') {
    if (banner) dismissBanner();
    else if (!$('failScreen').hidden) retryEra();
    else if (G && cosmos.z > 0.5) cosmos.zoomIn(G);   // galaxy → the chosen world's globe → its surface
    else G?.scene?.skip?.();
    return;
  }
  if (!sim() && e.code.startsWith('Arrow') && G?.brain && !headAimLive()) {
    // A headset without head tracking: the arrows aim, as they do on the emulator (#175).
    const a = G.brain.aim, step = 0.04;
    if (e.code === 'ArrowLeft') a.x = clamp(a.x - step);
    if (e.code === 'ArrowRight') a.x = clamp(a.x + step);
    if (e.code === 'ArrowUp') a.y = clamp(a.y - step);
    if (e.code === 'ArrowDown') a.y = clamp(a.y + step);
    e.preventDefault();
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
    if (tile.disabled) return;   // not a power in this era (#178)
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
$('againBtn').addEventListener('click', () => { $('endScreen').hidden = true; newGame(playSource()); });
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
  $('zoomIn').disabled = (in3dSurface() && (cosmos?.z || 0) <= 0.01) ? (engine?.get_camera_distance?.() <= 14) : (cosmos.target <= cosmos.minLevel());
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
// Every entry point (Play, both sandboxes, Play again) keeps a connected headset and only falls
// back to the emulator without one; newGame() disconnects whatever it isn't given (#185).
function playSource() { return liveBridge(G?.source, previewSource) || new SimSource(); }
function begin(source) {
  audio.unlock();
  $('titleScreen').hidden = true;
  newGame(source);
}
$('playSim').addEventListener('click', () => begin(playSource()));
$('playSandbox').addEventListener('click', () => {
  begin(playSource());
  dismissBanner();
  G.sandbox = true;
  cosmos.sandbox = true;
  cosmos.galaxy = null;
  cosmos.ensureGalaxy(G, { autonomousHome: true });
  cosmos.z = 1; cosmos.target = 2;
  advisor.setEra('space');
  advisor.say('sandbox', 'Galaxy sandbox, my lord. Three worlds are forming from your thoughts. Make more planets and let them run.', { mood: 'happy', priority: 3, cooldown: 999 });
});
// civ (#50): "Sandbox: a crowded planet" runs its own scene (Rust sim in src/civ/) on the connected headset, or the emulated one (#185).
initPlanetSizePicker($('planetSizePick'));   // Small/Medium/Large/Huge or Random (Alex)
$('playCiv').addEventListener('click', () => {
  begin(playSource());
  const scene = new CivSandbox({ cosmos });
  G.scene = scene;
  advisor.setEra(scene.era);
  scene.enter(G);
  hud.setScene(scene);
  document.body.dataset.era = scene.era;
  startIntro(scene, -1);
});
// Connect a real headset (#174): the run starts on its first EEG frame, not when the socket
// opens. While it waits the button cancels; after a failure it retries.
let bridgeWait = null;
$('playBridge').addEventListener('click', async () => {
  const btn = $('playBridge');
  const status = $('bridgeStatus');
  if (bridgeWait) {
    const s = bridgeWait;
    bridgeWait = null;
    s.disconnect();
    btn.textContent = 'Connect';
    status.textContent = 'Cancelled.';
    return;
  }
  const url = $('bridgeUrl').value.trim() || 'ws://localhost:8765';
  status.textContent = `Connecting to ${url}…`;
  const src = new BridgeSource(url);
  bridgeWait = src;
  btn.textContent = 'Cancel';
  try {
    await src.connect();
    if (bridgeWait !== src) return;
    status.textContent = 'Bridge connected. Waiting for the first EEG from the headset…';
    await src.waitForFrame(10000);
    if (bridgeWait !== src) return;
    bridgeWait = null;
    btn.textContent = 'Connect';
    status.textContent = src.status;
    begin(src);
  } catch (err) {
    if (bridgeWait !== src) return;   // cancelled
    bridgeWait = null;
    src.disconnect();
    btn.textContent = 'Retry';
    status.textContent = src.connected || /EEG/.test(err.message)
      ? `${err.message}. Is the Muse on, charged and paired with the bridge? Retry, or play with the emulated headset.`
      : `${err.message}. Start the bridge with: python bridge/muse_bridge.py --board muse2 (or --board synthetic to test).`;
  }
});

// Debug handle for the console (and for automated playtests).
window.__moc = {
  get G() { return G; }, startScene: (i) => startScene(i), cosmos, get engine() { return engine; }, intro, startTransition: (caption) => startTransition(G?.scene, caption),
  // Average render and frame cost since the last call (ms), then reset.
  perf() {
    const p = perfSample;
    perfSample = { n: 0, ms: 0, frames: 0, frameMs: 0, maxFrameMs: 0 };
    return { renderMs: p.n ? p.ms / p.n : 0, frameMs: p.frames ? p.frameMs / p.frames : 0, maxFrameMs: p.maxFrameMs, frames: p.frames };
  },
  // CPU cost of n back-to-back frames (sim step + render), ms per frame: a playtest benchmark
  // that doesn't depend on how often the browser schedules frames.
  bench(n = 120) {
    const t0 = performance.now();
    for (let i = 0; i < n; i++) { lastFrameAt = 0; lastTs = performance.now() - 16.7; frame(); }
    return (performance.now() - t0) / n;
  },
};

// camera (#33, #116–#123): opt-in webcam. Hands perform actions (pinch-grab, 2-hand zoom, palm, fist)
// with coach visual demonstration. Face sets mood.
window.__moc.camera = initCamera({
  getG: () => cosmos.castG(G), // planets (#68): casts and mood reach the world you're on
  canCast: () => !!G?.scene && !banner && !intro.active && !G.finished && $('titleScreen').hidden && $('failScreen').hidden && $('endScreen').hidden,
  // camera debug (#67): the fingertip aims on the panned surface like the mouse does.
  // Aims are 0..1 of the world; the camera's overlay draws in 0..1 of the screen (#129).
  mapX: (x) => cosmos.mapX(x * W) / cosmos.worldW,
  screenX: (x) => (cosmos.surfaceInView ? x : cosmos.toScreen(x * cosmos.worldW, 0).x / W),
  screenY: (y) => (cosmos.surfaceInView ? y : cosmos.toScreen(0, y * cosmos.worldH).y / H),
  onZoomDelta: (delta, factor) => {
    // Two-hand continuous pinch zoom: delta > 0 is zoom in (hands spread), delta < 0 is zoom out (hands together)
    if (!G) return;
    if (in3dSurface() && (cosmos?.z || 0) <= 0.01) {
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
    if (cosmos.active || (cosmos?.z || 0) > 0.05) {
      if (active) {
        // In normalized hand space (-1..1). Moving hand right rotates globe right.
        const spin = -dx * 3.5;
        cosmos.rot += spin;
        cosmos.spinVel = spin / 0.033;
      }
      return;
    }
    // On the surface, 2D or 3D, a one-hand pinch drags the map (#156): it used to orbit the 3D
    // camera, which felt like the world tilting away. Orbiting stays on right-drag.
    if (active && (Math.abs(dx) > 0.001 || Math.abs(dy) > 0.001)) {
      if (in3dSurface()) cam3dBase = null;
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
if (typeof window !== 'undefined' && window.location?.search?.includes('rust=1')) {
  startRustApp().catch((err) => console.error('Failed to auto-start RustApp:', err));
} else if (window.claude?.hot?.ready) {
  window.claude.hot.ready(start);
} else {
  start(window.claude?.hot?.data ?? {});
}
