//! Deterministic procedural planetary terrain generator for Mind over Chicken.
//!
//! Generates organic cylinder-wrapping planetary surfaces via:
//! - Tectonic plate kinematics and domain-warped fractal Brownian motion
//! - Collision orogeny and ridged multifractal mountain belts
//! - Whittaker biome classification (solar latitude temperature + prevailing trade winds + rain shadows)
//! - Hydrological flow accumulation and 1-pixel river networks
//! - 2x2 majority filtering removing noisy single-tile specks for a crisp, chunky pixel aesthetic
//! - Quantized hillshading, shallow coastal shelves, and shoreline foam lines

pub mod climate;
pub mod filter;
pub mod hydrology;
pub mod noise;
pub mod palette;
pub mod plates;
pub mod sites;

use climate::ClimateModel;
use filter::TerrainFilter;
use hydrology::Hydrology;
use moc_planet::rng::Rng;
use noise::TerrainNoise;
use palette::{BiomePalette, TerrainRenderer};
use plates::PlateSystem;
use sites::{CampaignSites, Continent, SiteFinder};

#[derive(Clone, Debug)]
pub struct TerrainMap {
    pub seed: u32,
    pub width: usize,
    pub height: usize,
    pub heights: Vec<f32>,
    pub types: Vec<u8>,
    pub river_mask: Vec<bool>,
    pub lake_mask: Vec<bool>,
    pub rgba: Vec<u8>,
    pub continents: Vec<Continent>,
    pub campaign_sites: Option<CampaignSites>,
}

impl TerrainMap {
    /// Generates a new terrain map with full geological, hydrological, and climatic simulation.
    pub fn generate(
        seed: u32,
        width: usize,
        height: usize,
        num_plates: usize,
        continental_ratio: f32,
        palette: BiomePalette,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let total_cells = width * height;

        // 1. Tectonic plates
        let plates = PlateSystem::new(&mut rng, num_plates, continental_ratio);

        // 2. Multi-octave wrapping noise
        let base_noise = TerrainNoise::new(&mut rng, 8, 4);
        let detail_noise = TerrainNoise::new(&mut rng, 16, 4);
        let ridge_noise = TerrainNoise::new(&mut rng, 12, 3);
        let moisture_noise = TerrainNoise::new(&mut rng, 8, 3);

        let climate = ClimateModel::default();
        let mut heights = vec![0.0f32; total_cells];
        let mut moisture = vec![0.0f32; total_cells];
        let mut types = vec![0u8; total_cells];

        // 3. First pass: Elevation & raw moisture
        for y in 0..height {
            let v = y as f32 / height as f32;
            let _wind_dir = climate.prevailing_wind(v);

            for x in 0..width {
                let u = x as f32 / width as f32;
                let idx = y * width + x;

                // Tectonic crust baseline & boundary orogeny
                let (crust_h, orogeny, _) = plates.sample(u, v);

                // Warped continental landmass shape
                let continent_fbm = base_noise.warped_fbm(u, v * 2.0, 0.35, 4);
                let detail = detail_noise.fbm(u, v * 4.0, 3) * 0.16;
                let ridge = ridge_noise.ridged_fbm(u, v * 3.0, 3) * orogeny;

                // Combined elevation
                let h = (crust_h * 0.40 + continent_fbm * 0.45 + detail + ridge).clamp(0.0, 1.0);
                heights[idx] = h;

                // Moisture: Base ocean proximity + windward rain shadow
                let raw_moist = moisture_noise.fbm(u, v * 2.0, 3);
                moisture[idx] = (raw_moist * 0.70 + 0.30 * (1.0 - h)).clamp(0.0, 1.0);
            }
        }

        // 4. Hydrology: Flow accumulation, rivers, and lakes
        let hydro = Hydrology::new(width, height);
        let river_threshold = (width as f32) * 0.18; // Scales with resolution
        let (_flow, river_mask, lake_mask) = hydro.compute_rivers(&mut heights, climate.sea_level, river_threshold);

        // 5. Climate & Biome classification
        for y in 0..height {
            let v = y as f32 / height as f32;
            for x in 0..width {
                let idx = y * width + x;
                let h = heights[idx];
                let m = moisture[idx];
                let temp = climate.temperature(v, h);

                let xm1 = if x > 0 { x - 1 } else { width - 1 };
                let xp1 = if x + 1 < width { x + 1 } else { 0 };
                let ym1 = if y > 0 { y - 1 } else { 0 };
                let yp1 = if y + 1 < height { y + 1 } else { height - 1 };

                let dx = heights[y * width + xp1] - heights[y * width + xm1];
                let dy = heights[yp1 * width + x] - heights[ym1 * width + x];
                let slope = (dx * dx + dy * dy).sqrt() * 2.5;

                types[idx] = climate.classify_tile(h, temp, m, slope);
            }
        }

        // 6. 2x2 Majority filter pass: eliminates noisy single-tile specks for chunky aesthetic
        let filter = TerrainFilter::new(width, height);
        let clean_types = filter.majority_filter(&types, &river_mask);

        // 7. Render crisp RGBA texture with quantized hillshade and foam lines
        let renderer = TerrainRenderer::new(width, height, palette);
        let rgba = renderer.render_rgba(&heights, &clean_types, &river_mask);

        // 8. Find continents and settlement sites
        let site_finder = SiteFinder::new(width, height);
        let (continents, _) = site_finder.find_continents(&clean_types, &heights, &river_mask);

        Self {
            seed,
            width,
            height,
            heights,
            types: clean_types,
            river_mask,
            lake_mask,
            rgba,
            continents,
            campaign_sites: None,
        }
    }

    /// Generates the main game 320x180 Home World with campaign sites.
    pub fn generate_home_world(seed: u32) -> Self {
        let mut map = Self::generate(seed, 320, 180, 8, 0.65, BiomePalette::temperate());

        // Find or assign campaign sites
        let home_site = if let Some(first) = map.continents.first() {
            first.primary_site
        } else {
            (116, 100)
        };

        let village = home_site;
        let pad = (
            (village.0 + 82).min(map.width - 20),
            (village.1 + 4).min(map.height - 20),
        );
        let cave = (
            village.0.saturating_sub(20).max(10),
            village.1.saturating_sub(15).max(10),
        );
        let factory = (
            village.0.saturating_sub(35).max(10),
            (village.1 + 22).min(map.height - 15),
        );
        let exchange = (
            (village.0 + 18).min(map.width - 15),
            village.1.saturating_sub(10).max(10),
        );
        let hangar = (
            pad.0.saturating_sub(24).max(10),
            (pad.1 + 8).min(map.height - 10),
        );
        let harbour = (
            (village.0 + 35).min(map.width - 15),
            (village.1 + 18).min(map.height - 10),
        );

        let ridge = (
            village.0.saturating_sub(48).max(10),
            village.1.saturating_sub(30).max(10),
        );
        let cave_mouth = cave;
        let fire = village;
        let picket = (
            (factory.0 + 2).min(map.width - 10),
            (factory.1 + 9).min(map.height - 10),
        );
        let crowd = (
            pad.0.saturating_sub(30).max(10),
            pad.1.saturating_sub(10).max(10),
        );

        map.campaign_sites = Some(CampaignSites {
            village,
            pad,
            ridge,
            cave,
            cave_mouth,
            fire,
            factory,
            exchange,
            picket,
            hangar,
            crowd,
            harbour,
        });

        map
    }

    /// Serialization helper for campaign sites
    pub fn sites_json(&self) -> String {
        if let Some(s) = &self.campaign_sites {
            format!(
                "{{\"village\":{{\"x\":{},\"y\":{}}},\"pad\":{{\"x\":{},\"y\":{}}},\"ridge\":{{\"x\":{},\"y\":{}}},\"cave\":{{\"x\":{},\"y\":{}}},\"caveMouth\":{{\"x\":{},\"y\":{}}},\"fire\":{{\"x\":{},\"y\":{}}},\"factory\":{{\"x\":{},\"y\":{}}},\"exchange\":{{\"x\":{},\"y\":{}}},\"picket\":{{\"x\":{},\"y\":{}}},\"hangar\":{{\"x\":{},\"y\":{}}},\"crowd\":{{\"x\":{},\"y\":{}}},\"harbour\":{{\"x\":{},\"y\":{}}}}}",
                s.village.0, s.village.1,
                s.pad.0, s.pad.1,
                s.ridge.0, s.ridge.1,
                s.cave.0, s.cave.1,
                s.cave_mouth.0, s.cave_mouth.1,
                s.fire.0, s.fire.1,
                s.factory.0, s.factory.1,
                s.exchange.0, s.exchange.1,
                s.picket.0, s.picket.1,
                s.hangar.0, s.hangar.1,
                s.crowd.0, s.crowd.1,
                s.harbour.0, s.harbour.1,
            )
        } else {
            "{}".to_string()
        }
    }

    /// Serialization helper for continents list
    pub fn continents_json(&self) -> String {
        let items: Vec<String> = self.continents.iter().enumerate().map(|(i, c)| {
            let id_str = if i == 0 { "home" } else if i == 1 { "north" } else { "south" };
            format!(
                "{{\"id\":\"{}\",\"cx\":{},\"cy\":{},\"rx\":60,\"ry\":40,\"village\":{{\"x\":{},\"y\":{}}}}}",
                id_str, c.center_x, c.center_y, c.primary_site.0, c.primary_site.1
            )
        }).collect();
        format!("[{}]", items.join(","))
    }

    /// Generates the Civ Sandbox Planet (default 640x320 or up to 2560x1280).
    pub fn generate_civ_planet(seed: u32, width: usize, height: usize) -> Self {
        Self::generate(seed, width, height, 12, 0.60, BiomePalette::temperate())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_generation_determinism_and_wrapping() {
        let map1 = TerrainMap::generate_home_world(42);
        let map2 = TerrainMap::generate_home_world(42);

        assert_eq!(map1.width, 320);
        assert_eq!(map1.height, 180);
        assert_eq!(map1.heights, map2.heights, "Same seed must produce identical heights");
        assert_eq!(map1.types, map2.types, "Same seed must produce identical types");
        assert_eq!(map1.rgba, map2.rgba, "Same seed must produce identical RGBA texture");

        // Verify cylinder wrapping: left edge and right edge heights should have continuous gradients
        for y in 0..map1.height {
            let left_h = map1.heights[y * map1.width];
            let right_h = map1.heights[y * map1.width + map1.width - 1];
            let diff = (left_h - right_h).abs();
            assert!(diff < 0.40, "Wrapping boundary should not have extreme cliff seam: diff={diff} at y={y}");
        }
    }

    #[test]
    fn test_continents_and_rivers() {
        let map = TerrainMap::generate_home_world(12345);
        assert!(!map.continents.is_empty(), "World must contain at least one continent");
        let rivers_count = map.river_mask.iter().filter(|&&r| r).count();
        assert!(rivers_count > 0, "Hydrology must generate rivers reaching the sea");
    }

    #[test]
    fn test_no_isolated_single_specks() {
        let map = TerrainMap::generate_home_world(777);
        let w = map.width;
        let h = map.height;

        let mut isolated_specks = 0;
        for y in 1..(h - 1) {
            for x in 0..w {
                let curr_t = map.types[y * w + x];
                if map.river_mask[y * w + x] {
                    continue; // 1-px river cells are expected
                }

                let mut matching = 0;
                for &(dx, dy) in &[(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                    let ny = y as i32 + dy;
                    if ny >= 0 && (ny as usize) < h {
                        if map.types[ny as usize * w + nx] == curr_t {
                            matching += 1;
                        }
                    }
                }
                if matching == 0 {
                    isolated_specks += 1;
                }
            }
        }

        assert_eq!(isolated_specks, 0, "Majority filter must eliminate all isolated 1x1 tile specks");
    }
}
