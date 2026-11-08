//! Before/after images: the whole moc-terrain generator with the procedural detail vs with
//! the learned detail, for each map size, on the same seed (same plates and continents).
//!
//!     cargo run --release -p moc-diffusion --example compare -- <weights.mocd> <out_dir> [seed]
//!
//! Each PNG is two columns (procedural | learned) and two rows (the game's painted map, then
//! a hillshaded height view that shows the relief itself). The civ planet is drawn at half
//! size. PNGs are stored uncompressed (no dependencies); `rl/terrain_diffusion/png.py
//! --recompress` shrinks them.
use moc_diffusion::{LearnedDiffusion, ProceduralDiffusion};
use moc_terrain::{generate, generate_with, TerrainMap, TerrainParams};
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

/// RGB views of a map, downsampled by `k`: (painted, hillshaded heights).
fn views(m: &TerrainMap, k: usize) -> (Vec<u8>, Vec<u8>, usize, usize) {
    let (w, h) = (m.width, m.height);
    let (ow, oh) = (w / k, h / k);
    // Heights are 0..1 with the sea at m.sea_level; back to sea 0, peaks ~1, floor -1.
    let rel = |i: usize| {
        let v = m.heights[i];
        if v >= m.sea_level {
            (v - m.sea_level) / (1.0 - m.sea_level)
        } else {
            (v - m.sea_level) / m.sea_level
        }
    };
    let (mut paint, mut shade) = (Vec::with_capacity(ow * oh * 3), Vec::with_capacity(ow * oh * 3));
    for y in 0..oh {
        for x in 0..ow {
            let (sx, sy) = (x * k, y * k);
            let i = sy * w + sx;
            paint.extend_from_slice(&m.rgba[i * 4..i * 4 + 3]);
            let e = rel(sy * w + (sx + k) % w) - rel(sy * w + (sx + w - k) % w);
            let n = rel(((sy + k) % h) * w + sx) - rel(((sy + h - k) % h) * w + sx);
            let v = rel(i);
            let light = if v > 0.0 { (1.0 - (e + n) * 6.0 / k as f32).clamp(0.5, 1.4) } else { 1.0 };
            shade.extend(colour(v).map(|c| (c * light).clamp(0.0, 255.0) as u8));
        }
    }
    (paint, shade, ow, oh)
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
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
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
    let seed: u32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
    std::fs::create_dir_all(&out_dir).unwrap();
    let base = LearnedDiffusion::from_bytes(&weights);
    println!("learned: {} ({:?}), steps {}, seed {seed}", base.is_learned(), base.load_error(), base.opts.steps);
    for (name, p, k) in [
        ("home", TerrainParams::home_world(seed), 1),
        ("galaxy", TerrainParams::galaxy_surface(seed, "temperate"), 1),
        ("civ", TerrainParams::civ_planet(seed, 2560, 1280), 2),
    ] {
        let relief = LearnedDiffusion::from_bytes(&weights).with_procedural(ProceduralDiffusion::from_params(&p));
        let t = Instant::now();
        let a = generate(&p);
        let tp = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let b = generate_with(&p, &relief);
        let tl = t.elapsed().as_secs_f64();
        let (pa, sa, w, h) = views(&a, k);
        let (pb, sb, _, _) = views(&b, k);
        let mut img = Vec::with_capacity(w * h * 12);
        for (l, r) in [(&pa, &pb), (&sa, &sb)] {
            for y in 0..h {
                img.extend_from_slice(&l[y * w * 3..(y + 1) * w * 3]);
                img.extend_from_slice(&r[y * w * 3..(y + 1) * w * 3]);
            }
        }
        let path = out_dir.join(format!("{name}_{seed}.png"));
        png(&path, &img, 2 * w, 2 * h);
        println!("{name:6} {}x{}: procedural {:.0} ms, with learned detail {:.0} ms -> {}", p.w, p.h, tp * 1e3, tl * 1e3, path.display());
    }
}
