// Deterministic hand-shape classification from 21 MediaPipe landmarks (#327).
// The game only needs pinch, fist and palm; this replaces the seven-gesture recognizer.

const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y, (a.z || 0) - (b.z || 0));

export function classifyHand(landmarks) {
  if (!landmarks || landmarks.length < 21) return { name: 'None', score: 0 };
  const wrist = landmarks[0];
  const palm = Math.max(dist(wrist, landmarks[9]), 1e-6);
  const pinchRatio = dist(landmarks[4], landmarks[8]) / palm;

  const mcp = [5, 9, 13, 17];
  const tip = [8, 12, 16, 20];
  let curled = 0, extended = 0;
  for (let i = 0; i < 4; i++) {
    const tipDist = dist(wrist, landmarks[tip[i]]);
    const mcpDist = dist(wrist, landmarks[mcp[i]]);
    if (tipDist < mcpDist * 1.02) curled++;
    else if (tipDist > mcpDist * 1.28) extended++;
  }
  const indexExtended = dist(wrist, landmarks[8]) > dist(wrist, landmarks[5]) * 1.1;

  if (pinchRatio < 0.32 && indexExtended) {
    return { name: 'Pinch', score: Math.max(0.6, Math.min(1, 1 - pinchRatio / 0.4)) };
  }
  if (curled >= 3) return { name: 'Closed_Fist', score: 0.9 };
  if (extended >= 3) return { name: 'Open_Palm', score: 0.9 };
  return { name: 'None', score: 0 };
}
