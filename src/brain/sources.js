// Signal sources. Every source emits "frames" at roughly 10 Hz:
//   { bands: {delta, theta, alpha, beta, gamma},   relative band power, sums to ~1
//     blink: boolean,                              a deliberate blink happened since last frame
//     hr: number|null,                             heart rate in bpm
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

export class SimSource {
  constructor() {
    this.kind = 'sim';
    this.label = 'Emulated headset';
    this.connected = true;
    this.status = 'Emulated headset (keyboard)';
    // What the player is pressing right now.
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
      gammaArousal: rand(0.3, 1.0),
      restHr: Math.round(rand(62, 76)),
    };
    // Latent mental state, smoothed so it behaves like a person rather than a switch.
    this.latent = { calm: 0.15, focus: 0.1, arousal: 0.1 };
    this.hr = this.fp.restHr;
    this.startle = 0;
    this.acc = 0;
  }

  connect() { return Promise.resolve(); }
  disconnect() {}

  blink() { this.pendingBlink = true; }
  setAim(x, y) {
    this.aimVel += Math.hypot(x - this.aim.x, y - this.aim.y);
    this.aim.x = clamp(x); this.aim.y = clamp(y);
  }
  // Called by the game when something alarming happens (a market crash, a scrubbed launch).
  // A real player's pulse jumps at these moments; the sim imitates that a little.
  addStartle(amount) { this.startle = Math.min(1, this.startle + amount); }

  update(dt, emit) {
    const L = this.latent, I = this.input;
    L.calm = smooth(L.calm, I.calm ? 0.95 : (I.panic ? 0.02 : 0.15), I.calm ? 1.1 : 1.6, dt);
    L.focus = smooth(L.focus, I.focus ? 0.95 : 0.12, I.focus ? 1.3 : 1.8, dt);
    const arousalTarget = I.panic ? 1 : clamp(0.12 + 0.06 * L.focus - 0.12 * L.calm + this.startle * 0.8);
    L.arousal = smooth(L.arousal, arousalTarget, I.panic ? 0.9 : 0.35, dt);
    this.startle = Math.max(0, this.startle - dt * 0.12);

    const hrTarget = this.fp.restHr + 42 * L.arousal - 7 * L.calm + 2 * L.focus;
    this.hr = smooth(this.hr, hrTarget, 0.45, dt) + gauss() * 0.15;

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
    let alpha = (0.17 + 0.32 * f.alphaGain * L.calm - 0.07 * L.focus - 0.06 * L.arousal) * n();
    let beta = (0.15 + 0.26 * f.betaGain * L.focus + 0.1 * L.arousal - 0.05 * L.calm) * n();
    let gamma = (0.05 + 0.06 * f.gammaArousal * L.arousal + 0.02 * L.focus) * n();
    const blink = this.pendingBlink;
    if (blink) delta *= 3.2; // blinks smear huge low-frequency power across frontal channels
    alpha = Math.max(alpha, 0.02); beta = Math.max(beta, 0.02); theta = Math.max(theta, 0.02);
    const sum = delta + theta + alpha + beta + gamma;
    this.pendingBlink = false;
    const motion = clamp(this.aimVel * 6);
    this.aimVel = 0;
    return {
      bands: { delta: delta / sum, theta: theta / sum, alpha: alpha / sum, beta: beta / sum, gamma: gamma / sum },
      blink,
      hr: Math.round(this.hr * 10) / 10,
      tilt: { x: this.aim.x * 2 - 1, y: this.aim.y * 2 - 1 },
      motion,
      quality: 1,
    };
  }
}

// Reads frames from the Python bridge (BrainFlow → band powers, blinks, PPG heart rate, accelerometer
// and gyroscope). With the bridge's fused `orientation` the aim follows yaw (left/right) and pitch
// (up/down); without it (no gyro, older bridge) it falls back to accelerometer tilt.
export class BridgeSource {
  static AIM_RANGE_DEG = 25;    // degrees of yaw or pitch from neutral that reach the screen edge
  static AIM_SMOOTH = 15;       // 1/s, light smoothing on top of Brain's own aim smoothing
  static NEUTRAL_FRAMES = 10;   // the first ~1 s of frames sets the neutral pose
  static YAW_ZERO_TAU = 20;     // s; the yaw neutral fades to 0 as the bridge re-centres yaw itself
  static AIM_SIGN = { yaw: 1, pitch: 1 };   // flip one if a headset aims mirrored

  constructor(url) {
    this.kind = 'bridge';
    this.label = 'Headset bridge';
    this.url = url;
    this.connected = false;
    this.status = 'Connecting…';
    this.queue = [];
    this.ws = null;
    this.lastFrameAt = 0;
    this.recenter();
  }

  // Forget the neutral pose; the next ~1 s of frames becomes the new straight-ahead.
  recenter() {
    this.tiltZero = null;
    this.tiltSamples = [];
    this.aimZero = null;        // {yaw, pitch} neutral from frame.orientation
    this.aimSamples = [];
    this.aimTilt = null;
    this.lastT = null;
  }

  connect() {
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
        this.status = 'Headset streaming';
        if (!settled) { settled = true; clearTimeout(timer); resolve(); }
      };
      ws.onerror = () => {
        this.status = 'Bridge error';
        if (!settled) { settled = true; clearTimeout(timer); reject(new Error(`No bridge answered at ${this.url}`)); }
      };
      ws.onclose = () => { this.connected = false; this.status = 'Bridge disconnected'; };
      ws.onmessage = (ev) => {
        try {
          const msg = JSON.parse(ev.data);
          if (msg.type === 'status') { this.status = msg.text; return; }
          this.queue.push(this.normalise(msg));
          this.lastFrameAt = performance.now();
        } catch { /* ignore malformed frames */ }
      };
    });
  }

  disconnect() { try { this.ws?.close(); } catch {} }

  normalise(msg) {
    const b = msg.bands || {};
    const sum = (b.delta || 0) + (b.theta || 0) + (b.alpha || 0) + (b.beta || 0) + (b.gamma || 0) || 1;
    let tilt = null;
    const o = msg.orientation;
    if (o && Number.isFinite(o.yaw) && Number.isFinite(o.pitch)) {
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
    return {
      bands: { delta: (b.delta || 0) / sum, theta: (b.theta || 0) / sum, alpha: (b.alpha || 0) / sum, beta: (b.beta || 0) / sum, gamma: (b.gamma || 0) / sum },
      blink: !!msg.blink,
      hr: typeof msg.hr === 'number' && msg.hr > 30 && msg.hr < 220 ? msg.hr : null,
      tilt,
      motion: clamp(msg.motion ?? 0),   // the bridge folds gyro rotation speed into this
      quality: clamp(msg.quality ?? 1),
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
      this.aimSamples.push([o.yaw, o.pitch]);
      if (this.aimSamples.length < S.NEUTRAL_FRAMES) return { x: 0, y: 0 };
      const y0 = this.aimSamples[0][0], n = this.aimSamples.length;
      this.aimZero = {
        yaw: wrap(y0 + this.aimSamples.reduce((s, v) => s + wrap(v[0] - y0), 0) / n),
        pitch: this.aimSamples.reduce((s, v) => s + v[1], 0) / n,
      };
      this.aimTilt = { x: 0, y: 0 };
    } else {
      // The bridge leaks yaw back to 0, so a fixed yaw offset would turn into a permanent bias.
      this.aimZero.yaw *= Math.exp(-dt / S.YAW_ZERO_TAU);
    }
    const x = clamp(S.AIM_SIGN.yaw * wrap(o.yaw - this.aimZero.yaw) / S.AIM_RANGE_DEG, -1, 1);
    const y = clamp(S.AIM_SIGN.pitch * (o.pitch - this.aimZero.pitch) / S.AIM_RANGE_DEG, -1, 1);
    this.aimTilt.x = smooth(this.aimTilt.x, x, S.AIM_SMOOTH, FRAME_DT);
    this.aimTilt.y = smooth(this.aimTilt.y, y, S.AIM_SMOOTH, FRAME_DT);
    return { x: this.aimTilt.x, y: this.aimTilt.y };
  }

  blink() {}
  setAim() {}
  addStartle() {}

  update(dt, emit) {
    while (this.queue.length) emit(this.queue.shift());
    if (this.connected && performance.now() - this.lastFrameAt > 3000) this.status = 'Waiting for headset data';
  }
}
