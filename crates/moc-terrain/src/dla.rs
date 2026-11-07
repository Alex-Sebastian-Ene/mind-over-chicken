//! Mountain ranges by diffusion-limited aggregation (DLA), then diffusion.
//!
//! 1. **Grow.** Clusters start from roots placed along convergent plate boundaries, on a
//!    coarse grid. Random walkers launched round the cluster wander inside the range's
//!    corridor until they touch it, and stick: the result is a branching tree, with tips that
//!    catch most walkers (so they keep branching) and a trunk that stays put.
//! 2. **Upscale step by step.** The grid doubles; every node's position doubles and every
//!    parent→child link gets a midpoint node, nudged sideways at random (jitter) so lines don't
//!    stay straight. New walkers then add finer branches at the new level.
//! 3. **Heights.** Each node gets a value from its distance to the farthest tip below it (its
//!    "reach"): the trunk is high, the spurs fall away towards their tips.
//! 4. **Diffusion.** The sharp tree is blurred in several passes on a pyramid, coarse to fine:
//!    the trunk becomes a broad massif, each branch a ridge spur, and the gaps between spurs
//!    become branching valleys, which the rivers later follow.

use crate::grid::{self, Dims};
use crate::rng::Rng;

pub const NO_PARENT: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub struct DlaParams {
    /// Pyramid levels (each one halves the grid). The finest level is the input grid.
    pub levels: u32,
    /// Share of corridor cells occupied after the coarsest level, and after the finest.
    pub fill_coarse: f32,
    pub fill_fine: f32,
    /// Walkers are launched on a ring of this radius (cells) round a random cluster node.
    pub launch: i32,
    pub max_steps: u32,
    /// Chance that a midpoint is nudged sideways when upscaling.
    pub jitter: f32,
    /// Blur passes per pyramid level.
    pub blur: usize,
}

impl Default for DlaParams {
    fn default() -> Self {
        DlaParams { levels: 4, fill_coarse: 0.3, fill_fine: 0.16, launch: 5, max_steps: 400, jitter: 0.5, blur: 2 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub parent: u32,
    pub cluster: u32,
}

#[derive(Clone, Debug)]
pub struct ClusterStats {
    pub root: (i32, i32),
    pub nodes: usize,
    /// Tips: nodes nothing else is attached to (each one ends a ridge spur).
    pub leaves: usize,
    /// Nodes where the tree forks.
    pub branch_points: usize,
    pub max_reach: u32,
}

pub struct Dla {
    pub d: Dims,
    pub nodes: Vec<Node>,
    pub clusters: Vec<ClusterStats>,
    /// The diffused ranges, 0..1, on the input grid.
    pub height: Vec<f32>,
}

fn level_dims(d: Dims, l: u32) -> Dims {
    d.sized(d.w.div_ceil(1 << l), d.h.div_ceil(1 << l))
}

/// Grow ranges in `corridor` (cells where ranges may go) from `seeds` (cells on the input grid).
pub fn grow(d: Dims, corridor: &[bool], seeds: &[(i32, i32)], p: &DlaParams, seed: u32) -> Dla {
    let levels = p.levels.max(1);
    let mut rng = Rng::stream(seed, 0xD1A);
    // Corridor pyramid: a coarse cell is in the corridor if any of its children is.
    let mut corr: Vec<Vec<bool>> = vec![corridor.to_vec()];
    for l in 1..levels {
        let (pd, cd) = (level_dims(d, l - 1), level_dims(d, l));
        let prev = &corr[l as usize - 1];
        let mut c = vec![false; cd.n()];
        for y in 0..pd.h {
            for x in 0..pd.w {
                if prev[y * pd.w + x] {
                    c[(y / 2) * cd.w + x / 2] = true;
                }
            }
        }
        corr.push(c);
    }

    let mut nodes: Vec<Node> = Vec::new();
    let top = levels - 1;
    {
        let td = level_dims(d, top);
        let mut taken = vec![false; td.n()];
        for &(sx, sy) in seeds {
            let (x, y) = ((sx >> top).clamp(0, td.w as i32 - 1), (sy >> top).clamp(0, td.h as i32 - 1));
            let i = y as usize * td.w + x as usize;
            if !taken[i] {
                taken[i] = true;
                let k = nodes.len() as u32;
                nodes.push(Node { x, y, parent: NO_PARENT, cluster: k });
            }
        }
    }
    let n_clusters = nodes.len();
    if n_clusters == 0 {
        return Dla { d, nodes, clusters: Vec::new(), height: vec![0.0; d.n()] };
    }

    for l in (0..levels).rev() {
        let ld = level_dims(d, l);
        if l != top {
            // Upscale: double positions, add a (jittered) midpoint on every link.
            for nd in nodes.iter_mut() {
                nd.x = (nd.x * 2).rem_euclid(ld.w as i32);
                nd.y = if ld.wrap_y { (nd.y * 2).rem_euclid(ld.h as i32) } else { (nd.y * 2).min(ld.h as i32 - 1) };
            }
            let count = nodes.len();
            for k in 0..count {
                let par = nodes[k].parent;
                if par == NO_PARENT {
                    continue;
                }
                let (a, b) = (nodes[k], nodes[par as usize]);
                let dx = ld.dx(a.x as f32, b.x as f32).round() as i32;
                let dy = ld.dy(a.y as f32, b.y as f32).round() as i32;
                let (mut mx, mut my) = (a.x + dx / 2, a.y + dy / 2);
                if rng.chance(p.jitter) {
                    let s = if rng.chance(0.5) { 1 } else { -1 };
                    if dx == 0 && dy != 0 {
                        mx += s;
                    } else if dy == 0 && dx != 0 {
                        my += s;
                    }
                }
                let m = nodes.len() as u32;
                let my = if ld.wrap_y { my.rem_euclid(ld.h as i32) } else { my.clamp(0, ld.h as i32 - 1) };
                nodes.push(Node { x: mx.rem_euclid(ld.w as i32), y: my, parent: par, cluster: a.cluster });
                nodes[k].parent = m;
            }
        }
        // Occupancy at this level.
        let mut occ = vec![0u32; ld.n()];
        for (k, nd) in nodes.iter().enumerate() {
            occ[nd.y as usize * ld.w + nd.x as usize] = k as u32 + 1;
        }
        let c = &corr[l as usize];
        let corr_cells = c.iter().filter(|&&b| b).count();
        let t = if top == 0 { 1.0 } else { 1.0 - l as f32 / top as f32 };
        let fill = p.fill_coarse + (p.fill_fine - p.fill_coarse) * t;
        let target = (fill * corr_cells as f32) as usize;
        let on_level = nodes.len();
        let needed = target.saturating_sub(on_level);
        let launch = p.launch.max(2);
        let mut added = 0;
        let mut attempts = 0;
        while added < needed && attempts < needed * 10 + 50 {
            attempts += 1;
            let a = nodes[rng.below(nodes.len())];
            let ang = rng.f32() * std::f32::consts::TAU;
            let mut x = (a.x + (ang.cos() * launch as f32).round() as i32).rem_euclid(ld.w as i32);
            let y0 = a.y + (ang.sin() * launch as f32).round() as i32;
            let Some(si) = ld.idx(x, y0) else { continue };
            let mut y = (si / ld.w) as i32;
            if !c[si] || occ[si] != 0 {
                continue;
            }
            let lost = (2 * launch * 2 * launch) as f32;
            for _ in 0..p.max_steps {
                // Touching the cluster? Stick to the first neighbour found (random start).
                let start = rng.below(8);
                let mut hit = 0u32;
                for q in 0..8 {
                    let (ox, oy) = grid::N8[(start + q) & 7];
                    if let Some(j) = ld.idx(x + ox, y + oy) {
                        if occ[j] != 0 {
                            hit = occ[j];
                            break;
                        }
                    }
                }
                if hit != 0 {
                    let par = hit - 1;
                    let k = nodes.len() as u32;
                    let cl = nodes[par as usize].cluster;
                    nodes.push(Node { x, y, parent: par, cluster: cl });
                    occ[y as usize * ld.w + x as usize] = k + 1;
                    added += 1;
                    break;
                }
                let (ox, oy) = grid::N8[rng.below(8)];
                if let Some(j) = ld.idx(x + ox, y + oy) {
                    if c[j] && occ[j] == 0 {
                        x = (j % ld.w) as i32;
                        y = (j / ld.w) as i32;
                    }
                }
                if ld.dist2(x as f32, y as f32, a.x as f32, a.y as f32) > lost {
                    break;
                }
            }
        }
    }

    // ---- reach (longest path to a tip) and cluster stats ----
    let n = nodes.len();
    let mut child_count = vec![0u32; n];
    for nd in nodes.iter() {
        if nd.parent != NO_PARENT {
            child_count[nd.parent as usize] += 1;
        }
    }
    let mut start = vec![0u32; n + 1];
    for k in 0..n {
        start[k + 1] = start[k] + child_count[k];
    }
    let mut fillp = start.clone();
    let mut children = vec![0u32; start[n] as usize];
    for (k, nd) in nodes.iter().enumerate() {
        if nd.parent != NO_PARENT {
            let pi = nd.parent as usize;
            children[fillp[pi] as usize] = k as u32;
            fillp[pi] += 1;
        }
    }
    let mut order: Vec<u32> = (0..n as u32).filter(|&k| nodes[k as usize].parent == NO_PARENT).collect();
    let mut head = 0;
    while head < order.len() {
        let k = order[head] as usize;
        head += 1;
        for c in start[k]..start[k + 1] {
            order.push(children[c as usize]);
        }
    }
    let mut reach = vec![1u32; n];
    for &k in order.iter().rev() {
        let nd = nodes[k as usize];
        if nd.parent != NO_PARENT {
            let pi = nd.parent as usize;
            reach[pi] = reach[pi].max(reach[k as usize] + 1);
        }
    }
    let mut clusters: Vec<ClusterStats> = (0..n_clusters)
        .map(|k| ClusterStats { root: (nodes[k].x, nodes[k].y), nodes: 0, leaves: 0, branch_points: 0, max_reach: 0 })
        .collect();
    for (k, nd) in nodes.iter().enumerate() {
        let cs = &mut clusters[nd.cluster as usize];
        cs.nodes += 1;
        if child_count[k] == 0 {
            cs.leaves += 1;
        }
        if child_count[k] >= 2 {
            cs.branch_points += 1;
        }
        cs.max_reach = cs.max_reach.max(reach[k]);
    }

    // ---- crisp image, then diffusion on a pyramid (coarse to fine) ----
    let mut crisp = vec![0.0f32; d.n()];
    for (k, nd) in nodes.iter().enumerate() {
        let v = 1.0 - 1.0 / (1.0 + reach[k] as f32 / 6.0);
        let i = nd.y as usize * d.w + nd.x as usize;
        crisp[i] = crisp[i].max(v);
    }
    let mut pyr: Vec<(Dims, Vec<f32>)> = vec![(d, crisp)];
    for l in 1..levels {
        let (pd, prev) = (&pyr[l as usize - 1].0, &pyr[l as usize - 1].1);
        let cd = level_dims(d, l);
        let mut c = vec![0.0f32; cd.n()];
        for y in 0..pd.h {
            for x in 0..pd.w {
                let j = (y / 2) * cd.w + x / 2;
                c[j] = c[j].max(prev[y * pd.w + x]);
            }
        }
        pyr.push((cd, c));
    }
    let (mut ad, mut acc) = pyr[top as usize].clone();
    grid::blur(ad, &mut acc, p.blur);
    for l in (0..top).rev() {
        let (ld, ref crisp_l) = pyr[l as usize];
        let mut up = grid::resample(ad, &acc, ld);
        for (u, c) in up.iter_mut().zip(crisp_l.iter()) {
            *u += c;
        }
        // The finest level keeps its spurs crisp; coarser levels diffuse more.
        grid::blur(ld, &mut up, if l == 0 { p.blur.min(1) } else { p.blur });
        ad = ld;
        acc = up;
    }
    // Normalise to the highest crest, then sharpen so crests are lines, not plateaus.
    let top = acc.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
    for v in acc.iter_mut() {
        *v = (*v / top).powf(1.35);
    }
    Dla { d, nodes, clusters, height: acc }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(d: Dims) -> Vec<bool> {
        (0..d.n()).map(|i| {
            let (x, y) = d.xy(i);
            (y - (d.h as i32 / 2 + ((x as f32) * 0.05).sin() as i32 * 3)).abs() < 10
        }).collect()
    }

    #[test]
    fn each_run_grows_different_ranges() {
        // Maps are random each run (Alex), so only check that seeds give different ranges.
        let d = Dims::new(160, 80, true);
        let c = band(d);
        let a = grow(d, &c, &[(20, 40), (90, 40)], &DlaParams::default(), 9);
        let e = grow(d, &c, &[(20, 40), (90, 40)], &DlaParams::default(), 10);
        assert_ne!(a.height, e.height);
    }

    #[test]
    fn ranges_branch() {
        let d = Dims::new(160, 80, true);
        let c = band(d);
        let dla = grow(d, &c, &[(20, 40), (90, 40)], &DlaParams::default(), 4);
        for cs in dla.clusters.iter() {
            assert!(cs.leaves >= 3 && cs.branch_points >= 2, "{cs:?}");
        }
        assert!(dla.height.iter().all(|v| (0.0..=1.0).contains(v)));
        // Heights stay inside (or right by) the corridor.
        let dist = grid::distance(d, &c);
        for i in 0..d.n() {
            if dla.height[i] > 0.2 {
                assert!(dist[i] < 12.0);
            }
        }
    }
}
