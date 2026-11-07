//! Water: priority-flood drainage (Barnes et al.), flow accumulation, lakes and rivers.
//!
//! The flood starts at every sea cell on a coast and works uphill, always taking the lowest
//! cell next. Each land cell drains to the cell that reached it first (its receiver), and a
//! depression is crossed at its spill point, so every drop reaches the sea: rivers end in the
//! sea or flow through a lake that spills towards it. Cells come out of the flood in an order
//! where every receiver precedes the cells draining into it, which makes accumulation and
//! implicit stream-power erosion single passes. Works on the torus like everything else, so a
//! river can cross a seam.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::grid::{Dims, N8, SQRT2};

pub const NONE: u32 = u32::MAX;

pub struct Drainage {
    /// Downstream neighbour of each land cell (`NONE` for sea cells and sinks).
    pub recv: Vec<u32>,
    /// Flood order: sea outlets first, then land, each cell after its receiver.
    pub order: Vec<u32>,
    /// Depression-filled heights (a tiny slope added across flats so everything drains).
    pub filled: Vec<f32>,
}

#[derive(PartialEq)]
struct Item(f32, u32);
impl Eq for Item {}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Route water over `h` (sea where `h < sea`).
pub fn route(d: Dims, h: &[f32], sea: f32, eps: f32) -> Drainage {
    let n = d.n();
    let mut recv = vec![NONE; n];
    let mut filled = h.to_vec();
    let mut seen = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        if h[i] < sea {
            seen[i] = true;
            let mut coast = false;
            d.nb8(i, |j, _| coast |= h[j] >= sea);
            if coast {
                heap.push(Item(h[i], i as u32));
            }
        }
    }
    let mut next_sink = 0usize;
    loop {
        while let Some(Item(_, ci)) = heap.pop() {
            let c = ci as usize;
            order.push(ci);
            let fc = filled[c];
            let (x, y) = d.xy(c);
            for (k, &(ox, oy)) in N8.iter().enumerate() {
                let Some(j) = d.idx(x + ox, y + oy) else { continue };
                if seen[j] {
                    continue;
                }
                seen[j] = true;
                let step = if k == 0 || k == 2 || k == 5 || k == 7 { SQRT2 } else { 1.0 };
                filled[j] = h[j].max(fc + eps * step);
                recv[j] = ci;
                heap.push(Item(filled[j], j as u32));
            }
        }
        // Land with no sea at all drains to its lowest cell (an inland sink).
        while next_sink < n && seen[next_sink] {
            next_sink += 1;
        }
        if next_sink >= n {
            break;
        }
        let mut lo = next_sink;
        for i in next_sink..n {
            if !seen[i] && h[i] < h[lo] {
                lo = i;
            }
        }
        seen[lo] = true;
        heap.push(Item(h[lo], lo as u32));
    }
    Drainage { recv, order, filled }
}

/// Flow accumulation: each cell's own `weight` plus everything upstream of it.
pub fn accumulate(dr: &Drainage, weight: &[f32]) -> Vec<f32> {
    let mut acc = weight.to_vec();
    for &i in dr.order.iter().rev() {
        let r = dr.recv[i as usize];
        if r != NONE {
            acc[r as usize] += acc[i as usize];
        }
    }
    acc
}

/// Lakes: depressions (filled above the ground) at least `min_area` cells big and
/// `min_depth` deep, at most `max_lakes` of them (the biggest). Other pits are filled in (`h`
/// raised to the flood level), as sediment would. Returns the lake mask; lake cells keep the
/// ground height.
pub fn lakes(d: Dims, h: &mut [f32], filled: &[f32], sea: f32, min_area: usize, min_depth: f32, max_lakes: usize) -> Vec<bool> {
    let n = d.n();
    let wet: Vec<bool> = (0..n).map(|i| h[i] >= sea && filled[i] - h[i] > min_depth * 0.2).collect();
    let mut label = vec![0u32; n];
    let mut stack = Vec::new();
    let mut basins: Vec<(f32, Vec<usize>)> = Vec::new();
    let mut id = 0;
    for s in 0..n {
        if !wet[s] || label[s] != 0 {
            continue;
        }
        id += 1;
        label[s] = id;
        stack.push(s);
        let mut members = Vec::new();
        let (mut deepest, mut volume) = (0.0f32, 0.0f32);
        while let Some(i) = stack.pop() {
            members.push(i);
            deepest = deepest.max(filled[i] - h[i]);
            volume += filled[i] - h[i];
            d.nb4(i, |j| {
                if wet[j] && label[j] == 0 {
                    label[j] = id;
                    stack.push(j);
                }
            });
        }
        if members.len() >= min_area && deepest >= min_depth {
            basins.push((volume, members));
        }
    }
    basins.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1[0].cmp(&b.1[0])));
    let mut lake = vec![false; n];
    for (_, members) in basins.iter().take(max_lakes) {
        for &i in members {
            lake[i] = true;
        }
    }
    // Every other pit is filled in.
    for i in 0..n {
        if !lake[i] && h[i] >= sea && filled[i] > h[i] {
            h[i] = filled[i];
        }
    }
    lake
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_land_cell_drains_to_the_sea() {
        let d = Dims::new(40, 30, true);
        // A bowl of land in a sea, with a pit in the middle.
        let h: Vec<f32> = (0..d.n())
            .map(|i| {
                let (x, y) = d.xy(i);
                let r = (((x - 20) * (x - 20) + (y - 15) * (y - 15)) as f32).sqrt();
                let pit = if r < 3.0 { -0.3 } else { 0.0 };
                0.6 - r * 0.05 + pit
            })
            .collect();
        let dr = route(d, &h, 0.0, 1e-5);
        for i in 0..d.n() {
            if h[i] >= 0.0 {
                let mut j = i;
                let mut steps = 0;
                while dr.recv[j] != NONE {
                    assert!(dr.filled[dr.recv[j] as usize] <= dr.filled[j]);
                    j = dr.recv[j] as usize;
                    steps += 1;
                    assert!(steps < d.n());
                }
                assert!(h[j] < 0.0, "cell {i} drains to a sea cell");
            }
        }
        let acc = accumulate(&dr, &vec![1.0; d.n()]);
        assert!(acc.iter().cloned().fold(0.0, f32::max) > 50.0);
        let mut hh = h.clone();
        let lake = lakes(d, &mut hh, &dr.filled, 0.0, 4, 0.05, 3);
        assert!(lake[15 * 40 + 20], "the pit holds a lake");
    }
}
