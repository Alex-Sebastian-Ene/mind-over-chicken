//! Pure-Rust multilayer perceptron (MLP) policy runtime for PPO actor models.
//!
//! Evaluates the trained PPO actor policy exported from Stable-Baselines3 (`src/ai/stone_policy.json`).
//! Zero dependencies beyond Serde for JSON parsing.

use serde::Deserialize;

pub const DEFAULT_POLICY_JSON: &str = include_str!("../../../src/ai/stone_policy.json");
pub const NUM_INPUTS: usize = 20;
pub const NUM_ACTIONS: usize = 5;

pub const ACTION_NAMES: [&str; NUM_ACTIONS] = [
    "wait",
    "strike_fire",
    "strike_mammoth",
    "calm",
    "strike_random",
];

pub const OBS_NAMES: [&str; NUM_INPUTS] = [
    "fire_lit",
    "fire_heat",
    "food",
    "alive",
    "hunger",
    "starving",
    "fear",
    "hold_active",
    "hold_progress",
    "holds",
    "mammoth_dist",
    "mammoths",
    "colonies",
    "rafts",
    "overseas",
    "time",
    "calm",
    "calm_margin",
    "bolt_ready",
    "meat",
];

#[derive(Debug, Clone, Deserialize)]
pub struct LayerSpec {
    pub w: Vec<Vec<f32>>,
    pub b: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckSpec {
    pub obs: Vec<Vec<f32>>,
    pub probs: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicySpec {
    pub format: String,
    pub game: String,
    pub scene: String,
    pub obs_version: u32,
    pub obs_names: Vec<String>,
    pub actions: Vec<String>,
    pub decision_dt: f32,
    pub activation: String,
    pub layers: Vec<LayerSpec>,
    pub check: Option<CheckSpec>,
}

#[derive(Debug, Clone)]
pub struct MLPPolicy {
    pub spec: PolicySpec,
}

impl Default for MLPPolicy {
    fn default() -> Self {
        Self::from_embedded()
    }
}

impl MLPPolicy {
    /// Load policy from the embedded `stone_policy.json` actor weights
    pub fn from_embedded() -> Self {
        Self::from_json(DEFAULT_POLICY_JSON).expect("Embedded stone_policy.json must be valid")
    }

    /// Load policy from a JSON string
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let spec: PolicySpec = serde_json::from_str(json_str)?;
        Ok(Self { spec })
    }

    #[inline]
    fn activate(&self, x: f32) -> f32 {
        match self.spec.activation.as_str() {
            "tanh" => x.tanh(),
            "relu" => {
                if x > 0.0 {
                    x
                } else {
                    0.0
                }
            }
            "identity" => x,
            _ => x.tanh(),
        }
    }

    /// Compute unnormalized linear logits for the given 20 observation inputs
    pub fn logits(&self, obs: &[f32]) -> [f32; NUM_ACTIONS] {
        let mut h: Vec<f32> = obs.to_vec();
        let num_layers = self.spec.layers.len();

        for (l_idx, layer) in self.spec.layers.iter().enumerate() {
            let is_last = l_idx == num_layers - 1;
            let mut out = vec![0.0f32; layer.b.len()];

            for (i, row) in layer.w.iter().enumerate() {
                let mut sum = layer.b[i];
                for (j, &w_ij) in row.iter().enumerate() {
                    sum += w_ij * h[j];
                }
                out[i] = if is_last { sum } else { self.activate(sum) };
            }
            h = out;
        }

        let mut res = [0.0f32; NUM_ACTIONS];
        for i in 0..NUM_ACTIONS.min(h.len()) {
            res[i] = h[i];
        }
        res
    }

    /// Compute normalized softmax probability distribution over all actions
    pub fn probs(&self, obs: &[f32]) -> [f32; NUM_ACTIONS] {
        let z = self.logits(obs);
        let mut max_z = -f32::INFINITY;
        for &val in &z {
            if val > max_z {
                max_z = val;
            }
        }

        let mut e = [0.0f32; NUM_ACTIONS];
        let mut sum = 0.0f32;
        for i in 0..NUM_ACTIONS {
            e[i] = (z[i] - max_z).exp();
            sum += e[i];
        }

        let mut out = [0.0f32; NUM_ACTIONS];
        for i in 0..NUM_ACTIONS {
            out[i] = e[i] / sum;
        }
        out
    }

    /// Select action (greedy argmax or stochastic sampling)
    pub fn act(&self, obs: &[f32], greedy: bool, sample_val: f32) -> usize {
        let z = self.logits(obs);
        if greedy {
            let mut best_idx = 0;
            let mut best_val = -f32::INFINITY;
            for (i, &val) in z.iter().enumerate() {
                if val > best_val {
                    best_val = val;
                    best_idx = i;
                }
            }
            best_idx
        } else {
            let p = self.probs(obs);
            let mut u = sample_val;
            for (i, &prob) in p.iter().enumerate() {
                u -= prob;
                if u <= 0.0 {
                    return i;
                }
            }
            NUM_ACTIONS - 1
        }
    }

    /// Self-check against PyTorch export test vectors embedded in the JSON.
    /// Returns the maximum absolute difference between computed and reference probabilities.
    pub fn self_check(&self) -> f32 {
        let check = match &self.spec.check {
            Some(c) => c,
            None => return 0.0,
        };

        let mut worst_err = 0.0f32;
        for (k, obs) in check.obs.iter().enumerate() {
            let computed_probs = self.probs(obs);
            let expected_probs = &check.probs[k];

            for (i, &comp) in computed_probs.iter().enumerate() {
                let exp = expected_probs[i];
                let err = (comp - exp).abs();
                if err > worst_err {
                    worst_err = err;
                }
            }
        }
        worst_err
    }
}
