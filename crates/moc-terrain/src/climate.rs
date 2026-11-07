//! Climate on the simulation grid: temperature from the climate belts and altitude; moisture
//! from distance to the sea plus prevailing winds that rain out over mountains (rain shadow).
//!
//! The map is a torus (#111), so there are no hard poles: the climate is periodic north–south.
//! "Coldness" is a cosine of the row (0 in the middle of a warm belt, 1 in the middle of a cold
//! belt), and the cold belt that runs along the top edge continues along the bottom edge.

use crate::grid::{self, Dims};
use crate::noise::{coarse_field, Fbm, Torus};
use crate::rng::Rng;
use crate::plates::coldness;

/// Biomes: what covers a tile. Water first, then land (Whittaker), then ice.
pub mod biome {
    pub const OCEAN: u8 = 0;
    pub const SHELF: u8 = 1;
    pub const LAKE: u8 = 2;
    pub const SEA_ICE: u8 = 3;
    pub const BEACH: u8 = 4;
    pub const DESERT: u8 = 5;
    pub const STEPPE: u8 = 6;
    pub const SAVANNA: u8 = 7;
    pub const GRASSLAND: u8 = 8;
    pub const FOREST: u8 = 9;
    pub const JUNGLE: u8 = 10;
    pub const TAIGA: u8 = 11;
    pub const TUNDRA: u8 = 12;
    pub const ICE: u8 = 13;
    /// Wetland: marsh and swamp on flat, wet low ground by rivers, lakes and deltas.
    pub const MARSH: u8 = 14;
    pub const COUNT: usize = 15;
    pub const NAMES: [&str; COUNT] =
        ["ocean", "shelf", "lake", "sea ice", "beach", "desert", "steppe", "savanna", "grassland", "forest", "jungle", "taiga", "tundra", "ice", "marsh"];

    /// Open water (sea ice and land are not).
    #[inline]
    pub fn is_water(b: u8) -> bool {
        b <= LAKE
    }

    /// Land, including beaches and land ice.
    #[inline]
    pub fn is_land(b: u8) -> bool {
        b >= BEACH
    }
}

/// Relief classes (height bands that change how a tile plays and looks).
pub mod relief {
    pub const LOW: u8 = 0;
    pub const HILL: u8 = 1;
    pub const MOUNTAIN: u8 = 2;
    pub const PEAK: u8 = 3;
}

/// Game tiles: the same codes as the JS `T` the eras and pathfinding use.
pub mod tile {
    pub const DEEP: u8 = 0;
    pub const SHALLOW: u8 = 1;
    pub const SAND: u8 = 2;
    pub const GRASS: u8 = 3;
    pub const FOREST: u8 = 4;
    pub const HILL: u8 = 5;
    pub const MOUNTAIN: u8 = 6;
    pub const SNOW: u8 = 7;

    #[inline]
    pub fn walkable(t: u8) -> bool {
        t >= SAND && t != MOUNTAIN && t != SNOW
    }
}

/// The game tile for a biome and relief class.
pub fn tile_of(b: u8, r: u8) -> u8 {
    use biome::*;
    match b {
        OCEAN => tile::DEEP,
        SHELF | LAKE => tile::SHALLOW,
        SEA_ICE | ICE => tile::SNOW,
        _ => match r {
            relief::PEAK => tile::SNOW,
            relief::MOUNTAIN => tile::MOUNTAIN,
            relief::HILL => tile::HILL,
            _ => match b {
                BEACH | DESERT => tile::SAND,
                FOREST | JUNGLE | TAIGA => tile::FOREST,
                _ => tile::GRASS,
            },
        },
    }
}

/// Whittaker-style biome from temperature (°C) and moisture (0..1).
pub fn whittaker(t: f32, p: f32) -> u8 {
    use biome::*;
    if t < -9.0 {
        ICE
    } else if t < -1.0 {
        if p > 0.5 { TAIGA } else { TUNDRA }
    } else if t < 6.0 {
        if p > 0.36 { TAIGA } else if p > 0.16 { STEPPE } else { TUNDRA }
    } else if t < 19.0 {
        if p < 0.13 { DESERT } else if p < 0.28 { STEPPE } else if p < 0.55 { GRASSLAND } else { FOREST }
    } else if p < 0.17 {
        DESERT
    } else if p < 0.42 {
        SAVANNA
    } else if p < 0.6 {
        FOREST
    } else {
        JUNGLE
    }
}

#[derive(Clone, Debug)]
pub struct ClimateParams {
    /// Sea-level temperature (°C) in the middle of a warm belt, and of a cold belt.
    pub warm: f32,
    pub cold: f32,
    /// Warm belts per map height.
    pub bands: f32,
    /// Moisture multiplier (dry worlds < 1 < wet worlds).
    pub moisture: f32,
    /// Cooling with height: °C per unit of height above the sea (1 ≈ the highest peaks).
    pub lapse: f32,
}

impl Default for ClimateParams {
    fn default() -> Self {
        ClimateParams { warm: 28.0, cold: -26.0, bands: 1.0, moisture: 1.0, lapse: 30.0 }
    }
}

/// Coldness per row of a grid `h` rows high.
pub fn row_coldness(h: usize, bands: f32) -> Vec<f32> {
    (0..h).map(|y| coldness((y as f32 + 0.5) / h as f32, bands)).collect()
}

/// Temperature in °C on grid `d` (heights: sea at 0, peaks near 1).
pub fn temperature(d: Dims, tor: &Torus, height: &[f32], cp: &ClimateParams, seed: u32) -> Vec<f32> {
    let mut rng = Rng::stream(seed, 0x7E4F);
    let wobble = Fbm::new(&mut rng, 1.0 / (tor.map_min() * 0.35), 3, 0.5);
    let (mw, mh) = tor.map_size();
    let wob = coarse_field(d, mw, mh, (d.w.min(d.h) / 24).max(1), |p| wobble.sample(p));
    let cold = row_coldness(d.h, cp.bands);
    let mut t = vec![0.0f32; d.n()];
    for y in 0..d.h {
        // Earth-like: broad warm and temperate belts, a narrower cold one.
        let c = cold[y].powf(2.4);
        let base = cp.warm + (cp.cold - cp.warm) * c;
        for x in 0..d.w {
            let i = y * d.w + x;
            let alt = height[i].max(0.0);
            t[i] = base - cp.lapse * alt + wob[i] * 4.5;
        }
    }
    t
}

/// Moisture 0..1. `ocean`: sea cells. `cell`: tiles per cell (wind and coast reach scale with
/// the map).
pub fn moisture(d: Dims, tor: &Torus, height: &[f32], ocean: &[bool], temp: &[f32], cp: &ClimateParams, cell: f32, seed: u32) -> Vec<f32> {
    let n = d.n();
    let (w, h) = (d.w, d.h);
    let cold = row_coldness(h, cp.bands);
    // ---- prevailing winds, swept along each row both ways ----
    // Trades blow west near the warm belt, westerlies east at mid-belt, polar easterlies near
    // the cold belt; the two sweeps are blended smoothly so belts don't leave stripes.
    let mut wind = vec![0.0f32; n];
    let look = ((w as f32 / 120.0).round() as usize).max(1);
    let mut sweep = vec![0.0f32; w];
    for y in 0..h {
        let c = cold[y];
        let east = grid::smoothstep(0.22, 0.42, c) * (1.0 - grid::smoothstep(0.62, 0.82, c));
        let row = y * w;
        let start = (0..w).find(|&x| ocean[row + x]).unwrap_or(0);
        for (dir, weight) in [(1i32, east), (-1i32, 1.0 - east)] {
            if weight < 0.01 {
                continue;
            }
            sweep.iter_mut().for_each(|v| *v = 0.0);
            let mut m = 0.35f32;
            for lap in 0..2 {
                for s in 0..w {
                    let x = (start as i32 + dir * s as i32).rem_euclid(w as i32) as usize;
                    let i = row + x;
                    if ocean[i] {
                        let warmth = ((temp[i] + 5.0) / 33.0).clamp(0.05, 1.0);
                        m += (1.0 - m) * (0.05 + 0.14 * warmth);
                    } else {
                        let ahead = row + (x as i32 + dir * look as i32).rem_euclid(w as i32) as usize;
                        let rise = (height[ahead] - height[i]).max(0.0);
                        let rain = m * (0.010 * cell.sqrt() + 2.2 * rise).min(0.6);
                        m -= rain;
                        if lap == 1 {
                            sweep[x] = rain;
                        }
                    }
                }
            }
            for x in 0..w {
                wind[row + x] += weight * sweep[x];
            }
        }
    }
    grid::blur(d, &mut wind, 2);
    // Normalise by the wettest land.
    let mut land_vals: Vec<f32> = (0..n).filter(|&i| !ocean[i]).map(|i| wind[i]).collect();
    let norm = if land_vals.is_empty() {
        1.0
    } else {
        let k = ((land_vals.len() as f32 * 0.95) as usize).min(land_vals.len() - 1);
        *land_vals.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1
    }
    .max(1e-6);
    // ---- distance to the sea ----
    let dist = grid::distance(d, ocean);
    let reach = (w.min(h) as f32 * 0.13).max(3.0);
    let mut rng = Rng::stream(seed, 0x3015);
    let wobble = Fbm::new(&mut rng, 1.0 / (tor.map_min() * 0.3), 3, 0.5);
    let (mw, mh) = tor.map_size();
    let wob = coarse_field(d, mw, mh, (w.min(h) / 24).max(1), |p| wobble.sample(p));
    let mut m = vec![0.0f32; n];
    // The belts meander: the coldness used for the rain belts is bent by noise.
    let bend = Fbm::new(&mut rng, 1.0 / (tor.map_min() * 0.5), 2, 0.5);
    let bent = coarse_field(d, mw, mh, (w.min(h) / 24).max(1), |p| bend.sample(p) * 0.12);
    for y in 0..h {
        // Wet where the warm belt's air rises, dry in the subtropical highs, a mid-belt storm
        // track, dry cold air.
        let c0 = cold[y];
        for x in 0..w {
            let i = y * w + x;
            let c = (c0 + bent[i]).clamp(0.0, 1.0);
        let band = 0.12 * (-(c / 0.10).powi(2)).exp() - 0.08 * (-((c - 0.3) / 0.1).powi(2)).exp() + 0.06 * (-((c - 0.55) / 0.14).powi(2)).exp() - 0.08 * ((c - 0.8) / 0.2).clamp(0.0, 1.0);
            if ocean[i] {
                m[i] = 1.0;
                continue;
            }
            let coast = (-dist[i] / reach).exp();
            let v = 0.12 + 0.5 * (wind[i] / norm).min(1.3) + 0.34 * coast + band + wob[i] * 0.14;
            m[i] = v;
        }
    }
    grid::blur(d, &mut m, 3);
    for i in 0..n {
        m[i] = if ocean[i] { 1.0 } else { (m[i] * cp.moisture).clamp(0.0, 1.0) };
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whittaker_covers_the_required_biomes() {
        use biome::*;
        let mut seen = [false; COUNT];
        for t in -30..35 {
            for p in 0..20 {
                seen[whittaker(t as f32, p as f32 / 19.0) as usize] = true;
            }
        }
        for b in [DESERT, STEPPE, GRASSLAND, FOREST, JUNGLE, TAIGA, TUNDRA, ICE, SAVANNA] {
            assert!(seen[b as usize], "{}", NAMES[b as usize]);
        }
    }

    #[test]
    fn tiles_match_the_game_codes() {
        assert_eq!(tile_of(biome::OCEAN, 0), tile::DEEP);
        assert_eq!(tile_of(biome::GRASSLAND, relief::LOW), tile::GRASS);
        assert_eq!(tile_of(biome::TAIGA, relief::LOW), tile::FOREST);
        assert_eq!(tile_of(biome::DESERT, relief::HILL), tile::HILL);
        assert_eq!(tile_of(biome::FOREST, relief::MOUNTAIN), tile::MOUNTAIN);
        assert_eq!(tile_of(biome::ICE, relief::LOW), tile::SNOW);
        assert!(!tile::walkable(tile::MOUNTAIN) && tile::walkable(tile::FOREST));
    }
}
