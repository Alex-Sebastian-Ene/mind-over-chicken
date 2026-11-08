// Hand moves are opt-in. The real setup is the EEG headset for aim, calm and focus plus the webcam
// for the face's mood, so the camera starts with hand tracking off: no hand model runs, which also
// leaves the face tracker more time. Ticking "Hand moves" in the Camera panel turns pinch-grab,
// hand-pan and two-hand zoom on, and the choice is remembered in this browser.
export const HANDS_KEY = 'moc.camera.hands';

export function handsEnabled(store = globalThis.localStorage) {
  try { return store?.getItem(HANDS_KEY) === '1'; } catch { return false; }
}

export function setHandsEnabled(on, store = globalThis.localStorage) {
  try { store?.setItem(HANDS_KEY, on ? '1' : '0'); } catch {}
  return !!on;
}
