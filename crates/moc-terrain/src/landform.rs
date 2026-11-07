//! Landforms and variety (Alex, #109: "it becomes too much of one landform").
//!
//! Every continent gets a character drawn at random each run (mountainous, arid, wet, broken
//! into islands and inland seas, highland, plains), and the land is classified into landform
//! classes so the mix can be measured: no class should cover more than ~40% of the land and no
//! biome more than ~35%, and the Shannon entropy of both should stay high.

use crate::rng::Rng;

/// What a continent is like. Multipliers and offsets applied to its territory.
#[derive(Clone, Copy, Debug)]
pub struct Character {
    pub name: &'static str,
    /// Mountain range strength.
    pub mountains: f32,
    /// Subtracted from moisture (dry > 0 > wet).
    pub aridity: f32,
    /// Coastline irregularity.
    pub roughness: f32,
    /// Tableland height.
    pub plateau: f32,
    /// How hilly the rugged regions get.
    pub rugged: f32,
    /// Inland seas and straits that break it into big islands (0..1).
    pub broken: f32,
}

pub const ARCHETYPES: [Character; 6] = [
    Character { name: "mountainous", mountains: 1.6, aridity: 0.0, roughness: 1.0, plateau: 0.03, rugged: 1.3, broken: 0.0 },
    Character { name: "arid", mountains: 0.6, aridity: 0.3, roughness: 0.8, plateau: 0.1, rugged: 0.6, broken: 0.0 },
    Character { name: "wet", mountains: 0.8, aridity: -0.22, roughness: 1.15, plateau: 0.0, rugged: 0.8, broken: 0.0 },
    Character { name: "broken", mountains: 0.9, aridity: -0.05, roughness: 1.7, plateau: 0.0, rugged: 1.0, broken: 1.0 },
    Character { name: "highland", mountains: 1.0, aridity: 0.08, roughness: 0.9, plateau: 0.16, rugged: 0.7, broken: 0.0 },
    Character { name: "plains", mountains: 0.45, aridity: 0.04, roughness: 1.0, plateau: 0.0, rugged: 0.35, broken: 0.0 },
];

/// One character per continent, shuffled so a map mixes them. With `home` set, continent 0
/// (the campaign's home) is never broken up or a desert.
pub fn characters(n: usize, home: bool, rng: &mut Rng) -> Vec<Character> {
    let mut order: Vec<usize> = (0..ARCHETYPES.len()).collect();
    for k in (1..order.len()).rev() {
        let j = rng.below(k + 1);
        order.swap(k, j);
    }
    if home {
        // Put a habitable character first.
        if let Some(pos) = order.iter().position(|&k| matches!(ARCHETYPES[k].name, "wet" | "plains" | "mountainous" | "highland")) {
            order.swap(0, pos);
        }
    }
    (0..n)
        .map(|c| {
            let a = ARCHETYPES[order[c % order.len()]];
            let j = |v: f32, r: &mut Rng| v * r.range(0.8, 1.2);
            Character {
                name: a.name,
                mountains: j(a.mountains, rng),
                aridity: a.aridity + rng.range(-0.05, 0.05),
                roughness: j(a.roughness, rng),
                plateau: j(a.plateau, rng),
                rugged: j(a.rugged, rng),
                broken: a.broken,
            }
        })
        .collect()
}

/// Landform classes, for the variety metric and the landform map.
pub mod class {
    pub const WATER: u8 = 0;
    pub const PLAINS: u8 = 1;
    pub const COASTAL: u8 = 2;
    pub const DELTA: u8 = 3;
    pub const WETLAND: u8 = 4;
    pub const HILLS: u8 = 5;
    pub const PLATEAU: u8 = 6;
    pub const MOUNTAINS: u8 = 7;
    pub const BASIN: u8 = 8;
    pub const DESERT: u8 = 9;
    pub const COLD: u8 = 10;
    pub const ISLAND: u8 = 11;
    pub const COUNT: usize = 12;
    pub const NAMES: [&str; COUNT] =
        ["water", "plains", "coastal lowland", "delta", "wetland", "hills", "plateau", "mountains", "basin / rift", "desert", "tundra / ice", "island"];
    /// A colour per class for the landform map.
    pub const COLOURS: [[u8; 3]; COUNT] = [
        [24, 44, 92],
        [140, 190, 90],
        [214, 222, 140],
        [90, 200, 190],
        [60, 140, 120],
        [170, 120, 70],
        [200, 150, 210],
        [110, 100, 100],
        [220, 90, 70],
        [240, 200, 100],
        [220, 235, 245],
        [255, 140, 40],
    ];
}

/// Shares of each class over the land, and their Shannon entropy (nats).
#[derive(Clone, Debug, Default)]
pub struct Variety {
    pub landform_share: Vec<f32>,
    pub biome_share: Vec<f32>,
    pub landform_entropy: f32,
    pub biome_entropy: f32,
    /// The characters drawn for the continents, in plan order.
    pub characters: Vec<&'static str>,
}

impl Variety {
    pub fn max_landform(&self) -> f32 {
        self.landform_share.iter().cloned().fold(0.0, f32::max)
    }
    pub fn max_biome(&self) -> f32 {
        self.biome_share.iter().cloned().fold(0.0, f32::max)
    }
}

/// Shares (over the land tiles counted by `count`) and their entropy.
pub fn shares(counts: &[usize]) -> (Vec<f32>, f32) {
    let total: usize = counts.iter().sum();
    let t = total.max(1) as f32;
    let s: Vec<f32> = counts.iter().map(|&c| c as f32 / t).collect();
    let e = -s.iter().filter(|&&p| p > 0.0).map(|&p| p * p.ln()).sum::<f32>();
    (s, e)
}

/// How a map's continents are laid out and shaped (Alex: uneven sizes, clustered, real
/// shapes), over its major continents.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// Coefficient of variation of the major continents' areas (> 0.6: uneven sizes).
    pub size_cv: f32,
    /// Largest over smallest major continent.
    pub size_ratio: f32,
    /// Quadrat dispersion of the land share (0: land spread evenly over every quadrat, 1: every
    /// quadrat all land or all sea); bunched continents with open ocean between score high.
    pub bunching: f32,
    /// The largest disc of open ocean (no land in it), as a share of the map.
    pub open_ocean: f32,
    /// Mean isoperimetric quotient 4πA/P² (P: land–water edges), and that of a 2:1 ellipse of
    /// the same mean area drawn on the same grid (an irregular coast scores well below it).
    pub iq_mean: f32,
    pub iq_ellipse: f32,
    /// Long, ruler-straight thin strips of land (should be 0), and the straightest long thin
    /// piece's linearity (`metrics::strips`).
    pub straight_strips: usize,
    pub max_strip_linearity: f32,
}

/// Measure the layout. `continent`: landmass index + 1 per tile (0 water); `majors`: the
/// indices of the major continents with their area and centroid.
pub fn layout(d: crate::grid::Dims, continent: &[u16], majors: &[(usize, usize, f32, f32)]) -> Layout {
    let n = majors.len();
    if n < 2 {
        return Layout::default();
    }
    let areas: Vec<f32> = majors.iter().map(|m| m.1 as f32).collect();
    let mean = areas.iter().sum::<f32>() / n as f32;
    let sd = (areas.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / n as f32).sqrt();
    let lo = areas.iter().cloned().fold(f32::INFINITY, f32::min);
    let hi = areas.iter().cloned().fold(0.0, f32::max);
    // Bunching: the land share in quadrats half the spacing an even layout of `n` continents
    // would have; its variance as a share of the most it could be (m(1 - m), every quadrat all
    // land or all sea). Evenly spread continents score low, bunched ones high.
    let land: Vec<bool> = continent.iter().map(|&c| c > 0).collect();
    let q = (((d.n() as f32 / n as f32).sqrt() * 0.5).round() as usize).max(4);
    let (qx, qy) = (d.w.div_ceil(q), d.h.div_ceil(q));
    let mut cnt = vec![0f32; qx * qy];
    let mut tot = vec![0f32; qx * qy];
    for y in 0..d.h {
        for x in 0..d.w {
            let k = (y / q) * qx + x / q;
            tot[k] += 1.0;
            if land[y * d.w + x] {
                cnt[k] += 1.0;
            }
        }
    }
    let share: Vec<f32> = cnt.iter().zip(tot.iter()).map(|(c, t)| c / t).collect();
    let m = share.iter().sum::<f32>() / share.len() as f32;
    let var = share.iter().map(|s| (s - m).powi(2)).sum::<f32>() / share.len() as f32;
    let bunching = var / (m * (1.0 - m)).max(1e-6);
    // Open ocean: the largest disc with no land in it.
    let dist = crate::grid::distance(d, &land);
    let rmax = dist.iter().cloned().fold(0.0f32, f32::max);
    let open_ocean = std::f32::consts::PI * rmax * rmax / d.n() as f32;
    // Perimeter: land–water edges of each major continent.
    let mut edges = vec![0usize; majors.iter().map(|m| m.0).max().unwrap_or(0) + 1];
    for i in 0..d.n() {
        let c = continent[i];
        if c == 0 {
            continue;
        }
        let ci = c as usize - 1;
        if ci >= edges.len() {
            continue;
        }
        d.nb4(i, |j| {
            if continent[j] != c {
                edges[ci] += 1;
            }
        });
    }
    let iq = |a: f32, p: usize| 4.0 * std::f32::consts::PI * a / (p as f32 * p as f32).max(1.0);
    let iq_mean = majors.iter().map(|m| iq(m.1 as f32, edges[m.0])).sum::<f32>() / n as f32;
    // A 2:1 ellipse of the mean area on a plain grid, measured the same way.
    let b = (mean / (2.0 * std::f32::consts::PI)).sqrt();
    let a = 2.0 * b;
    let (w, h) = ((2.0 * a + 4.0) as usize, (2.0 * b + 4.0) as usize);
    let inside = |x: usize, y: usize| {
        let (dx, dy) = ((x as f32 + 0.5 - w as f32 / 2.0) / a, (y as f32 + 0.5 - h as f32 / 2.0) / b);
        dx * dx + dy * dy <= 1.0
    };
    let (mut ea, mut ep) = (0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if !inside(x, y) {
                continue;
            }
            ea += 1;
            for (ox, oy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i32 + ox, y as i32 + oy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 || !inside(nx as usize, ny as usize) {
                    ep += 1;
                }
            }
        }
    }
    Layout { size_cv: sd / mean, size_ratio: hi / lo.max(1.0), bunching, open_ocean, iq_mean, iq_ellipse: iq(ea as f32, ep), ..Layout::default() }
}
