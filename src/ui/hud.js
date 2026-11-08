// HTML HUD: era chip, objectives, the god-powers toolbar (each power is a brain signal
// with its live meter), and the judge overlay that shows the personal model.

import { FEATURE_NAMES } from '../brain/model.js';
import { STRESSED, QUALITY_USABLE } from '../brain/brain.js';
import { moodChip, sceneKind } from '../camera/mood-actions.js';

// Signal truth (#85): how far to trust what the headset says, next to the source chip. Quality is
// the brain's smoothed contact quality (the #175 gate), plus head tracking and the calibration
// window that seeds your civilisation. Null for the emulator (nothing to doubt).
export const QUALITY_GOOD = 0.7;
export function signalSummary(G, nowMs = globalThis.performance?.now?.() ?? 0) {
  const src = G?.source, b = G?.brain;
  if (!src || src.kind !== 'bridge' || !b) return null;
  const c = G.calibration;
  const calib = !c ? '' : c.done ? 'calibrated' : `calibrating ${Math.ceil(c.remaining)} s`;
  const streaming = src.connected && src.lastFrameAt > 0 && nowMs - src.lastFrameAt < 3000;
  if (!streaming) {
    return { level: 'poor', text: 'Signal: no data', title: 'The headset is not sending EEG: check that the Muse is on and paired with the bridge.' };
  }
  const q = Math.round((b.signalQuality ?? 1) * 100);
  const level = b.signalUnusable || b.signalQuality < QUALITY_USABLE ? 'poor' : b.signalQuality < QUALITY_GOOD ? 'fair' : 'good';
  const parts = [`Signal ${level} ${q}%`];
  if (!b.headTracking) parts.push('no head aim');
  if (calib) parts.push(calib);
  const why = level === 'poor' ? 'Poor contact: settle the band on your forehead and behind your ears. The era waits while it is this low.'
    : level === 'fair' ? 'Fair contact: calm and focus readings are noisier than usual.' : 'Good contact.';
  return { level, text: parts.join(' · '), title: `${why}${b.headTracking ? '' : ' Head tracking is unavailable: the mouse and arrow keys aim.'}${calib ? ` Seed window: ${calib}.` : ''}` };
}

// 8×8 pixel icons for the power tiles.
const ICONS = {
  blink: { c: '#ffcf4a', p: ['....##..', '...##...', '..##....', '.######.', '....##..', '...##...', '..##....', '.#......'] },
  // Rain spell (#276, #330): a rain cloud.
  focus: { c: '#9ec9ff', p: ['..####..', '.######.', '########', '..#..#..', '.#..#...', '..#..#..', '.#..#...', '........'] },
  // Stress: the cartoon "cross vein" of an agitated god.
  stress: { c: '#ff5a72', p: ['..#..#..', '.##..##.', '##....##', '........', '........', '##....##', '.##..##.', '..#..#..'] },
};

function paintIcon(canvas, icon) {
  if (!canvas || !icon) return;
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, 8, 8);
  ctx.fillStyle = icon.c;
  icon.p.forEach((row, y) => [...row].forEach((ch, x) => { if (ch === '#') ctx.fillRect(x, y, 1, 1); }));
}

const POWER_UNLOCK = { blink: 'stone', focus: 'stone', stress: 'industry' };
const ERA_ORDER = ['stone', 'farming', 'kingdoms', 'industry', 'space'];

// What each power tile is in a scene (#178): 'on' (in scene.powers: it changes this scene),
// 'off' (unlocked but does nothing here: shown disabled, not focusable) or 'hidden' (the campaign
// hasn't reached it yet, POWER_UNLOCK). Scenes outside the campaign order (the galaxy, the
// sandboxes, other worlds) count as having everything unlocked.
export function tileStates(scene) {
  const powers = scene?.powers || [];
  const at = ERA_ORDER.indexOf(scene?.era);
  const out = {};
  for (const [k, era] of Object.entries(POWER_UNLOCK)) {
    if (powers.includes(k)) out[k] = 'on';
    else if (at >= 0 && ERA_ORDER.indexOf(era) > at) out[k] = 'hidden';
    else out[k] = 'off';
  }
  return out;
}

let rustEngine = null;

export function setRustHudEngine(eng) {
  rustEngine = eng;
}

export class Hud {
  constructor(doc) {
    this.eraChip = doc.getElementById('eraChip');
    this.objectives = doc.getElementById('objectives');
    this.clock = doc.getElementById('clock');
    this.powers = {
      blink: doc.querySelector('[data-power="blink"]'),
      focus: doc.querySelector('[data-power="focus"]'),
      stress: doc.querySelector('[data-power="stress"]'),
    };
    this.source = doc.getElementById('sourceStatus');
    this.signal = doc.getElementById('signalChip');   // #85
    this.cause = doc.getElementById('causeToast');   // #177: what your hands and face just did
    this.moodEl = doc.getElementById('moodChip');    // #152: the joy buff and surprise cooldown
    this.causeLeft = 0;
    this.modelPanel = doc.getElementById('modelPanel');
    this.modelBody = doc.getElementById('modelBody');
    this.lastObj = '';
    this.blinkFlash = 0;
    for (const [k, el] of Object.entries(this.powers)) paintIcon(el.querySelector('.power-icon'), ICONS[k]);
  }

  setScene(scene) {
    this.eraChip.textContent = scene.label;
    const states = tileStates(scene);
    for (const [k, el] of Object.entries(this.powers)) {
      const s = states[k];
      el.classList.toggle('is-active', s === 'on');
      el.hidden = s === 'hidden';
      el.disabled = s !== 'on';   // a tile that does nothing here can't be pressed or focused
      el.title = s === 'off' ? 'Not used in this era' : '';
    }
  }

  // A short line over the map saying what a camera action did (grab, drop, palm, fireball, a
  // smile or surprise), or when a cooling cast will be ready (#177). The camera called this
  // optionally and it didn't exist, so every one of those calls did nothing.
  showCause(text, { ms = 2600, tone = 'ok' } = {}) {
    if (!this.cause || !text) return;
    this.cause.textContent = text;
    this.cause.dataset.tone = tone;
    this.cause.hidden = false;
    this.causeLeft = ms / 1000;
  }

  flashBlink() { this.blinkFlash = 0.3; this.powers.blink.classList.add('is-firing'); }

  // The headset chip in the top bar. The Rust shell never writes it, so both builds do it here:
  // under the Rust HUD update() used to return first, and a connected Muse still read "No headset".
  showSource(src) {
    if (!this.source || !src) return;
    const text = String(src.status ?? '');
    const ok = src.connected ? '1' : '0';
    if (this.source.textContent !== text) this.source.textContent = text;
    if (this.source.dataset.ok !== ok) this.source.dataset.ok = ok;
  }

  // The signal chip (#85): hidden for the emulator, else quality, head tracking and calibration.
  showSignal(G) {
    if (!this.signal) return;
    this.signalAt = (this.signalAt || 0) + 1;
    if (this.signalAt % 10) return;   // a few times a second is plenty
    const s = signalSummary(G);
    this.signal.hidden = !s;
    if (!s) return;
    if (this.signal.textContent !== s.text) this.signal.textContent = s.text;
    if (this.signal.dataset.level !== s.level) this.signal.dataset.level = s.level;
    if (this.signal.title !== s.title) this.signal.title = s.title;
  }

  // The mood chip (#152): hidden with no live mood, else the joy buff and the surprise cooldown.
  showMood(G, nowSec = (globalThis.performance?.now?.() ?? 0) / 1000) {
    const el = this.moodEl;
    if (!el) return;
    // The camera stamps the kind of the scene its mood reaches (the HUD's own scene is a wrapper).
    const kind = G?.mood && 'kind' in G.mood ? G.mood.kind : sceneKind(G?.scene);
    const c = moodChip(G?.mood, kind, nowSec);
    el.hidden = !c;
    if (!c) return;
    if (el.textContent !== c.text) el.textContent = c.text;
    if (el.dataset.tone !== c.tone) el.dataset.tone = c.tone;
    if (el.title !== c.title) el.title = c.title;
  }

  update(dt, G) {
    this.showSignal(G);
    this.showMood(G);
    if (this.causeLeft > 0) {
      this.causeLeft -= dt;
      if (this.causeLeft <= 0 && this.cause) this.cause.hidden = true;
    }
    this.showSource(G.source);
    if (rustEngine) {
      if (this.blinkFlash > 0) {
        this.blinkFlash -= dt;
        if (this.blinkFlash <= 0) this.powers.blink.classList.remove('is-firing');
      }
      // The Rust shell draws the meters, not the model panel (#173: this called a missing
      // updateModel(), which threw every frame the panel was open and froze the render).
      if (this.modelPanel && !this.modelPanel.hidden && G.brain) this.renderModel(G.brain, G);
      return;
    }
    const b = G.brain;
    const scene = G.scene;
    // Objectives (rebuild only when the text changes)
    if (scene) {
      const obj = scene.objectives();
      const key = JSON.stringify(obj);
      if (key !== this.lastObj) {
        this.lastObj = key;
        const items = this.objectives.children;
        if (items.length !== obj.length) {
          this.objectives.replaceChildren(...obj.map((o) => {
            const li = document.createElement('li');
            li.className = o.done ? 'is-done' : o.bad ? 'is-bad' : o.plain ? 'is-plain' : '';
            const l = document.createElement('span'); l.className = 'obj-label'; l.textContent = o.label;
            const v = document.createElement('span'); v.className = 'obj-value'; v.textContent = o.value;
            li.append(l, v);
            return li;
          }));
        } else {
          obj.forEach((o, i) => {
            const li = items[i];
            const cls = o.done ? 'is-done' : o.bad ? 'is-bad' : o.plain ? 'is-plain' : '';
            if (li.className !== cls) li.className = cls;
            const l = li.querySelector('.obj-label');
            if (l && l.textContent !== o.label) l.textContent = o.label;
            const v = li.querySelector('.obj-value');
            if (v && v.textContent !== o.value) v.textContent = o.value;
          });
        }
      }
    }
    const t = Math.floor(G.runTime);
    this.clock.textContent = `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;

    // Power meters
    const set = (el, value, thr, text) => {
      el.style.setProperty('--level', `${Math.round(value * 100)}%`);
      if (thr != null) el.style.setProperty('--thr', `${Math.round(thr * 100)}%`);
      el.querySelector('.power-value').textContent = text;
    };
    const focus = b.source?.input?.focus ? 1 : b.focus;
    set(this.powers.focus, focus, scene?.powers.includes('focus') ? b.threshold('focus', 0.5) : null, `${Math.round(focus * 100)}%`);
    // Stress: about 0.15 at rest, up with EEG agitation or an angry face (brain.js).
    const stress = b.stress ?? 0;
    set(this.powers.stress, stress, null, `${Math.round(stress * 100)}%`);
    this.powers.stress.classList.toggle('is-hot', stress > STRESSED);
    if (this.blinkFlash > 0) { this.blinkFlash -= dt; if (this.blinkFlash <= 0) this.powers.blink.classList.remove('is-firing'); }
    this.powers.blink.querySelector('.power-value').textContent = `${b.stats.blinks}`;


    if (!this.modelPanel.hidden) this.renderModel(b, G);
  }

  renderModel(b, G) {
    const m = b.model;
    const pct = (x) => `${Math.round(x * 100)}%`;
    const p = b.personal || {};
    const bar = (v, cls) => `<span class="mbar ${cls}"><span style="width:${pct(v || 0)}"></span></span>`;
    const weights = m.weights();
    const wRows = weights.map(({ c, w }) => `<tr><th>${c}</th>${w.map((x) => `<td class="${x >= 0 ? 'pos' : 'neg'}">${x.toFixed(1)}</td>`).join('')}</tr>`).join('');
    this.modelBody.innerHTML = `
      <p class="model-lede">Trained silently on the moments the game asked you to relax, look around and concentrate. There was no calibration screen.</p>
      <dl class="model-stats">
        <div><dt>Rest windows</dt><dd>${m.count('rest')}</dd></div>
        <div><dt>Calm windows</dt><dd>${m.count('calm')}</dd></div>
        <div><dt>Focus windows</dt><dd>${m.count('focus')}</dd></div>
        <div><dt>Cross-validated accuracy</dt><dd>${m.trained ? `${pct(m.accuracy)} <small>(chance ${pct(m.chance)})</small>` : 'not trained yet'}</dd></div>
        <div><dt>Trust in personal model</dt><dd>calm ${pct(m.trust('calm'))} · focus ${pct(m.trust('focus'))}</dd></div>
        <div><dt>Retrains</dt><dd>${m.trainCount}${m.trained ? ` · last took ${m.lastTrainMs.toFixed(0)} ms` : ''}</dd></div>
        <div><dt>Recording now</dt><dd>${b.cue || 'nothing'}</dd></div>
        <div><dt>Calibration seed</dt><dd>${calibrationText(G?.calibration)}</dd></div>
      </dl>
      <div class="model-live">
        <div><span>generic calm</span>${bar(b.genericCalm, 'calm')}</div>
        <div><span>personal calm</span>${bar(p.calm, 'calm')}</div>
        <div><span>generic focus</span>${bar(b.genericFocus, 'focus')}</div>
        <div><span>personal focus</span>${bar(p.focus, 'focus')}</div>
      </div>
      ${weights.length ? `<div class="model-weights"><table><thead><tr><th></th>${FEATURE_NAMES.map((n) => `<th>${n}</th>`).join('')}</tr></thead><tbody>${wRows}</tbody></table></div>` : ''}`;
  }
}

export { POWER_UNLOCK };

// calibration (#54): the player's civilisation seed, shown in the model panel.
function calibrationText(c) {
  if (!c) return 'n/a';
  if (!c.done) return `${c.config.mode}, ${Math.ceil(c.remaining)} s of ${c.seconds} s left`;
  const r = c.result;
  return `${r.source} · friendliness ${r.friendliness} · aggressiveness ${r.aggressiveness} · violence ${r.violence} · seed ${r.seed}`;
}
