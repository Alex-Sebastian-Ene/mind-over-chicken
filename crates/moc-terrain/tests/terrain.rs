//! moc-terrain acceptance tests (#109, #111, #129, Alex's feedback). Maps are random each run
//! (Alex: "random each time"): every test draws fresh seeds and prints them when it fails.

use std::sync::OnceLock;

use moc_terrain::climate::{biome, tile};
use moc_terrain::filter;
use moc_terrain::grid::Dims;
use moc_terrain::hydrology::NONE;
use moc_terrain::{generate, TerrainMap, TerrainParams};

fn random_seed(salt: u64) -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64;
    let mut z = t ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (z ^ (z >> 31)) as u32
}

/// Three random 640×360 home worlds and one galaxy surface, shared by the tests.
fn maps() -> &'static Vec<(u32, TerrainMap)> {
    static M: OnceLock<Vec<(u32, TerrainMap)>> = OnceLock::new();
    M.get_or_init(|| {
        let mut v: Vec<(u32, TerrainMap)> = (0..3).map(|k| random_seed(k)).map(|s| (s, generate(&TerrainParams::home_world_sized(s, 640, 360)))).collect();
        let s = random_seed(99);
        v.push((s, generate(&TerrainParams::galaxy_surface(s, "temperate"))));
        v
    })
}

#[test]
fn seams_are_continuous_both_ways() {
    for (seed, m) in maps() {
        let (w, h) = (m.width, m.height);
        // Height steps across each seam are like the steps between neighbouring columns/rows.
        let step = |a: usize, b: usize| (m.heights[a] - m.heights[b]).abs();
        let col_seam: f32 = (0..h).map(|y| step(y * w, y * w + w - 1)).sum::<f32>() / h as f32;
        let col_in: f32 = (0..h).map(|y| step(y * w + w / 2, y * w + w / 2 - 1)).sum::<f32>() / h as f32;
        let row_seam: f32 = (0..w).map(|x| step(x, (h - 1) * w + x)).sum::<f32>() / w as f32;
        let row_in: f32 = (0..w).map(|x| step((h / 2) * w + x, (h / 2 - 1) * w + x)).sum::<f32>() / w as f32;
        assert!(col_seam < (col_in * 3.0).max(0.02), "seed {seed}: east–west seam step {col_seam} vs {col_in}");
        assert!(row_seam < (row_in * 3.0).max(0.02), "seed {seed}: north–south seam step {row_seam} vs {row_in}");
        // Biomes match across the seams about as often as across any other column/row.
        let same = |a: usize, b: usize| (m.biome[a] == m.biome[b]) as usize;
        let bs = (0..h).map(|y| same(y * w, y * w + w - 1)).sum::<usize>() + (0..w).map(|x| same(x, (h - 1) * w + x)).sum::<usize>();
        let bi = (0..h).map(|y| same(y * w + w / 2, y * w + w / 2 - 1)).sum::<usize>() + (0..w).map(|x| same((h / 2) * w + x, (h / 2 - 1) * w + x)).sum::<usize>();
        assert!(bs as f32 >= bi as f32 * 0.85, "seed {seed}: biomes across seams {bs} vs inside {bi}");
    }
}

#[test]
fn a_river_can_cross_a_seam() {
    // A torus map whose only sea is just east of the west edge, with the land falling all the
    // way round to its west shore: the land at the east edge drains across the seam.
    let d = Dims::torus(48, 24);
    let h: Vec<f32> = (0..d.n())
        .map(|i| {
            let (x, _) = d.xy(i);
            if (2..6).contains(&x) {
                -0.5
            } else {
                // Rising westwards from the east edge round to the sea's east shore.
                let k = (2 - x as i32).rem_euclid(48) as f32;
                0.02 + k * 0.01
            }
        })
        .collect();
    let dr = moc_terrain::hydrology::route(d, &h, 0.0, 1e-5);
    let i = 10 * 48 + 47; // east edge
    let r = dr.recv[i] as usize;
    assert_ne!(dr.recv[i], NONE);
    assert_eq!(r % 48, 0, "the east edge drains to column 0, across the seam");
}

#[test]
fn rivers_end_in_the_sea_or_a_lake_and_follow_valley_floors() {
    for (seed, m) in maps() {
        let s = &m.sim;
        let d = Dims::torus(s.w, s.h);
        let (mut valley, mut total) = (0usize, 0usize);
        for i in 0..s.w * s.h {
            if !s.river[i] {
                continue;
            }
            let r = s.recv[i];
            assert_ne!(r, NONE, "seed {seed}: river cell {i} has nowhere to go");
            let r = r as usize;
            assert!(s.river[r] || s.lake[r] || s.ocean[r], "seed {seed}: river cell {i} drains to dry land");
            // Valley floor: lower than the land either side, across the flow.
            let (x, y) = d.xy(i);
            let (rx, ry) = d.xy(r);
            let (fx, fy) = (d.dx(x as f32, rx as f32).round() as i32, d.dy(y as f32, ry as f32).round() as i32);
            let (px, py) = (-fy, fx);
            if let (Some(a), Some(b)) = (d.idx(x + px, y + py), d.idx(x - px, y - py)) {
                if !s.ocean[a] && !s.ocean[b] {
                    total += 1;
                    if s.height[i] <= 0.5 * (s.height[a] + s.height[b]) + 1e-4 {
                        valley += 1;
                    }
                }
            }
        }
        assert!(total > 20, "seed {seed}: too few rivers ({total})");
        assert!(valley as f32 >= total as f32 * 0.75, "seed {seed}: {valley} of {total} river cells on valley floors");
        // Full-resolution rivers are on land.
        for i in 0..m.width * m.height {
            if m.river_mask[i] {
                assert!(biome::is_land(m.biome[i]), "seed {seed}: river tile {i} off land");
            }
        }
    }
}

#[test]
fn coasts_are_clean_islands_big_and_no_specks() {
    for (seed, m) in maps() {
        let d = m.dims();
        let land: Vec<bool> = m.biome.iter().map(|&b| biome::is_land(b)).collect();
        let (threads, checkers, _) = filter::defects(d, &land);
        assert_eq!(threads, 0, "seed {seed}: land threads under 3 tiles");
        assert_eq!(checkers, 0, "seed {seed}: diagonal-only joins");
        for c in m.continents.iter() {
            assert!(c.area >= m.stats.min_island, "seed {seed}: landmass of {} tiles (min {})", c.area, m.stats.min_island);
        }
        // Every tile (bar 1-px rivers) sits in a 2×2 block of its own tile type.
        let specks = filter::specks(d, &m.types, |i| m.river_mask[i]);
        // Where three classes meet on a diagonal a tile can be left outside any 2×2 block of its
        // own; that's allowed for a handful, never an isolated one.
        assert!(specks * 400 <= m.width * m.height, "seed {seed}: {specks} tiles outside 2×2 blocks");
        let mut isolated = 0;
        for i in 0..d.n() {
            if m.river_mask[i] {
                continue;
            }
            let mut same = false;
            d.nb4(i, |j| same |= m.types[j] == m.types[i]);
            isolated += (!same) as usize;
        }
        assert_eq!(isolated, 0, "seed {seed}: single-tile specks");
        assert_eq!(m.stats.layout.straight_strips, 0, "seed {seed}: a straight strip of land");
    }
}

#[test]
fn home_worlds_have_five_continents_and_a_real_layout() {
    for (seed, m) in maps().iter().take(3) {
        let majors = m.continents.iter().filter(|c| c.major).count();
        assert!(majors >= 5, "seed {seed}: {majors} major continents");
        let sea = m.biome.iter().filter(|&&b| !biome::is_land(b)).count() as f32 / (m.width * m.height) as f32;
        assert!((0.45..=0.70).contains(&sea), "seed {seed}: sea share {sea}");
        let l = &m.stats.layout;
        assert!(l.size_cv > 0.6, "seed {seed}: continent sizes too even (CV {})", l.size_cv);
        assert!(l.iq_mean < 0.75 * l.iq_ellipse, "seed {seed}: shapes too round ({} vs ellipse {})", l.iq_mean, l.iq_ellipse);
        assert!(l.open_ocean > 0.03, "seed {seed}: no open ocean ({})", l.open_ocean);
        let s = m.campaign_sites.as_ref().expect("campaign");
        for (x, y) in [s.village, s.pad, s.factory, s.exchange, s.hangar] {
            assert!(tile::walkable(m.types[y * m.width + x]), "seed {seed}: site {x},{y} not walkable");
        }
    }
}

#[test]
fn civ_planets_have_a_continent_per_people() {
    // #101 / #164: the small civ planet (1280×640, 8 peoples).
    let seed = random_seed(7);
    let m = generate(&TerrainParams::civ_planet_for(seed, 1280, 640, 8));
    assert!(m.continents.len() >= 8, "seed {seed}: {} landmasses for 8 peoples", m.continents.len());
    for c in m.continents.iter() {
        assert!(c.area >= m.stats.min_island, "seed {seed}: landmass of {} tiles", c.area);
    }
    let sea = m.biome.iter().filter(|&&b| !biome::is_land(b)).count() as f32 / (m.width * m.height) as f32;
    assert!((0.45..=0.75).contains(&sea), "seed {seed}: sea share {sea}");
    let l = &m.stats.layout;
    assert!(l.size_cv > 0.6, "seed {seed}: continent sizes too even (CV {})", l.size_cv);
    assert_eq!(l.straight_strips, 0, "seed {seed}: a straight strip of land");
    assert!(m.sites.len() >= 8, "seed {seed}: {} sites", m.sites.len());
}

#[test]
fn landforms_and_biomes_are_varied() {
    for (seed, m) in maps() {
        let v = &m.stats.variety;
        assert!(v.max_landform() <= 0.45, "seed {seed}: one landform covers {:.0}% of the land: {:?}", v.max_landform() * 100.0, v.landform_share);
        assert!(v.max_biome() <= 0.40, "seed {seed}: one biome covers {:.0}% of the land", v.max_biome() * 100.0);
        assert!(v.landform_entropy >= 1.4, "seed {seed}: landform entropy {}", v.landform_entropy);
        assert!(v.biome_entropy >= 1.5, "seed {seed}: biome entropy {}", v.biome_entropy);
    }
}

#[test]
fn ranges_branch() {
    for (seed, m) in maps() {
        let big: Vec<_> = m.ranges.iter().filter(|c| c.nodes >= 40).collect();
        assert!(!big.is_empty(), "seed {seed}: no mountain ranges");
        for c in big {
            assert!(c.leaves >= 3 && c.branch_points >= 1, "seed {seed}: a range without spurs {c:?}");
        }
    }
}

#[test]
fn sites_are_on_land_and_reachable() {
    for (seed, m) in maps() {
        let d = m.dims();
        assert!(!m.sites.is_empty(), "seed {seed}: no sites");
        for s in m.sites.iter() {
            let i = s.y as usize * m.width + s.x as usize;
            assert!(tile::walkable(m.types[i]), "seed {seed}: site on {}", m.types[i]);
        }
        // Sites in the same region really are connected on foot (BFS from the first).
        let first = &m.sites[0];
        let mut seen = vec![false; d.n()];
        let mut q = vec![first.y as usize * m.width + first.x as usize];
        seen[q[0]] = true;
        while let Some(i) = q.pop() {
            d.nb8(i, |j, _| {
                if !seen[j] && tile::walkable(m.types[j]) {
                    seen[j] = true;
                    q.push(j);
                }
            });
        }
        for s in m.sites.iter().filter(|s| s.region == first.region) {
            assert!(seen[s.y as usize * m.width + s.x as usize], "seed {seed}: site {},{} unreachable", s.x, s.y);
        }
    }
}

#[test]
fn every_run_is_different() {
    let a = &maps()[0].1;
    let b = &maps()[1].1;
    let land = |m: &TerrainMap, i: usize| biome::is_land(m.biome[i]);
    let (mut inter, mut union) = (0, 0);
    for i in 0..a.width * a.height {
        inter += (land(a, i) && land(b, i)) as usize;
        union += (land(a, i) || land(b, i)) as usize;
    }
    assert!((inter as f32 / union as f32) < 0.6, "two runs overlap {}", inter as f32 / union as f32);
}

#[test]
fn the_detail_hook_takes_another_model() {
    // A stand-in "learned" model: plain upsampling. The pipeline runs on its output.
    struct Flat;
    impl moc_terrain::ReliefDetail for Flat {
        fn detail(&self, input: &moc_terrain::DetailInput, _rng: &mut moc_terrain::Rng) -> Vec<f32> {
            moc_terrain::grid::resample(input.coarse_dims, input.coarse, input.dims)
        }
    }
    let m = moc_terrain::generate_with(&TerrainParams::home_world(random_seed(5)), &Flat);
    assert_eq!(m.heights.len(), m.width * m.height);
    assert!(m.continents.iter().filter(|c| c.major).count() >= 3);
}
