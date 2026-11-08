// camera (#189): what actually moves the cursor (G.brain.aim). The webcam has no head-pose model:
// it reads hands and face only. The cursor follows the EEG headset's head pose (IMU) while that
// headset is streaming, and the mouse (or arrow keys) otherwise; with the emulated headset the
// emulated head tilt is the mouse. Camera panel, coach and HUD copy read this instead of claiming
// head tracking. Pure logic, no DOM, so Node can unit-test it.

export const HEADSET_STALE_MS = 3000;   // same window as main.js isEegLive()

export const AIM_SOURCES = {
  headset: { id: 'headset', label: 'EEG headset (head pose)', short: 'headset' },
  emulated: { id: 'emulated', label: 'mouse (emulated headset)', short: 'mouse' },
  mouse: { id: 'mouse', label: 'mouse (headset not streaming)', short: 'mouse' },
  none: { id: 'none', label: 'mouse', short: 'mouse' },
};

export function aimSource(G, now = (typeof performance !== 'undefined' ? performance.now() : 0)) {
  const src = G?.source;
  if (!src) return AIM_SOURCES.none;
  if (src.kind === 'bridge') {
    const live = src.connected && src.streaming && src.lastFrameAt > 0 && now - src.lastFrameAt < HEADSET_STALE_MS;
    return live ? AIM_SOURCES.headset : AIM_SOURCES.mouse;
  }
  if (src.kind === 'sim') return AIM_SOURCES.emulated;
  return AIM_SOURCES.none;
}
