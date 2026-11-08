// The setup screen between New Game and the home planet: your mind makes four worlds.
//
// Alex: "the 4 should only read emotions rather than by doing it via sliders and only one at a
// time kind of like a fifa pack where it shows you one card at a time and afterward it moves to
// the left". Each world is a card. Its back reads the player's mind for a few seconds (calm,
// focus, stress and blinks); then it flips to show the world that reading made, and slides left
// into the row while the next card is read. After the fourth, Begin starts the home campaign
// and the four worlds join the galaxy with their temperaments (they replace the three worlds the
// first ten seconds of play used to make, so nothing is read twice).
//
// The maths is pure and lives in src/ai/personality.js. This file builds the cards and runs the
// reading; with no root element (Node tests) it runs the same state machine without a DOM.

import {
  BIOME_COLORS, BIOME_LABELS, MOOD_LABEL, READ_SECONDS, SETUP_COUNT, SETUP_NAMES,
  MoodReading, personalityLabel, planetFromMood, toPlanetOptions, traitWeights, actionPreview,
} from '../ai/personality.js';
import { reducedMotion } from './motion.js';

export { SETUP_COUNT, planetFromMood, toPlanetOptions, traitWeights, actionPreview, personalityLabel };

const REVEAL_HOLD = 1.5;   // seconds a revealed card stays centre stage before it moves left
const pct = (v) => Math.round(Math.max(0, Math.min(1, v || 0)) * 100);
// Settings › Reduce motion (body[data-reduced-motion]) or the system preference: no flips or glides.
const reduceMotion = () => {
  if (typeof document === 'undefined') return true;
  try { return reducedMotion(document) || matchMedia('(prefers-reduced-motion: reduce)').matches; } catch { return false; }
};

function el(tag, className, text) {
  const e = document.createElement(tag);
  if (className) e.className = className;
  if (text != null) e.textContent = text;
  return e;
}

// FLIP: measure the row, change it, then glide every card from where it was to where it is.
function glide(container, mutate) {
  if (!container || reduceMotion() || typeof container.animate !== 'function') { mutate(); return; }
  const before = new Map([...container.children].map((c) => [c, c.getBoundingClientRect()]));
  mutate();
  for (const c of container.children) {
    const a = before.get(c);
    if (!a) {
      c.animate([{ opacity: 0, transform: 'translateX(48px) scale(0.92)' }, { opacity: 1, transform: 'none' }], { duration: 380, easing: 'cubic-bezier(.2,.8,.2,1)' });
      continue;
    }
    const b = c.getBoundingClientRect();
    const dx = a.left - b.left, dy = a.top - b.top, s = a.width / (b.width || 1);
    if (Math.abs(dx) < 1 && Math.abs(dy) < 1 && Math.abs(s - 1) < 0.01) continue;
    c.animate([
      { transformOrigin: 'top left', transform: `translate(${dx}px, ${dy}px) scale(${s})` },
      { transformOrigin: 'top left', transform: 'none' },
    ], { duration: 460, easing: 'cubic-bezier(.2,.8,.2,1)' });
  }
}

export class PlanetSetup {
  // getBrain: the brain to read (the headset, or the emulated one on the keyboard).
  // isEmulated: true when that brain is the keyboard emulator, to show its keys.
  constructor(root, { onBegin = null, onCancel = null, getBrain = () => null, isEmulated = () => false } = {}) {
    this.root = root || null;
    this.onBegin = onBegin;
    this.onCancel = onCancel;
    this.getBrain = getBrain;
    this.isEmulated = isEmulated;
    this.planets = [];
    this.phase = 'closed';
    this.reading = null;
    this.hold = 0;
    this.current = null;     // the centre card's element
    if (!this.root) return;
    this.row = this.root.querySelector('#planetSetupGrid');
    this.count = this.root.querySelector('[data-setup-count]');
    this.hint = this.root.querySelector('[data-setup-hint]');
    this.beginBtn = this.root.querySelector('#planetSetupBegin');
    this.againBtn = this.root.querySelector('#planetSetupAgain');
    this.backBtn = this.root.querySelector('#planetSetupBack');
    this.beginBtn?.addEventListener('click', () => this.begin());
    this.againBtn?.addEventListener('click', () => this.restart());
    this.backBtn?.addEventListener('click', () => this.cancel());
  }

  get isOpen() { return this.phase !== 'closed' && (!this.root || !this.root.hidden); }
  get isDone() { return this.phase === 'done'; }

  open() {
    if (this.root) this.root.hidden = false;
    this.restart();
    this.backBtn?.focus();
    return this;
  }

  close() {
    this.phase = 'closed';
    if (this.root) this.root.hidden = true;
    return this;
  }

  // Read all four again from the start.
  restart() {
    this.planets = [];
    this.row?.replaceChildren();
    this.phase = 'reading';
    if (this.beginBtn) this.beginBtn.hidden = true;
    if (this.againBtn) this.againBtn.hidden = true;
    if (this.hint) this.hint.textContent = this.isEmulated() ? 'Keyboard headset: hold Space to calm, F to focus, Shift for stress, B to blink.' : 'Hold a feeling. Each world is made from it.';
    this.nextCard();
    return this;
  }

  // A deep copy, so nothing the galaxy receives is shared with the screen.
  result() { return this.planets.map((p) => ({ ...p, traits: { ...p.traits }, mood: { ...p.mood } })); }

  begin() {
    if (this.planets.length < SETUP_COUNT) return;
    const planets = toPlanetOptions(this.result());
    this.close();
    this.onBegin?.(planets);
  }

  cancel() {
    this.close();
    this.onCancel?.();
  }

  // Called every frame while open. The reading advances on the brain's live values.
  update(dt) {
    if (this.phase === 'reading') {
      this.reading.add(dt, this.getBrain?.());
      this.syncBack();
      if (this.reading.done) this.reveal();
    } else if (this.phase === 'reveal') {
      this.hold -= dt;
      if (this.hold <= 0) this.stash();
    }
  }

  nextCard() {
    const i = this.planets.length;
    this.reading = new MoodReading(READ_SECONDS);
    if (this.count) this.count.textContent = `World ${i + 1} of ${SETUP_COUNT}`;
    if (!this.row) return;
    const card = this.buildCard(i);
    glide(this.row, () => this.row.append(card));
    this.current = card;
    this.syncBack();
  }

  reveal() {
    const planet = planetFromMood(this.reading.mood, this.planets.length);
    this.planets.push(planet);
    this.phase = 'reveal';
    this.hold = reduceMotion() ? 0.6 : REVEAL_HOLD;
    if (this.current) {
      this.fillFront(this.current, planet);
      this.current.classList.add('is-revealed');
      this.current.setAttribute('aria-label', `${planet.name}: ${personalityLabel(planet.traits)}, ${BIOME_LABELS[planet.biome]}, born from your ${MOOD_LABEL[planet.channel].toLowerCase()}`);
    }
  }

  // The revealed card moves left into the row; the next one is read, or Begin appears.
  stash() {
    const card = this.current;
    if (this.planets.length < SETUP_COUNT) {
      this.phase = 'reading';
      if (card && this.row) glide(this.row, () => card.classList.add('is-done'));
      this.nextCard();
      return;
    }
    this.phase = 'done';
    this.current = null;
    if (card && this.row) glide(this.row, () => card.classList.add('is-done'));
    if (this.count) this.count.textContent = `${SETUP_COUNT} worlds`;
    if (this.hint) this.hint.textContent = 'Your mind made these. They join your sky.';
    if (this.beginBtn) { this.beginBtn.hidden = false; this.beginBtn.focus(); }
    if (this.againBtn) this.againBtn.hidden = false;
  }

  buildCard(i) {
    const card = el('article', 'pack-card');
    card.dataset.index = String(i);
    card.setAttribute('aria-label', `${SETUP_NAMES[i] || `World ${i + 1}`}: reading your mind`);
    const flip = el('div', 'pack-flip');

    const back = el('div', 'pack-face pack-back');
    back.append(el('span', 'pack-q', '?'), el('p', 'pack-reading', 'Reading your mind'));
    const meter = el('span', 'pack-meter');
    meter.append(el('i'));
    const live = el('ul', 'pack-live');
    for (const k of ['calm', 'focus', 'stress']) {
      const li = el('li');
      li.dataset.live = k;
      li.append(el('span', null, MOOD_LABEL[k]), el('b', null, '0'));
      live.append(li);
    }
    back.append(meter, live);

    const front = el('div', 'pack-face pack-front');
    flip.append(back, front);
    card.append(flip);
    return card;
  }

  syncBack() {
    const card = this.current;
    if (!card || !this.reading) return;
    card.querySelector('.pack-meter i')?.style.setProperty('--p', `${pct(this.reading.progress)}%`);
    const brain = this.getBrain?.();
    for (const k of ['calm', 'focus', 'stress']) {
      const b = card.querySelector(`[data-live="${k}"] b`);
      if (b) b.textContent = String(pct(brain?.[k]));
    }
  }

  fillFront(card, planet) {
    card.style.setProperty('--planet', BIOME_COLORS[planet.biome] || BIOME_COLORS.forming);
    const front = card.querySelector('.pack-front');
    if (!front) return;
    const top = el('div', 'pack-top');
    top.append(el('b', 'pack-rating', String(planet.rating)), el('span', 'pack-ch', MOOD_LABEL[planet.channel]));
    const stats = el('dl', 'pack-stats');
    for (const [k, short] of [['friendliness', 'FRI'], ['aggressiveness', 'AGG'], ['violence', 'VIO']]) {
      const d = el('div');
      d.append(el('dt', null, short), el('dd', null, String(pct(planet.traits[k]))));
      stats.append(d);
    }
    front.replaceChildren(
      top,
      el('span', 'pack-orb'),
      el('h2', null, planet.name),
      el('p', 'pack-sub', `${personalityLabel(planet.traits)} · ${BIOME_LABELS[planet.biome]}`),
      stats,
    );
  }
}
