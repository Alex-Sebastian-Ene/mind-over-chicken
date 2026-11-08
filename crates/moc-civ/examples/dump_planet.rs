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
    // Landmass sizes (spread) and shapes (isoperimetric quotient 4πA/P², 1 = a disc).
    let mut area = vec![0u32; masses as usize + 1];
    let mut perim = vec![0u32; masses as usize + 1];
    for y in 0..t.d.h as i32 {
        for x in 0..t.d.w as i32 {
            let l = t.land(x, y);
            if l == 0 {
                continue;
            }
            area[l as usize] += 1;
            perim[l as usize] += [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(ox, oy)| t.land(x + ox, y + oy) != l).count() as u32;
        }
    }
    let big: Vec<usize> = (1..area.len()).filter(|&l| area[l] >= 400).collect();
    let sizes: Vec<f32> = big.iter().map(|&l| area[l] as f32).collect();
    let mean = sizes.iter().sum::<f32>() / sizes.len().max(1) as f32;
    let cv = (sizes.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / sizes.len().max(1) as f32).sqrt() / mean.max(1.0);
    let iq: Vec<f32> = big.iter().map(|&l| 4.0 * std::f32::consts::PI * area[l] as f32 / (perim[l] as f32).powi(2)).collect();
    let iq_mean = iq.iter().sum::<f32>() / iq.len().max(1) as f32;
    eprintln!(
        "seed {seed} {w}x{h}: {} ms, {} continents, land {:.0}%, {masses} landmasses ({} of 400+ tiles), size CV {cv:.2}, IQ {iq_mean:.2}",
        t0.elapsed().as_millis(),
        t.regions.len(),
        land * 100.0,
        big.len()
    );
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("create"));
    write!(f, "P6\n{} {}\n255\n", t.d.w, t.d.h).unwrap();
    for px in t.rgba.chunks_exact(4) {
        f.write_all(&px[..3]).unwrap();
    }
}
