//! Paint a civ planet to a PPM image, to look at the terrain generator's output.
//!
//! `cargo run -p moc-civ --release --example dump_planet -- <seed> <w> <h> <out.ppm>`

use std::io::Write;

use moc_civ::{Dims, Terrain};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let w: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2560);
    let h: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1280);
    let out = a.get(4).cloned().unwrap_or_else(|| "planet.ppm".into());
    let t0 = std::time::Instant::now();
    let t = Terrain::generate(seed, Dims::new(w, h));
    let land = t.walk.iter().filter(|&&w| w).count() as f32 / t.d.n() as f32;
    let masses = t.land_id.iter().copied().max().unwrap_or(0);
    eprintln!("seed {seed} {w}x{h}: {} ms, {} continents, land {:.0}%, {masses} landmasses", t0.elapsed().as_millis(), t.regions.len(), land * 100.0);
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("create"));
    write!(f, "P6\n{} {}\n255\n", t.d.w, t.d.h).unwrap();
    for px in t.rgba.chunks_exact(4) {
        f.write_all(&px[..3]).unwrap();
    }
}
