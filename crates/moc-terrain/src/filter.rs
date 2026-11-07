//! The filters that make the map read as clean, chunky pixel art: the 2×2 majority filter
//! (`despeckle`) and the coastline cleaner (`clean`).
//!
//! Clean coastlines (Alex, #109: "terrain is connecting weirdly and islands are too small").
//!
//! On the final land/water mask:
//! 1. a 5×5 majority vote smooths the coast at the pixel scale (no stair-step jaggies, no
//!    1–2 tile slivers of land or water);
//! 2. an opening with a 3×3 square removes land threads thinner than 3 tiles (every land tile
//!    ends up inside a 3×3 block of land);
//! 3. diagonal-only joins (a 2×2 checkerboard of land and water) are broken by sinking the
//!    lower land tile, so land and water connect only edge to edge;
//! 4. water slivers (water not in any 2×2 of water) are filled;
//! 5. landmasses under `min_island` tiles are removed and enclosed water under `min_water`
//!    tiles is filled.
//!
//! Everything except the component passes runs only in a band round the coast.

use crate::grid::{self, Dims};

/// Does tile (x, y) (index `i`) sit in at least one 2×2 block of its own class?
#[inline]
fn in_block_xy<T: Copy + Eq>(d: Dims, c: &[T], x: i32, y: i32, i: usize) -> bool {
    let v = c[i];
    if x > 0 && x < d.w as i32 - 1 && y > 0 && y < d.h as i32 - 1 {
        let w = d.w;
        let (l, r, u, dn) = (c[i - 1] == v, c[i + 1] == v, c[i - w] == v, c[i + w] == v);
        return (l && u && c[i - w - 1] == v) || (r && u && c[i - w + 1] == v) || (l && dn && c[i + w - 1] == v) || (r && dn && c[i + w + 1] == v);
    }
    let same = |ox: i32, oy: i32| d.idx(x + ox, y + oy).map(|j| c[j] == v).unwrap_or(false);
    let l = same(-1, 0);
    let r = same(1, 0);
    let u = same(0, -1);
    let dn = same(0, 1);
    (l && u && same(-1, -1)) || (r && u && same(1, -1)) || (l && dn && same(-1, 1)) || (r && dn && same(1, 1))
}

#[inline]
fn in_block<T: Copy + Eq>(d: Dims, c: &[T], i: usize) -> bool {
    let (x, y) = d.xy(i);
    in_block_xy(d, c, x, y, i)
}

/// The majority filter. Every tile ends up in at least one 2×2 block of its own class, so the
/// smallest feature on the map is 2×2 (no single-tile specks or 1-wide slivers). A tile that
/// isn't takes the most common class among its 8 neighbours (`allowed` filters the candidates,
/// e.g. "same biome"). Returns how many tiles changed.
pub fn despeckle<T: Copy + Eq>(d: Dims, c: &mut [T], allowed: impl Fn(usize, T) -> bool, max_passes: usize) -> usize {
    let n = d.n();
    let mut changed_total = 0;
    let mut work: Vec<usize> = Vec::new();
    for y in 0..d.h {
        for x in 0..d.w {
            let i = y * d.w + x;
            if !in_block_xy(d, c, x as i32, y as i32, i) {
                work.push(i);
            }
        }
    }
    let mut mark = vec![false; n];
    for _ in 0..max_passes {
        if work.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for &i in work.iter() {
            if in_block(d, c, i) {
                continue;
            }
            // Count neighbour classes.
            let mut cand: [(Option<T>, u8); 8] = [(None, 0); 8];
            let mut k = 0;
            d.nb8(i, |j, _| {
                let v = c[j];
                if !allowed(i, v) {
                    return;
                }
                for slot in cand.iter_mut().take(k) {
                    if slot.0 == Some(v) {
                        slot.1 += 1;
                        return;
                    }
                }
                cand[k] = (Some(v), 1);
                k += 1;
            });
            if k == 0 {
                continue;
            }
            let old = c[i];
            // Most common; ties go to the class that makes a 2×2 block here.
            let mut best: Option<T> = None;
            let mut best_score = -1i32;
            for slot in cand.iter().take(k) {
                let v = slot.0.unwrap();
                c[i] = v;
                let score = slot.1 as i32 * 4 + if in_block(d, c, i) { 2 } else { 0 } + if v == old { 1 } else { 0 };
                if score > best_score {
                    best_score = score;
                    best = Some(v);
                }
            }
            let v = best.unwrap();
            c[i] = v;
            if v != old {
                changed_total += 1;
                let (x, y) = d.xy(i);
                for oy in -1..=1 {
                    for ox in -1..=1 {
                        if let Some(j) = d.idx(x + ox, y + oy) {
                            if !mark[j] {
                                mark[j] = true;
                                next.push(j);
                            }
                        }
                    }
                }
            } else if !in_block(d, c, i) && !mark[i] {
                mark[i] = true;
                next.push(i);
            }
        }
        for &j in next.iter() {
            mark[j] = false;
        }
        next.retain(|&j| !in_block(d, c, j));
        next.sort_unstable();
        next.dedup();
        work = next;
    }
    changed_total
}

/// How many tiles are not part of any 2×2 block of their own class (ignoring `skip`).
pub fn specks<T: Copy + Eq>(d: Dims, c: &[T], skip: impl Fn(usize) -> bool) -> usize {
    let mut k = 0;
    for y in 0..d.h {
        for x in 0..d.w {
            let i = y * d.w + x;
            if !skip(i) && !in_block_xy(d, c, x as i32, y as i32, i) {
                k += 1;
            }
        }
    }
    k
}


/// Counts of what changed, for stats and tests.
#[derive(Clone, Debug, Default)]
pub struct CleanStats {
    pub smoothed: usize,
    pub threads: usize,
    pub diagonals: usize,
    pub slivers: usize,
    pub islands_removed: usize,
    pub holes_filled: usize,
}

fn coast_band(d: Dims, land: &[bool], r: i32) -> (Vec<usize>, Vec<bool>) {
    let n = d.n();
    let mut band = vec![false; n];
    let mark = |band: &mut Vec<bool>, x: i32, y: i32| {
        for oy in -r..=r {
            for ox in -r..=r {
                if let Some(j) = d.idx(x + ox, y + oy) {
                    band[j] = true;
                }
            }
        }
    };
    // A coast edge: a tile whose right or lower neighbour is the other kind.
    for y in 0..d.h as i32 {
        let row = y as usize * d.w;
        let down = d.idx(0, y + 1).map(|j| j - (j % d.w));
        for x in 0..d.w as i32 {
            let i = row + x as usize;
            let right = if (x as usize) + 1 < d.w { Some(i + 1) } else { d.idx(x + 1, y) };
            let below = down.map(|r0| r0 + x as usize);
            let diff = right.map(|j| land[j] != land[i]).unwrap_or(false) || below.map(|j| land[j] != land[i]).unwrap_or(false);
            if diff {
                mark(&mut band, x, y);
            }
        }
    }
    let list = (0..n).filter(|&i| band[i]).collect();
    (list, band)
}

#[inline]
fn all_land_3x3(d: Dims, land: &[bool], x: i32, y: i32) -> bool {
    for oy in -1..=1 {
        for ox in -1..=1 {
            match d.idx(x + ox, y + oy) {
                Some(j) if land[j] => {}
                _ => return false,
            }
        }
    }
    true
}

/// Is land tile (x, y) inside some 3×3 block of land?
#[inline]
pub fn in_land_block(d: Dims, land: &[bool], x: i32, y: i32) -> bool {
    for cy in -1..=1 {
        for cx in -1..=1 {
            if all_land_3x3(d, land, x + cx, y + cy) {
                return true;
            }
        }
    }
    false
}

/// Is (x, y) the top-left of a 2×2 checkerboard of land and water?
#[inline]
pub fn checker_at(d: Dims, land: &[bool], x: i32, y: i32) -> Option<(usize, usize, usize, usize)> {
    let a = d.idx(x, y)?;
    let b = d.idx(x + 1, y)?;
    let c = d.idx(x, y + 1)?;
    let e = d.idx(x + 1, y + 1)?;
    if land[a] == land[e] && land[b] == land[c] && land[a] != land[b] {
        Some((a, b, c, e))
    } else {
        None
    }
}

#[inline]
fn water_in_block(d: Dims, land: &[bool], x: i32, y: i32) -> bool {
    let w = |ox: i32, oy: i32| d.idx(x + ox, y + oy).map(|j| !land[j]).unwrap_or(false);
    let (l, r, u, dn) = (w(-1, 0), w(1, 0), w(0, -1), w(0, 1));
    (l && u && w(-1, -1)) || (r && u && w(1, -1)) || (l && dn && w(-1, 1)) || (r && dn && w(1, 1))
}

/// Clean `land` in place (`height` decides which tile of a diagonal join sinks).
pub fn clean(d: Dims, land: &mut [bool], height: &[f32], min_island: usize, min_water: usize) -> CleanStats {
    let mut st = CleanStats::default();
    let (band, _) = coast_band(d, land, 4);
    // 1. 5×5 majority.
    {
        let old = land.to_vec();
        for &i in band.iter() {
            let (x, y) = d.xy(i);
            let mut c = 0;
            let mut t = 0;
            for oy in -2..=2 {
                for ox in -2..=2 {
                    if let Some(j) = d.idx(x + ox, y + oy) {
                        t += 1;
                        if old[j] {
                            c += 1;
                        }
                    }
                }
            }
            let v = c * 2 > t;
            if v != land[i] {
                land[i] = v;
                st.smoothed += 1;
            }
        }
    }
    let (band, in_band) = coast_band(d, land, 4);
    let mut eroded = vec![false; d.n()];
    for round in 0..6 {
        let mut changed = 0;
        // 2. opening: keep land only inside a 3×3 block of land. (`eroded` is only computed
        // inside the band; outside it every tile is surrounded by its own kind.)
        for &i in band.iter() {
            let (x, y) = d.xy(i);
            eroded[i] = land[i] && all_land_3x3(d, land, x, y);
        }
        for &i in band.iter() {
            if !land[i] {
                continue;
            }
            let (x, y) = d.xy(i);
            let mut keep = false;
            'o: for oy in -1..=1 {
                for ox in -1..=1 {
                    if let Some(j) = d.idx(x + ox, y + oy) {
                        // Tiles outside the band are deep inside land or water: eroded == land.
                        let e = if in_band[j] { eroded[j] } else { land[j] };
                        if e {
                            keep = true;
                            break 'o;
                        }
                    }
                }
            }
            if !keep {
                land[i] = false;
                st.threads += 1;
                changed += 1;
            }
        }
        // 3. diagonal-only joins: sink the lower land tile.
        for &i in band.iter() {
            let (x, y) = d.xy(i);
            if let Some((a, b, c, e)) = checker_at(d, land, x, y) {
                let (p, q) = if land[a] { (a, e) } else { (b, c) };
                let sink = if height[p] < height[q] || (height[p] == height[q] && p < q) { p } else { q };
                land[sink] = false;
                st.diagonals += 1;
                changed += 1;
            }
        }
        if round == 5 {
            break;
        }
        // 4. water slivers: fill water that isn't in any 2×2 of water.
        for &i in band.iter() {
            if land[i] {
                continue;
            }
            let (x, y) = d.xy(i);
            if !water_in_block(d, land, x, y) {
                land[i] = true;
                st.slivers += 1;
                changed += 1;
            }
        }
        if changed == 0 {
            break;
        }
    }
    // 5. small landmasses and enclosed water.
    let (lab, sizes) = grid::components(d, land);
    for i in 0..d.n() {
        if lab[i] != 0 && sizes[lab[i] as usize - 1] < min_island {
            land[i] = false;
        }
    }
    st.islands_removed = sizes.iter().filter(|&&s| s < min_island).count();
    let water: Vec<bool> = land.iter().map(|&l| !l).collect();
    let (lab, sizes) = grid::components(d, &water);
    for i in 0..d.n() {
        if lab[i] != 0 && sizes[lab[i] as usize - 1] < min_water {
            land[i] = true;
        }
    }
    st.holes_filled = sizes.iter().filter(|&&s| s < min_water).count();
    st
}

/// Checks for tests: land tiles not inside a 3×3 land block, 2×2 checkerboards, and water
/// tiles not inside a 2×2 water block.
pub fn defects(d: Dims, land: &[bool]) -> (usize, usize, usize) {
    let mut threads = 0;
    let mut checkers = 0;
    let mut slivers = 0;
    for i in 0..d.n() {
        let (x, y) = d.xy(i);
        if land[i] && !in_land_block(d, land, x, y) {
            threads += 1;
        }
        if !land[i] && !water_in_block(d, land, x, y) {
            slivers += 1;
        }
        if checker_at(d, land, x, y).is_some() {
            checkers += 1;
        }
    }
    (threads, checkers, slivers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn despeckle_leaves_only_2x2_features() {
        let d = Dims::new(40, 30, true);
        let mut c: Vec<u8> = (0..d.n()).map(|i| ((i * 7919) % 13 % 3) as u8).collect();
        assert!(specks(d, &c, |_| false) > 0);
        despeckle(d, &mut c, |_, _| true, 30);
        assert_eq!(specks(d, &c, |_| false), 0);
    }

    #[test]
    fn cleans_threads_diagonals_and_specks() {
        let d = Dims::torus(60, 40);
        let mut land = vec![false; d.n()];
        let mut set = |x: i32, y: i32| land[d.idx(x, y).unwrap()] = true;
        // Two blobs joined by a 1-tile thread, a diagonal touch, and a speck.
        for y in 10..30 {
            for x in 5..20 {
                set(x, y);
            }
            for x in 35..55 {
                set(x, y);
            }
        }
        for x in 20..35 {
            set(x, 20);
        }
        set(20, 9);
        set(45, 2);
        let h = vec![0.5f32; d.n()];
        clean(d, &mut land, &h, 30, 6);
        assert_eq!(defects(d, &land), (0, 0, 0));
        assert!(!land[d.idx(27, 20).unwrap()], "the thread is gone");
        assert!(!land[d.idx(45, 2).unwrap()], "the speck is gone");
        let (_, sizes) = grid::components(d, &land);
        assert_eq!(sizes.len(), 2);
    }
}
