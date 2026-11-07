// camera debug (#67): the Debug tab. A small panel, opened from the top bar or with the backquote
// key (` — D already pans the planet), that shows what the game reads from you:
//   - the mirrored camera thumbnail with the 21 hand landmarks, the aiming fingertip, the pinch and the face box;
//   - the recognised gesture and its confidence, hold progress, the pinch, the learned hand range and cooldowns;
//   - live emotion bars and the dominant mood;
//   - inference rate (Hz), model time and end-to-end latency (ms) for hands and face, and where they run;
//   - the brain signals: calm, focus, stress (EEG agitation or face anger), blinks; and the game's frame rate.
// Without a camera it still shows key casts, cooldowns, brain values and the frame rate.

import { SPELL_BY_ID, GESTURE_LABEL } from './spells.js';
import { EMOTIONS } from './emotion.js';
import { drawHand } from './draw.js';
import { FrameMeter } from './meter.js';

export const DEBUG_KEY = 'Backquote';
const TEXT_MS = 120;   // text refresh; the thumbnail redraws every frame while open

const TEMPLATE = `
  <header class="dbg-head">
    <h2 class="dbg-title">Debug <kbd>\`</kbd></h2>
    <span class="dbg-mode" data-dbg="mode">camera off</span>
    <button type="button" class="tool dbg-close" data-dbg-close aria-label="Close the debug panel">Close</button>
  </header>
  <p class="dbg-note" data-dbg="note" hidden></p>
  <div class="dbg-cam">
    <canvas class="dbg-thumb" width="160" height="120" role="img" aria-label="Mirrored camera thumbnail with hand landmarks"></canvas>
    <dl class="dbg-rates">
      <div><dt>Hands</dt><dd data-dbg="hands">–</dd></div>
      <div><dt>Face</dt><dd data-dbg="face">–</dd></div>
      <div><dt>Game</dt><dd data-dbg="game">–</dd></div>
    </dl>
  </div>
  <section class="dbg-sec" aria-label="Hand">
    <p class="dbg-row"><span>Gesture</span><b data-dbg="gesture">–</b></p>
    <p class="dbg-row"><span>Hold</span><span class="dbg-meter"><span data-dbg="hold"></span></span><b data-dbg="holdText">–</b></p>
    <p class="dbg-row"><span>Pinch</span><b data-dbg="pinch">–</b></p>
    <p class="dbg-row"><span>Range</span><b data-dbg="range">–</b></p>
    <ul class="dbg-cds" data-dbg="cds" aria-label="Cooldowns"></ul>
    <p class="dbg-row"><span>Last cast</span><b data-dbg="last">none yet · press 1–7</b></p>
  </section>
  <section class="dbg-sec" aria-label="Mood">
    <p class="dbg-row"><span>Mood</span><b data-dbg="mood">camera off</b></p>
    ${EMOTIONS.map((k) => `<p class="dbg-emo" data-emotion="${k}"><span>${k}</span><span class="dbg-meter"><span></span></span><b>0</b></p>`).join('')}
  </section>
  <section class="dbg-sec" aria-label="Brain signals">
    <dl class="dbg-brain">
      <div><dt>Calm</dt><dd data-dbg="calm">–</dd></div>
      <div><dt>Focus</dt><dd data-dbg="focus">–</dd></div>
      <div><dt>Stress</dt><dd data-dbg="stress">–</dd></div>
      <div><dt>Blinks</dt><dd data-dbg="blinks">–</dd></div>
    </dl>
    <p class="dbg-src" data-dbg="source">no game running</p>
  </section>`;

const pct = (v) => `${Math.round((v || 0) * 100)}%`;
const secs = (ms) => `${(ms / 1000).toFixed(1)} s`;

export function initDebug({ getG, camera }) {
  const btn = document.getElementById('dbgBtn');
  const panel = document.getElementById('dbgPanel');
  if (!btn || !panel) return null;
  panel.innerHTML = TEMPLATE;
  const q = (k) => panel.querySelector(`[data-dbg="${k}"]`);
  const el = Object.fromEntries(['mode', 'note', 'hands', 'face', 'game', 'gesture', 'hold', 'holdText', 'pinch', 'range', 'cds', 'last', 'mood', 'calm', 'focus', 'stress', 'blinks', 'source'].map((k) => [k, q(k)]));
  const emo = Object.fromEntries(EMOTIONS.map((k) => {
    const row = panel.querySelector(`.dbg-emo[data-emotion="${k}"]`);
    return [k, { bar: row.querySelector('.dbg-meter > span'), num: row.querySelector('b') }];
  }));
  const thumb = panel.querySelector('.dbg-thumb');
  const tctx = thumb.getContext('2d');
  const frames = new FrameMeter();
  let open = false, textAt = 0, cdKey = '';

  const set = (node, text) => { if (node.textContent !== text) node.textContent = text; };

  function toggle(next = !open) {
    if (camera?.togglePanel) {
      camera.togglePanel(next);
      open = !document.getElementById('camPanel')?.hidden;
      return;
    }
    open = next;
    panel.hidden = !open;
    btn.setAttribute('aria-pressed', String(open));
    if (open) { frames.reset(); textAt = 0; }
  }
  btn.addEventListener('click', () => toggle());
  panel.querySelector('[data-dbg-close]').addEventListener('click', () => toggle(false));
  window.addEventListener('keydown', (e) => {
    if (e.code !== DEBUG_KEY || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
    e.preventDefault();
    toggle();
  });

  // ---------- thumbnail ----------
  function drawThumb(s) {
    const w = thumb.width, h = thumb.height;
    tctx.setTransform(1, 0, 0, 1, 0, 0);
    tctx.fillStyle = '#0b0c16';
    tctx.fillRect(0, 0, w, h);
    const v = s?.video;
    if (!s?.live || !v || v.readyState < 2) {
      tctx.fillStyle = '#a3a6c4';
      tctx.font = '11px "Atkinson Hyperlegible", system-ui, sans-serif';
      tctx.textAlign = 'center';
      tctx.fillText(s?.status === 'starting' ? 'starting camera…' : 'no camera', w / 2, h / 2 - 4);
      tctx.fillText('keys 1–7 still cast', w / 2, h / 2 + 12);
      return;
    }
    // Mirrored, like the camera panel, so it moves the way you do.
    tctx.setTransform(-1, 0, 0, 1, w, 0);
    const vw = v.videoWidth || 4, vh = v.videoHeight || 3;
    // object-fit: cover into the thumbnail
    const k = Math.max(w / vw, h / vh), dw = vw * k, dh = vh * k, ox = (w - dw) / 2, oy = (h - dh) / 2;
    tctx.drawImage(v, ox, oy, dw, dh);
    tctx.translate(ox, oy);
    if (s.faceBox) {
      const b = s.faceBox;
      tctx.strokeStyle = 'rgba(255,207,74,0.7)';
      tctx.lineWidth = 1;
      tctx.strokeRect(b.x0 * dw, b.y0 * dh, (b.x1 - b.x0) * dw, (b.y1 - b.y0) * dh);
    }
    drawHand(tctx, s.hand, dw, dh, {
      held: !!s.hold?.held, aiming: s.finger?.mode === 'point', pinching: !!s.finger?.pinching,
      nearPinch: s.finger?.ratio != null && s.finger.ratio < s.pinch.exit,
    });
    tctx.setTransform(1, 0, 0, 1, 0, 0);
  }

  // ---------- text ----------
  function rate(r) {
    if (!r || !r.count) return 'waiting for frames';
    const parts = [`${Math.round(r.hz)} Hz`];
    if (r.ms != null) parts.push(`${Math.round(r.ms)} ms`);
    if (r.latency != null) parts.push(`${Math.round(r.latency)} ms lag`);
    return parts.join(' · ');
  }

  function updateText(now, s, G) {
    const e = s?.engine;
    // Where inference runs.
    let mode = 'camera off';
    if (s?.status === 'starting') mode = 'loading models…';
    else if (s?.status === 'error') mode = 'camera unavailable';
    else if (s?.live && e) {
      const where = e.mode === 'worker' ? `worker ×${e.workers}` : 'main thread';
      const dl = [...new Set([e.gesture.delegate, e.face.delegate].filter(Boolean))].join('/');
      mode = `${where}${dl ? ` · ${dl}` : ''}`;
    }
    set(el.mode, mode);
    const note = s?.live && e?.note ? e.note : '';
    set(el.note, note);
    el.note.hidden = !note;
    set(el.hands, s?.live && e && s.castsEnabled ? rate(e.gesture) : s?.castsEnabled === false ? 'casts off' : '–');
    set(el.face, s?.live && e ? rate(e.face) : '–');
    set(el.game, `${Math.round(frames.fps(now))} fps · ${frames.long} long`);
    el.game.title = frames.long ? `Frames over 50 ms since the panel opened (worst ${Math.round(frames.worst)} ms)` : 'No frame over 50 ms since the panel opened';

    // Hand
    const g = s?.gesture || { name: 'None', score: 0 };
    const f = s?.finger;
    const handTxt = !s?.live ? 'no camera' : !s.hand ? 'no hand' : `${GESTURE_LABEL[g.name] || g.name} ${g.score.toFixed(2)}${f?.mode === 'point' ? ' · aiming' : ''}`;
    set(el.gesture, handTxt);
    const held = s?.hold?.held ? SPELL_BY_ID[s.hold.held] : null;
    const heldCd = held ? s.cooldowns.find((c) => c.id === held.id) : null;
    el.hold.style.width = pct(held ? s.hold.progress : 0);
    set(el.holdText, held ? (heldCd.left > 0 ? `${held.glyph} ready in ${secs(heldCd.left)}` : `${held.glyph} ${held.name} ${pct(s.hold.progress)}`) : '–');
    const armed = SPELL_BY_ID[s?.armed || 'fireball'];
    const pinchTxt = f?.ratio == null ? `let go casts ${armed.glyph}` : f.pinching
      ? `held ${f.ratio.toFixed(2)} · let go casts ${armed.glyph}`
      : `open ${f.ratio.toFixed(2)} (< ${s.pinch.enter}) · ${armed.glyph}`;
    set(el.pinch, pinchTxt);
    const rg = s?.range;
    set(el.range, !rg ? '–' : `${rg.calibrated ? 'learned' : `learning ${pct(rg.progress)}`} · x ${rg.x[0].toFixed(2)}–${rg.x[1].toFixed(2)} y ${rg.y[0].toFixed(2)}–${rg.y[1].toFixed(2)}`);

    // Cooldowns: rebuild the chips only when the set of cooling spells changes; update widths always.
    const cds = s?.cooldowns || [];
    const key = cds.map((c) => `${c.id}${c.left > 0 ? 1 : 0}${c.id === s?.armed ? 'a' : ''}`).join();
    if (key !== cdKey) {
      cdKey = key;
      el.cds.replaceChildren(...cds.map((c) => {
        const li = document.createElement('li');
        li.className = 'dbg-cd';
        li.dataset.spell = c.id;
        if (c.left > 0) li.classList.add('is-cooling');
        if (c.id === s.armed) li.classList.add('is-armed');
        li.title = `${c.name} · key ${c.key}${c.id === s.armed ? ' · armed for pinch' : ''}`;
        const gl = document.createElement('span'); gl.textContent = c.glyph; gl.setAttribute('aria-hidden', 'true');
        const t = document.createElement('b');
        const bar = document.createElement('i');
        li.append(gl, t, bar);
        return li;
      }));
    }
    for (const c of cds) {
      const li = el.cds.querySelector(`[data-spell="${c.id}"]`);
      if (!li) continue;
      set(li.querySelector('b'), c.left > 0 ? (c.left / 1000).toFixed(1) : String(c.key));
      li.querySelector('i').style.width = pct(c.frac);
    }
    const last = s?.history?.[0];
    set(el.last, last ? `${SPELL_BY_ID[last.id].glyph} ${SPELL_BY_ID[last.id].name} · ${last.via} · ${secs(now - last.at)} ago` : 'none yet · press 1–7');

    // Mood
    const m = s?.mood;
    set(el.mood, !s?.live ? 'camera off' : !m?.face ? 'no face' : m.calibrating ? 'reading your face…' : `${m.dominant} ${pct(m.intensity)}`);
    for (const k of EMOTIONS) {
      const v = s?.live ? m?.[k] || 0 : 0;
      emo[k].bar.style.width = pct(v);
      set(emo[k].num, v.toFixed(2));
    }
    panel.dataset.mood = s?.live ? m?.dominant || 'neutral' : 'off';

    // Brain
    const b = G?.brain;
    if (b) {
      set(el.calm, pct(b.calm));
      set(el.focus, pct(b.focus));
      set(el.stress, b.stress == null ? '–' : `${pct(b.stress)}${b.anger > 0 && b.anger >= b.stress - 1e-6 ? ' · face anger' : ''}`);
      set(el.blinks, String(b.stats?.blinks ?? 0));
      set(el.source, `${G.source?.kind === 'sim' ? 'emulated headset' : 'headset bridge'} · signal ${pct(b.quality)}`);
    } else {
      for (const k of ['calm', 'focus', 'stress', 'blinks']) set(el[k], '–');
      set(el.source, 'no game running: start one for brain signals');
    }
  }

  // Scheduled exactly like the game loop (rAF, with a 33 ms timer when an embedded view throttles
  // rAF), on the same thread, so its frame pacing is the game's: a blocked main thread shows here.
  let lastAt = 0;
  function tick() {
    const now = performance.now();
    if (now - lastAt < 8) return;
    lastAt = now;
    frames.frame(now);
    if (open) {
      const s = camera?.debugState?.(now) || null;
      drawThumb(s);
      if (now - textAt >= TEXT_MS) { textAt = now; updateText(now, s, getG()); }
    }
  }
  function rafLoop() { tick(); requestAnimationFrame(rafLoop); }
  requestAnimationFrame(rafLoop);
  setInterval(() => { if (performance.now() - lastAt > 45) tick(); }, 33);

  return {
    toggle,
    get open() { return open; },
    frames,
    // Game frame pacing over the panel's lifetime, for checking that inference drops no frames.
    frameStats(now = performance.now()) { return { fps: Math.round(frames.fps(now) * 10) / 10, long: frames.long, worst: Math.round(frames.worst) }; },
  };
}
