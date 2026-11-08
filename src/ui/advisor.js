// Cluck, the advisor chicken: high priest, then finance bro (Chief Egg Officer), then flight
// director, which for a chicken is ambitious. Reacts to your brain live.

import { drawAdvisorPortrait } from '../render/sprites.js';

const NAMES = {
  stone: 'Cluck · High Priest of the Flame',
  farming: 'Cluck · High Priest of the Harvest',
  kingdoms: 'Cluck · Royal Scribe',
  industry: 'Cluck III · Chief Egg Officer',
  space: 'Cluck IX · Flight Director',
};

let rustEngine = null;

export function setRustAdvisorEngine(eng) {
  rustEngine = eng;
}

export class Advisor {
  constructor(root) {
    this.root = root;
    this.nameEl = root.querySelector('[data-advisor-name]');
    this.textEl = root.querySelector('[data-advisor-text]');
    this.portrait = root.querySelector('canvas').getContext('2d');
    this.era = 'stone';
    this.mood = 'nervous';
    this.current = null;     // {text, shown, hold, priority}
    this.queue = [];
    this.cooldowns = new Map();
    this.time = 0;
  }

  setEra(era) {
    this.era = era;
    this.nameEl.textContent = NAMES[era] || NAMES.stone;
    this.queue = [];
    this.current = null;
    this.cooldowns.clear();
    this.textEl.textContent = '';
    if (rustEngine && typeof rustEngine.advisor_set_era === 'function') {
      try { rustEngine.advisor_set_era(era); } catch {}
    }
  }

  // key: dedupe/cooldown id. priority: higher interrupts lower.
  say(key, text, { mood = 'nervous', priority = 1, cooldown = 8 } = {}) {
    if (rustEngine && typeof rustEngine.advisor_say === 'function') {
      try {
        return rustEngine.advisor_say(key, text, mood, priority, cooldown);
      } catch {}
    }
    const last = this.cooldowns.get(key);
    if (last != null && this.time - last < cooldown) return false;
    this.cooldowns.set(key, this.time);
    const line = { text, mood, priority, shown: 0, hold: Math.max(3.2, text.length / 16) };
    if (!this.current || priority > this.current.priority) {
      if (this.current && this.current.shown < this.current.text.length) this.queue.unshift(this.current);
      this.start(line);
    } else if (this.queue.length < 3) this.queue.push(line);
    return true;
  }

  start(line) {
    this.current = line;
    this.mood = line.mood;
    this.root.classList.remove('is-idle');
    this.root.dataset.mood = line.mood;
  }

  update(dt) {
    this.time += dt;
    if (rustEngine) {
      const domMood = this.root.dataset.mood || this.mood;
      drawAdvisorPortrait(this.portrait, this.era, domMood, this.time);
      return;
    }
    const c = this.current;
    if (c) {
      if (c.shown < c.text.length) {
        c.shown = Math.min(c.text.length, c.shown + dt * 48);
        this.textEl.textContent = c.text.slice(0, Math.floor(c.shown));
      } else {
        c.hold -= dt;
        if (c.hold <= 0) {
          this.current = this.queue.shift() || null;
          if (this.current) this.start(this.current);
          else this.root.classList.add('is-idle');
        }
      }
    }
    drawAdvisorPortrait(this.portrait, this.era, this.mood, this.time);
  }
}
