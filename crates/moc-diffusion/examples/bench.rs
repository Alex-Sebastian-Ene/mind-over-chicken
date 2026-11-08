//! Native timings of the full detail pass (random weights of the shipping size).
//!
//!     cargo run --release -p moc-diffusion --example bench -- [steps] [c0 c1 c2]
//!
//! The browser numbers come from `tools/bench_wasm.mjs` (same code compiled to wasm).
use moc_diffusion::grid;
use moc_diffusion::rng::SplitMix64;
use moc_diffusion::unet::Arch;
use moc_diffusion::{DetailInput, Dims, LearnedDiffusion};
use std::time::Instant;

fn main() {
    let args: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let steps = args.first().copied().unwrap_or(6);
    let widths = if args.len() >= 4 { [args[1], args[2], args[3]] } else { [24, 32, 48] };
    let arch = Arch { n_cond: 4, widths, mid_blocks: 2, time_freqs: 16, time_dim: 64 };
    let mut ld = LearnedDiffusion::random_for_timing(arch.clone(), 1);
    ld.opts.steps = steps;
    let params = ld.model().unwrap().net.param_count();
    println!("widths {widths:?}  params {params}  MACs/px/step {:.0}  steps {steps}", arch.macs_per_pixel());
    for (name, w, h, cell) in [("home world", 320, 180, 2), ("galaxy surface", 640, 360, 2), ("civ planet", 2560, 1280, 4)] {
        let (cw, ch) = (w / cell, h / cell);
        let mut r = SplitMix64::new(3);
        let coarse: Vec<f32> = grid::fbm(cw, ch, 6, 3, 4, &mut r).iter().map(|v| v * 0.8 - 0.1).collect();
        let uplift: Vec<f32> = coarse.iter().map(|v| (v * 2.0).clamp(0.0, 1.0)).collect();
        let land: Vec<f32> = coarse.iter().map(|v| if *v > 0.0 { 1.0 } else { 0.0 }).collect();
        let input = DetailInput { coarse: &coarse, coarse_dims: Dims::torus(cw, ch), dims: Dims::torus(w, h), cell, uplift: &uplift, land: &land, scale: w as f32 / 4.0 };
        let (mw, mh) = ld.model_grid(&input);
        let reps = if w * h > 1_000_000 { 1 } else { 3 };
        let mut best = f64::MAX;
        for k in 0..reps {
            let t = Instant::now();
            let out = ld.try_detail(&input, k as u64).unwrap();
            best = best.min(t.elapsed().as_secs_f64());
            assert_eq!(out.len(), w * h);
        }
        let gmac = arch.macs_per_pixel() * (mw * mh) as f64 * steps as f64 / 1e9;
        println!("{name:15} {w}x{h}  model grid {mw}x{mh}  {:.0} ms  ({gmac:.1} GMAC, {:.1} GMAC/s)", best * 1e3, gmac / best);
    }
}
