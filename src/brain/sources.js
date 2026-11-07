// Signal sources. Every source emits "frames" at roughly 10 Hz:
//   { bands: {delta, theta, alpha, beta, gamma},   relative band power, sums to ~1
//     blink: boolean,                              a deliberate blink happened since last frame
//     tilt: {x, y}|null,                           head tilt, -1..1 on each axis
//     motion: number,                              head movement magnitude, 0..1
//     quality: number }                            0..1 signal quality
//
// SimSource is the placeholder headset: it emits exactly the frames the real Muse will,
// driven by keyboard/touch, so the game is playable (and demo-safe) without hardware.
// BridgeSource reads real frames from bridge/muse_bridge.py over a WebSocket. Swapping one
// for the other needs no game changes.

import { clamp, gauss, rand, smooth } from '../sim/util.js';

const FRAME_DT = 0.1;

let RustSimSourceClass = null;

export function setRustSimSource(cls) {
  RustSimSourceClass = cls;
}

export class SimSource {
  constructor() {
    this.kind = 'sim';
    this.label = 'Emulated headset';
    this.connected = true;
    this.status = 'Emulated headset (keyboard)';
    if (RustSimSourceClass) {
      try {
        this.wasm = new RustSimSourceClass(Math.floor(Math.random() * 100000));
      } catch {
        this.wasm = null;
      }
    }
    // What the player is pressing right now. `panic` is the Stress demo key (Shift): full
    // agitation (beta and gamma up, alpha and calm down), so brain.stress rises.
    this.input = { calm: false, focus: false, panic: false };
    this.pendingBlink = false;
    this.aim = { x: 0.5, y: 0.6 };
    this.aimVel = 0;
    // Every simulated player gets a slightly different brain, so the personal
    // classifier has a real fingerprint to learn rather than a fixed rule.
    this.fp = {
      alphaGain: rand(0.55, 1.25),   // how strongly calm shows up as alpha
      betaGain: rand(0.55, 1.2),     // how strongly focus shows up as beta
      thetaFocus: rand(-0.4, 0.5),   // some people's theta rises with focus, some fall
      gammaArousal: rand(0.6, 1.0),  // how much muscle tension (gamma) agitation adds
    };
    // Latent mental state, smoothed so it behaves like a person rather than a switch.
    this.latent = { calm: 0.15, focus: 0.1, arousal: 0.1 };
    this.startle = 0;
    this.acc = 0;
  }

  connect() { return Promise.resolve(); }
  disconnect() {}

  blink() {
    this.pendingBlink = true;
    if (this.wasm) this.wasm.blink();
  }
  setAim(x, y) {
    this.aimVel += Math.hypot(x - this.aim.x, y - this.aim.y);
    this.aim.x = clamp(x); this.aim.y = clamp(y);
  }
  // Called by the game when something alarming happens (a market crash, a scrubbed launch).
  // A real player tenses up at these moments; the sim imitates that a little (more agitation).
  addStartle(amount) { this.startle = Math.min(1, this.startle + amount); }

  update(dt, emit) {
    const L = this.latent, I = this.input;
    if (this.wasm) {
      this.wasm.set_input(!!I.calm, !!I.focus, !!I.panic);
      this.acc += dt;
      while (this.acc >= FRAME_DT) {
        this.acc -= FRAME_DT;
        const out = this.wasm.step(FRAME_DT);
        if (out && out[10] > 0) {
          emit({
            bands: { delta: out[0], theta: out[1], alpha: out[2], beta: out[3], gamma: out[4] },
            blink: out[5] > 0.5,
            hr: out[6] > 0 ? out[6] : null,
            tilt: [out[7], out[8]],
            motion: out[9],
            quality: out[10],
          });
        } else {
          emit(this.makeFrame());
        }
      }
      return;
    }
    // The Stress key wins over the calm key, so stress can spike mid-calm (e.g. during the IPO).
    L.calm = smooth(L.calm, I.panic ? 0.02 : I.calm ? 0.95 : 0.15, I.calm && !I.panic ? 1.1 : 1.6, dt);
    L.focus = smooth(L.focus, I.focus ? 0.95 : 0.12, I.focus ? 1.3 : 1.8, dt);
    const arousalTarget = I.panic ? 1 : clamp(0.12 + 0.06 * L.focus - 0.12 * L.calm + this.startle * 0.8);
    L.arousal = smooth(L.arousal, arousalTarget, I.panic ? 0.9 : 0.35, dt);
    this.startle = Math.max(0, this.startle - dt * 0.12);

    this.acc += dt;
    while (this.acc >= FRAME_DT) {
      this.acc -= FRAME_DT;
      emit(this.makeFrame());
    }
  }

  makeFrame() {
    const L = this.latent, f = this.fp;
    const n = () => Math.exp(gauss() * 0.16); // multiplicative band noise
    let delta = 0.26 * n();
    let theta = (0.17 + 0.06 * f.thetaFocus * L.focus + 0.03 * L.calm) * n();
    // Agitation (the Stress key, startles) suppresses alpha and raises beta and, like a clenched
    // jaw on a real headset, gamma. Focus raises beta far more than gamma, so the brain's
    // agitation index tells the two apart.
    let alpha = (0.17 + 0.32 * f.alphaGain * L.calm - 0.07 * L.focus - 0.08 * L.arousal) * n();
    let beta = (0.15 + 0.26 * f.betaGain * L.focus + 0.12 * L.arousal - 0.05 * L.calm) * n();
    let gamma = (0.05 + 0.3 * f.gammaArousal * L.arousal + 0.02 * L.focus) * n();
    const blink = this.pendingBlink;
    if (blink) delta *= 3.2; // blinks smear huge low-frequency power across frontal channels
    alpha = Math.max(alpha, 0.02); beta = Math.max(beta, 0.02); theta = Math.max(theta, 0.02);
    const sum = delta + theta + alpha + beta + gamma;
    this.pendingBlink = false;
    const motion = clamp(this.aimVel * 6);
    this.aimVel = 0;
    const rel = { delta: delta / sum, theta: theta / sum, alpha: alpha / sum, beta: beta / sum, gamma: gamma / sum };
    const t0 = this.acc;
    const mkWave = (seed) => Array.from({ length: 6 }, (_, i) => {
      const ph = (t0 + i * 0.015 + seed) * Math.PI * 2;
      const base = 18 * rel.alpha * Math.sin(ph * 10) + 12 * rel.beta * Math.sin(ph * 20) + 22 * rel.delta * Math.sin(ph * 2.5) + gauss() * 4;
      return Math.round((base + (blink && (seed === 1 || seed === 2) && i >= 2 && i <= 4 ? 140 : 0)) * 10) / 10;
    });
    const pitch = Math.round((this.aim.y * 2 - 1) * 20 * 10) / 10;
    const yaw = Math.round((this.aim.x * 2 - 1) * 25 * 10) / 10;
    return {
      isSim: true,
      bands: rel,
      blink,
      tilt: { x: this.aim.x * 2 - 1, y: this.aim.y * 2 - 1 },
      motion,
      quality: 1,
      eeg: { TP9: mkWave(0), AF7: mkWave(1), AF8: mkWave(2), TP10: mkWave(3) },
      contact: { TP9: 'GOOD', AF7: 'GOOD', AF8: 'GOOD', TP10: 'GOOD' },
      orientation: { pitch, roll: 0, yaw },
      accel: [Math.round(Math.sin(pitch * Math.PI / 180) * 1000) / 1000, 0, Math.round(Math.cos(pitch * Math.PI / 180) * 1000) / 1000],
      gyro: [0, 0, 0],
    };
  }
}

// Reads frames from the Python bridge (BrainFlow → band powers, blinks, accelerometer and
// gyroscope). With the bridge's fused `orientation` the aim follows yaw (left/right) and pitch
// (up/down); without it (no gyro, older bridge) it falls back to accelerometer tilt.
export class BridgeSource {
  static AIM_RANGE_DEG = 25;    // degrees of yaw or pitch from neutral that reach the screen edge
  static AIM_SMOOTH = 15;       // 1/s, light smoothing on top of Brain's own aim smoothing
  static NEUTRAL_FRAMES = 10;   // the first ~1 s of frames sets the neutral pose
  static YAW_ZERO_TAU = 20;     // s; the yaw neutral fades to 0 as the bridge re-centres yaw itself
  static AIM_SIGN = { yaw: 1, pitch: 1 };   // flip one if a headset aims mirrored
  static DEADZONE_DEG = 0.85;   // degrees of micro-tremor deadband when interactive mode is active
  static RustImuTracker = null;
  static RustBlinkClassifier = null;

  static setWasmClasses(ImuCls, BlinkCls) {
    BridgeSource.RustImuTracker = ImuCls || null;
    BridgeSource.RustBlinkClassifier = BlinkCls || null;
  }

  constructor(url) {
    this.kind = 'bridge';
    this.label = 'Headset bridge';
    this.url = url;
    this.connected = false;
    this.streaming = false;
    this.status = 'Connecting…';
    this.queue = [];
    this.ws = null;
    this.lastFrameAt = 0;
    this.imuCalib = 0;
    this.sensitivity = 1.0;     // multiplier on AIM_RANGE_DEG (higher sensitivity = smaller angle needed)
    this.invertYaw = false;
    this.invertPitch = false;
    this.useDeadzone = false;   // enabled in live browser session so stationary tremor is locked
    this.rustImu = null;
    this.rustBlink = null;
    this.manualBlinkPending = false;
    this.ensureWasm();
    this.recenter();
  }

  ensureWasm() {
    if (!this.rustImu && BridgeSource.RustImuTracker) {
      try { this.rustImu = new BridgeSource.RustImuTracker(); } catch {}
    }
    if (!this.rustBlink && BridgeSource.RustBlinkClassifier) {
      try { this.rustBlink = new BridgeSource.RustBlinkClassifier(); } catch {}
    }
  }

  // Forget the neutral pose; the next ~1 s of frames becomes the new straight-ahead.
  recenter(recalibrateBridge = false) {
    this.ensureWasm();
    this.tiltZero = null;
    this.tiltSamples = [];
    this.aimZero = null;        // {yaw, pitch, roll} neutral from frame.orientation
    this.aimSamples = [];
    this.aimTilt = null;
    this.lastT = null;
    this.relAngles = { pitch: 0, roll: 0, yaw: 0 };
    if (this.rustImu) {
      try { this.rustImu.tare(); } catch {}
    }
    if (recalibrateBridge && this.ws && this.ws.readyState === 1) {
      try { this.ws.send(JSON.stringify({ type: 'recenter' })); } catch {}
    }
  }

  recordBlinkSample() {
    this.ensureWasm();
    if (this.rustBlink) {
      return this.rustBlink.record_blink_button();
    }
    return false;
  }

  armBlinkFlashWindow(seconds = 1.2) {
    this.ensureWasm();
    if (this.rustBlink && typeof this.rustBlink.arm_flash_window === 'function') {
      this.rustBlink.arm_flash_window(seconds);
      return true;
    }
    return false;
  }

  recordBaselineSample() {
    this.ensureWasm();
    if (this.rustBlink) {
      return this.rustBlink.record_baseline_button();
    }
    return false;
  }

  resetBlinkModel() {
    this.ensureWasm();
    if (this.rustBlink) {
      this.rustBlink.reset_model();
    }
  }

  beginMotionCalibration() {
    this.ensureWasm();
    if (this.ws && this.ws.readyState === 1) {
      try { this.ws.send(JSON.stringify({ type: 'recenter' })); } catch {}
    }
    if (this.rustImu && typeof this.rustImu.begin_motion_calibration === 'function') {
      this.rustImu.begin_motion_calibration();
      return true;
    }
    return false;
  }

  addMotionCalibPoint(x, y) {
    this.ensureWasm();
    if (this.rustImu && typeof this.rustImu.add_calib_point === 'function') {
      this.rustImu.add_calib_point(x, y);
      return this.rustImu.calib_sample_count();
    }
    return 0;
  }

  finishMotionCalibration() {
    this.ensureWasm();
    if (this.rustImu && typeof this.rustImu.finish_motion_calibration === 'function') {
      const ok = this.rustImu.finish_motion_calibration();
      return {
        ok,
        samples: this.rustImu.calib_sample_count(),
        r2: this.rustImu.calib_r2_score(),
        yawEdgeDeg: this.rustImu.learned_yaw_deg_for_edge(),
        pitchEdgeDeg: this.rustImu.learned_pitch_deg_for_edge(),
        yawSign: this.rustImu.learned_yaw_sign(),
        pitchSign: this.rustImu.learned_pitch_sign(),
      };
    }
    return { ok: false, samples: 0, r2: 0 };
  }

  cancelMotionCalibration() {
    if (this.rustImu && typeof this.rustImu.cancel_motion_calibration === 'function') {
      this.rustImu.cancel_motion_calibration();
    }
  }

  resetMotionModel() {
    if (this.rustImu && typeof this.rustImu.reset_motion_model === 'function') {
      this.rustImu.reset_motion_model();
    }
  }

  connect() {
    this._manualDisconnect = false;
    if (this._reconnectTimer) {
      clearTimeout(this._reconnectTimer);
      this._reconnectTimer = null;
    }
    this.recenter();
    return new Promise((resolve, reject) => {
      let settled = false;
      let ws;
      try { ws = new WebSocket(this.url); } catch (e) { reject(new Error(`Could not open ${this.url}`)); return; }
      this.ws = ws;
      const timer = setTimeout(() => {
        if (!settled) { settled = true; try { ws.close(); } catch {} reject(new Error(`No bridge answered at ${this.url}`)); }
      }, 4000);
      ws.onopen = () => {
        this.connected = true;
        this.streaming = false;
        this.useDeadzone = true;
        this.status = 'Connected to bridge, waiting for Muse 2…';
        if (!settled) { settled = true; clearTimeout(timer); resolve(); }
      };
      ws.onerror = () => {
        this.status = 'Bridge error';
        if (!settled) { settled = true; clearTimeout(timer); reject(new Error(`No bridge answered at ${this.url}`)); }
      };
      ws.onclose = () => {
        this.connected = false;
        this.streaming = false;
        this.status = this._manualDisconnect ? 'Bridge disconnected' : 'Bridge disconnected — reconnecting…';
        if (!this._manualDisconnect) {
          if (this._reconnectTimer) clearTimeout(this._reconnectTimer);
          this._reconnectTimer = setTimeout(() => {
            this._reconnectTimer = null;
            if (!this._manualDisconnect && !this.connected) {
              this.connect().catch(() => {});
            }
          }, 2000);
        }
      };
      ws.onmessage = (ev) => {
        try {
          const msg = JSON.parse(ev.data);
          if (msg.type === 'status') {
            this.status = msg.text;
            this.streaming = /^Streaming/i.test(msg.text);
            return;
          }
          this.streaming = true;
          this.queue.push(this.normalise(msg));
          this.lastFrameAt = performance.now();
        } catch { /* ignore malformed frames */ }
      };
    });
  }

  disconnect() {
    this._manualDisconnect = true;
    if (this._reconnectTimer) {
      clearTimeout(this._reconnectTimer);
      this._reconnectTimer = null;
    }
    try { this.ws?.close(); } catch {}
  }

  normalise(msg) {
    this.ensureWasm();
    const b = msg.bands || {};
    const sum = (b.delta || 0) + (b.theta || 0) + (b.alpha || 0) + (b.beta || 0) + (b.gamma || 0) || 1;
    if (typeof msg.imu_calib === 'number') this.imuCalib = clamp(msg.imu_calib);
    const motion = clamp(msg.motion ?? 0);
    const dt = typeof msg.t === 'number' && this.lastT != null ? clamp(msg.t - this.lastT, 0.01, 0.5) : FRAME_DT;

    let tilt = null;
    const o = msg.orientation;
    if (this.rustImu && Array.isArray(msg.accel) && msg.accel.length >= 3) {
      if (typeof msg.t === 'number') this.lastT = msg.t;
      const [ax, ay, az] = msg.accel;
      const [gx, gy, gz] = Array.isArray(msg.gyro) && msg.gyro.length >= 3 ? msg.gyro : [0, 0, 0];
      const hasYaw = !!(o && Number.isFinite(o.yaw));
      this.rustImu.set_config(
        this.sensitivity || 1.0,
        !!this.invertYaw,
        !!this.invertPitch,
        this.useDeadzone ? BridgeSource.DEADZONE_DEG : 0.0
      );
      this.rustImu.update(ax, ay, az, gx, gy, gz, hasYaw ? o.yaw : 0.0, hasYaw, dt);
      tilt = { x: this.rustImu.tilt_x(), y: this.rustImu.tilt_y() };
      this.relAngles = {
        pitch: this.rustImu.rel_pitch_deg(),
        roll: this.rustImu.rel_roll_deg(),
        yaw: this.rustImu.rel_yaw_deg(),
      };
      this.imuCalib = Math.max(this.imuCalib, this.rustImu.calib_progress());
    } else if (o && Number.isFinite(o.yaw) && Number.isFinite(o.pitch)) {
      tilt = this.orientationTilt(o, msg.t);
    } else if (Array.isArray(msg.accel)) {
      // Head tilt from the accelerometer's gravity vector, zeroed on the first second of data.
      const [ax, ay] = msg.accel;
      if (!this.tiltZero) {
        this.tiltSamples.push([ax, ay]);
        if (this.tiltSamples.length >= 10) {
          const mx = this.tiltSamples.reduce((s, v) => s + v[0], 0) / this.tiltSamples.length;
          const my = this.tiltSamples.reduce((s, v) => s + v[1], 0) / this.tiltSamples.length;
          this.tiltZero = [mx, my];
        }
      }
      const z = this.tiltZero || [ax, ay];
      tilt = { x: clamp((ay - z[1]) * 2.6, -1, 1), y: clamp((ax - z[0]) * 2.6, -1, 1) };
    }

    let rustBlinkFired = false;
    let blinkModel = null;
    if (this.rustBlink && msg.eeg && typeof msg.eeg === 'object') {
      const tp9 = new Float32Array(msg.eeg.TP9 || []);
      const af7 = new Float32Array(msg.eeg.AF7 || []);
      const af8 = new Float32Array(msg.eeg.AF8 || []);
      const tp10 = new Float32Array(msg.eeg.TP10 || []);
      rustBlinkFired = this.rustBlink.ingest_frame(tp9, af7, af8, tp10, motion, dt);
      blinkModel = {
        trained: this.rustBlink.is_trained(),
        capturing: this.rustBlink.is_capturing(),
        blinks: this.rustBlink.blink_samples(),
        baselines: this.rustBlink.baseline_samples(),
        accuracy: this.rustBlink.accuracy(),
        prob: this.rustBlink.last_probability(),
        thresholdUv: this.rustBlink.learned_threshold_uv(),
        learnedAf7Uv: this.rustBlink.learned_af7_uv(),
        learnedAf8Uv: this.rustBlink.learned_af8_uv(),
        tp9Uv: this.rustBlink.last_tp9_ptp(),
        af7Uv: this.rustBlink.last_af7_ptp(),
        af8Uv: this.rustBlink.last_af8_ptp(),
        tp10Uv: this.rustBlink.last_tp10_ptp(),
        effUv: this.rustBlink.last_eff_amp(),
        corr: this.rustBlink.last_corr(),
        rustFired: rustBlinkFired,
        bridgeFired: !!msg.blink,
      };
    }

    const blink = blinkModel
      ? (rustBlinkFired || this.manualBlinkPending)
      : (!!msg.blink || this.manualBlinkPending);
    this.manualBlinkPending = false;

    const neutralProgress = this.rustImu
      ? (this.rustImu.is_tared() ? 1 : this.rustImu.calib_progress())
      : (this.aimZero ? 1 : clamp(this.aimSamples.length / Math.max(1, BridgeSource.NEUTRAL_FRAMES)));

    const motionModel = this.rustImu && typeof this.rustImu.has_motion_model === 'function' ? {
      trained: this.rustImu.has_motion_model(),
      samples: this.rustImu.calib_sample_count(),
      r2: this.rustImu.calib_r2_score(),
      yawEdgeDeg: this.rustImu.learned_yaw_deg_for_edge(),
      pitchEdgeDeg: this.rustImu.learned_pitch_deg_for_edge(),
      yawSign: this.rustImu.learned_yaw_sign(),
      pitchSign: this.rustImu.learned_pitch_sign(),
    } : null;

    return {
      bands: { delta: (b.delta || 0) / sum, theta: (b.theta || 0) / sum, alpha: (b.alpha || 0) / sum, beta: (b.beta || 0) / sum, gamma: (b.gamma || 0) / sum },
      blink,
      tilt,
      motion,   // the bridge folds gyro rotation speed into this
      quality: clamp(msg.quality ?? 1),
      eeg: msg.eeg || null,
      contact: msg.contact || null,
      orientation: o && Number.isFinite(o.yaw) && Number.isFinite(o.pitch) ? { pitch: o.pitch, roll: o.roll ?? 0, yaw: o.yaw } : null,
      relOrientation: this.relAngles ? { ...this.relAngles } : null,
      imuCalib: this.imuCalib,
      neutralProgress,
      motionModel,
      blinkModel,
      accel: Array.isArray(msg.accel) ? msg.accel : null,
      gyro: Array.isArray(msg.gyro) ? msg.gyro : null,
    };
  }

  // Fused head orientation (degrees) → tilt: yaw aims left/right, pitch aims up/down. Positive yaw is
  // a turn to the right; pitch keeps the accelerometer path's sign (it rises with accel x), so both
  // paths aim the same way. ±AIM_RANGE_DEG from the neutral pose spans the screen.
  orientationTilt(o, t) {
    const S = BridgeSource;
    const wrap = (deg) => ((deg + 540) % 360) - 180;
    const dt = typeof t === 'number' && this.lastT != null ? clamp(t - this.lastT, 0, 1) : FRAME_DT;
    if (typeof t === 'number') this.lastT = t;
    if (!this.aimZero) {
      if (this.useDeadzone && this.imuCalib > 0 && this.imuCalib < 1) return { x: 0, y: 0 };
      this.aimSamples.push([o.yaw, o.pitch, o.roll ?? 0]);
      if (this.aimSamples.length < S.NEUTRAL_FRAMES) return { x: 0, y: 0 };
      const y0 = this.aimSamples[0][0], n = this.aimSamples.length;
      const rMean = this.aimSamples.reduce((s, v) => s + v[2], 0) / n;
      this.aimZero = {
        yaw: wrap(y0 + this.aimSamples.reduce((s, v) => s + wrap(v[0] - y0), 0) / n),
        pitch: this.aimSamples.reduce((s, v) => s + v[1], 0) / n,
      };
      this.aimRollZero = rMean;
      this.aimTilt = { x: 0, y: 0 };
    } else {
      // The bridge leaks yaw back to 0, so a fixed yaw offset would turn into a permanent bias.
      this.aimZero.yaw *= Math.exp(-dt / S.YAW_ZERO_TAU);
    }
    let dyaw = wrap(o.yaw - this.aimZero.yaw);
    let dpitch = o.pitch - this.aimZero.pitch;
    const droll = (o.roll ?? 0) - (this.aimRollZero ?? 0);
    this.relAngles = { pitch: dpitch, roll: droll, yaw: dyaw };

    if (this.useDeadzone) {
      const dz = S.DEADZONE_DEG;
      dyaw = Math.abs(dyaw) <= dz ? 0 : Math.sign(dyaw) * (Math.abs(dyaw) - dz);
      dpitch = Math.abs(dpitch) <= dz ? 0 : Math.sign(dpitch) * (Math.abs(dpitch) - dz);
    }

    const range = S.AIM_RANGE_DEG / Math.max(0.25, this.sensitivity || 1);
    const sx = S.AIM_SIGN.yaw * (this.invertYaw ? -1 : 1);
    const sy = S.AIM_SIGN.pitch * (this.invertPitch ? -1 : 1);
    const x = clamp(sx * dyaw / range, -1, 1);
    const y = clamp(sy * dpitch / range, -1, 1);
    this.aimTilt.x = smooth(this.aimTilt.x, x, S.AIM_SMOOTH, FRAME_DT);
    this.aimTilt.y = smooth(this.aimTilt.y, y, S.AIM_SMOOTH, FRAME_DT);
    return { x: this.aimTilt.x, y: this.aimTilt.y };
  }

  blink() { this.manualBlinkPending = true; }
  setAim() {}
  addStartle() {}

  update(dt, emit) {
    while (this.queue.length) emit(this.queue.shift());
    if (this.connected && this.lastFrameAt > 0 && performance.now() - this.lastFrameAt > 3000) {
      this.streaming = false;
      this.status = 'Waiting for headset data';
    }
  }
}
