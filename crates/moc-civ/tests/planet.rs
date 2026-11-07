//! The planet generator: bigger planets get bigger lands, continents are shaped like real ones
//! (arms, peninsulas and bays rather than blobs), and the home continents stay joined on foot by
//! their isthmuses (which bend instead of running ruler-straight, Alex).

use moc_civ::terrain::HOME_REGIONS;
use moc_civ::{Dims, Terrain};

fn land_share(t: &Terrain) -> f32 {
    t.walk.iter().filter(|&&w| w).count() as f32 / t.d.n() as f32
}

/// Mean roundness of the big landmasses at the continent scale: the planet is coarsened to
/// cells of `8·s` tiles (land where most of a cell is), then each landmass's isoperimetric
/// quotient is 4πA/P² (1 for a disc, about 0.8 for a square or blob, low for a shape with
/// arms, peninsulas and bays).
fn coarse_roundness(t: &Terrain) -> f32 {
    let s = (t.d.n() as f32 / Dims::DEFAULT.n() as f32).sqrt().max(1.0);
    let c = (8.0 * s) as usize;
    let (gw, gh) = (t.d.w / c, t.d.h / c);
    let mut land = vec![false; gw * gh];
    for gy in 0..gh {
        for gx in 0..gw {
            let n = (0..c).flat_map(|y| (0..c).map(move |x| (gx * c + x, gy * c + y))).filter(|&(x, y)| t.walk[y * t.d.w + x]).count();
            land[gy * gw + gx] = n * 2 > c * c;
        }
    }
    // Landmasses on the coarse grid (4-neighbour, wrapping east–west).
    let mut id = vec![0usize; gw * gh];
    let mut iq = Vec::new();
    let mut next = 0;
    for s0 in 0..gw * gh {
        if !land[s0] || id[s0] != 0 {
            continue;
        }
        next += 1;
        let (mut area, mut perim, mut stack) = (0u32, 0u32, vec![s0]);
        id[s0] = next;
        while let Some(k) = stack.pop() {
            area += 1;
            let (x, y) = ((k % gw) as i32, (k / gw) as i32);
            for (ox, oy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = ((x + ox).rem_euclid(gw as i32), y + oy);
                if ny < 0 || ny >= gh as i32 || !land[ny as usize * gw + nx as usize] {
                    perim += 1;
                    continue;
                }
                let j = ny as usize * gw + nx as usize;
                if id[j] == 0 {
                    id[j] = next;
                    stack.push(j);
                }
            }
        }
        if area >= 25 {
            iq.push(4.0 * std::f32::consts::PI * area as f32 / (perim * perim) as f32);
        }
    }
    iq.iter().sum::<f32>() / iq.len().max(1) as f32
}

#[test]
fn bigger_planets_have_bigger_lands_not_more_ocean() {
    for seed in [3, 11, 29] {
        let t = Terrain::generate(seed, Dims::new(2560, 1280));
        let land = land_share(&t);
        assert!((0.2..0.45).contains(&land), "seed {seed}: land {land:.2} on 2560x1280");
        assert!(t.regions.len() >= 8, "seed {seed}: only {} continents", t.regions.len());
    }
}

#[test]
fn home_continents_are_joined_on_foot() {
    for (seed, w, h) in [(1, 1280, 640), (7, 1920, 960), (11, 2560, 1280), (5, 3072, 1536), (33, 1280, 640), (44, 2560, 1280)] {
        let t = Terrain::generate(seed, Dims::new(w, h));
        let homes: Vec<u16> = t.regions[..HOME_REGIONS].iter().map(|r| t.land(r.start.0, r.start.1)).collect();
        assert!(homes.iter().all(|&l| l != 0 && l == homes[0]), "seed {seed} {w}x{h}: home landmasses {homes:?}");
    }
}

#[test]
fn continents_have_arms_and_bays_not_blob_shapes() {
    for (seed, w, h) in [(5, 1280, 640), (9, 1920, 960), (11, 2560, 1280), (7, 3072, 1536), (21, 2560, 1280)] {
        let t = Terrain::generate(seed, Dims::new(w, h));
        let iq = coarse_roundness(&t);
        eprintln!("seed {seed} {w}x{h}: coarse roundness {iq:.2}");
        assert!(iq < 0.45, "seed {seed} {w}x{h}: landmasses too blob-like (IQ {iq:.2})");
    }
}

#[test]
fn the_same_seed_and_size_make_the_same_planet() {
    let a = Terrain::generate(42, Dims::new(1920, 960));
    let b = Terrain::generate(42, Dims::new(1920, 960));
    assert_eq!(a.kind, b.kind);
}
