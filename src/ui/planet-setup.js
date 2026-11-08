// The setup screen between the title and the home planet.
//
// Four worlds are waiting to be shaped. Each card exposes the three temperament traits the
// rest of the game already understands (friendliness, aggressiveness, violence) and shows how
// they lean the PPO god's five actions. The worlds then join the galaxy with those weights, so
// the player meets them again when they zoom out — but the campaign still starts at home,
// exactly as before.
//
// Pure state and maths live in src/ai/personality.js; this file only builds the cards and keeps
// the inputs in sync, which is why the Node tests can import the pure half without a DOM.

import {
  BIOMES, BIOME_COLORS, BIOME_LABELS, SETUP_COUNT, TRAITS,
  actionPreview, clampTrait, clampTraits, defaultSetupPlanets, personalityLabel,
  rerollSetupPlanets, toPlanetOptions, traitWeights,
} from '../ai/personality.js';

export { SETUP_COUNT, defaultSetupPlanets, rerollSetupPlanets, toPlanetOptions, traitWeights, actionPreview, personalityLabel };

const fmtPct = (v) => `${Math.round(v * 100)}%`;
const clonePlanets = (planets) => planets.map((p) => ({ ...p, traits: { ...p.traits } }));

export class PlanetSetup {
  constructor(root, { onBegin = null, onCancel = null } = {}) {
    this.root = root || null;
    this.onBegin = onBegin;
    this.onCancel = onCancel;
    this.planets = defaultSetupPlanets();
    if (!this.root) return;
    this.grid = this.root.querySelector('#planetSetupGrid');
    this.count = this.root.querySelector('[data-setup-count]');
    this.beginBtn = this.root.querySelector('#planetSetupBegin');
    this.randomBtn = this.root.querySelector('#planetSetupRandomise');
    this.backBtn = this.root.querySelector('#planetSetupBack');
    this.beginBtn?.addEventListener('click', () => this.begin());
    this.randomBtn?.addEventListener('click', () => this.randomise());
    this.backBtn?.addEventListener('click', () => this.cancel());
    this.grid?.addEventListener('input', (e) => this.onInput(e));
    this.grid?.addEventListener('change', (e) => this.onInput(e));
  }

  get isOpen() { return !!this.root && !this.root.hidden; }

  open({ planets = null } = {}) {
    this.planets = Array.isArray(planets) && planets.length
      ? clonePlanets(planets).map((p) => ({ ...p, traits: clampTraits(p.traits) }))
      : defaultSetupPlanets();
    this.render();
    if (this.root) {
      this.root.hidden = false;
      this.beginBtn?.focus();
    }
    return this;
  }

  close() {
    if (this.root) this.root.hidden = true;
    return this;
  }

  // A deep copy, so closing the screen never mutates what the galaxy will receive.
  result() { return clonePlanets(this.planets); }

  randomise() {
    this.planets = rerollSetupPlanets(this.planets);
    this.render();
    return this;
  }

  begin() {
    const planets = toPlanetOptions(this.result());
    this.close();
    this.onBegin?.(planets);
  }

  cancel() {
    this.close();
    this.onCancel?.();
  }

  render() {
    if (!this.grid) return;
    const cards = this.planets.map((p, i) => this.buildCard(p, i));
    this.grid.replaceChildren(...cards);
    if (this.count) this.count.textContent = `${this.planets.length} worlds`;
  }

  buildCard(planet, index) {
    const card = document.createElement('article');
    card.className = 'setup-card';
    card.dataset.index = String(index);

    const head = document.createElement('header');
    head.className = 'setup-card-head';
    const orb = document.createElement('span');
    orb.className = 'setup-orb';
    orb.setAttribute('aria-hidden', 'true');
    const title = document.createElement('div');
    title.className = 'setup-card-title';
    const h = document.createElement('h2');
    h.textContent = planet.name;
    const sub = document.createElement('p');
    const temperament = document.createElement('span');
    temperament.dataset.personality = '';
    const biomeName = document.createElement('span');
    biomeName.dataset.biomeName = '';
    sub.append(temperament, document.createTextNode(' · '), biomeName);
    title.append(h, sub);
    head.append(orb, title);

    const biome = document.createElement('label');
    biome.className = 'setup-biome';
    biome.append(document.createTextNode('Biome'));
    const select = document.createElement('select');
    select.dataset.biome = '';
    for (const b of BIOMES) {
      const opt = document.createElement('option');
      opt.value = b;
      opt.textContent = BIOME_LABELS[b] || b;
      if (b === planet.biome) opt.selected = true;
      select.append(opt);
    }
    biome.append(select);

    const traits = document.createElement('div');
    traits.className = 'setup-traits';
    for (const trait of TRAITS) {
      const label = document.createElement('label');
      label.className = 'setup-trait';
      label.title = trait.hint;
      const head2 = document.createElement('span');
      head2.className = 'setup-trait-head';
      const b = document.createElement('b');
      b.textContent = trait.label;
      const value = document.createElement('output');
      value.dataset.traitValue = trait.key;
      head2.append(b, value);
      const input = document.createElement('input');
      input.type = 'range';
      input.min = '0';
      input.max = '1';
      input.step = '0.05';
      input.value = String(planet.traits[trait.key]);
      input.dataset.trait = trait.key;
      input.setAttribute('aria-label', `${planet.name}: ${trait.label}`);
      label.append(head2, input);
      traits.append(label);
    }

    const weights = document.createElement('section');
    weights.className = 'setup-weights';
    weights.setAttribute('aria-label', `${planet.name}: PPO action bias`);
    const wh = document.createElement('header');
    const wl = document.createElement('span');
    wl.textContent = 'PPO bias';
    const ws = document.createElement('small');
    ws.textContent = 'action lean';
    wh.append(wl, ws);
    const list = document.createElement('ul');
    actionPreview(traitWeights(planet.traits)).forEach((a, i) => {
      const li = document.createElement('li');
      li.dataset.actionIndex = String(i);
      const name = document.createElement('span');
      name.className = 'setup-action';
      name.textContent = a.label;
      const bar = document.createElement('span');
      bar.className = 'setup-bar';
      const fill = document.createElement('i');
      fill.dataset.actionBar = '';
      bar.append(fill);
      const pct = document.createElement('b');
      pct.dataset.actionPct = '';
      li.append(name, bar, pct);
      list.append(li);
    });
    weights.append(wh, list);

    card.append(head, biome, traits, weights);
    this.syncCard(card, planet);
    return card;
  }

  // Update everything on a card except the inputs the player is dragging.
  syncCard(card, planet) {
    if (!card) return;
    const traits = clampTraits(planet.traits);
    card.style.setProperty('--planet', BIOME_COLORS[planet.biome] || BIOME_COLORS.forming);
    card.querySelector('[data-personality]').textContent = personalityLabel(traits);
    card.querySelector('[data-biome-name]').textContent = BIOME_LABELS[planet.biome] || planet.biome;
    for (const trait of TRAITS) {
      card.querySelector(`[data-trait-value="${trait.key}"]`).textContent = fmtPct(traits[trait.key]);
      const input = card.querySelector(`input[data-trait="${trait.key}"]`);
      if (input && document.activeElement !== input) input.value = String(traits[trait.key]);
    }
    actionPreview(traitWeights(traits)).forEach((a, i) => {
      const li = card.querySelector(`[data-action-index="${i}"]`);
      if (!li) return;
      li.querySelector('[data-action-bar]').style.setProperty('--p', `${a.pct}%`);
      li.querySelector('[data-action-pct]').textContent = `${a.pct}%`;
      li.title = `${a.label}: ${a.pct}% of the god's choices (weight ${a.weight.toFixed(2)})`;
    });
  }

  onInput(e) {
    const card = e.target.closest?.('.setup-card');
    if (!card || !this.grid?.contains(card)) return;
    const i = Number(card.dataset.index);
    const planet = this.planets[i];
    if (!planet) return;
    if (e.target.matches('input[data-trait]')) {
      planet.traits[e.target.dataset.trait] = clampTrait(Number(e.target.value), planet.traits[e.target.dataset.trait] ?? 0.5);
      this.syncCard(card, planet);
    } else if (e.target.matches('select[data-biome]')) {
      planet.biome = BIOMES.includes(e.target.value) ? e.target.value : planet.biome;
      this.syncCard(card, planet);
    }
  }
}
