// Brains for worlds the AI god runs. Rule: the AI's calm and focus are its OWN values,
// used only for the world it drives. Nothing here ever reads or writes the human player's
// headset inputs.
//
// ProxyBrain: a cheap stand-in with the surface the eras read (calm, focus, hr, aim,
// threshold, hrDelta, setCue, model.trust). Its calm follows the emulated headset's latent
// dynamics (SimSource), minus band-power noise and the personal classifier.
//
// SharedBrain: what a simulated world reads while a human watches it. The AI adds, the
// human is never blocked: calm and focus are the higher of the two, aim and pulse are the
// human's, and calibration cues stay on the AI's brain so the human's model is untouched.

import { smooth } from '../sim/util.js';

export class ProxyBrain {
  constructor() {
    this.kind = 'proxy';
    this.input = { calm: false, focus: false, panic: false };
    this.latent = { calm: 0.15, focus: 0.1 };
    this.calm = 0;
    this.focus = 0;
    this.hr = 70;
    this.restHr = 70;
    this.aim = { x: 0.5, y: 0.55 };
    this.cue = null;
    this.time = 0;
    this.model = { trust: () => 0, total: () => 0, trained: false, accuracy: 0, chance: 0 };
    this.stats = { blinks: 0, calmSeconds: 0, focusSeconds: 0, peakHr: 0, framesSeen: 0 };
  }

  onBlink() {}
  setCue(label) { this.cue = label; }
  threshold(kind, base = 0.55) { return base; }
  hrDelta() { return 0; }

  update(dt) {
    const L = this.latent, I = this.input;
    this.time += dt;
    L.calm = smooth(L.calm, I.calm ? 0.95 : I.panic ? 0.02 : 0.15, I.calm ? 1.1 : 1.6, dt);
    L.focus = smooth(L.focus, I.focus ? 0.95 : 0.12, I.focus ? 1.3 : 1.8, dt);
    this.calm = smooth(this.calm, L.calm, 4, dt);
    this.focus = smooth(this.focus, L.focus, 4, dt);
    if (this.calm > 0.6) this.stats.calmSeconds += dt;
  }
}

export class SharedBrain {
  constructor(ai, human) {
    this.ai = ai;
    this.human = human;
    this.kind = 'shared';
  }
  get calm() { return Math.max(this.ai.calm || 0, this.human?.calm || 0); }
  get focus() { return Math.max(this.ai.focus || 0, this.human?.focus || 0); }
  get aim() { return this.human?.aim || this.ai.aim; }
  get hr() { return this.human?.hr ?? this.ai.hr; }
  get model() { return this.ai.model; }
  get stats() { return this.ai.stats; }
  get cue() { return this.ai.cue; }
  setCue(label) { this.ai.setCue(label); }
  threshold(kind, base = 0.55) { return this.ai.threshold(kind, base); }
  hrDelta() { return this.human?.hrDelta ? this.human.hrDelta() : this.ai.hrDelta(); }
  onBlink() {}
  // Only the AI half is stepped here; the human's brain is stepped by the game loop.
  update(dt) { this.ai.update(dt); }
}
