// camera (#123): Animated Hand Coach Panel.
// Renders procedural 21-landmark ghost hands looping the expected move for the current context,
// reusing drawHand() from ./draw.js. Supports reduced-motion (static pose + caption) and H toggle.

import { clamp, lerp } from '../sim/util.js';
import { drawHand } from './draw.js';

export const COACH_KEY = 'KeyH';

// Base 21-landmark right hand in local hand space: wrist at (0,0), palm length = 1.
const BASE_HAND = [
  [0, 0],                                                       // 0 wrist
  [-0.25, 0.15], [-0.45, 0.35], [-0.6, 0.55], [-0.72, 0.75],    // 1–4 thumb
  [-0.3, 0.95], [-0.33, 1.35], [-0.35, 1.6], [-0.36, 1.8],      // 5–8 index
  [0, 1], [0, 1.45], [0, 1.72], [0, 1.95],                      // 9–12 middle
  [0.25, 0.95], [0.27, 1.35], [0.28, 1.6], [0.29, 1.8],         // 13–16 ring
  [0.47, 0.85], [0.52, 1.15], [0.55, 1.35], [0.57, 1.5],        // 17–20 pinky
];

const POSE_DELTAS = {
  open: {},
  pinch: { 4: [-0.48, 1.06], 6: [-0.34, 1.3], 7: [-0.43, 1.22], 8: [-0.46, 1.1] },
  fist: {
    4: [-0.1, 0.75],
    6: [-0.32, 1.15], 7: [-0.3, 0.8], 8: [-0.25, 0.6],
    10: [0, 1.15], 11: [0, 0.8], 12: [0, 0.65],
    14: [0.25, 1.1], 15: [0.25, 0.8], 16: [0.24, 0.65],
    18: [0.48, 1.0], 19: [0.46, 0.8], 20: [0.44, 0.7],
  },
  point: {
    4: [-0.08, 0.72],
    10: [0, 1.18], 11: [0, 0.92], 12: [0, 0.78],
    14: [0.26, 1.12], 15: [0.26, 0.92], 16: [0.25, 0.78],
    18: [0.49, 0.98], 19: [0.49, 0.84], 20: [0.47, 0.74],
  },
};

// Interpolate between two poses and project into 0..1 canvas space.
export function buildGhostHand(poseA, poseB, t, { cx = 0.5, cy = 0.78, size = 0.28, aspect = 1.6, mirrorX = false } = {}) {
  const a = POSE_DELTAS[poseA] || {};
  const b = POSE_DELTAS[poseB] || {};
  const k = clamp(t);
  return BASE_HAND.map(([bx, by], i) => {
    const [ax, ay] = a[i] || [bx, by];
    const [tx, ty] = b[i] || [bx, by];
    const hx = lerp(ax, tx, k) * (mirrorX ? -1 : 1);
    const hy = lerp(ay, ty, k);
    return {
      x: clamp(cx + (hx * size) / aspect, 0.02, 0.98),
      y: clamp(cy - hy * size, 0.02, 0.98),
      z: 0,
    };
  });
}

// Declarative keyframe data per move (#123: new moves are data, not new drawings).
export const COACH_MOVES = [
  {
    id: 'pinch_grab',
    title: 'Pinch Grab',
    key: '1',
    glyph: '🤏',
    instruction: 'Pinch to pick up a living creature under your head cursor, move head, release to drop.',
    contexts: ['surface', 'civ'],
    sample(phase) {
      // 0..0.25 close into pinch, 0.25..0.75 hold pinch & glide horizontally, 0.75..1.0 open to drop
      if (phase < 0.25) {
        const k = phase / 0.25;
        return [{ hand: buildGhostHand('open', 'pinch', k, { cx: 0.38, cy: 0.78 }), state: { nearPinch: k > 0.4, pinching: k > 0.85 } }];
      }
      if (phase < 0.75) {
        const k = (phase - 0.25) / 0.5;
        const cx = lerp(0.38, 0.64, Math.sin(k * Math.PI * 0.5));
        return [{ hand: buildGhostHand('pinch', 'pinch', 1, { cx, cy: 0.74 }), state: { pinching: true, aiming: true } }];
      }
      const k = (phase - 0.75) / 0.25;
      return [{ hand: buildGhostHand('pinch', 'open', k, { cx: 0.64, cy: 0.78 }), state: { nearPinch: k < 0.5 } }];
    },
  },
  {
    id: 'two_hand_zoom',
    title: 'Two-Hand Zoom',
    key: 'Z / X',
    glyph: '👐',
    instruction: 'Two hands pinch · move apart to zoom in, closer to zoom out; unpinch to freeze.',
    contexts: ['surface', 'globe', 'galaxy', 'civ'],
    sample(phase) {
      // Both hands pinch and spread apart, then unpinch to freeze
      const spread = phase < 0.7 ? Math.sin((phase / 0.7) * Math.PI * 0.5) : 1;
      const pinching = phase > 0.12 && phase < 0.78;
      const poseB = pinching ? 'pinch' : 'open';
      const k = phase < 0.15 ? phase / 0.15 : phase > 0.75 ? 1 - (phase - 0.75) / 0.25 : 1;
      const offset = lerp(0.13, 0.29, spread);
      return [
        { hand: buildGhostHand('open', poseB, k, { cx: 0.5 - offset, cy: 0.78, size: 0.24, mirrorX: true }), state: { pinching } },
        { hand: buildGhostHand('open', poseB, k, { cx: 0.5 + offset, cy: 0.78, size: 0.24, mirrorX: false }), state: { pinching } },
      ];
    },
  },
  {
    id: 'palm_calm',
    title: 'Open Palm · Calm',
    key: '2',
    glyph: '🖐️',
    instruction: 'Hold an open palm (or press 2) to calm fear, broker peace and bless the land.',
    contexts: ['surface', 'civ'],
    coachHidden: true,
    sample(phase) {
      const k = clamp(phase / 0.35);
      return [{ hand: buildGhostHand('open', 'open', k, { cx: 0.5, cy: 0.78, size: 0.27 }), state: { held: phase > 0.25 } }];
    },
  },
  {
    id: 'fist_strike',
    title: 'Closed Fist · Strike',
    key: '3',
    glyph: '✊',
    instruction: 'Hold a closed fist (or press 3) to strike lightning at your head cursor.',
    contexts: ['surface', 'civ'],
    sample(phase) {
      const k = phase < 0.3 ? phase / 0.3 : phase > 0.8 ? 1 - (phase - 0.8) / 0.2 : 1;
      return [{ hand: buildGhostHand('open', 'fist', k, { cx: 0.5, cy: 0.76, size: 0.27 }), state: { held: k > 0.75 } }];
    },
  },
];

export const COACH_MOVE_BY_ID = Object.fromEntries(COACH_MOVES.map((m) => [m.id, m]));
export const COACH_CARDS = COACH_MOVES.filter((m) => !m.coachHidden);

const TEMPLATE = `
  <div class="coach-bar">
    <button type="button" class="tool coach-toggle" data-coach-toggle aria-expanded="true" title="Toggle Hand Coach panel (H)">
      <span class="coach-dot" data-coach-dot data-ok="0"></span>
      <span>Hand Coach <kbd>H</kbd></span>
      <span class="coach-live-tag" data-coach-live>Head cursor · ${COACH_CARDS.length} moves</span>
    </button>
    <div class="coach-meter" title="Gesture hold / pinch strength"><span data-coach-meter></span></div>
    <span class="coach-feedback" data-coach-feedback aria-live="polite">Ready</span>
  </div>
  <div class="coach-body" data-coach-body>
    <div class="coach-grid" role="list" aria-label="Hand moves">
      ${COACH_CARDS.map((m) => `
        <article class="coach-card" role="listitem" data-coach-move="${m.id}">
          <div class="coach-card-head">
            <span class="coach-card-glyph" aria-hidden="true">${m.glyph}</span>
            <span class="coach-card-title">${m.title}</span>
            <kbd>${m.key}</kbd>
          </div>
          <canvas class="coach-canvas" width="220" height="110" data-coach-canvas="${m.id}" role="img" aria-label="Animated demonstration of ${m.title}"></canvas>
          <p class="coach-instr">${m.instruction}</p>
        </article>`).join('')}
    </div>
  </div>
`;

export function initCoach({ getContext = () => 'surface', onCastMove = null } = {}) {
  const panel = document.getElementById('coachPanel');
  if (!panel) return null;
  panel.innerHTML = TEMPLATE;
  const q = (sel) => panel.querySelector(sel);
  const toggleBtn = q('[data-coach-toggle]');
  const body = q('[data-coach-body]');
  const liveEl = q('[data-coach-live]');
  const dotEl = q('[data-coach-dot]');
  const feedbackEl = q('[data-coach-feedback]');
  const meterEl = q('[data-coach-meter]');
  const cardEls = new Map(COACH_CARDS.map((m) => [m.id, panel.querySelector(`[data-coach-move="${m.id}"]`)]));
  const canvases = new Map(COACH_CARDS.map((m) => {
    const canvas = panel.querySelector(`[data-coach-canvas="${m.id}"]`);
    return [m.id, { canvas, ctx: canvas.getContext('2d') }];
  }));

  // Collapsed by default: open on the first run it covered the playfield and the era card's
  // Begin button (#182). The bar stays visible (H or its toggle opens it); the player's last
  // choice is remembered.
  let open = false;
  try { open = localStorage.getItem('moc.coachOpen') === '1'; } catch {}

  let activeMoveId = COACH_CARDS[0].id;
  let flashState = null; // { text, kind: 'ok' | 'bad', until }

  function setOpen(next = !open) {
    open = !!next;
    try { localStorage.setItem('moc.coachOpen', open ? '1' : '0'); } catch {}
    body.hidden = !open;
    panel.classList.toggle('is-collapsed', !open);
    toggleBtn.setAttribute('aria-expanded', String(open));
  }
  setOpen(open);

  function selectMove(id) {
    if (!COACH_MOVE_BY_ID[id] || !cardEls.has(id)) return;
    activeMoveId = id;
    for (const [cardId, el] of cardEls) el.classList.toggle('is-live', cardId === id);
  }
  selectMove(activeMoveId);

  toggleBtn.addEventListener('click', () => setOpen(!open));

  window.addEventListener('keydown', (e) => {
    if (e.code !== COACH_KEY || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
    if (['titleScreen', 'failScreen', 'endScreen', 'banner'].some((id) => document.getElementById(id)?.hidden === false) || document.body.dataset.intro === 'on') return;   // not during the era intro either
    e.preventDefault();
    setOpen(!open);
  });

  function flash(text, kind = 'ok', durationMs = 2000) {
    flashState = { text, kind, until: performance.now() + durationMs };
    feedbackEl.textContent = text;
    feedbackEl.dataset.tone = kind;
  }

  function isReducedMotion() {
    return document.body.dataset.reducedMotion === '1' ||
      (typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  }

  function drawMove(move, ctx, w, h, now) {
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#0b0c16';
    ctx.fillRect(0, 0, w, h);

    const phase = isReducedMotion() ? 0.45 : ((now / 2200) % 1);
    const ghosts = move.sample(phase);

    ctx.save();
    ctx.globalAlpha = 0.88;
    for (const g of ghosts) drawHand(ctx, g.hand, w, h, g.state);
    ctx.restore();

    ctx.fillStyle = 'rgba(255, 207, 74, 0.85)';
    ctx.font = '11px "Pixelify Sans", monospace';
    ctx.textAlign = 'center';
    if (move.id === 'two_hand_zoom') {
      ctx.fillText('←  PINCH & SPREAD  →', w / 2, 16);
    } else if (move.id === 'pinch_grab') {
      ctx.fillText('PINCH · MOVE HEAD · DROP', w / 2, 16);
    }
  }

  function update(now = performance.now(), state = {}) {
    const context = getContext() || 'surface';

    if (state.bothPinched) selectMove('two_hand_zoom');
    else if (state.grabActive) selectMove('pinch_grab');
    else if (state.heldMove === 'fist') selectMove('fist_strike');

    const handCount = (state.leftVisible ? 1 : 0) + (state.rightVisible ? 1 : 0);
    dotEl.dataset.ok = state.live ? (handCount > 0 ? '1' : 'wait') : '0';
    liveEl.textContent = !state.live
      ? 'Camera off · Keys 1–3 active'
      : handCount === 2
        ? '2 hands tracked'
        : handCount === 1
          ? '1 hand tracked'
          : 'Show hand(s) to camera';

    const strength = clamp(state.holdProgress || state.pinchStrength || 0);
    meterEl.style.width = `${Math.round(strength * 100)}%`;

    if (flashState && now < flashState.until) {
      feedbackEl.textContent = flashState.text;
      feedbackEl.dataset.tone = flashState.kind;
    } else {
      flashState = null;
      feedbackEl.dataset.tone = 'neutral';
      feedbackEl.textContent = state.statusText || 'Ready';
    }

    if (!open) return;

    for (const move of COACH_CARDS) {
      const { canvas, ctx } = canvases.get(move.id);
      drawMove(move, ctx, canvas.width, canvas.height, now);
    }
  }

  return {
    get open() { return open; },
    get activeMoveId() { return activeMoveId; },
    setOpen,
    selectMove,
    flash,
    update,
  };
}
