// Settings (#186), one panel behind the gear button in the top bar and on the title screen:
// comfort (reduced motion), head aim (the same saved values as the EEG panel), the calibration
// window that seeds your civilisation, the camera, and an Advanced group holding the Camera,
// Model and Debug buttons that used to crowd the top bar (#89). Everything is saved in
// localStorage and applied at boot; Escape closes the panel.

export const SETTINGS_KEY = 'moc.settings';
// reducedMotion: null follows the system setting (prefers-reduced-motion), true/false overrides it.
export const SETTINGS_DEFAULTS = Object.freeze({ reducedMotion: null, cameraOnStart: true, cursorSize: 'm', cursorContrast: false });
export const CURSOR_SIZES = ['s', 'm', 'l'];

function safeStorage() { try { return globalThis.localStorage ?? null; } catch { return null; } }

export function loadSettings(storage = safeStorage()) {
  let saved = {};
  try { saved = JSON.parse(storage?.getItem(SETTINGS_KEY) || '{}') || {}; } catch { saved = {}; }
  return {
    reducedMotion: typeof saved.reducedMotion === 'boolean' ? saved.reducedMotion : null,
    cameraOnStart: saved.cameraOnStart !== false,   // on by default; only an explicit opt-out disables it
    cursorSize: CURSOR_SIZES.includes(saved.cursorSize) ? saved.cursorSize : 'm',
    cursorContrast: saved.cursorContrast === true,
  };
}

export function saveSettings(settings, storage = safeStorage()) {
  try { storage?.setItem(SETTINGS_KEY, JSON.stringify({ reducedMotion: settings.reducedMotion, cameraOnStart: !!settings.cameraOnStart, cursorSize: CURSOR_SIZES.includes(settings.cursorSize) ? settings.cursorSize : 'm', cursorContrast: !!settings.cursorContrast })); } catch {}
}

// Whether motion is reduced: the player's choice, else the system's.
export function reducedMotionOn(settings, systemReduce = false) {
  return settings.reducedMotion ?? !!systemReduce;
}

function systemReducedMotion() {
  try { return !!globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches; } catch { return false; }
}

// Wire the panel in index.html. deps:
//   eegPanel        get/set the head-aim settings (EegPanel.prefs / setAimPrefs)
//   calibration     { read(): {seconds, mode}, write({seconds, mode}) } for the seed window
//   defaults        { aim, calibration } to reset to
//   saves           { count(), clear() } for "Delete all saves" (src/game/saves.js)
export function initSettings(doc, { eegPanel = null, calibration = null, defaults = {}, saves = null } = {}) {
  const $ = (id) => doc.getElementById(id);
  const panel = $('settingsPanel'), btn = $('settingsBtn');
  if (!panel || !btn) return null;
  const settings = loadSettings();
  const ui = {
    motion: $('setReducedMotion'), sens: $('setAimSens'), sensVal: $('setAimSensVal'),
    invYaw: $('setInvYaw'), invPitch: $('setInvPitch'),
    calibSeconds: $('setCalibSeconds'), calibMode: $('setCalibMode'), cameraOnStart: $('setCameraOnStart'),
    savesNote: $('setSavesNote'), deleteSaves: $('setDeleteSaves'),
    cursorSize: $('setCursorSize'), cursorContrast: $('setCursorContrast'),
  };

  function applyMotion() {
    doc.body.dataset.reducedMotion = reducedMotionOn(settings, systemReducedMotion()) ? '1' : '0';
  }
  // The aim cursor (#301): main.js reads these when it draws the reticle.
  function applyCursor() {
    doc.body.dataset.cursorSize = settings.cursorSize;
    doc.body.dataset.cursorContrast = settings.cursorContrast ? '1' : '0';
  }
  applyCursor();

  function fill() {
    if (ui.motion) ui.motion.checked = doc.body.dataset.reducedMotion === '1';
    if (ui.cursorSize) ui.cursorSize.value = settings.cursorSize;
    if (ui.cursorContrast) ui.cursorContrast.checked = !!settings.cursorContrast;
    const aim = eegPanel?.prefs;
    if (aim) {
      if (ui.sens) ui.sens.value = String(aim.sensitivity);
      if (ui.sensVal) ui.sensVal.textContent = `${Number(aim.sensitivity).toFixed(1)}×`;
      if (ui.invYaw) ui.invYaw.checked = !!aim.invertYaw;
      if (ui.invPitch) ui.invPitch.checked = !!aim.invertPitch;
    }
    const c = calibration?.read();
    if (c) {
      if (ui.calibSeconds) ui.calibSeconds.value = String(c.seconds);
      if (ui.calibMode) ui.calibMode.value = c.mode;
    }
    if (ui.cameraOnStart) ui.cameraOnStart.checked = settings.cameraOnStart;
    fillSaves();
  }

  // Saves: how many runs are kept, and a two-click "Delete all saves" (no dialog to get stuck on).
  let deleteArmed = false;
  function fillSaves() {
    deleteArmed = false;
    const n = saves?.count?.() ?? 0;
    if (ui.savesNote) ui.savesNote.textContent = n ? `${n} saved run${n === 1 ? '' : 's'} on this device. Each run saves itself when an era starts.` : 'No saved runs yet. Each run saves itself when an era starts.';
    if (ui.deleteSaves) { ui.deleteSaves.textContent = 'Delete all saves'; ui.deleteSaves.disabled = !n; }
  }

  const isOpen = () => !panel.hidden;
  function open() {
    fill();
    panel.hidden = false;
    btn.setAttribute('aria-pressed', 'true');
    // One side panel at a time.
    for (const [p, b] of [['eegPanel', 'eegBtn'], ['modelPanel', 'modelBtn'], ['camPanel', 'camBtn'], ['dbgPanel', 'dbgBtn']]) {
      const el = $(p);
      if (el && !el.hidden) { el.hidden = true; $(b)?.setAttribute('aria-pressed', 'false'); }
    }
  }
  function close() {
    panel.hidden = true;
    btn.setAttribute('aria-pressed', 'false');
  }
  const toggle = () => (isOpen() ? close() : open());

  btn.addEventListener('click', toggle);
  $('titleSettings')?.addEventListener('click', open);
  $('settingsClose')?.addEventListener('click', close);
  // The Advanced buttons (Camera, Model, Debug) open their own panels: make way for them.
  panel.querySelector('[data-set-advanced]')?.addEventListener('click', (e) => { if (e.target.closest?.('button')) close(); });

  ui.cursorSize?.addEventListener('change', () => {
    settings.cursorSize = CURSOR_SIZES.includes(ui.cursorSize.value) ? ui.cursorSize.value : 'm';
    saveSettings(settings);
    applyCursor();
  });
  ui.cursorContrast?.addEventListener('change', () => {
    settings.cursorContrast = !!ui.cursorContrast.checked;
    saveSettings(settings);
    applyCursor();
  });
  ui.motion?.addEventListener('change', () => {
    settings.reducedMotion = !!ui.motion.checked;
    saveSettings(settings);
    applyMotion();
  });
  const aimChanged = () => {
    const v = parseFloat(ui.sens?.value) || 1;
    if (ui.sensVal) ui.sensVal.textContent = `${v.toFixed(1)}×`;
    eegPanel?.setAimPrefs?.({ sensitivity: v, invertYaw: !!ui.invYaw?.checked, invertPitch: !!ui.invPitch?.checked });
  };
  ui.sens?.addEventListener('input', aimChanged);
  ui.invYaw?.addEventListener('change', aimChanged);
  ui.invPitch?.addEventListener('change', aimChanged);
  const calibChanged = () => {
    const seconds = Math.min(120, Math.max(1, Math.round(Number(ui.calibSeconds?.value) || 10)));
    calibration?.write({ seconds, mode: ui.calibMode?.value === 'sensor' ? 'sensor' : 'random' });
  };
  ui.calibSeconds?.addEventListener('change', calibChanged);
  ui.calibMode?.addEventListener('change', calibChanged);
  ui.cameraOnStart?.addEventListener('change', () => {
    settings.cameraOnStart = !!ui.cameraOnStart.checked;
    saveSettings(settings);
  });
  ui.deleteSaves?.addEventListener('click', () => {
    if (!deleteArmed) { deleteArmed = true; ui.deleteSaves.textContent = 'Click again to delete them all'; return; }
    saves?.clear?.();
    fillSaves();
  });
  $('settingsReset')?.addEventListener('click', () => {
    Object.assign(settings, SETTINGS_DEFAULTS);
    saveSettings(settings);
    applyMotion();
    if (defaults.aim) eegPanel?.setAimPrefs?.({ ...defaults.aim });
    if (defaults.calibration) calibration?.write({ ...defaults.calibration });
    fill();
  });

  applyMotion();
  return { settings, open, close, toggle, isOpen, fill };
}

// The top bar keeps a small "Camera on" indicator while the webcam runs, now that the Camera
// button lives in Settings (#89, #90: the player can always see the camera is on).
export function syncCameraIndicator(doc) {
  const live = doc.getElementById('camBtn')?.dataset.live === '1';
  const chip = doc.getElementById('camLive');
  if (chip && chip.hidden === live) chip.hidden = !live;
}
