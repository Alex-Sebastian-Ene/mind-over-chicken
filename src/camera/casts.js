// camera (#33): gesture → cast debounce. Pure logic, no DOM, so Node can unit-test it.
//
// A gesture has to be held for `holdMs` (about 300 ms) before it casts, and it casts once per
// hold: to cast again, change or drop the gesture. Short tracking dropouts (under `graceMs`)
// don't break a hold. Each spell has its own cooldown; a gesture held through a cooldown casts
// as soon as the cooldown ends. Keyboard casts (`trigger`) share the same cooldowns.

import { SPELLS } from './spells.js';

let RustCastControllerClass = null;

export function setRustCastController(cls) {
  RustCastControllerClass = cls;
}

export class CastController {
  constructor({ spells = SPELLS, holdMs = 300, minScore = 0.55, aimScore = 0.45, graceMs = 150 } = {}) {
    if (RustCastControllerClass) {
      try {
        this.wasm = new RustCastControllerClass();
      } catch (e) {
        this.wasm = null;
      }
    }
    this.spells = new Map(spells.map((s) => [s.id, s]));
    this.byGesture = new Map(spells.map((s) => [s.gesture, s.id]));
    this.holdMs = holdMs;
    this.minScore = minScore;
    this.aimScore = aimScore;
    this.graceMs = graceMs;
    this.readyAt = new Map();   // spell id → time its cooldown ends
    this.reset();
  }

  reset() {
    if (this.wasm) this.wasm.reset();
    this.held = null;     // spell id of the gesture being held
    this.since = 0;       // when the hold started
    this.lastSeen = 0;    // last time the held gesture was actually seen
    this.latched = false; // already cast during this hold
  }

  spellFor(gesture) {
    if (this.wasm) return this.wasm.spell_for(gesture);
    return this.byGesture.get(gesture) ?? null;
  }

  // gesture: Gesture Recognizer category name ('None' when no hand). now: ms.
  // Returns { held, progress 0..1, cast: spell id | null, aiming }.
  update(gesture, score, now) {
    if (this.wasm) {
      const res = this.wasm.update(gesture, score, now);
      return { held: res[0], progress: res[1], cast: res[2], aiming: res[3] };
    }
    const id = score >= this.minScore ? this.spellFor(gesture) : null;
    const aiming = gesture === 'Pointing_Up' && score >= this.aimScore;
    if (id) {
      if (id !== this.held) { this.held = id; this.since = now; this.latched = false; }
      this.lastSeen = now;
    } else if (this.held && now - this.lastSeen > this.graceMs) {
      this.reset();
    }
    const heldFor = this.held ? now - this.since : 0;
    let cast = null;
    if (this.held && !this.latched && heldFor >= this.holdMs && this.ready(this.held, now)) {
      cast = this.held;
      this.latched = true;
      this.startCooldown(cast, now);
    }
    return { held: this.held, progress: this.held ? Math.min(1, heldFor / this.holdMs) : 0, cast, aiming };
  }

  // Keyboard / tap fallback. Returns true when the spell was cast.
  trigger(id, now) {
    if (this.wasm) return this.wasm.trigger(id, now);
    if (!this.spells.has(id) || !this.ready(id, now)) return false;
    this.startCooldown(id, now);
    return true;
  }

  ready(id, now) {
    if (this.wasm) return this.wasm.ready(id, now);
    return now >= (this.readyAt.get(id) ?? -Infinity);
  }
  startCooldown(id, now) { this.readyAt.set(id, now + (this.spells.get(id)?.cooldown || 0)); }
  cooldownLeft(id, now) {
    if (this.wasm) return this.wasm.cooldown_left(id, now);
    return Math.max(0, (this.readyAt.get(id) ?? -Infinity) - now);
  }
  cooldownFrac(id, now) {
    if (this.wasm) return this.wasm.cooldown_frac(id, now);
    const cd = this.spells.get(id)?.cooldown || 0;
    return cd ? Math.min(1, this.cooldownLeft(id, now) / cd) : 0;
  }
}
