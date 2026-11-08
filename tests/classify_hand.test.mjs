// Hand Landmarker → pinch/fist/palm classification (#327).
import test from 'node:test';
import assert from 'node:assert/strict';
import { classifyHand } from '../src/camera/classify-hand.js';

const hand = (pose) => {
  const p = Array.from({ length: 21 }, () => ({ x: 0.5, y: 0.65, z: 0 }));
  p[0] = { x: 0.5, y: 0.82, z: 0 };          // wrist
  p[9] = { x: 0.5, y: 0.62, z: 0 };          // middle MCP (palm size)
  const mcp = [5, 9, 13, 17];
  const tip = [8, 12, 16, 20];
  for (const i of mcp) p[i] = { x: 0.5 + (i - 9) * 0.01, y: 0.62, z: 0 };
  for (let i = 0; i < 4; i++) {
    p[tip[i]] = pose === 'fist'
      ? { x: p[mcp[i]].x, y: 0.70, z: 0 }
      : { x: p[mcp[i]].x, y: 0.42, z: 0 };
  }
  p[4] = pose === 'pinch' ? { x: 0.49, y: 0.55, z: 0 } : { x: 0.35, y: 0.62, z: 0 };
  if (pose === 'pinch') p[8] = { x: 0.50, y: 0.54, z: 0 };
  return p;
};

test('classifies pinch, fist, palm and uncertain hands', () => {
  assert.equal(classifyHand(hand('pinch')).name, 'Pinch');
  assert.equal(classifyHand(hand('fist')).name, 'Closed_Fist');
  assert.equal(classifyHand(hand('palm')).name, 'Open_Palm');
  const odd = hand('palm');
  odd[8].y = 0.58; odd[12].y = 0.58;
  assert.equal(classifyHand(odd).name, 'None');
  assert.equal(classifyHand(null).name, 'None');
});
