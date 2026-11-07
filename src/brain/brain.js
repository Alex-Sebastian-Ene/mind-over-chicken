// Brain = source frames → smoothed features → calm / focus / stress the game can use.
// It blends a generic rule (alpha/beta ratios) with the personal model as that model
// earns trust, and records labelled windows whenever the game sets a cue.
//
// Stress is the one number the game reads for "how wound up is the god":
// stress = max(1 - calm, face anger while the camera sees a face), 0..1. It is computed here,
// once, so the eras, HUD, calibration and galaxy all agree on it.

import { clamp, sigmoid, smooth, median } from '../sim/util.js';
import { PersonalModel, featuresFromBands } from './model.js';

// stress = max(1 - calm, anger), both 0..1. `anger` is the face's anger score while the camera
// is on and sees a face, else 0. ProxyBrain and SharedBrain use the same function.
export function stressFrom(calm, anger = 0) {
  return clamp(Math.max(1 - clamp(calm || 0), clamp(anger || 0)));
}

// The face's anger, if the camera is on and sees a face (G.mood from src/camera/camera.js).
export function faceAnger(mood) {
  return mood?.active && mood.face ? clamp(mood.anger || 0) : 0;
}

const WINDOW = 6;            // frames averaged per feature window (~0.6 s)
const TRAIN_EVERY = 2.5;     // seconds between retrains

export class Brain {
  constructor(source) {
    this.source = source;
    this.model = new PersonalModel();
    this.window = [];
    this.feat = null;
    this.calm = 0;
    this.focus = 0;
    this.calmRaw = 0;
    this.focusRaw = 0;
    this.genericCalm = 0;
    this.genericFocus = 0;
    this.personal = null;
    this.anger = 0;           // face anger counted into stress (0 without a camera)
    this.stress = 1;          // max(1 - calm, anger), updated every frame
    this.aim = { x: 0.5, y: 0.55 };
    this.motion = 0;
    this.quality = 1;
    this.bands = { delta: 0.2, theta: 0.2, alpha: 0.2, beta: 0.2, gamma: 0.2 };
    this.contact = null;
    this.orientation = null;
    this.relOrientation = null;
    this.imuCalib = 1;
    this.neutralProgress = 1;
    this.blinkModel = null;
    this.accel = null;
    this.gyro = null;
    this.eegWave = { TP9: [], AF7: [], AF8: [], TP10: [] };
    this.accelHistory = [];
    this.gyroHistory = [];
    this.cue = null;          // 'rest' | 'calm' | 'focus' | null
    this.blinkListeners = [];
    this.lastBlinkAt = -10;
    this.time = 0;
    this.trainTimer = 0;
    this.restRef = { ab: [], bat: [] };
    this.stats = { blinks: 0, calmSeconds: 0, focusSeconds: 0, framesSeen: 0 };
    this.onTrained = null;
  }

  onBlink(fn) { this.blinkListeners.push(fn); }
  setCue(label) { this.cue = label; }

  resetTelemetry() {
    this.window = [];
    this.feat = null;
    this.calm = 0;
    this.focus = 0;
    this.calmRaw = 0;
    this.focusRaw = 0;
    this.anger = 0;
    this.stress = 1;
    this.motion = 0;
    this.quality = 0;
    this.bands = { delta: 0, theta: 0, alpha: 0, beta: 0, gamma: 0 };
    this.contact = null;
    this.orientation = null;
    this.relOrientation = null;
    this.imuCalib = 0;
    this.neutralProgress = 0;
    this.blinkModel = null;
    this.accel = null;
    this.gyro = null;
    this.eegWave = { TP9: [], AF7: [], AF8: [], TP10: [] };
    this.accelHistory = [];
    this.gyroHistory = [];
  }

  // mood: G.mood from the camera (optional). Only its anger is read, for stress.
  update(dt, mood = null) {
    this.time += dt;
    this.source.update(dt, (frame) => this.ingest(frame));

    // Smooth the outputs the game reads, so a single noisy window can't flip a challenge.
    this.calm = smooth(this.calm, this.calmRaw, 4, dt);
    this.focus = smooth(this.focus, this.focusRaw, 4, dt);
    this.anger = faceAnger(mood);
    this.stress = stressFrom(this.calm, this.anger);
    if (this.calm > 0.6) this.stats.calmSeconds += dt;
    if (this.focus > 0.6) this.stats.focusSeconds += dt;

    this.trainTimer += dt;
    if (this.trainTimer > TRAIN_EVERY) {
      this.trainTimer = 0;
      if (this.model.maybeTrain() && this.onTrained) this.onTrained(this.model);
    }
  }

  ingest(frame) {
    this.stats.framesSeen++;
    this.quality = frame.quality;
    if (frame.bands) this.bands = { ...frame.bands };
    if (frame.contact) this.contact = frame.contact;
    if (frame.orientation !== undefined) this.orientation = frame.orientation;
    if (frame.relOrientation !== undefined) this.relOrientation = frame.relOrientation;
    if (typeof frame.imuCalib === 'number') this.imuCalib = frame.imuCalib;
    if (typeof frame.neutralProgress === 'number') this.neutralProgress = frame.neutralProgress;
    if (frame.motionModel !== undefined) this.motionModel = frame.motionModel;
    if (frame.blinkModel !== undefined) this.blinkModel = frame.blinkModel;
    if (frame.accel !== undefined) {
      this.accel = frame.accel;
      if (Array.isArray(frame.accel) && frame.accel.length === 3) {
        this.accelHistory.push(frame.accel);
        if (this.accelHistory.length > 150) this.accelHistory.shift();
      }
    }
    if (frame.gyro !== undefined) {
      this.gyro = frame.gyro;
      if (Array.isArray(frame.gyro) && frame.gyro.length === 3) {
        this.gyroHistory.push(frame.gyro);
        if (this.gyroHistory.length > 150) this.gyroHistory.shift();
      }
    }
    if (frame.eeg && typeof frame.eeg === 'object') {
      for (const [ch, vals] of Object.entries(frame.eeg)) {
        if (!Array.isArray(vals)) continue;
        if (!this.eegWave[ch]) this.eegWave[ch] = [];
        this.eegWave[ch].push(...vals);
        if (this.eegWave[ch].length > 180) this.eegWave[ch].splice(0, this.eegWave[ch].length - 180);
      }
    }
    if (frame.tilt) {
      this.aim.x = smooth(this.aim.x, clamp(0.5 + frame.tilt.x * 0.5), 10, 0.1);
      this.aim.y = smooth(this.aim.y, clamp(0.5 + frame.tilt.y * 0.5), 10, 0.1);
    }
    this.motion = frame.motion;

    if (frame.blink) {
      if (this.time - this.lastBlinkAt > 0.45) {
        this.lastBlinkAt = this.time;
        this.stats.blinks++;
        this.blinkListeners.forEach((fn) => fn(this.aim));
      }
      return; // blink frames are artifacts: keep them out of the classifier
    }

    this.window.push(featuresFromBands(frame.bands));
    if (this.window.length > WINDOW) this.window.shift();
    if (this.window.length < 3) return;
    const D = this.window[0].length;
    const f = new Array(D).fill(0);
    for (const w of this.window) for (let j = 0; j < D; j++) f[j] += w[j] / this.window.length;
    this.feat = f;

    if (this.cue) this.model.add(this.cue, f);
    if (this.cue === 'rest') {
      this.restRef.ab.push(f[5]); this.restRef.bat.push(f[6]);
      if (this.restRef.ab.length > 300) { this.restRef.ab.shift(); this.restRef.bat.shift(); }
    }

    // Generic rule: alpha/beta up = calm, beta/(alpha+theta) up = focus, relative to this player's rest.
    const baseAB = this.restRef.ab.length > 20 ? median(this.restRef.ab) : 0.1;
    const baseBAT = this.restRef.bat.length > 20 ? median(this.restRef.bat) : -0.8;
    this.genericCalm = sigmoid((f[5] - baseAB - 0.45) * 4.5);
    this.genericFocus = sigmoid((f[6] - baseBAT - 0.4) * 4.5);

    const p = this.model.predict(f);
    this.personal = p;
    const tc = this.model.trust('calm'), tf = this.model.trust('focus');
    this.calmRaw = p && p.calm != null ? this.genericCalm * (1 - tc) + p.calm * tc : this.genericCalm;
    this.focusRaw = p && p.focus != null ? this.genericFocus * (1 - tf) + p.focus * tf : this.genericFocus;
  }

  // The better the model knows the player, the higher the bar. Players never see this.
  threshold(kind, base = 0.55) {
    return clamp(base + 0.14 * this.model.trust(kind), 0, 0.85);
  }
}
