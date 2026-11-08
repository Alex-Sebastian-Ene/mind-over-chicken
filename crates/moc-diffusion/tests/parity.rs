//! The Rust runtime against PyTorch, on the synthetic-weight fixtures written by
//! `rl/terrain_diffusion/make_fixtures.py` (f16 weights, circular padding).

use moc_diffusion::format;
use moc_diffusion::sampler;
use moc_diffusion::unet::{ReliefUNet, Workspace};

const WEIGHTS: &[u8] = include_bytes!("fixtures/tiny.mocd");
const REF: &[u8] = include_bytes!("fixtures/tiny_ref.mocd");

struct Ref {
    h: usize,
    w: usize,
    x_t: Vec<f32>,
    cond_hwc: Vec<f32>,
    t: f32,
    v: Vec<f32>,
    noise: Vec<f32>,
    x0: Vec<f32>,
    steps: usize,
}

fn load_ref() -> Ref {
    let r = format::parse(REF).unwrap();
    let get = |n: &str| r.tensor(n).unwrap().clone();
    let x_t = get("x_t");
    let (h, w) = (x_t.shape[0], x_t.shape[1]);
    let cond = get("cond"); // CHW from PyTorch
    let c = cond.shape[0];
    let mut cond_hwc = vec![0.0; c * h * w];
    for ch in 0..c {
        for i in 0..h * w {
            cond_hwc[i * c + ch] = cond.data[ch * h * w + i];
        }
    }
    Ref {
        h,
        w,
        x_t: x_t.data,
        cond_hwc,
        t: get("t").data[0],
        v: get("v").data,
        noise: get("noise").data,
        x0: get("ddim_x0").data,
        steps: r.header.get("ddim_steps").and_then(|j| j.as_usize()).unwrap(),
    }
}

fn max_err(a: &[f32], b: &[f32]) -> (f32, f32) {
    assert_eq!(a.len(), b.len());
    let err = a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max);
    let scale = b.iter().map(|x| x.abs()).fold(0.0, f32::max);
    (err, scale)
}

#[test]
fn forward_matches_pytorch() {
    let net = ReliefUNet::from_mocd(&format::parse(WEIGHTS).unwrap()).unwrap();
    let r = load_ref();
    let mut ws = Workspace::default();
    let cache = net.prepare(&r.cond_hwc, r.h, r.w, &mut ws);
    let mut v = vec![0.0; r.h * r.w];
    net.forward(&r.x_t, &cache, r.t, &mut ws, &mut v);
    let (err, scale) = max_err(&v, &r.v);
    eprintln!("forward: max abs error {err:.2e}, output scale {scale:.3}");
    assert!(err < 1e-4 * scale.max(1.0), "max |rust - torch| = {err} (output scale {scale})");
}

#[test]
fn ddim_matches_pytorch() {
    let net = ReliefUNet::from_mocd(&format::parse(WEIGHTS).unwrap()).unwrap();
    let r = load_ref();
    let mut ws = Workspace::default();
    let cache = net.prepare(&r.cond_hwc, r.h, r.w, &mut ws);
    let mut x = r.noise.clone();
    sampler::ddim(&net, &cache, &mut x, r.steps, &mut ws);
    let (err, scale) = max_err(&x, &r.x0);
    eprintln!("ddim: max abs error {err:.2e}, output scale {scale:.3}");
    assert!(err < 3e-4 * scale.max(1.0), "max |rust - torch| = {err} (output scale {scale})");
}

#[test]
fn the_wrap_is_not_a_seam() {
    // Rolling the input by a multiple of the model stride rolls the output exactly: the
    // wrap-around edge is computed like any interior pixel, so the torus has no seam.
    let net = ReliefUNet::from_mocd(&format::parse(WEIGHTS).unwrap()).unwrap();
    let r = load_ref();
    let (h, w, c) = (r.h, r.w, 4);
    let (dy, dx) = (8, 16);
    let roll = |src: &[f32], ch: usize| {
        let mut out = vec![0.0; src.len()];
        for y in 0..h {
            for x in 0..w {
                let (ty, tx) = ((y + dy) % h, (x + dx) % w);
                out[(ty * w + tx) * ch..(ty * w + tx + 1) * ch].copy_from_slice(&src[(y * w + x) * ch..(y * w + x + 1) * ch]);
            }
        }
        out
    };
    let run = |x: &[f32], cond: &[f32]| {
        let mut ws = Workspace::default();
        let cache = net.prepare(cond, h, w, &mut ws);
        let mut v = vec![0.0; h * w];
        net.forward(x, &cache, 0.4, &mut ws, &mut v);
        v
    };
    let a = roll(&run(&r.x_t, &r.cond_hwc), 1);
    let b = run(&roll(&r.x_t, 1), &roll(&r.cond_hwc, c));
    let (err, _) = max_err(&a, &b);
    assert!(err < 1e-5, "rolled output differs by {err}");
}
