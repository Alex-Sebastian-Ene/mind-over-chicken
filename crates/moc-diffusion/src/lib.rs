//! Learned terrain relief for Mind over Chicken (#110).
//!
//! A small conditional diffusion UNet, trained offline on real elevation data
//! (`rl/terrain_diffusion/`), turns the procedural generator's coarse heightmap into
//! realistic relief: ranges with spurs and valleys, plateaus, rough hills, ragged coasts.
//! Inference is pure Rust (no dependencies beyond moc-terrain) and runs in wasm.
//!
//! [`LearnedDiffusion`] implements moc-terrain's [`ReliefDetail`] hook:
//! 1. The procedural stage shapes the coarse grid first ([`ProceduralDiffusion::shape`]: DLA
//!    ranges along the plates' uplift, erosion), so ranges stay where the plates put them.
//! 2. The coarse heights, uplift and land weight are resampled onto the *model grid*: two
//!    model pixels per coarse cell (the full map for the 320x180 and 640x360 maps, half
//!    resolution for the 2560x1280 civ planet), rounded to a multiple of 8.
//! 3. A fourth conditioning channel, *roughness*, comes from the uplift plus a smooth random
//!    field, so different regions of one map get different landforms (plains, hills,
//!    plateaus, ranges) instead of one landform everywhere.
//! 4. DDIM (a few steps) denoises a fresh random field into a height residual, running the
//!    UNet on the whole map with wrap-around padding: the map is a torus, so there are no
//!    patches and no seams on either axis.
//! 5. The residual is added to the coarse heights at full resolution, and the coarse
//!    land/sea layout is kept (only a thin band around the coast may move).
//!
//! Every run is random: the sampling noise comes from the game's RNG. If the weights are
//! missing or do not load, the procedural detail runs instead, unchanged.

pub mod format;
pub mod grid;
pub mod json;
pub mod nn;
pub mod rng;
pub mod sampler;
pub mod unet;

#[cfg(feature = "wasm-bench")]
pub mod bench_ffi;

use format::LoadError;
use json::Json;
pub use moc_terrain::{DetailInput, Dims, ProceduralDiffusion, ReliefDetail, Rng};
use rng::SplitMix64;
use unet::{ReliefUNet, Workspace};

/// Sampling knobs. Defaults come from the weights' header where it sets them.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleOptions {
    /// DDIM steps (the budget knob: cost is linear in it).
    pub steps: usize,
    /// Model-grid pixels per coarse cell. 2 means: full resolution when cell = 2, half
    /// resolution when cell = 4 (the civ planet), where the 2x2 majority filter would
    /// remove finer detail anyway.
    pub model_px_per_cell: f32,
    /// Upper bound on model-grid pixels (memory and time cap for very large maps).
    pub max_model_px: usize,
    /// Multiplies the generated residual (1 = as learned from real terrain).
    pub gain: f32,
    /// The model may flip land/sea only where |coarse| < this (coast roughening).
    pub coast_band: f32,
    /// Uplift conditioning = clamp(bias + gain * uplift).
    pub uplift_bias: f32,
    pub uplift_gain: f32,
    /// Roughness conditioning = clamp(base + k_uplift * uplift + k_noise * (field - 0.5)).
    pub rough_base: f32,
    pub rough_uplift: f32,
    pub rough_noise: f32,
    /// Size of the random landform regions, as a fraction of the continent scale.
    pub region_frac: f32,
}

impl Default for SampleOptions {
    fn default() -> Self {
        SampleOptions {
            steps: 6,
            model_px_per_cell: 2.0,
            max_model_px: 1 << 20,
            gain: 1.0,
            coast_band: 0.012,
            uplift_bias: 0.15,
            uplift_gain: 0.85,
            rough_base: 0.2,
            rough_uplift: 0.45,
            rough_noise: 0.9,
            region_frac: 0.35,
        }
    }
}

impl SampleOptions {
    fn apply_header(&mut self, s: &Json) {
        if let Some(n) = s.get("steps").and_then(Json::as_usize) {
            self.steps = n.max(1);
        }
        self.gain = s.f32_or("gain", self.gain);
        self.coast_band = s.f32_or("coast_band", self.coast_band);
        self.uplift_bias = s.f32_or("uplift_bias", self.uplift_bias);
        self.uplift_gain = s.f32_or("uplift_gain", self.uplift_gain);
        self.rough_base = s.f32_or("rough_base", self.rough_base);
        self.rough_uplift = s.f32_or("rough_uplift", self.rough_uplift);
        self.rough_noise = s.f32_or("rough_noise", self.rough_noise);
        self.region_frac = s.f32_or("region_frac", self.region_frac);
    }
}

/// How model inputs and outputs map to game height units (from the training data).
#[derive(Clone, Debug, PartialEq)]
pub struct Norm {
    pub coarse_mean: f32,
    pub coarse_std: f32,
    /// Game units per unit of generated residual.
    pub resid_std: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DetailError {
    /// No usable weights (missing or failed to load).
    NoModel,
    /// Input arrays do not match their dims.
    BadInput(String),
}

pub struct Model {
    pub net: ReliefUNet,
    pub norm: Norm,
}

/// The learned relief detail. Holds the unpacked weights; build it once and reuse it.
pub struct LearnedDiffusion {
    model: Option<Model>,
    error: Option<String>,
    pub opts: SampleOptions,
    /// Shapes the coarse grid before the model (and is the whole detail stage when there is
    /// no model). Set it from the preset with [`with_procedural`](Self::with_procedural).
    pub procedural: ProceduralDiffusion,
}

/// The weights baked in at build time (`crates/moc-diffusion/weights/relief.mocd`), if any.
#[cfg(moc_relief_weights)]
pub const EMBEDDED_WEIGHTS: Option<&[u8]> = Some(include_bytes!("../weights/relief.mocd"));
#[cfg(not(moc_relief_weights))]
pub const EMBEDDED_WEIGHTS: Option<&[u8]> = None;

const LAND_EPS: f32 = 0.004;
const SEA_EPS: f32 = 0.004;

impl LearnedDiffusion {
    /// Parses `.mocd` weights. Never fails: on error the instance falls back to procedural
    /// detail and [`load_error`](Self::load_error) says why.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match Self::load(bytes) {
            Ok((model, opts)) => LearnedDiffusion { model: Some(model), error: None, opts, procedural: ProceduralDiffusion::default() },
            Err(e) => Self::without_model(e.to_string()),
        }
    }

    /// The weights embedded at build time, or the procedural fallback if there are none.
    pub fn embedded() -> Self {
        match EMBEDDED_WEIGHTS {
            Some(b) => Self::from_bytes(b),
            None => Self::without_model("built without weights/relief.mocd".into()),
        }
    }

    pub fn without_model(reason: String) -> Self {
        LearnedDiffusion { model: None, error: Some(reason), opts: SampleOptions::default(), procedural: ProceduralDiffusion::default() }
    }

    /// A model with random weights of the given size: timing runs only.
    #[doc(hidden)]
    pub fn random_for_timing(arch: unet::Arch, seed: u64) -> Self {
        let model = Model { net: ReliefUNet::random(arch, seed), norm: Norm { coarse_mean: 0.0, coarse_std: 0.35, resid_std: 0.05 } };
        LearnedDiffusion { model: Some(model), error: None, opts: SampleOptions::default(), procedural: ProceduralDiffusion::default() }
    }

    fn load(bytes: &[u8]) -> Result<(Model, SampleOptions), LoadError> {
        let m = format::parse(bytes)?;
        let fmt = m.header.get("format").and_then(Json::as_str).unwrap_or("");
        if fmt != "moc-relief-unet/1" {
            return Err(LoadError::Header(format!("format '{fmt}'")));
        }
        let net = ReliefUNet::from_mocd(&m)?;
        if net.arch.n_cond != 4 {
            return Err(LoadError::Arch(format!("{} conditioning channels, runtime builds 4", net.arch.n_cond)));
        }
        let n = m.header.get("norm").ok_or_else(|| LoadError::Header("no norm".into()))?;
        let norm = Norm { coarse_mean: n.f32_or("coarse_mean", 0.0), coarse_std: n.f32_or("coarse_std", 0.35), resid_std: n.f32_or("resid_std", 0.05) };
        if !(norm.coarse_std > 0.0 && norm.resid_std > 0.0) {
            return Err(LoadError::Header("norm scales must be positive".into()));
        }
        let mut opts = SampleOptions::default();
        if let Some(s) = m.header.get("sampling") {
            opts.apply_header(s);
        }
        Ok((Model { net, norm }, opts))
    }

    pub fn is_learned(&self) -> bool {
        self.model.is_some()
    }

    pub fn load_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn model(&self) -> Option<&Model> {
        self.model.as_ref()
    }

    /// The same model with a preset's procedural stage (`ProceduralDiffusion::from_params`).
    pub fn with_procedural(mut self, p: ProceduralDiffusion) -> Self {
        self.procedural = p;
        self
    }

    /// The model grid used for a map: `(w, h)`, multiples of 8.
    pub fn model_grid(&self, input: &DetailInput) -> (usize, usize) {
        let s = self.opts.model_px_per_cell / input.cell.max(1) as f32;
        let mut s = s.min(1.0);
        let fit = |s: f32| {
            let r = |n: usize| (((n as f32 * s) / 8.0).round() as usize).max(1) * 8;
            (r(input.dims.w), r(input.dims.h))
        };
        let (mut w, mut h) = fit(s);
        while w * h > self.opts.max_model_px && s > 0.05 {
            s *= 0.9;
            (w, h) = fit(s);
        }
        (w, h)
    }

    /// The model alone on `input.coarse` (no procedural shaping first): what the hook runs
    /// after [`ProceduralDiffusion::shape`]. `seed` sets the sampling noise.
    pub fn try_detail(&self, input: &DetailInput, seed: u64) -> Result<Vec<f32>, DetailError> {
        let model = self.model.as_ref().ok_or(DetailError::NoModel)?;
        check(input)?;
        let o = &self.opts;
        let (cw, ch) = (input.coarse_dims.w, input.coarse_dims.h);
        let (w, h) = (input.dims.w, input.dims.h);
        let (mw, mh) = self.model_grid(input);
        let np = mw * mh;
        let mut rng = SplitMix64::new(seed);

        // Conditioning on the model grid, normalised as in training.
        let coarse_m = grid::resample_cubic(input.coarse, cw, ch, mw, mh);
        let uplift_m = grid::resample_linear(input.uplift, cw, ch, mw, mh);
        let land_m = grid::resample_linear(input.land, cw, ch, mw, mh);
        let regions = (input.scale * o.region_frac).max(4.0) * mw as f32 / w as f32; // model px per region
        let (nx, ny) = (((mw as f32 / regions).round() as usize).clamp(2, mw / 4), ((mh as f32 / regions).round() as usize).clamp(2, mh / 4));
        let field = grid::value_noise(mw, mh, nx, ny, &mut rng);
        // HWC: the four channels of a pixel are adjacent.
        let mut cond = vec![0.0f32; 4 * np];
        let n = &model.norm;
        for i in 0..np {
            let up = (o.uplift_bias + o.uplift_gain * uplift_m[i]).clamp(0.0, 1.0);
            let rough = (o.rough_base + o.rough_uplift * uplift_m[i] + o.rough_noise * (field[i] - 0.5)).clamp(0.0, 1.0);
            cond[4 * i] = (coarse_m[i] - n.coarse_mean) / n.coarse_std;
            cond[4 * i + 1] = up * 2.0 - 1.0;
            cond[4 * i + 2] = land_m[i].clamp(0.0, 1.0) * 2.0 - 1.0;
            cond[4 * i + 3] = rough * 2.0 - 1.0;
        }

        let mut x = vec![0.0f32; np];
        rng.fill_normal(&mut x);
        let mut ws = Workspace::default();
        let cache = model.net.prepare(&cond, mh, mw, &mut ws);
        drop(cond);
        sampler::ddim(&model.net, &cache, &mut x, o.steps, &mut ws);
        drop(ws);

        let k = n.resid_std * o.gain;
        for v in &mut x {
            *v *= k;
        }
        let resid = grid::resample_cubic(&x, mw, mh, w, h);
        let coarse = grid::resample_cubic(input.coarse, cw, ch, w, h);
        Ok(finish(&coarse, &resid, o.coast_band))
    }
}

fn check(input: &DetailInput) -> Result<(), DetailError> {
    let (cd, d) = (input.coarse_dims, input.dims);
    let bad = |m: &str| Err(DetailError::BadInput(m.to_string()));
    if cd.n() == 0 || d.n() == 0 {
        return bad("empty dims");
    }
    if input.coarse.len() != cd.n() || input.uplift.len() != cd.n() || input.land.len() != cd.n() {
        return bad("coarse, uplift and land must be coarse_dims.w * coarse_dims.h");
    }
    if d.w < 8 || d.h < 8 {
        return bad("map smaller than 8x8");
    }
    if input.coarse.iter().any(|v| !v.is_finite()) {
        return bad("non-finite coarse heights");
    }
    Ok(())
}

/// coarse + residual, keeping the coarse land/sea layout outside the coast band, with a
/// soft ceiling (peaks ~1) and the -1 floor.
fn finish(coarse: &[f32], resid: &[f32], band: f32) -> Vec<f32> {
    coarse
        .iter()
        .zip(resid)
        .map(|(&c, &r)| {
            let mut v = c + r;
            if c > band {
                v = v.max(LAND_EPS);
            } else if c < -band {
                v = v.min(-SEA_EPS);
            }
            if v > 0.85 {
                v = 0.85 + 0.2 * ((v - 0.85) / 0.2).tanh();
            }
            v.max(-1.0)
        })
        .collect()
}

impl LearnedDiffusion {
    /// The hook body with a given procedural stage: procedural shaping on the coarse grid,
    /// then the learned detail; `procedural.detail` unchanged when there is no model or the
    /// input is unusable.
    pub fn detail_with(&self, procedural: &ProceduralDiffusion, input: &DetailInput, rng: &mut Rng) -> Vec<f32> {
        if self.model.is_none() || check(input).is_err() {
            return procedural.detail(input, rng);
        }
        let (shaped, _) = procedural.shape(input, rng);
        let seed = rng.next_u64();
        let shaped_in = DetailInput {
            coarse: &shaped,
            coarse_dims: input.coarse_dims,
            dims: input.dims,
            cell: input.cell,
            uplift: input.uplift,
            land: input.land,
            scale: input.scale,
        };
        match self.try_detail(&shaped_in, seed) {
            Ok(v) => v,
            Err(_) => procedural.upsample(input, &shaped, rng),
        }
    }
}

/// The hook with this instance's own `procedural` stage.
impl ReliefDetail for LearnedDiffusion {
    fn detail(&self, input: &DetailInput, rng: &mut Rng) -> Vec<f32> {
        self.detail_with(&self.procedural, input, rng)
    }
}

/// The hook for one preset: a shared model with that preset's procedural stage.
pub struct Relief<'a> {
    pub learned: &'a LearnedDiffusion,
    pub procedural: ProceduralDiffusion,
}

impl ReliefDetail for Relief<'_> {
    fn detail(&self, input: &DetailInput, rng: &mut Rng) -> Vec<f32> {
        self.learned.detail_with(&self.procedural, input, rng)
    }
}

/// Generate a map with the learned relief when `learned` holds a model, and with plain
/// `moc_terrain::generate` (exactly today's procedural relief) otherwise.
pub fn generate(p: &moc_terrain::TerrainParams, learned: Option<&LearnedDiffusion>) -> moc_terrain::TerrainMap {
    match learned {
        Some(l) if l.is_learned() => moc_terrain::generate_with(p, &Relief { learned: l, procedural: ProceduralDiffusion::from_params(p) }),
        _ => moc_terrain::generate(p),
    }
}
