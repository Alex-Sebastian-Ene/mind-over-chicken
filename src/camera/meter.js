// camera debug (#67): rate and timing meters for the debug tab. Pure logic, no DOM.

// Events per second over a sliding window. Reads 0 once nothing has happened for a window.
export class RateMeter {
  constructor(windowMs = 1000) { this.windowMs = windowMs; this.times = []; this.count = 0; }

  tick(now) {
    this.times.push(now);
    this.count++;
    this.prune(now);
  }

  prune(now) { while (this.times.length && now - this.times[0] > this.windowMs) this.times.shift(); }

  hz(now) {
    this.prune(now);
    const n = this.times.length;
    if (n < 2) return 0;
    const span = this.times[n - 1] - this.times[0];
    return span > 0 ? ((n - 1) * 1000) / span : 0;
  }

  reset() { this.times.length = 0; this.count = 0; }
}

// Exponential moving average; the first sample sets it.
export class Ema {
  constructor(k = 0.2) { this.k = k; this.value = null; }
  add(x) { if (Number.isFinite(x)) this.value = this.value == null ? x : this.value + (x - this.value) * this.k; return this.value; }
  reset() { this.value = null; }
}

// Frame pacing of the page (rAF intervals): fps and long frames.
export class FrameMeter {
  constructor({ windowMs = 2000, longMs = 50 } = {}) { this.rate = new RateMeter(windowMs); this.longMs = longMs; this.last = null; this.long = 0; this.worst = 0; }
  frame(now) {
    if (this.last != null) {
      const dt = now - this.last;
      if (dt > this.longMs && dt < 1000) { this.long++; this.worst = Math.max(this.worst, dt); }  // >1 s is a hidden tab, not a dropped frame
    }
    this.last = now;
    this.rate.tick(now);
  }
  fps(now) { return this.rate.hz(now); }
  reset() { this.rate.reset(); this.last = null; this.long = 0; this.worst = 0; }
}
