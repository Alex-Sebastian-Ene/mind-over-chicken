// camera (#33, #150, #159): gesture → cast debounce. Pure logic, no DOM, so Node can unit-test it.
// Mirrors crates/engine/src/camera/casts.rs; in the browser the wasm RustCastController runs.
//
// Hold moves (Calm): held for `holdMs` (about 300 ms), they cast once per hold: to cast again,
// change or drop the gesture. Short tracking dropouts (under `graceMs`) don't break a hold. Each
// spell has its own cooldown; a gesture held through a cooldown casts as soon as it ends.
// Charge moves (Fireball, #150): holding the charge gesture (a fist) for `holdMs` arms the move
// and never casts by itself; its release gesture (an open hand) within `releaseMs` of the last
// charged frame casts it once. That open hand is spent: it must leave Open_Palm before it can
// hold-cast Calm, so one fist-to-open never casts both. Keyboard casts (`trigger`) share the
// same cooldowns.

import { SPELLS } from './spells.js';

let RustCastControllerClass = null;

export function setRustCastController(cls) {
  RustCastControllerClass = cls;
}

export class CastController {
  constructor({ spells = SPELLS, holdMs = 300, minScore = 0.55, aimScore = 0.45, graceMs = 150, releaseMs = 600 } = {}) {
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
    this.releaseMs = releaseMs;
    this.readyAt = new Map();   // spell id → time its cooldown ends
    this.reset();
  }

  reset() {
    if (this.wasm) this.wasm.reset();
    this.dropHold();
    this.armed = null;    // { id, seen }: a charged move waiting for its release gesture
  }

  dropHold() {
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
  // Returns { held, progress 0..1, cast: spell id | null, aiming, armed: spell id | null }.
  update(gesture, score, now) {
    if (this.wasm) {
      const res = this.wasm.update(gesture, score, now);
      return { held: res[0] ?? null, progress: res[1], cast: res[2] ?? null, aiming: res[3], armed: res[4] ?? null };
    }
    const confident = score >= this.minScore;
    const id = confident ? this.spellFor(gesture) : null;
    const aiming = gesture === 'Pointing_Up' && score >= this.aimScore;
    let cast = null;

    // A charged move fires on its release gesture, before that gesture can start a hold.
    if (this.armed) {
      const release = this.spells.get(this.armed.id)?.release;
      if (now - this.armed.seen > this.releaseMs) this.armed = null;
      else if (confident && release === gesture) {
        const armedId = this.armed.id;
        this.armed = null;
        if (this.ready(armedId, now)) { cast = armedId; this.startCooldown(armedId, now); }
        // The opening hand is spent on the release (no Calm from the same motion).
        this.held = id; this.since = now; this.lastSeen = now; this.latched = true;
        return { held: this.held, progress: 0, cast, aiming, armed: null };
      }
    }

    if (id) {
      if (id !== this.held) { this.held = id; this.since = now; this.latched = false; }
      this.lastSeen = now;
    } else if (this.held && now - this.lastSeen > this.graceMs) {
      this.dropHold();
    }
    const heldFor = this.held ? now - this.since : 0;
    if (this.held) {
      if (this.spells.get(this.held)?.release) {
        // Charging never casts; once charged it stays armed for releaseMs after the fist was last seen.
        if (heldFor >= this.holdMs) this.armed = { id: this.held, seen: this.lastSeen };
      } else if (!this.latched && heldFor >= this.holdMs && this.ready(this.held, now)) {
        cast = this.held;
        this.latched = true;
        this.startCooldown(cast, now);
      }
    }
    return { held: this.held, progress: this.held ? Math.min(1, heldFor / this.holdMs) : 0, cast, aiming, armed: this.armed?.id ?? null };
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
