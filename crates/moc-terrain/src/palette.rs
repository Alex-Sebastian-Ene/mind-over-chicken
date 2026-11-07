//! Color palettes, posterized elevation bands, quantized hillshading, crisp coastlines
//! with shallow shelves and foam lines, and river rendering.

use crate::climate::*;

pub struct BiomePalette {
    pub deep: [u8; 3],
    pub shallow: [u8; 3],
    pub sand: [u8; 3],
    pub grass_low: [u8; 3],
    pub grass_mid: [u8; 3],
    pub grass_high: [u8; 3],
    pub forest: [u8; 3],
    pub hill: [u8; 3],
    pub mountain: [u8; 3],
    pub snow: [u8; 3],
    pub river: [u8; 3],
    pub foam: [u8; 3],
}

impl BiomePalette {
    pub fn temperate() -> Self {
        Self {
            deep: [24, 46, 94],
            shallow: [42, 88, 148],
            sand: [214, 196, 136],
            grass_low: [96, 156, 62],
            grass_mid: [88, 147, 57],
            grass_high: [104, 163, 68],
            forest: [46, 98, 46],
            hill: [124, 132, 74],
            mountain: [118, 110, 100],
            snow: [232, 232, 228],
            river: [45, 95, 158],
            foam: [180, 220, 240],
        }
    }

    pub fn desert() -> Self {
        Self {
            deep: [26, 72, 98],
            shallow: [46, 124, 140],
            sand: [232, 206, 140],
            grass_low: [210, 172, 102],
            grass_mid: [200, 162, 94],
            grass_high: [218, 180, 110],
            forest: [120, 150, 70],
            hill: [184, 120, 72],
            mountain: [140, 78, 52],
            snow: [240, 222, 190],
            river: [50, 130, 145],
            foam: [190, 225, 230],
        }
    }

    pub fn ice() -> Self {
        Self {
            deep: [16, 32, 66],
            shallow: [54, 92, 132],
            sand: [176, 186, 196],
            grass_low: [214, 228, 236],
            grass_mid: [204, 220, 230],
            grass_high: [222, 234, 240],
            forest: [40, 84, 86],
            hill: [170, 190, 206],
            mountain: [110, 124, 146],
            snow: [248, 250, 255],
            river: [60, 100, 142],
            foam: [210, 230, 245],
        }
    }
}

pub struct TerrainRenderer {
    pub width: usize,
    pub height: usize,
    pub palette: BiomePalette,
}

impl TerrainRenderer {
    pub fn new(width: usize, height: usize, palette: BiomePalette) -> Self {
        Self {
            width,
            height,
            palette,
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    #[inline]
    fn wrap_x(&self, x: i32) -> usize {
        x.rem_euclid(self.width as i32) as usize
    }

    /// Renders the complete terrain into an RGBA pixel buffer (width * height * 4).
    pub fn render_rgba(
        &self,
        heights: &[f32],
        types: &[u8],
        river_mask: &[bool],
    ) -> Vec<u8> {
        let mut rgba = vec![0u8; self.width * self.height * 4];

        for y in 0..self.height {
            for x in 0..self.width {
                let curr_idx = self.idx(x, y);
                let ty = types[curr_idx];
                let h = heights[curr_idx];

                // 1. Quantized hillshade from normal gradient
                let xm1 = self.wrap_x(x as i32 - 1);
                let xp1 = self.wrap_x(x as i32 + 1);
                let ym1 = if y > 0 { y - 1 } else { 0 };
                let yp1 = if y + 1 < self.height { y + 1 } else { self.height - 1 };

                let dh_dx = heights[self.idx(xp1, y)] - heights[self.idx(xm1, y)];
                let dh_dy = heights[self.idx(x, yp1)] - heights[self.idx(x, ym1)];

                // Sun from upper-left / northwest
                let slope = -dh_dx * 2.0 - dh_dy * 2.0;
                let shade_mult = if slope > 0.08 {
                    1.12 // Lit
                } else if slope < -0.08 {
                    0.86 // Shaded
                } else {
                    1.00 // Neutral
                };

                // 2. Base color selection with posterized bands
                let mut rgb = match ty {
                    T_DEEP => self.palette.deep,
                    T_SHALLOW => self.palette.shallow,
                    T_SAND => self.palette.sand,
                    T_GRASS => {
                        if h < 0.44 {
                            self.palette.grass_low
                        } else if h < 0.54 {
                            self.palette.grass_mid
                        } else {
                            self.palette.grass_high
                        }
                    }
                    T_FOREST => self.palette.forest,
                    T_HILL => self.palette.hill,
                    T_MOUNTAIN => self.palette.mountain,
                    T_SNOW => self.palette.snow,
                    _ => self.palette.grass_mid,
                };

                // 3. Rivers
                if curr_idx < river_mask.len() && river_mask[curr_idx] {
                    rgb = self.palette.river;
                } else if ty == T_SHALLOW {
                    // Check if adjacent to land for coastal foam line
                    let mut adjacent_to_land = false;
                    for &(dx, dy) in &[(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let ny = y as i32 + dy;
                        if ny >= 0 && ny < self.height as i32 {
                            let nx = self.wrap_x(x as i32 + dx);
                            let nt = types[self.idx(nx, ny as usize)];
                            if nt >= T_SAND {
                                adjacent_to_land = true;
                                break;
                            }
                        }
                    }
                    if adjacent_to_land {
                        rgb = self.palette.foam;
                    }
                }

                // 4. Apply hillshade (only on land)
                if ty >= T_SAND {
                    rgb[0] = ((rgb[0] as f32 * shade_mult).round() as u32).min(255) as u8;
                    rgb[1] = ((rgb[1] as f32 * shade_mult).round() as u32).min(255) as u8;
                    rgb[2] = ((rgb[2] as f32 * shade_mult).round() as u32).min(255) as u8;
                }

                let p = curr_idx * 4;
                rgba[p] = rgb[0];
                rgba[p + 1] = rgb[1];
                rgba[p + 2] = rgb[2];
                rgba[p + 3] = 255;
            }
        }

        rgba
    }
}
