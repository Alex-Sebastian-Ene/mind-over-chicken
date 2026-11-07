//! Morphological filters: 2x2 majority filtering to remove high-frequency single-pixel
//! specks while preserving coasts, rivers, and natural land boundaries across the cylinder.

pub struct TerrainFilter {
    pub width: usize,
    pub height: usize,
}

impl TerrainFilter {
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

    /// Performs multiple majority filter passes on the terrain type grid to guarantee
    /// elimination of isolated 1x1 noise specks.
    pub fn majority_filter(&self, types: &[u8], river_mask: &[bool]) -> Vec<u8> {
        let mut current = types.to_vec();
        let neighbors: [(i32, i32); 8] = [
            (-1, -1), (0, -1), (1, -1),
            (-1,  0),          (1,  0),
            (-1,  1), (0,  1), (1,  1),
        ];

        let ortho_neighbors: [(i32, i32); 4] = [
            (-1, 0), (1, 0), (0, -1), (0, 1)
        ];

        for _pass in 0..2 {
            let mut next = current.clone();

            for y in 0..self.height {
                for x in 0..self.width {
                    let curr_idx = self.idx(x, y);

                    // Rivers should remain 1-pixel channels
                    if curr_idx < river_mask.len() && river_mask[curr_idx] {
                        continue;
                    }

                    let curr_type = current[curr_idx];
                    let mut counts = [0u8; 16];
                    let mut ortho_same = 0;

                    for &(dx, dy) in &ortho_neighbors {
                        let ny = y as i32 + dy;
                        if ny < 0 || ny >= self.height as i32 {
                            continue;
                        }
                        let nx = self.wrap_x(x as i32 + dx);
                        let n_idx = self.idx(nx, ny as usize);
                        if current[n_idx] == curr_type {
                            ortho_same += 1;
                        }
                    }

                    // Count all 8 neighbors
                    for &(dx, dy) in &neighbors {
                        let ny = y as i32 + dy;
                        if ny < 0 || ny >= self.height as i32 {
                            continue;
                        }
                        let nx = self.wrap_x(x as i32 + dx);
                        let n_idx = self.idx(nx, ny as usize);
                        let nt = current[n_idx] as usize;
                        if nt < counts.len() {
                            counts[nt] += 1;
                        }
                    }

                    // If zero orthogonal neighbors share this type, it's an isolated 1x1 speck
                    if ortho_same == 0 {
                        let mut best_type = curr_type;
                        let mut max_count = 0;
                        for (t, &c) in counts.iter().enumerate() {
                            if c > max_count {
                                max_count = c;
                                best_type = t as u8;
                            }
                        }
                        next[curr_idx] = best_type;
                    }
                }
            }

            current = next;
        }

        current
    }
}
