//! A tiny policy runtime for the PPO war councils (#100): the actor half of a
//! stable-baselines3 (sb3-contrib MaskablePPO) MLP, read from a compact binary file and run in
//! plain Rust, here and inside the wasm. No dependencies, no JSON parser.
//!
//! Why not `crates/moc-ai` (#82): its runtime is fixed at 20 inputs and 5 actions, has a single
//! categorical head, and parses JSON with serde, which this crate keeps out of its wasm. The war
//! councils need any input size, several categorical heads at once (a MultiDiscrete action) and
//! action masks.
//!
//! **Format `MOCP` v1** (all little-endian; written by `rl/civ_war/export_policy.py`):
//! ```text
//! b"MOCP" u32 format(=1) u32 kind(0 council, 1 commander) u32 obs_version u32 obs_size
//! u32 n_heads  u32 head_size × n_heads
//! u32 activation (0 tanh, 1 relu)
//! u32 n_layers (u32 in, u32 out) × n_layers
//! per layer: f32 weight[out][in], f32 bias[out]
//! u32 n_checks (f32 obs[obs_size], f32 logits[Σ heads]) × n_checks
//! b"END!"
//! ```
//! The check vectors are PyTorch's own logits for a few probe observations; a file whose
//! logits we can't reproduce (to 1e-3) is rejected, and the caller falls back to the rules.

pub const MAGIC: &[u8; 4] = b"MOCP";
pub const TRAILER: &[u8; 4] = b"END!";
pub const FORMAT: u32 = 1;
/// Biggest network accepted (keeps a corrupt header from allocating gigabytes).
const MAX_WIDTH: usize = 1024;
const MAX_LAYERS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Council = 0,
    Commander = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    Tanh,
    Relu,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub n_in: usize,
    pub n_out: usize,
    /// Row-major [out][in].
    pub w: Vec<f32>,
    pub b: Vec<f32>,
}

/// An MLP actor with several categorical heads.
#[derive(Clone, Debug)]
pub struct Mlp {
    pub kind: Kind,
    pub obs_version: u32,
    pub obs_size: usize,
    pub heads: Vec<usize>,
    pub act: Activation,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    Empty,
    BadMagic,
    BadFormat(u32),
    Truncated,
    WrongKind,
    WrongObsVersion { want: u32, got: u32 },
    WrongShape(&'static str),
    NotFinite,
    CheckFailed(f32),
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], LoadError> {
        let end = self.at.checked_add(n).ok_or(LoadError::Truncated)?;
        let s = self.b.get(self.at..end).ok_or(LoadError::Truncated)?;
        self.at = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, LoadError> {
        let s = self.bytes(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, LoadError> {
        let s = self.bytes(n.checked_mul(4).ok_or(LoadError::Truncated)?)?;
        let v: Vec<f32> = s.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        if v.iter().any(|x| !x.is_finite()) {
            return Err(LoadError::NotFinite);
        }
        Ok(v)
    }
}

impl Mlp {
    /// Parse and validate a `MOCP` file for `kind`, observation version `obs_version` and
    /// `obs_size` inputs, with the given heads. Runs the embedded self-check.
    pub fn load(bytes: &[u8], kind: Kind, obs_version: u32, obs_size: usize, heads: &[usize]) -> Result<Mlp, LoadError> {
        if bytes.is_empty() {
            return Err(LoadError::Empty);
        }
        let mut r = Reader { b: bytes, at: 0 };
        if r.bytes(4)? != MAGIC {
            return Err(LoadError::BadMagic);
        }
        let format = r.u32()?;
        if format != FORMAT {
            return Err(LoadError::BadFormat(format));
        }
        let k = r.u32()?;
        if k != kind as u32 {
            return Err(LoadError::WrongKind);
        }
        let ov = r.u32()?;
        if ov != obs_version {
            return Err(LoadError::WrongObsVersion { want: obs_version, got: ov });
        }
        let n_obs = r.u32()? as usize;
        if n_obs != obs_size {
            return Err(LoadError::WrongShape("observation size"));
        }
        let n_heads = r.u32()? as usize;
        if n_heads != heads.len() {
            return Err(LoadError::WrongShape("number of heads"));
        }
        for &h in heads {
            if r.u32()? as usize != h {
                return Err(LoadError::WrongShape("head size"));
            }
        }
        let act = match r.u32()? {
            0 => Activation::Tanh,
            1 => Activation::Relu,
            _ => return Err(LoadError::WrongShape("activation")),
        };
        let n_layers = r.u32()? as usize;
        if n_layers == 0 || n_layers > MAX_LAYERS {
            return Err(LoadError::WrongShape("layer count"));
        }
        let mut dims = Vec::with_capacity(n_layers);
        for _ in 0..n_layers {
            let (i, o) = (r.u32()? as usize, r.u32()? as usize);
            if i == 0 || o == 0 || i > MAX_WIDTH || o > MAX_WIDTH {
                return Err(LoadError::WrongShape("layer width"));
            }
            dims.push((i, o));
        }
        let total: usize = heads.iter().sum();
        if dims[0].0 != obs_size || dims[n_layers - 1].1 != total || dims.windows(2).any(|p| p[0].1 != p[1].0) {
            return Err(LoadError::WrongShape("layer chain"));
        }
        let mut layers = Vec::with_capacity(n_layers);
        for &(i, o) in &dims {
            let w = r.f32s(i * o)?;
            let b = r.f32s(o)?;
            layers.push(Layer { n_in: i, n_out: o, w, b });
        }
        let mlp = Mlp { kind, obs_version, obs_size, heads: heads.to_vec(), act, layers };
        let n_checks = r.u32()? as usize;
        if n_checks > 64 {
            return Err(LoadError::WrongShape("check count"));
        }
        let mut worst = 0.0f32;
        let mut out = vec![0.0; total];
        for _ in 0..n_checks {
            let obs = r.f32s(obs_size)?;
            let want = r.f32s(total)?;
            mlp.logits(&obs, &mut out);
            for (a, b) in out.iter().zip(&want) {
                worst = worst.max((a - b).abs());
            }
        }
        if r.bytes(4)? != TRAILER {
            return Err(LoadError::Truncated);
        }
        if worst > 1e-3 {
            return Err(LoadError::CheckFailed(worst));
        }
        Ok(mlp)
    }

    /// Raw logits for every head, concatenated, into `out` (length Σ heads).
    pub fn logits(&self, obs: &[f32], out: &mut [f32]) {
        let mut a = [0.0f32; MAX_WIDTH];
        let mut b = [0.0f32; MAX_WIDTH];
        let n = obs.len().min(self.obs_size);
        a[..n].copy_from_slice(&obs[..n]);
        let last = self.layers.len() - 1;
        for (li, l) in self.layers.iter().enumerate() {
            for o in 0..l.n_out {
                let row = &l.w[o * l.n_in..(o + 1) * l.n_in];
                let mut s = l.b[o];
                for (wi, xi) in row.iter().zip(&a[..l.n_in]) {
                    s += wi * xi;
                }
                b[o] = if li == last {
                    s
                } else {
                    match self.act {
                        Activation::Tanh => s.tanh(),
                        Activation::Relu => s.max(0.0),
                    }
                };
            }
            std::mem::swap(&mut a, &mut b);
        }
        let k = out.len().min(self.layers[last].n_out);
        out[..k].copy_from_slice(&a[..k]);
    }

    /// Pick one option per head: the best legal one (`greedy`), or a sample from the masked
    /// softmax with uniform draws `u` (one per head). `mask[i]` false = option i is illegal;
    /// a head with no legal option picks 0. Returns false (and picks nothing) if the
    /// observation has the wrong size or anything in it or the logits is not finite: the
    /// caller then falls back to the rules.
    pub fn act(&self, obs: &[f32], mask: &[bool], greedy: bool, u: &[f32], out: &mut [u8]) -> bool {
        if obs.len() != self.obs_size || !obs.iter().all(|x| x.is_finite()) {
            return false;
        }
        let total: usize = self.heads.iter().sum();
        let mut z = [0.0f32; MAX_WIDTH];
        self.logits(obs, &mut z[..total]);
        if !z[..total].iter().all(|x| x.is_finite()) {
            return false;
        }
        pick(&self.heads, &z[..total], mask, greedy, u, out);
        true
    }
}

/// Masked per-head choice from concatenated logits (shared with tests and the gym's league).
pub fn pick(heads: &[usize], z: &[f32], mask: &[bool], greedy: bool, u: &[f32], out: &mut [u8]) {
    let mut at = 0;
    for (h, &n) in heads.iter().enumerate() {
        let legal = |i: usize| mask.get(at + i).copied().unwrap_or(true);
        let zs = &z[at..at + n];
        let mut best = None;
        for i in 0..n {
            if legal(i) && best.map_or(true, |b: usize| zs[i] > zs[b]) {
                best = Some(i);
            }
        }
        let mut choice = best.unwrap_or(0);
        if !greedy && best.is_some() {
            let m = zs[choice];
            let sum: f32 = (0..n).filter(|&i| legal(i)).map(|i| (zs[i] - m).exp()).sum();
            let mut r = u.get(h).copied().unwrap_or(0.5) * sum;
            for i in 0..n {
                if !legal(i) {
                    continue;
                }
                r -= (zs[i] - m).exp();
                if r <= 0.0 {
                    choice = i;
                    break;
                }
            }
        }
        out[h] = choice as u8;
        at += n;
    }
}

/// Serialise a network in the `MOCP` format (tests and tools; the trainer writes the same bytes
/// from Python). `checks`: probe observations whose logits are stored for the self-check.
pub fn encode(m: &Mlp, checks: &[Vec<f32>]) -> Vec<u8> {
    let mut b = Vec::new();
    let u = |b: &mut Vec<u8>, x: u32| b.extend_from_slice(&x.to_le_bytes());
    let f = |b: &mut Vec<u8>, x: f32| b.extend_from_slice(&x.to_le_bytes());
    b.extend_from_slice(MAGIC);
    u(&mut b, FORMAT);
    u(&mut b, m.kind as u32);
    u(&mut b, m.obs_version);
    u(&mut b, m.obs_size as u32);
    u(&mut b, m.heads.len() as u32);
    for &h in &m.heads {
        u(&mut b, h as u32);
    }
    u(&mut b, if m.act == Activation::Tanh { 0 } else { 1 });
    u(&mut b, m.layers.len() as u32);
    for l in &m.layers {
        u(&mut b, l.n_in as u32);
        u(&mut b, l.n_out as u32);
    }
    for l in &m.layers {
        l.w.iter().for_each(|&x| f(&mut b, x));
        l.b.iter().for_each(|&x| f(&mut b, x));
    }
    u(&mut b, checks.len() as u32);
    let total: usize = m.heads.iter().sum();
    let mut z = vec![0.0; total];
    for c in checks {
        c.iter().for_each(|&x| f(&mut b, x));
        m.logits(c, &mut z);
        z.iter().for_each(|&x| f(&mut b, x));
    }
    b.extend_from_slice(TRAILER);
    b
}

/// A random network of the given shape (tests, and a stand-in before training).
pub fn random(kind: Kind, obs_version: u32, obs_size: usize, heads: &[usize], hidden: &[usize], seed: u32) -> Mlp {
    let mut rng = moc_planet::rng::Rng::new(seed);
    let mut dims = vec![obs_size];
    dims.extend_from_slice(hidden);
    dims.push(heads.iter().sum());
    let layers = dims
        .windows(2)
        .map(|p| {
            let s = 1.0 / (p[0] as f32).sqrt();
            Layer { n_in: p[0], n_out: p[1], w: (0..p[0] * p[1]).map(|_| rng.range(-s, s)).collect(), b: (0..p[1]).map(|_| rng.range(-0.1, 0.1)).collect() }
        })
        .collect();
    Mlp { kind, obs_version, obs_size, heads: heads.to_vec(), act: Activation::Tanh, layers }
}
