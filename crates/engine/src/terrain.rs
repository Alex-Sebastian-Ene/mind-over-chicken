use crate::sprites::*;

pub const T_DEEP: u8 = 0;
pub const T_SHALLOW: u8 = 1;
pub const T_SAND: u8 = 2;
pub const T_GRASS: u8 = 3;
pub const T_FOREST: u8 = 4;
pub const T_HILL: u8 = 5;
pub const T_MOUNTAIN: u8 = 6;
pub const T_SNOW: u8 = 7;
pub const T_FARM: u8 = 8;
pub const T_ROAD: u8 = 9;
pub const T_PAVED: u8 = 10;

#[derive(Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Default)]
pub struct Sites {
    pub village: Point,
    pub pad: Point,
    pub ridge: Point,
    pub cave: Point,
    pub cave_mouth: Point,
    pub fire: Point,
    pub factory: Point,
    pub exchange: Point,
    pub hangar: Point,
}

pub struct Mulberry32 {
    pub state: u32,
}

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }
    pub fn next(&mut self) -> f32 {
        // All arithmetic in u32, matching JS `Math.imul`, so seeds produce the same
        // sequence as the JS build's mulberry32.
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        let res = t ^ (t >> 14);
        (res as f64 / 4294967296.0) as f32
    }
}

pub struct ValueNoise {
    grid: [f32; 33 * 33],
}

impl ValueNoise {
    pub fn new(rng: &mut Mulberry32) -> Self {
        let mut grid = [0.0f32; 33 * 33];
        for val in grid.iter_mut() {
            *val = rng.next();
        }
        Self { grid }
    }

    #[inline]
    fn at(&self, x: i32, y: i32) -> f32 {
        let gx = ((x % 33) + 33) % 33;
        let gy = ((y % 33) + 33) % 33;
        self.grid[(gy * 33 + gx) as usize]
    }

    #[inline]
    fn sm(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    pub fn value(&self, x: f32, y: f32) -> f32 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let xf = Self::sm(x - xi as f32);
        let yf = Self::sm(y - yi as f32);

        let v00 = self.at(xi, yi);
        let v10 = self.at(xi + 1, yi);
        let v01 = self.at(xi, yi + 1);
        let v11 = self.at(xi + 1, yi + 1);

        let top = v00 + (v10 - v00) * xf;
        let bot = v01 + (v11 - v01) * xf;
        top + (bot - top) * yf
    }

    pub fn noise(&self, x: f32, y: f32, octaves: usize) -> f32 {
        let mut sum = 0.0;
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += amp * self.value(x * freq, y * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

pub struct Terrain {
    pub seed: u32,
    pub h: Vec<f32>,
    pub base_type: Vec<u8>,
    pub tile_type: Vec<u8>,
    pub pixels: Vec<u8>, // RGBA 320x180x4
    pub sites: Sites,
}

impl Terrain {
    pub fn new(seed: u32) -> Self {
        let mut t = Self {
            seed,
            h: vec![0.0; W * H],
            base_type: vec![0; W * H],
            tile_type: vec![0; W * H],
            pixels: vec![0; W * H * 4],
            sites: Sites::default(),
        };
        t.generate();
        t.bake_era(0); // Stone Age default
        t
    }

    pub fn generate(&mut self) {
        let mut rng = Mulberry32::new(self.seed);
        let noise = ValueNoise::new(&mut rng);
        let mut moist_rng = Mulberry32::new(self.seed ^ 0x9e3779b9);
        let moist = ValueNoise::new(&mut moist_rng);

        let cx = 160.0f32;
        let cy = 92.0f32;
        let v = Point { x: 150, y: 102 };
        let p = Point { x: 262, y: 104 };
        self.sites.village = v;
        self.sites.pad = p;

        let ridge_a = Point {
            x: (cx - 62.0 + rng.next() * 10.0) as i32,
            y: (cy - 34.0 + rng.next() * 8.0) as i32,
        };
        let ridge_b = Point {
            x: (cx + 34.0 + rng.next() * 14.0) as i32,
            y: (cy - 46.0 + rng.next() * 6.0) as i32,
        };
        self.sites.ridge = ridge_a;

        for y in 0..H {
            for x in 0..W {
                let rx = (x as f32 - cx) / 138.0;
                let ry = (y as f32 - cy) / 80.0;
                let r2 = rx * rx + ry * ry;
                let mut h = (1.0 - r2) * 0.56 + (noise.noise(x as f32 / 26.0, y as f32 / 26.0, 4) - 0.5) * 0.56 + 0.12;

                let dx_a = x as f32 - ridge_a.x as f32;
                let dy_a = y as f32 - ridge_a.y as f32;
                h += 0.3 * (-((dx_a * dx_a + dy_a * dy_a) / (2.0 * 17.0 * 17.0))).exp();

                let dx_b = x as f32 - ridge_b.x as f32;
                let dy_b = y as f32 - ridge_b.y as f32;
                h += 0.2 * (-((dx_b * dx_b + dy_b * dy_b) / (2.0 * 12.0 * 12.0))).exp();

                let dv = (((x as i32 - v.x).pow(2) + (y as i32 - v.y).pow(2)) as f32).sqrt();
                if dv < 30.0 {
                    let factor = ((dv - 18.0) / 12.0).clamp(0.0, 1.0);
                    h = 0.5 + (h - 0.5) * factor;
                }

                let dp = (((x as i32 - p.x).pow(2) + (y as i32 - p.y).pow(2)) as f32).sqrt();
                if dp < 18.0 {
                    let factor = ((dp - 10.0) / 8.0).clamp(0.0, 1.0);
                    h = 0.47 + (h - 0.47) * factor;
                }

                let p_vx = (p.x - v.x) as f32;
                let p_vy = (p.y - v.y) as f32;
                let len_sq = p_vx * p_vx + p_vy * p_vy;
                let t_line = (((x as f32 - v.x as f32) * p_vx + (y as f32 - v.y as f32) * p_vy) / len_sq).clamp(0.0, 1.0);
                let lx = v.x as f32 + p_vx * t_line;
                let ly = v.y as f32 + p_vy * t_line + (t_line * 9.0 + self.seed as f32).sin() * 5.0;
                let dl = ((x as f32 - lx).powi(2) + (y as f32 - ly).powi(2)).sqrt();
                if dl < 4.0 {
                    h = h.clamp(0.42, 0.66);
                }

                let idx = y * W + x;
                self.h[idx] = h;

                let ty = if h < 0.24 {
                    T_DEEP
                } else if h < 0.33 {
                    T_SHALLOW
                } else if h < 0.37 {
                    T_SAND
                } else if h < 0.66 {
                    if moist.noise(x as f32 / 18.0, y as f32 / 18.0, 4) > 0.56 && dv > 26.0 && dp > 16.0 && dl > 4.0 {
                        T_FOREST
                    } else {
                        T_GRASS
                    }
                } else if h < 0.76 {
                    T_HILL
                } else if h < 0.90 {
                    T_MOUNTAIN
                } else {
                    T_SNOW
                };
                self.tile_type[idx] = ty;
            }
        }
        self.base_type = self.tile_type.clone();

        // Cave location
        self.sites.cave = Point { x: v.x - 20, y: v.y - 20 };
        for i in 0..=200 {
            let t = i as f32 / 200.0;
            let cx = (v.x as f32 + (ridge_a.x as f32 - v.x as f32) * t).round() as i32;
            let cy = (v.y as f32 + (ridge_a.y as f32 - v.y as f32) * t).round() as i32;
            if self.get(cx, cy) >= T_HILL {
                let back = (i as i32 - 3).max(0) as f32 / 200.0;
                self.sites.cave = Point {
                    x: (v.x as f32 + (ridge_a.x as f32 - v.x as f32) * back).round() as i32,
                    y: (v.y as f32 + (ridge_a.y as f32 - v.y as f32) * back).round() as i32,
                };
                self.sites.cave_mouth = Point { x: cx, y: cy };
                break;
            }
        }
        self.sites.fire = v;
        self.sites.factory = self.nearest_walkable(v.x - 46, v.y + 26, 60);
        self.sites.exchange = self.nearest_walkable(v.x + 18, v.y - 10, 60);
        self.sites.hangar = self.nearest_walkable(p.x - 26, p.y + 10, 60);
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            T_DEEP
        } else {
            self.tile_type[(y as usize) * W + (x as usize)]
        }
    }

    #[inline]
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        let t = self.get(x, y);
        t >= T_SAND && t != T_MOUNTAIN && t != T_SNOW
    }

    pub fn nearest_walkable(&self, x: i32, y: i32, max_r: i32) -> Point {
        for r in 0..=max_r {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    if self.walkable(x + dx, y + dy) && self.get(x + dx, y + dy) != T_FOREST {
                        return Point { x: x + dx, y: y + dy };
                    }
                }
            }
        }
        Point { x, y }
    }

    pub fn bake_era(&mut self, era: u8) {
        self.tile_type = self.base_type.clone();
        let v = self.sites.village;
        let s = self.sites;
        let mut rng = Mulberry32::new(self.seed + 17);

        // Era modifications
        if era == 0 {
            // Stone Age: huts around village
        } else if era == 1 {
            // Farming: farms
            for _ in 0..14 {
                let a = rng.next() * std::f32::consts::PI * 2.0;
                let r = 18.0 + rng.next() * 42.0;
                let cx = (v.x as f32 + a.cos() * r) as i32;
                let cy = (v.y as f32 + a.sin() * r) as i32;
                let fw = 6 + (rng.next() * 6.0) as i32;
                let fh = 4 + (rng.next() * 4.0) as i32;
                for fy in cy..(cy + fh) {
                    for fx in cx..(cx + fw) {
                        if self.get(fx, fy) == T_GRASS {
                            self.set_tile(fx, fy, T_FARM);
                        }
                    }
                }
            }
        } else if era >= 2 {
            // Industry or Space Age: paved central, roads
            for fy in -2..=2 {
                for fx in -3..=3 {
                    self.set_tile(v.x + fx, v.y + fy, T_PAVED);
                }
            }
            if era == 3 {
                for fy in -6..=6 {
                    for fx in -8..=8 {
                        if self.walkable(s.pad.x + fx, s.pad.y + fy) {
                            self.set_tile(s.pad.x + fx, s.pad.y + fy, T_PAVED);
                        }
                    }
                }
            }
        }

        // Rasterize terrain to RGBA pixel buffer
        let mut color_rng = Mulberry32::new(self.seed + 3);
        for y in 0..H {
            for x in 0..W {
                let i = y * W + x;
                let t = self.tile_type[i];
                let (mut r, mut g, mut b) = match t {
                    T_DEEP => (22, 44, 92),
                    T_SHALLOW => (40, 86, 146),
                    T_SAND => (214, 196, 136),
                    T_GRASS => {
                        let pick = (color_rng.next() * 3.0) as i32;
                        if pick == 0 { (96, 156, 62) } else if pick == 1 { (88, 147, 57) } else { (104, 163, 68) }
                    }
                    T_FOREST => {
                        let pick = (color_rng.next() * 3.0) as i32;
                        if pick == 0 { (46, 98, 46) } else if pick == 1 { (58, 116, 52) } else { (38, 84, 40) }
                    }
                    T_HILL => (124, 132, 74),
                    T_MOUNTAIN => (118, 110, 100),
                    T_SNOW => (232, 232, 228),
                    T_FARM => if (y >> 1) % 2 == 0 { (196, 170, 74) } else { (120, 158, 60) },
                    T_ROAD => (150, 128, 98),
                    T_PAVED => (146, 146, 152),
                    _ => (0, 0, 0),
                };

                // Relief shading
                if t >= T_SAND && t != T_ROAD && t != T_PAVED && t != T_FARM {
                    let h_left = if x > 0 { self.h[i - 1] } else { self.h[i] };
                    let h_up = if y > 0 { self.h[i - W] } else { self.h[i] };
                    let dx = self.h[i] - h_left;
                    let dy = self.h[i] - h_up;
                    let shade = (1.0 + (dx + dy) * 7.0).clamp(0.78, 1.20);
                    r = ((r as f32) * shade).min(255.0) as u8;
                    g = ((g as f32) * shade).min(255.0) as u8;
                    b = ((b as f32) * shade).min(255.0) as u8;
                }

                // Foam on shallow water
                if t == T_SHALLOW && (
                    self.get(x as i32 + 1, y as i32) == T_SAND
                    || self.get(x as i32 - 1, y as i32) == T_SAND
                    || self.get(x as i32, y as i32 + 1) == T_SAND
                    || self.get(x as i32, y as i32 - 1) == T_SAND
                ) {
                    r = 120;
                    g = 170;
                    b = 205;
                }

                let p_idx = i * 4;
                self.pixels[p_idx] = r;
                self.pixels[p_idx + 1] = g;
                self.pixels[p_idx + 2] = b;
                self.pixels[p_idx + 3] = 255;
            }
        }

        // Tree crowns on forest pixels
        let mut trng = Mulberry32::new(self.seed + 5);
        for y in (1..H).step_by(2) {
            let start_x = ((y >> 1) % 2) as usize;
            for x in (start_x..W).step_by(2) {
                if self.tile_type[y * W + x] == T_FOREST && trng.next() <= 0.7 {
                    put_pixel(&mut self.pixels, x as i32, y as i32, 0x2c, 0x5a, 0x2a, 255);
                    let col = if trng.next() > 0.5 { (0x6a, 0xa8, 0x4f) } else { (0x4f, 0x8d, 0x3e) };
                    put_pixel(&mut self.pixels, x as i32, (y - 1) as i32, col.0, col.1, col.2, 255);
                }
            }
        }

        // Cave mouth
        if era <= 1 {
            let m = self.sites.cave_mouth;
            fill_rect(&mut self.pixels, m.x - 2, m.y - 1, 4, 3, 0x1a, 0x17, 0x14, 255);
            fill_rect(&mut self.pixels, m.x - 3, m.y, 1, 2, 0x2b, 0x26, 0x22, 255);
            fill_rect(&mut self.pixels, m.x + 2, m.y, 1, 2, 0x2b, 0x26, 0x22, 255);
        }

        // Bake static structures
        if era == 0 {
            for i in 0..4 {
                let a = (i as f32 / 4.0) * std::f32::consts::PI * 2.0 + 0.5;
                let hx = v.x + (a.cos() * 11.0).round() as i32;
                let hy = v.y + (a.sin() * 7.0).round() as i32;
                draw_hut(&mut self.pixels, hx, hy);
            }
        } else if era == 1 {
            for i in 0..10 {
                let a = (i as f32 / 10.0) * std::f32::consts::PI * 2.0;
                let hx = v.x + (a.cos() * (12.0 + ((i % 2) * 6) as f32)).round() as i32;
                let hy = v.y + (a.sin() * (8.0 + ((i % 2) * 4) as f32)).round() as i32;
                draw_hut(&mut self.pixels, hx, hy);
            }
        } else if era == 2 {
            draw_factory(&mut self.pixels, s.factory.x, s.factory.y);
            draw_exchange(&mut self.pixels, s.exchange.x, s.exchange.y);
            for ring in 0..3 {
                let n = 10 + ring * 6;
                for i in 0..n {
                    let a = (i as f32 / n as f32) * std::f32::consts::PI * 2.0 + ring as f32 * 0.3;
                    let hx = v.x + (a.cos() * (12.0 + ring as f32 * 9.0)).round() as i32;
                    let hy = v.y + (a.sin() * (8.0 + ring as f32 * 6.0)).round() as i32;
                    if self.walkable(hx, hy) {
                        draw_house(&mut self.pixels, hx, hy, (0xa4, 0x40, 0x2f));
                    }
                }
            }
        } else if era == 3 {
            draw_hangar(&mut self.pixels, s.hangar.x, s.hangar.y);
            draw_pad(&mut self.pixels, s.pad.x, s.pad.y);
            draw_exchange(&mut self.pixels, s.exchange.x, s.exchange.y);
            draw_tower(&mut self.pixels, v.x - 14, v.y - 12, 14);
            draw_tower(&mut self.pixels, v.x + 18, v.y - 14, 12);
        }
    }

    #[inline]
    fn set_tile(&mut self, x: i32, y: i32, t: u8) {
        if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H {
            self.tile_type[(y as usize) * W + (x as usize)] = t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_generation_and_eras() {
        let mut terrain = Terrain::new(12345);
        assert_eq!(terrain.h.len(), W * H);
        assert_eq!(terrain.tile_type.len(), W * H);
        assert_eq!(terrain.pixels.len(), W * H * 4);

        // Village site must be walkable
        assert!(terrain.walkable(terrain.sites.village.x, terrain.sites.village.y));

        // Test baking all eras
        for era in 0..=3 {
            terrain.bake_era(era);
            // Pixels should not be all zero
            let non_zero = terrain.pixels.iter().any(|&p| p != 0);
            assert!(non_zero, "Pixels should be non-zero after baking era {}", era);
        }
    }
}
