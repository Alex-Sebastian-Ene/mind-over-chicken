// HTML HUD: era chip, objectives, the god-powers toolbar (each power is a brain signal
// with its live meter), and the judge overlay that shows the personal model.

import { FEATURE_NAMES } from '../brain/model.js';

// 8×8 pixel icons for the power tiles.
const ICONS = {
  blink: { c: '#ffcf4a', p: ['....##..', '...##...', '..##....', '.######.', '....##..', '...##...', '..##....', '.#......'] },
  calm: { c: '#59d8c8', p: ['...#....', '...##...', '..###...', '..####..', '.##.###.', '.#...##.', '.##.###.', '..####..'] },
  focus: { c: '#ffcf4a', p: ['........', '..####..', '.#....#.', '#..##..#', '#..##..#', '.#....#.', '..####..', '........'] },
  heart: { c: '#ff5a72', p: ['........', '.##.##..', '#######.', '#######.', '.#####..', '..###...', '...#....', '........'] },
};

function paintIcon(canvas, icon) {
  if (!canvas || !icon) return;
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, 8, 8);
  ctx.fillStyle = icon.c;
  icon.p.forEach((row, y) => [...row].forEach((ch, x) => { if (ch === '#') ctx.fillRect(x, y, 1, 1); }));
}

const POWER_UNLOCK = { blink: 'stone', calm: 'stone', focus: 'kingdoms', heart: 'industry' };

export class Hud {
  constructor(doc) {
    this.eraChip = doc.getElementById('eraChip');
    this.objectives = doc.getElementById('objectives');
    this.clock = doc.getElementById('clock');
    this.powers = {
      blink: doc.querySelector('[data-power="blink"]'),
      calm: doc.querySelector('[data-power="calm"]'),
      focus: doc.querySelector('[data-power="focus"]'),
      heart: doc.querySelector('[data-power="heart"]'),
    };
    this.source = doc.getElementById('sourceStatus');
    this.modelPanel = doc.getElementById('modelPanel');
    this.modelBody = doc.getElementById('modelBody');
    this.lastObj = '';
    this.blinkFlash = 0;
    for (const [k, el] of Object.entries(this.powers)) paintIcon(el.querySelector('.power-icon'), ICONS[k]);
  }

  setScene(scene) {
    this.eraChip.textContent = scene.label;
    for (const [k, el] of Object.entries(this.powers)) {
      el.classList.toggle('is-active', scene.powers.includes(k));
    }
  }

  flashBlink() { this.blinkFlash = 0.3; this.powers.blink.classList.add('is-firing'); }

  update(dt, G) {
    const b = G.brain;
    const scene = G.scene;
    // Objectives (rebuild only when the text changes)
    if (scene) {
      const obj = scene.objectives();
      const key = JSON.stringify(obj);
      if (key !== this.lastObj) {
        this.lastObj = key;
        this.objectives.replaceChildren(...obj.map((o) => {
          const li = document.createElement('li');
          li.className = o.done ? 'is-done' : o.bad ? 'is-bad' : o.plain ? 'is-plain' : '';
          const l = document.createElement('span'); l.className = 'obj-label'; l.textContent = o.label;
          const v = document.createElement('span'); v.className = 'obj-value'; v.textContent = o.value;
          li.append(l, v);
          return li;
        }));
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
    set(this.powers.calm, b.calm, scene?.powers.includes('calm') ? b.threshold('calm', 0.55) : null, `${Math.round(b.calm * 100)}%`);
    set(this.powers.focus, b.focus, scene?.powers.includes('focus') ? b.threshold('focus', 0.5) : null, `${Math.round(b.focus * 100)}%`);
    const d = b.hrDelta();
    set(this.powers.heart, Math.min(1, Math.max(0, d / 30)), null, b.hr ? `${Math.round(b.hr)} bpm` : 'no pulse');
    this.powers.heart.classList.toggle('is-hot', d > 10);
    this.powers.heart.querySelector('.power-sub').textContent = b.hr && b.restHr ? `${d >= 0 ? '+' : ''}${Math.round(d)} vs resting` : 'Heart rate';
    if (this.blinkFlash > 0) { this.blinkFlash -= dt; if (this.blinkFlash <= 0) this.powers.blink.classList.remove('is-firing'); }
    this.powers.blink.querySelector('.power-value').textContent = `${b.stats.blinks}`;

    this.source.textContent = G.source.status;
    this.source.dataset.ok = G.source.connected ? '1' : '0';

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
      <p class="model-lede">Trained silently on the moments the game asked you to relax, look around and do sums. There was no calibration screen.</p>
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
