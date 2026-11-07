use crate::sprites::*;

pub const T_DEEP: u8 = 0;
pub const T_SHALLOW: u8 = 1;
pub const T_SAND: u8 = 2;
pub const T_GRASS: u8 = 3;
#[allow(dead_code)]
pub const T_FOREST: u8 = 4;
#[allow(dead_code)]
pub const T_HILL: u8 = 5;
pub const T_MOUNTAIN: u8 = 6;
pub const T_SNOW: u8 = 7;
pub const T_FARM: u8 = 8;
pub const T_ROAD: u8 = 9;
pub const T_PAVED: u8 = 10;

#[derive(Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Default)]
pub struct Sites {
    pub village: Point,
    pub pad: Point,
    pub cave: Point,
    pub cave_mouth: Point,
    pub factory: Point,
    pub exchange: Point,
    pub hangar: Point,
}

pub struct Mulberry32 {
    pub state: u32,
}

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }
    pub fn next(&mut self) -> f32 {
        // All arithmetic in u32, matching JS `Math.imul`, so seeds produce the same
        // sequence as the JS build's mulberry32.
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        let res = t ^ (t >> 14);
        (res as f64 / 4294967296.0) as f32
    }
}

pub struct Terrain {
    pub seed: u32,
    pub h: Vec<f32>,
    pub base_type: Vec<u8>,
    pub tile_type: Vec<u8>,
    /// The base colour layer from moc-terrain (no per-tile random colour).
    pub base_pixels: Vec<u8>,
    pub pixels: Vec<u8>, // RGBA 320x180x4
    pub sites: Sites,
}

impl Terrain {
    /// The home world for a seed, from `moc-terrain` (#109). The old single-island generator
    /// is gone: the engine shows the same world the JS `World` (and the eras) use.
    pub fn new(seed: u32) -> Self {
        Self::from_map(&moc_terrain::TerrainMap::generate_home_world(seed))
    }

    /// The engine's copy of a generated home world (it must be the engine's W×H).
    pub fn from_map(map: &moc_terrain::TerrainMap) -> Self {
        assert_eq!((map.width, map.height), (W, H), "the engine draws a {W}x{H} world");
        let p = |q: (usize, usize)| Point { x: q.0 as i32, y: q.1 as i32 };
        let s = map.campaign_sites.clone().unwrap_or_default();
        let sites = Sites {
            village: p(s.village),
            pad: p(s.pad),
            cave: p(s.cave),
            cave_mouth: p(s.cave_mouth),
            factory: p(s.factory),
            exchange: p(s.exchange),
            hangar: p(s.hangar),
        };
        let mut t = Self {
            seed: map.seed,
            h: map.heights.clone(),
            base_type: map.types.clone(),
            tile_type: map.types.clone(),
            base_pixels: map.rgba.clone(),
            pixels: map.rgba.clone(),
            sites,
        };
        t.bake_era(0);
        t
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            T_DEEP
        } else {
            self.tile_type[(y as usize) * W + (x as usize)]
        }
    }

    #[inline]
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        let t = self.get(x, y);
        t >= T_SAND && t != T_MOUNTAIN && t != T_SNOW
    }

    pub fn bake_era(&mut self, era: u8) {
        self.tile_type = self.base_type.clone();
        let v = self.sites.village;
        let s = self.sites;
        let mut rng = Mulberry32::new(self.seed + 17);

        // Era modifications
        if era == 0 {
            // Stone Age: huts around village
        } else if era == 1 {
            // Farming: farms
            for _ in 0..14 {
                let a = rng.next() * std::f32::consts::PI * 2.0;
                let r = 18.0 + rng.next() * 42.0;
                let cx = (v.x as f32 + a.cos() * r) as i32;
                let cy = (v.y as f32 + a.sin() * r) as i32;
                let fw = 6 + (rng.next() * 6.0) as i32;
                let fh = 4 + (rng.next() * 4.0) as i32;
                for fy in cy..(cy + fh) {
                    for fx in cx..(cx + fw) {
                        if self.get(fx, fy) == T_GRASS {
                            self.set_tile(fx, fy, T_FARM);
                        }
                    }
                }
            }
        } else if era >= 2 {
            // Industry or Space Age: paved central, roads
            for fy in -2..=2 {
                for fx in -3..=3 {
                    self.set_tile(v.x + fx, v.y + fy, T_PAVED);
                }
            }
            if era == 3 {
                for fy in -6..=6 {
                    for fx in -8..=8 {
                        if self.walkable(s.pad.x + fx, s.pad.y + fy) {
                            self.set_tile(s.pad.x + fx, s.pad.y + fy, T_PAVED);
                        }
                    }
                }
            }
        }

        // The base colour layer, with the era's land use painted over it.
        self.pixels = paint_land_use(&self.base_pixels, &self.tile_type, &self.base_type, W, "temperate");

        // Cave mouth
        if era <= 1 {
            let m = self.sites.cave_mouth;
            fill_rect(&mut self.pixels, m.x - 2, m.y - 1, 4, 3, 0x1a, 0x17, 0x14, 255);
            fill_rect(&mut self.pixels, m.x - 3, m.y, 1, 2, 0x2b, 0x26, 0x22, 255);
            fill_rect(&mut self.pixels, m.x + 2, m.y, 1, 2, 0x2b, 0x26, 0x22, 255);
        }

        // Bake static structures
        if era == 0 {
            for i in 0..4 {
                let a = (i as f32 / 4.0) * std::f32::consts::PI * 2.0 + 0.5;
                let hx = v.x + (a.cos() * 11.0).round() as i32;
                let hy = v.y + (a.sin() * 7.0).round() as i32;
                draw_hut(&mut self.pixels, hx, hy);
            }
        } else if era == 1 {
            for i in 0..10 {
                let a = (i as f32 / 10.0) * std::f32::consts::PI * 2.0;
                let hx = v.x + (a.cos() * (12.0 + ((i % 2) * 6) as f32)).round() as i32;
                let hy = v.y + (a.sin() * (8.0 + ((i % 2) * 4) as f32)).round() as i32;
                draw_hut(&mut self.pixels, hx, hy);
            }
        } else if era == 2 {
            draw_factory(&mut self.pixels, s.factory.x, s.factory.y);
            draw_exchange(&mut self.pixels, s.exchange.x, s.exchange.y);
            for ring in 0..3 {
                let n = 10 + ring * 6;
                for i in 0..n {
                    let a = (i as f32 / n as f32) * std::f32::consts::PI * 2.0 + ring as f32 * 0.3;
                    let hx = v.x + (a.cos() * (12.0 + ring as f32 * 9.0)).round() as i32;
                    let hy = v.y + (a.sin() * (8.0 + ring as f32 * 6.0)).round() as i32;
                    if self.walkable(hx, hy) {
                        draw_house(&mut self.pixels, hx, hy, (0xa4, 0x40, 0x2f));
                    }
                }
            }
        } else if era == 3 {
            draw_hangar(&mut self.pixels, s.hangar.x, s.hangar.y);
            draw_pad(&mut self.pixels, s.pad.x, s.pad.y);
            draw_exchange(&mut self.pixels, s.exchange.x, s.exchange.y);
            draw_tower(&mut self.pixels, v.x - 14, v.y - 12, 14);
            draw_tower(&mut self.pixels, v.x + 18, v.y - 14, 12);
        }
    }

    #[inline]
    fn set_tile(&mut self, x: i32, y: i32, t: u8) {
        if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H {
            self.tile_type[(y as usize) * W + (x as usize)] = t;
        }
    }
}

/// Farm, road and paved colours per planet biome (the JS `BIOMES` palettes).
fn land_use_colours(biome: &str) -> [[u8; 3]; 6] {
    // farm A, farm B, road A, road B, paved A, paved B
    match biome {
        "alien" => [[230, 140, 210], [150, 100, 190], [150, 128, 98], [142, 121, 92], [146, 146, 152], [136, 136, 144]],
        _ => [[196, 170, 74], [120, 158, 60], [150, 128, 98], [142, 121, 92], [146, 146, 152], [136, 136, 144]],
    }
}

/// The base colour layer with land use painted on: tiles whose type differs from the base type
/// (farms in stripes, roads, paving) get their colours, everything else keeps the terrain's.
pub fn paint_land_use(base: &[u8], types: &[u8], base_types: &[u8], w: usize, biome: &str) -> Vec<u8> {
    let c = land_use_colours(biome);
    let mut out = base.to_vec();
    for (i, (&t, &b)) in types.iter().zip(base_types.iter()).enumerate() {
        if t == b {
            continue;
        }
        let (x, y) = (i % w, i / w);
        let k = match t {
            T_FARM => (y >> 1) % 2,
            T_ROAD => 2 + (x + y) % 2,
            T_PAVED => 4 + (x + y) % 2,
            _ => continue,
        };
        out[i * 4..i * 4 + 3].copy_from_slice(&c[k]);
        out[i * 4 + 3] = 255;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_generation_and_eras() {
        let mut terrain = Terrain::new(12345);
        assert_eq!(terrain.h.len(), W * H);
        assert_eq!(terrain.tile_type.len(), W * H);
        assert_eq!(terrain.pixels.len(), W * H * 4);

        // Village site must be walkable
        assert!(terrain.walkable(terrain.sites.village.x, terrain.sites.village.y));

        // Test baking all eras
        for era in 0..=3 {
            terrain.bake_era(era);
            // Pixels should not be all zero
            let non_zero = terrain.pixels.iter().any(|&p| p != 0);
            assert!(non_zero, "Pixels should be non-zero after baking era {}", era);
        }
    }
}
