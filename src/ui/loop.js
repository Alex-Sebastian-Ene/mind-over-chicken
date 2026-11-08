// The game's requestAnimationFrame driver (#171). The next frame is booked before this one runs,
// so a throw inside one frame (a scene bug, a run torn down mid-frame) is reported once as an
// uncaught error and the loop carries on, instead of silently dropping to the 33 ms timer for
// the rest of the session.
export function frameLoop(frame, { raf = (cb) => requestAnimationFrame(cb), running = () => true } = {}) {
  function tick() {
    if (!running()) return;
    raf(tick);
    frame();
  }
  return tick;
}
