// Brain = source frames → smoothed features → calm / focus / stress the game can use.
// It blends a generic rule (alpha/beta ratios) with the personal model as that model
// earns trust, and records labelled windows whenever the game sets a cue.
//
// Stress is the one number the game reads for "how wound up is the god", 0..1. It is computed
// here, once, so the eras, HUD, calibration and galaxy all agree on it. It is neutral at rest
// (about REST_STRESS) and rises on agitation, not on a missing calm:
//
//   stress = max(agitation, face anger while the camera sees a face, REST_STRESS * (1 - calm))
//
// Agitation is EEG, measured against the player's own resting level (agitationIndex below): the
// high-frequency power a tense player adds (gamma, which on a Muse is mostly muscle tension: a
// clenched jaw, a frown) relative to beta, plus a fifth of the fast/slow ratio beta/(alpha+theta).
// Focus raises beta far more than gamma, so it lowers this index: focus does not read as stress.
// The resting level is the calibration window's (#54, lockAgitationBaseline), or a slow running
// average until that window has run (and for brains without one, like the AI's).

import { clamp, sigmoid, smooth, median } from '../sim/util.js';
import { PersonalModel, featuresFromBands } from './model.js';

export const REST_STRESS = 0.15;   // stress of a player at rest: the small (1 - calm) term
export const STRESSED = 0.5;       // above this the god counts as stressed (eras' thresholds)
const AG_MARGIN = 0.12;            // index above rest that still reads as rest (window noise)
const AG_SPAN = 0.4;               // index above rest + margin that reads as full agitation
const AG_WARMUP = 30;              // windows (~3 s) whose median starts the running average
const AG_HISTORY = 300;            // feature windows kept for the baseline (~30 s at 10 Hz)
const AG_RUNNING_TAU = 60;         // s, the running-average baseline's time constant

// ProxyBrain and SharedBrain use the same function. All inputs 0..1; `anger` is the face's anger
// while the camera is on and sees a face, else 0.
export function stressFrom(calm, anger = 0, agitation = 0) {
  return clamp(Math.max(clamp(agitation || 0), clamp(anger || 0), REST_STRESS * (1 - clamp(calm || 0))));
}

// Agitation index of one feature window f (model.js FEATURE_NAMES):
// ln(gamma/beta) + 0.2 * ln(beta/(alpha+theta)) = ln γ - 0.8 ln β - 0.2 ln(α+θ).
export function agitationIndex(f) {
  return f[4] - f[3] + 0.2 * f[6];
}

// The face's anger, if the camera is on and sees a face (G.mood from src/camera/camera.js).
export function faceAnger(mood) {
  return mood?.active && mood.face ? clamp(mood.anger || 0) : 0;
}

const WINDOW = 6;            // frames averaged per feature window (~0.6 s)
const TRAIN_EVERY = 2.5;     // seconds between retrains

let RustBrainClass = null;

export function setRustBrain(cls) {
  RustBrainClass = cls;
}

export class Brain {
  constructor(source) {
    this.source = source;
    if (RustBrainClass) {
      try {
        this.wasm = new RustBrainClass();
      } catch (e) {
        this.wasm = null;
      }
    }
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
    this.agitation = 0;       // 0..1, EEG agitation above the player's resting level
    this.agitationRaw = 0;
    this.agHistory = [];      // recent agitation indices, for the calibration baseline
    this.agBaseline = null;   // resting index from the calibration window (#54)
    this.agRunning = null;    // slow running average of the index: the fallback baseline
    this.stress = REST_STRESS;  // stressFrom(calm, anger, agitation), updated every frame
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
  setCue(label) {
    this.cue = label;
    if (this.wasm) {
      const id = label === 'rest' ? 0 : label === 'calm' ? 1 : label === 'focus' ? 2 : -1;
      this.wasm.set_cue(id);
    }
  }

  resetTelemetry() {
    this.window = [];
    this.feat = null;
    this.calm = 0;
    this.focus = 0;
    this.calmRaw = 0;
    this.focusRaw = 0;
    this.anger = 0;
    this.agitation = 0;
    this.agitationRaw = 0;
    this.stress = REST_STRESS;
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
    this.agitation = smooth(this.agitation, this.agitationRaw, 3, dt);
    this.anger = faceAnger(mood);
    this.stress = stressFrom(this.calm, this.anger, this.agitation);
    if (this.calm > 0.6) this.stats.calmSeconds += dt;
    if (this.focus > 0.6) this.stats.focusSeconds += dt;

    this.trainTimer += dt;
    if (this.trainTimer > TRAIN_EVERY) {
      this.trainTimer = 0;
      if (this.model.maybeTrain() && this.onTrained) this.onTrained(this.model);
    }

    if (this.wasm) {
      try {
        this.wasm.update(dt);
        this.calm = this.wasm.calm();
        this.focus = this.wasm.focus();
        this.stress = this.wasm.stress();
        this.anger = this.wasm.anger();
        this.stats.blinks = this.wasm.blinks();
      } catch (err) {
        console.warn('RustBrain update failed, falling back to JS brain simulation:', err);
        this.wasm = null;
      }
    }
  }

  ingest(frame) {
    this.stats.framesSeen++;
    if (this.wasm && frame.bands) {
      const b = frame.bands;
      try {
        this.wasm.ingest(
          b.delta || 0, b.theta || 0, b.alpha || 0, b.beta || 0, b.gamma || 0,
          !!frame.blink, frame.hr || 0,
          frame.tilt ? frame.tilt[0] : 0, frame.tilt ? frame.tilt[1] : 0,
          frame.motion || 0, frame.quality || 1
        );
      } catch (err) {
        console.warn('RustBrain ingest failed, disabling wasm brain:', err);
        this.wasm = null;
      }
    }
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
      if (frame.isSim) {
        this.aim.x = clamp(0.5 + frame.tilt.x * 0.5);
        this.aim.y = clamp(0.5 + frame.tilt.y * 0.5);
      } else {
        // A headset aims at the screen. On a world bigger than the screen (#129) the game maps
        // that through its camera (screenToAim); the smoothing stays in screen space.
        const s = (this.screenAim ||= { x: this.aim.x, y: this.aim.y });
        s.x = smooth(s.x, clamp(0.5 + frame.tilt.x * 0.5), 10, 0.1);
        s.y = smooth(s.y, clamp(0.5 + frame.tilt.y * 0.5), 10, 0.1);
        const a = this.screenToAim ? this.screenToAim(s.x, s.y) : s;
        this.aim.x = a.x; this.aim.y = a.y;
      }
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
    this.updateAgitation(agitationIndex(f));

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

  // One feature window's agitation index → agitationRaw, against the player's resting level.
  updateAgitation(ag) {
    this.agHistory.push(ag);
    if (this.agHistory.length > AG_HISTORY) this.agHistory.shift();
    // Slow running average (one window ≈ 0.1 s, time constant AG_RUNNING_TAU): the fallback
    // baseline until the calibration window sets one.
    if (this.agHistory.length <= AG_WARMUP) this.agRunning = median(this.agHistory);
    else this.agRunning += (ag - this.agRunning) * (1 - Math.exp(-0.1 / AG_RUNNING_TAU));
    const base = this.agBaseline ?? this.agRunning;
    this.agitationRaw = clamp((ag - base - AG_MARGIN) / AG_SPAN);
  }

  // calibration (#54): the window that seeds the player's civilisation also sets their resting
  // agitation level, from its last `seconds` of feature windows.
  lockAgitationBaseline(seconds = 10) {
    const recent = this.agHistory.slice(-Math.max(10, Math.round(seconds * 10)));
    if (recent.length >= 10) this.agBaseline = median(recent);
  }
}
