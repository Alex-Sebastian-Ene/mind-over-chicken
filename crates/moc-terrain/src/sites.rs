//! Continent discovery and settlement site evaluation: identifying flat fertile plains
//! near fresh water and natural harbours.

use crate::climate::*;

#[derive(Clone, Debug)]
pub struct Continent {
    pub id: usize,
    pub cell_count: usize,
    pub center_x: usize,
    pub center_y: usize,
    pub primary_site: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct CampaignSites {
    pub village: (usize, usize),
    pub pad: (usize, usize),
    pub cave: (usize, usize),
    pub factory: (usize, usize),
    pub exchange: (usize, usize),
    pub hangar: (usize, usize),
    pub harbour: (usize, usize),
}

pub struct SiteFinder {
    pub width: usize,
    pub height: usize,
}

impl SiteFinder {
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

    pub fn is_walkable(ty: u8) -> bool {
        ty >= T_SAND && ty != T_MOUNTAIN && ty != T_SNOW
    }

    /// Discovers connected landmasses (continents) across cylinder wrapping.
    pub fn find_continents(
        &self,
        types: &[u8],
        heights: &[f32],
        river_mask: &[bool],
    ) -> (Vec<Continent>, Vec<usize>) {
        let total = self.width * self.height;
        let mut land_id = vec![0usize; total];
        let mut current_id = 0;
        let mut raw_continents = Vec::new();

        let mut queue = std::collections::VecDeque::new();

        for y in 0..self.height {
            for x in 0..self.width {
                let start_idx = self.idx(x, y);
                if !Self::is_walkable(types[start_idx]) || land_id[start_idx] != 0 {
                    continue;
                }

                current_id += 1;
                land_id[start_idx] = current_id;
                queue.push_back((x, y));

                let mut count = 0;
                let mut sum_x = 0;
                let mut sum_y = 0;

                while let Some((cx, cy)) = queue.pop_front() {
                    count += 1;
                    sum_x += cx;
                    sum_y += cy;

                    for &(dx, dy) in &[(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let ny = cy as i32 + dy;
                        if ny < 0 || ny >= self.height as i32 {
                            continue;
                        }
                        let nx = self.wrap_x(cx as i32 + dx);
                        let n_idx = self.idx(nx, ny as usize);

                        if Self::is_walkable(types[n_idx]) && land_id[n_idx] == 0 {
                            land_id[n_idx] = current_id;
                            queue.push_back((nx, ny as usize));
                        }
                    }
                }

                if count >= 16 {
                    let avg_x = sum_x / count;
                    let avg_y = sum_y / count;
                    raw_continents.push((current_id, count, avg_x, avg_y));
                }
            }
        }

        // Sort continents by size descending
        raw_continents.sort_by(|a, b| b.1.cmp(&a.1));

        let mut continents = Vec::new();
        for (i, &(cid, count, cx, cy)) in raw_continents.iter().enumerate() {
            // Find best settlement site on this continent: flat, near fresh water or coast
            let best_site = self.find_best_site_on_continent(cid, &land_id, types, heights, river_mask);
            continents.push(Continent {
                id: i + 1,
                cell_count: count,
                center_x: cx,
                center_y: cy,
                primary_site: best_site,
            });
        }

        (continents, land_id)
    }

    fn find_best_site_on_continent(
        &self,
        target_id: usize,
        land_id: &[usize],
        types: &[u8],
        heights: &[f32],
        river_mask: &[bool],
    ) -> (usize, usize) {
        let mut best_score = -9999.0f32;
        let mut best_coord = (self.width / 2, self.height / 2);

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = self.idx(x, y);
                if land_id[idx] != target_id || types[idx] != T_GRASS {
                    continue;
                }

                let mut score = 50.0;
                // Flatness: check height delta with neighbors
                let mut max_diff = 0.0f32;
                for &(dx, dy) in &[(-2, 0), (2, 0), (0, -2), (0, 2)] {
                    let ny = y as i32 + dy;
                    if ny >= 0 && ny < self.height as i32 {
                        let nx = self.wrap_x(x as i32 + dx);
                        let diff = (heights[self.idx(nx, ny as usize)] - heights[idx]).abs();
                        if diff > max_diff {
                            max_diff = diff;
                        }
                    }
                }
                score -= max_diff * 60.0;

                // Fresh water bonus
                for dy in -5..=5 {
                    for dx in -5..=5 {
                        let ny = y as i32 + dy;
                        if ny >= 0 && ny < self.height as i32 {
                            let nx = self.wrap_x(x as i32 + dx);
                            let n_idx = self.idx(nx, ny as usize);
                            if river_mask[n_idx] {
                                score += 25.0;
                                break;
                            }
                        }
                    }
                }

                if score > best_score {
                    best_score = score;
                    best_coord = (x, y);
                }
            }
        }

        best_coord
    }
}
