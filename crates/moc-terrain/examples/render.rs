//! Generate maps and write their base colour layer as raw RGBA, with a summary.
//!
//! ```text
//! cargo run --release -p moc-terrain --example render -- <home|galaxy|civ> <out_dir> <seed>... [--biome NAME] [--size W H] [--peoples N]
//! ```
//! Writes `<out_dir>/<kind>-<seed>.rgba` (and `.txt` with the size). `scripts/terrain_images.py`
//! turns them into PNGs.

use std::io::Write;
use std::time::Instant;

use moc_terrain::{climate::biome, generate, TerrainParams};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: render <home|galaxy|civ> <out_dir> <seed>... [--biome NAME] [--size W H] [--peoples N]");
        std::process::exit(2);
    }
    let kind = args[0].clone();
    let out = std::path::PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out).expect("out dir");
    let mut seeds = Vec::new();
    let mut biome_name = "temperate".to_string();
    let mut repeat = 1;
    let mut size = (2560usize, 1280usize);
    let mut peoples = 0usize;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--biome" => {
                biome_name = args[i + 1].clone();
                i += 2;
            }
            "--repeat" => {
                repeat = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--peoples" => {
                peoples = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--size" => {
                size = (args[i + 1].parse().unwrap(), args[i + 2].parse().unwrap());
                i += 3;
            }
            s => {
                seeds.push(s.parse::<u32>().expect("seed"));
                i += 1;
            }
        }
    }
    for seed in seeds {
        let p = match kind.as_str() {
            "home" => TerrainParams::home_world(seed),
            "home640" => TerrainParams::home_world_sized(seed, 640, 360),
            "galaxy" => TerrainParams::galaxy_surface(seed, &biome_name),
            _ if peoples > 0 => TerrainParams::civ_planet_for(seed, size.0, size.1, peoples),
            _ => TerrainParams::civ_planet(seed, size.0, size.1),
        };
        // The fastest of `repeat` runs (the machine may be busy), and the thread's CPU time per
        // run, which a busy machine doesn't inflate.
        let mut best = f64::INFINITY;
        let mut t = generate(&p);
        let c0 = cpu_ms();
        for _ in 0..repeat {
            let t0 = Instant::now();
            let tt = generate(&p);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            if ms < best {
                best = ms;
                t = tt;
            }
        }
        let cpu = (cpu_ms() - c0) / repeat.max(1) as f64;
        let ms = if repeat > 0 { best } else { 0.0 };
        let name = if kind == "galaxy" { format!("{kind}-{biome_name}-{seed}") } else { format!("{kind}-{seed}") };
        let path = out.join(format!("{name}.rgba"));
        std::fs::File::create(&path).unwrap().write_all(&t.rgba).unwrap();
        std::fs::write(out.join(format!("{name}.txt")), format!("{} {}\n", t.width, t.height)).unwrap();
        // The landform map: a colour per landform class.
        {
            use moc_terrain::landform::class;
            let mut lf = Vec::with_capacity(t.landform.len() * 4);
            for &c in t.landform.iter() {
                let k = class::COLOURS[c as usize];
                lf.extend_from_slice(&[k[0], k[1], k[2], 255]);
            }
            std::fs::write(out.join(format!("{name}-landform.rgba")), lf).unwrap();
            std::fs::write(out.join(format!("{name}-landform.txt")), format!("{} {}
", t.width, t.height)).unwrap();
            let v = &t.stats.variety;
            let mut lfs: Vec<(f32, &str)> = v.landform_share.iter().enumerate().filter(|(_, &s)| s > 0.0).map(|(k, &s)| (s, class::NAMES[k])).collect();
            lfs.sort_by(|a, b| b.0.total_cmp(&a.0));
            println!(
                "  variety: landform entropy {:.2}, max landform {:.1}%, biome entropy {:.2}, max biome {:.1}% ({}); landforms {}",
                v.landform_entropy,
                v.max_landform() * 100.0,
                v.biome_entropy,
                v.max_biome() * 100.0,
                moc_terrain::climate::biome::NAMES[v.biome_share.iter().enumerate().fold((0, 0.0f32), |a, (k, &s)| if s > a.1 { (k, s) } else { a }).0],
                lfs.iter().map(|(s, n)| format!("{n} {:.0}%", s * 100.0)).collect::<Vec<_>>().join(", ")
            );
        }
        if std::env::var("TERRAIN_DEBUG").is_ok() {
            // Grey layers on the simulation grid: eroded height, DLA ranges, moisture, temperature.
            let (sw, sh) = (t.sim.w, t.sim.h);
            let grey = |v: &dyn Fn(usize) -> f32| -> Vec<u8> {
                let mut o = Vec::with_capacity(sw * sh * 4);
                for i in 0..sw * sh {
                    let g = (v(i).clamp(0.0, 1.0) * 255.0) as u8;
                    o.extend_from_slice(&[g, g, g, 255]);
                }
                o
            };
            let lay = |nm: &str, data: Vec<u8>| {
                std::fs::write(out.join(format!("{name}-{nm}.rgba")), data).unwrap();
                std::fs::write(out.join(format!("{name}-{nm}.txt")), format!("{sw} {sh}\n")).unwrap();
            };
            let cs = t.sim.cell;
            let at = |i: usize| (i / sw) * cs * t.width + (i % sw) * cs;
            lay("height", grey(&|i| if t.sim.ocean[i] { 0.0 } else { 0.25 + t.sim.height[i] * 0.75 }));
            lay("dla", grey(&|i| t.sim.dla[i] + if t.sim.corridor[i] { 0.15 } else { 0.0 }));
            lay("moist", grey(&|i| t.moisture[at(i)]));
            lay("temp", grey(&|i| (t.temperature[at(i)] + 30.0) / 60.0));
        }
        let land = t.biome.iter().filter(|&&b| biome::is_land(b)).count();
        let majors: Vec<usize> = t.continents.iter().filter(|c| c.major).map(|c| c.area).collect();
        let rivers = t.river_mask.iter().filter(|&&r| r).count();
        let lakes = t.lake_mask.iter().filter(|&&r| r).count();
        let mut counts = [0usize; biome::COUNT];
        for &b in t.biome.iter() {
            counts[b as usize] += 1;
        }
        let biomes: Vec<String> = (0..biome::COUNT).filter(|&b| counts[b] > 0).map(|b| format!("{} {:.1}%", biome::NAMES[b], 100.0 * counts[b] as f32 / t.biome.len() as f32)).collect();
        println!("{name}: {}x{} in {ms:.1} ms wall, {cpu:.1} ms CPU (sim cell {}, {} DLA nodes)", t.width, t.height, t.stats.sim_cell, t.stats.dla_nodes);
        println!("  stages: {}", t.stats.stages.iter().map(|(s, v)| format!("{s} {v:.1}")).collect::<Vec<_>>().join(", "));
        println!("  land {:.1}%, major continents {:?}, {} landmasses, river px {rivers}, lake px {lakes}, {} sites", 100.0 * land as f32 / t.biome.len() as f32, majors, t.continents.len(), t.sites.len());
        let land_mask: Vec<bool> = t.biome.iter().map(|&b| biome::is_land(b)).collect();
        let (threads, diagonals, _) = moc_terrain::filter::defects(t.dims(), &land_mask);
        println!("  seed {}: coast threads {threads}, diagonal joins {diagonals}", t.seed);
        let mut isl: Vec<usize> = t.continents.iter().filter(|c| !c.major).map(|c| c.area).collect();
        isl.sort();
        if isl.is_empty() {
            println!("  islands: none (min allowed {})", t.stats.min_island);
        } else {
            println!("  islands: {} (min {}, median {}, max {}; min allowed {})", isl.len(), isl[0], isl[isl.len() / 2], isl[isl.len() - 1], t.stats.min_island);
        }
        println!("  coast clean: {:?}", t.stats.coast);
        let l = &t.stats.layout;
        println!("  layout: size CV {:.2}, largest/smallest {:.1}, bunching {:.2}, open ocean {:.1}%, isoperimetric {:.3} (2:1 ellipse {:.3}), straight strips {} (max linearity {:.2}); characters {:?}", l.size_cv, l.size_ratio, l.bunching, l.open_ocean * 100.0, l.iq_mean, l.iq_ellipse, l.straight_strips, l.max_strip_linearity, t.stats.characters);
        println!("  ranges: {}", t.ranges.iter().map(|r| format!("{}n/{}l/{}b", r.nodes, r.leaves, r.branch_points)).collect::<Vec<_>>().join(" "));
        println!("  biomes: {}", biomes.join(", "));
    }
}

/// CPU time of this thread, ms (Windows; elsewhere wall time is all we report).
#[cfg(windows)]
fn cpu_ms() -> f64 {
    #[repr(C)]
    #[derive(Default)]
    struct Ft {
        lo: u32,
        hi: u32,
    }
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn GetThreadTimes(h: isize, c: *mut Ft, e: *mut Ft, k: *mut Ft, u: *mut Ft) -> i32;
    }
    let (mut c, mut e, mut k, mut u) = (Ft::default(), Ft::default(), Ft::default(), Ft::default());
    unsafe {
        GetThreadTimes(GetCurrentThread(), &mut c, &mut e, &mut k, &mut u);
    }
    let t = |f: &Ft| ((f.hi as u64) << 32 | f.lo as u64) as f64 / 10_000.0;
    t(&k) + t(&u)
}

#[cfg(not(windows))]
fn cpu_ms() -> f64 {
    0.0
}
