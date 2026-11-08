// Multi-continent planetary terrain with biome palettes, era repainting,
// and cached BFS distance fields for land walkers and boats.
//
// Port of `src/world/terrain.js` into pure Rust for the engine crate. The land itself (heights,
// tiles, the base colour layer, continents, harbours and campaign sites) comes from
// `moc-terrain` (#109); this module adds the eras' land use, structures and distance fields.

use crate::sprites::*;
use std::collections::HashMap;

pub const W: usize = 320;
pub const H: usize = 180;

pub const T_DEEP: u8 = 0;
pub const T_SHALLOW: u8 = 1;
pub const T_SAND: u8 = 2;
pub const T_GRASS: u8 = 3;
pub const T_FOREST: u8 = 4;
#[allow(dead_code)]
pub const T_HILL: u8 = 5;
pub const T_MOUNTAIN: u8 = 6;
pub const T_SNOW: u8 = 7;
pub const T_FARM: u8 = 8;
pub const T_ROAD: u8 = 9;
pub const T_PAVED: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeType {
    Temperate,
    Desert,
    Ice,
    Jungle,
    Alien,
}

#[derive(Clone, Debug)]
pub struct BiomePalette {
    pub deep: [(u8, u8, u8); 2],
    pub shallow: [(u8, u8, u8); 2],
    pub sand: [(u8, u8, u8); 2],
    pub grass: Vec<(u8, u8, u8)>,
    pub forest: Vec<(u8, u8, u8)>,
    pub hill: [(u8, u8, u8); 2],
    pub mountain: Vec<(u8, u8, u8)>,
    pub snow: [(u8, u8, u8); 2],
    pub farm: [(u8, u8, u8); 2],
    pub road: [(u8, u8, u8); 2],
    pub paved: [(u8, u8, u8); 2],
    pub trees: [(u8, u8, u8); 3],
    pub foam: (u8, u8, u8),
}

impl BiomePalette {
    pub fn for_biome(biome: BiomeType) -> Self {
        match biome {
            BiomeType::Temperate => Self::temperate(),
            BiomeType::Desert => Self::desert(),
            BiomeType::Ice => Self::ice(),
            BiomeType::Jungle => Self::jungle(),
            BiomeType::Alien => Self::alien(),
        }
    }

    pub fn temperate() -> Self {
        Self {
            deep: [(22, 44, 92), (25, 49, 99)],
            shallow: [(40, 86, 146), (45, 94, 155)],
            sand: [(214, 196, 136), (205, 186, 126)],
            grass: vec![(96, 156, 62), (88, 147, 57), (104, 163, 68)],
            forest: vec![(46, 98, 46), (58, 116, 52), (38, 84, 40)],
            hill: [(124, 132, 74), (114, 122, 68)],
            mountain: vec![(118, 110, 100), (104, 97, 89), (132, 124, 114)],
            snow: [(232, 232, 228), (216, 218, 222)],
            farm: [(196, 170, 74), (120, 158, 60)],
            road: [(150, 128, 98), (142, 121, 92)],
            paved: [(146, 146, 152), (136, 136, 144)],
            trees: [(0x2c, 0x5a, 0x2a), (0x6a, 0xa8, 0x4f), (0x4f, 0x8d, 0x3e)],
            foam: (120, 170, 205),
        }
    }

    pub fn desert() -> Self {
        let mut p = Self::temperate();
        p.deep = [(24, 70, 96), (27, 76, 102)];
        p.shallow = [(44, 122, 138), (50, 130, 144)];
        p.sand = [(232, 206, 140), (224, 198, 132)];
        p.grass = vec![(210, 172, 102), (200, 162, 94), (218, 180, 110)];
        p.forest = vec![(120, 150, 70), (104, 136, 60), (136, 160, 80)];
        p.hill = [(184, 120, 72), (172, 110, 66)];
        p.mountain = vec![(140, 78, 52), (126, 70, 46), (152, 88, 60)];
        p.snow = [(240, 222, 190), (230, 212, 180)];
        p.trees = [(0x5e, 0x6f, 0x2c), (0x9f, 0xbf, 0x4f), (0x7d, 0x9a, 0x3a)];
        p.foam = (140, 200, 200);
        p
    }

    pub fn ice() -> Self {
        let mut p = Self::temperate();
        p.deep = [(16, 30, 64), (18, 34, 70)];
        p.shallow = [(52, 90, 130), (58, 98, 140)];
        p.sand = [(176, 186, 196), (168, 178, 188)];
        p.grass = vec![(214, 228, 236), (204, 220, 230), (222, 234, 240)];
        p.forest = vec![(40, 84, 86), (48, 96, 98), (34, 74, 78)];
        p.hill = [(170, 190, 206), (160, 180, 198)];
        p.mountain = vec![(110, 124, 146), (98, 112, 134), (122, 136, 158)];
        p.snow = [(248, 250, 255), (236, 242, 250)];
        p.trees = [(0x1f, 0x4a, 0x4c), (0x5f, 0x9a, 0x96), (0x3c, 0x74, 0x72)];
        p.foam = (190, 215, 235);
        p
    }

    pub fn jungle() -> Self {
        let mut p = Self::temperate();
        p.deep = [(16, 70, 90), (18, 76, 96)];
        p.shallow = [(34, 140, 150), (40, 150, 158)];
        p.grass = vec![(52, 140, 60), (46, 130, 54), (58, 150, 66)];
        p.forest = vec![(24, 84, 36), (30, 96, 42), (20, 72, 30)];
        p.hill = [(78, 124, 58), (70, 114, 52)];
        p.trees = [(0x17, 0x4d, 0x22), (0x4f, 0xa6, 0x4a), (0x2f, 0x7f, 0x36)];
        p.foam = (130, 210, 210);
        p
    }

    pub fn alien() -> Self {
        let mut p = Self::temperate();
        p.deep = [(18, 60, 56), (20, 66, 60)];
        p.shallow = [(36, 120, 104), (42, 130, 112)];
        p.sand = [(210, 190, 226), (200, 180, 218)];
        p.grass = vec![(140, 92, 176), (128, 84, 164), (152, 102, 188)];
        p.forest = vec![(90, 44, 120), (104, 52, 136), (78, 38, 106)];
        p.hill = [(110, 120, 170), (100, 110, 160)];
        p.mountain = vec![(70, 58, 96), (62, 52, 88), (80, 66, 108)];
        p.snow = [(214, 255, 236), (200, 246, 226)];
        p.farm = [(230, 140, 210), (150, 100, 190)];
        p.trees = [(0x3e, 0x1d, 0x5c), (0xd0, 0x7a, 0xd8), (0x9a, 0x52, 0xb8)];
        p.foam = (150, 230, 200);
        p
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Default, Debug)]
#[allow(dead_code)]
pub struct HarborSite {
    pub land: Point,
    pub dock: Point,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Continent {
    pub id: String,
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
    pub village: Point,
    pub harbor: HarborSite,
    pub home_harbor: HarborSite,
}

#[derive(Clone, Copy, Default, Debug)]
#[allow(dead_code)] // mirrors the JS `World.sites`
pub struct Sites {
    pub village: Point,
    pub pad: Point,
    pub ridge: Point,
    pub cave: Point,
    pub cave_mouth: Point,
    pub fire: Point,
    pub factory: Point,
    pub exchange: Point,
    pub picket: Point,
    pub hangar: Point,
    pub crowd: Point,
}

#[derive(Clone, Copy, Debug)]
pub struct TreeInfo {
    pub x: i32,
    pub y: i32,
    pub variant: u32,
}

#[derive(Clone, Debug)]
pub struct StructureInfo {
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub h: i32,
    pub roof: (u8, u8, u8),
}

pub struct Mulberry32 {
    pub state: u32,
}

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }
    pub fn next(&mut self) -> f32 {
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
    pub biome: BiomeType,
    pub palette: BiomePalette,
    pub h: Vec<f32>,
    pub base_type: Vec<u8>,
    pub tile_type: Vec<u8>,
    pub owner: Vec<u8>,
    pub continents: Vec<Continent>,
    pub sites: Sites,
    pub trees: Vec<TreeInfo>,
    pub structures: Vec<StructureInfo>,
    /// The base colour layer from moc-terrain (coherent fields, no per-tile random colour).
    pub base_pixels: Vec<u8>,
    pub pixels: Vec<u8>,       // 320x180x4 RGBA base landscape
    pub pixels_2d: Vec<u8>,    // 320x180x4 RGBA 2D fallback with flat buildings & tree crowns
    pub cached_fields: HashMap<i32, Vec<i16>>,
}

impl Terrain {
    pub fn new(seed: u32) -> Self {
        Self::with_biome(seed, BiomeType::Temperate)
    }

    /// A terrain with nothing generated yet (fill it with `apply_map`).
    pub fn new_empty(seed: u32) -> Self {
        Self::empty(seed, BiomeType::Temperate)
    }

    pub fn with_biome(seed: u32, biome: BiomeType) -> Self {
        let mut t = Self::empty(seed, biome);
        t.generate();
        t.bake_era(0); // Stone Age default
        t
    }

    fn empty(seed: u32, biome: BiomeType) -> Self {
        let palette = BiomePalette::for_biome(biome);
        Self {
            seed,
            biome,
            palette,
            h: vec![0.0f32; W * H],
            base_type: vec![0u8; W * H],
            tile_type: vec![0u8; W * H],
            owner: vec![0u8; W * H],
            continents: Vec::new(),
            sites: Sites::default(),
            trees: Vec::new(),
            structures: Vec::new(),
            base_pixels: vec![0u8; W * H * 4],
            pixels: vec![0u8; W * H * 4],
            pixels_2d: vec![0u8; W * H * 4],
            cached_fields: HashMap::new(),
        }
    }

    pub fn generate(&mut self) {
        let biome = match self.biome {
            BiomeType::Desert => "desert",
            BiomeType::Ice => "ice",
            BiomeType::Jungle => "jungle",
            BiomeType::Alien => "alien",
            BiomeType::Temperate => "temperate",
        };
        let mut p = moc_terrain::TerrainParams::home_world_sized(self.seed, W, H);
        p.set_biome(biome);
        self.apply_map(&moc_terrain::generate(&p));
    }

    /// Take a generated home world (it must be W×H) as this terrain.
    pub fn apply_map(&mut self, map: &moc_terrain::TerrainMap) {
        assert_eq!((map.width, map.height), (W, H), "the engine's world is {W}x{H}");
        self.h.copy_from_slice(&map.heights);
        self.base_type.copy_from_slice(&map.types);
        self.tile_type.copy_from_slice(&map.types);
        self.base_pixels.copy_from_slice(&map.rgba);
        self.owner = map.owner().iter().map(|&o| if o == 255 { 0 } else { o }).collect();
        let pt = |p: (usize, usize)| Point { x: p.0 as i32, y: p.1 as i32 };
        let pi = |p: (i32, i32)| Point { x: p.0, y: p.1 };
        if let Some(s) = &map.campaign_sites {
            self.sites = Sites {
                village: pt(s.village),
                pad: pt(s.pad),
                ridge: pt(s.ridge),
                cave: pt(s.cave),
                cave_mouth: pt(s.cave_mouth),
                fire: pt(s.fire),
                factory: pt(s.factory),
                exchange: pt(s.exchange),
                picket: pt(s.picket),
                hangar: pt(s.hangar),
                crowd: pt(s.crowd),
            };
        }
        let hb = |h: &Option<((i32, i32), (i32, i32))>| h.map(|(l, d)| HarborSite { land: pi(l), dock: pi(d) }).unwrap_or_default();
        self.continents = map
            .game_continents
            .iter()
            .map(|c| Continent { id: c.id.clone(), cx: c.cx, cy: c.cy, rx: c.rx, ry: c.ry, village: pi(c.village), harbor: hb(&c.harbor), home_harbor: hb(&c.home_harbor) })
            .collect();
        self.cached_fields.clear();
    }

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

    #[inline]
    pub fn nearest_walkable(&self, x: i32, y: i32, max_r: i32) -> Point {
        for r in 0..=max_r {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    if self.walkable(x + dx, y + dy) && self.get(x + dx, y + dy) != T_FOREST {
                        return Point { x: x + dx, y: y + dy };
                    }
                }
            }
        }
        Point { x, y }
    }

    /// BFS distance field toward target on 3px quantized grid, with LRU cache.
    pub fn field(&mut self, tx: i32, ty: i32) -> &[i16] {
        let qx = (tx as f32 / 3.0).round() as i32 * 3;
        let qy = (ty as f32 / 3.0).round() as i32 * 3;
        let qx = qx.clamp(0, W as i32 - 1);
        let qy = qy.clamp(0, H as i32 - 1);
        let key = qy * (W as i32) + qx;

        if !self.cached_fields.contains_key(&key) {
            let mut f = vec![-1i16; W * H];
            let start = if self.walkable(qx, qy) {
                Point { x: qx, y: qy }
            } else {
                self.nearest_walkable(qx, qy, 12)
            };

            let mut queue = vec![0usize; W * H];
            let mut head = 0;
            let mut tail = 0;

            let s0 = (start.y as usize) * W + (start.x as usize);
            if s0 < f.len() {
                f[s0] = 0;
                queue[tail] = s0;
                tail += 1;
            }

            while head < tail {
                let curr = queue[head];
                head += 1;
                let cx = (curr % W) as i32;
                let cy = (curr / W) as i32;
                let d = f[curr] + 1;

                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
                            continue;
                        }
                        let n_idx = (ny as usize) * W + (nx as usize);
                        if f[n_idx] != -1 {
                            continue;
                        }
                        let t = self.tile_type[n_idx];
                        if t < T_SAND || t == T_MOUNTAIN || t == T_SNOW {
                            continue;
                        }
                        f[n_idx] = d;
                        queue[tail] = n_idx;
                        tail += 1;
                    }
                }
            }

            if self.cached_fields.len() > 64 {
                if let Some(old_key) = self.cached_fields.keys().next().copied() {
                    self.cached_fields.remove(&old_key);
                }
            }
            self.cached_fields.insert(key, f);
        }

        self.cached_fields.get(&key).unwrap()
    }

    /// Maritime BFS distance field over deep and shallow water.
    pub fn sea_field(&mut self, tx: i32, ty: i32) -> &[i16] {
        let qx = tx.clamp(0, W as i32 - 1);
        let qy = ty.clamp(0, H as i32 - 1);
        let key = -(qy * (W as i32) + qx) - 1;

        if !self.cached_fields.contains_key(&key) {
            let mut f = vec![-1i16; W * H];
            let mut queue = vec![0usize; W * H];
            let mut head = 0;
            let mut tail = 0;

            let s0 = (qy as usize) * W + (qx as usize);
            f[s0] = 0;
            queue[tail] = s0;
            tail += 1;

            while head < tail {
                let curr = queue[head];
                head += 1;
                let cx = (curr % W) as i32;
                let cy = (curr / W) as i32;
                let d = f[curr] + 1;

                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
                            continue;
                        }
                        let n_idx = (ny as usize) * W + (nx as usize);
                        if f[n_idx] != -1 || self.tile_type[n_idx] > T_SHALLOW {
                            continue;
                        }
                        f[n_idx] = d;
                        queue[tail] = n_idx;
                        tail += 1;
                    }
                }
            }

            if self.cached_fields.len() > 64 {
                if let Some(old_key) = self.cached_fields.keys().next().copied() {
                    self.cached_fields.remove(&old_key);
                }
            }
            self.cached_fields.insert(key, f);
        }

        self.cached_fields.get(&key).unwrap()
    }

    pub fn reachable(&mut self, x: i32, y: i32) -> bool {
        let v = self.sites.village;
        let field = self.field(v.x, v.y);
        let idx = (y.clamp(0, H as i32 - 1) as usize) * W + (x.clamp(0, W as i32 - 1) as usize);
        field[idx] >= 0
    }

    /// Shape land use and structures for a specific era (0: Stone, 1: Farming, 2: Industry, 3: Space).
    pub fn shape(&mut self, era: u8) {
        self.tile_type = self.base_type.clone();
        self.cached_fields.clear();
        self.structures.clear();

        let v = self.sites.village;
        let s = self.sites;
        let mut rng = Mulberry32::new(self.seed + 17);

        // Stone Age
        if era == 0 {
            for i in 0..4 {
                let a = (i as f32 / 4.0) * std::f32::consts::PI * 2.0 + 0.5;
                let hx = v.x + (a.cos() * 11.0).round() as i32;
                let hy = v.y + (a.sin() * 7.0).round() as i32;
                self.structures.push(StructureInfo {
                    kind: "hut".to_string(),
                    x: hx,
                    y: hy,
                    h: 0,
                    roof: (0, 0, 0),
                });
            }
        }

        // Farming Age
        if era == 1 {
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
            for i in 0..10 {
                let a = (i as f32 / 10.0) * std::f32::consts::PI * 2.0;
                let hx = v.x + (a.cos() * (12.0 + ((i % 2) * 6) as f32)).round() as i32;
                let hy = v.y + (a.sin() * (8.0 + ((i % 2) * 4) as f32)).round() as i32;
                self.structures.push(StructureInfo {
                    kind: "hut".to_string(),
                    x: hx,
                    y: hy,
                    h: 0,
                    roof: (0, 0, 0),
                });
            }
        }

        // Settlements spread to other continents from farming age on
        if era >= 1 {
            for c in self.continents.iter().skip(1) {
                let n = if era == 1 { 5 } else { 8 };
                for i in 0..n {
                    let a = (i as f32 / n as f32) * std::f32::consts::PI * 2.0;
                    let hx = (c.village.x as f32 + a.cos() * 7.0).round() as i32;
                    let hy = (c.village.y as f32 + a.sin() * 5.0).round() as i32;
                    if !self.walkable(hx, hy) {
                        continue;
                    }
                    if era == 1 {
                        self.structures.push(StructureInfo {
                            kind: "hut".to_string(),
                            x: hx,
                            y: hy,
                            h: 0,
                            roof: (0, 0, 0),
                        });
                    } else if era == 3 && i % 3 == 0 {
                        self.structures.push(StructureInfo {
                            kind: "tower".to_string(),
                            x: hx,
                            y: hy,
                            h: 7 + (i % 4) as i32,
                            roof: (0, 0, 0),
                        });
                    } else {
                        let roof = if era == 3 { (0x5a, 0x6b, 0x8c) } else { (0x8e, 0x3b, 0x2a) };
                        self.structures.push(StructureInfo {
                            kind: "house".to_string(),
                            x: hx,
                            y: hy,
                            h: 0,
                            roof,
                        });
                    }
                }
            }
        }

        // Industry or Space Age
        if era >= 2 {
            let farm_count = if era == 2 { 22 } else { 12 };
            for _ in 0..farm_count {
                let a = rng.next() * std::f32::consts::PI * 2.0;
                let r = 34.0 + rng.next() * 56.0;
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

            self.carve_road(v, s.factory, 2);
            self.carve_road(v, s.exchange, 1);
            self.carve_road(v, s.pad, 1);

            for fy in -2..=2 {
                for fx in -3..=3 {
                    self.set_tile(v.x + fx, v.y + fy, T_PAVED);
                }
            }

            let roofs = if era == 2 {
                [(0xa4, 0x40, 0x2f), (0x8e, 0x3b, 0x2a), (0xb5, 0x54, 0x3a), (0x6f, 0x4a, 0x3a)]
            } else {
                [(0x5a, 0x6b, 0x8c), (0x7f, 0x8f, 0xa8), (0xa4, 0x40, 0x2f), (0x4d, 0x5d, 0x7a)]
            };

            for ring in 0..3 {
                let n = 10 + ring * 6;
                for i in 0..n {
                    let a = (i as f32 / n as f32) * std::f32::consts::PI * 2.0 + ring as f32 * 0.3;
                    let hx = (v.x as f32 + a.cos() * (12.0 + ring as f32 * 9.0)).round() as i32;
                    let hy = (v.y as f32 + a.sin() * (8.0 + ring as f32 * 6.0)).round() as i32;
                    let dist_ex = ((hx - s.exchange.x).pow(2) + (hy - s.exchange.y).pow(2)) as f32;
                    let dist_fa = ((hx - s.factory.x).pow(2) + (hy - s.factory.y).pow(2)) as f32;
                    if !self.walkable(hx, hy) || dist_ex < 81.0 || dist_fa < 144.0 {
                        continue;
                    }
                    if era == 3 && ring == 0 && i % 2 == 0 {
                        let h = 8 + (rng.next() * 7.0) as i32;
                        self.structures.push(StructureInfo {
                            kind: "tower".to_string(),
                            x: hx,
                            y: hy,
                            h,
                            roof: (0, 0, 0),
                        });
                    } else {
                        let roof = roofs[(i + ring) % roofs.len()];
                        self.structures.push(StructureInfo {
                            kind: "house".to_string(),
                            x: hx,
                            y: hy,
                            h: 0,
                            roof,
                        });
                    }
                }
            }

            self.structures.push(StructureInfo {
                kind: "factory".to_string(),
                x: s.factory.x,
                y: s.factory.y,
                h: 0,
                roof: (0, 0, 0),
            });
            self.structures.push(StructureInfo {
                kind: "exchange".to_string(),
                x: s.exchange.x,
                y: s.exchange.y,
                h: 0,
                roof: (0, 0, 0),
            });
        }

        // Space Age Launchpad
        if era == 3 {
            for fy in -6..=6 {
                for fx in -8..=8 {
                    if self.walkable(s.pad.x + fx, s.pad.y + fy) {
                        self.set_tile(s.pad.x + fx, s.pad.y + fy, T_PAVED);
                    }
                }
            }
            self.structures.push(StructureInfo {
                kind: "hangar".to_string(),
                x: s.hangar.x,
                y: s.hangar.y,
                h: 0,
                roof: (0, 0, 0),
            });
            self.structures.push(StructureInfo {
                kind: "pad".to_string(),
                x: s.pad.x,
                y: s.pad.y,
                h: 0,
                roof: (0, 0, 0),
            });
        }

        self.structures.sort_by_key(|s| s.y);
    }

    fn carve_road(&mut self, from: Point, to: Point, width: i32) {
        let dx = (to.x - from.x) as f32;
        let dy = (to.y - from.y) as f32;
        let n = (dx * dx + dy * dy).sqrt().ceil() as i32;
        for i in 0..=n {
            let t = if n > 0 { i as f32 / n as f32 } else { 0.0 };
            let x = (from.x as f32 + dx * t).round() as i32;
            let y = (from.y as f32 + dy * t).round() as i32;
            for d in 0..width {
                if self.walkable(x, y + d) {
                    self.set_tile(x, y + d, T_ROAD);
                }
            }
        }
    }

    /// Bake terrain textures for both 3D clean landscape and 2D fallback mode.
    pub fn bake_era(&mut self, era: u8) {
        self.shape(era);

        // 1. Collect trees on forest tiles
        self.trees.clear();
        let mut trng = Mulberry32::new(self.seed + 5);
        for y in (1..H).step_by(2) {
            let start_x = ((y >> 1) % 2) as usize;
            for x in (start_x..W).step_by(2) {
                let idx = y * W + x;
                if self.tile_type[idx] == T_FOREST && trng.next() <= 0.7 {
                    let variant = if trng.next() > 0.5 { 1 } else { 0 };
                    self.trees.push(TreeInfo {
                        x: x as i32,
                        y: y as i32,
                        variant,
                    });
                }
            }
        }

        // 2. Base landscape texture for 3D WebGL upload (no baked flat buildings): the base
        // colour layer, with the era's land use (farms, roads, paving) painted over it.
        self.pixels = paint_land_use(&self.base_pixels, &self.tile_type, &self.base_type, W, &self.palette);

        // Cave mouth on 3D landscape
        if era <= 1 {
            let m = self.sites.cave_mouth;
            fill_rect(&mut self.pixels, m.x - 2, m.y - 1, 4, 3, 0x1a, 0x17, 0x14, 255);
            fill_rect(&mut self.pixels, m.x - 3, m.y, 1, 2, 0x2b, 0x26, 0x22, 255);
            fill_rect(&mut self.pixels, m.x + 2, m.y, 1, 2, 0x2b, 0x26, 0x22, 255);
        }

        // 3. 2D fallback texture with baked flat tree crowns and structures
        self.pixels_2d.copy_from_slice(&self.pixels);

        for t in &self.trees {
            let tc = self.palette.trees;
            put_pixel(&mut self.pixels_2d, t.x, t.y, tc[0].0, tc[0].1, tc[0].2, 255);
            let col = if t.variant == 1 { tc[1] } else { tc[2] };
            put_pixel(&mut self.pixels_2d, t.x, t.y - 1, col.0, col.1, col.2, 255);
        }

        for s in &self.structures {
            match s.kind.as_str() {
                "hut" => draw_hut(&mut self.pixels_2d, s.x, s.y),
                "house" => draw_house(&mut self.pixels_2d, s.x, s.y, s.roof),
                "tower" => draw_tower(&mut self.pixels_2d, s.x, s.y, s.h),
                "factory" => draw_factory(&mut self.pixels_2d, s.x, s.y),
                "exchange" => draw_exchange(&mut self.pixels_2d, s.x, s.y),
                "hangar" => draw_hangar(&mut self.pixels_2d, s.x, s.y),
                "pad" => draw_pad(&mut self.pixels_2d, s.x, s.y),
                _ => {}
            }
        }
    }

    #[inline]
    fn set_tile(&mut self, x: i32, y: i32, t: u8) {
        if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H {
            self.tile_type[(y as usize) * W + (x as usize)] = t;
        }
    }

}

/// The base colour layer with land use painted on: tiles whose type differs from the base
/// type get farm (striped), road or paving colours; everything else keeps the terrain's.
pub fn paint_land_use(base: &[u8], types: &[u8], base_types: &[u8], w: usize, pal: &BiomePalette) -> Vec<u8> {
    let mut out = base.to_vec();
    for (i, (&t, &b)) in types.iter().zip(base_types.iter()).enumerate() {
        if t == b {
            continue;
        }
        let (x, y) = (i % w, i / w);
        let c = match t {
            T_FARM => pal.farm[(y >> 1) % 2],
            T_ROAD => pal.road[(x + y) % 2],
            T_PAVED => pal.paved[(x + y) % 2],
            _ => continue,
        };
        out[i * 4] = c.0;
        out[i * 4 + 1] = c.1;
        out[i * 4 + 2] = c.2;
        out[i * 4 + 3] = 255;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The land comes from moc-terrain (#109): random each run, at least five continents on
    // the home world (#129), 45–70% sea. (Same-seed determinism isn't a feature any more,
    // per Alex: "remove the deterministic feature for now".)
    #[test]
    fn test_continents_and_water_distribution() {
        for seed in [42u32, 7, 2026] {
            let t = Terrain::new(seed);
            assert!(t.continents.len() >= 3, "home and at least two landmasses boats reach, got {}", t.continents.len());
            assert_eq!(t.continents[0].id, "home");
            let total = (W * H) as f32;
            let deep = t.base_type.iter().filter(|&&k| k == T_DEEP).count() as f32 / total;
            let sea = t.base_type.iter().filter(|&&k| k <= T_SHALLOW).count() as f32 / total;
            assert!((0.45..=0.70).contains(&sea), "sea share {sea:.2}");
            assert!(deep > 0.3, "deep water {deep:.2}");
            assert!(t.walkable(t.sites.village.x, t.sites.village.y));
            assert!(t.walkable(t.sites.pad.x, t.sites.pad.y));
            for c in t.continents.iter().skip(1) {
                assert!(t.get(c.harbor.dock.x, c.harbor.dock.y) <= T_SHALLOW, "{} dock on water", c.id);
                assert!(t.walkable(c.village.x, c.village.y), "{} village on land", c.id);
            }
        }
    }

    #[test]
    fn test_all_biomes_palettes() {
        for biome in [BiomeType::Temperate, BiomeType::Desert, BiomeType::Ice, BiomeType::Jungle, BiomeType::Alien] {
            let t = Terrain::with_biome(42, biome);
            assert_eq!(t.pixels.len(), W * H * 4);
            assert!(t.pixels.iter().any(|&b| b > 0));
        }
    }

    #[test]
    fn test_bfs_field_pathfinding_and_mountain_barrier() {
        let mut t = Terrain::new(42);
        let v = t.sites.village;
        let field = t.field(v.x, v.y).to_vec();

        // Target on 3px quantized grid has distance 0, village is <= 1 away
        let qx = (v.x as f32 / 3.0).round() as usize * 3;
        let qy = (v.y as f32 / 3.0).round() as usize * 3;
        assert_eq!(field[qy * W + qx], 0, "quantized center must have distance 0");
        let v_idx = (v.y as usize) * W + (v.x as usize);
        assert!(field[v_idx] <= 1, "village tile distance must be <= 1");

        // Water and mountains must be unreachable (-1)
        for y in 0..H {
            for x in 0..W {
                let idx = y * W + x;
                let tile = t.tile_type[idx];
                if tile <= T_SHALLOW || tile == T_MOUNTAIN || tile == T_SNOW {
                    assert_eq!(field[idx], -1, "unwalkable tiles must have distance -1");
                }
            }
        }

        // Sea field for boats
        let sea = t.sea_field(10, 10).to_vec();
        assert_eq!(sea[10 * W + 10], 0);
    }

    #[test]
    fn test_era_repaints() {
        let mut t = Terrain::new(42);
        t.bake_era(0); // Stone
        let stone_structures = t.structures.len();
        assert_eq!(stone_structures, 4, "Stone era has 4 huts");

        t.bake_era(1); // Farming
        assert!(t.structures.len() > 10, "Farming has village and external huts");

        t.bake_era(2); // Industry
        assert!(t.structures.iter().any(|s| s.kind == "factory"));
        assert!(t.structures.iter().any(|s| s.kind == "exchange"));

        t.bake_era(3); // Space
        assert!(t.structures.iter().any(|s| s.kind == "hangar"));
        assert!(t.structures.iter().any(|s| s.kind == "pad"));
    }
}
