//! Micro-benchmark of the conv kernels: `cargo run --release -p moc-diffusion --example kernels`
use moc_diffusion::nn::{self, Conv};
use std::time::Instant;

fn main() {
    for (c, h, w) in [(24usize, 92usize, 160usize), (32, 46, 80), (48, 23, 40)] {
        let wt: Vec<f32> = (0..c * c * 9).map(|i| ((i * 37 % 17) as f32 - 8.0) / 50.0).collect();
        let b = vec![0.01f32; c];
        let k = Conv::from_torch(&wt, Some(&b), 3, c, c, 0, c);
        let x: Vec<f32> = (0..c * h * w).map(|i| ((i * 13 % 23) as f32 - 11.0) / 7.0).collect();
        let mut pad = vec![0.0; c * (h + 2) * (w + 2)];
        let mut out = vec![0.0; c * h * w];
        let ones = vec![1.0; c];
        let reps = 50;
        let t = Instant::now();
        for _ in 0..reps {
            nn::pad_wrap(&x, c, h, w, true, &mut pad);
        }
        let tp = t.elapsed().as_secs_f64() / reps as f64;
        let t = Instant::now();
        for _ in 0..reps {
            nn::conv(&pad, h, w, &k, &ones, &b, Some(&x), &mut out);
        }
        let tc = t.elapsed().as_secs_f64() / reps as f64;
        let macs = (9 * c * c * h * w) as f64;
        let w1: Vec<f32> = (0..c * 4 * c).map(|i| ((i * 7 % 11) as f32 - 5.0) / 40.0).collect();
        let k1 = Conv::from_torch(&w1, Some(&b), 1, c, 4 * c, 0, 4 * c);
        let (hq, wq) = (h / 2, w / 2);
        let inp = vec![0.5f32; 4 * c * hq * wq];
        let mut o1 = vec![0.0f32; c * hq * wq];
        let t = Instant::now();
        for _ in 0..reps {
            nn::conv_bias(&inp, hq, wq, &k1, &mut o1);
        }
        let t1 = t.elapsed().as_secs_f64() / reps as f64;
        println!(
            "c{c} {h}x{w}: pad+silu {:.2} ms, conv3x3 {:.2} ms = {:.1} GMAC/s, conv1x1 {:.1} GMAC/s",
            tp * 1e3,
            tc * 1e3,
            macs / tc / 1e9,
            (4 * c * c * hq * wq) as f64 / t1 / 1e9
        );
    }
}
