//! The large scale, on a coarse "growth" grid (~13k cells whatever the map size):
//!
//! - **Continents** grow from scattered cores by a noisy, anisotropic flood (cheapest cells
//!   first), each until it reaches its target area. A continent never comes within `gap` of
//!   another, so oceans always separate them; the noise gives bays, peninsulas and elongated,
//!   irregular shapes.
//! - **Plates** are a domain-warped Voronoi diagram with a random motion per plate. Where two
//!   plates move towards each other the boundary is convergent: mountain ranges rise there on
//!   land (and island arcs at sea); where they move apart it is divergent (rifts).

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::grid::{self, Dims, N8, SQRT2};
use crate::noise::{coarse_field, Fbm};
use crate::rng::Rng;

pub const OCEAN: u8 = 255;

#[derive(Clone, Copy, Debug)]
pub struct Plate {
    pub cx: f32,
    pub cy: f32,
    pub vx: f32,
    pub vy: f32,
}

/// Where and how big the continents should be.
#[derive(Clone, Debug)]
pub struct ContinentPlan {
    /// Target land area of each continent, in tiles.
    pub areas: Vec<f32>,
    /// Pin continent 0's core at (x, y) tiles, optionally confined to a box (x0, y0, x1, y1).
    pub anchor: Option<(f32, f32, Option<[f32; 4]>)>,
    /// Ocean kept between continents, in tiles.
    pub gap: f32,
    /// Ocean kept along the east–west seam (x = 0), in tiles (0 = none).
    pub seam_margin: f32,
    /// No continent cells where the climate's coldness (0 in a warm belt, 1 in the middle of
    /// a cold belt) is above this.
    pub cold_limit: f32,
    /// Warm belts per map height (the climate is periodic north–south, #111).
    pub bands: f32,
    /// How irregular the shapes are (0 = round blobs).
    pub roughness: f32,
    /// Clusters the continents bunch into (0: one to three, at random).
    pub clusters: usize,
}

pub struct Tectonics {
    pub gd: Dims,
    /// Tiles per growth cell, x and y.
    pub cell: (f32, f32),
    pub plate: Vec<u16>,
    pub plates: Vec<Plate>,
    /// Signed convergence at plate-boundary cells (> 0 converging), 0 elsewhere.
    pub conv: Vec<f32>,
    pub boundary: Vec<bool>,
    /// Continent per growth cell (`OCEAN` = none).
    pub owner: Vec<u8>,
    /// Continent cores, growth-grid coordinates.
    pub cores: Vec<(f32, f32)>,
}

#[derive(PartialEq)]
struct Item {
    cost: f32,
    cell: u32,
    cont: u8,
}
impl Eq for Item {}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        // Min-heap on cost, then cell, then continent: fully deterministic.
        o.cost.total_cmp(&self.cost).then(o.cell.cmp(&self.cell)).then(o.cont.cmp(&self.cont))
    }
}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// The growth grid for a `w`×`h` map: about `target` cells, square-ish cells.
pub fn growth_dims(w: usize, h: usize, wrap: bool, target: usize) -> Dims {
    let cell = ((w * h) as f32 / target as f32).sqrt().max(1.0);
    let gw = ((w as f32 / cell).round() as usize).max(8);
    let gh = ((h as f32 / cell).round() as usize).max(6);
    Dims::new(gw, gh, wrap)
}

pub fn build(map_w: usize, map_h: usize, gd: Dims, plan: &ContinentPlan, plates: usize, seed: u32) -> Tectonics {
    let cell = (map_w as f32 / gd.w as f32, map_h as f32 / gd.h as f32);
    let md = Dims::new(map_w, map_h, gd.wrap_x);
    let mut rng = Rng::stream(seed, 0x7EC7);

    // ---- plates: best-candidate seeds, warped Voronoi ----
    let area = (map_w * map_h) as f32;
    let spacing = (area / plates.max(1) as f32).sqrt();
    let mut seeds: Vec<Plate> = Vec::new();
    for _ in 0..plates.max(1) {
        let mut best = (0.0f32, 0.0f32, -1.0f32);
        for _ in 0..24 {
            let (x, y) = (rng.range(0.0, map_w as f32), rng.range(0.0, map_h as f32));
            let dmin = seeds.iter().map(|p| md.dist2(p.cx, p.cy, x, y)).fold(f32::INFINITY, f32::min);
            if dmin > best.2 {
                best = (x, y, dmin);
            }
        }
        let a = rng.range(0.0, std::f32::consts::TAU);
        let s = rng.range(0.35, 1.0);
        seeds.push(Plate { cx: best.0, cy: best.1, vx: a.cos() * s, vy: a.sin() * s });
    }
    let wx = Fbm::new(&mut rng, 1.0 / (spacing * 0.9), 3, 0.5);
    let wy = Fbm::new(&mut rng, 1.0 / (spacing * 0.9), 3, 0.5);
    let warp_amp = spacing * 0.32;
    let mut plate = vec![0u16; gd.n()];
    let wxf = coarse_field(gd, map_w, map_h, 2, |p| wx.sample(p) * warp_amp);
    let wyf = coarse_field(gd, map_w, map_h, 2, |p| wy.sample(p) * warp_amp);
    for gy in 0..gd.h {
        for gx in 0..gd.w {
            let x = (gx as f32 + 0.5) * cell.0 + wxf[gy * gd.w + gx];
            let y = (gy as f32 + 0.5) * cell.1 + wyf[gy * gd.w + gx];
            let mut best = (0usize, f32::INFINITY);
            for (k, s) in seeds.iter().enumerate() {
                let d = md.dist2(s.cx, s.cy, x, y);
                if d < best.1 {
                    best = (k, d);
                }
            }
            plate[gy * gd.w + gx] = best.0 as u16;
        }
    }
    let mut conv = vec![0.0f32; gd.n()];
    let mut boundary = vec![false; gd.n()];
    for i in 0..gd.n() {
        let (x, y) = gd.xy(i);
        let a = plate[i] as usize;
        let mut best = 0.0f32;
        for &(ox, oy) in [(1, 0), (-1, 0), (0, 1), (0, -1)].iter() {
            if let Some(j) = gd.idx(x + ox, y + oy) {
                let b = plate[j] as usize;
                if a != b {
                    boundary[i] = true;
                    let (nx, ny) = (ox as f32, oy as f32);
                    let c = ((seeds[a].vx - seeds[b].vx) * nx + (seeds[a].vy - seeds[b].vy) * ny) * 0.5;
                    if c.abs() > best.abs() {
                        best = c;
                    }
                }
            }
        }
        conv[i] = best;
    }

    // ---- continents: noisy anisotropic growth from cores ----
    let n = plan.areas.len().min(250);
    let rough = Fbm::new(&mut rng, 1.0 / (spacing * 0.55), 4, 0.55);
    let cost_noise: Vec<f32> = coarse_field(gd, map_w, map_h, 2, |p| (rough.sample(p) * 0.5 + 0.5).clamp(0.0, 1.0));
    let lat_of = |gy: usize| coldness((gy as f32 + 0.5) / gd.h as f32, plan.bands);
    let allowed = |i: usize| -> bool {
        let (x, y) = gd.xy(i);
        if lat_of(y as usize) > plan.cold_limit {
            return false;
        }
        if plan.seam_margin > 0.0 {
            let (xt, yt) = ((x as f32 + 0.5) * cell.0, (y as f32 + 0.5) * cell.1);
            let m = plan.seam_margin;
            if xt < m || xt > map_w as f32 - m || yt < m * 0.6 || yt > map_h as f32 - m * 0.6 {
                return false;
            }
        }
        true
    };
    // Cores: continents bunch round one to three cluster centres (some nearly touching across
    // narrow straits) and leave wide open oceans elsewhere; no even spacing (Alex, #109).
    let mut cores: Vec<(f32, f32)> = Vec::new();
    // Elongated, at 1.8–3.6 : 1; continents in one cluster lie roughly parallel (like
    // neighbouring plates), so they can sit close side by side.
    let aspect = |r: &mut Rng, base: f32| (base + r.range(-0.45, 0.45), r.range(1.8, 3.6));
    let mut shape: Vec<(f32, f32)> = Vec::new();
    let radius = |k: usize| (plan.areas[k] / (cell.0 * cell.1) / std::f32::consts::PI).sqrt();
    let mean_r = (0..n).map(radius).sum::<f32>() / n.max(1) as f32;
    let ok_core = |gx: f32, gy: f32| {
        let i = (gy as usize).min(gd.h - 1) * gd.w + (gx as usize).min(gd.w - 1);
        allowed(i) && lat_of(gy as usize) <= plan.cold_limit * 0.9
    };
    let wrap_pt = |gx: f32, gy: f32| {
        let x = if gd.wrap_x { gx.rem_euclid(gd.w as f32) } else { gx.clamp(0.0, gd.w as f32 - 1.0) };
        let y = if gd.wrap_y { gy.rem_euclid(gd.h as f32) } else { gy.clamp(0.0, gd.h as f32 - 1.0) };
        (x, y)
    };
    let n_clusters = if plan.clusters > 0 { plan.clusters } else { (1 + (n >= 5 && rng.chance(0.4)) as usize).min(n.max(1)) };
    let mut centres: Vec<(f32, f32)> = Vec::new();
    for k in 0..n_clusters {
        if k == 0 {
            if let Some((ax, ay, _)) = plan.anchor {
                centres.push((ax / cell.0, ay / cell.1));
                continue;
            }
        }
        let mut best = (gd.w as f32 * 0.5, gd.h as f32 * 0.5, f32::NEG_INFINITY);
        for _ in 0..40 {
            let (gx, gy) = (rng.range(0.0, gd.w as f32), rng.range(0.0, gd.h as f32));
            if !ok_core(gx, gy) {
                continue;
            }
            let d = centres.iter().map(|&(cx, cy)| gd.dist2(cx, cy, gx, gy)).fold(f32::INFINITY, f32::min);
            if d > best.2 {
                best = (gx, gy, d);
            }
        }
        centres.push((best.0, best.1));
    }
    let cluster_dir: Vec<f32> = (0..n_clusters).map(|_| rng.range(0.0, std::f32::consts::PI)).collect();
    let gauss = |r: &mut Rng| {
        let (u, v) = (r.range(1e-6, 1.0), r.f32());
        (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
    };
    for c in 0..n {
        if c == 0 {
            if let Some((ax, ay, _)) = plan.anchor {
                cores.push((ax / cell.0, ay / cell.1));
                shape.push(aspect(&mut rng, cluster_dir[0]));
                continue;
            }
        }
        let cl = if c < n_clusters { c } else { rng.below(n_clusters) };
        let sigma = (radius(c) + mean_r) * 0.85;
        let mut placed = None;
        for _ in 0..80 {
            let (gx, gy) = wrap_pt(centres[cl].0 + gauss(&mut rng) * sigma, centres[cl].1 + gauss(&mut rng) * sigma * 0.8);
            if !ok_core(gx, gy) {
                continue;
            }
            if cores.iter().enumerate().all(|(k, &(cx, cy))| gd.dist2(cx, cy, gx, gy).sqrt() >= 0.7 * (radius(c) + radius(k))) {
                placed = Some((gx, gy));
                break;
            }
        }
        let p = placed.unwrap_or_else(|| {
            // Crowded: the most open allowed spot.
            let mut best = (rng.range(0.0, gd.w as f32), gd.h as f32 * 0.5, f32::NEG_INFINITY);
            for _ in 0..60 {
                let (gx, gy) = (rng.range(0.0, gd.w as f32), rng.range(0.0, gd.h as f32));
                if !ok_core(gx, gy) {
                    continue;
                }
                let d = cores.iter().enumerate().map(|(k, &(cx, cy))| gd.dist2(cx, cy, gx, gy).sqrt() / (radius(k) + radius(c))).fold(f32::INFINITY, f32::min);
                if d > best.2 {
                    best = (gx, gy, d);
                }
            }
            (best.0, best.1)
        });
        cores.push(p);
        shape.push(aspect(&mut rng, cluster_dir[cl]));
    }

    let mut owner = vec![OCEAN; gd.n()];
    // near[i]: the continent claiming cells within `gap` of i (OCEAN = none, 254 = several).
    let mut near = vec![OCEAN; gd.n()];
    let gap_cells = (plan.gap / cell.0.min(cell.1)).ceil().max(1.0) as i32;
    let mut disc: Vec<(i32, i32)> = Vec::new();
    for oy in -gap_cells..=gap_cells {
        for ox in -gap_cells..=gap_cells {
            if ox * ox + oy * oy <= gap_cells * gap_cells {
                disc.push((ox, oy));
            }
        }
    }
    let targets: Vec<usize> = plan.areas.iter().take(n).map(|&a| (a / (cell.0 * cell.1)).round().max(4.0) as usize).collect();
    let mut grown = vec![0usize; n];
    let mut heap = BinaryHeap::new();
    let mut best_cost = vec![f32::INFINITY; gd.n()];
    let in_box = |c: usize, i: usize| -> bool {
        if c != 0 {
            return true;
        }
        match plan.anchor {
            Some((_, _, Some(b))) => {
                let (x, y) = gd.xy(i);
                let (xt, yt) = ((x as f32 + 0.5) * cell.0, (y as f32 + 0.5) * cell.1);
                xt >= b[0] && xt <= b[2] && yt >= b[1] && yt <= b[3]
            }
            _ => true,
        }
    };
    for (c, &(cx, cy)) in cores.iter().enumerate() {
        let i = (cy as usize).min(gd.h - 1) * gd.w + (cx as usize).min(gd.w - 1);
        heap.push(Item { cost: 0.0, cell: i as u32, cont: c as u8 });
    }
    while let Some(Item { cost, cell: ci, cont }) = heap.pop() {
        let (i, c) = (ci as usize, cont as usize);
        if owner[i] != OCEAN || grown[c] >= targets[c] {
            continue;
        }
        if !(near[i] == OCEAN || near[i] == cont) || !allowed(i) || !in_box(c, i) {
            continue;
        }
        owner[i] = cont;
        grown[c] += 1;
        let (x, y) = gd.xy(i);
        for &(ox, oy) in disc.iter() {
            if let Some(j) = gd.idx(x + ox, y + oy) {
                if near[j] == OCEAN {
                    near[j] = cont;
                } else if near[j] != cont {
                    near[j] = 254;
                }
            }
        }
        let (phi, elong) = shape[c];
        let (cp, sp) = (phi.cos(), phi.sin());
        for (k, &(ox, oy)) in N8.iter().enumerate() {
            if let Some(j) = gd.idx(x + ox, y + oy) {
                if owner[j] != OCEAN {
                    continue;
                }
                let step = if k == 0 || k == 2 || k == 5 || k == 7 { SQRT2 } else { 1.0 };
                let (dx, dy) = (ox as f32 * cell.0, oy as f32 * cell.1);
                let len = (dx * dx + dy * dy).sqrt();
                let (u, v) = ((dx * cp + dy * sp) / len, (-dx * sp + dy * cp) / len);
                let aniso = (u * u / elong + v * v * elong).sqrt();
                let (_, jy) = gd.xy(j);
                let lat = lat_of(jy as usize);
                let lat_pen = 1.0 + 30.0 * (lat - plan.cold_limit * 0.8).max(0.0).powi(2);
                let cn = cost_noise[j];
                // Cheap fingers grow peninsulas, dear pockets stay bays and gulfs.
                let rough_mul = 0.45 + plan.roughness * 8.0 * cn * cn * cn;
                let nc = cost + step * aniso * rough_mul * lat_pen;
                if nc < best_cost[j] || owner[j] == OCEAN {
                    best_cost[j] = best_cost[j].min(nc);
                    heap.push(Item { cost: nc, cell: j as u32, cont });
                }
            }
        }
    }
    // Fill tiny enclosed ocean holes so continents have no pinholes (real lakes come later).
    let ocean: Vec<bool> = owner.iter().map(|&o| o == OCEAN).collect();
    let (lab, sizes) = grid::components(gd, &ocean);
    for i in 0..gd.n() {
        if lab[i] != 0 && sizes[lab[i] as usize - 1] <= 3 {
            let (x, y) = gd.xy(i);
            for &(ox, oy) in N8.iter() {
                if let Some(j) = gd.idx(x + ox, y + oy) {
                    if owner[j] != OCEAN {
                        owner[i] = owner[j];
                        break;
                    }
                }
            }
        }
    }

    Tectonics { gd, cell, plate, plates: seeds, conv, boundary, owner, cores }
}

/// How cold a row is, 0 (middle of a warm belt) to 1 (middle of a cold belt), for a row at
/// fraction `v` of the map height. Periodic: the cold belt along the top edge continues along
/// the bottom edge (#111). It is the distance from the cold line as a fraction of the way to
/// the warm line (like latitude / 90°), rounded off at both ends so it stays smooth.
#[inline]
pub fn coldness(v: f32, bands: f32) -> f32 {
    const K: f32 = 0.985;
    let r0 = K.acos() / std::f32::consts::PI;
    let r = (K * (std::f32::consts::TAU * bands * v).cos()).acos() / std::f32::consts::PI;
    1.0 - ((r - r0) / (1.0 - 2.0 * r0)).clamp(0.0, 1.0)
}
