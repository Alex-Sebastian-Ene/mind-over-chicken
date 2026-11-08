// Era intro transition (Alex: "remove kind of the intro and use a transition as a delay to act
// over it"). Each era opens with a short camera move onto its stage, the era title and one-line
// goal fading over it as a caption, and the player's inputs held until it ends. No card to
// dismiss: Space, Enter or a click skips it. The simulation stays paused for the whole move
// (deterministic: the era starts in exactly the state enter() left it).
//
// Pure logic (no DOM): main.js drives it from the frame loop and asks it for the camera blend,
// the caption's opacity and whether an input may pass. Tested in Node (tests/intro.test.mjs).

export const INTRO = {
  seconds: 3.2,          // the whole move
  captionIn: 0.35,       // caption fades in over this long
  captionOut: 0.6,       // and out over the last this long
  skipKeys: ['Space', 'Enter', 'NumpadEnter'],
};

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
// Ease out (fast start, gentle landing), like a camera settling onto the stage.
export const easeOutCubic = (t) => 1 - (1 - clamp01(t)) ** 3;

export class EraIntro {
  constructor(opts = {}) {
    this.seconds = opts.seconds ?? INTRO.seconds;
    this.t = 0;
    this.active = false;
    this.caption = null;   // { eyebrow, title, goal }
    this.onEnd = null;
  }

  // Begin the move for an era. caption: { eyebrow, title, goal }. onEnd runs once, when the
  // move finishes or is skipped.
  start(caption, onEnd = null) {
    this.caption = caption || null;
    this.t = 0;
    this.active = true;
    this.onEnd = onEnd;
    return this;
  }

  // Advance by dt seconds (real time). Returns true while the intro is still running.
  update(dt) {
    if (!this.active) return false;
    this.t += Math.max(0, dt);
    if (this.t >= this.seconds) this.finish();
    return this.active;
  }

  skip() { if (this.active) this.finish(); }

  // Drop it without running onEnd (Quit to Title, a new game).
  cancel() { this.active = false; this.onEnd = null; this.caption = null; }

  finish() {
    if (!this.active) return;
    this.active = false;
    this.t = this.seconds;
    const fn = this.onEnd;
    this.onEnd = null;
    fn?.();
  }

  // 0 at the start of the move, 1 when the camera has landed (eased).
  progress() { return this.active ? easeOutCubic(this.t / this.seconds) : 1; }

  // How far the camera still is from its play pose: 1 at the start, 0 when landed.
  away() { return 1 - this.progress(); }

  // Caption opacity: fades in, holds, fades out before the move ends.
  captionAlpha() {
    if (!this.active || !this.caption) return 0;
    const fin = clamp01(this.t / INTRO.captionIn);
    const fout = clamp01((this.seconds - this.t) / INTRO.captionOut);
    return Math.min(fin, fout);
  }

  // Input hold. kind: 'key' (with the KeyboardEvent code), 'pointer', 'wheel', 'blink', 'cast'.
  // While the intro runs nothing reaches the game; skip inputs (Space, Enter, a click or tap)
  // end it instead. Returns 'pass' (not holding), 'skip' (end the intro, swallow the input) or
  // 'hold' (swallow it).
  gate(kind, code = '') {
    if (!this.active) return 'pass';
    if (kind === 'pointer') return 'skip';
    if (kind === 'key' && INTRO.skipKeys.includes(code)) return 'skip';
    return 'hold';
  }
}
