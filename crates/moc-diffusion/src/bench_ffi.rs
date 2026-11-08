//! Plain extern "C" exports for the wasm timing harness (`tools/bench_wasm.mjs`); only
//! built with `--features wasm-bench`. Random weights: timings do not depend on values.

use crate::grid;
use crate::rng::SplitMix64;
use crate::unet::Arch;
use crate::{DetailInput, Dims, LearnedDiffusion};
use std::cell::RefCell;

thread_local! {
    static MODEL: RefCell<Option<LearnedDiffusion>> = const { RefCell::new(None) };
}

/// Builds a random-weight model of the given widths. Returns its parameter count.
#[no_mangle]
pub extern "C" fn mocd_bench_setup(c0: u32, c1: u32, c2: u32, mid: u32) -> u32 {
    let arch = Arch { n_cond: 4, widths: [c0 as usize, c1 as usize, c2 as usize], mid_blocks: mid as usize, time_freqs: 16, time_dim: 64 };
    let m = LearnedDiffusion::random_for_timing(arch, 1);
    let n = m.model().map_or(0, |m| m.net.param_count());
    MODEL.with(|c| *c.borrow_mut() = Some(m));
    n as u32
}

/// Details a synthetic w x h map (coarse cell `cell`) in `steps` DDIM steps. Returns the
/// mean output height, so the work cannot be optimised away.
#[no_mangle]
pub extern "C" fn mocd_bench_run(w: u32, h: u32, cell: u32, steps: u32, seed: u32) -> f32 {
    let (w, h, cell) = (w as usize, h as usize, cell as usize);
    let (cw, ch) = (w / cell, h / cell);
    let mut r = SplitMix64::new(seed as u64);
    let coarse: Vec<f32> = grid::fbm(cw, ch, 6, 3, 4, &mut r).iter().map(|v| v * 0.8 - 0.1).collect();
    let uplift: Vec<f32> = coarse.iter().map(|v| (v * 2.0).clamp(0.0, 1.0)).collect();
    let land: Vec<f32> = coarse.iter().map(|v| if *v > 0.0 { 1.0 } else { 0.0 }).collect();
    let input = DetailInput { coarse: &coarse, coarse_dims: Dims::torus(cw, ch), dims: Dims::torus(w, h), cell, uplift: &uplift, land: &land, scale: w as f32 / 4.0 };
    MODEL.with(|c| {
        let mut g = c.borrow_mut();
        let m = g.as_mut().expect("call mocd_bench_setup first");
        m.opts.steps = steps as usize;
        let out = m.try_detail(&input, seed as u64).expect("bench input is valid");
        out.iter().sum::<f32>() / out.len() as f32
    })
}
