// Skyfall's on-canvas HUD (#160): the telegraph, live focus vs threshold, hold progress and the
// explicit input source (EEG or the keyboard fallback). Scenes call this from drawScreenOverlay so
// it stays fixed to the screen while the map pans underneath.

import { W, H, clamp } from '../sim/util.js';

const FAIL_LINE = {
  stone: 'people, food and morale lost',
  industry: 'market panic: price and confidence fall',
  space: 'launch setback: parts and courage lost',
};

function pct(v) {
  return `${Math.round(clamp(Number(v) || 0) * 100)}%`;
}

function seconds(v) {
  return `${Math.max(0, Math.ceil(Number(v) || 0))}s`;
}

function line(ctx, x, y, text, color, size = 5.5, align = 'center') {
  ctx.font = `${size}px "Pixelify Sans", monospace`;
  ctx.textAlign = align;
  ctx.fillStyle = 'rgba(8,10,24,0.9)';
  ctx.fillText(text, x + 0.6, y + 0.6);
  ctx.fillStyle = color;
  ctx.fillText(text, x, y);
}

export function drawSkyfallHud(ctx, sky, { y = 80 } = {}) {
  if (!sky || sky.phase === 'idle') return;
  ctx.save();

  if (sky.phase === 'active') {
    const urgency = 1 - clamp(sky.timeLeft / Math.max(1, sky.countdown));
    // The sky darkens/reddens as impact nears; the meteor falls along a straight, deterministic
    // path. It is only a visual, so a frame drops or a pause cannot change the outcome.
    ctx.fillStyle = `rgba(54,12,8,${0.05 + urgency * 0.11})`;
    ctx.fillRect(0, 0, W, H);
    if (urgency > 0.03) {
      const v = sky.visuals || {};
      const x = W * 0.88 - W * 0.30 * urgency;
      const yy = -10 + H * 0.42 * urgency;
      ctx.strokeStyle = v.trail || '#ffd166';
      ctx.lineWidth = 1.6;
      ctx.globalAlpha = 0.35 + urgency * 0.55;
      for (let i = 1; i <= 4; i++) {
        ctx.beginPath();
        ctx.moveTo(x + i * 7, yy - i * 8);
        ctx.lineTo(x + i * 9 + 4, yy - i * 9 - 5);
        ctx.stroke();
      }
      ctx.fillStyle = v.meteor || '#ff9a2e';
      ctx.fillRect(Math.round(x - 3), Math.round(yy - 3), 6, 6);
      ctx.fillStyle = v.impact || '#e8452c';
      ctx.fillRect(Math.round(x - 1), Math.round(yy - 1), 2, 2);
      ctx.globalAlpha = 1;
    }
  } else if (sky.phase === 'averted') {
    ctx.fillStyle = `rgba(120,210,255,${0.06 + 0.06 * clamp(sky.outcomeLeft / 5)})`;
    ctx.fillRect(0, 0, W, H);
  } else {
    ctx.fillStyle = `rgba(160,20,30,${0.08 + 0.08 * clamp(sky.outcomeLeft / 5)})`;
    ctx.fillRect(0, 0, W, H);
  }

  const x0 = W / 2 - 108;
  const w = 216;
  const h = 46;
  ctx.fillStyle = 'rgba(10,12,26,0.88)';
  ctx.fillRect(x0, y, w, h);
  const tone = sky.phase === 'averted' ? '#59d8c8' : sky.phase === 'failed' ? '#ff5a72' : '#ffcf4a';
  ctx.strokeStyle = tone;
  ctx.globalAlpha = 0.55;
  ctx.lineWidth = 0.6;
  ctx.strokeRect(x0 + 0.5, y + 0.5, w - 1, h - 1);
  ctx.globalAlpha = 1;

  const cx = W / 2;
  if (sky.phase === 'active') {
    const ok = sky.focus >= sky.threshold;
    const keyboard = sky.source === 'keyboard';
    const source = sky.sourceLabel || (keyboard ? 'Keyboard F' : sky.source === 'eeg' ? 'EEG focus' : 'No focus input');
    line(ctx, cx, y + 8, `SKYFALL · ${String(sky.threat).toUpperCase()}`, '#ffcf4a', 6);
    line(ctx, cx, y + 17, `IMPACT IN ${seconds(sky.timeLeft)}  ·  HOLD FOCUS`, ok ? '#59d8c8' : '#ff8a9c', 5.5);
    line(ctx, cx, y + 26, `FOCUS ${pct(sky.focus)}  ·  THRESHOLD ${pct(sky.threshold)}  ·  ${source.toUpperCase()}`, ok ? '#ffcf4a' : '#ff8a9c', 5);
    // Hold bar: threshold is marked by the notch, shield charge by the fill.
    const bx = x0 + 8, bw = w - 16, by = y + 31;
    ctx.fillStyle = 'rgba(255,255,255,0.14)';
    ctx.fillRect(bx, by, bw, 4);
    ctx.fillStyle = keyboard ? '#ffb84a' : '#59c9ff';
    ctx.fillRect(bx, by, bw * clamp(sky.progress), 4);
    const thr = bx + bw * clamp(sky.threshold);
    ctx.fillStyle = 'rgba(255,255,255,0.65)';
    ctx.fillRect(Math.round(thr), by - 1, 0.7, 6);
    line(ctx, cx, y + 42, `SHIELD ${pct(sky.progress)}  ·  ${ok ? 'CHARGING' : 'HOLD CONTROLS THE SHIELD'}`, ok ? '#9be38a' : '#ff8a9c', 5);
  } else if (sky.phase === 'averted') {
    line(ctx, cx, y + 11, 'SKYFALL AVERTED', '#9be38a', 7);
    line(ctx, cx, y + 22, 'the shield holds · the sky clears', '#d7f5e0', 5.5);
    line(ctx, cx, y + 33, 'the tribe celebrates; the advisor explains why', '#9be38a', 5);
    line(ctx, cx, y + 42, 'the event rests before it can return', '#f2e8d5', 5);
  } else {
    line(ctx, cx, y + 11, 'SKYFALL STRUCK', '#ff5a72', 7);
    line(ctx, cx, y + 22, FAIL_LINE[sky.era] || 'damage taken', '#ffd166', 5.5);
    line(ctx, cx, y + 33, 'no soft-lock: the era continues', '#d7f5e0', 5);
    line(ctx, cx, y + 42, 'the advisor names the cause', '#f2e8d5', 5);
  }
  ctx.restore();
}
