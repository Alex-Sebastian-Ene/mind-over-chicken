// A manual mood (#155): one slider, Happy ↔ Mix ↔ Surprised, so the joy and surprise mechanics
// can be played without a webcam. 0 is all joy, 0.5 both at full, 1 all surprise. It produces the
// same mood shape the camera does (emotion.js neutralMood), marked face: false, source 'manual'.
// Surprise is a one-shot event: it fires when the slider enters the surprised side, not every frame.
import { neutralMood } from './emotion.js';

export const SURPRISE_EDGE = 0.5;   // Mix (0.5) already has surprise at full, so it fires there once

export function manualMood(value) {
  const v = Math.min(1, Math.max(0, Number(value) || 0));
  const joy = Math.min(1, 2 * (1 - v)), surprise = Math.min(1, 2 * v);
  // Joy is the continuous buff, so a mix keeps it dominant; surprise also fires as an event.
  const dominant = surprise > joy ? 'surprise' : 'joy';
  return { ...neutralMood(), joy, surprise, neutral: 0, dominant, intensity: Math.max(joy, surprise), source: 'manual' };
}

// Did this change of slider value cross into the surprised side (fire the surprise once)?
export function surpriseRisingEdge(prev, next) {
  return (prev == null || prev < SURPRISE_EDGE) && next >= SURPRISE_EDGE;
}
