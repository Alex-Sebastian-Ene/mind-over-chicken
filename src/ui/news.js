// One news ticker for the one game (epic #220): a tape along the bottom of the map that every
// part of the game feeds. Industry's goat-market headlines and the rival peoples' news (#212)
// share it, instead of each drawing a ticker of its own. DOM-free apart from draw(ctx).
//
// Items scroll right to left, the newest last; repeats within `dedupe` seconds are dropped, and
// news older than `keep` seconds leaves the tape (a quiet tape hides).

export const TONE_COLOUR = { info: '#e8e4d8', good: '#8fe3a1', bad: '#ff8a7a', warn: '#ffcf4a', market: '#ffcf4a' };
const TONE_NAMES = ['info', 'good', 'bad', 'warn'];   // news::TONE_* order (civ.js TONE)

export class News {
  constructor({ max = 6, keep = 90, dedupe = 20, speed = 40 } = {}) {
    this.items = [];
    this.max = max; this.keep = keep; this.dedupe = dedupe; this.speed = speed;
    this.t = 0;
    this.x = null;   // the tape's left edge on screen; null until first drawn
  }

  // Add a line. tone: 'info' | 'good' | 'bad' | 'warn' | 'market' (or a civ TONE number).
  push(text, { tone = 'info', source = '' } = {}) {
    text = String(text ?? '').trim();
    if (!text) return false;
    if (typeof tone === 'number') tone = TONE_NAMES[tone] || 'info';
    if (this.items.some((n) => n.text === text && this.t - n.t < this.dedupe)) return false;
    this.items.push({ text, tone, source, t: this.t });
    if (this.items.length > this.max) this.items.splice(0, this.items.length - this.max);
    return true;
  }

  // A feed that looks like the old `headlines` array to a scene: push() adds to the tape,
  // slice() reads this source's lines back.
  channel(source, initial = [], tone = 'info') {
    for (const t of initial) this.push(t, { tone, source });
    const news = this;
    return {
      push: (...texts) => { for (const t of texts) news.push(t, { tone, source }); return news.items.length; },
      slice: (a, b) => news.items.filter((n) => n.source === source).map((n) => n.text).slice(a, b),
      get length() { return news.items.filter((n) => n.source === source).length; },
    };
  }

  update(dt) {
    this.t += dt;
    this.items = this.items.filter((n) => this.t - n.t < this.keep);
    if (this.x != null) this.x -= dt * this.speed;
  }

  get active() { return this.items.length > 0; }

  // The tape: a strip `h` high along the bottom of a `w`×`H` screen.
  draw(ctx, w, H, { h = 8 } = {}) {
    if (!this.active) { this.x = null; return; }
    ctx.save();
    ctx.fillStyle = 'rgba(14, 15, 26, 0.88)';
    ctx.fillRect(0, H - h, w, h);
    ctx.font = '5px "Pixelify Sans", monospace';
    ctx.textAlign = 'left';
    const gap = '   ·   ';
    const widths = this.items.map((n) => ctx.measureText(n.text + gap).width);
    const total = widths.reduce((a, b) => a + b, 0);
    if (this.x == null || this.x < -total) this.x = w;
    let x = this.x;
    this.items.forEach((n, i) => {
      if (x < w && x + widths[i] > 0) {
        ctx.fillStyle = TONE_COLOUR[n.tone] || TONE_COLOUR.info;
        ctx.fillText(n.text, x, H - 2.2);
        ctx.fillStyle = '#5a5f7a';
        ctx.fillText('·', x + widths[i] - ctx.measureText('   ·   ').width / 2 - 1, H - 2.2);
      }
      x += widths[i];
    });
    ctx.restore();
  }
}
