// EEG & Biosensing Side Panel: connect/disconnect Muse 2 bridge at any time,
// inspect electrode contact (TP9, AF7, AF8, TP10), 4-channel oscilloscope,
// 5-band frequency spectrum, mental indices, PPG cardiac pulse waveform + BPM,
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
    </div>
    <details class="eeg-adv">
      <summary>Bridge WebSocket URL</summary>
      <input type="text" data-eeg-url value="ws://localhost:8765" spellcheck="false" autocomplete="off" aria-label="Bridge WebSocket URL">
    </details>
  </section>

  <section class="eeg-sec">
    <div class="eeg-sec-head">
      <h3>Head Cursor &amp; Motion Calibration</h3>
      <button type="button" class="btn btn-primary btn-small" data-eeg-recenter title="Look at the center of the screen and click (or press C) to zero Pitch/Yaw and recalibrate gyro drift" disabled>Calibrate Center <kbd>C</kbd></button>
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
    <div class="eeg-sec-head">
      <h3>PPG Optical Cardiac Monitor</h3>
      <strong class="eeg-bpm-readout" data-eeg-hr>❤️ -- BPM</strong>
    </div>
    <canvas class="eeg-ppg-scope" width="356" height="50" aria-label="PPG optical cardiac pulse wave"></canvas>
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
    </div>
    <div class="eeg-vitals">
      <div><span>❤️ Resting Δ</span><b data-eeg-hr-delta>--</b></div>
      <div><span>⚡ Blinks</span><b data-eeg-blinks>--</b></div>
      <div><span>🎯 Motion</span><b data-eeg-motion>--</b></div>
    </div>
  </section>
`;

export class EegPanel {
  constructor(doc, { getSource, getBrain, onConnectBridge, onDisconnectBridge }) {
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
      msg: $('[data-eeg-msg]'),
      recenterBtn: $('[data-eeg-recenter]'),
      calibTxt: $('[data-imu-calib-txt]'),
      calibBar: $('[data-imu-calib-bar]'),
      pad: $('.eeg-pad'),
      sensInput: $('[data-imu-sens]'),
      sensVal: $('[data-imu-sens-val]'),
      invYaw: $('[data-imu-inv-yaw]'),
      invPitch: $('[data-imu-inv-pitch]'),
      scope: $('.eeg-scope'),
      ppgScope: $('.eeg-ppg-scope'),
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
      hr: $('[data-eeg-hr]'),
      hrDelta: $('[data-eeg-hr-delta]'),
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
    this.ppgCtx = this.ui.ppgScope.getContext('2d');
    this.accelCtx = this.ui.accelScope.getContext('2d');
    this.gyroCtx = this.ui.gyroScope.getContext('2d');
    this.connecting = false;
    this.prefs = { sensitivity: 1.0, invertYaw: false, invertPitch: false };

    this.btn.addEventListener('click', () => this.toggle());
    $('[data-eeg-close]').addEventListener('click', () => this.toggle(false));

    this.ui.connectBtn.addEventListener('click', () => this.handleConnect());
    this.ui.disconnectBtn.addEventListener('click', () => this.handleDisconnect());
    this.ui.recenterBtn.addEventListener('click', () => this.calibrateCenter());

    this.ui.sensInput.addEventListener('input', () => {
      const v = parseFloat(this.ui.sensInput.value) || 1.0;
      this.prefs.sensitivity = v;
      this.ui.sensVal.textContent = `${v.toFixed(1)}×`;
      const src = this.getSource();
      if (src) src.sensitivity = v;
    });
    this.ui.invYaw.addEventListener('change', () => {
      this.prefs.invertYaw = this.ui.invYaw.checked;
      const src = this.getSource();
      if (src) src.invertYaw = this.prefs.invertYaw;
    });
    this.ui.invPitch.addEventListener('change', () => {
      this.prefs.invertPitch = this.ui.invPitch.checked;
      const src = this.getSource();
      if (src) src.invertPitch = this.prefs.invertPitch;
    });
  }

  applyPrefsToSource(src) {
    if (!src) return;
    src.sensitivity = this.prefs.sensitivity;
    src.invertYaw = this.prefs.invertYaw;
    src.invertPitch = this.prefs.invertPitch;
  }

  calibrateCenter() {
    const src = this.getSource();
    const b = this.getBrain();
    if (src?.recenter) {
      src.recenter(true);
      if (b) { b.aim.x = 0.5; b.aim.y = 0.5; }
      this.ui.msg.textContent = 'Hold your head still looking at screen center for ~1.5s to lock gyro bias & neutral pose…';
    }
  }

  toggle(force) {
    if (!this.panel) return;
    const open = force !== undefined ? force : this.panel.hidden;
    this.panel.hidden = !open;
    this.btn.setAttribute('aria-pressed', String(open));
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
    this.ui.hr.textContent = '❤️ -- BPM';
    this.ui.hrDelta.textContent = '--';
    this.ui.blinks.textContent = '--';
    this.ui.motion.textContent = '--';
    this.ui.accelText.textContent = '--';
    this.ui.gyroText.textContent = '--';
    this.ui.pitch.textContent = '--°';
    this.ui.roll.textContent = '--°';
    this.ui.yaw.textContent = '--°';
    this.drawPad(null, false);
    this.drawScope(null, reasonText);
    this.drawPpg(null, 'No PPG optical signal');
    this.drawImu(this.accelCtx, this.ui.accelScope, null, 1.5, 'Accel (g) — waiting');
    this.drawImu(this.gyroCtx, this.ui.gyroScope, null, 90, 'Gyro (°/s) — waiting');
  }

  update() {
    const src = this.getSource();
    const b = this.getBrain();
    const isBridge = src?.kind === 'bridge';
    const wsConnected = isBridge && src.connected;
    const hasRealStream = wsConnected && src.streaming && src.lastFrameAt > 0 && (performance.now() - src.lastFrameAt < 3000);

    if (isBridge) this.applyPrefsToSource(src);

    if (this.btn) {
      this.btn.dataset.live = hasRealStream ? '1' : wsConnected ? 'wait' : '0';
      this.btn.textContent = hasRealStream ? 'EEG Live' : wsConnected ? 'EEG Waiting…' : 'EEG';
    }
    if (!this.panel || this.panel.hidden) return;

    this.ui.disconnectBtn.disabled = !isBridge;
    this.ui.recenterBtn.disabled = !hasRealStream;
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
        this.ui.msg.textContent = 'Tip: Do NOT use pairing mode (flashing lights) or pair in OS Bluetooth. Turn Muse 2 off, then press power once so lights softly pulse.';
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
      this.ui.calibTxt.textContent = stillLocked ? 'Locked (Zero-Drift ZUPT Active)' : 'Tracking Head Motion';
      this.ui.calibBar.style.background = 'var(--good)';
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

    // PPG Cardiac + Mental state
    const calmPct = `${Math.round((b.calm || 0) * 100)}%`;
    const focusPct = `${Math.round((b.focus || 0) * 100)}%`;
    this.ui.calm.fill.style.width = calmPct;
    this.ui.calm.val.textContent = calmPct;
    this.ui.focus.fill.style.width = focusPct;
    this.ui.focus.val.textContent = focusPct;

    this.ui.hr.textContent = b.hr ? `❤️ ${Math.round(b.hr)} BPM` : '❤️ calibrating…';
    const d = b.hrDelta ? b.hrDelta() : 0;
    this.ui.hrDelta.textContent = b.hr && b.restHr ? `${d >= 0 ? '+' : ''}${Math.round(d)} bpm` : '--';
    this.ui.blinks.textContent = `${b.stats?.blinks ?? 0}`;
    this.ui.motion.textContent = `${Math.round((b.motion || 0) * 100)}%`;
    this.drawPpg(b.ppgWave, null);

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
    this.drawPad(b.aim, totalCalib >= 1);
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

  drawPpg(ppgWave, emptyMessage) {
    const ctx = this.ppgCtx;
    const w = this.ui.ppgScope.width;
    const h = this.ui.ppgScope.height;
    ctx.clearRect(0, 0, w, h);

    const yMid = h / 2;
    ctx.strokeStyle = 'rgba(44, 49, 82, 0.55)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, yMid);
    ctx.lineTo(w, yMid);
    ctx.stroke();

    if (!ppgWave || ppgWave.length < 2 || emptyMessage) {
      ctx.fillStyle = 'rgba(163, 166, 196, 0.7)';
      ctx.font = '11px "Atkinson Hyperlegible", system-ui, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(emptyMessage || 'Waiting for optical PPG pulse…', w / 2, yMid + 4);
      return;
    }

    ctx.strokeStyle = '#ff5a72';
    ctx.lineWidth = 1.6;
    ctx.beginPath();
    const n = ppgWave.length;
    for (let i = 0; i < n; i++) {
      const x = (i / (180 - 1)) * w;
      const clamped = Math.max(-1.2, Math.min(1.2, ppgWave[i]));
      const y = yMid - (clamped / 1.2) * (h * 0.42);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
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
