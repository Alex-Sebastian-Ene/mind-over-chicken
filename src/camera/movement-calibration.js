// Pinch movement calibration (#282): a guided wireframe for left-hand pan, right-hand pan and
// pinch + head tilt. Captures the player's comfortable movement span and stores per-input gains.

import { buildGhostHand } from './coach.js';
import { drawHand } from './draw.js';

const KEY = 'moc.pinchCalibration';
const TARGET_SPAN = 0.3;
const STEP_MS = [5000, 5000, 6000];
const STEP_IDS = ['left', 'right', 'head'];

export function gainFromSpan(span, target = TARGET_SPAN) {
  const s = Number.isFinite(span) ? Math.max(0, span) : 0;
  return Math.max(0.5, Math.min(2.5, target / Math.max(0.01, s)));
}

export function defaultPinchCalibration() {
  return {
    left: { gain: 1, span: 0, calibrated: false },
    right: { gain: 1, span: 0, calibrated: false },
    head: { gain: 1, span: 0, calibrated: false },
  };
}

function normalise(raw) {
  const base = defaultPinchCalibration();
  if (!raw || typeof raw !== 'object') return base;
  for (const k of STEP_IDS) {
    const v = raw[k];
    if (!v || typeof v !== 'object') continue;
    const gain = Number(v.gain);
    base[k] = {
      gain: Number.isFinite(gain) ? Math.max(0.5, Math.min(2.5, gain)) : 1,
      span: Number.isFinite(v.span) ? Math.max(0, v.span) : 0,
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

const TITLES = ['LEFT HAND · pinch & sweep', 'RIGHT HAND · pinch & sweep', 'PINCH + TURN HEAD'];
const HINTS = [
  'Pinch your left hand and sweep it across the wireframe.',
  'Pinch your right hand and sweep it across the wireframe.',
  'Keep a hand pinched and turn your head left and right.',
];

const smooth = (t) => 0.5 - 0.5 * Math.cos(Math.max(0, Math.min(1, t)) * Math.PI);
const clamp01 = (v) => Math.max(0, Math.min(1, v));

function drawArrow(ctx, x0, x1, y, color) {
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.beginPath(); ctx.moveTo(x0, y); ctx.lineTo(x1, y); ctx.stroke();
  const dir = Math.sign(x1 - x0) || 1;
  ctx.beginPath(); ctx.moveTo(x1, y); ctx.lineTo(x1 - dir * 7, y - 4); ctx.lineTo(x1 - dir * 7, y + 4); ctx.closePath(); ctx.fillStyle = color; ctx.fill();
}

function drawHead(ctx, w, h, yaw) {
  const cx = w / 2, cy = h * 0.45, r = Math.min(w, h) * 0.23;
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
  for (const s of [-1, 1]) { ctx.beginPath(); ctx.arc(cx + s * r * 0.28 + ex, cy - r * 0.12, r * 0.07, 0, Math.PI * 2); ctx.fill(); }
  const nx = yaw * r * 0.45;
  ctx.strokeStyle = '#ffd166';
  ctx.beginPath();
  ctx.moveTo(cx + nx, cy - r * 0.02);
  ctx.lineTo(cx + nx - r * 0.12, cy + r * 0.36);
  ctx.lineTo(cx + nx + r * 0.14, cy + r * 0.36);
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
const spanOf = (c) => (c && c.n > 1 ? Math.max(c.maxX - c.minX, c.maxY - c.minY) : 0);

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
      setMsg('Start Camera first, then pinch and follow the wireframe.');
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
      const span = spanOf(captures[k]);
      const calibrated = captures[k].n >= 10 && span > 0.01;
      out[k] = { gain: calibrated ? gainFromSpan(span) : 1, span, calibrated };
    }
    active = false;
    if (skipBtn) skipBtn.disabled = true;
    if (startBtn) startBtn.textContent = 'Calibrate';
    if (onChange) onChange(out);
    setStatus(`calibrated · L×${out.left.gain.toFixed(2)} R×${out.right.gain.toFixed(2)} H×${out.head.gain.toFixed(2)}`);
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
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#0b0c16';
    ctx.fillRect(0, 0, w, h);

    const phase = active ? clamp01((now - stepStart) / STEP_MS[step]) : (now / 2600) % 1;
    if (active && step === 2) {
      drawHead(ctx, w, h, Math.sin(phase * Math.PI * 2) * 0.9);
      const g = buildGhostHand('pinch', 'pinch', 1, { cx: 0.5, cy: 0.9, size: 0.2 });
      drawHand(ctx, g, w, h, { pinching: true });
      drawArrow(ctx, w * 0.12, w * 0.35, h * 0.12, 'rgba(255,207,74,0.8)');
      drawArrow(ctx, w * 0.88, w * 0.65, h * 0.12, 'rgba(255,207,74,0.8)');
      ctx.fillStyle = 'rgba(244,240,228,0.75)';
      ctx.font = `${Math.max(12, Math.round(w / 60))}px "Pixelify Sans", monospace`; ctx.textAlign = 'center';
      ctx.fillText('TURN HEAD LEFT / RIGHT', w / 2, h - 14);
    } else {
      const dir = active && step === 1 ? -1 : 1;
      const p = smooth(phase);
      const cx = dir > 0 ? 0.18 + 0.64 * p : 0.82 - 0.64 * p;
      const g = buildGhostHand('pinch', 'pinch', 1, { cx, cy: 0.62, size: 0.34, mirrorX: dir < 0 });
      drawHand(ctx, g, w, h, { pinching: true, side: dir < 0 ? 'right' : 'left' });
      drawArrow(ctx, w * 0.15, w * 0.85, h * 0.16, 'rgba(255,207,74,0.65)');
      ctx.fillStyle = 'rgba(244,240,228,0.75)';
      ctx.font = `${Math.max(12, Math.round(w / 60))}px "Pixelify Sans", monospace`; ctx.textAlign = 'center';
      ctx.fillText(dir < 0 ? 'RIGHT HAND · PINCH & SWEEP' : 'LEFT HAND · PINCH & SWEEP', w / 2, h - 14);
    }

    const s = getState();
    const hand = active && step < 2 ? (step === 0 ? s.hands?.left : s.hands?.right) : null;
    const valid = active && (step < 2
      ? !!(hand?.visible && hand.pinching && hand.point)
      : !!(s.multi?.grab?.active && !s.grabbed && (s.brain?.screenAim || s.brain?.aim)));
    ctx.fillStyle = valid ? '#7ee08a' : 'rgba(163,166,196,0.7)';
    ctx.beginPath(); ctx.arc(10, 10, 3, 0, Math.PI * 2); ctx.fill();
    if (active) {
      const left = Math.max(0, (STEP_MS[step] - (now - stepStart)) / 1000);
      setStatus(`${TITLES[step]} · ${left.toFixed(1)}s`);
      if (step === 2 && !valid) setMsg('Pinch a hand, then turn your head to capture the tilt.');
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
