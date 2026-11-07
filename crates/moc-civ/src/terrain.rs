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

use std::f32::consts::{PI, TAU};

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
        // Past the default planet, continents (and the noise cells that shape them) grow with
        // the planet, so a bigger planet has bigger lands rather than more empty ocean (Alex:
        // bigger islands). The default planet and smaller ones are unchanged.
        let s = (d.n() as f32 / Dims::DEFAULT.n() as f32).sqrt().max(1.0);
        let period = |cell: f32| ((w / cell).round() as usize).max(2);
        let (c_shape, c_relief, c_moist, c_ice) = (80.0 * s, 46.0 * s, 107.0 * s, 40.0);
        let shape = WrapNoise::new(&mut rng, period(c_shape), 5);
        let relief = WrapNoise::new(&mut rng, period(c_relief), 4);
        let moist = WrapNoise::new(&mut rng, period(c_moist), 4);
        let icy = WrapNoise::new(&mut rng, period(c_ice), 3);

        // Continents spread unevenly and shaped like real ones (Alex). Each is a chain of
        // overlapping, rotated, stretched lobes along a wandering (sometimes branching) spine,
        // and the whole map is domain-warped, so they come out with arms, peninsulas and bays.
        // Sizes follow a power law (a few big continents among many smaller ones), most land
        // is placed in clusters next to other land, and island chains trail off the coasts,
        // leaving wide open oceans elsewhere.
        let (c_warp, warp_amp) = (150.0 * s, 34.0 * s);
        let warp_x = WrapNoise::new(&mut rng, period(c_warp), 3);
        let warp_y = WrapNoise::new(&mut rng, period(c_warp), 3);
        let mut lobes: Vec<Lobe> = Vec::new();
        let mut regions: Vec<Region> = Vec::new();
        // Each continent's reach from its centre, for spacing.
        let mut reach: Vec<f32> = Vec::new();
        // The four home continents (one per civilisation): a loose cluster, each one budding
        // off one placed before it and joined to it by an isthmus, so the first peoples can
        // meet on foot.
        let mut home_parent = [0usize; HOME_REGIONS];
        let mut home_r = [0.0f32; HOME_REGIONS];
        for i in 0..HOME_REGIONS {
            let r = rng.range(66.0, 100.0) * s;
            let (cx, cy) = if i == 0 {
                (rng.range(0.0, w), h * rng.range(0.42, 0.58))
            } else {
                let p = rng.below(i);
                home_parent[i] = p;
                let mut pos = (0.0, 0.0);
                for _ in 0..12 {
                    // Bud off sideways rather than in a line, and away from the other homes.
                    let a = rng.range(0.0, TAU);
                    let dist = (home_r[p] + r) * rng.range(0.85, 1.1);
                    pos = (d.wrap_xf(regions[p].cx + a.cos() * dist), (regions[p].cy + a.sin() * dist * 0.7).clamp(0.28 * h, 0.72 * h));
                    if (0..i).all(|j| d.dist(regions[j].cx, regions[j].cy, pos.0, pos.1) > (home_r[j] + r) * 0.8) {
                        break;
                    }
                }
                pos
            };
            let (reg, ext) = continent(&mut rng, &mut lobes, regions.len(), cx, cy, r, s, h);
            regions.push(reg);
            reach.push(ext);
            home_r[i] = r;
        }
        // The other continents, waiting to be settled; more on a bigger planet.
        let per = 3.2 + (s - 1.0).min(1.0) * 1.8;
        let extra = ((d.n() as f32 / (640.0 * 320.0 * s * s) * per).round() as usize).saturating_sub(HOME_REGIONS).min(18);
        let mut tries = 0;
        while regions.len() < HOME_REGIONS + extra && tries < 900 {
            tries += 1;
            let r = (30.0 + 110.0 * rng.range(0.0, 1.0).powf(2.0)) * s;
            // Mostly next to land already placed (clusters), sometimes out in the open ocean.
            let (cx, cy) = if rng.chance(0.65) {
                let p = rng.below(regions.len());
                let a = rng.range(0.0, TAU);
                let dist = (reach[p] * 0.7 + r) * rng.range(0.9, 1.5);
                (d.wrap_xf(regions[p].cx + a.cos() * dist), regions[p].cy + a.sin() * dist * 0.7)
            } else {
                (rng.range(0.0, w), rng.range(0.25 * h, 0.75 * h))
            };
            if !(0.22 * h..0.78 * h).contains(&cy) {
                continue;
            }
            let clear = regions.iter().zip(&reach).all(|(o, &oe)| d.dist(o.cx, o.cy, cx, cy) > (oe * 0.6 + r) * 0.9);
            if clear {
                let (reg, ext) = continent(&mut rng, &mut lobes, regions.len(), cx, cy, r, s, h);
                regions.push(reg);
                reach.push(ext);
            }
        }
        // Island chains trailing off some coasts (not continents: no region of their own).
        let n_cont = lobes.len();
        for k in 0..regions.len() {
            if !rng.chance(0.4) {
                continue;
            }
            let own: Vec<Lobe> = lobes[..n_cont].iter().copied().filter(|l| l.owner == k).collect();
            let l = own[rng.below(own.len())];
            let mut a = rng.range(0.0, TAU);
            let lr = l.rx.max(l.ry);
            let (mut x, mut y) = (l.cx + a.cos() * (lr + 14.0 * s), l.cy + a.sin() * (lr + 14.0 * s));
            for _ in 0..(3 + rng.below(5)) {
                let ir = rng.range(5.0, 15.0) * s;
                if (0.2 * h..0.8 * h).contains(&y) {
                    lobes.push(Lobe::new(d.wrap_xf(x), y, ir * rng.range(1.0, 1.6), ir, rng.range(0.0, PI), usize::MAX));
                }
                a += rng.range(-0.6, 0.6);
                let step = ir * rng.range(2.0, 3.6);
                (x, y) = (x + a.cos() * step, y + a.sin() * step);
            }
        }
        // Land bridges: between consecutive home continents, and from each other continent to
        // its nearest neighbour half the time, only across a narrow strait: a long bridge over
        // open sea reads as a strip of land drawn with a ruler (Alex). Each runs between the two
        // continents' closest lobes.
        let closest = |a: usize, b: usize| -> (Region, Region, f32) {
            let mut best = (Region::at(0.0, 0.0, 1.0), Region::at(0.0, 0.0, 1.0), f32::MAX);
            for la in lobes[..n_cont].iter().filter(|l| l.owner == a) {
                for lb in lobes[..n_cont].iter().filter(|l| l.owner == b) {
                    let (ra, rb) = ((la.rx * la.ry).sqrt(), (lb.rx * lb.ry).sqrt());
                    let gap = d.dist(la.cx, la.cy, lb.cx, lb.cy) - ra - rb;
                    if gap < best.2 {
                        best = (Region::at(la.cx, la.cy, ra), Region::at(lb.cx, lb.cy, rb), gap);
                    }
                }
            }
            best
        };
        let mut bridges: Vec<(Region, Region, f32)> = Vec::new();
        for i in 1..HOME_REGIONS {
            let (a, b, _) = closest(i, home_parent[i]);
            bridges.push((a, b, rng.range(9.0, 13.0) * s));
        }
        for e in HOME_REGIONS..regions.len() {
            let near = (0..regions.len()).filter(|&j| j != e).map(|j| (j, closest(e, j))).min_by(|a, b| a.1 .2.total_cmp(&b.1 .2));
            let bw = rng.range(8.0, 12.0) * s;
            if let Some((_, (a, b, gap))) = near {
                if rng.chance(0.5) && gap < 5.0 * bw {
                    bridges.push((a, b, bw));
                }
            }
        }
        // The bridges as land: bent, wiggling isthmuses, never ruler-straight strips (Alex).
        let mut bmap = vec![-1.0f32; d.n()];
        let mut spine = vec![false; d.n()];
        for (a, b, bw) in &bridges {
            stamp_bridge(d, a, b, *bw, &mut rng, &mut bmap, &mut spine);
        }

        // Which lobes can reach each 64-tile cell (with the warp), so a tile only tests a few.
        const CELL: usize = 64;
        let (gw, gh) = (d.w.div_ceil(CELL), d.h.div_ceil(CELL));
        let mut grid: Vec<Vec<u16>> = vec![Vec::new(); gw * gh];
        for (k, l) in lobes.iter().enumerate() {
            let rr = 1.75 * l.rx.max(l.ry) + warp_amp;
            let (x0, x1) = (((l.cx - rr) / CELL as f32).floor() as i32, ((l.cx + rr) / CELL as f32).floor() as i32);
            let (y0, y1) = ((((l.cy - rr) / CELL as f32).floor() as i32).max(0), (((l.cy + rr) / CELL as f32).floor() as i32).min(gh as i32 - 1));
            for gy in y0..=y1 {
                for gx in x0..=x1.min(x0 + gw as i32 - 1) {
                    grid[gy as usize * gw + gx.rem_euclid(gw as i32) as usize].push(k as u16);
                }
            }
        }
        let mut kind = vec![DEEP; d.n()];
        // Height of each tile (continent shape plus noise), kept for the rivers.
        let mut elev = vec![-2.0f32; d.n()];
        for y in 0..d.h {
            let yf = y as f32;
            let lat = ((yf + 0.5) / h - 0.5).abs() * 2.0;
            let row = &grid[(y / CELL) * gw..(y / CELL + 1) * gw];
            for x in 0..d.w {
                let xf = x as f32;
                let u = xf / w;
                let mut e = -2.0f32;
                // Only lobes that reach this cell (plus the warp) can lift it out of the open
                // ocean (`e` below -0.72 is deep sea whatever the noise).
                let near = &row[x / CELL];
                if !near.is_empty() {
                    // Domain warp: bends coasts into bays and peninsulas at the continent scale.
                    let wx = xf + (warp_x.fbm(u, yf / c_warp, 3) - 0.5) * 2.0 * warp_amp;
                    let wy = yf + (warp_y.fbm(u, yf / c_warp + 17.0, 3) - 0.5) * 2.0 * warp_amp;
                    for &k in near {
                        let l = &lobes[k as usize];
                        let (dx, dy) = (d.dx(l.cx, wx), wy - l.cy);
                        let (a, b) = ((dx * l.cos + dy * l.sin) / l.rx, (dy * l.cos - dx * l.sin) / l.ry);
                        e = e.max(1.0 - (a * a + b * b).sqrt());
                    }
                }
                let i = y * d.w + x;
                let b = bmap[i];
                if lat > 0.79 && lat > 0.86 + (icy.fbm(u, yf / c_ice * 2.0, 3) - 0.5) * 0.14 {
                    kind[i] = ICE;
                    continue;
                }
                // Far from every continent and bridge: open ocean, no noise needed.
                if e < -0.72 && b < 0.02 {
                    continue;
                }
                // The coast noise roughens the bridges' edges too (softer); only a bridge's spine
                // is sure to be land, so the continents it joins always stay joined on foot.
                let n = (shape.fbm(u, yf / c_shape, 5) - 0.5) * 1.4;
                let e = (e + n).max(b + n * 0.45).max(if spine[i] { 0.2 } else { -2.0 });
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
                    if r > 0.71 && !spine[i] {
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

/// One lobe of a continent (or an island): a rotated ellipse.
#[derive(Clone, Copy, Debug)]
struct Lobe {
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    cos: f32,
    sin: f32,
    /// The continent (`regions` index) it belongs to; `usize::MAX` for an island.
    owner: usize,
}

impl Lobe {
    fn new(cx: f32, cy: f32, rx: f32, ry: f32, angle: f32, owner: usize) -> Lobe {
        Lobe { cx, cy, rx, ry, cos: angle.cos(), sin: angle.sin(), owner }
    }
}

impl Region {
    /// A round region (bridge ends).
    fn at(cx: f32, cy: f32, r: f32) -> Region {
        Region { cx, cy, rx: r, ry: r, start: (0, 0) }
    }
}

/// Lay out one continent of size `r` round (`cx`, `cy`): a big stretched lobe, then more
/// along a spine that wanders and now and then branches back from the middle (arms and
/// peninsulas). Returns its region (centred on the main lobe) and its reach from the centre.
#[allow(clippy::too_many_arguments)]
fn continent(rng: &mut Rng, lobes: &mut Vec<Lobe>, owner: usize, cx: f32, cy: f32, r: f32, s: f32, h: f32) -> (Region, f32) {
    let main = r * rng.range(0.74, 0.9);
    let aspect = rng.range(1.0, 1.9).sqrt();
    let mut heading = rng.range(0.0, TAU);
    lobes.push(Lobe::new(cx, cy, main * aspect, main / aspect, heading, owner));
    let n = (1 + rng.below(3) + (r / (32.0 * s)) as usize).min(7);
    let (mut x, mut y, mut lr) = (cx, cy, main);
    let mut ext = main * aspect;
    for _ in 1..n {
        if rng.chance(0.3) {
            // Branch: a new arm from the middle.
            (x, y, lr) = (cx, cy, main);
            heading += rng.range(1.6, 4.7);
        } else {
            heading += rng.range(-0.9, 0.9);
        }
        let nr = r * rng.range(0.36, 0.7);
        let step = (lr + nr) * rng.range(0.42, 0.72);
        x += heading.cos() * step;
        y = (y + heading.sin() * step * 0.8).clamp(0.16 * h, 0.84 * h);
        let st = rng.range(1.2, 2.6).sqrt();
        lobes.push(Lobe::new(x, y, nr * st, nr / st, heading + rng.range(-0.4, 0.4), owner));
        ext = ext.max(((x - cx).powi(2) + (y - cy).powi(2)).sqrt() + nr * st);
        lr = nr;
    }
    (Region::at(cx, cy, main), ext)
}

/// Paint one land bridge from continent `a` to `b` into `bmap` (0.5 on the spine, 0 at the
/// edge) and `spine` (its middle third, kept walkable). The spine bows to one side and wiggles,
/// the width swells and narrows along it, and it flares where it leaves each continent, so
/// it reads as a natural isthmus rather than a straight strip.
fn stamp_bridge(d: Dims, a: &Region, b: &Region, bw: f32, rng: &mut Rng, bmap: &mut [f32], spine: &mut [bool]) {
    let (vx, vy) = (d.dx(a.cx, b.cx), b.cy - a.cy);
    let len = (vx * vx + vy * vy).sqrt().max(1.0);
    let (nx, ny) = (-vy / len, vx / len);
    // Each continent's radius along the bridge, to find where the bridge leaves it.
    let radius = |r: &Region| {
        let (c, s) = (vx / len, vy / len);
        1.0 / ((c / r.rx).powi(2) + (s / r.ry).powi(2)).sqrt()
    };
    let (ra, rb) = (radius(a), radius(b));
    let bend = rng.range(0.1, 0.28) * len * if rng.chance(0.5) { 1.0 } else { -1.0 };
    let (wig, wk, wp) = (rng.range(0.03, 0.07) * len, rng.range(2.0, 3.5), rng.range(0.0, TAU));
    let (swell, sp) = (rng.range(0.15, 0.35), rng.range(0.0, TAU));
    let at = |t: f32| {
        let env = (PI * t).sin();
        let off = bend * env + wig * env * (PI * wk * t + wp).sin();
        let shore = (t * len - ra).min((1.0 - t) * len - rb).max(0.0);
        let width = bw * (1.0 + swell * (TAU * 1.5 * t + sp).sin()) * (1.0 + 1.4 * (-shore / (1.5 * bw)).exp());
        (a.cx + vx * t + nx * off, a.cy + vy * t + ny * off, width)
    };
    let steps = ((len / 4.0).ceil() as usize).clamp(8, 400);
    let mut p0 = at(0.0);
    for k in 1..=steps {
        let p1 = at(k as f32 / steps as f32);
        let r = p0.2.max(p1.2);
        let (sx, sy) = (p1.0 - p0.0, p1.1 - p0.1);
        let sl = (sx * sx + sy * sy).max(1e-6);
        let (x0, x1) = ((p0.0.min(p1.0) - r).floor() as i32, (p0.0.max(p1.0) + r).ceil() as i32);
        let (y0, y1) = ((p0.1.min(p1.1) - r).floor() as i32, (p0.1.max(p1.1) + r).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 - p0.0, y as f32 - p0.1);
                let u = ((px * sx + py * sy) / sl).clamp(0.0, 1.0);
                let (qx, qy) = (px - u * sx, py - u * sy);
                let dist = (qx * qx + qy * qy).sqrt();
                let width = p0.2 + (p1.2 - p0.2) * u;
                let i = d.idx(x, y);
                bmap[i] = bmap[i].max(0.5 * (1.0 - dist / width));
                if dist < width * 0.3 {
                    spine[i] = true;
                }
            }
        }
        p0 = p1;
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
