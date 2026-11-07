//! Few-step DDIM (eta = 0) on the cosine VP schedule with v-prediction, as
//! `rl/terrain_diffusion/diffusion.py::ddim_sample`.

use crate::unet::{CondCache, ReliefUNet, Workspace};

pub fn alpha_sigma(t: f32) -> (f32, f32) {
    let a = 0.5 * std::f32::consts::PI * t;
    (a.cos(), a.sin())
}

/// Runs `steps` DDIM steps from `x` (pure noise at t = 1) to t = 0, in place.
pub fn ddim(net: &ReliefUNet, cond: &CondCache, x: &mut [f32], steps: usize, ws: &mut Workspace) {
    let steps = steps.max(1);
    let mut v = vec![0.0f32; x.len()];
    for k in 0..steps {
        let t = (1.0 - k as f64 / steps as f64) as f32;
        let t2 = (1.0 - (k + 1) as f64 / steps as f64) as f32;
        net.forward(x, cond, t, ws, &mut v);
        let (a, s) = alpha_sigma(t);
        let (a2, s2) = alpha_sigma(t2);
        for (xi, &vi) in x.iter_mut().zip(&v) {
            let x0 = a * *xi - s * vi;
            let eps = s * *xi + a * vi;
            *xi = a2 * x0 + s2 * eps;
        }
    }
}
