// camera (#33, #67): draws a tracked hand on a 2D canvas. Shared by the camera panel and the debug tab.

export const HAND_COLORS = { left: '#6fb8ff', right: '#ffb15c' };
export const HAND_LINKS = [[0, 1], [1, 2], [2, 3], [3, 4], [0, 5], [5, 6], [6, 7], [7, 8], [5, 9], [9, 10], [10, 11], [11, 12],
  [9, 13], [13, 14], [14, 15], [15, 16], [13, 17], [17, 18], [18, 19], [19, 20], [0, 17]];

// hand: 21 landmarks in frame coordinates (0..1). The canvas is w × h; mirror the context first if
// the picture is mirrored. state: { held, aiming, pinching, nearPinch, side }.
export function drawHand(ctx, hand, w, h, state = {}) {
  if (!hand || hand.length < 21) return;
  const lw = Math.max(1.5, w / 160);
  ctx.lineWidth = lw;
  ctx.strokeStyle = state.held ? '#ffcf4a' : HAND_COLORS[state.side] || HAND_COLORS.left;
  ctx.beginPath();
  for (const [a, b] of HAND_LINKS) { ctx.moveTo(hand[a].x * w, hand[a].y * h); ctx.lineTo(hand[b].x * w, hand[b].y * h); }
  ctx.stroke();
  ctx.fillStyle = '#f2e8d5';
  for (const p of hand) ctx.fillRect(p.x * w - lw, p.y * h - lw, lw * 2, lw * 2);
  // Thumb tip to index tip: the pinch.
  if (state.pinching || state.nearPinch) {
    ctx.strokeStyle = state.pinching ? '#ff8a3d' : 'rgba(255,138,61,0.5)';
    ctx.lineWidth = lw * (state.pinching ? 2 : 1);
    ctx.beginPath(); ctx.moveTo(hand[4].x * w, hand[4].y * h); ctx.lineTo(hand[8].x * w, hand[8].y * h); ctx.stroke();
  }
  // The index fingertip.
  ctx.lineWidth = lw;
  ctx.strokeStyle = state.aiming || state.pinching ? '#ff5a72' : 'rgba(255,90,114,0.6)';
  ctx.beginPath(); ctx.arc(hand[8].x * w, hand[8].y * h, lw * 3.5, 0, Math.PI * 2); ctx.stroke();
}
