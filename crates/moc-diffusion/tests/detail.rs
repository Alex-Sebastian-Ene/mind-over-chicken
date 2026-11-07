//! The whole detail pass: sizes, wrap, land/sea layout, randomness and the fallback.
//! Uses the tiny synthetic-weight model (untrained), so it checks plumbing, not looks.

use moc_diffusion::grid;
use moc_diffusion::rng::SplitMix64;
use moc_diffusion::{procedural_detail, DetailError, DetailInput, Dims, LearnedDiffusion};

const WEIGHTS: &[u8] = include_bytes!("fixtures/tiny.mocd");

struct Coarse {
    cw: usize,
    ch: usize,
    heights: Vec<f32>,
    uplift: Vec<f32>,
    land: Vec<f32>,
}

fn coarse(cw: usize, ch: usize, seed: u64) -> Coarse {
    let mut r = SplitMix64::new(seed);
    let heights: Vec<f32> = grid::fbm(cw, ch, 6, 4, 4, &mut r).iter().map(|v| v * 0.7 - 0.05).collect();
    let uplift: Vec<f32> = grid::fbm(cw, ch, 5, 3, 2, &mut r).iter().map(|v| (v * 1.5).clamp(0.0, 1.0)).collect();
    let land = heights.iter().map(|h| if *h > 0.0 { 1.0 } else { 0.0 }).collect();
    Coarse { cw, ch, heights, uplift, land }
}

fn input<'a>(c: &'a Coarse, cell: usize) -> DetailInput<'a> {
    DetailInput {
        coarse: &c.heights,
        coarse_dims: Dims::torus(c.cw, c.ch),
        dims: Dims::torus(c.cw * cell, c.ch * cell),
        cell,
        uplift: &c.uplift,
        land: &c.land,
        scale: (c.cw * cell) as f32 / 4.0,
    }
}

fn learned() -> LearnedDiffusion {
    let ld = LearnedDiffusion::from_bytes(WEIGHTS);
    assert!(ld.is_learned(), "fixture failed to load: {:?}", ld.load_error());
    ld
}

#[test]
fn model_grid_per_map_size() {
    let ld = learned();
    for (w, h, cell, want) in [(320, 180, 2, (320, 184)), (640, 360, 2, (640, 360)), (2560, 1280, 4, (1280, 640))] {
        let c = coarse(w / cell, h / cell, 1);
        assert_eq!(ld.model_grid(&input(&c, cell)), want, "{w}x{h}");
    }
}

#[test]
fn home_world_output_is_complete_and_in_range() {
    let ld = learned();
    let c = coarse(160, 90, 2);
    let inp = input(&c, 2);
    let out = ld.try_detail(&inp, 42).unwrap();
    assert_eq!(out.len(), 320 * 180);
    assert!(out.iter().all(|v| v.is_finite() && *v >= -1.0 && *v <= 1.06));
}

#[test]
fn keeps_the_coarse_land_sea_layout() {
    let ld = learned();
    let band = ld.opts.coast_band;
    let c = coarse(80, 48, 3);
    let inp = input(&c, 2);
    let out = ld.try_detail(&inp, 7).unwrap();
    let full = grid::resample_cubic(&c.heights, 80, 48, 160, 96);
    for (o, f) in out.iter().zip(&full) {
        if *f > band {
            assert!(*o > 0.0, "land became sea: coarse {f} -> {o}");
        } else if *f < -band {
            assert!(*o < 0.0, "sea became land: coarse {f} -> {o}");
        }
    }
}

#[test]
fn random_per_seed_and_repeatable() {
    let ld = learned();
    let c = coarse(40, 24, 4);
    let inp = input(&c, 2);
    let a = ld.try_detail(&inp, 1).unwrap();
    let b = ld.try_detail(&inp, 1).unwrap();
    let d = ld.try_detail(&inp, 2).unwrap();
    assert_eq!(a, b);
    let diff = a.iter().zip(&d).filter(|(x, y)| (*x - *y).abs() > 1e-4).count();
    assert!(diff > a.len() / 2, "seeds 1 and 2 gave nearly the same map ({diff} px differ)");
}

#[test]
fn no_seam_at_the_wrap() {
    // Steps across the wrap edges look like steps anywhere else.
    let ld = learned();
    let c = coarse(64, 40, 5);
    let inp = input(&c, 2);
    let (w, h) = (128, 80);
    let out = ld.try_detail(&inp, 9).unwrap();
    let at = |x: usize, y: usize| out[(y % h) * w + (x % w)];
    let col_step = |x: usize| (0..h).map(|y| (at(x + 1, y) - at(x, y)).abs()).sum::<f32>() / h as f32;
    let row_step = |y: usize| (0..w).map(|x| (at(x, y + 1) - at(x, y)).abs()).sum::<f32>() / w as f32;
    let interior_x = (0..w - 1).map(col_step).sum::<f32>() / (w - 1) as f32;
    let interior_y = (0..h - 1).map(row_step).sum::<f32>() / (h - 1) as f32;
    let (seam_x, seam_y) = (col_step(w - 1), row_step(h - 1));
    assert!(seam_x < 2.0 * interior_x, "x seam {seam_x} vs interior {interior_x}");
    assert!(seam_y < 2.0 * interior_y, "y seam {seam_y} vs interior {interior_y}");
}

#[test]
fn falls_back_when_weights_are_missing_or_broken() {
    let c = coarse(40, 24, 6);
    let inp = input(&c, 2);
    let want = procedural_detail(&inp, 3);
    for bytes in [&b""[..], &b"MOCDxxxxxxxxxxxx"[..], &WEIGHTS[..WEIGHTS.len() / 2]] {
        let ld = LearnedDiffusion::from_bytes(bytes);
        assert!(!ld.is_learned());
        assert!(ld.load_error().is_some());
        assert_eq!(ld.try_detail(&inp, 3), Err(DetailError::NoModel));
        assert_eq!(ld.detail(&inp, 3), want);
    }
    // A build without weights/relief.mocd embeds nothing and falls back too.
    let e = LearnedDiffusion::embedded();
    if moc_diffusion::EMBEDDED_WEIGHTS.is_none() {
        assert!(!e.is_learned());
        assert_eq!(e.detail(&inp, 3), want);
    }
}

#[test]
fn bad_input_falls_back_instead_of_panicking() {
    let ld = learned();
    let c = coarse(40, 24, 8);
    let mut inp = input(&c, 2);
    let short = vec![0.0f32; 10];
    inp.uplift = &short;
    assert!(matches!(ld.try_detail(&inp, 1), Err(DetailError::BadInput(_))));
    assert_eq!(ld.detail(&inp, 1).len(), 80 * 48);
}

#[test]
fn procedural_fallback_keeps_layout_and_wraps() {
    let c = coarse(80, 45, 10);
    let inp = input(&c, 2);
    let out = procedural_detail(&inp, 1);
    assert_eq!(out.len(), 160 * 90);
    let full = grid::resample_cubic(&c.heights, 80, 45, 160, 90);
    assert!(out.iter().zip(&full).all(|(o, f)| (*f <= 0.012 || *o > 0.0) && (*f >= -0.012 || *o < 0.0)));
}

#[test]
#[ignore = "needs MOCD_WEIGHTS=/path/to/relief.mocd (e.g. from rl/terrain_diffusion/smoke_test.py)"]
fn loads_exported_weights() {
    let path = std::env::var("MOCD_WEIGHTS").expect("set MOCD_WEIGHTS");
    let ld = LearnedDiffusion::from_bytes(&std::fs::read(&path).unwrap());
    assert!(ld.is_learned(), "{path}: {:?}", ld.load_error());
    let c = coarse(160, 90, 11);
    let t = std::time::Instant::now();
    let out = ld.try_detail(&input(&c, 2), 1).unwrap();
    eprintln!("{path}: steps {}, home world in {:.0} ms", ld.opts.steps, t.elapsed().as_secs_f64() * 1e3);
    assert!(out.iter().all(|v| v.is_finite()));
}
