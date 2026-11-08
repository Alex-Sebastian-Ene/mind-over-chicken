// EEG & Biosensing Side Panel: connect/disconnect Muse 2 bridge at any time,
// inspect electrode contact (TP9, AF7, AF8, TP10), 4-channel oscilloscope,
// 5-band frequency spectrum, mental state (calm, focus, stress: EEG agitation or face anger),
// and 3-axis Accelerometer & Gyroscope waveforms + head pose calibration & cursor pad.

const BANDS = [
  { key: 'delta', label: 'Delta (1–4Hz)', color: '#f38ba8' },
  { key: 'theta', label: 'Theta (4–8Hz)', color: '#fab387' },
  { key: 'alpha', label: 'Alpha (8–13Hz)', color: '#7ee08a' },
  { key: 'beta',  label: 'Beta (13–30Hz)', color: '#59d8c8' },
  { key: 'gamma', label: 'Gamma (30–45Hz)', color: '#cba6f7' },
];

const CHANNELS = [
  { key: 'TP9',  label: 'TP9 (L Ear)',      color: '#fab387' },
  { key: 'AF7',  label: 'AF7 (L Forehead)', color: '#59d8c8' },
  { key: 'AF8',  label: 'AF8 (R Forehead)', color: '#89b4fa' },
  { key: 'TP10', label: 'TP10 (R Ear)',     color: '#cba6f7' },
];

const IMU_AXES = [
  { idx: 0, label: 'X', color: '#f38ba8' },
  { idx: 1, label: 'Y', color: '#7ee08a' },
  { idx: 2, label: 'Z', color: '#89b4fa' },
];

// Brain.aim is world-space on the bigger home world (#129); every calibration preview, the aim
// pad and the sandbox test are screen-space, so read the screen aim that Brain maps from.
const screenAimOf = (brain) => brain?.screenAim || brain?.aim || null;

const TEMPLATE = `
  <header class="eeg-head">
    <h2 id="eegHeading">EEG &amp; Biosensing</h2>
    <button type="button" class="tool" data-eeg-close title="Close panel (E)">Close <kbd>E</kbd></button>
  </header>

  <section class="eeg-sec eeg-conn">
    <div class="eeg-status-row">
      <span class="eeg-dot" data-eeg-dot></span>
      <strong data-eeg-state>Not connected</strong>
      <span class="eeg-qual" data-eeg-qual>--</span>
    </div>
    <p class="eeg-msg" data-eeg-msg>Turn your Muse 2 on (LEDs pulsing, not paired in OS settings) and click Connect.</p>
    <div class="eeg-url-row">
      <button type="button" class="btn btn-primary btn-small" data-eeg-connect>Connect Muse 2</button>
      <button type="button" class="btn btn-small" data-eeg-disconnect disabled>Disconnect</button>
      <button type="button" class="btn btn-primary btn-small" data-eeg-wizard disabled title="Guided visual calibration: follow the white dot with your head, then blink when the screen flashes">🎯 Guided Calibration (Dot + Flash)</button>
    </div>
    <details class="eeg-adv">
      <summary>Bridge WebSocket URL</summary>
      <input type="text" data-eeg-url value="ws://localhost:8765" spellcheck="false" autocomplete="off" aria-label="Bridge WebSocket URL">
    </details>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Head Cursor &amp; Motion Calibration</h3>
      <div class="eeg-btn-group">
        <button type="button" class="btn btn-small" data-eeg-dot-calib disabled title="Calibrate 9 separate dots at the full edges & corners of the screen (Up/Down/Left/Right)">⚪ 9-Dot Edge Calib</button>
        <button type="button" class="btn btn-primary btn-small" data-eeg-recenter title="Look at the center of the screen and click (or press Shift+C) to zero Pitch/Yaw and recalibrate gyro drift" disabled>Center <kbd>Shift+C</kbd></button>
      </div>
    </div>
    <div class="eeg-calib-row">
      <span data-imu-calib-txt>Gyro Drift Lock: --</span>
      <span class="eeg-bar"><span data-imu-calib-bar style="background:var(--good)"></span></span>
    </div>
    <div class="eeg-cursor-grid">
      <canvas class="eeg-pad" width="136" height="84" aria-label="2D Head Aim Pad"></canvas>
      <div class="eeg-cursor-controls">
        <label class="eeg-slider-lbl">
          <span>Sensitivity (<b data-imu-sens-val>1.0×</b>)</span>
          <input type="range" data-imu-sens min="0.5" max="2.5" step="0.1" value="1.0">
        </label>
        <div class="eeg-chk-row">
          <label><input type="checkbox" data-imu-inv-yaw> Flip Yaw (L/R)</label>
          <label><input type="checkbox" data-imu-inv-pitch> Flip Pitch (U/D)</label>
        </div>
        <div class="eeg-imu-mini">
          <span>ΔYaw: <b data-imu-yaw>--°</b></span>
          <span>ΔPitch: <b data-imu-pitch>--°</b></span>
          <span>ΔRoll: <b data-imu-roll>--°</b></span>
        </div>
      </div>
    </div>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Eye Blink Calibration (Rust/WASM Model)</h3>
      <span class="eeg-qual" data-blink-status>Untrained</span>
    </div>
    <p class="eeg-msg" data-blink-hint>Use <b>⚡ Flash Calibration</b> (blinks on visual flash cues) or click <b>Record Blink</b> right after blinking.</p>
    <div class="eeg-conn-row">
      <button type="button" class="btn btn-primary btn-small" data-blink-flash-calib disabled>⚡ Flash Calibration</button>
      <button type="button" class="btn btn-small" data-blink-record disabled>+1 Blink</button>
      <button type="button" class="btn btn-ghost btn-small" data-blink-rest disabled>+1 Rest</button>
      <button type="button" class="btn btn-ghost btn-small" data-blink-reset disabled>Reset</button>
    </div>
    <div class="eeg-band-row" style="margin-top:6px">
      <span class="eeg-band-lbl">Blink Probability</span>
      <span class="eeg-bar"><span data-blink-prob-bar style="background:var(--blink);width:0%"></span></span>
      <span class="eeg-band-val" data-blink-prob-val>0%</span>
    </div>
    <div class="eeg-imu-mini" style="margin-top:6px">
      <span>Blinks: <b data-blink-pos>0</b></span>
      <span>Rest: <b data-blink-neg>0</b></span>
      <span>Thr: <b data-blink-thr>155 uV</b></span>
      <span>Peak: <b data-blink-uv>-- uV</b></span>
    </div>
    <div class="eeg-log" data-eeg-log aria-live="polite"></div>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Accelerometer (g) &amp; Gyroscope (°/s)</h3>
      <span class="eeg-legend">
        <i style="color:#f38ba8">● X</i>
        <i style="color:#7ee08a">● Y</i>
        <i style="color:#89b4fa">● Z</i>
      </span>
    </div>
    <div class="eeg-imu-xyz">
      <div><span>Accel X/Y/Z (g)</span><b data-imu-accel>--</b></div>
      <div><span>Gyro X/Y/Z (°/s)</span><b data-imu-gyro>--</b></div>
    </div>
    <canvas class="eeg-imu-scope" data-scope-accel width="356" height="54" aria-label="3-axis accelerometer waveform"></canvas>
    <canvas class="eeg-imu-scope" data-scope-gyro width="356" height="54" aria-label="3-axis gyroscope waveform"></canvas>
  </section>

  <section class="eeg-sec">
    <h3>Electrode Contact Quality</h3>
    <div class="eeg-contacts">
      ${CHANNELS.map((c) => `<div class="eeg-badge" data-contact="${c.key}"><b>${c.key}</b><span>OFF</span></div>`).join('')}
    </div>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Live EEG Waveforms (µV)</h3>
      <span class="eeg-legend">${CHANNELS.map((c) => `<i style="color:${c.color}">● ${c.key}</i>`).join(' ')}</span>
    </div>
    <canvas class="eeg-scope" width="356" height="104" aria-label="4-channel EEG oscilloscope"></canvas>
  </section>

  <section class="eeg-sec">
    <h3>Brainwave Frequency Spectrum</h3>
    <div class="eeg-bands">
      ${BANDS.map((b) => `
        <div class="eeg-band-row" data-band="${b.key}">
          <span class="eeg-band-lbl">${b.label}</span>
          <span class="eeg-bar"><span style="background:${b.color}"></span></span>
          <span class="eeg-band-val">--</span>
        </div>`).join('')}
    </div>
  </section>

  <section class="eeg-sec">
    <h3>Mental State</h3>
    <div class="eeg-bands">
      <div class="eeg-band-row" data-metric="calm">
        <span class="eeg-band-lbl">Calm (Alpha/Beta)</span>
        <span class="eeg-bar"><span style="background:var(--calm)"></span></span>
        <span class="eeg-band-val">--</span>
      </div>
      <div class="eeg-band-row" data-metric="focus">
        <span class="eeg-band-lbl">Focus (Beta/Theta)</span>
        <span class="eeg-bar"><span style="background:var(--focus)"></span></span>
        <span class="eeg-band-val">--</span>
      </div>
      <div class="eeg-band-row" data-metric="stress">
        <span class="eeg-band-lbl">Stress (agitation, anger)</span>
        <span class="eeg-bar"><span style="background:var(--alarm)"></span></span>
        <span class="eeg-band-val">--</span>
      </div>
    </div>
    <div class="eeg-vitals">
      <div><span>😠 Face anger</span><b data-eeg-anger>--</b></div>
      <div><span>⚡ Blinks</span><b data-eeg-blinks>--</b></div>
      <div><span>🎯 Motion</span><b data-eeg-motion>--</b></div>
    </div>
  </section>
`;

// The calibration wizard's clock (#176): real time minus the time the headset wasn't streaming,
// so a phase can't run out, record or finish on a dead stream. tick(live) once per frame.
export class PausableClock {
  constructor(realNow = () => performance.now()) {
    this.realNow = realNow;
    this.pausedMs = 0;
    this.last = realNow();
  }
  tick(live) {
    const r = this.realNow();
    if (!live) this.pausedMs += r - this.last;
    this.last = r;
    return r - this.pausedMs;
  }
  now() { return this.realNow() - this.pausedMs; }
}

// Head-aim settings (sensitivity, yaw and pitch inversion) survive a reload (#186), under
// localStorage "moc.aimPrefs". Bad or missing values fall back to the defaults.
export const AIM_PREFS_KEY = 'moc.aimPrefs';
const AIM_DEFAULTS = { sensitivity: 1.0, invertYaw: false, invertPitch: false };
function prefsStorage() { try { return globalThis.localStorage ?? null; } catch { return null; } }
export function loadAimPrefs(storage = prefsStorage()) {
  let saved = {};
  try { saved = JSON.parse(storage?.getItem(AIM_PREFS_KEY) || '{}') || {}; } catch { saved = {}; }
  const s = Number(saved.sensitivity);
  return {
    sensitivity: Number.isFinite(s) ? Math.min(2.5, Math.max(0.5, s)) : AIM_DEFAULTS.sensitivity,
    invertYaw: saved.invertYaw === true,
    invertPitch: saved.invertPitch === true,
  };
}
export function saveAimPrefs(prefs, storage = prefsStorage()) {
  try { storage?.setItem(AIM_PREFS_KEY, JSON.stringify({ sensitivity: prefs.sensitivity, invertYaw: !!prefs.invertYaw, invertPitch: !!prefs.invertPitch })); } catch {}
}

export class EegPanel {
  constructor(doc, { getSource, getBrain, onConnectBridge, onDisconnectBridge }) {
    this.doc = doc;
    this.btn = doc.getElementById('eegBtn');
    this.panel = doc.getElementById('eegPanel');
    this.getSource = getSource;
    this.getBrain = getBrain;
    this.onConnectBridge = onConnectBridge;
    this.onDisconnectBridge = onDisconnectBridge;
    if (!this.btn || !this.panel) return;

    this.panel.innerHTML = TEMPLATE;
    const $ = (s) => this.panel.querySelector(s);

    this.ui = {
      dot: $('[data-eeg-dot]'),
      state: $('[data-eeg-state]'),
      qual: $('[data-eeg-qual]'),
      url: $('[data-eeg-url]'),
      connectBtn: $('[data-eeg-connect]'),
      disconnectBtn: $('[data-eeg-disconnect]'),
      wizardBtn: $('[data-eeg-wizard]'),
      dotCalibBtn: $('[data-eeg-dot-calib]'),
      flashCalibBtn: $('[data-blink-flash-calib]'),
      msg: $('[data-eeg-msg]'),
      recenterBtn: $('[data-eeg-recenter]'),
      calibTxt: $('[data-imu-calib-txt]'),
      calibBar: $('[data-imu-calib-bar]'),
      blinkStatus: $('[data-blink-status]'),
      blinkHint: $('[data-blink-hint]'),
      blinkRecordBtn: $('[data-blink-record]'),
      blinkRestBtn: $('[data-blink-rest]'),
      blinkResetBtn: $('[data-blink-reset]'),
      blinkProbBar: $('[data-blink-prob-bar]'),
      blinkProbVal: $('[data-blink-prob-val]'),
      blinkPos: $('[data-blink-pos]'),
      blinkNeg: $('[data-blink-neg]'),
      blinkThr: $('[data-blink-thr]'),
      blinkUv: $('[data-blink-uv]'),
      logBox: $('[data-eeg-log]'),
      pad: $('.eeg-pad'),
      sensInput: $('[data-imu-sens]'),
      sensVal: $('[data-imu-sens-val]'),
      invYaw: $('[data-imu-inv-yaw]'),
      invPitch: $('[data-imu-inv-pitch]'),
      scope: $('.eeg-scope'),
      accelScope: $('[data-scope-accel]'),
      gyroScope: $('[data-scope-gyro]'),
      contacts: Object.fromEntries(CHANNELS.map((c) => [c.key, $(`[data-contact="${c.key}"]`)])),
      bands: Object.fromEntries(BANDS.map((b) => [b.key, {
        fill: $(`[data-band="${b.key}"] .eeg-bar > span`),
        val: $(`[data-band="${b.key}"] .eeg-band-val`),
      }])),
      calm: {
        fill: $('[data-metric="calm"] .eeg-bar > span'),
        val: $('[data-metric="calm"] .eeg-band-val'),
      },
      focus: {
        fill: $('[data-metric="focus"] .eeg-bar > span'),
        val: $('[data-metric="focus"] .eeg-band-val'),
      },
      stress: {
        fill: $('[data-metric="stress"] .eeg-bar > span'),
        val: $('[data-metric="stress"] .eeg-band-val'),
      },
      anger: $('[data-eeg-anger]'),
      blinks: $('[data-eeg-blinks]'),
      motion: $('[data-eeg-motion]'),
      accelText: $('[data-imu-accel]'),
      gyroText: $('[data-imu-gyro]'),
      pitch: $('[data-imu-pitch]'),
      roll: $('[data-imu-roll]'),
      yaw: $('[data-imu-yaw]'),
    };
    this.padCtx = this.ui.pad.getContext('2d');
    this.scopeCtx = this.ui.scope.getContext('2d');
    this.accelCtx = this.ui.accelScope.getContext('2d');
    this.gyroCtx = this.ui.gyroScope.getContext('2d');
    this.connecting = false;
    this.prefs = loadAimPrefs();   // saved by the controls below (#186)
    this.prefsSource = null;       // the source they were last applied to
    this.ui.sensInput.value = String(this.prefs.sensitivity);
    this.ui.sensVal.textContent = `${this.prefs.sensitivity.toFixed(1)}×`;
    this.ui.invYaw.checked = this.prefs.invertYaw;
    this.ui.invPitch.checked = this.prefs.invertPitch;
    this.logs = [];
    this.lastLoggedBlinkCount = 0;
    this.lastLoggedSampleCount = 0;
    this.lastCandidateLogAt = 0;
    this.wasStreaming = false;
    this.wiz = null;
    this.buildWizardModal();
    // Closed until asked for (the EEG button or E): open on load it covered the playfield and the
    // era card's Begin button (#182). ?eeg=1 opens it on load for headset sessions.
    this.toggle(new URLSearchParams(globalThis.location?.search || '').has('eeg'));
    this.addLog('EEG & Rust/WASM calibration panel ready.');

    this.btn.addEventListener('click', () => this.toggle());
    $('[data-eeg-close]').addEventListener('click', () => this.toggle(false));

    this.ui.connectBtn.addEventListener('click', () => this.handleConnect());
    this.ui.disconnectBtn.addEventListener('click', () => this.handleDisconnect());
    this.ui.wizardBtn?.addEventListener('click', () => this.startWizard('both'));
    this.ui.dotCalibBtn?.addEventListener('click', () => this.startWizard('motion'));
    this.ui.flashCalibBtn?.addEventListener('click', () => this.startWizard('blink'));
    this.ui.recenterBtn.addEventListener('click', () => this.calibrateCenter());
    this.ui.blinkRecordBtn.addEventListener('click', () => this.handleRecordBlink());
    this.ui.blinkRestBtn.addEventListener('click', () => this.handleRecordBaseline());
    this.ui.blinkResetBtn.addEventListener('click', () => this.handleResetBlinkModel());

    this.ui.sensInput.addEventListener('input', () => {
      const v = parseFloat(this.ui.sensInput.value) || 1.0;
      this.prefs.sensitivity = v;
      this.ui.sensVal.textContent = `${v.toFixed(1)}×`;
      const src = this.getSource();
      if (src) src.sensitivity = v;
      saveAimPrefs(this.prefs);
    });
    this.ui.invYaw.addEventListener('change', () => {
      this.prefs.invertYaw = this.ui.invYaw.checked;
      const src = this.getSource();
      if (src) src.invertYaw = this.prefs.invertYaw;
      saveAimPrefs(this.prefs);
    });
    this.ui.invPitch.addEventListener('change', () => {
      this.prefs.invertPitch = this.ui.invPitch.checked;
      const src = this.getSource();
      if (src) src.invertPitch = this.prefs.invertPitch;
      saveAimPrefs(this.prefs);
    });
  }

  // Set, save and apply the head-aim settings from elsewhere (the Settings panel, #186), keeping
  // this panel's controls in step.
  setAimPrefs(p) {
    this.prefs = { sensitivity: Number(p.sensitivity) || 1, invertYaw: !!p.invertYaw, invertPitch: !!p.invertPitch };
    if (this.ui) {
      this.ui.sensInput.value = String(this.prefs.sensitivity);
      this.ui.sensVal.textContent = `${this.prefs.sensitivity.toFixed(1)}×`;
      this.ui.invYaw.checked = this.prefs.invertYaw;
      this.ui.invPitch.checked = this.prefs.invertPitch;
    }
    saveAimPrefs(this.prefs);
    const src = this.getSource?.();
    if (src?.kind === 'bridge') this.applyPrefsToSource(src);
  }

  applyPrefsToSource(src) {
    if (!src) return;
    src.sensitivity = this.prefs.sensitivity;
    src.invertYaw = this.prefs.invertYaw;
    src.invertPitch = this.prefs.invertPitch;
  }

  addLog(text) {
    const now = new Date();
    const ts = now.toTimeString().slice(0, 8);
    const line = `[${ts}] ${text}`;
    console.info('[MOC-EEG]', line);
    this.logs.unshift(line);
    if (this.logs.length > 25) this.logs.length = 25;
    if (this.ui?.logBox) {
      this.ui.logBox.textContent = this.logs.join('\n');
    }
    if (this.wiz?.logEl) {
      this.wiz.logEl.textContent = this.logs.slice(0, 8).join('\n');
    }
  }

  buildWizardModal() {
    if (!this.doc?.body) return;
    const modal = this.doc.createElement('section');
    modal.className = 'calib-wiz';
    modal.hidden = true;
    modal.innerHTML = `
      <div class="calib-wiz-top">
        <div>
          <span class="calib-wiz-step" data-wiz-step>STEP 1 OF 3 · HYBRID SMOOTH-PURSUIT + BOUNDARY + REVERSE CALIBRATION</span>
          <h2 data-wiz-title>Follow the Moving White Dot Out to Each Edge, Hold, and Reverse Back</h2>
          <p data-wiz-sub>Smoothly follow the glowing white dot with your head as it sweeps out to each boundary, pauses at the edge, and reverses back through Center. <b>DO NOT BLINK</b>.</p>
        </div>
        <div class="calib-wiz-actions">
          <button type="button" class="btn btn-small btn-primary" data-wiz-lock hidden>Lock Point</button>
          <button type="button" class="btn btn-small" data-wiz-skip>Skip Step</button>
          <button type="button" class="btn btn-small" data-wiz-close>Close <kbd>Esc</kbd></button>
        </div>
      </div>
      <div class="calib-wiz-stage">
        <canvas class="calib-wiz-canvas" width="960" height="500"></canvas>
      </div>
      <div class="calib-wiz-bottom">
        <div class="calib-wiz-stats" data-wiz-stats>Waiting for Muse 2 telemetry…</div>
        <pre class="calib-wiz-log" data-wiz-log></pre>
      </div>
    `;
    this.doc.body.appendChild(modal);
    const canvas = modal.querySelector('.calib-wiz-canvas');
    this.wiz = {
      modal,
      canvas,
      ctx: canvas.getContext('2d'),
      stepEl: modal.querySelector('[data-wiz-step]'),
      titleEl: modal.querySelector('[data-wiz-title]'),
      subEl: modal.querySelector('[data-wiz-sub]'),
      statsEl: modal.querySelector('[data-wiz-stats]'),
      logEl: modal.querySelector('[data-wiz-log]'),
      lockBtn: modal.querySelector('[data-wiz-lock]'),
      skipBtn: modal.querySelector('[data-wiz-skip]'),
      closeBtn: modal.querySelector('[data-wiz-close]'),
      active: false,
      mode: 'both',
      phase: 'motion', // 'motion' | 'noise' | 'blink' | 'sandbox'
      t0: 0,
      lastFrameNow: 0,
      lastSampleAt: 0,
      lastNoiseSampleAt: 0,
      dotIdx: 0,
      dotEnteredAt: 0,
      dotHoldSec: 0,
      dotSamplesTaken: 0,
      taredAtCenter: false,
      flashIdx: -1,
      flashRestRecorded: false,
      flashTriggered: false,
      sandboxBolts: [],
      lastBlinkCount: 0,
      dots: [
        { x: 0.50, y: 0.50, short: 'CENTER',       holdReq: 1.5, prompt: '1/9 · Look at CENTER & HOLD STILL — DO NOT BLINK' },
        { x: 0.50, y: 0.04, short: 'TOP EDGE ↑',   holdReq: 1.6, prompt: '2/9 · Tilt Head UP to FULL TOP EDGE & HOLD — DO NOT BLINK' },
        { x: 0.50, y: 0.96, short: 'BOTTOM EDGE ↓',holdReq: 1.6, prompt: '3/9 · Tilt Head DOWN to FULL BOTTOM EDGE & HOLD — DO NOT BLINK' },
        { x: 0.04, y: 0.50, short: 'LEFT EDGE ←',  holdReq: 1.5, prompt: '4/9 · Turn Head LEFT to FULL LEFT EDGE & HOLD — DO NOT BLINK' },
        { x: 0.96, y: 0.50, short: 'RIGHT EDGE →', holdReq: 1.5, prompt: '5/9 · Turn Head RIGHT to FULL RIGHT EDGE & HOLD — DO NOT BLINK' },
        { x: 0.06, y: 0.06, short: 'TOP-LEFT ↖',   holdReq: 1.4, prompt: '6/9 · Tilt UP-LEFT to Corner & HOLD — DO NOT BLINK' },
        { x: 0.94, y: 0.06, short: 'TOP-RIGHT ↗',  holdReq: 1.4, prompt: '7/9 · Tilt UP-RIGHT to Corner & HOLD — DO NOT BLINK' },
        { x: 0.06, y: 0.94, short: 'BOT-LEFT ↙',   holdReq: 1.4, prompt: '8/9 · Tilt DOWN-LEFT to Corner & HOLD — DO NOT BLINK' },
        { x: 0.94, y: 0.94, short: 'BOT-RIGHT ↘',  holdReq: 1.4, prompt: '9/9 · Tilt DOWN-RIGHT to Corner & HOLD — DO NOT BLINK' },
      ],
    };
    this.wiz.closeBtn?.addEventListener('click', () => this.closeWizard());
    this.wiz.skipBtn?.addEventListener('click', () => this.advanceWizardPhase());
    this.wiz.lockBtn?.addEventListener('click', () => this.lockCurrentDotManually());
    this.wiz.canvas?.addEventListener('click', () => {
      if (this.wiz?.active && this.wiz.phase === 'motion' && this.wiz.lockBtn && !this.wiz.lockBtn.hidden) {
        this.lockCurrentDotManually();
      }
    });
  }

  lockCurrentDotManually() {
    if (!this.wiz || !this.wiz.active || this.wiz.phase !== 'motion') return;
    const src = this.getSource();
    const b = this.getBrain();
    const dot = this.wiz.dots[this.wiz.dotIdx];
    if (!dot) return;
    if (this.wiz.dotIdx === 0 && !this.wiz.taredAtCenter) {
      this.wiz.taredAtCenter = true;
      src?.beginMotionCalibration?.();
    }
    if (src?.addMotionCalibPoint) {
      for (let k = 0; k < 6; k++) src.addMotionCalibPoint(dot.x, dot.y);
    }
    if (src?.recordBaselineSample) {
      src.recordBaselineSample();
    }
    const rel = b?.relOrientation || { yaw: 0, pitch: 0 };
    this.addLog(`Locked Dot #${this.wiz.dotIdx + 1}/9 (${dot.short}) at ΔYaw=${rel.yaw.toFixed(1)}° ΔPitch=${rel.pitch.toFixed(1)}° (no-blink baseline saved)`);
    this.advanceToNextDot();
  }

  advanceToNextDot() {
    if (!this.wiz) return;
    this.wiz.dotIdx += 1;
    this.wiz.dotEnteredAt = this.wiz.clock ? this.wiz.clock.now() : performance.now();
    this.wiz.dotHoldSec = 0;
    this.wiz.dotSamplesTaken = 0;
    if (this.wiz.dotIdx >= this.wiz.dots.length) {
      this.advanceWizardPhase();
    }
  }

  startWizard(mode = 'both') {
    if (!this.wiz) return;
    const src = this.getSource();
    const b = this.getBrain();
    this.wiz.active = true;
    this.wiz.clock = new PausableClock();
    this.wiz.lost = false;
    this.wiz.mode = mode;
    this.wiz.modal.hidden = false;
    this.wiz.lastBlinkCount = b?.stats?.blinks ?? 0;
    this.wiz.sandboxBolts = [];
    if (mode === 'blink') {
      this.enterWizardPhase('noise');
    } else {
      this.enterWizardPhase('motion');
    }
    if (src?.kind === 'bridge' && !src.streaming) {
      this.addLog('Guided Calibration opened (waiting for Muse 2 stream...)');
    }
  }

  enterWizardPhase(phase) {
    if (!this.wiz) return;
    const src = this.getSource();
    const now = this.wiz.clock ? this.wiz.clock.now() : performance.now();
    this.wiz.phase = phase;
    this.wiz.t0 = now;
    this.wiz.lastFrameNow = now;
    this.wiz.lastSampleAt = 0;
    this.wiz.lastNoiseSampleAt = 0;
    this.wiz.dotIdx = 0;
    this.wiz.dotEnteredAt = now;
    this.wiz.dotHoldSec = 0;
    this.wiz.dotSamplesTaken = 0;
    this.wiz.taredAtCenter = false;
    this.wiz.flashIdx = -1;
    this.wiz.flashRestRecorded = false;
    this.wiz.flashTriggered = false;

    if (phase === 'motion') {
      this.wiz.stepEl.textContent = this.wiz.mode === 'both'
        ? 'STEP 1 OF 3 · HYBRID PURSUIT + BOUNDARY HOLD + REVERSE CALIBRATION'
        : 'HYBRID PURSUIT + BOUNDARY + REVERSE HEAD CALIBRATION (RUST ML)';
      this.wiz.titleEl.textContent = 'Follow the White Dot Out to Each Boundary, Pause at Edge & Reverse Back';
      this.wiz.subEl.textContent = 'Follow the white dot smoothly with your head: Center → Left Edge (hold) → Reverse to Center → Right Edge (hold) → Top Edge (hold) → Bottom Edge (hold) → 4-Corner Figure-8. Keep eyes open (DO NOT BLINK).';
      if (this.wiz.lockBtn) this.wiz.lockBtn.hidden = true;
      this.wiz.skipBtn.textContent = this.wiz.mode === 'both' ? 'Skip to Noise Canceler →' : 'Finish';
      this.addLog('Started Hybrid Smooth-Pursuit + Boundary-Hold + Reverse Head Calibration (24.5s).');
    } else if (phase === 'noise') {
      this.wiz.stepEl.textContent = 'STEP 2 OF 3 · HEAD-MOTION EEG NOISE CANCELLATION (DO NOT BLINK)';
      this.wiz.titleEl.textContent = 'Move Your Head Around Freely — DO NOT BLINK!';
      this.wiz.subEl.textContent = 'For 4.5 seconds, nod UP/DOWN and turn LEFT/RIGHT while keeping your eyes open (NO BLINKING). This teaches Rust to reject muscle & motion noise!';
      if (this.wiz.lockBtn) this.wiz.lockBtn.hidden = true;
      this.wiz.skipBtn.textContent = 'Next: Flash Blink Cues →';
      this.addLog('Recording 4.5s of head-motion non-blink EEG noise so head movement never triggers random blinks...');
    } else if (phase === 'blink') {
      this.wiz.stepEl.textContent = 'STEP 3 OF 3 · EYE BLINK FLASH CALIBRATION (RUST 4-CH EEG)';
      this.wiz.titleEl.textContent = 'Hold Eyes Open… Then BLINK When the Screen Flashes Gold!';
      this.wiz.subEl.textContent = 'During the blue ring ("HOLD — DO NOT BLINK"), keep eyes still. When the screen FLASHES GOLD ("⚡ BLINK NOW!"), do one clear blink.';
      if (this.wiz.lockBtn) this.wiz.lockBtn.hidden = true;
      this.wiz.skipBtn.textContent = 'Test in Sandbox →';
      this.addLog('Started 6-Cue Eye Blink Flash Calibration.');
    } else {
      this.wiz.stepEl.textContent = 'CALIBRATION COMPLETE · FULL-EDGE SANDBOX TEST';
      this.wiz.titleEl.textContent = 'Test Full Screen Edges (Up/Down/Left/Right) & Eye Blinks!';
      this.wiz.subEl.textContent = 'Steer the green head cursor to the TOP, BOTTOM, LEFT, and RIGHT edge targets, and BLINK to strike lightning!';
      if (this.wiz.lockBtn) this.wiz.lockBtn.hidden = true;
      this.wiz.skipBtn.textContent = 'Done — Return to Game';
      if (src?.cancelMotionCalibration) src.cancelMotionCalibration();
      this.addLog('Calibration Sandbox ready: test reaching full Top/Bottom/Left/Right edges and blinking.');
    }
  }

  advanceWizardPhase() {
    if (!this.wiz || !this.wiz.active) return;
    if (this.wiz.lost && this.wiz.phase !== 'sandbox') return;   // never finish a step on a dead stream (#176)
    const src = this.getSource();
    if (this.wiz.phase === 'motion') {
      if (src?.finishMotionCalibration) {
        const res = src.finishMotionCalibration();
        if (res.ok) {
          this.addLog(`Rust Motion ML trained (${res.samples} pts, R²=${Math.round(res.r2 * 100)}%, YawEdge=${res.yawEdgeDeg.toFixed(1)}° sign=${res.yawSign > 0 ? '+' : '-'}, PitchEdge=${res.pitchEdgeDeg.toFixed(1)}° sign=${res.pitchSign > 0 ? '+' : '-'})`);
        } else {
          src.cancelMotionCalibration?.();
          this.addLog(`Motion calibration skipped/insufficient samples (${res.samples} pts). Using 3D gravity tare.`);
        }
      }
      if (this.wiz.mode === 'both') {
        this.enterWizardPhase('noise');
      } else {
        this.enterWizardPhase('sandbox');
      }
    } else if (this.wiz.phase === 'noise') {
      this.enterWizardPhase('blink');
    } else if (this.wiz.phase === 'blink') {
      this.enterWizardPhase('sandbox');
    } else {
      this.closeWizard();
    }
  }

  closeWizard() {
    if (!this.wiz) return;
    const src = this.getSource();
    if (this.wiz.phase === 'motion' && src?.cancelMotionCalibration) {
      src.cancelMotionCalibration();
    }
    this.wiz.active = false;
    this.wiz.modal.hidden = true;
    this.addLog('Closed Guided Calibration Wizard.');
  }

  updateWizard(hasRealStream) {
    if (!this.wiz || !this.wiz.active) return;
    const src = this.getSource();
    const b = this.getBrain();
    const aim = screenAimOf(b);
    // #176: no live frames, no calibration. The wizard clock stands still, nothing is recorded,
    // no step can finish, and the wizard says so; it carries on where it was once frames return.
    const now = this.wiz.clock ? this.wiz.clock.tick(hasRealStream) : performance.now();
    const lost = !hasRealStream && this.wiz.phase !== 'sandbox';
    if (lost !== this.wiz.lost) {
      this.wiz.lost = lost;
      if (this.wiz.skipBtn) this.wiz.skipBtn.disabled = lost;
      this.addLog(lost ? 'Headset lost: calibration paused until the stream is back.' : 'Headset back: calibration resumes.');
    }
    if (lost) {
      const ctx = this.wiz.ctx, w = this.wiz.canvas.width, h = this.wiz.canvas.height;
      this.wiz.lastFrameNow = now;
      ctx.fillStyle = 'rgba(8, 10, 24, 0.82)';
      ctx.fillRect(0, 0, w, h);
      ctx.fillStyle = '#ff9a2e';
      ctx.textAlign = 'center';
      ctx.font = 'bold 20px "Pixelify Sans", monospace';
      ctx.fillText('Headset lost: calibration paused', w / 2, h / 2 - 8);
      ctx.fillStyle = '#f2e8d5';
      ctx.font = '14px "Atkinson Hyperlegible", sans-serif';
      ctx.fillText('It resumes when the Muse streams again. Close (Esc) to stop.', w / 2, h / 2 + 18);
      return;
    }
    const frameDt = Math.max(0.008, Math.min(0.25, (now - (this.wiz.lastFrameNow || now)) / 1000));
    this.wiz.lastFrameNow = now;
    const elapsed = (now - this.wiz.t0) / 1000;
    const ctx = this.wiz.ctx;
    const w = this.wiz.canvas.width;
    const h = this.wiz.canvas.height;

    ctx.clearRect(0, 0, w, h);

    // Subtle crosshair & full-edge border guide
    ctx.strokeStyle = 'rgba(44, 49, 82, 0.45)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(w / 2, 0); ctx.lineTo(w / 2, h);
    ctx.moveTo(0, h / 2); ctx.lineTo(w, h / 2);
    ctx.stroke();
    ctx.strokeRect(w * 0.04, h * 0.04, w * 0.92, h * 0.92);

    const rel = b?.relOrientation || { yaw: 0, pitch: 0, roll: 0 };
    const bm = b?.blinkModel;
    const mm = b?.motionModel;
    const motionVal = b?.motion ?? 0;

    if (this.wiz.phase === 'motion') {
      // Hybrid Smooth-Pursuit + Boundary-Hold + Center-Reverse Trajectory (24.5s total)
      // Keyframes: [t_sec, x, y, label, isBoundaryHold]
      const keyframes = [
        { t: 0.0,  x: 0.50, y: 0.50, label: '1/7 · Look at CENTER & Hold Still (Locking 3D Gyro Zero)', hold: true },
        { t: 1.8,  x: 0.50, y: 0.50, label: '2/7 · Follow Dot Smoothly LEFT to Full Left Boundary ←', hold: false },
        { t: 3.3,  x: 0.04, y: 0.50, label: '2/7 · HOLD LEFT BOUNDARY ← (Do NOT Blink)', hold: true },
        { t: 4.5,  x: 0.04, y: 0.50, label: '2/7 · REVERSE Smoothly Back to CENTER →', hold: false },
        { t: 5.8,  x: 0.50, y: 0.50, label: 'Center Checkpoint — Re-Anchoring Zero', hold: true },
        { t: 6.2,  x: 0.50, y: 0.50, label: '3/7 · Follow Dot Smoothly RIGHT to Full Right Boundary →', hold: false },
        { t: 7.7,  x: 0.96, y: 0.50, label: '3/7 · HOLD RIGHT BOUNDARY → (Do NOT Blink)', hold: true },
        { t: 8.9,  x: 0.96, y: 0.50, label: '3/7 · REVERSE Smoothly Back to CENTER ←', hold: false },
        { t: 10.2, x: 0.50, y: 0.50, label: 'Center Checkpoint — Re-Anchoring Zero', hold: true },
        { t: 10.6, x: 0.50, y: 0.50, label: '4/7 · Follow Dot Smoothly UP to Full Top Boundary ↑', hold: false },
        { t: 12.0, x: 0.50, y: 0.04, label: '4/7 · HOLD TOP BOUNDARY ↑ (Do NOT Blink)', hold: true },
        { t: 13.3, x: 0.50, y: 0.04, label: '4/7 · REVERSE Smoothly Down to CENTER ↓', hold: false },
        { t: 14.5, x: 0.50, y: 0.50, label: 'Center Checkpoint — Re-Anchoring Zero', hold: true },
        { t: 14.9, x: 0.50, y: 0.50, label: '5/7 · Follow Dot Smoothly DOWN to Full Bottom Boundary ↓', hold: false },
        { t: 16.3, x: 0.50, y: 0.96, label: '5/7 · HOLD BOTTOM BOUNDARY ↓ (Do NOT Blink)', hold: true },
        { t: 17.6, x: 0.50, y: 0.96, label: '5/7 · REVERSE Smoothly Up to CENTER ↑', hold: false },
        { t: 18.8, x: 0.50, y: 0.50, label: '6/7 · Follow Full-Corner Figure-8 Sweep (Top-Left ↖)', hold: false },
        { t: 19.9, x: 0.06, y: 0.06, label: '6/7 · Sweep Left Boundary to Bottom-Left Corner ↙', hold: false },
        { t: 21.0, x: 0.06, y: 0.94, label: '6/7 · Cross Diagonally Through Center to Top-Right ↗', hold: false },
        { t: 22.3, x: 0.94, y: 0.06, label: '6/7 · Sweep Right Boundary to Bottom-Right Corner ↘', hold: false },
        { t: 23.4, x: 0.94, y: 0.94, label: '7/7 · Return Smoothly to CENTER & Hold', hold: false },
        { t: 24.4, x: 0.50, y: 0.50, label: '7/7 · Locking Final Center Reference…', hold: true },
        { t: 25.0, x: 0.50, y: 0.50, label: 'Complete!', hold: true },
      ];

      const totalDur = keyframes[keyframes.length - 1].t;
      if (elapsed >= totalDur) {
        this.advanceWizardPhase();
        return;
      }

      // Find active keyframe segment and cosine-smooth interpolate
      let segIdx = 0;
      for (let i = 0; i < keyframes.length - 1; i++) {
        if (elapsed >= keyframes[i].t && elapsed <= keyframes[i + 1].t) {
          segIdx = i;
          break;
        }
      }
      const k0 = keyframes[segIdx];
      const k1 = keyframes[segIdx + 1];
      const segSpan = Math.max(0.01, k1.t - k0.t);
      const rawU = Math.max(0, Math.min(1, (elapsed - k0.t) / segSpan));
      // Smooth cosine ease-in-out on moving segments so turnaround velocity is natural
      const smoothU = 0.5 - 0.5 * Math.cos(rawU * Math.PI);
      const dotX = k0.x + (k1.x - k0.x) * smoothU;
      const dotY = k0.y + (k1.y - k0.y) * smoothU;
      const isHold = Math.hypot(k1.x - k0.x, k1.y - k0.y) < 0.01;

      if (!this.wiz.taredAtCenter && elapsed >= 1.2) {
        this.wiz.taredAtCenter = true;
        if (src?.beginMotionCalibration) {
          src.beginMotionCalibration();
          this.addLog(`3D Center Tare locked (raw P:${b?.orientation?.pitch?.toFixed(1) ?? 0}° Y:${b?.orientation?.yaw?.toFixed(1) ?? 0}°). Recording hybrid pursuit + boundary holds + reverse sweeps...`);
        }
      }

      if (this.wiz.taredAtCenter && now - this.wiz.lastSampleAt >= 75) {
        this.wiz.lastSampleAt = now;
        this.wiz.dotSamplesTaken += 1;
        if (src?.addMotionCalibPoint) {
          src.addMotionCalibPoint(dotX, dotY);
        }
        // Record non-blink baseline during holds so eye classifier rejects stationary & edge hum
        if (isHold && this.wiz.dotSamplesTaken % 3 === 0 && src?.recordBaselineSample) {
          src.recordBaselineSample();
        }
      }

      // Draw boundary anchor guides at Left, Right, Top, Bottom, Center, and 4 Corners
      const anchors = [
        [0.50, 0.50, 'CENTER'],
        [0.04, 0.50, 'LEFT'],
        [0.96, 0.50, 'RIGHT'],
        [0.50, 0.04, 'TOP'],
        [0.50, 0.96, 'BOTTOM'],
        [0.06, 0.06, 'TL'],
        [0.94, 0.06, 'TR'],
        [0.06, 0.94, 'BL'],
        [0.94, 0.94, 'BR'],
      ];
      for (const [ax, ay, alabel] of anchors) {
        const px = ax * w;
        const py = ay * h;
        const near = Math.hypot(dotX - ax, dotY - ay) < 0.06;
        ctx.strokeStyle = near ? '#7ee08a' : 'rgba(163, 166, 196, 0.28)';
        ctx.lineWidth = near ? 2.5 : 1.2;
        ctx.beginPath();
        ctx.arc(px, py, 12, 0, Math.PI * 2);
        ctx.stroke();
        ctx.fillStyle = near ? '#7ee08a' : 'rgba(163, 166, 196, 0.45)';
        ctx.font = '10px "Pixelify Sans", monospace';
        ctx.textAlign = 'center';
        ctx.fillText(alabel, px, py + (ay < 0.15 ? 24 : -16));
      }

      // Draw trajectory direction arrow from current dot to next target
      if (!isHold) {
        ctx.strokeStyle = 'rgba(255, 207, 74, 0.40)';
        ctx.lineWidth = 2;
        ctx.setLineDash([6, 6]);
        ctx.beginPath();
        ctx.moveTo(dotX * w, dotY * h);
        ctx.lineTo(k1.x * w, k1.y * h);
        ctx.stroke();
        ctx.setLineDash([]);
      }

      const dx = dotX * w;
      const dy = dotY * h;
      const glow = 1 + 0.14 * Math.sin(now * 0.012);
      const grad = ctx.createRadialGradient(dx, dy, 4, dx, dy, 42 * glow);
      grad.addColorStop(0, 'rgba(255, 255, 255, 1)');
      grad.addColorStop(0.45, isHold ? 'rgba(126, 224, 138, 0.88)' : 'rgba(255, 230, 150, 0.88)');
      grad.addColorStop(1, 'rgba(255, 207, 74, 0)');
      ctx.fillStyle = grad;
      ctx.beginPath();
      ctx.arc(dx, dy, 42 * glow, 0, Math.PI * 2);
      ctx.fill();

      if (isHold) {
        ctx.strokeStyle = '#7ee08a';
        ctx.lineWidth = 4;
        ctx.beginPath();
        ctx.arc(dx, dy, 22, -Math.PI / 2, -Math.PI / 2 + rawU * Math.PI * 2);
        ctx.stroke();
      }

      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.arc(dx, dy, 12, 0, Math.PI * 2);
      ctx.fill();

      // Draw live head cursor preview
      if (aim) {
        const ux = aim.x * w;
        const uy = aim.y * h;
        ctx.strokeStyle = '#59d8c8';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(ux, uy, 11, 0, Math.PI * 2);
        ctx.stroke();
      }

      // Instruction banner at bottom-center so it never blocks top/center targets
      const bannerY = dotY > 0.62 ? h * 0.16 : h * 0.76;
      ctx.fillStyle = 'rgba(9, 10, 18, 0.84)';
      ctx.fillRect(w * 0.16, bannerY, w * 0.68, 64);
      ctx.strokeStyle = isHold ? '#7ee08a' : '#ffcf4a';
      ctx.lineWidth = 1.5;
      ctx.strokeRect(w * 0.16, bannerY, w * 0.68, 64);

      ctx.fillStyle = '#f4f0e4';
      ctx.font = '700 19px "Pixelify Sans", sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(k0.label, w / 2, bannerY + 26);

      const overallPct = Math.round((elapsed / totalDur) * 100);
      ctx.fillStyle = isHold ? '#7ee08a' : '#ffcf4a';
      ctx.font = '14px "Atkinson Hyperlegible", sans-serif';
      ctx.fillText(
        isHold
          ? `HOLD BOUNDARY STEADY (DO NOT BLINK)… ${overallPct}% total (${this.wiz.dotSamplesTaken} pts)`
          : `FOLLOW WHITE DOT SMOOTHLY WITH YOUR HEAD (DO NOT BLINK)… ${overallPct}% total (${this.wiz.dotSamplesTaken} pts)`,
        w / 2,
        bannerY + 50,
      );

      this.wiz.statsEl.textContent = `Hybrid Calib: ${overallPct}% (${this.wiz.dotSamplesTaken} pts) · Target: (${dotX.toFixed(2)}, ${dotY.toFixed(2)}) · Head ΔYaw: ${rel.yaw.toFixed(1)}° · ΔPitch: ${rel.pitch.toFixed(1)}° · Rest EEG: ${bm?.baselines ?? 0}`;
    } else if (this.wiz.phase === 'noise') {
      // 4.5-second "Move Head Around Without Blinking" noise-rejection stage
      const totalDur = 4.5;
      const frac = Math.min(1, elapsed / totalDur);
      if (now - this.wiz.lastNoiseSampleAt >= 120) {
        this.wiz.lastNoiseSampleAt = now;
        if (src?.recordBaselineSample) {
          src.recordBaselineSample();
        }
      }

      const cx = w / 2;
      const cy = h * 0.36;
      const orbitX = cx + Math.sin(elapsed * 2.6) * 160;
      const orbitY = cy + Math.cos(elapsed * 3.4) * 85;

      ctx.strokeStyle = 'rgba(89, 216, 200, 0.35)';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.ellipse(cx, cy, 160, 85, 0, 0, Math.PI * 2);
      ctx.stroke();

      ctx.fillStyle = '#59d8c8';
      ctx.beginPath();
      ctx.arc(orbitX, orbitY, 16, 0, Math.PI * 2);
      ctx.fill();

      ctx.fillStyle = '#f4f0e4';
      ctx.font = '700 24px "Pixelify Sans", sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('MOVE HEAD UP / DOWN / AROUND — DO NOT BLINK!', cx, 48);

      ctx.fillStyle = '#7ee08a';
      ctx.font = '16px "Atkinson Hyperlegible", sans-serif';
      ctx.fillText(`Recording Head-Motion Non-Blink Noise… ${Math.round(frac * 100)}% (${bm?.baselines ?? 0} negative samples)`, cx, 78);

      this.drawWizardEegStrip(ctx, b?.eegWave, 80, h - 155, w - 160, 130);
      this.wiz.statsEl.textContent = `Noise Rejection: ${Math.round(frac * 100)}% · Negative (Non-Blink) Samples: ${bm?.baselines ?? 0} · Head ΔYaw: ${rel.yaw.toFixed(1)}° ΔPitch: ${rel.pitch.toFixed(1)}°`;

      if (elapsed >= totalDur) {
        this.addLog(`Finished Head-Motion Noise Rejection (${bm?.baselines ?? 0} total non-blink EEG windows recorded).`);
        this.advanceWizardPhase();
      }
    } else if (this.wiz.phase === 'blink') {
      // 6 Flash cycles, each 2.8 seconds long (1.8s HOLD DO NOT BLINK -> 1.0s BRIGHT GOLD FLASH)
      const cycleDur = 2.8;
      const totalFlashes = 6;
      const idx = Math.floor(elapsed / cycleDur);
      const phaseT = (elapsed % cycleDur) / cycleDur; // 0..1

      if (idx >= totalFlashes) {
        this.advanceWizardPhase();
        return;
      }

      if (idx !== this.wiz.flashIdx) {
        this.wiz.flashIdx = idx;
        this.wiz.flashRestRecorded = false;
        this.wiz.flashTriggered = false;
      }

      // At 35% of cycle (during HOLD DO NOT BLINK), record another clean non-blink EEG sample
      if (!this.wiz.flashRestRecorded && phaseT > 0.30 && phaseT < 0.50) {
        this.wiz.flashRestRecorded = true;
        if (src?.recordBaselineSample) {
          src.recordBaselineSample();
        }
      }

      const isFlash = phaseT >= 0.64;
      if (isFlash && !this.wiz.flashTriggered) {
        this.wiz.flashTriggered = true;
        if (src?.armBlinkFlashWindow) {
          src.armBlinkFlashWindow(1.15);
        } else if (src?.recordBlinkSample) {
          src.recordBlinkSample();
        }
        this.addLog(`⚡ FLASH #${idx + 1}/${totalFlashes} — BLINK NOW! (Armed 1.15s 4-channel EEG capture)`);
      }

      const cx = w / 2;
      const cy = h * 0.38;
      if (isFlash) {
        const flashAlpha = Math.max(0, 1 - (phaseT - 0.64) / 0.36);
        ctx.fillStyle = `rgba(255, 215, 90, ${0.44 * flashAlpha})`;
        ctx.fillRect(0, 0, w, h);

        ctx.strokeStyle = '#ffcf4a';
        ctx.lineWidth = 5;
        ctx.beginPath();
        ctx.arc(cx, cy, 58, 0, Math.PI * 2);
        ctx.stroke();

        ctx.fillStyle = '#ffffff';
        ctx.font = '700 34px "Pixelify Sans", sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('⚡ BLINK NOW! ⚡', cx, cy + 12);
      } else {
        const ringFrac = 1 - (phaseT / 0.64);
        const radius = 36 + ringFrac * 110;
        ctx.strokeStyle = 'rgba(89, 216, 200, 0.85)';
        ctx.lineWidth = 4;
        ctx.beginPath();
        ctx.arc(cx, cy, radius, 0, Math.PI * 2);
        ctx.stroke();

        ctx.strokeStyle = 'rgba(244, 240, 228, 0.5)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.ellipse(cx, cy, 48, 26, 0, 0, Math.PI * 2);
        ctx.stroke();
        ctx.fillStyle = '#59d8c8';
        ctx.beginPath();
        ctx.arc(cx, cy, 12, 0, Math.PI * 2);
        ctx.fill();

        ctx.fillStyle = '#f4f0e4';
        ctx.font = '700 22px "Pixelify Sans", sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(`Cue ${idx + 1} of ${totalFlashes} — HOLD EYES OPEN (DO NOT BLINK)…`, cx, 42);
      }

      this.drawWizardEegStrip(ctx, b?.eegWave, 80, h - 155, w - 160, 130);

      this.wiz.statsEl.textContent = `Flash ${idx + 1}/${totalFlashes} · Blinks Learned: ${bm?.blinks ?? 0} · Non-Blink Rest/Noise: ${bm?.baselines ?? 0} · Surge: ${Math.round(bm?.effUv ?? 0)} µV · Thr: ${Math.round(bm?.thresholdUv ?? 175)} µV`;
    } else {
      // Sandbox phase: test full-edge head cursor + blink lightning
      const curBlinks = b?.stats?.blinks ?? 0;
      if (curBlinks > this.wiz.lastBlinkCount) {
        this.wiz.lastBlinkCount = curBlinks;
        this.wiz.sandboxBolts.push({
          x: (aim?.x ?? 0.5) * w,
          y: (aim?.y ?? 0.5) * h,
          born: now,
        });
      }

      const targets = [
        { x: 0.50, y: 0.06, lbl: 'TOP EDGE' },
        { x: 0.50, y: 0.94, lbl: 'BOTTOM EDGE' },
        { x: 0.06, y: 0.50, lbl: 'LEFT EDGE' },
        { x: 0.94, y: 0.50, lbl: 'RIGHT EDGE' },
        { x: 0.50, y: 0.50, lbl: 'CENTER' },
        { x: 0.12, y: 0.14, lbl: 'TOP LEFT' },
        { x: 0.88, y: 0.14, lbl: 'TOP RIGHT' },
        { x: 0.12, y: 0.86, lbl: 'BOT LEFT' },
        { x: 0.88, y: 0.86, lbl: 'BOT RIGHT' },
      ];
      for (const t of targets) {
        const tx = t.x * w, ty = t.y * h;
        const dist = Math.hypot((aim?.x ?? 0.5) - t.x, (aim?.y ?? 0.5) - t.y);
        ctx.strokeStyle = dist < 0.09 ? '#7ee08a' : 'rgba(163, 166, 196, 0.45)';
        ctx.lineWidth = dist < 0.09 ? 3 : 1.5;
        ctx.beginPath();
        ctx.arc(tx, ty, 28, 0, Math.PI * 2);
        ctx.stroke();
        ctx.fillStyle = 'rgba(244, 240, 228, 0.75)';
        ctx.font = '11px "Pixelify Sans", monospace';
        ctx.textAlign = 'center';
        ctx.fillText(t.lbl, tx, ty + 4);
      }

      this.wiz.sandboxBolts = this.wiz.sandboxBolts.filter((bolt) => now - bolt.born < 700);
      for (const bolt of this.wiz.sandboxBolts) {
        const age = (now - bolt.born) / 700;
        ctx.strokeStyle = `rgba(255, 230, 120, ${1 - age})`;
        ctx.lineWidth = 4;
        ctx.beginPath();
        ctx.arc(bolt.x, bolt.y, 18 + age * 55, 0, Math.PI * 2);
        ctx.stroke();
      }

      if (aim) {
        const ux = aim.x * w;
        const uy = aim.y * h;
        ctx.strokeStyle = '#7ee08a';
        ctx.fillStyle = '#7ee08a';
        ctx.lineWidth = 2.5;
        ctx.beginPath();
        ctx.arc(ux, uy, 14, 0, Math.PI * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(ux, uy, 4, 0, Math.PI * 2);
        ctx.fill();
      }

      // Brain drops non-finite head tilts (#221) and says why: show it instead of a silent canvas.
      if (b?.aimFault) {
        ctx.fillStyle = '#ffcf4a';
        ctx.font = '13px "Atkinson Hyperlegible", sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(`Head aim: ${b.aimFault}`, w / 2, h - 18);
      }

      this.wiz.statsEl.textContent = `Head Aim (screen): (${(aim?.x ?? 0.5).toFixed(2)}, ${(aim?.y ?? 0.5).toFixed(2)})${b?.aimFault ? ` [${b.aimFault}]` : ''} · Motion R²: ${Math.round((mm?.r2 ?? 0) * 100)}% (PitchEdge: ${(mm?.pitchEdgeDeg ?? 8).toFixed(1)}°) · Blink Acc: ${Math.round((bm?.accuracy ?? 0) * 100)}% (Thr ${Math.round(bm?.thresholdUv ?? 175)}µV)`;
    }
  }

  drawWizardEegStrip(ctx, eegWave, x0, y0, w, h) {
    ctx.fillStyle = 'rgba(9, 10, 18, 0.88)';
    ctx.strokeStyle = 'rgba(44, 49, 82, 0.85)';
    ctx.lineWidth = 1;
    ctx.fillRect(x0, y0, w, h);
    ctx.strokeRect(x0, y0, w, h);

    if (!eegWave) return;
    const laneH = h / CHANNELS.length;
    const scaleUv = 140;
    CHANNELS.forEach((c, idx) => {
      const samples = eegWave[c.key];
      const yMid = y0 + (idx + 0.5) * laneH;
      ctx.fillStyle = c.color;
      ctx.font = '11px "Pixelify Sans", monospace';
      ctx.textAlign = 'left';
      ctx.fillText(c.key, x0 + 6, yMid + 4);
      if (!samples || samples.length < 2) return;
      ctx.strokeStyle = c.color;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      for (let i = 0; i < samples.length; i++) {
        const px = x0 + 44 + (i / (180 - 1)) * (w - 52);
        const clamped = Math.max(-scaleUv, Math.min(scaleUv, samples[i]));
        const py = yMid - (clamped / scaleUv) * (laneH * 0.42);
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.stroke();
    });
  }

  calibrateCenter() {
    const src = this.getSource();
    const b = this.getBrain();
    if (src?.recenter) {
      const o = b?.orientation;
      src.recenter(true);
      if (b) {
        b.screenAim = { ...(b.screenAim || {}), x: 0.5, y: 0.5 };
        const a = b.screenToAim ? b.screenToAim(0.5, 0.5) : { x: 0.5, y: 0.5 };
        b.aim.x = a.x; b.aim.y = a.y;
      }
      this.ui.msg.textContent = '3D Head pose tared to screen center (0.50, 0.50) in Rust!';
      this.addLog(`Head cursor tared at raw=(P:${o ? o.pitch.toFixed(1) : '?'}° R:${o ? o.roll.toFixed(1) : '?'}° Y:${o ? o.yaw.toFixed(1) : '?'}°) -> center (0.5, 0.5)`);
    }
  }

  handleRecordBlink() {
    const src = this.getSource();
    const b = this.getBrain();
    if (!src?.recordBlinkSample) return;
    const immediate = src.recordBlinkSample();
    const bm = b?.blinkModel;
    if (immediate) {
      this.ui.blinkHint.textContent = 'Blink sample captured from recent EEG peak and Rust classifier retrained!';
      this.addLog(`Recorded blink sample (eff=${Math.round(bm?.effUv || 0)}uV, TP9=${Math.round(bm?.tp9Uv || 0)} AF7=${Math.round(bm?.af7Uv || 0)} AF8=${Math.round(bm?.af8Uv || 0)} TP10=${Math.round(bm?.tp10Uv || 0)})`);
    } else {
      this.ui.blinkHint.textContent = 'Armed for 2.5s — blink clearly right now to capture your blink wave!';
      this.addLog('Armed blink capture for 2.5s — blink now...');
    }
  }

  handleRecordBaseline() {
    const src = this.getSource();
    if (!src?.recordBaselineSample) return;
    if (src.recordBaselineSample()) {
      this.ui.blinkHint.textContent = 'Resting non-blink EEG sample added to Rust classifier.';
      this.addLog('Recorded resting baseline EEG sample.');
    }
  }

  handleResetBlinkModel() {
    const src = this.getSource();
    if (!src?.resetBlinkModel) return;
    src.resetBlinkModel();
    this.lastLoggedSampleCount = 0;
    this.ui.blinkHint.textContent = 'Rust blink classifier reset. Blink and click Record Blink to train.';
    this.addLog('Reset Rust blink classifier to default (thr=155uV).');
  }

  toggle(force) {
    if (!this.panel) return;
    const open = force !== undefined ? force : this.panel.hidden;
    this.panel.hidden = !open;
    this.btn.setAttribute('aria-pressed', String(open));
    if (open && this.doc) {
      const camP = this.doc.getElementById('camPanel');
      const camB = this.doc.getElementById('camBtn');
      if (camP && !camP.hidden) {
        camP.hidden = true;
        camB?.setAttribute('aria-pressed', 'false');
      }
      const modP = this.doc.getElementById('modelPanel');
      const modB = this.doc.getElementById('modelBtn');
      if (modP && !modP.hidden) {
        modP.hidden = true;
        modB?.setAttribute('aria-pressed', 'false');
      }
      const setP = this.doc.getElementById('settingsPanel');   // one side panel at a time (#186)
      if (setP && !setP.hidden) {
        setP.hidden = true;
        this.doc.getElementById('settingsBtn')?.setAttribute('aria-pressed', 'false');
      }
    }
  }

  async handleConnect() {
    if (this.connecting) return;
    const url = this.ui.url.value.trim() || 'ws://localhost:8765';
    this.connecting = true;
    this.ui.connectBtn.disabled = true;
    this.ui.msg.textContent = `Connecting to bridge at ${url}…`;
    try {
      await this.onConnectBridge(url);
      this.applyPrefsToSource(this.getSource());
      this.ui.msg.textContent = 'Bridge connected. Hold head still for ~1.5s to lock neutral pose.';
    } catch (err) {
      this.ui.msg.textContent = `${err.message}. Is ./run.sh muse2 running?`;
    } finally {
      this.connecting = false;
      this.ui.connectBtn.disabled = false;
    }
  }

  handleDisconnect() {
    this.onDisconnectBridge();
    this.ui.msg.textContent = 'Disconnected from Muse bridge.';
  }

  clearDisplays(reasonText) {
    this.ui.qual.textContent = '--';
    this.ui.calibTxt.textContent = 'Gyro Drift Lock: --';
    this.ui.calibBar.style.width = '0%';
    for (const c of CHANNELS) {
      const el = this.ui.contacts[c.key];
      el.dataset.state = 'OFF';
      el.querySelector('span').textContent = 'OFF';
    }
    for (const band of BANDS) {
      this.ui.bands[band.key].fill.style.width = '0%';
      this.ui.bands[band.key].val.textContent = '--';
    }
    this.ui.calm.fill.style.width = '0%';
    this.ui.calm.val.textContent = '--';
    this.ui.focus.fill.style.width = '0%';
    this.ui.focus.val.textContent = '--';
    this.ui.stress.fill.style.width = '0%';
    this.ui.stress.val.textContent = '--';
    this.ui.anger.textContent = '--';
    this.ui.blinks.textContent = '--';
    this.ui.motion.textContent = '--';
    this.ui.accelText.textContent = '--';
    this.ui.gyroText.textContent = '--';
    this.ui.pitch.textContent = '--°';
    this.ui.roll.textContent = '--°';
    this.ui.yaw.textContent = '--°';
    this.drawPad(null, false);
    this.drawScope(null, reasonText);
    this.drawImu(this.accelCtx, this.ui.accelScope, null, 1.5, 'Accel (g) — waiting');
    this.drawImu(this.gyroCtx, this.ui.gyroScope, null, 90, 'Gyro (°/s) — waiting');
  }

  update() {
    const src = this.getSource();
    const b = this.getBrain();
    // Every headset gets the saved aim settings, however it was connected (title screen, this
    // panel, auto-connect at boot), not only one connected from this panel (#186).
    if (src !== this.prefsSource) {
      this.prefsSource = src;
      if (src?.kind === 'bridge') this.applyPrefsToSource(src);
    }
    const isBridge = src?.kind === 'bridge';
    const wsConnected = isBridge && src.connected;
    const hasRealStream = wsConnected && src.streaming && src.lastFrameAt > 0 && (performance.now() - src.lastFrameAt < 3000);

    if (isBridge) this.applyPrefsToSource(src);

    if (this.btn) {
      this.btn.dataset.live = hasRealStream ? '1' : wsConnected ? 'wait' : '0';
      this.btn.textContent = hasRealStream ? 'EEG Live' : wsConnected ? 'EEG Waiting…' : 'EEG';
    }
    this.updateWizard(hasRealStream);
    if (!this.panel || this.panel.hidden) return;

    this.ui.disconnectBtn.disabled = !isBridge;
    if (this.ui.wizardBtn) this.ui.wizardBtn.disabled = !hasRealStream;
    if (this.ui.dotCalibBtn) this.ui.dotCalibBtn.disabled = !hasRealStream;
    if (this.ui.flashCalibBtn) this.ui.flashCalibBtn.disabled = !hasRealStream;
    this.ui.recenterBtn.disabled = !hasRealStream;
    this.ui.blinkRecordBtn.disabled = !hasRealStream;
    this.ui.blinkRestBtn.disabled = !hasRealStream;
    this.ui.blinkResetBtn.disabled = !hasRealStream;
    this.ui.connectBtn.textContent = wsConnected ? 'Reconnect' : 'Connect Muse 2';

    if (!isBridge) {
      this.ui.dot.dataset.ok = '0';
      this.ui.state.textContent = 'Headset Disconnected (Keyboard Mode)';
      this.clearDisplays('No EEG connected — click "Connect Muse 2" above');
      return;
    }

    if (!hasRealStream) {
      this.ui.dot.dataset.ok = wsConnected ? 'wait' : '0';
      this.ui.state.textContent = src.status || 'Waiting for Muse 2…';
      if (/not found|Waiting for headset/i.test(src.status || '')) {
        this.ui.msg.textContent = 'Tip: Do NOT use pairing mode (flashing lights) or pair in OS Bluetooth. Turn Muse 2 off, then press power once so its lights fade softly in and out.';
      }
      this.clearDisplays(src.status || 'Waiting for Muse 2 stream…');
      return;
    }

    this.ui.dot.dataset.ok = '1';
    this.ui.state.textContent = src.status;

    if (!b) return;

    const qPct = Math.round((b.quality ?? 0) * 100);
    this.ui.qual.textContent = `Signal: ${qPct}%`;

    // Calibration progress (gyro bias + neutral pose)
    const totalCalib = Math.min(1, ((b.imuCalib ?? 1) * 0.5) + ((b.neutralProgress ?? 1) * 0.5));
    const calibPct = Math.round(totalCalib * 100);
    this.ui.calibBar.style.width = `${calibPct}%`;
    if (totalCalib < 1) {
      this.ui.calibTxt.textContent = `Calibrating Stillness… ${calibPct}%`;
      this.ui.calibBar.style.background = 'var(--focus)';
    } else {
      const stillLocked = (b.motion || 0) < 0.08;
      this.ui.calibTxt.textContent = stillLocked ? 'Locked (Rust 3D Tare + ZUPT Active)' : 'Tracking Head Motion (Rust 3D)';
      this.ui.calibBar.style.background = 'var(--good)';
    }

    // Rust Blink Classifier telemetry
    const bm = b.blinkModel;
    if (!this.wasStreaming) {
      this.wasStreaming = true;
      this.addLog(`Streaming live Muse 2 telemetry (${src.status})`);
    }
    if (bm) {
      const probPct = Math.round((bm.prob || 0) * 100);
      this.ui.blinkProbBar.style.width = `${probPct}%`;
      this.ui.blinkProbVal.textContent = `${probPct}%`;
      this.ui.blinkPos.textContent = `${bm.blinks || 0}`;
      this.ui.blinkNeg.textContent = `${bm.baselines || 0}`;
      this.ui.blinkThr.textContent = `${Math.round(bm.thresholdUv || 155)} uV`;
      this.ui.blinkUv.textContent = `${Math.round(bm.effUv || 0)} uV (AF8:${Math.round(bm.af8Uv || 0)} TP:${Math.round(Math.max(bm.tp9Uv || 0, bm.tp10Uv || 0))})`;
      if (bm.capturing) {
        this.ui.blinkStatus.textContent = 'Blink Now (Armed)…';
      } else if (bm.trained) {
        this.ui.blinkStatus.textContent = `Trained (${Math.round((bm.accuracy || 0) * 100)}% acc)`;
      } else {
        this.ui.blinkStatus.textContent = 'Default (Ready)';
      }

      if ((bm.blinks || 0) > this.lastLoggedSampleCount) {
        this.lastLoggedSampleCount = bm.blinks;
        this.addLog(`Rust Blink Model trained (#${bm.blinks} blinks, ${bm.baselines} rest): thr=${Math.round(bm.thresholdUv)}uV acc=${Math.round((bm.accuracy || 0) * 100)}%`);
      }

      const curBlinks = b.stats?.blinks ?? 0;
      if (curBlinks > this.lastLoggedBlinkCount) {
        this.lastLoggedBlinkCount = curBlinks;
        this.addLog(`BLINK #${curBlinks} FIRED! eff=${Math.round(bm.effUv || 0)}uV (TP9:${Math.round(bm.tp9Uv || 0)} AF7:${Math.round(bm.af7Uv || 0)} AF8:${Math.round(bm.af8Uv || 0)} TP10:${Math.round(bm.tp10Uv || 0)}) corr=${(bm.corr || 0).toFixed(2)} prob=${probPct}%`);
      } else if ((bm.effUv || 0) >= 95 && performance.now() - this.lastCandidateLogAt > 900) {
        this.lastCandidateLogAt = performance.now();
        this.addLog(`EEG spike: eff=${Math.round(bm.effUv)}uV (TP9:${Math.round(bm.tp9Uv || 0)} AF7:${Math.round(bm.af7Uv || 0)} AF8:${Math.round(bm.af8Uv || 0)} TP10:${Math.round(bm.tp10Uv || 0)}) corr=${(bm.corr || 0).toFixed(2)} prob=${probPct}% (thr=${Math.round(bm.thresholdUv || 155)}uV)`);
      }
    }

    // Contact badges
    const contact = b.contact || {};
    for (const c of CHANNELS) {
      const el = this.ui.contacts[c.key];
      const st = contact[c.key] || 'POOR';
      el.dataset.state = st;
      el.querySelector('span').textContent = st;
    }

    // EEG Oscilloscope
    this.drawScope(b.eegWave, null);

    // Band powers
    const bands = b.bands || {};
    for (const band of BANDS) {
      const v = Math.max(0, Math.min(1, bands[band.key] || 0));
      const pct = `${Math.round(v * 100)}%`;
      this.ui.bands[band.key].fill.style.width = pct;
      this.ui.bands[band.key].val.textContent = pct;
    }

    // Mental state: calm, focus and stress (brain.stress: EEG agitation or face anger)
    const calmPct = `${Math.round((b.calm || 0) * 100)}%`;
    const focusPct = `${Math.round((b.focus || 0) * 100)}%`;
    const stressPct = `${Math.round((b.stress ?? 0) * 100)}%`;
    this.ui.calm.fill.style.width = calmPct;
    this.ui.calm.val.textContent = calmPct;
    this.ui.focus.fill.style.width = focusPct;
    this.ui.focus.val.textContent = focusPct;
    this.ui.stress.fill.style.width = stressPct;
    this.ui.stress.val.textContent = stressPct;

    this.ui.anger.textContent = `${Math.round((b.anger || 0) * 100)}%`;
    this.ui.blinks.textContent = `${b.stats?.blinks ?? 0}`;
    this.ui.motion.textContent = `${Math.round((b.motion || 0) * 100)}%`;

    // IMU Accelerometer & Gyroscope
    if (Array.isArray(b.accel) && b.accel.length === 3) {
      this.ui.accelText.textContent = b.accel.map((v) => (v >= 0 ? `+${v.toFixed(2)}` : v.toFixed(2))).join(' ');
    } else {
      this.ui.accelText.textContent = '--';
    }
    if (Array.isArray(b.gyro) && b.gyro.length === 3) {
      this.ui.gyroText.textContent = b.gyro.map((v) => (v >= 0 ? `+${v.toFixed(1)}` : v.toFixed(1))).join(' ');
    } else {
      this.ui.gyroText.textContent = '--';
    }
    this.drawImu(this.accelCtx, this.ui.accelScope, b.accelHistory, 1.5, 'Accel (g)');
    this.drawImu(this.gyroCtx, this.ui.gyroScope, b.gyroHistory, 90, 'Gyro (°/s)');

    const rel = b.relOrientation || b.orientation;
    if (rel && Number.isFinite(rel.pitch)) {
      const fmt = (v) => `${v >= 0 ? '+' : ''}${v.toFixed(1)}°`;
      this.ui.pitch.textContent = fmt(rel.pitch);
      this.ui.roll.textContent = fmt(rel.roll ?? 0);
      this.ui.yaw.textContent = fmt(rel.yaw);
    } else {
      this.ui.pitch.textContent = '--°';
      this.ui.roll.textContent = '--°';
      this.ui.yaw.textContent = '--°';
    }
    this.drawPad(screenAimOf(b), totalCalib >= 1);
  }

  drawPad(aim, calibrated) {
    const ctx = this.padCtx;
    const w = this.ui.pad.width;
    const h = this.ui.pad.height;
    ctx.clearRect(0, 0, w, h);

    // Crosshair & deadzone box
    const cx = w / 2, cy = h / 2;
    ctx.strokeStyle = 'rgba(44, 49, 82, 0.75)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(cx, 0); ctx.lineTo(cx, h);
    ctx.moveTo(0, cy); ctx.lineTo(w, cy);
    ctx.stroke();

    ctx.strokeRect(cx - 8, cy - 6, 16, 12);

    if (!aim) return;
    const x = Math.max(6, Math.min(w - 6, aim.x * w));
    const y = Math.max(6, Math.min(h - 6, aim.y * h));

    ctx.strokeStyle = calibrated ? '#7ee08a' : '#ffcf4a';
    ctx.fillStyle = calibrated ? '#7ee08a' : '#ffcf4a';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(x, y, 5, 0, Math.PI * 2);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(x, y, 2, 0, Math.PI * 2);
    ctx.fill();
  }

  drawScope(eegWave, emptyMessage) {
    const ctx = this.scopeCtx;
    const w = this.ui.scope.width;
    const h = this.ui.scope.height;
    ctx.clearRect(0, 0, w, h);

    ctx.strokeStyle = 'rgba(44, 49, 82, 0.55)';
    ctx.lineWidth = 1;
    const laneH = h / CHANNELS.length;
    for (let i = 0; i < CHANNELS.length; i++) {
      const yMid = (i + 0.5) * laneH;
      ctx.beginPath();
      ctx.moveTo(0, yMid);
      ctx.lineTo(w, yMid);
      ctx.stroke();
    }

    if (!eegWave || emptyMessage) {
      ctx.fillStyle = 'rgba(163, 166, 196, 0.75)';
      ctx.font = '12px "Atkinson Hyperlegible", system-ui, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(emptyMessage || 'No signal', w / 2, h / 2 + 4);
      return;
    }

    const scaleUv = 80;
    CHANNELS.forEach((c, idx) => {
      const samples = eegWave[c.key];
      if (!samples || samples.length < 2) return;
      const yMid = (idx + 0.5) * laneH;
      ctx.strokeStyle = c.color;
      ctx.lineWidth = 1.4;
      ctx.beginPath();
      const n = samples.length;
      for (let i = 0; i < n; i++) {
        const x = (i / (180 - 1)) * w;
        const clamped = Math.max(-scaleUv, Math.min(scaleUv, samples[i]));
        const y = yMid - (clamped / scaleUv) * (laneH * 0.42);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    });
  }

  drawImu(ctx, canvas, history, scale, label) {
    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    const yMid = h / 2;
    ctx.strokeStyle = 'rgba(44, 49, 82, 0.55)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, yMid);
    ctx.lineTo(w, yMid);
    ctx.stroke();

    ctx.fillStyle = 'rgba(163, 166, 196, 0.65)';
    ctx.font = '10px "Pixelify Sans", monospace';
    ctx.textAlign = 'left';
    ctx.fillText(label, 5, 11);

    if (!history || history.length < 2) return;

    const n = history.length;
    for (const axis of IMU_AXES) {
      ctx.strokeStyle = axis.color;
      ctx.lineWidth = 1.3;
      ctx.beginPath();
      for (let i = 0; i < n; i++) {
        const x = (i / (150 - 1)) * w;
        const v = history[i][axis.idx] || 0;
        const clamped = Math.max(-scale, Math.min(scale, v));
        const y = yMid - (clamped / scale) * (h * 0.42);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }
  }
}
