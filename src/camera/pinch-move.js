// One-hand pinch movement (#270): hand translation pans the map, head-cursor movement tilts
// the 3D camera. Both can be active at once; small deadzones keep hand tremor and head jitter
// from fighting the other input.
//
// #280: the raw per-frame hand and aim deltas are low-passed before arbitration, because
// MediaPipe landmarks and inference frames jitter. The time constant is short enough that
// deliberate movement still feels immediate.

export const PINCH_HAND_DEADZONE = 0.001;
export const PINCH_HEAD_DEADZONE = 0.004;
export const PINCH_MOVE_TAU = 0.07;   // s, exponential low-pass time constant

export function lowPass(prev, next, dt, tau = PINCH_MOVE_TAU) {
  if (!Number.isFinite(prev)) return next;
  const a = 1 - Math.exp(-Math.max(0, dt) / Math.max(0.001, tau));
  return prev + (next - prev) * a;
}

export class PinchMoveSmoother {
  constructor(tau = PINCH_MOVE_TAU) {
    this.tau = tau;
    this.reset();
  }

  reset() {
    this.hand = { x: 0, y: 0 };
    this.head = { x: 0, y: 0 };
    this.primed = false;
  }

  filter({ handDx = 0, handDy = 0, headDx = 0, headDy = 0 } = {}, dt = 1 / 30) {
    if (!this.primed) {
      this.primed = true;
      this.hand = { x: handDx, y: handDy };
      this.head = { x: headDx, y: headDy };
      return { handDx, handDy, headDx, headDy };
    }
    this.hand.x = lowPass(this.hand.x, handDx, dt, this.tau);
    this.hand.y = lowPass(this.hand.y, handDy, dt, this.tau);
    this.head.x = lowPass(this.head.x, headDx, dt, this.tau);
    this.head.y = lowPass(this.head.y, headDy, dt, this.tau);
    return { handDx: this.hand.x, handDy: this.hand.y, headDx: this.head.x, headDy: this.head.y };
  }
}

export function pinchMove({ handDx = 0, handDy = 0, headDx = 0, headDy = 0 } = {}) {
  const pan = Math.abs(handDx) > PINCH_HAND_DEADZONE || Math.abs(handDy) > PINCH_HAND_DEADZONE;
  const tilt = Math.abs(headDx) > PINCH_HEAD_DEADZONE || Math.abs(headDy) > PINCH_HEAD_DEADZONE;
  return {
    panX: pan ? handDx : 0,
    panY: pan ? handDy : 0,
    tiltX: tilt ? headDx : 0,
    tiltY: tilt ? headDy : 0,
  };
}
