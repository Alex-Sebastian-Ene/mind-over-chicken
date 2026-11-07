//! The planet the civilisations live on. Four home continents in a row along the equator,
//! joined by land bridges, one per starting civilisation; on a bigger planet, more continents
//! elsewhere wait to be settled (some joined by bridges, some islands). Polar ice at the top
//! and bottom; everything wraps east–west.
//!
//! Rivers run from the high middle of each continent down to the coast: walkable fords that
//! slow walkers and weaken an attack across them (`terrain_mul::RIVER_CROSSING`, #69).
//!
//! World generation lives behind one function, [`Terrain::generate`], so another world shape
//! (for example a disc, see #52) can be swapped in without touching the simulation.

use moc_planet::noise::WrapNoise;
use moc_planet::rng::Rng;

use crate::planet::Dims;

pub const DEEP: u8 = 0;
pub const WATER: u8 = 1;
pub const SAND: u8 = 2;
pub const GRASS: u8 = 3;
pub const FOREST: u8 = 4;
pub const HILLS: u8 = 5;
pub const MOUNTAIN: u8 = 6;
pub const ICE: u8 = 7;

/// Home continents (one per starting civilisation).
pub const HOME_REGIONS: usize = 4;

/// Most food a tile can hold, by kind.
pub fn food_max(kind: u8) -> f32 {
    match kind {
        GRASS => 1.0,
        FOREST => 0.8,
        HILLS => 0.45,
        SAND => 0.2,
        _ => 0.0,
    }
}

/// Walking speed multiplier, by kind.
pub fn speed_mul(kind: u8) -> f32 {
    match kind {
        HILLS => 0.7,
        FOREST => 0.85,
        SAND => 0.95,
        _ => 1.0,
    }
}

/// How much harder a defender standing on this terrain is to hurt (combat divides damage by
/// it): `data/ages.json` conflict.battle.terrain. Walls and river crossings are applied in
/// combat (`agents::combat`), since they depend on villages and on where the attacker stands.
pub fn defence(kind: u8) -> f32 {
    use crate::ages_table::terrain_mul as m;
    match kind {
        HILLS => m::HILL,
        FOREST => m::FOREST,
        _ => m::OPEN_GROUND,
    }
}

/// Walking speed through a river ford.
pub const RIVER_SPEED: f32 = 0.6;

pub fn flammable(kind: u8) -> bool {
    kind == GRASS || kind == FOREST
}

/// A continent. The first four are the civilisations' home regions.
#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
    /// A grass tile with room round it, near the centre: where a civilisation starts.
    pub start: (i32, i32),
}

pub struct Terrain {
    pub d: Dims,
    pub kind: Vec<u8>,
    pub walk: Vec<bool>,
    /// Connected landmass of each walkable tile (0 = not walkable). Same id = reachable on foot.
    pub land_id: Vec<u16>,
    /// River fords (walkable land tiles a river runs through).
    pub river: Vec<bool>,
    /// The planet painted once, w×h RGBA (burnt tiles are repainted as they burn and regrow).
    pub rgba: Vec<u8>,
    pub regions: Vec<Region>,
    pub seed: u32,
}

fn hash(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

impl Terrain {
    /// Generate a planet of size `d` for a run seed. Same seed and size, same planet.
    pub fn generate(seed: u32, d: Dims) -> Terrain {
        let (w, h) = (d.w as f32, d.h as f32);
        let mut rng = Rng::new(seed ^ 0x51ed_270b);
        // Noise cells keep the same size in tiles whatever the planet size.
        let period = |cell: f32| ((w / cell).round() as usize).max(2);
        let (c_shape, c_relief, c_moist, c_ice) = (80.0, 46.0, 107.0, 40.0);
        let shape = WrapNoise::new(&mut rng, period(c_shape), 5);
        let relief = WrapNoise::new(&mut rng, period(c_relief), 4);
        let moist = WrapNoise::new(&mut rng, period(c_moist), 4);
        let icy = WrapNoise::new(&mut rng, period(c_ice), 3);

        // Four home continents in a row along the equator, one per civilisation.
        let gap = (w / 4.0).min(115.0);
        let base = rng.range(0.0, w);
        let mut regions: Vec<Region> = Vec::new();
        for i in 0..HOME_REGIONS {
            let cx = (base + i as f32 * gap + rng.range(-0.08, 0.08) * gap).rem_euclid(w);
            let cy = h / 2.0 + rng.range(-0.075, 0.075) * h;
            regions.push(Region { cx, cy, rx: gap * rng.range(0.56, 0.64), ry: rng.range(58.0, 74.0).min(h * 0.2), start: (0, 0) });
        }
        // More continents on a bigger planet, waiting to be settled.
        let extra = ((d.n() as f32 / (640.0 * 320.0) * 2.5).round() as usize).saturating_sub(HOME_REGIONS).min(10);
        let mut tries = 0;
        while regions.len() < HOME_REGIONS + extra && tries < 600 {
            tries += 1;
            let (rx, ry) = (rng.range(60.0, 110.0), rng.range(45.0, 80.0));
            let (cx, cy) = (rng.range(0.0, w), rng.range(0.24 * h, 0.76 * h));
            let clear = regions.iter().all(|r| {
                let dx = d.dx(r.cx, cx).abs() / (r.rx + rx);
                let dy = (r.cy - cy).abs() / (r.ry + ry);
                dx * dx + dy * dy > 1.0
            });
            if clear {
                regions.push(Region { cx, cy, rx, ry, start: (0, 0) });
            }
        }
        // Land bridges: between neighbouring home continents (closing the ring on a small
        // planet), and from each other continent to its nearest neighbour half the time.
        let mut bridges = Vec::new();
        for i in 0..HOME_REGIONS - 1 {
            bridges.push((i, i + 1, rng.range(9.0, 13.0)));
        }
        if gap * 4.0 >= w - 1.0 && rng.chance(0.7) {
            bridges.push((HOME_REGIONS - 1, 0, rng.range(9.0, 13.0)));
        }
        for e in HOME_REGIONS..regions.len() {
            let near = (0..regions.len())
                .filter(|&j| j != e)
                .min_by(|&a, &b| {
                    let da = d.dist(regions[e].cx, regions[e].cy, regions[a].cx, regions[a].cy);
                    let db = d.dist(regions[e].cx, regions[e].cy, regions[b].cx, regions[b].cy);
                    da.total_cmp(&db)
                })
                .unwrap_or(0);
            if rng.chance(0.5) {
                bridges.push((e, near, rng.range(8.0, 12.0)));
            }
        }

        let mut kind = vec![DEEP; d.n()];
        // Height of each tile (continent shape plus noise), kept for the rivers.
        let mut elev = vec![-2.0f32; d.n()];
        for y in 0..d.h {
            let yf = y as f32;
            let lat = ((yf + 0.5) / h - 0.5).abs() * 2.0;
            for x in 0..d.w {
                let xf = x as f32;
                let u = xf / w;
                let mut e = -2.0f32;
                for r in &regions {
                    let dx = d.dx(r.cx, xf) / r.rx;
                    let dy = (yf - r.cy) / r.ry;
                    e = e.max(1.0 - (dx * dx + dy * dy).sqrt());
                }
                let mut b = -1.0f32;
                for &(ia, ib, bw) in &bridges {
                    let (ra, rb) = (&regions[ia], &regions[ib]);
                    let (vx, vy) = (d.dx(ra.cx, rb.cx), rb.cy - ra.cy);
                    let (px, py) = (d.dx(ra.cx, xf), yf - ra.cy);
                    let t = ((px * vx + py * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
                    let (qx, qy) = (px - t * vx, py - t * vy);
                    b = b.max(0.5 * (1.0 - (qx * qx + qy * qy).sqrt() / bw));
                }
                let i = y * d.w + x;
                if lat > 0.79 && lat > 0.86 + (icy.fbm(u, yf / c_ice * 2.0, 3) - 0.5) * 0.14 {
                    kind[i] = ICE;
                    continue;
                }
                // Far from every continent and bridge: open ocean, no noise needed.
                if e < -0.72 && b < 0.02 {
                    continue;
                }
                let e = (e + (shape.fbm(u, yf / c_shape, 5) - 0.5) * 1.4).max(b);
                elev[i] = e;
                kind[i] = if e < 0.02 {
                    DEEP
                } else if e < 0.08 {
                    WATER
                } else if e < 0.115 {
                    SAND
                } else {
                    let r = relief.fbm(u, yf / c_relief, 4);
                    let m = moist.fbm(u, yf / c_moist, 4);
                    if r > 0.71 {
                        MOUNTAIN
                    } else if r > 0.64 {
                        HILLS
                    } else if m < 0.33 {
                        SAND
                    } else if m > 0.56 {
                        FOREST
                    } else {
                        GRASS
                    }
                };
            }
        }
        let river = trace_rivers(d, &kind, &elev, &regions, &mut rng);
        Self::finish(d, kind, river, regions, seed)
    }

    /// A planet of one kind everywhere (ice at the poles), for tests: four regions round the
    /// equator.
    pub fn uniform(d: Dims, k: u8) -> Terrain {
        let regions = (0..HOME_REGIONS)
            .map(|i| Region { cx: (i * d.w / HOME_REGIONS + 80) as f32, cy: d.h as f32 / 2.0, rx: 60.0, ry: 60.0, start: (0, 0) })
            .collect();
        let mut kind = vec![k; d.n()];
        for x in 0..d.w {
            kind[x] = ICE;
            kind[(d.h - 1) * d.w + x] = ICE;
        }
        Self::finish(d, kind, vec![false; d.n()], regions, 1)
    }

    /// Build a terrain from a kind grid (walkability, landmasses, colours, starts).
    pub fn from_kinds(d: Dims, kind: Vec<u8>, regions: Vec<Region>) -> Terrain {
        Self::finish(d, kind, vec![false; d.n()], regions, 1)
    }

    /// Mark a river ford (tests).
    pub fn set_river(&mut self, x: i32, y: i32, on: bool) {
        if self.d.in_y(y) {
            let i = self.d.idx(x, y);
            self.river[i] = on && self.walk[i];
            let c = self.base_colour(x, y);
            self.rgba[i * 4..i * 4 + 4].copy_from_slice(&c);
        }
    }

    fn finish(d: Dims, kind: Vec<u8>, river: Vec<bool>, mut regions: Vec<Region>, seed: u32) -> Terrain {
        let walk: Vec<bool> = kind.iter().map(|&k| matches!(k, SAND | GRASS | FOREST | HILLS)).collect();
        let land_id = label_landmasses(d, &walk);
        let river = river.iter().zip(&walk).map(|(&r, &w)| r && w).collect();
        let mut t = Terrain { d, kind, walk, land_id, river, rgba: vec![0; d.n() * 4], regions: Vec::new(), seed };
        for r in regions.iter_mut() {
            r.start = t.find_start(r.cx, r.cy);
        }
        t.regions = regions;
        for y in 0..d.h {
            for x in 0..d.w {
                let i = y * d.w + x;
                let c = t.base_colour(x as i32, y as i32);
                t.rgba[i * 4..i * 4 + 4].copy_from_slice(&c);
            }
        }
        t
    }

    #[inline]
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.d.in_y(y) && self.walk[self.d.idx(x, y)]
    }

    #[inline]
    pub fn kind_at(&self, x: i32, y: i32) -> u8 {
        if !self.d.in_y(y) {
            return ICE;
        }
        self.kind[self.d.idx(x, y)]
    }

    #[inline]
    pub fn river_at(&self, x: i32, y: i32) -> bool {
        self.d.in_y(y) && self.river[self.d.idx(x, y)]
    }

    /// Is there open water within `r` tiles?
    pub fn near_water(&self, x: i32, y: i32, r: i32) -> bool {
        (-r..=r).any(|oy| (-r..=r).any(|ox| matches!(self.kind_at(x + ox, y + oy), WATER | DEEP)))
    }

    #[inline]
    pub fn land(&self, x: i32, y: i32) -> u16 {
        if !self.d.in_y(y) {
            return 0;
        }
        self.land_id[self.d.idx(x, y)]
    }

    /// Nearest good spot to a point: grass (or any land) with walkable room around it.
    fn find_start(&self, cx: f32, cy: f32) -> (i32, i32) {
        let (cx, cy) = (cx.round() as i32, cy.round() as i32);
        let roomy = |x: i32, y: i32, need: i32| {
            let mut ok = 0;
            for oy in -3..=3 {
                for ox in -3..=3 {
                    if self.walkable(x + ox, y + oy) {
                        ok += 1;
                    }
                }
            }
            ok >= need
        };
        for pass in 0..3 {
            for r in 0..60i32 {
                for oy in -r..=r {
                    for ox in -r..=r {
                        if ox.abs() != r && oy.abs() != r {
                            continue;
                        }
                        let (x, y) = (self.d.wrap_x(cx + ox), cy + oy);
                        let k = self.kind_at(x, y);
                        let good = match pass {
                            0 => k == GRASS && roomy(x, y, 45),
                            1 => self.walkable(x, y) && roomy(x, y, 35),
                            _ => self.walkable(x, y),
                        };
                        if good {
                            return (x, y);
                        }
                    }
                }
            }
        }
        (self.d.wrap_x(cx), self.d.wrap_y(cy))
    }

    /// The painted colour of a tile (used again when burnt land grows back).
    pub fn base_colour(&self, x: i32, y: i32) -> [u8; 4] {
        let k = self.kind_at(x, y);
        let near_land = || (-1..=1).any(|oy| (-1..=1).any(|ox| self.walkable(x + ox, y + oy)));
        let river = self.river_at(x, y);
        let (r, g, b) = match k {
            _ if river => (70, 132, 196),
            DEEP => (24, 52, 98),
            WATER => {
                if near_land() {
                    (64, 124, 170)
                } else {
                    (40, 86, 140)
                }
            }
            SAND => (212, 192, 132),
            GRASS => (88, 154, 70),
            FOREST => (44, 104, 56),
            HILLS => (132, 128, 84),
            MOUNTAIN => (118, 110, 108),
            _ => (226, 236, 244),
        };
        let h = hash((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ self.seed);
        let j = (h % 13) as i32 - 6;
        let j = if k == FOREST && h & 0x100 != 0 { j - 10 } else { j };
        let f = |c: i32| (c + j).clamp(0, 255) as u8;
        [f(r), f(g), f(b), 255]
    }
}

/// Rivers: from high ground in the middle of a continent, downhill (by `elev`) to the sea.
/// A river that finds no lower ground carves on through a few rises, then ends in a marsh.
fn trace_rivers(d: Dims, kind: &[u8], elev: &[f32], regions: &[Region], rng: &mut Rng) -> Vec<bool> {
    let mut river = vec![false; d.n()];
    let want = (regions.len() * 3).min(40);
    let mut made = 0;
    for _ in 0..want * 12 {
        if made >= want {
            break;
        }
        let r = regions[rng.below(regions.len())];
        let (x, y) = ((r.cx + rng.range(-0.5, 0.5) * r.rx) as i32, (r.cy + rng.range(-0.5, 0.5) * r.ry) as i32);
        if !d.in_y(y) {
            continue;
        }
        let s = d.idx(x, y);
        if !matches!(kind[s], GRASS | FOREST | HILLS) || elev[s] < 0.4 || river[s] {
            continue;
        }
        let (mut cx, mut cy) = (d.wrap_x(x), y);
        let mut path = vec![s];
        let mut carve = 0;
        let mut ok = false;
        for _ in 0..260 {
            let here = elev[d.idx(cx, cy)];
            let mut best: Option<(f32, i32, i32)> = None;
            for (ox, oy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                let (nx, ny) = (d.wrap_x(cx + ox), d.wrap_y(cy + oy));
                let j = d.idx(nx, ny);
                if matches!(kind[j], MOUNTAIN | ICE) || path.contains(&j) {
                    continue;
                }
                let e = elev[j] + if ox != 0 && oy != 0 { 0.004 } else { 0.0 };
                if best.map_or(true, |b| e < b.0) {
                    best = Some((e, nx, ny));
                }
            }
            let Some((e, nx, ny)) = best else { break };
            if e >= here {
                carve += 1;
                if carve > 4 {
                    break;
                }
            }
            let j = d.idx(nx, ny);
            (cx, cy) = (nx, ny);
            if matches!(kind[j], WATER | DEEP) {
                ok = true;
                break;
            }
            path.push(j);
        }
        if path.len() >= 12 && (ok || path.len() >= 30) {
            for &i in &path {
                river[i] = true;
            }
            made += 1;
        }
    }
    river
}

/// Connected components of walkable land (8-neighbour, wrapping both ways).
fn label_landmasses(d: Dims, walk: &[bool]) -> Vec<u16> {
    let mut id = vec![0u16; d.n()];
    let mut next = 1u16;
    let mut stack = Vec::new();
    for s in 0..d.n() {
        if !walk[s] || id[s] != 0 {
            continue;
        }
        id[s] = next;
        stack.push(s);
        while let Some(i) = stack.pop() {
            let (x, y) = ((i % d.w) as i32, (i / d.w) as i32);
            for oy in -1..=1 {
                let ny = y + oy;
                for ox in -1..=1 {
                    let j = d.idx(x + ox, ny);
                    if walk[j] && id[j] == 0 {
                        id[j] = next;
                        stack.push(j);
                    }
                }
            }
        }
        next = next.saturating_add(1);
    }
    id
}
