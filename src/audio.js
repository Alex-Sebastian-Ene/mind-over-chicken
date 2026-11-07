// Tiny WebAudio synth: every sound is generated, so there are no asset files to ship.

export class Audio {
  constructor() {
    this.ctx = null;
    this.enabled = true;
    this.master = null;
    this.lastBeat = 0;
  }

  // Must be called from a user gesture.
  unlock() {
    if (this.ctx) { this.ctx.resume?.(); return; }
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    this.ctx = new AC();
    this.master = this.ctx.createGain();
    this.master.gain.value = 0.5;
    this.master.connect(this.ctx.destination);
  }

  toggle() { this.enabled = !this.enabled; if (this.master) this.master.gain.value = this.enabled ? 0.5 : 0; return this.enabled; }

  noiseBuffer(seconds) {
    const len = Math.floor(this.ctx.sampleRate * seconds);
    const buf = this.ctx.createBuffer(1, len, this.ctx.sampleRate);
    const d = buf.getChannelData(0);
    for (let i = 0; i < len; i++) d[i] = Math.random() * 2 - 1;
    return buf;
  }

  noise({ dur = 0.5, freq = 800, q = 1, gain = 0.4, type = 'lowpass', sweepTo = null, attack = 0.005 }) {
    if (!this.ctx || !this.enabled) return;
    const t = this.ctx.currentTime;
    const src = this.ctx.createBufferSource();
    src.buffer = this.noiseBuffer(dur);
    const f = this.ctx.createBiquadFilter();
    f.type = type; f.frequency.setValueAtTime(freq, t); f.Q.value = q;
    if (sweepTo) f.frequency.exponentialRampToValueAtTime(sweepTo, t + dur);
    const g = this.ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(gain, t + attack);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    src.connect(f); f.connect(g); g.connect(this.master);
    src.start(t); src.stop(t + dur + 0.05);
  }

  tone({ freq = 440, dur = 0.15, type = 'square', gain = 0.12, slideTo = null, delay = 0 }) {
    if (!this.ctx || !this.enabled) return;
    const t = this.ctx.currentTime + delay;
    const o = this.ctx.createOscillator();
    o.type = type; o.frequency.setValueAtTime(freq, t);
    if (slideTo) o.frequency.exponentialRampToValueAtTime(slideTo, t + dur);
    const g = this.ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(gain, t + 0.01);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    o.connect(g); g.connect(this.master);
    o.start(t); o.stop(t + dur + 0.05);
  }

  thunder() { this.noise({ dur: 0.08, freq: 4000, type: 'highpass', gain: 0.5 }); this.noise({ dur: 1.4, freq: 600, sweepTo: 80, gain: 0.6, attack: 0.02 }); }
  ignite() { this.noise({ dur: 0.6, freq: 1200, sweepTo: 300, gain: 0.3, type: 'bandpass', q: 0.8 }); }
  blip() { this.tone({ freq: 660, dur: 0.06, gain: 0.06 }); }
  good() { [523, 659, 784].forEach((f, i) => this.tone({ freq: f, dur: 0.14, gain: 0.09, delay: i * 0.08 })); }
  bad() { this.tone({ freq: 220, dur: 0.35, type: 'sawtooth', gain: 0.08, slideTo: 110 }); }
  pop() { this.tone({ freq: 880, dur: 0.05, type: 'triangle', gain: 0.08, slideTo: 1320 }); }
  coin() { this.tone({ freq: 988, dur: 0.06, gain: 0.07 }); this.tone({ freq: 1319, dur: 0.12, gain: 0.07, delay: 0.06 }); }
  bell() { [880, 1320, 1760].forEach((f) => this.tone({ freq: f, dur: 1.2, type: 'sine', gain: 0.07 })); }
  crash() { this.noise({ dur: 0.9, freq: 300, sweepTo: 60, gain: 0.45 }); this.tone({ freq: 330, dur: 0.6, type: 'sawtooth', gain: 0.08, slideTo: 82 }); }
  tick() { this.tone({ freq: 1200, dur: 0.04, type: 'square', gain: 0.05 }); }
  rumble(dur = 3) { this.noise({ dur, freq: 120, sweepTo: 900, gain: 0.7, attack: 0.3 }); }
  fanfare() { [523, 659, 784, 1047, 784, 1047].forEach((f, i) => this.tone({ freq: f, dur: 0.22, type: 'triangle', gain: 0.1, delay: i * 0.14 })); }

  // Thump at the player's own heart rate when it is running hot.
  heartbeat(now, bpm, intensity) {
    if (!this.ctx || !this.enabled || !bpm || intensity <= 0) return;
    if (now - this.lastBeat < 60 / bpm) return;
    this.lastBeat = now;
    const g = Math.min(0.25, 0.05 + intensity * 0.2);
    this.tone({ freq: 70, dur: 0.12, type: 'sine', gain: g, slideTo: 40 });
    this.tone({ freq: 60, dur: 0.1, type: 'sine', gain: g * 0.7, slideTo: 35, delay: 0.16 });
  }
}
