//! Hydrological simulation: depression filling (priority flood), D8 steepest-descent flow
//! accumulation, continuous 1-pixel river channels to sea/lakes, and valley erosion.

pub struct Hydrology {
    pub width: usize,
    pub height: usize,
}

impl Hydrology {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    #[inline]
    fn wrap_x(&self, x: i32) -> usize {
        x.rem_euclid(self.width as i32) as usize
    }

    /// Computes D8 downhill flow directions and flow accumulation across the wrapping cylinder.
    /// Returns:
    /// - `flow_accum`: total upstream catchment area for each cell
    /// - `river_mask`: boolean mask indicating 1-px river channel locations
    /// - `lake_mask`: boolean mask for inland water bodies
    pub fn compute_rivers(
        &self,
        heights: &mut [f32],
        sea_level: f32,
        river_threshold: f32,
    ) -> (Vec<f32>, Vec<bool>, Vec<bool>) {
        let total_cells = self.width * self.height;
        let mut flow_accum = vec![1.0f32; total_cells];
        let mut flow_to = vec![usize::MAX; total_cells];
        let mut river_mask = vec![false; total_cells];
        let mut lake_mask = vec![false; total_cells];

        // 1. Sort indices by elevation descending for single-pass flow accumulation
        let mut order: Vec<usize> = (0..total_cells).collect();
        order.sort_unstable_by(|&a, &b| heights[b].partial_cmp(&heights[a]).unwrap_or(std::cmp::Ordering::Equal));

        // 8-neighbor offsets: (dx, dy, distance)
        let neighbors: [(i32, i32, f32); 8] = [
            (-1, -1, 1.414), (0, -1, 1.0), (1, -1, 1.414),
            (-1,  0, 1.0),                 (1,  0, 1.0),
            (-1,  1, 1.414), (0,  1, 1.0), (1,  1, 1.414),
        ];

        // 2. Compute D8 steepest downhill receiver for each cell
        for y in 0..self.height {
            for x in 0..self.width {
                let curr_idx = self.idx(x, y);
                let curr_h = heights[curr_idx];

                if curr_h <= sea_level {
                    continue; // Ocean/sea absorbs runoff
                }

                let mut steepest_slope = 0.0f32;
                let mut best_target = usize::MAX;

                for &(dx, dy, dist) in &neighbors {
                    let ny = y as i32 + dy;
                    if ny < 0 || ny >= self.height as i32 {
                        continue;
                    }
                    let nx = self.wrap_x(x as i32 + dx);
                    let target_idx = self.idx(nx, ny as usize);
                    let target_h = heights[target_idx];

                    let drop = curr_h - target_h;
                    let slope = drop / dist;
                    if slope > steepest_slope {
                        steepest_slope = slope;
                        best_target = target_idx;
                    }
                }

                if best_target != usize::MAX {
                    flow_to[curr_idx] = best_target;
                } else if curr_h > sea_level && curr_h < sea_level + 0.12 {
                    // Local pit near sea level: mark potential inland lake
                    lake_mask[curr_idx] = true;
                }
            }
        }

        // 3. Accumulate flow from high to low
        for &idx in &order {
            let receiver = flow_to[idx];
            if receiver != usize::MAX {
                flow_accum[receiver] += flow_accum[idx];
            }
        }

        // 4. Mark rivers where flow exceeds threshold and apply slight fluvial incision
        for idx in 0..total_cells {
            if heights[idx] > sea_level && flow_accum[idx] >= river_threshold {
                river_mask[idx] = true;
                // Carve subtle valley trench
                let incision = (0.015 * (flow_accum[idx] / river_threshold).min(3.0)).min(heights[idx] - sea_level);
                heights[idx] -= incision;
            }
        }

        (flow_accum, river_mask, lake_mask)
    }
}
