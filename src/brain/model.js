// The hidden calibration. While the player does things everyone can do (relax, blink,
// look around), the game records feature windows labelled by what it asked for at the time.
// A small softmax regression is retrained on those windows every few seconds, so by the
// time the finale needs a focus classifier it has been trained on this player's own brain.

export const CLASSES = ['rest', 'calm', 'focus'];
export const FEATURE_NAMES = ['ln δ', 'ln θ', 'ln α', 'ln β', 'ln γ', 'ln α/β', 'ln β/(α+θ)'];

const EPS = 1e-4;
const MAX_PER_CLASS = 600;
const MIN_PER_CLASS = 15;
const TRAIN_CAP = 120;

export function featuresFromBands(b) {
  const d = b.delta + EPS, t = b.theta + EPS, a = b.alpha + EPS, be = b.beta + EPS, g = b.gamma + EPS;
  return [Math.log(d), Math.log(t), Math.log(a), Math.log(be), Math.log(g), Math.log(a / be), Math.log(be / (a + t))];
}

export class PersonalModel {
  constructor() {
    this.samples = { rest: [], calm: [], focus: [] };
    this.trained = false;
    this.classes = [];       // classes present at the last training
    this.W = null; this.b = null; this.mu = null; this.sd = null;
    this.accuracy = 0;       // blocked cross-validated accuracy
    this.chance = 0;
    this.newSinceTrain = 0;
    this.trainCount = 0;
    this.lastTrainMs = 0;
  }

  add(label, f) {
    const arr = this.samples[label];
    if (!arr) return;
    arr.push(f);
    if (arr.length > MAX_PER_CLASS) arr.shift();
    this.newSinceTrain++;
  }

  count(label) { return this.samples[label]?.length || 0; }
  total() { return CLASSES.reduce((s, c) => s + this.count(c), 0); }
  has(label) { return this.trained && this.classes.includes(label); }

  maybeTrain() {
    if (this.newSinceTrain < 10) return false;
    return this.train();
  }

  train() {
    const classes = CLASSES.filter((c) => this.count(c) >= MIN_PER_CLASS);
    if (classes.length < 2) return false;
    const t0 = performance.now();
    // Train on an evenly strided subset (≤ TRAIN_CAP per class) so a retrain stays a few ms.
    const data = [];
    const sub = {};
    classes.forEach((c, k) => {
      const arr = this.samples[c];
      const stride = Math.max(1, Math.ceil(arr.length / TRAIN_CAP));
      const picked = [];
      for (let i = 0; i < arr.length; i += stride) picked.push(arr[i]);
      sub[c] = picked.length;
      picked.forEach((f, i) => data.push({ f, k, c, i }));
    });

    // Blocked cross-validation: each class is split into 4 contiguous time chunks, so
    // neighbouring (highly correlated) windows never sit on both sides of a split.
    const FOLDS = 4;
    let correct = 0, tested = 0;
    for (let fold = 0; fold < FOLDS; fold++) {
      const train = [], test = [];
      for (const d of data) {
        const n = sub[d.c];
        const chunk = Math.min(FOLDS - 1, Math.floor((d.i / n) * FOLDS));
        (chunk === fold ? test : train).push(d);
      }
      if (!test.length || !train.length) continue;
      const m = fit(train, classes.length);
      for (const d of test) { if (argmax(predictRaw(m, d.f)) === d.k) correct++; tested++; }
    }
    const model = fit(data, classes.length);
    this.W = model.W; this.b = model.b; this.mu = model.mu; this.sd = model.sd;
    this.classes = classes;
    this.accuracy = tested ? correct / tested : 0;
    this.chance = 1 / classes.length;
    this.trained = true;
    this.newSinceTrain = 0;
    this.trainCount++;
    this.lastTrainMs = performance.now() - t0;
    return true;
  }

  // Returns {rest, calm, focus} probabilities for the classes it knows, or null.
  predict(f) {
    if (!this.trained) return null;
    const p = predictRaw(this, f);
    const out = {};
    this.classes.forEach((c, k) => { out[c] = p[k]; });
    return out;
  }

  // How much the game should trust the personal model over the generic rule (0..1).
  trust(label) {
    if (!this.has(label)) return 0;
    const skill = Math.max(0, Math.min(1, (this.accuracy - this.chance - 0.1) / 0.3));
    const amount = Math.min(1, this.count(label) / 60);
    return skill * amount;
  }

  // Per-class weights in original feature units, for the judge overlay.
  weights() {
    if (!this.trained) return [];
    return this.classes.map((c, k) => ({ c, w: this.W[k].map((w, j) => w / this.sd[j]) }));
  }
}

function fit(data, K) {
  const D = data[0].f.length;
  const mu = new Array(D).fill(0), sd = new Array(D).fill(0);
  for (const d of data) for (let j = 0; j < D; j++) mu[j] += d.f[j];
  for (let j = 0; j < D; j++) mu[j] /= data.length;
  for (const d of data) for (let j = 0; j < D; j++) sd[j] += (d.f[j] - mu[j]) ** 2;
  for (let j = 0; j < D; j++) sd[j] = Math.sqrt(sd[j] / data.length) || 1;
  const X = data.map((d) => d.f.map((v, j) => (v - mu[j]) / sd[j]));

  // Class weights so a class with many samples doesn't swamp a rare one.
  const counts = new Array(K).fill(0);
  data.forEach((d) => counts[d.k]++);
  const cw = counts.map((n) => data.length / (K * Math.max(1, n)));

  const W = Array.from({ length: K }, () => new Array(D).fill(0));
  const b = new Array(K).fill(0);
  const lr = 0.8, l2 = 0.02, iters = 90, N = data.length;
  const p = new Array(K);
  for (let it = 0; it < iters; it++) {
    const gW = Array.from({ length: K }, () => new Array(D).fill(0));
    const gb = new Array(K).fill(0);
    for (let n = 0; n < N; n++) {
      const x = X[n];
      let max = -Infinity;
      for (let k = 0; k < K; k++) {
        let z = b[k];
        for (let j = 0; j < D; j++) z += W[k][j] * x[j];
        p[k] = z; if (z > max) max = z;
      }
      let s = 0;
      for (let k = 0; k < K; k++) { p[k] = Math.exp(p[k] - max); s += p[k]; }
      const w = cw[data[n].k];
      for (let k = 0; k < K; k++) {
        const g = (p[k] / s - (data[n].k === k ? 1 : 0)) * w;
        gb[k] += g;
        for (let j = 0; j < D; j++) gW[k][j] += g * x[j];
      }
    }
    for (let k = 0; k < K; k++) {
      b[k] -= (lr * gb[k]) / N;
      for (let j = 0; j < D; j++) W[k][j] -= lr * (gW[k][j] / N + l2 * W[k][j]);
    }
  }
  return { W, b, mu, sd };
}

function predictRaw(m, f) {
  const K = m.W.length, D = f.length;
  const z = new Array(K);
  let max = -Infinity;
  for (let k = 0; k < K; k++) {
    let s = m.b[k];
    for (let j = 0; j < D; j++) s += m.W[k][j] * ((f[j] - m.mu[j]) / m.sd[j]);
    z[k] = s; if (s > max) max = s;
  }
  let sum = 0;
  for (let k = 0; k < K; k++) { z[k] = Math.exp(z[k] - max); sum += z[k]; }
  return z.map((v) => v / sum);
}

function argmax(a) { let bi = 0; for (let i = 1; i < a.length; i++) if (a[i] > a[bi]) bi = i; return bi; }
