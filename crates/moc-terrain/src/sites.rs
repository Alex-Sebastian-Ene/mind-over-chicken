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

/// The main game's places on the home world (the eras and pathfinding use these; the names
/// match the JS `World.sites`). Gameplay paths don't cross the map's seams, so everything here
/// works on the plain rectangle.
#[derive(Clone, Debug, Default)]
pub struct CampaignSites {
    pub village: (usize, usize),
    pub pad: (usize, usize),
    pub ridge: (usize, usize),
    pub cave: (usize, usize),
    pub cave_mouth: (usize, usize),
    pub fire: (usize, usize),
    pub factory: (usize, usize),
    pub exchange: (usize, usize),
    pub picket: (usize, usize),
    pub hangar: (usize, usize),
    pub crowd: (usize, usize),
    pub harbour: (usize, usize),
}

/// A landmass as the game sees it: home first, then the others boats can reach.
#[derive(Clone, Debug)]
pub struct GameContinent {
    pub id: String,
    /// `TerrainMap::continents` index.
    pub index: usize,
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
    pub village: (i32, i32),
    /// (quay on land, dock on water) facing home, and home's facing this one.
    pub harbor: Option<((i32, i32), (i32, i32))>,
    pub home_harbor: Option<((i32, i32), (i32, i32))>,
}

/// Inputs for [`campaign`].
pub struct CampaignInput<'a> {
    pub w: usize,
    pub h: usize,
    pub heights: &'a [f32],
    pub types: &'a [u8],
    pub biome: &'a [u8],
    pub continent: &'a [u16],
    pub continents: &'a [Continent],
    pub sites: &'a [Site],
}

/// Lay out the campaign on a home world: the village on the home continent's best roomy site,
/// a coastal launch pad 45–120 tiles away on the same walkable land, the cave on the first
/// hill towards the nearest high ground, and a village and harbours on each other landmass,
/// with docks on open sea that boats can sail between.
pub fn campaign(inp: &CampaignInput) -> (CampaignSites, Vec<GameContinent>) {
    let (w, h) = (inp.w as i32, inp.h as i32);
    let d = Dims::new(inp.w, inp.h, false);
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
    let walk = |x: i32, y: i32| inside(x, y) && tile::walkable(inp.types[(y * w + x) as usize]);
    let sea = |x: i32, y: i32| inside(x, y) && matches!(inp.biome[(y * w + x) as usize], biome::OCEAN | biome::SHELF);
    let (region, region_size) = regions(d, inp.types);
    let reg = |x: i32, y: i32| if inside(x, y) { region[(y * w + x) as usize] } else { 0 };
    let roomy = |x: i32, y: i32, r: i32| {
        let mut k = 0;
        let mut t = 0;
        for oy in (-r..=r).step_by(2) {
            for ox in (-r..=r).step_by(2) {
                t += 1;
                if walk(x + ox, y + oy) {
                    k += 1;
                }
            }
        }
        k as f32 / t as f32
    };
    // Nearest walkable, non-forest tile in region `rg` (any walkable region when 0).
    let nearest = |x: i32, y: i32, rg: u32| -> (i32, i32) {
        for r in 0..80i32 {
            for oy in -r..=r {
                for ox in -r..=r {
                    if ox.abs().max(oy.abs()) != r {
                        continue;
                    }
                    let (px, py) = (x + ox, y + oy);
                    if walk(px, py) && inp.types[(py * w + px) as usize] != tile::FOREST && (rg == 0 || reg(px, py) == rg) {
                        return (px, py);
                    }
                }
            }
        }
        (x.clamp(0, w - 1), y.clamp(0, h - 1))
    };
    let home = 0usize;
    // Village: the best-ranked site on the home continent with room round it.
    let mut village = None;
    let mut best = f32::NEG_INFINITY;
    for s in inp.sites.iter().filter(|s| s.continent == home) {
        let room = roomy(s.x, s.y, 14);
        let rs = region_size.get(reg(s.x, s.y).max(1) as usize - 1).copied().unwrap_or(0) as f32;
        let score = s.score + room * 2.0 + (rs / 4000.0).min(1.0);
        if room > 0.55 && score > best {
            best = score;
            village = Some((s.x, s.y));
        }
    }
    let c0 = &inp.continents[home.min(inp.continents.len().saturating_sub(1))];
    let v = village.unwrap_or_else(|| nearest(c0.cx as i32, c0.cy as i32, 0));
    let v = nearest(v.0, v.1, 0);
    let vr = reg(v.0, v.1);
    // Launch pad: coastal, roomy, on the village's land, 45–120 tiles away (80 is best).
    let mut pad = None;
    let mut best = f32::NEG_INFINITY;
    for y in (2..h - 2).step_by(2) {
        for x in (2..w - 2).step_by(2) {
            if !walk(x, y) || reg(x, y) != vr {
                continue;
            }
            let dist = (((x - v.0).pow(2) + (y - v.1).pow(2)) as f32).sqrt();
            if !(45.0..=120.0).contains(&dist) {
                continue;
            }
            let coast = (1..=5).any(|r| sea(x + r, y) || sea(x - r, y) || sea(x, y + r) || sea(x, y - r));
            let room = roomy(x, y, 8);
            let score = -(dist - 80.0).abs() / 40.0 + if coast { 1.5 } else { 0.0 } + room * 2.0;
            if score > best {
                best = score;
                pad = Some((x, y));
            }
        }
    }
    let pad = pad.unwrap_or_else(|| nearest(v.0 + 70, v.1, vr));
    // Ridge: the highest ground near the village; the cave is where the walk there meets hills.
    let mut ridge = (v.0, (v.1 - 20).max(0));
    let mut top = f32::NEG_INFINITY;
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let dist = (((x - v.0).pow(2) + (y - v.1).pow(2)) as f32).sqrt();
            if !(12.0..=90.0).contains(&dist) {
                continue;
            }
            let i = (y * w + x) as usize;
            if inp.continent[i] as usize != home + 1 {
                continue;
            }
            let score = inp.heights[i] - dist * 0.0015;
            if score > top {
                top = score;
                ridge = (x, y);
            }
        }
    }
    let mut cave = nearest(v.0 - 20, v.1 - 20, vr);
    let mut cave_mouth = cave;
    let steps = 200;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (x, y) = ((v.0 as f32 + (ridge.0 - v.0) as f32 * t).round() as i32, (v.1 as f32 + (ridge.1 - v.1) as f32 * t).round() as i32);
        if !inside(x, y) {
            break;
        }
        if inp.types[(y * w + x) as usize] >= tile::HILL {
            let b = (i as i32 - 3).max(0) as f32 / steps as f32;
            cave = ((v.0 as f32 + (ridge.0 - v.0) as f32 * b).round() as i32, (v.1 as f32 + (ridge.1 - v.1) as f32 * b).round() as i32);
            cave_mouth = (x, y);
            break;
        }
    }
    let factory = nearest(v.0 - 46, v.1 + 26, vr);
    let exchange = nearest(v.0 + 18, v.1 - 10, vr);
    let picket = nearest(factory.0 + 2, factory.1 + 9, vr);
    let hangar = nearest(pad.0 - 26, pad.1 + 10, vr);
    let crowd = nearest(pad.0 - 30, pad.1 - 10, vr);

    // Other landmasses, nearest to home first, up to four.
    let mut others: Vec<usize> = (0..inp.continents.len()).filter(|&k| k != home).collect();
    let dist_home = |k: usize| {
        let c = &inp.continents[k];
        ((c.cx - v.0 as f32).powi(2) + (c.cy - v.1 as f32).powi(2)).sqrt()
    };
    others.sort_by(|&a, &b| dist_home(a).total_cmp(&dist_home(b)));
    others.truncate(4);
    // Water a boat can reach from a dock (8-neighbour, no wrap, like the JS seaField).
    let water_field = |x: i32, y: i32| -> Vec<bool> {
        let mut seen = vec![false; inp.w * inp.h];
        let mut q = std::collections::VecDeque::new();
        if sea(x, y) {
            seen[(y * w + x) as usize] = true;
            q.push_back((x, y));
        }
        while let Some((cx, cy)) = q.pop_front() {
            for &(ox, oy) in crate::grid::N8.iter() {
                let (nx, ny) = (cx + ox, cy + oy);
                if inside(nx, ny) && !seen[(ny * w + nx) as usize] && matches!(inp.types[(ny * w + nx) as usize], tile::DEEP | tile::SHALLOW) {
                    seen[(ny * w + nx) as usize] = true;
                    q.push_back((nx, ny));
                }
            }
        }
        seen
    };
    // Walk from `from` towards `to`: the last walkable tile of the landmass before open sea is
    // the quay, the first sea tile is the dock. Falls back to the coast tile of the landmass
    // nearest `to` whose sea `ok` accepts.
    let coast_toward = |from: (i32, i32), to: (i32, i32), land_of: usize, ok: &dyn Fn(i32, i32) -> bool| -> ((i32, i32), (i32, i32)) {
        let n = ((to.0 - from.0).abs().max((to.1 - from.1).abs())).max(1);
        let mut last = from;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (x, y) = ((from.0 as f32 + (to.0 - from.0) as f32 * t).round() as i32, (from.1 as f32 + (to.1 - from.1) as f32 * t).round() as i32);
            if sea(x, y) {
                if ok(x, y) {
                    return (last, (x, y));
                }
                break;
            }
            if walk(x, y) && inp.continent[(y * w + x) as usize] as usize == land_of + 1 {
                last = (x, y);
            }
        }
        let mut best = (from, from);
        let mut bd = f32::INFINITY;
        for y in 0..h {
            for x in 0..w {
                if !walk(x, y) || inp.continent[(y * w + x) as usize] as usize != land_of + 1 {
                    continue;
                }
                for &(ox, oy) in crate::grid::N4.iter() {
                    if sea(x + ox, y + oy) && ok(x + ox, y + oy) {
                        let dd = ((x - to.0).pow(2) + (y - to.1).pow(2)) as f32;
                        if dd < bd {
                            bd = dd;
                            best = ((x, y), (x + ox, y + oy));
                        }
                    }
                }
            }
        }
        best
    };
    let mut out = vec![GameContinent { id: "home".into(), index: home, cx: c0.cx, cy: c0.cy, rx: c0.bw as f32 * 0.5, ry: c0.bh as f32 * 0.5, village: v, harbor: None, home_harbor: None }];
    let mut harbour = pad;
    let mut used: Vec<String> = vec!["home".into()];
    for &k in others.iter() {
        let c = &inp.continents[k];
        let guess = inp.sites.iter().find(|s| s.continent == k).map(|s| (s.x, s.y)).unwrap_or((c.cx as i32, c.cy as i32));
        // Nearest walkable tile on that landmass.
        let mut cv = None;
        for r in 0..60i32 {
            for oy in -r..=r {
                for ox in -r..=r {
                    let (x, y) = (guess.0 + ox, guess.1 + oy);
                    if cv.is_none() && ox.abs().max(oy.abs()) == r && walk(x, y) && inp.continent[(y * w + x) as usize] as usize == k + 1 {
                        cv = Some((x, y));
                    }
                }
            }
            if cv.is_some() {
                break;
            }
        }
        let Some(cv) = cv else { continue };
        let home_h = coast_toward(v, cv, home, &|_, _| true);
        if !sea(home_h.1 .0, home_h.1 .1) {
            continue;
        }
        let reach = water_field(home_h.1 .0, home_h.1 .1);
        let ok = |x: i32, y: i32| reach[(y * w + x) as usize];
        let col_h = coast_toward(cv, v, k, &ok);
        if !ok(col_h.1 .0, col_h.1 .1) {
            continue; // no sea route from home
        }
        let (dx, dy) = (c.cx - v.0 as f32, c.cy - v.1 as f32);
        let mut id = if dy.abs() > dx.abs() {
            if dy < 0.0 { "north" } else { "south" }
        } else if dx > 0.0 {
            "east"
        } else {
            "west"
        }
        .to_string();
        if used.contains(&id) {
            id = format!("{id}{}", out.len());
        }
        used.push(id.clone());
        if out.len() == 1 {
            harbour = home_h.0;
        }
        out.push(GameContinent { id, index: k, cx: c.cx, cy: c.cy, rx: c.bw as f32 * 0.5, ry: c.bh as f32 * 0.5, village: cv, harbor: Some(col_h), home_harbor: Some(home_h) });
    }
    let u = |p: (i32, i32)| (p.0.clamp(0, w - 1) as usize, p.1.clamp(0, h - 1) as usize);
    let sites = CampaignSites {
        village: u(v),
        pad: u(pad),
        ridge: u(ridge),
        cave: u(cave),
        cave_mouth: u(cave_mouth),
        fire: u(v),
        factory: u(factory),
        exchange: u(exchange),
        picket: u(picket),
        hangar: u(hangar),
        crowd: u(crowd),
        harbour: u(harbour),
    };
    (sites, out)
}
