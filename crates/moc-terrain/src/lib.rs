//! # moc-terrain
//!
//! One terrain generator for every map in Mind over Chicken (#109): the main game's home
//! world, the galaxy planets' surfaces and the civ sandbox planet. Pure Rust, no
//! dependencies, deterministic per seed, wasm-friendly.
//!
//! Every map is a torus (#111): it wraps east–west and north–south, with periodic climate belts
//! instead of hard poles.
//!
//! ## Pipeline ("procedural diffusion": DLA + diffusion smoothing + erosion)
//!
//! 1. **Continents and plates** ([`tectonics`], coarse grid): continents grow from scattered
//!    cores to their target areas, never touching; plates are a warped Voronoi diagram with a
//!    motion each.
//! 2. **Base relief** (simulation grid): signed distance to the coast + domain-warped fBm on
//!    the torus, plate uplift, ridged noise along convergent boundaries.
//! 3. **Mountain ranges by DLA** ([`dla`]): clusters grown from seeds on convergent boundaries,
//!    upscaled ×2 per level with jitter, turned into heights by distance to the tips and blurred
//!    in several passes into ridges with branching valleys; blended with the base.
//! 4. **Islands**: island arcs on oceanic convergent boundaries and hotspot chains.
//! 5. **Erosion and diffusion** ([`erosion`]): thermal, stream power, Laplacian smoothing of
//!    coasts and lowlands.
//! 6. **Climate** ([`climate`]): temperature from the belts and altitude; moisture from the sea,
//!    prevailing winds and rain shadow.
//! 7. **Water** ([`hydro`]): priority-flood drainage, lakes in basins, rivers by flow
//!    accumulation.
//! 8. **Detail** (the stage-2 hook, [`ReliefDetail`]): coarse heights to full resolution.
//! 9. **Biomes, tiles and the colour layer** ([`paint`]): Whittaker biomes, posterised height
//!    steps, quantised hillshade, shelf, foam and rivers; the majority filter keeps every
//!    feature at least 2×2.
//! 10. **Continents and sites** ([`sites`]).

pub mod climate;
pub mod dla;
pub mod erosion;
pub mod filter;
pub mod grid;
pub mod hydrology;
pub mod landform;
pub mod metrics;
pub mod noise;
pub mod palette;
pub mod plates;
pub mod rng;
pub mod sites;

use climate::ClimateParams;
use dla::DlaParams;
use grid::smoothstep;
use noise::{Fbm, Torus, Warp};
use climate::{biome, relief};
use palette::{Bands, BiomePalette};

pub use grid::Dims;
pub use rng::Rng;
pub use sites::{CampaignSites, Continent, GameContinent, Site};

/// Pin continent 0 (the "home" continent) to a place.
#[derive(Clone, Debug)]
pub struct Anchor {
    /// Core position, tiles.
    pub x: f32,
    pub y: f32,
    /// Its area relative to the mean continent area.
    pub share: f32,
    /// Keep it inside this box (x0, y0, x1, y1 tiles).
    pub bounds: Option<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub struct ErosionParams {
    pub thermal_passes: usize,
    pub stream_iters: usize,
    /// Stream-power erodibility.
    pub k: f32,
    pub diffuse_passes: usize,
}

impl Default for ErosionParams {
    fn default() -> Self {
        ErosionParams { thermal_passes: 3, stream_iters: 3, k: 0.0022, diffuse_passes: 3 }
    }
}

/// Everything that shapes a map. Use a preset ([`TerrainParams::home_world`],
/// [`TerrainParams::galaxy_surface`], [`TerrainParams::civ_planet`]) and adjust.
#[derive(Clone, Debug)]
pub struct TerrainParams {
    pub w: usize,
    pub h: usize,
    pub seed: u32,
    /// Wrap east–west / north–south. Every game map is a torus (#111).
    pub wrap_x: bool,
    pub wrap_y: bool,
    /// Major landmasses.
    pub continents: usize,
    /// Mean target area of a continent, tiles (0: from `land_fraction`).
    pub continent_area: f32,
    /// Share of the map that is continent (used when `continent_area` is 0).
    pub land_fraction: f32,
    /// 0: equal continents; 0.5: up to ±50%.
    pub size_spread: f32,
    /// > 0: continent sizes follow a power law, the k-th largest ∝ (k+1)^-size_power (Alex:
    /// one or two big continents, a few mid-sized, then islands).
    pub size_power: f32,
    pub anchor: Option<Anchor>,
    /// Ocean kept between continents, tiles (0: automatic).
    pub gap: f32,
    /// Ocean kept along both seams, tiles (0: none). For maps whose gameplay doesn't wrap.
    pub seam_margin: f32,
    /// Island chains: 0 none, 1 plenty.
    pub islands: f32,
    /// Tectonic plates (0: automatic).
    pub plates: usize,
    /// Output height of the coast: heights are 0..1 with water below this (0.33, like the old
    /// maps, so the 3D view's water level is unchanged).
    pub sea_level: f32,
    pub climate: ClimateParams,
    pub palette: BiomePalette,
    /// Mountain range height multiplier.
    pub mountains: f32,
    /// Coastline and continent irregularity.
    pub roughness: f32,
    pub dla: DlaParams,
    pub erosion: ErosionParams,
    /// Tiles per simulation cell (0: automatic, keeps the heavy stages near 200k cells).
    pub sim_cell: usize,
    /// Drainage area (tiles) that makes a river (0: automatic).
    pub river_area: f32,
    pub max_sites: usize,
    pub bands: Bands,
    /// Smallest landmass, as a share of the map area (Alex: 0.5%). Smaller ones are removed.
    pub min_island: f32,
    /// Lay out the main game's campaign (village, pad, cave, harbours) on this map.
    pub campaign: bool,
    /// Continents stay out of the middle of the cold belt beyond this coldness (0..1).
    pub cold_limit: f32,
}

impl TerrainParams {
    fn base(w: usize, h: usize, seed: u32) -> Self {
        TerrainParams {
            w,
            h,
            seed,
            wrap_x: true,
            wrap_y: true,
            continents: 4,
            continent_area: 0.0,
            land_fraction: 0.38,
            size_spread: 0.3,
            size_power: 0.0,
            anchor: None,
            gap: 0.0,
            seam_margin: 0.0,
            islands: 0.5,
            plates: 0,
            sea_level: 0.33,
            climate: ClimateParams::default(),
            palette: BiomePalette::temperate(),
            mountains: 1.0,
            roughness: 1.0,
            dla: DlaParams::default(),
            erosion: ErosionParams::default(),
            sim_cell: 0,
            river_area: 0.0,
            max_sites: 32,
            bands: Bands::default(),
            min_island: 0.005,
            campaign: false,
            cold_limit: 0.86,
        }
    }

    /// The main game's home world: 320×180, a big home continent and two or three smaller
    /// ones across the sea. Gameplay paths don't cross the seams, so a band of ocean runs
    /// along them (the terrain itself is still a seamless torus).
    pub fn home_world(seed: u32) -> Self {
        Self::home_world_sized(seed, 320, 180)
    }

    /// The home world at any size (#129: 640×360, at least five continents; 320×180 until the
    /// game switches). Tile distances scale with the map.
    pub fn home_world_sized(seed: u32, w: usize, h: usize) -> Self {
        let k = ((w * h) as f32 / (320.0 * 180.0)).sqrt();
        let mut r = Rng::stream(seed, 0x40E);
        let mut p = TerrainParams::base(w, h, seed);
        // Six or seven planned, so at least five major ones survive the crowding (#129).
        p.continents = 6 + (r.f32() < 0.4) as usize;
        p.land_fraction = r.range(0.4, 0.48);
        p.size_spread = 0.15;
        p.size_power = r.range(0.7, 0.9);
        let m = 14.0 * k;
        // Home lands anywhere away from the seams, so every run's map is laid out differently.
        p.anchor = Some(Anchor { x: w as f32 * r.range(0.2, 0.8), y: h as f32 * r.range(0.3, 0.7), share: r.range(1.3, 1.9), bounds: Some([m, m * 1.1, w as f32 - m, h as f32 - m * 1.1]) });
        p.gap = 5.0 * k;
        p.seam_margin = 5.0 * k;
        // A mild world: land may reach well into the cool belt along the top and bottom.
        p.cold_limit = 0.97;
        p.islands = 0.8;
        p.plates = (9.0 * k * k.sqrt()).round() as usize;
        p.climate = ClimateParams { warm: 25.0, cold: -5.0, bands: 1.0, moisture: 1.0, lapse: 30.0 };
        p.max_sites = (24.0 * k * k) as usize;
        // The heavy stages run on a grid half the size; the detail step brings it to full size
        // (< 50 ms for 320×180).
        p.sim_cell = 2;
        p.campaign = true;
        p
    }

    /// A planet biome's colours and climate (temperate, desert, ice, jungle, alien).
    pub fn set_biome(&mut self, biome: &str) {
        self.palette = BiomePalette::named(biome);
        let base = ClimateParams { bands: self.climate.bands, ..ClimateParams::default() };
        self.climate = match biome {
            "desert" => ClimateParams { warm: 34.0, cold: -8.0, moisture: 0.45, ..base },
            "ice" => ClimateParams { warm: 6.0, cold: -40.0, moisture: 0.8, ..base },
            "jungle" => ClimateParams { warm: 32.0, cold: -4.0, moisture: 1.5, ..base },
            "alien" => ClimateParams { warm: self.climate.warm, cold: self.climate.cold, ..base },
            _ => self.climate.clone(),
        };
    }

    /// A galaxy planet's surface: 640×360. Continent 0 is the capital's, kept inside the
    /// 320×180 window at (0, 90) where the AI tribe's World lives. `biome`: the planet's climate
    /// and palette (temperate, desert, ice, jungle, alien).
    pub fn galaxy_surface(seed: u32, biome: &str) -> Self {
        let (w, h) = (640, 360);
        let mut r = Rng::stream(seed, 0x6A1);
        let mut p = TerrainParams::base(w, h, seed);
        p.continents = 6 + r.below(2);
        p.land_fraction = 0.36;
        p.size_power = r.range(0.9, 1.15);
        p.anchor = Some(Anchor { x: 160.0 + r.range(-20.0, 20.0), y: 180.0 + r.range(-8.0, 8.0), share: 1.5, bounds: Some([14.0, 104.0, 306.0, 256.0]) });
        p.gap = 10.0;
        p.islands = 0.6;
        p.plates = 14;
        p.set_biome(biome);
        p.max_sites = 40;
        p.sim_cell = 2;
        p
    }

    /// The civ sandbox planet (#101): 3–5 major continents of about 330k land tiles each (10×
    /// the old average), plus island chains, on at least 2560×1280.
    pub fn civ_planet(seed: u32, w: usize, h: usize) -> Self {
        let mut r = Rng::stream(seed, 0xC1F);
        let mut p = TerrainParams::base(w, h, seed);
        p.continents = 3 + r.below(3);
        let budget = 0.44 * (w * h) as f32;
        p.continent_area = (330_000.0f32).min(budget / p.continents as f32);
        p.size_spread = 0.12;
        p.islands = 1.0;
        p.plates = 26;
        p.gap = 48.0;
        p.max_sites = 160;
        p
    }

    fn sim_cell(&self) -> usize {
        if self.sim_cell > 0 {
            return self.sim_cell;
        }
        let mut cs = 1;
        while (self.w * self.h) / (cs * cs) > 240_000 && self.w % (cs * 2) == 0 && self.h % (cs * 2) == 0 {
            cs *= 2;
        }
        cs
    }
}

/// What the detail stage starts from: the large-scale relief (continents, plate uplift, broad
/// hills, islands) on a coarse grid. Units: sea level 0.0, land up to ~0.35 before mountains,
/// sea floor down to -1.0.
pub struct DetailInput<'a> {
    /// Coarse heights, row-major `coarse_dims.w` × `coarse_dims.h`.
    pub coarse: &'a [f32],
    pub coarse_dims: Dims,
    /// The output grid: the full map, in tiles (`dims.w == coarse_dims.w * cell`).
    pub dims: Dims,
    /// Tiles per coarse cell.
    pub cell: usize,
    /// 0..1 per coarse cell: where mountain ranges belong (convergent plate boundaries, old
    /// worn ranges). Conditioning; a model may ignore it.
    pub uplift: &'a [f32],
    /// 0..1 per coarse cell: land weight (0 open sea, 1 inland). Conditioning.
    pub land: &'a [f32],
    /// Typical continent size, tiles.
    pub scale: f32,
}

/// **The detail hook** (#109 stage 1, #110 stage 2): the single place relief detail is
/// produced. Gets the coarse heightmap and an RNG, returns detailed heights for every tile of
/// `input.dims` (row-major) in the same units (sea 0, highest peaks ≈ 1, sea floor ≥ -1). It
/// must wrap seamlessly on both axes and keep the coarse land/sea layout: climate, rivers,
/// lakes, the coast rules, biomes and colours are all computed from its output.
///
/// The default is [`ProceduralDiffusion`]. A learned diffusion model (trained on real
/// elevation data) implements this trait and is passed to [`generate_with`]; nothing else
/// changes.
pub trait ReliefDetail {
    fn detail(&self, input: &DetailInput, rng: &mut Rng) -> Vec<f32>;
}

/// Stage 1, procedural diffusion: DLA mountain ranges grown along `uplift` and diffused on a
/// pyramid into ridges and branching valleys, thermal and stream-power erosion, Laplacian
/// smoothing of coasts and lowlands, then upsampling with fine noise.
#[derive(Clone, Debug)]
pub struct ProceduralDiffusion {
    pub dla: DlaParams,
    pub erosion: ErosionParams,
    /// Range height multiplier.
    pub mountains: f32,
}

impl Default for ProceduralDiffusion {
    fn default() -> Self {
        ProceduralDiffusion { dla: DlaParams::default(), erosion: ErosionParams::default(), mountains: 1.0 }
    }
}

/// What the procedural diffusion did, for inspection and tests.
#[derive(Clone, Debug, Default)]
pub struct DiffusionInfo {
    /// Each DLA cluster (a mountain range): nodes, tips, forks.
    pub ranges: Vec<dla::ClusterStats>,
    /// The diffused ranges (0..1) and the corridor they grew in, coarse grid.
    pub dla: Vec<f32>,
    pub corridor: Vec<bool>,
    pub dla_nodes: usize,
}

impl ReliefDetail for ProceduralDiffusion {
    fn detail(&self, input: &DetailInput, rng: &mut Rng) -> Vec<f32> {
        self.run(input, rng).0
    }
}

impl ProceduralDiffusion {
    /// The detail and what it did.
    pub fn run(&self, inp: &DetailInput, rng: &mut Rng) -> (Vec<f32>, DiffusionInfo) {
        let sd = inp.coarse_dims;
        let n = sd.n();
        let csf = inp.cell as f32;
        let scale = inp.scale;
        let mut h = inp.coarse.to_vec();

        // DLA ranges: roots spaced along the corridors, grown, diffused, added with foothills.
        let corridor: Vec<bool> = inp.uplift.iter().map(|&u| u > 0.0).collect();
        let spacing = (0.22 * scale / csf).max(4.0);
        let mut seeds: Vec<(i32, i32)> = Vec::new();
        let mut cand: Vec<usize> = (0..n).filter(|&i| corridor[i]).collect();
        for k in (1..cand.len()).rev() {
            let j = rng.below(k + 1);
            cand.swap(k, j);
        }
        for &i in cand.iter() {
            let (x, y) = sd.xy(i);
            if seeds.iter().all(|&(sx, sy)| sd.dist2(sx as f32, sy as f32, x as f32, y as f32) > spacing * spacing) {
                seeds.push((x, y));
            }
        }
        let mut dp = self.dla.clone();
        // Enough levels that the coarsest grid is ~24 cells across.
        dp.levels = dp.levels.max(((sd.w.min(sd.h) as f32 / 24.0).log2().floor() as u32).clamp(2, 5));
        let ranges = dla::grow(sd, &corridor, &seeds, &dp, rng.u32());
        let mut foot = ranges.height.clone();
        grid::blur(sd, &mut foot, (0.05 * scale / csf).max(3.0) as usize);
        for i in 0..n {
            let strength = 0.45 + 0.45 * inp.uplift[i].min(1.6);
            h[i] += (ranges.height[i] * 0.74 * strength + foot[i] * 0.2) * self.mountains * inp.land[i];
        }

        // Erosion and diffusion.
        let e = &self.erosion;
        erosion::thermal(sd, &mut h, 0.0, 0.09, 0.5, e.thermal_passes);
        erosion::stream_power(sd, &mut h, 0.0, e.k, 0.5, e.stream_iters, csf);
        erosion::thermal(sd, &mut h, 0.0, 0.09, 0.4, 1);
        erosion::diffuse_lowlands(sd, &mut h, 0.0, 0.16, 0.5, e.diffuse_passes);
        // Peaks to about 1.
        let mut land: Vec<f32> = h.iter().copied().filter(|&v| v > 0.0).collect();
        if !land.is_empty() {
            let k = ((land.len() as f32 * 0.998) as usize).min(land.len() - 1);
            let top = *land.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1;
            if top > 0.92 {
                let s = 0.92 / top;
                for v in h.iter_mut() {
                    if *v > 0.0 {
                        *v *= s;
                    }
                }
            }
        }
        for v in h.iter_mut() {
            *v = v.max(-1.0);
        }

        // Up to the full grid, with two octaves of noise finer than the coarse grid (on a
        // lattice that repeats across the map, so it stays seamless both ways): stronger in the
        // mountains and along the coast, so coastlines get finer than the coarse grid.
        let mut out = if inp.cell <= 1 { h.clone() } else { grid::resample(sd, &h, inp.dims) };
        if inp.cell > 1 {
            let (w, hh) = (inp.dims.w, inp.dims.h);
            let wl = (csf * 2.5).max(4.0);
            let o1 = noise::Periodic2::new(rng, w, hh, wl);
            let o2 = noise::Periodic2::new(rng, w, hh, wl * 0.5);
            for y in 0..hh {
                for x in 0..w {
                    let i = y * w + x;
                    let v = out[i];
                    let amp = 0.006 + 0.07 * smoothstep(0.08, 0.6, v) + 0.035 * (1.0 - smoothstep(0.0, 0.06, v.abs()));
                    let (xf, yf) = (x as f32, y as f32);
                    out[i] = v + (o1.sample(xf, yf) * 0.67 + o2.sample(xf, yf) * 0.33) * amp;
                }
            }
        }
        let info = DiffusionInfo { dla_nodes: ranges.nodes.len(), ranges: ranges.clusters, dla: ranges.height, corridor };
        (out, info)
    }
}

/// Timings of each stage, ms (zero on wasm, where there is no clock).
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub stages: Vec<(&'static str, f64)>,
    pub sim_cell: usize,
    pub dla_nodes: usize,
    /// What the coastline cleaner changed.
    pub coast: filter::CleanStats,
    /// The smallest landmass allowed, tiles.
    pub min_island: usize,
    /// How the continents are laid out and shaped.
    pub layout: landform::Layout,
    /// Each planned continent's character.
    pub characters: Vec<&'static str>,
}

/// A generated map (the name and the first fields follow #112).
pub struct TerrainMap {
    pub seed: u32,
    pub width: usize,
    pub height: usize,
    pub wrap_x: bool,
    pub wrap_y: bool,
    pub sea_level: f32,
    /// Heights 0..1, water below `sea_level`.
    pub heights: Vec<f32>,
    /// [`climate::biome`] per tile.
    pub biome: Vec<u8>,
    /// [`climate::relief`] per tile.
    pub relief: Vec<u8>,
    /// Game tile ([`climate::tile`], the JS `T` codes).
    pub types: Vec<u8>,
    /// 1-px rivers.
    pub river_mask: Vec<bool>,
    pub lake_mask: Vec<bool>,
    /// °C.
    pub temperature: Vec<f32>,
    /// 0..1.
    pub moisture: Vec<f32>,
    /// The base colour layer, RGBA.
    pub rgba: Vec<u8>,
    /// Continent per tile: index + 1 into `continents`, 0 = water.
    pub continent: Vec<u16>,
    pub continents: Vec<Continent>,
    /// Settlement sites, best first.
    pub sites: Vec<Site>,
    /// The main game's places, when the params asked for a campaign (#112's name).
    pub campaign_sites: Option<CampaignSites>,
    /// The landmasses the campaign uses: home first, then those boats can reach.
    pub game_continents: Vec<GameContinent>,
    /// The DLA mountain clusters (for inspection and tests).
    pub ranges: Vec<dla::ClusterStats>,
    /// River network on the simulation grid: (cell, receiver) and its size, for tests.
    pub sim: SimInfo,
    pub stats: Stats,
}

/// The simulation grid's water network, kept for checks.
#[derive(Clone, Debug, Default)]
pub struct SimInfo {
    pub w: usize,
    pub h: usize,
    pub cell: usize,
    pub height: Vec<f32>,
    pub recv: Vec<u32>,
    pub river: Vec<bool>,
    pub lake: Vec<bool>,
    pub ocean: Vec<bool>,
    /// The diffused DLA ranges (0..1) and the corridor they grew in.
    pub dla: Vec<f32>,
    pub corridor: Vec<bool>,
}

impl TerrainMap {
    /// The main game's home world, 320×180 (#112's entry point).
    pub fn generate_home_world(seed: u32) -> Self {
        generate(&TerrainParams::home_world(seed))
    }

    /// A galaxy planet's surface, 640×360, for a planet biome (temperate, desert, ice, jungle,
    /// alien).
    pub fn generate_planet_surface(seed: u32, biome: &str) -> Self {
        generate(&TerrainParams::galaxy_surface(seed, biome))
    }

    /// The civ sandbox planet (#112's entry point; #101 sizes at 2560×1280 and up).
    pub fn generate_civ_planet(seed: u32, width: usize, height: usize) -> Self {
        generate(&TerrainParams::civ_planet(seed, width, height))
    }

    /// #112's general entry point: `continental_ratio` (0..1) sets how much of the map is land.
    pub fn generate(seed: u32, width: usize, height: usize, num_plates: usize, continental_ratio: f32, palette: BiomePalette) -> Self {
        let mut p = TerrainParams::civ_planet(seed, width, height);
        p.plates = num_plates;
        p.continent_area = 0.0;
        p.land_fraction = (0.2 + 0.3 * continental_ratio).clamp(0.2, 0.5);
        p.palette = palette;
        generate(&p)
    }

    /// The campaign sites as JSON, with the JS `World.sites` names.
    pub fn sites_json(&self) -> String {
        let Some(s) = &self.campaign_sites else { return "{}".into() };
        let pt = |p: (usize, usize)| format!("{{\"x\":{},\"y\":{}}}", p.0, p.1);
        format!(
            "{{\"village\":{},\"pad\":{},\"ridge\":{},\"cave\":{},\"caveMouth\":{},\"fire\":{},\"factory\":{},\"exchange\":{},\"picket\":{},\"hangar\":{},\"crowd\":{},\"harbour\":{}}}",
            pt(s.village), pt(s.pad), pt(s.ridge), pt(s.cave), pt(s.cave_mouth), pt(s.fire), pt(s.factory), pt(s.exchange), pt(s.picket), pt(s.hangar), pt(s.crowd), pt(s.harbour)
        )
    }

    /// The campaign's landmasses as JSON, as the JS `World.continents` expects them: id, centre,
    /// radii, a village, and harbours ({land, dock}) facing home.
    pub fn continents_json(&self) -> String {
        let pt = |p: (i32, i32)| format!("{{\"x\":{},\"y\":{}}}", p.0, p.1);
        let hb = |h: &Option<((i32, i32), (i32, i32))>| match h {
            Some((l, d)) => format!("{{\"land\":{},\"dock\":{}}}", pt(*l), pt(*d)),
            None => "null".into(),
        };
        let items: Vec<String> = self
            .game_continents
            .iter()
            .map(|c| {
                format!(
                    "{{\"id\":\"{}\",\"cx\":{:.1},\"cy\":{:.1},\"rx\":{:.1},\"ry\":{:.1},\"area\":{},\"village\":{},\"harbor\":{},\"homeHarbor\":{}}}",
                    c.id, c.cx, c.cy, c.rx, c.ry, self.continents[c.index].area, pt(c.village), hb(&c.harbor), hb(&c.home_harbor)
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Per tile, which campaign landmass it belongs to (index into `game_continents`), 255 for
    /// water and landmasses the campaign doesn't use.
    pub fn owner(&self) -> Vec<u8> {
        let mut map = vec![255u8; self.continents.len() + 1];
        for (k, c) in self.game_continents.iter().enumerate() {
            map[c.index + 1] = k as u8;
        }
        self.continent.iter().map(|&c| map[c as usize]).collect()
    }

    pub fn dims(&self) -> Dims {
        Dims { w: self.width, h: self.height, wrap_x: self.wrap_x, wrap_y: self.wrap_y }
    }

    /// A `w`×`h` window at (x0, y0) as a map of its own (no wrap: a region of a bigger
    /// planet, like the galaxy capital the AI tribe lives on). Landmasses, sites and, with
    /// `campaign`, the campaign layout are worked out again for the window.
    pub fn crop(&self, x0: i32, y0: i32, w: usize, h: usize, campaign: bool) -> TerrainMap {
        let d = self.dims();
        let n = w * h;
        let src: Vec<usize> = (0..n).map(|i| d.idx(x0 + (i % w) as i32, y0 + (i / w) as i32).unwrap_or(0)).collect();
        let pick = |v: &[f32]| src.iter().map(|&j| v[j]).collect::<Vec<f32>>();
        let pick8 = |v: &[u8]| src.iter().map(|&j| v[j]).collect::<Vec<u8>>();
        let pickb = |v: &[bool]| src.iter().map(|&j| v[j]).collect::<Vec<bool>>();
        let biome_map = pick8(&self.biome);
        let types = pick8(&self.types);
        let cd = Dims::new(w, h, false);
        let (cont_id, continents) = sites::continents(cd, &biome_map, (self.stats.min_island / 2).max(1));
        let (region, _) = sites::regions(cd, &types);
        let in_win = |s: &Site| {
            let (dx, dy) = (d.dx(x0 as f32, s.x as f32), d.dy(y0 as f32, s.y as f32));
            let (dx, dy) = (if dx < 0.0 { dx + d.w as f32 } else { dx }, if dy < 0.0 { dy + d.h as f32 } else { dy });
            (dx as usize) < w && (dy as usize) < h
        };
        let site_list: Vec<Site> = self
            .sites
            .iter()
            .filter(|s| in_win(s))
            .filter_map(|s| {
                let x = (s.x - x0).rem_euclid(d.w as i32);
                let y = (s.y - y0).rem_euclid(d.h as i32);
                let i = y as usize * w + x as usize;
                (cont_id[i] > 0 && region[i] > 0).then(|| Site { x, y, continent: cont_id[i] as usize - 1, region: region[i], ..s.clone() })
            })
            .collect();
        let heights = pick(&self.heights);
        let hf: Vec<f32> = heights.iter().map(|&v| if v >= self.sea_level { (v - self.sea_level) / (1.0 - self.sea_level) } else { v / self.sea_level - 1.0 }).collect();
        let (campaign_sites, game_continents) = if campaign && !continents.is_empty() {
            let (cs, gc) = sites::campaign(&sites::CampaignInput { w, h, heights: &hf, types: &types, biome: &biome_map, continent: &cont_id, continents: &continents, sites: &site_list });
            (Some(cs), gc)
        } else {
            (None, Vec::new())
        };
        TerrainMap {
            seed: self.seed,
            width: w,
            height: h,
            wrap_x: false,
            wrap_y: false,
            sea_level: self.sea_level,
            heights,
            biome: biome_map,
            relief: pick8(&self.relief),
            types,
            river_mask: pickb(&self.river_mask),
            lake_mask: pickb(&self.lake_mask),
            temperature: pick(&self.temperature),
            moisture: pick(&self.moisture),
            rgba: src.iter().flat_map(|&j| self.rgba[j * 4..j * 4 + 4].iter().copied()).collect(),
            continent: cont_id,
            continents,
            sites: site_list,
            campaign_sites,
            game_continents,
            ranges: Vec::new(),
            sim: SimInfo::default(),
            stats: self.stats.clone(),
        }
    }
    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> Option<usize> {
        self.dims().idx(x, y)
    }
    pub fn tile_at(&self, x: i32, y: i32) -> u8 {
        self.idx(x, y).map(|i| self.types[i]).unwrap_or(climate::tile::DEEP)
    }
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        climate::tile::walkable(self.tile_at(x, y))
    }
}

struct Clock {
    #[cfg(not(target_arch = "wasm32"))]
    t: std::time::Instant,
    stages: Vec<(&'static str, f64)>,
}

impl Clock {
    fn new() -> Self {
        Clock {
            #[cfg(not(target_arch = "wasm32"))]
            t: std::time::Instant::now(),
            stages: Vec::new(),
        }
    }
    fn lap(&mut self, name: &'static str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let now = std::time::Instant::now();
            self.stages.push((name, (now - self.t).as_secs_f64() * 1000.0));
            self.t = now;
        }
        #[cfg(target_arch = "wasm32")]
        self.stages.push((name, 0.0));
    }
}

/// Generate a map, with the procedural diffusion as the detail stage.
pub fn generate(p: &TerrainParams) -> TerrainMap {
    build(p, None)
}

/// Generate a map with another detail stage (e.g. a learned diffusion model, #110).
pub fn generate_with(p: &TerrainParams, detail: &dyn ReliefDetail) -> TerrainMap {
    build(p, Some(detail))
}

fn build(p: &TerrainParams, custom: Option<&dyn ReliefDetail>) -> TerrainMap {
    let mut clock = Clock::new();
    let full = Dims { w: p.w, h: p.h, wrap_x: p.wrap_x, wrap_y: p.wrap_y };
    let cs = p.sim_cell();
    let sd = full.sized(p.w / cs, p.h / cs);
    let csf = cs as f32;
    let n_tiles = (p.w * p.h) as f32;
    let seed = p.seed;
    let mut rng = Rng::stream(seed, 0x1A2D);

    // ---- 1. continents and plates (growth grid) ----
    let nc = p.continents.max(1);
    let mean_area = if p.continent_area > 0.0 { p.continent_area } else { p.land_fraction * n_tiles / nc as f32 };
    let mut areas: Vec<f32> = (0..nc).map(|_| mean_area * (1.0 + p.size_spread * rng.range(-1.0, 1.0))).collect();
    if p.size_power > 0.0 {
        // Power law: the largest few times the smallest, biggest first (the anchored home).
        let w: Vec<f32> = (0..nc).map(|k| ((k + 1) as f32).powf(-p.size_power) * rng.range(0.85, 1.15)).collect();
        let sw: f32 = w.iter().sum();
        let mut a: Vec<f32> = w.iter().map(|&v| v / sw * mean_area * nc as f32).collect();
        a.sort_by(|x, y| y.total_cmp(x));
        areas = a;
    } else if let Some(a) = &p.anchor {
        areas[0] = mean_area * a.share;
        // The others share what's left of the land budget.
        if nc > 1 && p.continent_area == 0.0 {
            let rest = (mean_area * nc as f32 - areas[0]).max(mean_area * 0.25 * (nc - 1) as f32);
            let sum: f32 = areas[1..].iter().sum();
            for v in areas[1..].iter_mut() {
                *v *= rest / sum;
            }
        }
    }
    let scale = mean_area.sqrt(); // typical continent size, tiles
    let gap = if p.gap > 0.0 { p.gap } else { 0.08 * scale + 4.0 };
    let plan = plates::ContinentPlan {
        areas: areas.clone(),
        anchor: p.anchor.as_ref().map(|a| (a.x, a.y, a.bounds)),
        gap,
        seam_margin: p.seam_margin,
        cold_limit: p.cold_limit,
        bands: p.climate.bands,
        roughness: p.roughness,
        clusters: 0,
    };
    let gd0 = plates::growth_dims(p.w, p.h, p.wrap_x, 8_000);
    let gd = Dims { wrap_y: p.wrap_y, ..gd0 };
    let plates = if p.plates > 0 { p.plates } else { (n_tiles / (scale * scale * 0.4)).round().clamp(6.0, 40.0) as usize };
    let tect = plates::build(p.w, p.h, gd, &plan, plates, seed);
    let gcell = (tect.cell.0 + tect.cell.1) * 0.5;
    clock.lap("tectonics");

    // Growth-grid fields: signed coast distance (tiles), convergent boundaries, plate lift.
    let g_land: Vec<bool> = tect.owner.iter().map(|&o| o != plates::OCEAN).collect();
    let g_ocean: Vec<bool> = g_land.iter().map(|&b| !b).collect();
    let din = grid::distance(gd, &g_ocean);
    let dout = grid::distance(gd, &g_land);
    let sdist_g: Vec<f32> = (0..gd.n()).map(|i| if g_land[i] { (din[i] - 0.5) * gcell } else { -(dout[i] - 0.5) * gcell }).collect();
    // Territories: every cell belongs to its nearest continent. Along the lines where two
    // territories meet, the sea is kept open, so continents never join through noise (no
    // artificial land bridges; Alex, #109).
    let owner_val: Vec<f32> = tect.owner.iter().map(|&o| o as f32).collect();
    let (_, territory) = grid::distance_carry(gd, &g_land, &owner_val);
    let midline: Vec<bool> = (0..gd.n())
        .map(|i| {
            let mut edge = false;
            gd.nb4(i, |j| edge |= territory[j] != territory[i]);
            edge
        })
        .collect();
    let dmid_g: Vec<f32> = grid::distance(gd, &midline).iter().map(|v| v * gcell).collect();
    let conv_src: Vec<bool> = (0..gd.n()).map(|i| tect.boundary[i] && tect.conv[i] > 0.12).collect();
    let (convd_g, convv_g) = grid::distance_carry(gd, &conv_src, &tect.conv);
    let convd_g: Vec<f32> = convd_g.iter().map(|&v| v * gcell).collect();
    let lifts: Vec<f32> = (0..tect.plates.len()).map(|_| rng.range(-0.05, 0.07)).collect();
    let mut lift_g: Vec<f32> = tect.plate.iter().map(|&k| lifts[k as usize]).collect();
    grid::blur(gd, &mut lift_g, 4);
    let sdist = grid::resample(gd, &sdist_g, sd);
    let convd = grid::resample(gd, &convd_g, sd);
    let convv = grid::resample(gd, &convv_g, sd);
    let lift = grid::resample(gd, &lift_g, sd);
    let dmid = grid::resample(gd, &dmid_g, sd);
    let strait = (gap * 0.45).max(6.0);
    let owner_s: Vec<u8> = (0..sd.n())
        .map(|i| {
            let (x, y) = sd.xy(i);
            let gx = ((x as f32 + 0.5) * csf / tect.cell.0) as usize;
            let gy = ((y as f32 + 0.5) * csf / tect.cell.1) as usize;
            tect.owner[gy.min(gd.h - 1) * gd.w + gx.min(gd.w - 1)]
        })
        .collect();

    // Each continent's character (mountainous, arid, wet, broken, highland, plains), drawn at
    // random each run and spread over its territory (smoothly, across the open sea).
    let chars = landform::characters(nc, p.campaign, &mut rng);
    let terr_s: Vec<u8> = (0..sd.n())
        .map(|i| {
            let (x, y) = sd.xy(i);
            let gx = ((x as f32 + 0.5) * csf / tect.cell.0) as usize;
            let gy = ((y as f32 + 0.5) * csf / tect.cell.1) as usize;
            (territory[gy.min(gd.h - 1) * gd.w + gx.min(gd.w - 1)] as usize).min(nc - 1) as u8
        })
        .collect();
    let char_field = |f: &dyn Fn(&landform::Character) -> f32| {
        let mut v: Vec<f32> = terr_s.iter().map(|&t| f(&chars[t as usize])).collect();
        grid::blur(sd, &mut v, 3);
        v
    };
    let ch_mtn = char_field(&|c| c.mountains);
    let ch_arid = char_field(&|c| c.aridity);
    let ch_rough = char_field(&|c| c.roughness);
    let ch_plateau = char_field(&|c| c.plateau);
    let ch_rugged = char_field(&|c| c.rugged);
    let ch_broken = char_field(&|c| c.broken);

    // ---- 2. base relief (simulation grid) ----
    // Low-frequency noise (warp, coast, the first hill octaves) lives on a grid half the size
    // and is upsampled; only the fine octaves and the ridges are sampled per cell.
    let tor = Torus::new(p.w, p.h, sd.w, sd.h);
    let lo = if sd.w >= 160 && sd.w % 2 == 0 && sd.h % 2 == 0 { 2 } else { 1 };
    // Octaves finer than ~2.5 cells of the grid they're sampled on add nothing.
    let octs = |freq: f32, max: u32, cell: f32| (0..max).take_while(|&o| 1.0 / (freq * (1 << o) as f32) >= 2.5 * cell).count().max(1) as u32;
    let warp = Warp::new(&mut rng, 1.0 / (0.9 * scale), octs(1.0 / (0.9 * scale), 3, csf * lo as f32), 0.15 * scale);
    let coast_n = Fbm::new(&mut rng, 1.0 / (0.32 * scale), 5, 0.52);
    let coast_oct = octs(1.0 / (0.32 * scale), 5, csf * lo as f32);
    let hills_n = Fbm::new(&mut rng, 1.0 / (0.13 * scale), 4, 0.5);
    let hills_oct = octs(1.0 / (0.13 * scale), 4, csf).max(2);
    let ridge_n = Fbm::new(&mut rng, 1.0 / (0.16 * scale), octs(1.0 / (0.16 * scale), 4, csf), 0.5);
    let coast_amp = 0.2 * scale * p.roughness.max(0.1);
    // Region masks: rugged against smooth country, tablelands, and the straits of broken
    // continents (all large scale, so on the half-size grid too).
    let rugged_n = Fbm::new(&mut rng, 1.0 / (0.7 * scale), 2, 0.5);
    let plateau_n = Fbm::new(&mut rng, 1.0 / (0.45 * scale), 3, 0.5);
    let strait_n = Fbm::new(&mut rng, 1.0 / (0.55 * scale), 3, 0.5);
    let belt_w = 0.075 * scale;
    let n = sd.n();
    let ld = sd.sized(sd.w / lo, sd.h / lo);
    let tor_l = Torus::new(p.w, p.h, ld.w, ld.h);
    let mut wxl = vec![0.0f32; ld.n()];
    let mut wyl = vec![0.0f32; ld.n()];
    for y in 0..ld.h {
        for x in 0..ld.w {
            let (a, b) = warp.offset(&tor_l.at(x, y));
            wxl[y * ld.w + x] = a;
            wyl[y * ld.w + x] = b;
        }
    }
    let (warp_x, warp_y) = if lo == 1 { (wxl.clone(), wyl.clone()) } else { (grid::resample(ld, &wxl, sd), grid::resample(ld, &wyl, sd)) };
    let mut coast_l = vec![0.0f32; ld.n()];
    let mut hills_l = vec![0.0f32; ld.n()];
    let mut rugged_l = vec![0.0f32; ld.n()];
    let mut plateau_l = vec![0.0f32; ld.n()];
    let mut strait_l = vec![0.0f32; ld.n()];
    for y in 0..ld.h {
        for x in 0..ld.w {
            let i = y * ld.w + x;
            let wp = tor_l.at(x, y).moved(wxl[i], wyl[i]);
            coast_l[i] = coast_n.sample_octaves(&wp, 0, coast_oct);
            hills_l[i] = hills_n.sample_octaves(&wp, 0, 2);
            rugged_l[i] = rugged_n.sample(&wp);
            plateau_l[i] = plateau_n.sample(&wp);
            strait_l[i] = strait_n.sample(&wp);
        }
    }
    let upl = |v: Vec<f32>| if lo == 1 { v } else { grid::resample(ld, &v, sd) };
    let (coast_nf, hills_lo) = (upl(coast_l), upl(hills_l));
    let (rugged_f, plateau_f, strait_f) = (upl(rugged_l), upl(plateau_l), upl(strait_l));
    // Rift valleys along divergent plate boundaries.
    let div_src: Vec<bool> = (0..gd.n()).map(|i| tect.boundary[i] && tect.conv[i] < -0.15).collect();
    let (divd_g, _) = grid::distance_carry(gd, &div_src, &tect.conv);
    let divd = grid::resample(gd, &divd_g.iter().map(|v| v * gcell).collect::<Vec<f32>>(), sd);
    let mut h = vec![0.0f32; n];
    let mut coast = vec![0.0f32; n];
    let mut land_f = vec![0.0f32; n];
    for y in 0..sd.h {
        for x in 0..sd.w {
            let i = y * sd.w + x;
            // The channel between two continents varies in width (0.5×–2.5× `strait`), so its
            // shores wiggle instead of following the straight territory line.
            let channel = strait + 0.12 * scale * (1.0 + strait_f[i] * 1.4).max(0.0);
            let mut c = (sdist[i] + coast_nf[i] * coast_amp * ch_rough[i]).min(dmid[i] - channel * 0.5);
            // Broken continents: inland seas and straits where the strait noise dips.
            if ch_broken[i] > 0.01 && c > 0.0 {
                c -= ch_broken[i] * 0.5 * scale * smoothstep(-0.05, -0.45, strait_f[i]);
            }
            coast[i] = c;
            let lf = smoothstep(-0.02 * scale, 0.06 * scale, c);
            land_f[i] = lf;
            let base = if c >= 0.0 {
                0.03 + 0.15 * smoothstep(0.0, 0.32 * scale, c) + lift[i]
            } else {
                -0.035 - 0.7 * smoothstep(0.0, 0.11 * scale, -c) + lift[i] * 0.5
            };
            let wp = tor.at(x, y).moved(warp_x[i], warp_y[i]);
            let fine = if c > -0.05 * scale && hills_oct > 2 { hills_n.sample_octaves(&wp, 2, hills_oct) } else { 0.0 };
            // Rugged regions are hilly, smooth ones plains (so hills aren't everywhere).
            let rug = (0.2 + 1.6 * smoothstep(-0.35, 0.45, rugged_f[i])) * ch_rugged[i];
            let hills = (hills_lo[i] + fine) * 0.075 * (0.3 + 0.7 * lf) * rug;
            // Tablelands: flat high ground with escarpments, away from the coast.
            let table = ch_plateau[i] * smoothstep(0.12, 0.28, plateau_f[i]) * smoothstep(0.04 * scale, 0.16 * scale, c);
            // Rifts sink along divergent boundaries inland.
            let rift = 0.12 * (-(divd[i] / (0.03 * scale)).powi(2)).exp() * smoothstep(0.05 * scale, 0.2 * scale, c);
            let belt = convv[i].max(0.0) * (-(convd[i] / belt_w).powi(2)).exp();
            let ridges = if belt * lf > 0.01 { belt * 0.22 * ridge_n.ridged(&wp) * lf } else { 0.0 };
            h[i] = base + hills + ridges * ch_mtn[i].min(1.4) + table - rift;
        }
    }
    clock.lap("base");

    // ---- 3. where ranges belong (the detail stage grows them) ----
    let corridor_w = 0.1 * scale;
    let margin = 0.03 * scale;
    let mut corridor: Vec<bool> = (0..n).map(|i| convd[i] < corridor_w && convv[i] > 0.03 && coast[i] > margin).collect();
    // A continent with no convergent boundary across it gets an old, worn range along a chord.
    for c in 0..nc {
        let cells: Vec<usize> = (0..n).filter(|&i| owner_s[i] == c as u8 && coast[i] > margin * 2.0).collect();
        if cells.len() < 12 || cells.iter().any(|&i| corridor[i]) {
            continue;
        }
        let a = cells[rng.below(cells.len())];
        let mut b = a;
        let mut best = 0.0;
        for _ in 0..24 {
            let k = cells[rng.below(cells.len())];
            let (ax, ay) = sd.xy(a);
            let (kx, ky) = sd.xy(k);
            let d2 = sd.dist2(ax as f32, ay as f32, kx as f32, ky as f32);
            if d2 > best {
                best = d2;
                b = k;
            }
        }
        let (ax, ay) = sd.xy(a);
        let (bx, by) = sd.xy(b);
        let (dx, dy) = (sd.dx(ax as f32, bx as f32), sd.dy(ay as f32, by as f32));
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let wcells = (corridor_w / csf * 0.8).max(2.0);
        for k in 0..=(len as usize) {
            let t = k as f32 / len;
            let (cx, cy) = (ax as f32 + dx * t * 0.8 + dx * 0.1, ay as f32 + dy * t * 0.8 + dy * 0.1);
            let r = wcells as i32 + 1;
            for oy in -r..=r {
                for ox in -r..=r {
                    if ((ox * ox + oy * oy) as f32) <= wcells * wcells {
                        if let Some(j) = sd.idx(cx as i32 + ox, cy as i32 + oy) {
                            if coast[j] > margin {
                                corridor[j] = true;
                            }
                        }
                    }
                }
            }
        }
    }
    // Ranges hugging one coast (like the Andes): on some continents, a corridor just inland
    // along the coast that faces a random direction.
    for c in 0..nc {
        let mountainous = chars[c].mountains > 1.2;
        if !(mountainous || rng.chance(0.4)) {
            continue;
        }
        let (gx, gy) = tect.cores[c];
        let (cx, cy) = (gx * tect.cell.0 / csf, gy * tect.cell.1 / csf);
        let th = rng.range(0.0, std::f32::consts::TAU);
        let (ux, uy) = (th.cos(), th.sin());
        let r = (areas[c] / std::f32::consts::PI).sqrt() / csf;
        for i in 0..n {
            if owner_s[i] != c as u8 || coast[i] < 0.025 * scale || coast[i] > 0.1 * scale {
                continue;
            }
            let (x, y) = sd.xy(i);
            let (dx, dy) = (sd.dx(cx, x as f32), sd.dy(cy, y as f32));
            if dx * ux + dy * uy > 0.3 * r {
                corridor[i] = true;
            }
        }
    }
    let mut uplift: Vec<f32> = (0..n).map(|i| if corridor[i] { (0.5 + 0.5 * convv[i].clamp(0.0, 1.0)) * ch_mtn[i] } else { 0.0 }).collect();
    clock.lap("uplift");

    // ---- 4. islands: a few big ones, in arcs on oceanic convergent boundaries or hotspot
    // chains. Each is at least `min_island` (smaller landmasses are removed later).
    let min_island = (p.min_island * n_tiles).max(16.0);
    {
        let total_land: f32 = areas.iter().sum();
        let budget = p.islands * 0.2 * total_land;
        let mut count = (budget / (2.6 * min_island)).round() as usize;
        if p.islands > 0.0 {
            count = count.max(1);
        }
        let far = gap * 0.6;
        let in_seam = |x: f32, y: f32, r: f32| {
            if p.seam_margin <= 0.0 {
                return false;
            }
            let (xt, yt) = (x * csf, y * csf);
            let m = p.seam_margin + r * csf + 2.0;
            xt < m || xt > p.w as f32 - m || yt < m || yt > p.h as f32 - m
        };
        // Offshore, within reach of a continent: chains and arcs off the coasts.
        let ocean_cells: Vec<usize> = (0..n).filter(|&i| coast[i] < -far && coast[i] > -0.45 * scale).collect();
        let arcs: Vec<usize> = ocean_cells.iter().copied().filter(|&i| convd[i] < 1.5 * gcell && convv[i] > 0.15).collect();
        let mut placed: Vec<(f32, f32, f32)> = Vec::new(); // x, y, r (cells)
        let mut chain = 0;
        let mut tries = 0;
        while placed.len() < count && tries < 80 && !ocean_cells.is_empty() {
            tries += 1;
            let arc = chain % 2 == 0 && !arcs.is_empty();
            let pool = if arc { &arcs } else { &ocean_cells };
            let start = pool[rng.below(pool.len())];
            let (sx, sy) = sd.xy(start);
            let ang = rng.range(0.0, std::f32::consts::TAU);
            let len = rng.int(2, 4).min((count - placed.len()) as i32);
            let mut prev: Option<(f32, f32, f32)> = None;
            for k in 0..len {
                let area = min_island * rng.range(2.4, 4.0);
                let r = (area / std::f32::consts::PI).sqrt() / csf;
                let (x, y) = match prev {
                    None => (sx as f32, sy as f32),
                    Some((px, py, pr)) => {
                        let a2 = ang + if arc { 0.35 * k as f32 } else { rng.range(-0.3, 0.3) };
                        let step = pr + r + rng.range(0.25, 0.6) * r;
                        (px + a2.cos() * step, py + a2.sin() * step)
                    }
                };
                let Some(ci) = sd.idx(x.round() as i32, y.round() as i32) else { break };
                let clear = coast[ci] < -(r * csf + far)
                    && !in_seam(x, y, r)
                    && placed.iter().all(|&(qx, qy, qr)| sd.dist2(qx, qy, x, y).sqrt() > qr + r + far / csf);
                if !clear {
                    break;
                }
                placed.push((x, y, r));
                prev = Some((x, y, r));
            }
            chain += 1;
        }
        for &(bx, by, r) in placed.iter() {
            let amp = rng.range(0.12, 0.4);
            let rr = (r * 1.6).ceil() as i32 + 1;
            for oy in -rr..=rr {
                for ox in -rr..=rr {
                    let Some(j) = sd.idx(bx.round() as i32 + ox, by.round() as i32 + oy) else { continue };
                    let ddx = ox as f32 - (bx - bx.round());
                    let ddy = oy as f32 - (by - by.round());
                    let d2 = (ddx * ddx + ddy * ddy) / (r * r);
                    // Ragged, not round: the hill noise bends the outline.
                    let f = 1.0 - d2 + hills_lo[j] * 0.6 + coast_nf[j] * 0.9;
                    if f > -0.4 {
                        let t = (f * 1.4).clamp(-1.0, 1.0);
                        let target = if t > 0.0 { 0.02 + amp * t.powf(0.8) + hills_lo[j] * 0.05 } else { 0.02 * (1.0 + t) - 0.12 * -t };
                        if target > h[j] {
                            h[j] = target;
                            land_f[j] = land_f[j].max(t.max(0.0));
                        }
                    }
                }
            }
        }
    }

    // Islands never grow ranges of their own beyond what their bumps give them.
    for i in 0..n {
        if land_f[i] < 0.999 {
            uplift[i] = 0.0;
        }
    }
    clock.lap("islands");

    // ---- 5. the detail stage (the hook): procedural diffusion by default ----
    let nf = full.n();
    let mut drng = Rng::stream(seed, 0xDE7A);
    let input = DetailInput { coarse: &h, coarse_dims: sd, dims: full, cell: cs, uplift: &uplift, land: &land_f, scale };
    let (mut hf, info) = match custom {
        Some(d) => (d.detail(&input, &mut drng), DiffusionInfo::default()),
        None => ProceduralDiffusion { dla: p.dla.clone(), erosion: p.erosion.clone(), mountains: p.mountains }.run(&input, &mut drng),
    };
    assert_eq!(hf.len(), nf, "ReliefDetail must return one height per tile");
    // The simulation grid sees the detailed relief, averaged back down.
    let h: Vec<f32> = if cs == 1 { hf.clone() } else { box_down(full, &hf, sd, cs) };
    clock.lap("detail");

    // ---- 6. climate ----
    let ocean_s: Vec<bool> = h.iter().map(|&v| v < 0.0).collect();
    let temp_s = climate::temperature(sd, &tor, &h, &p.climate, seed);
    let mut moist_s = climate::moisture(sd, &tor, &h, &ocean_s, &temp_s, &p.climate, csf, seed);
    // Mid-scale wobble so biome borders meander.
    {
        let wob = Fbm::new(&mut rng, 1.0 / (0.08 * scale), octs(1.0 / (0.08 * scale), 3, csf * lo as f32), 0.55);
        let wv = noise::coarse_field(sd, p.w, p.h, lo, |q| wob.sample(q));
        for i in 0..n {
            if !ocean_s[i] {
                moist_s[i] = (moist_s[i] + wv[i] * 0.07 - ch_arid[i]).clamp(0.0, 1.0);
            }
        }
    }
    clock.lap("climate");

    // ---- 7. water ----
    let dr = hydrology::route(sd, &h, 0.0, 1e-5);
    let max_lakes = 2 + 3 * nc;
    let mut hfill = h.clone();
    let lake_s = hydrology::lakes(sd, &mut hfill, &dr.filled, 0.0, ((60.0 / (csf * csf)).ceil() as usize).max(6), 0.035, max_lakes);
    let weights: Vec<f32> = (0..n).map(|i| (0.25 + moist_s[i]) * csf * csf).collect();
    let acc = hydrology::accumulate(&dr, &weights);
    let river_area = if p.river_area > 0.0 { p.river_area } else { (0.045 * mean_area).clamp(150.0, 2500.0) };
    let river_s: Vec<bool> = (0..n).map(|i| !ocean_s[i] && !lake_s[i] && acc[i] >= river_area).collect();
    clock.lap("water");

    // ---- 8. full resolution: sea, lakes, rivers ----
    let up = |v: &[f32]| if cs == 1 { v.to_vec() } else { grid::resample(sd, v, full) };
    let coarse_of = |i: usize| -> usize {
        let (x, y) = full.xy(i);
        (y as usize / cs).min(sd.h - 1) * sd.w + (x as usize / cs).min(sd.w - 1)
    };
    let mut lake_f: Vec<bool> = if cs == 1 {
        lake_s.clone()
    } else {
        let lf: Vec<f32> = lake_s.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
        up(&lf).iter().map(|&v| v > 0.5).collect()
    };
    // Sea: below sea level and connected to the coarse sea; stray pockets become land.
    let mut ocean_f: Vec<bool> = (0..nf).map(|i| hf[i] < 0.0 && !lake_f[i]).collect();
    if cs > 1 {
        let (lab, sizes) = grid::components(full, &ocean_f);
        let mut real = vec![false; sizes.len()];
        for i in 0..nf {
            if lab[i] != 0 && ocean_s[coarse_of(i)] {
                real[lab[i] as usize - 1] = true;
            }
        }
        for i in 0..nf {
            if lab[i] != 0 && !real[lab[i] as usize - 1] {
                ocean_f[i] = false;
                hf[i] = 0.004;
            }
        }
    }
    for i in 0..nf {
        if !ocean_f[i] && hf[i] < 0.0 && !lake_f[i] {
            hf[i] = 0.004;
        }
    }
    clock.lap("sea");
    // Rivers: 4-connected 1-px lines from each river cell to its receiver.
    let mut river_f = vec![false; nf];
    {
        let half = cs as i32 / 2;
        for i in 0..n {
            if !river_s[i] {
                continue;
            }
            let r = dr.recv[i];
            if r == hydrology::NONE {
                continue;
            }
            let (x0, y0) = sd.xy(i);
            let (x1, y1) = sd.xy(r as usize);
            let (dx, dy) = (sd.dx(x0 as f32, x1 as f32) as i32, sd.dy(y0 as f32, y1 as f32) as i32);
            let (ax, ay) = (x0 * cs as i32 + half, y0 * cs as i32 + half);
            let (bx, by) = (ax + dx * cs as i32, ay + dy * cs as i32);
            let (ha, hb) = (hfill[i], hfill[r as usize].max(0.0));
            line4(ax, ay, bx, by, |x, y, t| {
                if let Some(j) = full.idx(x, y) {
                    if !ocean_f[j] && !lake_f[j] {
                        river_f[j] = true;
                        if cs > 1 {
                            let lvl = (ha + (hb - ha) * t).max(0.003);
                            hf[j] = hf[j].min(lvl);
                        }
                    }
                }
            });
        }
    }
    clock.lap("rivers");
    // Clean coasts (Alex, #109): smooth at the pixel scale, no land threads under 3 tiles, no
    // diagonal-only joins, no landmass under `min_island`, no tiny enclosed water.
    let mut land_m: Vec<bool> = (0..nf).map(|i| !ocean_f[i] && !lake_f[i]).collect();
    let coast_stats = filter::clean(full, &mut land_m, &hf, min_island as usize, 20);
    {
        let water: Vec<bool> = land_m.iter().map(|&l| !l).collect();
        let (lab, sizes) = grid::components(full, &water);
        let mut sea = vec![false; sizes.len()];
        for i in 0..nf {
            if lab[i] != 0 && ocean_f[i] {
                sea[lab[i] as usize - 1] = true;
            }
        }
        for i in 0..nf {
            if land_m[i] {
                if ocean_f[i] || lake_f[i] {
                    hf[i] = hf[i].max(0.004);
                }
                ocean_f[i] = false;
                lake_f[i] = false;
            } else if sea[lab[i] as usize - 1] {
                if !ocean_f[i] {
                    hf[i] = hf[i].min(-0.004);
                }
                ocean_f[i] = true;
                lake_f[i] = false;
                river_f[i] = false;
            } else {
                lake_f[i] = true;
                ocean_f[i] = false;
                river_f[i] = false;
            }
        }
    }
    clock.lap("coast");
    let temp_f = up(&temp_s);
    let moist_f = up(&moist_s);
    clock.lap("fields");

    // ---- 9. biomes, tiles, colour ----
    // The sea-ice edge meanders (floes and bays) rather than following a temperature line.
    let ice_edge = {
        let f = Fbm::new(&mut rng, 1.0 / (0.06 * scale.max(40.0)), 3, 0.55);
        noise::coarse_field(full, p.w, p.h, (cs * 2).max(2), |q| f.sample(q) * 7.0)
    };
    let bands = &p.bands;
    let mut class = vec![0u8; nf];
    for i in 0..nf {
        let v = hf[i];
        let (b, r) = if ocean_f[i] {
            let b = if temp_f[i] + ice_edge[i] < -13.0 {
                biome::SEA_ICE
            } else if v > -bands.shelf {
                biome::SHELF
            } else {
                biome::OCEAN
            };
            (b, relief::LOW)
        } else if lake_f[i] {
            (biome::LAKE, relief::LOW)
        } else {
            let t = temp_f[i];
            let mut b = climate::whittaker(t, moist_f[i]);
            let r = if v >= bands.peak || (v >= bands.mountain && t < -6.0) {
                relief::PEAK
            } else if v >= bands.mountain {
                relief::MOUNTAIN
            } else if v >= bands.hill {
                relief::HILL
            } else {
                relief::LOW
            };
            if r == relief::LOW && v < bands.beach && t > 1.0 && b != biome::ICE && near(full, &ocean_f, i, 2) {
                b = biome::BEACH;
            }
            (b, r)
        };
        class[i] = palette::class_of(b, r);
    }
    // The majority filter never turns sea, lake and land into one another (the coast is final).
    let kind = |c: u8| {
        let b = palette::class_biome(c);
        if b == biome::LAKE {
            1
        } else if biome::is_land(b) {
            2
        } else {
            0
        }
    };
    let class0 = class.clone();
    filter::despeckle(full, &mut class, |i, v| kind(v) == kind(class0[i]), 24);
    let shade = palette::hillshade(full, &hf, 3.0 * cs.max(1) as f32);
    let mut paint_idx: Vec<u16> = (0..nf)
        .map(|i| {
            let c = class[i];
            let (b, r) = (palette::class_biome(c), palette::class_relief(c));
            let s = if biome::is_land(b) && b != biome::ICE { shade[i] } else { 1 };
            palette::paint_index(c, palette::step_of(b, r, hf[i], bands), s)
        })
        .collect();
    filter::despeckle(full, &mut paint_idx, |i, v| (v / 12) as u8 == class[i], 24);
    let biome_map: Vec<u8> = class.iter().map(|&c| palette::class_biome(c)).collect();
    let relief_map: Vec<u8> = class.iter().map(|&c| palette::class_relief(c)).collect();
    let tiles: Vec<u8> = class.iter().map(|&c| climate::tile_of(palette::class_biome(c), palette::class_relief(c))).collect();
    let rgba = palette::colour(full, &paint_idx, &biome_map, &river_f, &p.palette);
    clock.lap("paint");

    // ---- 10. continents and sites ----
    let major_min = (0.35 * areas.iter().cloned().fold(f32::INFINITY, f32::min)) as usize;
    let (cont_id, continents) = sites::continents(full, &biome_map, major_min);
    let fresh_s: Vec<bool> = (0..n).map(|i| river_s[i] || lake_s[i]).collect();
    let fresh_d: Vec<f32> = grid::distance(sd, &fresh_s).iter().map(|v| v * csf).collect();
    let sea_d: Vec<f32> = grid::distance(sd, &ocean_s).iter().map(|v| v * csf).collect();
    let (region, region_size) = sites::regions(full, &tiles);
    let site_list = sites::rank(
        &sites::SiteInput { d: full, height: &hf, biome: &biome_map, tiles: &tiles, continent: &cont_id, fresh_dist: &fresh_d, sea_dist: &sea_d, cell: cs, sim_w: sd.w, max_sites: p.max_sites },
        &region,
        &region_size,
    );
    clock.lap("sites");

    let (campaign_sites, game_continents) = if p.campaign {
        let (cs_, gc) = sites::campaign(&sites::CampaignInput { w: p.w, h: p.h, heights: &hf, types: &tiles, biome: &biome_map, continent: &cont_id, continents: &continents, sites: &site_list });
        (Some(cs_), gc)
    } else {
        (None, Vec::new())
    };
    // Output heights: sea level at `sea_level`, peaks at 1, sea floor at 0.
    let sl = p.sea_level;
    let height: Vec<f32> = hf.iter().map(|&v| if v >= 0.0 { sl + (1.0 - sl) * v.min(1.0) } else { sl * (1.0 + v.max(-1.0)) }).collect();
    let majors: Vec<(usize, usize, f32, f32)> = continents.iter().filter(|c| c.major).map(|c| (c.id, c.area, c.cx, c.cy)).collect();
    let mut layout = landform::layout(full, &cont_id, &majors);
    let land_mask: Vec<bool> = biome_map.iter().map(|&b| biome::is_land(b)).collect();
    (layout.straight_strips, layout.max_strip_linearity) = metrics::strips(full, &land_mask);
    let stats = Stats { stages: clock.stages, sim_cell: cs, dla_nodes: info.dla_nodes, coast: coast_stats, min_island: min_island as usize, layout, characters: chars.iter().map(|c| c.name).collect() };
    TerrainMap {
        seed,
        width: p.w,
        height: p.h,
        wrap_x: p.wrap_x,
        wrap_y: p.wrap_y,
        sea_level: sl,
        heights: height,
        biome: biome_map,
        relief: relief_map,
        types: tiles,
        river_mask: river_f,
        lake_mask: lake_f,
        temperature: temp_f,
        moisture: moist_f,
        rgba,
        continent: cont_id,
        continents,
        sites: site_list,
        campaign_sites,
        game_continents,
        ranges: info.ranges.clone(),
        sim: SimInfo { w: sd.w, h: sd.h, cell: cs, height: h, recv: dr.recv, river: river_s, lake: lake_s, ocean: ocean_s, dla: info.dla, corridor: info.corridor },
        stats,
    }
}

/// Average `cs`×`cs` blocks of the full grid down to the simulation grid.
fn box_down(full: Dims, v: &[f32], sd: Dims, cs: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; sd.n()];
    for y in 0..full.h {
        let row = (y / cs) * sd.w;
        for x in 0..full.w {
            out[row + x / cs] += v[y * full.w + x];
        }
    }
    let k = 1.0 / (cs * cs) as f32;
    for o in out.iter_mut() {
        *o *= k;
    }
    out
}

/// Any `mask` tile within `r` (Chebyshev) of `i`?
fn near(d: Dims, mask: &[bool], i: usize, r: i32) -> bool {
    let (x, y) = d.xy(i);
    for oy in -r..=r {
        for ox in -r..=r {
            if let Some(j) = d.idx(x + ox, y + oy) {
                if mask[j] {
                    return true;
                }
            }
        }
    }
    false
}

/// A 4-connected line (no diagonal-only steps, so a river reads as one stroke), calling
/// `f(x, y, t)` with t from 0 to 1 along it.
fn line4(x0: i32, y0: i32, x1: i32, y1: i32, mut f: impl FnMut(i32, i32, f32)) {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (sx, sy) = (dx.signum(), dy.signum());
    let steps = dx.abs() + dy.abs();
    let (mut x, mut y) = (x0, y0);
    f(x, y, 0.0);
    for k in 1..=steps {
        if x != x1 && y != y1 {
            // Step along whichever axis stays closer to the true line.
            let ex = ((x + sx - x0) * dy - (y - y0) * dx).abs();
            let ey = ((x - x0) * dy - (y + sy - y0) * dx).abs();
            if ex <= ey {
                x += sx;
            } else {
                y += sy;
            }
        } else if x != x1 {
            x += sx;
        } else {
            y += sy;
        }
        f(x, y, k as f32 / steps as f32);
    }
}
