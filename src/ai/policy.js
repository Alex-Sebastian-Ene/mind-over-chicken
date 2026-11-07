// Tiny dependency-free runtime for the trained AI god: a multilayer perceptron whose weights
// were exported from stable-baselines3 PPO (actor only) by rl/export_policy.py.
//
//   const policy = await loadPolicy();          // fetches ./stone_policy.json next to this file
//   const a = policy.act(obs);                  // greedy action index
//   const p = policy.probs(obs);                // action probabilities

const ACT = {
  tanh: Math.tanh,
  relu: (x) => (x > 0 ? x : 0),
  identity: (x) => x,
};

export class MLPPolicy {
  constructor(spec) {
    if (!spec || !Array.isArray(spec.layers) || !spec.layers.length) throw new Error('policy: no layers');
    this.spec = spec;
    this.layers = spec.layers;
    this.activation = ACT[spec.activation] || Math.tanh;
    this.actions = spec.actions || [];
    this.obsNames = spec.obs_names || [];
    this.obsSize = this.layers[0].w[0].length;
  }

  logits(obs) {
    if (obs.length !== this.obsSize) throw new Error(`policy: expected ${this.obsSize} inputs, got ${obs.length}`);
    let h = obs;
    const last = this.layers.length - 1;
    for (let l = 0; l <= last; l++) {
      const { w, b } = this.layers[l];
      const out = new Array(b.length);
      for (let i = 0; i < b.length; i++) {
        const row = w[i];
        let s = b[i];
        for (let j = 0; j < row.length; j++) s += row[j] * h[j];
        out[i] = l < last ? this.activation(s) : s;
      }
      h = out;
    }
    return h;
  }

  probs(obs) {
    const z = this.logits(obs);
    const m = Math.max(...z);
    const e = z.map((v) => Math.exp(v - m));
    const s = e.reduce((a, b) => a + b, 0);
    return e.map((v) => v / s);
  }

  // Greedy by default (what the evaluation reports); pass {greedy:false} to sample.
  act(obs, { greedy = true, rng = Math.random } = {}) {
    if (greedy) {
      const z = this.logits(obs);
      let bi = 0;
      for (let i = 1; i < z.length; i++) if (z[i] > z[bi]) bi = i;
      return bi;
    }
    const p = this.probs(obs);
    let u = rng();
    for (let i = 0; i < p.length; i++) { u -= p[i]; if (u <= 0) return i; }
    return p.length - 1;
  }

  // Compare against the probabilities PyTorch produced at export time. Returns max |diff|.
  selfCheck() {
    const c = this.spec.check;
    if (!c || !c.obs) return 0;
    let worst = 0;
    c.obs.forEach((o, k) => this.probs(o).forEach((p, i) => { worst = Math.max(worst, Math.abs(p - c.probs[k][i])); }));
    return worst;
  }
}

export async function loadPolicy(url = new URL('./stone_policy.json', import.meta.url)) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`policy: ${res.status} loading ${url}`);
  return new MLPPolicy(await res.json());
}
