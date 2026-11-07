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
import { Cosmos } from './cosmos/cosmos.js';
import { ERA_LABEL, ERA_NAMES, CHANNELS, fmtPop, planetDot, ownerLabel } from './cosmos/galaxy.js';

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
const cosmos = new Cosmos();
// What the HUD shows while you look at the galaxy.
const galaxyHudScene = { label: 'Galaxy', powers: ['calm', 'focus', 'heart'], objectives: () => cosmos.galaxy?.objectives() || [] };
let hudScene = null;
let G = null;
let fade = 0;           // 0..1 black overlay for transitions
let pendingScene = null;
let pendingTimer = 0;
let banner = null;      // {t}
let lastTs = 0;

function newGame(source, startIndex = 0) {
  if (G?.source && G.source !== source) G.source.disconnect();
  const brain = new Brain(source);
  cosmos.reset();
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
  cosmos.applyPan(low);   // the surface is a planet: it wraps east–west
  const cosmic = cosmos.render(low, lowCtx, G);

  const sx = G.shake ? (Math.random() - 0.5) * G.shake * K : 0;
  const sy = G.shake ? (Math.random() - 0.5) * G.shake * K : 0;
  main.setTransform(1, 0, 0, 1, 0, 0);
  main.imageSmoothingEnabled = false;
  main.fillStyle = '#05060f';
  main.fillRect(0, 0, screen.width, screen.height);
  main.drawImage(low, sx, sy, W * K, H * K);
  main.setTransform(K, 0, 0, K, sx, sy);
  if (!cosmic || cosmos.z < 0.3) {
    for (const off of cosmos.overlayOffsets()) {
      main.setTransform(K, 0, 0, K, sx + off * K, sy);
      main.globalAlpha = cosmic ? 1 - cosmos.z / 0.3 : 1;
      scene.drawOverlay?.(main);
    }
    main.globalAlpha = 1;
    main.setTransform(K, 0, 0, K, sx, sy);
  }
  if (cosmic) cosmos.overlay(main, G);
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
    } else if (G.sandbox) {
      G.runTime += dt;   // the hand-played game stays paused in the sandbox
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
    cosmos.update(dt, G);
    advisor.update(dt);
    const showGalaxyHud = cosmos.galaxy && (G.sandbox || cosmos.inGalaxy);
    const wanted = showGalaxyHud ? galaxyHudScene : G.scene;
    if (wanted !== hudScene) { hudScene = wanted; hud.setScene(wanted); }
    hud.update(dt, showGalaxyHud ? Object.assign(Object.create(G), { scene: galaxyHudScene }) : G);
    updateCosmosUi(dt);
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

function worldFromEvent(e) { const a = aimFromEvent(e); return { x: a.x * W, y: a.y * H }; }
// Aim on the (panned, wrapping) surface: screen position → map position.
function surfaceAim(e) { const a = aimFromEvent(e); return { x: cosmos.mapX(a.x * W) / W, y: a.y }; }
let panDrag = null;

screen.addEventListener('pointermove', (e) => {
  if (G && cosmos.active) { const p = worldFromEvent(e); cosmos.pointerMove(e.pointerId, p.x, p.y, G); return; }
  if (panDrag && panDrag.id === e.pointerId) { const x = worldFromEvent(e).x; cosmos.pan(x - panDrag.x); panDrag.x = x; return; }
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
  // Right or middle drag pans the planet's surface; left click stays the blink.
  if (G && (e.button === 1 || e.button === 2)) {
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
  // Horizontal scroll (or Shift+scroll) pans the surface; vertical scroll zooms.
  const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey;
  if (horizontal && !cosmos.active) { cosmos.pan(-(e.shiftKey ? e.deltaY : e.deltaX) * 0.15); return; }
  cosmos.wheel(e.deltaY, G);
}, { passive: false });
screen.addEventListener('contextmenu', (e) => e.preventDefault());

const KEYS = { Space: 'calm', KeyF: 'focus', ShiftLeft: 'panic', ShiftRight: 'panic' };
window.addEventListener('keydown', (e) => {
  if (e.target.closest?.('input')) return;
  if (e.code === 'KeyM') { toggleModel(); return; }
  if (G && (e.code === 'KeyZ' || e.code === 'Minus' || e.code === 'NumpadSubtract')) { cosmos.zoomOut(G); return; }
  if (G && (e.code === 'KeyX' || e.code === 'Equal' || e.code === 'NumpadAdd')) { cosmos.zoomIn(G); return; }
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
    : `Reading your mind: ${Math.max(0, Math.ceil(10 - m.t))} s of play left.`;
  $('galaxyLog').replaceChildren(...gal.log.slice(0, 4).map((l) => { const li = document.createElement('li'); li.textContent = l.text; return li; }));
}

$('zoomIn').addEventListener('click', () => { if (G) { audio.unlock(); cosmos.zoomIn(G); } });
$('zoomOut').addEventListener('click', () => { if (G) { audio.unlock(); cosmos.zoomOut(G); } });
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
$('playSim').addEventListener('click', () => begin(new SimSource()));
$('playSandbox').addEventListener('click', () => {
  begin(new SimSource());
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
window.__moc = { get G() { return G; }, startScene: (i) => startScene(i), cosmos };

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
