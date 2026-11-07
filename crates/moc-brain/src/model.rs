//! Softmax regression personal model for brain state classification.
//!
//! Evaluates band-power features with blocked 4-fold cross validation.
//! Accurately distinguishes Rest, Calm, and Focus states tailored to each individual.

use crate::math::{clamp, EPS};

pub const NUM_FEATURES: usize = 7;
pub const MAX_PER_CLASS: usize = 600;
pub const MIN_PER_CLASS: usize = 15;
pub const TRAIN_CAP: usize = 120;
pub const NUM_CLASSES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    Rest = 0,
    Calm = 1,
    Focus = 2,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Rest, Class::Calm, Class::Focus];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Class::Rest => "rest",
            Class::Calm => "calm",
            Class::Focus => "focus",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "rest" => Some(Class::Rest),
            "calm" => Some(Class::Calm),
            "focus" => Some(Class::Focus),
            _ => None,
        }
    }
}

pub const FEATURE_NAMES: [&str; NUM_FEATURES] = [
    "ln δ", "ln θ", "ln α", "ln β", "ln γ", "ln α/β", "ln β/(α+θ)",
];

/// Relative EEG band powers (typically sum to ~1.0).
#[derive(Debug, Clone, Copy, Default)]
pub struct Bands {
    pub delta: f32,
    pub theta: f32,
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,
}

/// Compute the 7 log-band-power features from relative band powers.
pub fn features_from_bands(b: &Bands) -> [f32; NUM_FEATURES] {
    let d = b.delta + EPS;
    let t = b.theta + EPS;
    let a = b.alpha + EPS;
    let be = b.beta + EPS;
    let g = b.gamma + EPS;

    [
        d.ln(),
        t.ln(),
        a.ln(),
        be.ln(),
        g.ln(),
        (a / be).ln(),
        (be / (a + t)).ln(),
    ]
}

#[derive(Debug, Clone)]
struct SampleEntry {
    f: [f32; NUM_FEATURES],
    class_idx: usize,
    sample_idx: usize,
}

#[derive(Debug, Clone)]
pub struct FittedModel {
    pub w: Vec<[f32; NUM_FEATURES]>,
    pub b: Vec<f32>,
    pub mu: [f32; NUM_FEATURES],
    pub sd: [f32; NUM_FEATURES],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Predictions {
    pub rest: Option<f32>,
    pub calm: Option<f32>,
    pub focus: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct PersonalModel {
    samples: [Vec<[f32; NUM_FEATURES]>; NUM_CLASSES],
    trained: bool,
    active_classes: Vec<Class>,
    fitted: Option<FittedModel>,
    pub accuracy: f32,
    pub chance: f32,
    pub new_since_train: usize,
    pub train_count: usize,
    pub last_train_ms: f64,
}

impl Default for PersonalModel {
    fn default() -> Self {
        Self::new()
    }
}

impl PersonalModel {
    pub fn new() -> Self {
        Self {
            samples: [Vec::new(), Vec::new(), Vec::new()],
            trained: false,
            active_classes: Vec::new(),
            fitted: None,
            accuracy: 0.0,
            chance: 0.0,
            new_since_train: 0,
            train_count: 0,
            last_train_ms: 0.0,
        }
    }

    pub fn add(&mut self, label: Class, f: [f32; NUM_FEATURES]) {
        let arr = &mut self.samples[label.index()];
        arr.push(f);
        if arr.len() > MAX_PER_CLASS {
            arr.remove(0);
        }
        self.new_since_train += 1;
    }

    pub fn count(&self, label: Class) -> usize {
        self.samples[label.index()].len()
    }

    pub fn total(&self) -> usize {
        self.samples.iter().map(|arr| arr.len()).sum()
    }

    pub fn has(&self, label: Class) -> bool {
        self.trained && self.active_classes.contains(&label)
    }

    pub fn is_trained(&self) -> bool {
        self.trained
    }

    pub fn active_classes(&self) -> &[Class] {
        &self.active_classes
    }

    pub fn maybe_train(&mut self) -> bool {
        if self.new_since_train < 10 {
            return false;
        }
        self.train()
    }

    pub fn train(&mut self) -> bool {
        let classes: Vec<Class> = Class::ALL
            .iter()
            .copied()
            .filter(|&c| self.count(c) >= MIN_PER_CLASS)
            .collect();

        if classes.len() < 2 {
            return false;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let start = std::time::Instant::now();

        // Downsample evenly strided subset (<= TRAIN_CAP per class)
        let mut data: Vec<SampleEntry> = Vec::new();
        let mut class_sub_counts: Vec<usize> = Vec::with_capacity(classes.len());

        for (k, &c) in classes.iter().enumerate() {
            let arr = &self.samples[c.index()];
            let stride = 1.max((arr.len() + TRAIN_CAP - 1) / TRAIN_CAP);
            let mut picked = Vec::new();
            let mut i = 0;
            while i < arr.len() {
                picked.push(arr[i]);
                i += stride;
            }
            class_sub_counts.push(picked.len());
            for (sample_idx, &f) in picked.iter().enumerate() {
                data.push(SampleEntry {
                    f,
                    class_idx: k,
                    sample_idx,
                });
            }
        }

        // Blocked 4-fold cross-validation:
        const FOLDS: usize = 4;
        let mut correct = 0usize;
        let mut tested = 0usize;

        for fold in 0..FOLDS {
            let mut train_fold = Vec::new();
            let mut test_fold = Vec::new();

            for d in &data {
                let n = class_sub_counts[d.class_idx];
                let chunk = (FOLDS - 1).min((d.sample_idx * FOLDS) / n);
                if chunk == fold {
                    test_fold.push(d.clone());
                } else {
                    train_fold.push(d.clone());
                }
            }

            if train_fold.is_empty() || test_fold.is_empty() {
                continue;
            }

            let m = fit(&train_fold, classes.len());
            for d in &test_fold {
                let probs = predict_raw(&m, &d.f);
                if argmax(&probs) == d.class_idx {
                    correct += 1;
                }
                tested += 1;
            }
        }

        let final_model = fit(&data, classes.len());
        self.accuracy = if tested > 0 {
            correct as f32 / tested as f32
        } else {
            0.0
        };
        self.chance = 1.0 / classes.len() as f32;
        self.fitted = Some(final_model);
        self.active_classes = classes;
        self.trained = true;
        self.new_since_train = 0;
        self.train_count += 1;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.last_train_ms = start.elapsed().as_secs_f64() * 1000.0;
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.last_train_ms = 0.5;
        }

        true
    }

    pub fn predict(&self, f: &[f32; NUM_FEATURES]) -> Option<Predictions> {
        let m = self.fitted.as_ref()?;
        let p = predict_raw(m, f);
        let mut out = Predictions::default();
        for (k, &c) in self.active_classes.iter().enumerate() {
            match c {
                Class::Rest => out.rest = Some(p[k]),
                Class::Calm => out.calm = Some(p[k]),
                Class::Focus => out.focus = Some(p[k]),
            }
        }
        Some(out)
    }

    /// Trust factor in 0..1 for the given class
    pub fn trust(&self, label: Class) -> f32 {
        if !self.has(label) {
            return 0.0;
        }
        let skill = clamp((self.accuracy - self.chance - 0.1) / 0.3, 0.0, 1.0);
        let amount = clamp(self.count(label) as f32 / 60.0, 0.0, 1.0);
        skill * amount
    }

    /// Per-class weights in original feature units for debugging / overlay
    pub fn weights(&self) -> Vec<(Class, [f32; NUM_FEATURES])> {
        let m = match self.fitted.as_ref() {
            Some(m) => m,
            None => return Vec::new(),
        };

        self.active_classes
            .iter()
            .enumerate()
            .map(|(k, &c)| {
                let mut scaled = [0.0f32; NUM_FEATURES];
                for j in 0..NUM_FEATURES {
                    scaled[j] = m.w[k][j] / m.sd[j];
                }
                (c, scaled)
            })
            .collect()
    }
}

fn fit(data: &[SampleEntry], num_k: usize) -> FittedModel {
    let d_len = NUM_FEATURES;
    let n_samples = data.len();
    assert!(n_samples > 0);

    let mut mu = [0.0f32; NUM_FEATURES];
    let mut sd = [0.0f32; NUM_FEATURES];

    for d in data {
        for j in 0..d_len {
            mu[j] += d.f[j];
        }
    }
    for j in 0..d_len {
        mu[j] /= n_samples as f32;
    }

    for d in data {
        for j in 0..d_len {
            let diff = d.f[j] - mu[j];
            sd[j] += diff * diff;
        }
    }
    for j in 0..d_len {
        let s = (sd[j] / n_samples as f32).sqrt();
        sd[j] = if s > 1e-6 { s } else { 1.0 };
    }

    // Normalized X
    let mut x_norm: Vec<[f32; NUM_FEATURES]> = Vec::with_capacity(n_samples);
    for d in data {
        let mut x = [0.0f32; NUM_FEATURES];
        for j in 0..d_len {
            x[j] = (d.f[j] - mu[j]) / sd[j];
        }
        x_norm.push(x);
    }

    // Class weights
    let mut counts = vec![0usize; num_k];
    for d in data {
        counts[d.class_idx] += 1;
    }
    let cw: Vec<f32> = counts
        .iter()
        .map(|&cnt| n_samples as f32 / (num_k as f32 * cnt.max(1) as f32))
        .collect();

    let mut w = vec![[0.0f32; NUM_FEATURES]; num_k];
    let mut b = vec![0.0f32; num_k];
    let lr = 0.8f32;
    let l2 = 0.02f32;
    let iters = 90;
    let mut p = vec![0.0f32; num_k];

    for _ in 0..iters {
        let mut gw = vec![[0.0f32; NUM_FEATURES]; num_k];
        let mut gb = vec![0.0f32; num_k];

        for n in 0..n_samples {
            let x = &x_norm[n];
            let mut max_z = -f32::INFINITY;
            for k in 0..num_k {
                let mut z = b[k];
                for j in 0..d_len {
                    z += w[k][j] * x[j];
                }
                p[k] = z;
                if z > max_z {
                    max_z = z;
                }
            }

            let mut sum_exp = 0.0f32;
            for k in 0..num_k {
                p[k] = (p[k] - max_z).exp();
                sum_exp += p[k];
            }

            let weight = cw[data[n].class_idx];
            let target_k = data[n].class_idx;

            for k in 0..num_k {
                let prob = p[k] / sum_exp;
                let target_indicator = if target_k == k { 1.0 } else { 0.0 };
                let g = (prob - target_indicator) * weight;
                gb[k] += g;
                for j in 0..d_len {
                    gw[k][j] += g * x[j];
                }
            }
        }

        let n_f32 = n_samples as f32;
        for k in 0..num_k {
            b[k] -= (lr * gb[k]) / n_f32;
            for j in 0..d_len {
                w[k][j] -= lr * (gw[k][j] / n_f32 + l2 * w[k][j]);
            }
        }
    }

    FittedModel { w, b, mu, sd }
}

fn predict_raw(m: &FittedModel, f: &[f32; NUM_FEATURES]) -> Vec<f32> {
    let num_k = m.w.len();
    let mut z = vec![0.0f32; num_k];
    let mut max_z = -f32::INFINITY;

    for k in 0..num_k {
        let mut s = m.b[k];
        for j in 0..NUM_FEATURES {
            s += m.w[k][j] * ((f[j] - m.mu[j]) / m.sd[j]);
        }
        z[k] = s;
        if s > max_z {
            max_z = s;
        }
    }

    let mut sum_exp = 0.0f32;
    for k in 0..num_k {
        z[k] = (z[k] - max_z).exp();
        sum_exp += z[k];
    }
    for k in 0..num_k {
        z[k] /= sum_exp;
    }
    z
}

fn argmax(slice: &[f32]) -> usize {
    let mut best_idx = 0;
    let mut best_val = -f32::INFINITY;
    for (i, &val) in slice.iter().enumerate() {
        if val > best_val {
            best_val = val;
            best_idx = i;
        }
    }
    best_idx
}
