// Oog: high priest, then finance bro, then flight director. Reacts to your brain live.

import { drawAdvisorPortrait } from '../render/sprites.js';

const NAMES = {
  stone: 'Oog · High Priest of the Flame',
  farming: 'Oog · High Priest of the Harvest',
  kingdoms: 'Oog · Royal Scribe',
  industry: 'Oog III · Chief Goat Officer',
  space: 'Oog IX · Flight Director',
};

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
    this.textEl.textContent = '';
  }

  // key: dedupe/cooldown id. priority: higher interrupts lower.
  say(key, text, { mood = 'nervous', priority = 1, cooldown = 8 } = {}) {
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
