// Pinch movement calibration (#282, #286, #288): live camera + wireframe, four cardinal targets
// per hand. The goal is to learn the player's movement span and align it to left/right/up/down.
// Head tilt is deliberately not calibrated here — it already works.

import { buildGhostHand } from './coach.js';
import { drawHand } from './draw.js';

const KEY = 'moc.pinchCalibration';
const TARGET_SPAN = 0.3;
const HANDS = ['left', 'right'];
const TARGETS = [
  { id: 'left', label: 'MOVE LEFT', x: 0.22, y: 0.50, arrow: [-1, 0] },
  { id: 'right', label: 'MOVE RIGHT', x: 0.78, y: 0.50, arrow: [1, 0] },
  { id: 'up', label: 'MOVE UP', x: 0.50, y: 0.30, arrow: [0, -1] },
  { id: 'down', label: 'MOVE DOWN', x: 0.50, y: 0.70, arrow: [0, 1] },
];
const REACH = 0.14;          // normalized canvas distance that counts as reaching the target
const HOLD_MS = 350;         // must stay there this long
const TIMEOUT_MS = 6000;     // skip a direction rather than trapping the player

export function gainFromSpan(span, target = TARGET_SPAN) {
  const s = Number.isFinite(span) ? Math.max(0, span) : 0;
  return Math.max(0.5, Math.min(2.5, target / Math.max(0.01, s)));
}

export function defaultPinchCalibration() {
  return {
    left: { gainX: 1, gainY: 1, spanX: 0, spanY: 0, calibrated: false },
    right: { gainX: 1, gainY: 1, spanX: 0, spanY: 0, calibrated: false },
  };
}

function normalise(raw) {
  const base = defaultPinchCalibration();
  if (!raw || typeof raw !== 'object') return base;
  for (const k of HANDS) {
    const v = raw[k];
    if (!v || typeof v !== 'object') continue;
    const legacy = Number(v.gain);
    const gx = Number(v.gainX ?? legacy), gy = Number(v.gainY ?? legacy);
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
  try { return normalise(JSON.parse(localStorage.getItem(KEY) || 'null')); }
  catch { return defaultPinchCalibration(); }
}

export function savePinchCalibration(cal) {
  try { localStorage.setItem(KEY, JSON.stringify(normalise(cal))); } catch {}
}

const clamp01 = (v) => Math.max(0, Math.min(1, v));

function arrow(ctx, x0, y0, x1, y1, color) {
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); ctx.stroke();
  const a = Math.atan2(y1 - y0, x1 - x0), d = 10;
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x1 - Math.cos(a - 0.5) * d, y1 - Math.sin(a - 0.5) * d);
  ctx.lineTo(x1 - Math.cos(a + 0.5) * d, y1 - Math.sin(a + 0.5) * d);
  ctx.closePath(); ctx.fillStyle = color; ctx.fill();
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

// Contain (not cover) so the whole camera frame is visible and all four targets are reachable.
function frameRect(video, w, h) {
  const vw = video?.videoWidth || 0, vh = video?.videoHeight || 0;
  if (!vw || !vh) return null;
  const k = Math.min(w / vw, h / vh), dw = vw * k, dh = vh * k;
  return { ox: (w - dw) / 2, oy: (h - dh) / 2, dw, dh };
}

function mapPoint(p, rect, w, h) {
  return { x: (rect.ox + p.x * rect.dw) / w, y: (rect.oy + p.y * rect.dh) / h };
}

function mapHand(hand, rect, w, h) {
  return hand.map((p) => mapPoint(p, rect, w, h));
}

export function initMovementCalibration({
  canvas, statusEl, msgEl, startBtn, skipBtn, resetBtn,
  getState, onChange, onComplete,
}) {
  if (!canvas) return null;
  const ctx = canvas.getContext('2d');
  let active = false, handIdx = 0, targetIdx = 0, targetStart = 0, holdMs = 0;
  let captures = { left: emptyCap(), right: emptyCap() };
  let hits = { left: 0, right: 0 };
  let complete = false, flashUntil = 0, lastUpdate = 0;

  const setStatus = (text) => { if (statusEl && statusEl.textContent !== text) statusEl.textContent = text; };
  const setMsg = (text) => { if (msgEl && msgEl.textContent !== text) msgEl.textContent = text; };
  const currentHand = () => HANDS[handIdx];
  const currentTarget = () => TARGETS[Math.min(targetIdx, TARGETS.length - 1)];

  function begin() {
    const s = getState();
    if (!s.live) {
      setStatus('camera off');
      setMsg('Start Camera first, then pinch and reach the four targets.');
      flashUntil = performance.now() + 3000;
      return;
    }
    active = true; complete = false; handIdx = 0; targetIdx = 0;
    targetStart = performance.now(); holdMs = 0;
    captures = { left: emptyCap(), right: emptyCap() };
    hits = { left: 0, right: 0 };
    setStatus('LEFT HAND · MOVE LEFT');
    setMsg('Pinch and move your hand to the target.');
    if (skipBtn) skipBtn.disabled = false;
    if (startBtn) startBtn.textContent = 'Restart';
  }

  function stop() {
    active = false;
    if (skipBtn) skipBtn.disabled = true;
    if (startBtn) startBtn.textContent = 'Calibrate';
    setStatus(complete ? 'complete' : 'idle');
  }

  function nextTarget(now, reached) {
    if (reached) hits[currentHand()]++;
    targetIdx++;
    holdMs = 0;
    targetStart = now;
    if (targetIdx >= TARGETS.length) {
      targetIdx = 0;
      handIdx++;
      if (handIdx >= HANDS.length) { finish(); return; }
    }
    const h = currentHand() === 'left' ? 'LEFT' : 'RIGHT';
    setStatus(`${h} HAND · ${currentTarget().label}`);
    setMsg(reached ? 'Good. Next target.' : 'Skipped — keep the hand pinched and move.');
  }

  function finish() {
    const out = defaultPinchCalibration();
    for (const k of HANDS) {
      const sx = spanX(captures[k]), sy = spanY(captures[k]);
      out[k] = {
        gainX: sx > 0.01 ? gainFromSpan(sx) : 1,
        gainY: sy > 0.01 ? gainFromSpan(sy) : 1,
        spanX: sx,
        spanY: sy,
        calibrated: hits[k] >= 3 && sx > 0.01 && sy > 0.01,
      };
    }
    active = false; complete = true;
    if (skipBtn) skipBtn.disabled = true;
    if (startBtn) startBtn.textContent = 'Calibrate';
    if (onChange) onChange(out);
    setStatus(`Calibration complete · L ${out.left.gainX.toFixed(1)}/${out.left.gainY.toFixed(1)} · R ${out.right.gainX.toFixed(1)}/${out.right.gainY.toFixed(1)}`);
    setMsg('Saved. Closing…');
    flashUntil = performance.now() + 2500;
    onComplete?.(out);
  }

  function reset() {
    active = false; complete = false;
    captures = { left: emptyCap(), right: emptyCap() };
    hits = { left: 0, right: 0 };
    if (skipBtn) skipBtn.disabled = true;
    if (startBtn) startBtn.textContent = 'Calibrate';
    if (onChange) onChange(defaultPinchCalibration());
    setStatus('reset'); setMsg('Pinch movement is back to 1×. Calibrate to tune it again.');
    flashUntil = performance.now() + 3000;
  }

  startBtn?.addEventListener('click', begin);
  skipBtn?.addEventListener('click', () => { if (active) nextTarget(performance.now(), false); });
  resetBtn?.addEventListener('click', reset);
  if (skipBtn) skipBtn.disabled = true;

  function draw(now) {
    const w = canvas.width, h = canvas.height;
    const s = getState();
    const video = s.video;
    const rect = video && video.readyState >= 2 ? frameRect(video, w, h) : null;
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#0b0c16';
    ctx.fillRect(0, 0, w, h);

    if (rect) {
      ctx.save();
      ctx.translate(w, 0); ctx.scale(-1, 1);
      ctx.drawImage(video, rect.ox, rect.oy, rect.dw, rect.dh);
      for (const side of HANDS) {
        const hand = s.hands?.[side];
        if (!hand?.visible || !hand.landmarks) continue;
        ctx.globalAlpha = (active && side !== currentHand()) ? 0.25 : 1;
        drawHand(ctx, mapHand(hand.landmarks, rect, w, h), w, h, { side, pinching: hand.pinching });
      }
      ctx.globalAlpha = 1;
      ctx.restore();
    } else {
      for (const side of HANDS) {
        const hand = s.hands?.[side];
        if (hand?.visible && hand.landmarks) {
          ctx.globalAlpha = (active && side !== currentHand()) ? 0.25 : 1;
          drawHand(ctx, hand.landmarks, w, h, { side, pinching: hand.pinching });
        }
      }
      ctx.globalAlpha = 1;
    }

    if (active) {
      const t = currentTarget();
      const tx = t.x * w, ty = t.y * h;
      arrow(ctx, w * 0.5, h * 0.5, tx, ty, 'rgba(255,207,84,0.45)');
      ctx.globalAlpha = 0.72;
      const g = buildGhostHand('pinch', 'pinch', 1, { cx: t.x, cy: t.y, size: 0.22, mirrorX: currentHand() === 'right' });
      drawHand(ctx, g, w, h, { pinching: true, held: true, side: currentHand() });
      ctx.globalAlpha = 1;
      ctx.strokeStyle = holdMs >= HOLD_MS ? '#7ee08a' : 'rgba(255,207,74,0.5)';
      ctx.lineWidth = 2;
      ctx.beginPath(); ctx.arc(tx, ty, 20, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * Math.min(1, holdMs / HOLD_MS)); ctx.stroke();
      ctx.fillStyle = '#f4f0e4';
      ctx.font = `${Math.max(14, Math.round(w / 50))}px "Pixelify Sans", monospace`;
      ctx.textAlign = 'center';
      ctx.fillText(`${currentHand().toUpperCase()} HAND · ${t.label}`, w / 2, h * 0.1);
      // progress dots
      for (let i = 0; i < TARGETS.length; i++) {
        ctx.beginPath(); ctx.arc(w * 0.5 + (i - 1.5) * 20, h * 0.16, 5, 0, Math.PI * 2);
        ctx.fillStyle = i < targetIdx ? '#7ee08a' : i === targetIdx ? '#ffcf4a' : 'rgba(163,166,196,0.4)';
        ctx.fill();
      }
    } else if (complete) {
      ctx.fillStyle = '#7ee08a';
      ctx.font = `700 ${Math.max(22, Math.round(w / 28))}px "Pixelify Sans", monospace`;
      ctx.textAlign = 'center';
      ctx.fillText('CALIBRATION COMPLETE', w / 2, h * 0.5);
    } else {
      ctx.globalAlpha = 0.5;
      const g = buildGhostHand('pinch', 'pinch', 1, { cx: 0.5, cy: 0.5, size: 0.2 });
      drawHand(ctx, g, w, h, { pinching: true, held: true });
      ctx.globalAlpha = 1;
      ctx.fillStyle = 'rgba(244,240,228,0.7)';
      ctx.font = `${Math.max(14, Math.round(w / 50))}px "Pixelify Sans", monospace`;
      ctx.textAlign = 'center';
      ctx.fillText('PRESS CALIBRATE', w / 2, h * 0.72);
    }

    const hand = active ? s.hands?.[currentHand()] : null;
    const valid = active && !!(hand?.visible && hand.pinching && hand.point);
    ctx.fillStyle = valid ? '#7ee08a' : 'rgba(163,166,196,0.7)';
    ctx.beginPath(); ctx.arc(12, 12, 4, 0, Math.PI * 2); ctx.fill();
    if (active) {
      if (!hand?.visible) setMsg(`Show your ${currentHand()} hand to the camera.`);
      else if (!hand.pinching) setMsg('Pinch to enable the hand target.');
      else if (holdMs > 0) setMsg('Hold at the target…');
      else setMsg('Move your pinched hand to the highlighted target.');
    } else if (complete) {
      if (now > flashUntil) stop();
    } else if (now > flashUntil) {
      setStatus('idle');
      setMsg('Press Calibrate, then reach left, right, up and down with each hand.');
    }
  }

  function update(now = performance.now()) {
    const dt = lastUpdate ? Math.min(0.1, (now - lastUpdate) / 1000) : 0.016;
    lastUpdate = now;
    if (active) {
      const s = getState();
      const hand = s.hands?.[currentHand()];
      const rect = s.video && s.video.readyState >= 2 ? frameRect(s.video, canvas.width, canvas.height) : null;
      let hit = false;
      if (hand?.visible && hand.pinching && hand.point) {
        const p = rect ? mapPoint(hand.point, rect, canvas.width, canvas.height) : hand.point;
        addCap(captures[currentHand()], p);
        const t = currentTarget();
        hit = Math.hypot(p.x - t.x, p.y - t.y) <= REACH;
      }
      holdMs = hit ? holdMs + dt * 1000 : 0;
      if (holdMs >= HOLD_MS) nextTarget(now, true);
      else if (now - targetStart >= TIMEOUT_MS) nextTarget(now, false);
    }
    draw(now);
  }

  return { update, start: begin, next: () => nextTarget(performance.now(), false), reset, stop, get active() { return active; } };
}
