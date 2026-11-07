//! Side-by-side images: procedural detail vs the learned model, per map size.
//!
//!     cargo run --release -p moc-diffusion --example compare -- <weights.mocd> <out_dir> [seed]
//!
//! Until moc-terrain's hook lands, the coarse maps are a synthetic stand-in (fBm continents
//! with uplift along ridges); after that this should take the real coarse stage.
use moc_diffusion::grid;
use moc_diffusion::rng::SplitMix64;
use moc_diffusion::{procedural_detail, DetailInput, Dims, LearnedDiffusion};
use std::time::Instant;

fn colour(h: f32) -> [f32; 3] {
    const STOPS: [(f32, [f32; 3]); 9] = [
        (-1.0, [8.0, 24.0, 68.0]),
        (-0.3, [22.0, 64.0, 128.0]),
        (-0.02, [64.0, 132.0, 186.0]),
        (0.0, [196.0, 186.0, 140.0]),
        (0.08, [92.0, 148.0, 72.0]),
        (0.3, [138.0, 160.0, 84.0]),
        (0.5, [150.0, 126.0, 88.0]),
        (0.75, [128.0, 112.0, 104.0]),
        (1.0, [240.0, 240.0, 244.0]),
    ];
    let h = h.clamp(-1.0, 1.0);
    for w in STOPS.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if h <= b {
            let t = (h - a) / (b - a);
            return [0, 1, 2].map(|i| ca[i] + (cb[i] - ca[i]) * t);
        }
    }
    STOPS[8].1
}

fn render(h: &[f32], w: usize, ht: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * ht * 3);
    for y in 0..ht {
        for x in 0..w {
            let v = h[y * w + x];
            let e = h[y * w + (x + 1) % w] - h[y * w + (x + w - 1) % w];
            let n = h[((y + 1) % ht) * w + x] - h[((y + ht - 1) % ht) * w + x];
            let light = if v > 0.0 { (1.0 - (e + n) * 6.0).clamp(0.55, 1.35) } else { 1.0 };
            out.extend(colour(v).map(|c| (c * light).clamp(0.0, 255.0) as u8));
        }
    }
    out
}

/// Minimal PNG (RGB8, zlib with stored blocks: big but dependency-free).
fn png(path: &std::path::Path, rgb: &[u8], w: usize, h: usize) {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for y in 0..h {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        z.push(last as u8);
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |tag: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut td = tag.to_vec();
        td.extend_from_slice(data);
        out.extend_from_slice(&td);
        out.extend_from_slice(&crc(&td).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    std::fs::write(path, out).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let weights = std::fs::read(args.first().expect("usage: compare <weights.mocd> <out_dir> [seed]")).unwrap_or_default();
    let out_dir = std::path::PathBuf::from(args.get(1).map(String::as_str).unwrap_or("."));
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64);
    std::fs::create_dir_all(&out_dir).unwrap();
    let ld = LearnedDiffusion::from_bytes(&weights);
    println!("learned: {} ({:?}), steps {}, seed {seed}", ld.is_learned(), ld.load_error(), ld.opts.steps);
    for (name, w, h, cell) in [("home", 320, 180, 2), ("galaxy", 640, 360, 2), ("civ", 2560, 1280, 4)] {
        let (cw, ch) = (w / cell, h / cell);
        let mut r = SplitMix64::new(seed ^ w as u64);
        let base = grid::fbm(cw, ch, 5, 3, 5, &mut r);
        let ridge = grid::fbm(cw, ch, 9, 5, 3, &mut r);
        let coarse: Vec<f32> = base.iter().zip(&ridge).map(|(b, q)| b * 0.9 - 0.08 + (0.25 - q.abs()).max(0.0) * (b * 3.0).clamp(0.0, 1.0)).collect();
        let uplift: Vec<f32> = ridge.iter().zip(&coarse).map(|(q, c)| ((0.25 - q.abs()) * 4.0).clamp(0.0, 1.0) * (*c > 0.0) as u8 as f32).collect();
        let land: Vec<f32> = coarse.iter().map(|c| (*c > 0.0) as u8 as f32).collect();
        let input = DetailInput { coarse: &coarse, coarse_dims: Dims::torus(cw, ch), dims: Dims::torus(w, h), cell, uplift: &uplift, land: &land, scale: w as f32 / 4.0 };
        let t = Instant::now();
        let proc_ = procedural_detail(&input, seed);
        let tp = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let learned = ld.detail(&input, seed);
        let tl = t.elapsed().as_secs_f64();
        let (a, b) = (render(&proc_, w, h), render(&learned, w, h));
        let mut both = Vec::with_capacity(a.len() * 2);
        for y in 0..h {
            both.extend_from_slice(&a[y * w * 3..(y + 1) * w * 3]);
            both.extend_from_slice(&b[y * w * 3..(y + 1) * w * 3]);
        }
        let path = out_dir.join(format!("{name}_{seed}.png"));
        png(&path, &both, 2 * w, h);
        println!("{name:6} {w}x{h}: procedural {:.0} ms, learned {:.0} ms -> {}", tp * 1e3, tl * 1e3, path.display());
    }
}
