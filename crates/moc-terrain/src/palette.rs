//! Biomes, game tiles and the base colour layer.
//!
//! The look (#109): no per-tile random colour. Every colour comes from coherent fields: the
//! biome, the height posterised into a few steps, and a quantised hillshade (shadow, flat,
//! lit). The classes and then the final colour indices go through the majority filter, so the
//! smallest feature is 2×2 and the map reads as clean, chunky pixel art. Water gets a shallow
//! shelf and depth steps, coasts a 1-px foam line, rivers are 1 px.

use crate::climate::{biome, relief};
use crate::grid::{self, Dims};

pub type Rgb = [u8; 3];

/// A planet's colours: the existing game palettes (src/world/terrain.js BIOMES), from which
/// every biome colour is derived so each planet keeps its style.
#[derive(Clone, Debug)]
pub struct BiomePalette {
    pub deep: Rgb,
    pub shallow: Rgb,
    pub sand: Rgb,
    pub grass: Rgb,
    pub forest: Rgb,
    pub hill: Rgb,
    pub mountain: Rgb,
    pub snow: Rgb,
    pub foam: Rgb,
}

impl BiomePalette {
    pub fn temperate() -> Self {
        BiomePalette {
            deep: [22, 44, 92],
            shallow: [40, 86, 146],
            sand: [214, 196, 136],
            grass: [96, 156, 62],
            forest: [46, 98, 46],
            hill: [124, 132, 74],
            mountain: [118, 110, 100],
            snow: [232, 232, 228],
            foam: [120, 170, 205],
        }
    }
    pub fn desert() -> Self {
        BiomePalette {
            deep: [24, 70, 96],
            shallow: [44, 122, 138],
            sand: [232, 206, 140],
            grass: [210, 172, 102],
            forest: [120, 150, 70],
            hill: [184, 120, 72],
            mountain: [140, 78, 52],
            snow: [240, 222, 190],
            foam: [140, 200, 200],
        }
    }
    pub fn ice() -> Self {
        BiomePalette {
            deep: [16, 30, 64],
            shallow: [52, 90, 130],
            sand: [176, 186, 196],
            grass: [214, 228, 236],
            forest: [40, 84, 86],
            hill: [170, 190, 206],
            mountain: [110, 124, 146],
            snow: [248, 250, 255],
            foam: [190, 215, 235],
        }
    }
    pub fn jungle() -> Self {
        BiomePalette {
            deep: [16, 70, 90],
            shallow: [34, 140, 150],
            sand: [214, 196, 136],
            grass: [52, 140, 60],
            forest: [24, 84, 36],
            hill: [78, 124, 58],
            mountain: [118, 110, 100],
            snow: [232, 232, 228],
            foam: [130, 210, 210],
        }
    }
    pub fn alien() -> Self {
        BiomePalette {
            deep: [18, 60, 56],
            shallow: [36, 120, 104],
            sand: [210, 190, 226],
            grass: [140, 92, 176],
            forest: [90, 44, 120],
            hill: [110, 120, 170],
            mountain: [70, 58, 96],
            snow: [214, 255, 236],
            foam: [150, 230, 200],
        }
    }
    /// By the game's planet biome names; unknown names get the temperate palette.
    pub fn named(name: &str) -> Self {
        match name {
            "desert" => BiomePalette::desert(),
            "ice" => BiomePalette::ice(),
            "jungle" => BiomePalette::jungle(),
            "alien" => BiomePalette::alien(),
            _ => BiomePalette::temperate(),
        }
    }

    /// Base colour of a land biome.
    pub fn biome_colour(&self, b: u8) -> [f32; 3] {
        use biome::*;
        let f = |c: Rgb| [c[0] as f32, c[1] as f32, c[2] as f32];
        let (sand, grass, forest, hill, snow, deep) = (f(self.sand), f(self.grass), f(self.forest), f(self.hill), f(self.snow), f(self.deep));
        match b {
            BEACH => sand,
            DESERT => mul(mix(sand, hill, 0.16), [1.02, 0.97, 0.9]),
            STEPPE => mix(grass, sand, 0.52),
            SAVANNA => mul(mix(grass, sand, 0.4), [1.07, 1.0, 0.84]),
            GRASSLAND => grass,
            FOREST => forest,
            JUNGLE => mul(mix(forest, grass, 0.14), [0.86, 1.06, 0.86]),
            TAIGA => mix(forest, deep, 0.24),
            TUNDRA => mix(hill, snow, 0.4),
            ICE | SEA_ICE => snow,
            _ => grass,
        }
    }
}

#[inline]
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

#[inline]
fn mul(a: [f32; 3], k: [f32; 3]) -> [f32; 3] {
    [a[0] * k[0], a[1] * k[1], a[2] * k[2]]
}

#[inline]
fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Height bands (internal units: sea 0, highest peaks about 1).
#[derive(Clone, Debug)]
pub struct Bands {
    pub hill: f32,
    pub mountain: f32,
    pub peak: f32,
    /// Lowland colour steps: below `low[0]` step 0, below `low[1]` step 1, else 2.
    pub low: [f32; 2],
    pub shelf: f32,
    pub abyss: f32,
    pub beach: f32,
}

impl Default for Bands {
    fn default() -> Self {
        Bands { hill: 0.36, mountain: 0.55, peak: 0.82, low: [0.07, 0.17], shelf: 0.08, abyss: 0.55, beach: 0.025 }
    }
}

/// A class for the first majority filter: biome and relief (what the tile *is*).
#[inline]
pub fn class_of(b: u8, r: u8) -> u8 {
    b * 4 + r
}
#[inline]
pub fn class_biome(c: u8) -> u8 {
    c / 4
}
#[inline]
pub fn class_relief(c: u8) -> u8 {
    c % 4
}

/// Colour step within a class (0..3), from the height.
#[inline]
pub fn step_of(b: u8, r: u8, h: f32, bands: &Bands) -> u8 {
    use biome::*;
    match b {
        OCEAN => {
            if h > -bands.shelf * 2.2 {
                0
            } else if h > -bands.abyss {
                1
            } else {
                2
            }
        }
        SHELF | LAKE | SEA_ICE => 0,
        _ => match r {
            relief::LOW => {
                if h < bands.low[0] {
                    0
                } else if h < bands.low[1] {
                    1
                } else {
                    2
                }
            }
            relief::HILL => {
                if h < (bands.hill + bands.mountain) * 0.5 { 0 } else { 1 }
            }
            relief::MOUNTAIN => {
                let t = (h - bands.mountain) / (bands.peak - bands.mountain);
                if t < 0.33 { 0 } else if t < 0.66 { 1 } else { 2 }
            }
            _ => 0,
        },
    }
}

/// Composite paint index: class × 12 + step × 3 + shade.
#[inline]
pub fn paint_index(class: u8, step: u8, shade: u8) -> u16 {
    class as u16 * 12 + step as u16 * 3 + shade as u16
}

/// Colour lookup for every paint index.
pub fn lut(p: &BiomePalette) -> Vec<[u8; 4]> {
    use biome::*;
    let f = |c: Rgb| [c[0] as f32, c[1] as f32, c[2] as f32];
    let (deep, shallow, hill, mountain, snow) = (f(p.deep), f(p.shallow), f(p.hill), f(p.mountain), f(p.snow));
    let shade_k = [0.84f32, 1.0, 1.1];
    let mut out = vec![[0u8, 0, 0, 255]; 64 * 12];
    for b in 0..COUNT as u8 {
        for r in 0..4u8 {
            let class = class_of(b, r);
            for step in 0..4u8 {
                for shade in 0..3u8 {
                    let col = match b {
                        OCEAN => match step {
                            0 => mix(shallow, deep, 0.55),
                            1 => deep,
                            _ => scale(deep, 0.84),
                        },
                        SHELF => shallow,
                        LAKE => mix(shallow, deep, 0.28),
                        SEA_ICE => mix(snow, shallow, 0.14),
                        _ => {
                            let base = p.biome_colour(b);
                            let c = match r {
                                relief::LOW => match step {
                                    0 => scale(base, 0.93),
                                    1 => base,
                                    _ => scale(mix(base, hill, 0.2), 1.04),
                                },
                                relief::HILL => mix(base, hill, 0.42 + 0.16 * step as f32),
                                relief::MOUNTAIN => {
                                    let rock = mix(mountain, base, if b == ICE || b == TUNDRA { 0.35 } else { 0.12 });
                                    match step {
                                        0 => scale(rock, 0.9),
                                        1 => rock,
                                        _ => mix(rock, snow, 0.22),
                                    }
                                }
                                _ => snow,
                            };
                            let k = if b == ICE || r == relief::PEAK { 1.0 + (shade_k[shade as usize] - 1.0) * 0.6 } else { shade_k[shade as usize] };
                            scale(c, k)
                        }
                    };
                    let c = [col[0].round().clamp(0.0, 255.0) as u8, col[1].round().clamp(0.0, 255.0) as u8, col[2].round().clamp(0.0, 255.0) as u8, 255];
                    out[paint_index(class, step, shade) as usize] = c;
                }
            }
        }
    }
    out
}

/// Quantised hillshade (0 shadow, 1 flat, 2 lit), light from the north-west. `z`: how much to
/// exaggerate height for the normals.
pub fn hillshade(d: Dims, h: &[f32], z: f32) -> Vec<u8> {
    let mut s: Vec<f32> = h.iter().map(|&v| v.max(0.0)).collect();
    grid::blur(d, &mut s, 1);
    let l = {
        let v = [-1.0f32, -1.0, 1.4];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / n, v[1] / n, v[2] / n]
    };
    let flat = l[2];
    let (w, hh) = (d.w, d.h);
    let mut out = vec![1u8; d.n()];
    for y in 0..hh {
        let yu = if y > 0 { y - 1 } else if d.wrap_y { hh - 1 } else { 0 };
        let yd = if y + 1 < hh { y + 1 } else if d.wrap_y { 0 } else { hh - 1 };
        for x in 0..w {
            let i = y * w + x;
            if h[i] < 0.0 {
                continue;
            }
            let xl = if x > 0 { x - 1 } else if d.wrap_x { w - 1 } else { 0 };
            let xr = if x + 1 < w { x + 1 } else if d.wrap_x { 0 } else { w - 1 };
            let dx = (s[y * w + xr] - s[y * w + xl]) * 0.5 * z;
            let dy = (s[yd * w + x] - s[yu * w + x]) * 0.5 * z;
            let nl = (dx * dx + dy * dy + 1.0).sqrt();
            let dot = (-dx * l[0] - dy * l[1] + l[2]) / nl;
            out[i] = if dot < flat - 0.1 { 0 } else if dot > flat + 0.07 { 2 } else { 1 };
        }
    }
    out
}

/// Paint the RGBA layer: the paint index colours, then the 1-px foam line on sea coasts and
/// the 1-px rivers.
pub fn colour(d: Dims, paint: &[u16], biome_map: &[u8], river: &[bool], p: &BiomePalette) -> Vec<u8> {
    let table = lut(p);
    let n = d.n();
    let mut rgba = vec![0u8; n * 4];
    let foam = [p.foam[0], p.foam[1], p.foam[2], 255];
    let rc = mix([p.shallow[0] as f32, p.shallow[1] as f32, p.shallow[2] as f32], [p.foam[0] as f32, p.foam[1] as f32, p.foam[2] as f32], 0.3);
    let river_c = [rc[0] as u8, rc[1] as u8, rc[2] as u8, 255];
    for i in 0..n {
        let b = biome_map[i];
        let mut c = table[paint[i] as usize];
        if b == biome::OCEAN || b == biome::SHELF {
            let mut coast = false;
            d.nb4(i, |j| coast |= biome_map[j] >= biome::BEACH);
            if coast {
                c = foam;
            }
        } else if river[i] && b >= biome::BEACH {
            c = river_c;
        }
        rgba[i * 4..i * 4 + 4].copy_from_slice(&c);
    }
    rgba
}

