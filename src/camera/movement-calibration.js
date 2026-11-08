// Pinch movement calibration (#282, #286): live camera with the tracked hand wireframe, a target
// ghost that sweeps across / up-down / diagonally, and per-axis gains.
//
// #286: draw the mirrored webcam frame under the wireframe so the player can line their real hand
// up; animate the target out and back (figure-eight); capture X and Y separately so up/down and
// diagonals are calibrated; left hand blue, right hand amber (draw.js).

import { buildGhostHand } from './coach.js';
import { drawHand } from './draw.js';

const KEY = 'moc.pinchCalibration';
const TARGET_SPAN = 0.3;
const STEP_MS = [8000, 8000, 8000];
const STEP_IDS = ['left', 'right', 'head'];

export function gainFromSpan(span, target = TARGET_SPAN) {
  const s = Number.isFinite(span) ? Math.max(0, span) : 0;
  return Math.max(0.5, Math.min(2.5, target / Math.max(0.01, s)));
}

export function defaultPinchCalibration() {
  return {
    left: { gainX: 1, gainY: 1, spanX: 0, spanY: 0, calibrated: false },
    right: { gainX: 1, gainY: 1, spanX: 0, spanY: 0, calibrated: false },
    head: { gainX: 1, gainY: 1, spanX: 0, spanY: 0, calibrated: false },
  };
}

function normalise(raw) {
  const base = defaultPinchCalibration();
  if (!raw || typeof raw !== 'object') return base;
  for (const k of STEP_IDS) {
    const v = raw[k];
    if (!v || typeof v !== 'object') continue;
    const legacy = Number(v.gain);
    const gx = Number(v.gainX ?? legacy);
    const gy = Number(v.gainY ?? legacy);
    base[k] = {
      gainX: Number.isFinite(gx) ? Math.max(0.5, Math.min(2.5, gx)) : 1,
      gainY: Number.isFinite(gy) ? Math.max(0.5, Math.min(2.5, gy)) : 1,
      spanX: Number.isFinite(v.spanX ?? v.span) ? Math.max(0, v.spanX ?? v.span) : 0,
      spanY: Number.isFinite(v.spanY ?? v.span) ? Math.max(0, v.spanY ?? v.span) : 0,
      calibrated: !!v.calibrated,
    };
  }
  return base;
}

export function loadPinchCalibration() {
  try {
    return normalise(JSON.parse(localStorage.getItem(KEY) || 'null'));
  } catch {
    return defaultPinchCalibration();
  }
}

export function savePinchCalibration(cal) {
  try { localStorage.setItem(KEY, JSON.stringify(normalise(cal))); } catch {}
}

const TITLES = ['LEFT HAND · pinch & sweep', 'RIGHT HAND · pinch & sweep', 'PINCH + TURN / NOD HEAD'];
const HINTS = [
  'Pinch your left hand and follow the path: across, up/down, then diagonals. Out and back.',
  'Pinch your right hand and follow the path: across, up/down, then diagonals. Out and back.',
  'Keep a hand pinched and turn + nod your head, including diagonals.',
];

const smooth = (t) => 0.5 - 0.5 * Math.cos(Math.max(0, Math.min(1, t)) * Math.PI);
const clamp01 = (v) => Math.max(0, Math.min(1, v));

function drawArrow(ctx, x0, y0, x1, y1, color) {
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); ctx.stroke();
  const a = Math.atan2(y1 - y0, x1 - x0), d = 7;
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x1 - Math.cos(a - 0.5) * d, y1 - Math.sin(a - 0.5) * d);
  ctx.lineTo(x1 - Math.cos(a + 0.5) * d, y1 - Math.sin(a + 0.5) * d);
  ctx.closePath(); ctx.fillStyle = color; ctx.fill();
}

function drawHead(ctx, w, h, yaw, pitch) {
  const cx = w / 2, cy = h * 0.45 + pitch * h * 0.08, r = Math.min(w, h) * 0.22;
  ctx.save();
  ctx.strokeStyle = '#59d8c8';
  ctx.fillStyle = 'rgba(89,216,200,0.08)';
  ctx.lineWidth = 1.6;
  ctx.beginPath(); ctx.ellipse(cx, cy, r * 0.78, r, 0, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
  ctx.beginPath(); ctx.arc(cx, cy + r * 0.18, r * 0.55, 0.15 * Math.PI, 0.85 * Math.PI); ctx.stroke();
  const ear = r * 0.78, ox = yaw * r * 0.22;
  ctx.beginPath(); ctx.arc(cx - ear + ox, cy, r * 0.16, 0, Math.PI * 2); ctx.stroke();
  ctx.beginPath(); ctx.arc(cx + ear + ox, cy, r * 0.16, 0, Math.PI * 2); ctx.stroke();
  const ex = yaw * r * 0.28;
  ctx.fillStyle = '#f4f0e4';
  for (const s of [-1, 1]) { ctx.beginPath(); ctx.arc(cx + s * r * 0.28 + ex, cy - r * 0.12 + pitch * r * 0.12, r * 0.07, 0, Math.PI * 2); ctx.fill(); }
  const nx = yaw * r * 0.45, ny = pitch * r * 0.16;
  ctx.strokeStyle = '#ffd166';
  ctx.beginPath();
  ctx.moveTo(cx + nx, cy - r * 0.02 + ny);
  ctx.lineTo(cx + nx - r * 0.12, cy + r * 0.36 + ny);
  ctx.lineTo(cx + nx + r * 0.14, cy + r * 0.36 + ny);
  ctx.closePath(); ctx.stroke();
  ctx.strokeStyle = 'rgba(255,207,74,0.35)';
  ctx.beginPath(); ctx.arc(cx, cy + r * 0.75, r * 0.8, Math.PI * 1.15, Math.PI * 1.85); ctx.stroke();
  ctx.restore();
}

const emptyCap = () => ({ n: 0, minX: 1, maxX: 0, minY: 1, maxY: 0 });
const addCap = (c, p) => {
  if (!c || !p || !Number.isFinite(p.x) || !Number.isFinite(p.y)) return;
  c.n++;
  c.minX = Math.min(c.minX, p.x); c.maxX = Math.max(c.maxX, p.x);
  c.minY = Math.min(c.minY, p.y); c.maxY = Math.max(c.maxY, p.y);
};
const spanX = (c) => (c && c.n > 1 ? c.maxX - c.minX : 0);
const spanY = (c) => (c && c.n > 1 ? c.maxY - c.minY : 0);

function coverRect(video, w, h) {
  const vw = video?.videoWidth || 0, vh = video?.videoHeight || 0;
  if (!vw || !vh) return null;
  const k = Math.max(w / vw, h / vh), dw = vw * k, dh = vh * k;
  return { ox: (w - dw) / 2, oy: (h - dh) / 2, dw, dh };
}

function mapHand(hand, rect, w, h) {
  return hand.map((p) => ({ x: (rect.ox + p.x * rect.dw) / w, y: (rect.oy + p.y * rect.dh) / h }));
}

export function initMovementCalibration({
  canvas, statusEl, msgEl, startBtn, skipBtn, resetBtn,
  getState, onChange,
}) {
  if (!canvas) return null;
  const ctx = canvas.getContext('2d');
  let active = false, step = 0, stepStart = 0, flashUntil = 0;
  let captures = {};

  const setStatus = (text) => { if (statusEl && statusEl.textContent !== text) statusEl.textContent = text; };
  const setMsg = (text) => { if (msgEl && msgEl.textContent !== text) msgEl.textContent = text; };

  function begin() {
    const s = getState();
    if (!s.live) {
      setStatus('camera off');
      setMsg('Start Camera first, then pinch and follow the path.');
      flashUntil = performance.now() + 3000;
      return;
    }
    active = true; step = 0; stepStart = performance.now();
    captures = Object.fromEntries(STEP_IDS.map((k) => [k, emptyCap()]));
    setStatus(TITLES[0]); setMsg(HINTS[0]);
    if (skipBtn) skipBtn.disabled = false;
    if (startBtn) startBtn.textContent = 'Restart';
  }

  function stop() {
    active = false;
    if (skipBtn) skipBtn.disabled = true;
    if (startBtn) startBtn.textContent = 'Calibrate';
    setStatus('idle');
  }

  function next() {
    if (!active) return;
    if (step < STEP_IDS.length - 1) {
      step++;
      stepStart = performance.now();
      setStatus(TITLES[step]);
      setMsg(HINTS[step]);
      return;
    }
    finish();
  }

  function finish() {
    const out = defaultPinchCalibration();
    for (const k of STEP_IDS) {
      const sx = spanX(captures[k]), sy = spanY(captures[k]);
      out[k] = {
        gainX: sx > 0.01 ? gainFromSpan(sx) : 1,
        gainY: sy > 0.01 ? gainFromSpan(sy) : 1,
        spanX: sx,
        spanY: sy,
        calibrated: sx > 0.01 && sy > 0.01,
      };
    }
    active = false;
    if (skipBtn) skipBtn.disabled = true;
    if (startBtn) startBtn.textContent = 'Calibrate';
    if (onChange) onChange(out);
    setStatus(`calibrated · L ${out.left.gainX.toFixed(1)}/${out.left.gainY.toFixed(1)} · R ${out.right.gainX.toFixed(1)}/${out.right.gainY.toFixed(1)} · H ${out.head.gainX.toFixed(1)}/${out.head.gainY.toFixed(1)}`);
    setMsg('Saved. Reset restores 1× for every input.');
    flashUntil = performance.now() + 3500;
  }

  function reset() {
    active = false; captures = {};
    if (skipBtn) skipBtn.disabled = true;
    if (startBtn) startBtn.textContent = 'Calibrate';
    if (onChange) onChange(defaultPinchCalibration());
    setStatus('reset'); setMsg('Pinch movement is back to 1×. Calibrate to tune it again.');
    flashUntil = performance.now() + 3000;
  }

  startBtn?.addEventListener('click', begin);
  skipBtn?.addEventListener('click', next);
  resetBtn?.addEventListener('click', reset);
  if (skipBtn) skipBtn.disabled = true;

  function draw(now) {
    const w = canvas.width, h = canvas.height;
    const s = getState();
    const video = s.video;
    const rect = video && video.readyState >= 2 ? coverRect(video, w, h) : null;
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#0b0c16';
    ctx.fillRect(0, 0, w, h);

    // Mirrored camera frame + the player's live hand wireframes.
    if (rect) {
      ctx.save();
      ctx.translate(w, 0); ctx.scale(-1, 1);
      ctx.drawImage(video, rect.ox, rect.oy, rect.dw, rect.dh);
      for (const side of ['left', 'right']) {
        const hand = s.hands?.[side];
        if (!hand?.visible || !hand.landmarks) continue;
        const isTarget = !active || step === 2 || (side === 'left' ? step === 0 : step === 1);
        ctx.globalAlpha = isTarget ? 1 : 0.35;
        drawHand(ctx, mapHand(hand.landmarks, rect, w, h), w, h, {
          side, pinching: hand.pinching,
        });
      }
      ctx.globalAlpha = 1;
      ctx.restore();
    } else {
      for (const side of ['left', 'right']) {
        const hand = s.hands?.[side];
        if (hand?.visible && hand.landmarks) {
          ctx.globalAlpha = (active && step < 2 && side !== (step === 0 ? 'left' : 'right')) ? 0.35 : 1;
          drawHand(ctx, hand.landmarks, w, h, { side, pinching: hand.pinching });
        }
      }
      ctx.globalAlpha = 1;
    }

    const phase = active ? clamp01((now - stepStart) / STEP_MS[step]) : (now / 3200) % 1;
    if (active && step === 2) {
      drawHead(ctx, w, h, Math.sin(phase * Math.PI * 2) * 0.9, Math.sin(phase * Math.PI * 4) * 0.7);
      drawArrow(ctx, w * 0.12, h * 0.12, w * 0.35, h * 0.12, 'rgba(255,207,74,0.8)');
      drawArrow(ctx, w * 0.88, h * 0.12, w * 0.65, h * 0.12, 'rgba(255,207,74,0.8)');
      drawArrow(ctx, w * 0.5, h * 0.04, w * 0.5, h * 0.18, 'rgba(255,207,74,0.5)');
      drawArrow(ctx, w * 0.5, h * 0.34, w * 0.5, h * 0.2, 'rgba(255,207,74,0.5)');
      ctx.fillStyle = 'rgba(244,240,228,0.78)';
      ctx.font = `${Math.max(12, Math.round(w / 60))}px "Pixelify Sans", monospace`; ctx.textAlign = 'center';
      ctx.fillText('TURN + NOD + DIAGONAL', w / 2, h - 14);
    } else {
      // Figure-eight target: across, up/down and diagonals, out and back.
      const cx = 0.5 + Math.sin(phase * Math.PI * 2) * 0.27;
      const cy = 0.5 + Math.sin(phase * Math.PI * 4) * 0.16;
      const dir = active && step === 1 ? 'right' : 'left';
      ctx.globalAlpha = 0.62;
      const g = buildGhostHand('pinch', 'pinch', 1, { cx, cy, size: 0.24, mirrorX: dir === 'right' });
      drawHand(ctx, g, w, h, { pinching: true, held: true, side: dir });
      ctx.globalAlpha = 1;
      drawArrow(ctx, w * 0.15, h * 0.2, w * 0.85, h * 0.2, 'rgba(255,207,84,0.45)');
      drawArrow(ctx, w * 0.5, h * 0.12, w * 0.5, h * 0.4, 'rgba(255,207,84,0.45)');
      drawArrow(ctx, w * 0.2, h * 0.75, w * 0.8, h * 0.3, 'rgba(255,207,84,0.3)');
      ctx.fillStyle = 'rgba(244,240,228,0.78)';
      ctx.font = `${Math.max(12, Math.round(w / 60))}px "Pixelify Sans", monospace`; ctx.textAlign = 'center';
      ctx.fillText('PINCH · ACROSS · UP/DOWN · DIAGONAL · OUT & BACK', w / 2, h - 14);
    }

    const hand = active && step < 2 ? (step === 0 ? s.hands?.left : s.hands?.right) : null;
    const valid = active && (step < 2
      ? !!(hand?.visible && hand.pinching && hand.point)
      : !!(s.multi?.grab?.active && !s.grabbed && (s.brain?.screenAim || s.brain?.aim)));
    ctx.fillStyle = valid ? '#7ee08a' : 'rgba(163,166,196,0.7)';
    ctx.beginPath(); ctx.arc(10, 10, 3, 0, Math.PI * 2); ctx.fill();
    if (active) {
      const left = Math.max(0, (STEP_MS[step] - (now - stepStart)) / 1000);
      setStatus(`${TITLES[step]} · ${left.toFixed(1)}s`);
      if (step === 2 && !valid) setMsg('Pinch a hand, then turn/nod your head to capture the tilt.');
      else setMsg(HINTS[step]);
    } else if (now > flashUntil) {
      setStatus('idle');
    }
  }

  function update(now = performance.now()) {
    if (active) {
      const s = getState();
      const c = captures[STEP_IDS[step]];
      if (step < 2) {
        const hand = step === 0 ? s.hands?.left : s.hands?.right;
        if (hand?.visible && hand.pinching && hand.point) addCap(c, hand.point);
      } else if (s.multi?.grab?.active && !s.grabbed) {
        addCap(c, s.brain?.screenAim || s.brain?.aim);
      }
      if (now - stepStart >= STEP_MS[step]) next();
    }
    draw(now);
  }

  return { update, start: begin, next, reset, stop, get active() { return active; } };
}
