//! ReliefUNet forward pass, op-for-op the same as `rl/terrain_diffusion/model.py`, run on
//! the whole map with wrap-around padding (the map is a torus, so there are no seams).
//! Tensors are HWC; PyTorch's are CHW, which only changes indexing, not arithmetic.

use crate::format::{LoadError, Mocd};
use crate::json::Json;
use crate::nn::{self, Conv, Linear};

#[derive(Clone, Debug, PartialEq)]
pub struct Arch {
    pub n_cond: usize,
    pub widths: [usize; 3],
    pub mid_blocks: usize,
    pub time_freqs: usize,
    pub time_dim: usize,
}

impl Arch {
    /// Map sides at the model grid must be multiples of this.
    pub const STRIDE: usize = 8;

    pub fn from_json(j: &Json) -> Result<Self, LoadError> {
        let num = |k: &str| j.get(k).and_then(Json::as_usize).ok_or_else(|| LoadError::Arch(format!("missing '{k}'")));
        if num("s2d")? != 2 {
            return Err(LoadError::Arch("s2d must be 2".into()));
        }
        let w = j.get("widths").and_then(Json::as_arr).ok_or_else(|| LoadError::Arch("missing widths".into()))?;
        if w.len() != 3 {
            return Err(LoadError::Arch("need three widths".into()));
        }
        let wd = |i: usize| w[i].as_usize().filter(|&c| c > 0).ok_or_else(|| LoadError::Arch("bad width".into()));
        Ok(Arch {
            n_cond: num("n_cond")?,
            widths: [wd(0)?, wd(1)?, wd(2)?],
            mid_blocks: num("mid_blocks")?,
            time_freqs: num("time_freqs")?,
            time_dim: num("time_dim")?,
        })
    }

    /// Multiply-adds per model-grid pixel per denoising step (the cond share of conv_in is
    /// paid once per map, not per step).
    pub fn macs_per_pixel(&self) -> f64 {
        let [c0, c1, c2] = self.widths.map(|c| c as f64);
        let q = 0.25;
        let res = |c: f64, a: f64| 2.0 * 9.0 * c * c * a;
        9.0 * 4.0 * c0 * q
            + res(c0, q)
            + 4.0 * c0 * c1 * q / 4.0
            + res(c1, q / 4.0)
            + 4.0 * c1 * c2 * q / 16.0
            + self.mid_blocks as f64 * res(c2, q / 16.0)
            + c2 * c1 * q / 16.0
            + res(c1, q / 4.0)
            + c1 * c0 * q / 4.0
            + res(c0, q)
            + 9.0 * c0 * 4.0 * q
    }
}

#[derive(Clone, Debug)]
struct Res {
    conv1: Conv,
    film: Linear,
    conv2: Conv,
}

#[derive(Clone, Debug)]
pub struct ReliefUNet {
    pub arch: Arch,
    params: usize,
    time_l1: Linear,
    time_l2: Linear,
    conv_in_x: Conv,
    conv_in_c: Conv,
    enc0: Res,
    down0: Conv,
    enc1: Res,
    down1: Conv,
    mid: Vec<Res>,
    up1: Conv,
    dec1: Res,
    up0: Conv,
    dec0: Res,
    conv_out: Conv,
}

/// Source of named tensors: a parsed `.mocd`, or random ones for timing runs.
trait Weights {
    fn get(&self, name: &str, shape: &[usize]) -> Result<Vec<f32>, LoadError>;
}

impl Weights for Mocd {
    fn get(&self, name: &str, shape: &[usize]) -> Result<Vec<f32>, LoadError> {
        let t = self.tensor(name)?;
        if t.shape != shape {
            return Err(LoadError::Tensor(format!("{name}: shape {:?}, expected {:?}", t.shape, shape)));
        }
        Ok(t.data.clone())
    }
}

struct RandomWeights(std::cell::Cell<u64>);

impl Weights for RandomWeights {
    fn get(&self, _name: &str, shape: &[usize]) -> Result<Vec<f32>, LoadError> {
        let fan_in: usize = shape.iter().skip(1).product::<usize>().max(1);
        let scale = if shape.len() > 1 { 1.0 / (fan_in as f32).sqrt() } else { 0.1 };
        let n: usize = shape.iter().product();
        let mut r = crate::rng::SplitMix64::new(self.0.get());
        let v = (0..n).map(|_| (r.f32() * 2.0 - 1.0) * scale * 1.7).collect();
        self.0.set(r.next_u64());
        Ok(v)
    }
}

/// Wraps a weight source and counts the parameters it hands out.
struct Counting<'a>(&'a dyn Weights, std::cell::Cell<usize>);

impl Weights for Counting<'_> {
    fn get(&self, name: &str, shape: &[usize]) -> Result<Vec<f32>, LoadError> {
        let v = self.0.get(name, shape)?;
        self.1.set(self.1.get() + v.len());
        Ok(v)
    }
}

fn linear(w: &dyn Weights, name: &str, out: usize, inp: usize) -> Result<Linear, LoadError> {
    Ok(Linear { out, inp, w: w.get(&format!("{name}.weight"), &[out, inp])?, b: w.get(&format!("{name}.bias"), &[out])? })
}

fn conv(w: &dyn Weights, name: &str, k: usize, cout: usize, cin: usize) -> Result<Conv, LoadError> {
    let wt = w.get(&format!("{name}.weight"), &[cout, cin, k, k])?;
    let b = w.get(&format!("{name}.bias"), &[cout])?;
    Ok(Conv::from_torch(&wt, Some(&b), k, cout, cin, 0, cin))
}

fn res(w: &dyn Weights, name: &str, c: usize, td: usize) -> Result<Res, LoadError> {
    Ok(Res {
        conv1: conv(w, &format!("{name}.conv1"), 3, c, c)?,
        film: linear(w, &format!("{name}.film"), 2 * c, td)?,
        conv2: conv(w, &format!("{name}.conv2"), 3, c, c)?,
    })
}

/// Sinusoidal time features, as `model.time_features`.
pub fn time_features(t: f32, freqs: usize) -> Vec<f32> {
    let c = -(1000f64.ln()) as f32;
    let a: Vec<f32> = (0..freqs).map(|k| (t * 1000.0) * ((c * k as f32) / freqs as f32).exp()).collect();
    a.iter().map(|x| x.sin()).chain(a.iter().map(|x| x.cos())).collect()
}

/// The conditioning's share of `conv_in` (plus its bias), computed once per map.
pub struct CondCache {
    pub h: usize,
    pub w: usize,
    feat: Vec<f32>,
}

/// Reusable buffers for one map size.
#[derive(Default)]
pub struct Workspace {
    pad: Vec<f32>,
    s2d: Vec<f32>,
    l0: [Vec<f32>; 3],
    l1: [Vec<f32>; 3],
    l2: [Vec<f32>; 3],
    out4: Vec<f32>,
}

fn grow(v: &mut Vec<f32>, n: usize) {
    if v.len() < n {
        v.resize(n, 0.0);
    }
}

impl ReliefUNet {
    pub fn from_mocd(m: &Mocd) -> Result<Self, LoadError> {
        let arch = Arch::from_json(m.header.get("arch").ok_or_else(|| LoadError::Arch("no arch".into()))?)?;
        Self::build(arch, m)
    }

    /// Random weights of a given size: for timing runs only.
    #[doc(hidden)]
    pub fn random(arch: Arch, seed: u64) -> Self {
        Self::build(arch, &RandomWeights(std::cell::Cell::new(seed))).expect("random weights always fit")
    }

    fn build(arch: Arch, src: &dyn Weights) -> Result<Self, LoadError> {
        let w = &Counting(src, std::cell::Cell::new(0));
        let [c0, c1, c2] = arch.widths;
        let td = arch.time_dim;
        let cin_total = 4 * (1 + arch.n_cond);
        let wi = w.get("conv_in.weight", &[c0, cin_total, 3, 3])?;
        let bi = w.get("conv_in.bias", &[c0])?;
        let mut net = ReliefUNet {
            params: 0,
            time_l1: linear(w, "time_l1", td, 2 * arch.time_freqs)?,
            time_l2: linear(w, "time_l2", td, td)?,
            // x_t is input channel 0, so after space-to-depth it owns channels 0..4.
            conv_in_x: Conv::from_torch(&wi, None, 3, c0, cin_total, 0, 4),
            conv_in_c: Conv::from_torch(&wi, Some(&bi), 3, c0, cin_total, 4, cin_total - 4),
            enc0: res(w, "enc0", c0, td)?,
            down0: conv(w, "down0", 1, c1, 4 * c0)?,
            enc1: res(w, "enc1", c1, td)?,
            down1: conv(w, "down1", 1, c2, 4 * c1)?,
            mid: (0..arch.mid_blocks).map(|i| res(w, &format!("mid.{i}"), c2, td)).collect::<Result<_, _>>()?,
            up1: conv(w, "up1", 1, c1, c2)?,
            dec1: res(w, "dec1", c1, td)?,
            up0: conv(w, "up0", 1, c0, c1)?,
            dec0: res(w, "dec0", c0, td)?,
            conv_out: conv(w, "conv_out", 3, 4, c0)?,
            arch,
        };
        net.params = w.1.get();
        Ok(net)
    }

    pub fn param_count(&self) -> usize {
        self.params
    }

    /// `cond` is h x w x n_cond (HWC, model grid; h and w multiples of 8).
    pub fn prepare(&self, cond: &[f32], h: usize, w: usize, ws: &mut Workspace) -> CondCache {
        assert!(h % Arch::STRIDE == 0 && w % Arch::STRIDE == 0, "model grid must be a multiple of 8");
        let n = self.arch.n_cond;
        assert_eq!(cond.len(), n * h * w);
        let (h0, w0) = (h / 2, w / 2);
        grow(&mut ws.s2d, 4 * n * h0 * w0);
        nn::space_to_depth(cond, n, h, w, &mut ws.s2d);
        grow(&mut ws.pad, 4 * n * (h0 + 2) * (w0 + 2));
        nn::pad_wrap(&ws.s2d, 4 * n, h0, w0, false, &mut ws.pad);
        let mut feat = vec![0.0; self.arch.widths[0] * h0 * w0];
        nn::conv_bias(&ws.pad, h0, w0, &self.conv_in_c, &mut feat);
        CondCache { h, w, feat }
    }

    #[allow(clippy::too_many_arguments)]
    fn res_block(&self, r: &Res, temb: &[f32], x: &[f32], h: usize, w: usize, pad: &mut Vec<f32>, tmp: &mut [f32], out: &mut [f32]) {
        let c = r.conv1.cout;
        let f = r.film.forward(temb);
        let (scale, shift) = f.split_at(c);
        let mul: Vec<f32> = scale.iter().map(|s| 1.0 + s).collect();
        let add: Vec<f32> = (0..c).map(|o| r.conv1.bias[o] * mul[o] + shift[o]).collect();
        grow(pad, c * (h + 2) * (w + 2));
        nn::pad_wrap(x, c, h, w, true, pad);
        nn::conv(pad, h, w, &r.conv1, &mul, &add, None, tmp);
        nn::pad_wrap(tmp, c, h, w, true, pad);
        let ones = vec![1.0; c];
        nn::conv(pad, h, w, &r.conv2, &ones, &r.conv2.bias, Some(x), out);
    }

    /// v = net(x_t, cond, t) on the whole torus. `x_t` and `v` are h x w.
    pub fn forward(&self, x_t: &[f32], cond: &CondCache, t: f32, ws: &mut Workspace, v: &mut [f32]) {
        let (h, w) = (cond.h, cond.w);
        let [c0, c1, c2] = self.arch.widths;
        let (h0, w0, h1, w1, h2, w2) = (h / 2, w / 2, h / 4, w / 4, h / 8, w / 8);
        let (n0, n1, n2) = (h0 * w0, h1 * w1, h2 * w2);
        let tf = time_features(t, self.arch.time_freqs);
        let temb: Vec<f32> = self.time_l1.forward(&tf).into_iter().map(nn::silu).collect();
        let temb: Vec<f32> = self.time_l2.forward(&temb).into_iter().map(nn::silu).collect();

        let Workspace { pad, s2d, l0, l1, l2, out4 } = ws;
        for b in l0.iter_mut() {
            grow(b, c0 * n0);
        }
        for b in l1.iter_mut() {
            grow(b, c1 * n1);
        }
        for b in l2.iter_mut() {
            grow(b, c2 * n2);
        }
        grow(s2d, (4 * n0).max(c0 * n0).max(c1 * n1).max(c1 * n2).max(c0 * n1));
        grow(pad, c0.max(4) * (h0 + 2) * (w0 + 2));

        // conv_in: the x_t share, plus the cached cond share (which carries the bias).
        nn::space_to_depth(x_t, 1, h, w, s2d);
        nn::pad_wrap(s2d, 4, h0, w0, false, pad);
        let [a0, b0, e0] = l0;
        nn::conv(pad, h0, w0, &self.conv_in_x, &vec![1.0; c0], &vec![0.0; c0], Some(&cond.feat), a0);
        self.res_block(&self.enc0, &temb, a0, h0, w0, pad, b0, e0);

        nn::space_to_depth(e0, c0, h0, w0, s2d);
        let [a1, b1, e1] = l1;
        nn::conv_bias(s2d, h1, w1, &self.down0, a1);
        self.res_block(&self.enc1, &temb, a1, h1, w1, pad, b1, e1);

        nn::space_to_depth(e1, c1, h1, w1, s2d);
        let [a2, b2, c2b] = l2;
        nn::conv_bias(s2d, h2, w2, &self.down1, a2);
        let (mut cur, mut nxt) = (a2, c2b);
        for blk in &self.mid {
            self.res_block(blk, &temb, cur, h2, w2, pad, b2, nxt);
            std::mem::swap(&mut cur, &mut nxt);
        }

        // Up: 1x1 at low resolution, nearest x2, add the skip.
        nn::conv_bias(cur, h2, w2, &self.up1, &mut s2d[..c1 * n2]);
        nn::up2_add(s2d, e1, c1, h2, w2, a1);
        self.res_block(&self.dec1, &temb, a1, h1, w1, pad, b1, e1);

        nn::conv_bias(e1, h1, w1, &self.up0, &mut s2d[..c0 * n1]);
        nn::up2_add(s2d, e0, c0, h1, w1, a0);
        self.res_block(&self.dec0, &temb, a0, h0, w0, pad, b0, e0);

        nn::pad_wrap(e0, c0, h0, w0, true, pad);
        grow(out4, 4 * n0);
        nn::conv_bias(pad, h0, w0, &self.conv_out, out4);
        nn::depth_to_space1(out4, h0, w0, v);
    }
}
