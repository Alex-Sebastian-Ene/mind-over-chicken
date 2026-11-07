//! Continents (connected landmasses) and ranked settlement sites.

use crate::grid::{self, Dims};
use crate::climate::{biome, tile};

#[derive(Clone, Debug)]
pub struct Continent {
    /// Index in `Terrain::continents` (largest first).
    pub id: usize,
    /// Land tiles.
    pub area: usize,
    /// Bounding box: x0, y0 and its width and height in tiles. On a wrapping map it can run
    /// across a seam (x0 + w > map width).
    pub x0: i32,
    pub y0: i32,
    pub bw: i32,
    pub bh: i32,
    /// Centroid (circular mean on wrapping axes).
    pub cx: f32,
    pub cy: f32,
    /// A major landmass rather than an island.
    pub major: bool,
}

#[derive(Clone, Debug)]
pub struct Site {
    pub x: i32,
    pub y: i32,
    pub score: f32,
    /// `Terrain::continents` index.
    pub continent: usize,
    /// Walkable region (same id: reachable on foot from each other).
    pub region: u32,
    pub coastal: bool,
    pub river: bool,
    pub lake: bool,
}

/// Label landmasses (biome is land, 8-neighbour, wrapping). Returns per-tile continent index
/// + 1 (0 = water) and the continents, largest first.
pub fn continents(d: Dims, biome_map: &[u8], major_min: usize) -> (Vec<u16>, Vec<Continent>) {
    let land: Vec<bool> = biome_map.iter().map(|&b| biome::is_land(b)).collect();
    let (label, sizes) = grid::components(d, &land);
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| sizes[b].cmp(&sizes[a]).then(a.cmp(&b)));
    let mut rank = vec![0usize; sizes.len()];
    for (r, &k) in order.iter().enumerate() {
        rank[k] = r;
    }
    let mut id = vec![0u16; d.n()];
    // Occupied columns/rows and circular sums per continent.
    let nc = sizes.len();
    let mut cols = vec![Vec::<bool>::new(); nc];
    let mut rows = vec![Vec::<bool>::new(); nc];
    let mut sums = vec![[0.0f64; 4]; nc];
    let (tx, ty) = (std::f64::consts::TAU / d.w as f64, std::f64::consts::TAU / d.h as f64);
    let col: Vec<(f64, f64)> = (0..d.w).map(|x| ((x as f64 * tx).cos(), (x as f64 * tx).sin())).collect();
    let row: Vec<(f64, f64)> = (0..d.h).map(|y| ((y as f64 * ty).cos(), (y as f64 * ty).sin())).collect();
    for i in 0..d.n() {
        if label[i] == 0 {
            continue;
        }
        let r = rank[label[i] as usize - 1];
        id[i] = (r + 1).min(u16::MAX as usize) as u16;
        if cols[r].is_empty() {
            cols[r] = vec![false; d.w];
            rows[r] = vec![false; d.h];
        }
        let (x, y) = d.xy(i);
        cols[r][x as usize] = true;
        rows[r][y as usize] = true;
        let s = &mut sums[r];
        let (cx, sx) = col[x as usize];
        let (cy, sy) = row[y as usize];
        s[0] += cx;
        s[1] += sx;
        s[2] += cy;
        s[3] += sy;
    }
    let span = |occ: &[bool], wrap: bool| -> (i32, i32) {
        let n = occ.len();
        if !wrap {
            let a = occ.iter().position(|&b| b).unwrap_or(0);
            let b = occ.iter().rposition(|&b| b).unwrap_or(0);
            return (a as i32, (b - a + 1) as i32);
        }
        // The complement of the longest circular run of empty cells.
        let (mut best_len, mut best_end, mut run) = (0usize, 0usize, 0usize);
        for k in 0..2 * n {
            if !occ[k % n] {
                run += 1;
                if run > best_len && run <= n {
                    best_len = run;
                    best_end = k % n;
                }
            } else {
                run = 0;
            }
        }
        if best_len == 0 {
            return (0, n as i32);
        }
        let start = (best_end + 1) % n;
        (start as i32, (n - best_len) as i32)
    };
    let mut out = Vec::with_capacity(nc);
    for (r, &k) in order.iter().enumerate() {
        let (x0, bw) = span(&cols[r], d.wrap_x);
        let (y0, bh) = span(&rows[r], d.wrap_y);
        let s = sums[r];
        let mean = |c: f64, sn: f64, n: usize| ((sn.atan2(c) / std::f64::consts::TAU).rem_euclid(1.0) * n as f64) as f32;
        let (cx, cy) = if d.wrap_x || d.wrap_y {
            (mean(s[0], s[1], d.w), mean(s[2], s[3], d.h))
        } else {
            (x0 as f32 + bw as f32 * 0.5, y0 as f32 + bh as f32 * 0.5)
        };
        out.push(Continent { id: r, area: sizes[k], x0, y0, bw, bh, cx, cy, major: sizes[k] >= major_min });
    }
    (id, out)
}

/// Inputs for ranking settlement sites, all on the full grid.
pub struct SiteInput<'a> {
    pub d: Dims,
    pub height: &'a [f32],
    pub biome: &'a [u8],
    pub tiles: &'a [u8],
    pub continent: &'a [u16],
    /// Distance (tiles) to fresh water (river or lake), and to the sea, on the simulation
    /// grid (`cell` tiles per cell, `sim_w` cells wide).
    pub fresh_dist: &'a [f32],
    pub sea_dist: &'a [f32],
    pub cell: usize,
    pub sim_w: usize,
    pub max_sites: usize,
}

/// Walkable regions (8-neighbour, wrapping): same id, reachable on foot.
pub fn regions(d: Dims, tiles: &[u8]) -> (Vec<u32>, Vec<usize>) {
    let walk: Vec<bool> = tiles.iter().map(|&t| tile::walkable(t)).collect();
    grid::components(d, &walk)
}

/// Rank settlement sites: flat, low, temperate land near fresh water (a river or lake) or on
/// the coast, spread out. Best first. Only sites in sizeable walkable regions.
pub fn rank(inp: &SiteInput, region: &[u32], region_size: &[usize]) -> Vec<Site> {
    let d = inp.d;
    let land_area = inp.biome.iter().filter(|&&b| biome::is_land(b)).count().max(1);
    let stride = ((d.n() as f32 / 30_000.0).sqrt().round() as i32).max(2);
    let min_region = (land_area / 200).max(40);
    let mut cands: Vec<(f32, usize, bool, bool)> = Vec::new();
    let scale = ((d.n() as f32).sqrt() / 240.0).max(1.0);
    for y in (0..d.h as i32).step_by(stride as usize) {
        for x in (0..d.w as i32).step_by(stride as usize) {
            let i = y as usize * d.w + x as usize;
            let t = inp.tiles[i];
            if !tile::walkable(t) || inp.continent[i] == 0 {
                continue;
            }
            let rg = region[i];
            if rg == 0 || region_size[rg as usize - 1] < min_region {
                continue;
            }
            let si = (y as usize / inp.cell) * inp.sim_w + x as usize / inp.cell;
            let (fd, sd) = (inp.fresh_dist[si] / scale, inp.sea_dist[si] / scale);
            if fd > 10.0 && sd > 6.0 {
                continue; // no water within reach
            }
            // Flatness and room: the height range and walkable share in a 5×5 window.
            let (mut lo, mut hi, mut walk) = (f32::INFINITY, f32::NEG_INFINITY, 0);
            for oy in -2..=2 {
                for ox in -2..=2 {
                    if let Some(j) = d.idx(x + ox * stride.min(2), y + oy * stride.min(2)) {
                        let v = inp.height[j];
                        lo = lo.min(v);
                        hi = hi.max(v);
                        if tile::walkable(inp.tiles[j]) {
                            walk += 1;
                        }
                    }
                }
            }
            if walk < 20 {
                continue;
            }
            let b = inp.biome[i];
            let climate = match b {
                biome::GRASSLAND | biome::SAVANNA => 0.5,
                biome::STEPPE => 0.3,
                biome::FOREST => 0.25,
                biome::BEACH => 0.1,
                biome::JUNGLE | biome::TAIGA => 0.0,
                biome::TUNDRA => -0.4,
                biome::DESERT => -0.5,
                _ => -1.0,
            };
            let river = fd <= 3.0;
            let coastal = sd <= 3.0;
            let score = 1.0 + 1.3 * (-fd / 3.0).exp() + 0.8 * (-sd / 2.5).exp() + climate - (hi - lo) * 6.0 - inp.height[i].max(0.0) * 1.2
                - if t == tile::HILL { 0.3 } else { 0.0 };
            cands.push((score, i, river, coastal));
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let spacing = ((land_area as f32 / inp.max_sites.max(1) as f32).sqrt() * 0.75).max(6.0);
    let mut out: Vec<Site> = Vec::new();
    for &(score, i, river, coastal) in cands.iter() {
        if out.len() >= inp.max_sites {
            break;
        }
        let (x, y) = d.xy(i);
        if out.iter().any(|s| d.dist2(s.x as f32, s.y as f32, x as f32, y as f32) < spacing * spacing) {
            continue;
        }
        let lake = river && inp.biome.iter().len() > 0 && near_lake(d, inp.biome, x, y, 4);
        out.push(Site { x, y, score, continent: inp.continent[i] as usize - 1, region: region[i], coastal, river, lake });
    }
    out
}

fn near_lake(d: Dims, b: &[u8], x: i32, y: i32, r: i32) -> bool {
    for oy in -r..=r {
        for ox in -r..=r {
            if let Some(j) = d.idx(x + ox, y + oy) {
                if b[j] == biome::LAKE {
                    return true;
                }
            }
        }
    }
    false
}
