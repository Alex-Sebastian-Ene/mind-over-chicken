// One-hand pinch movement (#270): hand translation pans the map, head-cursor movement tilts
// the 3D camera. Both can be active at once; small deadzones keep hand tremor and head jitter
// from fighting the other input.

export const PINCH_HAND_DEADZONE = 0.001;
export const PINCH_HEAD_DEADZONE = 0.004;

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
