//! CPU renderer: the camera's 320×180 window onto the planet, as RGBA, with terrain,
//! territory tint and borders, fires, villages, agents and effects. Seam-safe: the camera
//! and blits come from `moc_planet`.

use moc_planet::color::Rgba;
use moc_planet::{VH, VW};

use crate::jobs::{binfo, info, Building, Role};
use crate::planet::blit;
use crate::world::*;

const SKIN: Rgba = [236, 200, 160, 255];
const WALL: Rgba = [150, 108, 68, 255];
const DOOR: Rgba = [58, 38, 28, 255];
const PACK: Rgba = [140, 92, 52, 255];
const GRAIN: Rgba = [242, 220, 96, 255];

/// Overview map size.
pub const MINI_W: usize = 64;
pub const MINI_H: usize = 32;

#[inline]
fn hash(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

#[inline]
fn blend(dst: &mut [u8], c: [u8; 3], a: u32) {
    for k in 0..3 {
        dst[k] = ((c[k] as u32 * a + dst[k] as u32 * (255 - a)) / 255) as u8;
    }
}

impl World {
    /// Draw the camera's view into `self.view` (RGBA, VW×VH).
    pub fn render(&mut self) {
        let cam = self.cam;
        blit(&mut self.view, &self.terrain.rgba, self.d, &cam);
        self.draw_land();
        self.draw_territory();
        self.draw_fires();
        self.draw_deposits();
        self.draw_villages();
        self.draw_agents();
        self.draw_effects();
        self.draw_setup();
        if self.rain_t > 0.0 {
            self.draw_rain();
        }
    }

    fn screen(&self, x: f32, y: f32) -> (i32, i32) {
        let (sx, sy) = self.cam.to_screen(x, y);
        (sx.floor() as i32, sy.floor() as i32)
    }

    fn draw_territory(&mut self) {
        let (cx, cy) = (self.cam.ix(), self.cam.iy());
        let d = self.d;
        let (pw, ph) = (d.w, d.h);
        let mut colors = [[0u8; 3]; MAX_FACTIONS];
        for (i, f) in self.factions.iter().enumerate() {
            colors[i] = f.color;
        }
        let owner = &self.owner;
        let walk = &self.terrain.walk;
        let data = &mut self.view.data;
        for sy in 0..VH {
            let py = d.wrap_y(cy + sy as i32) as usize;
            let row = py * pw;
            let up = (py + ph - 1) % ph * pw;
            let down = (py + 1) % ph * pw;
            for sx in 0..VW {
                let px = d.wrap_x(cx + sx as i32) as usize;
                let o = owner[row + px];
                if o == NO_F {
                    continue;
                }
                let ns = [row + (px + pw - 1) % pw, row + (px + 1) % pw, up + px, down + px];
                let mut edge = 0u32;
                for &n in &ns {
                    let on = owner[n];
                    if on != o {
                        if on != NO_F {
                            edge = edge.max(2);
                        } else if walk[n] {
                            edge = edge.max(1);
                        }
                    }
                }
                let a = match edge {
                    2 => 220,
                    1 => 150,
                    _ => 52,
                };
                let i = (sy * VW + sx) * 4;
                blend(&mut data[i..i + 3], colors[o as usize], a);
            }
        }
    }

    fn draw_fires(&mut self) {
        let t = (self.time * 12.0) as u32;
        for k in 0..self.fires.len() {
            let i = self.fires[k].i as usize;
            let (x, y) = ((i % self.d.w) as f32, (i / self.d.w) as f32);
            let (sx, sy) = self.screen(x, y);
            if sx < -1 || sy < -2 || sx > VW as i32 || sy > VH as i32 {
                continue;
            }
            let h = hash(i as u32 ^ t.wrapping_mul(2_654_435_761));
            let c = match h % 3 {
                0 => [255, 214, 90, 255],
                1 => [255, 140, 40, 255],
                _ => [222, 60, 30, 255],
            };
            self.view.px(sx, sy, c);
            if h & 8 != 0 {
                self.view.px(sx, sy - 1, [255, 190, 70, 200]);
            }
        }
    }

    fn draw_villages(&mut self) {
        let blink = (self.time * 6.0) as i32 % 2 == 0;
        for vi in 0..self.villages.len() {
            let v = &self.villages[vi];
            if !v.alive {
                continue;
            }
            let (sx, sy) = self.screen(v.x as f32 + 0.5, v.y as f32 + 0.5);
            if sx < -8 || sy < -8 || sx > VW as i32 + 8 || sy > VH as i32 + 8 {
                continue;
            }
            let f = &self.factions[v.owner as usize];
            let roof: Rgba = [f.color[0], f.color[1], f.color[2], 255];
            let capital = f.capital as usize == vi;
            let big = capital || v.pop >= 6;
            let siege = v.siege > 0.0;
            let age = f.age;
            let (built, plague, camp, side, fest, mourn) = (v.built, v.plague, v.camp, v.side, v.festival_t > 0.0, v.mourn_t > 0.0);
            let px = |s: &mut Self, x: i32, y: i32, c: Rgba| s.view.px(sx + x, sy + y, c);
            if big {
                px(self, 0, -3, roof);
                for x in -1..=1 {
                    px(self, x, -2, roof);
                }
                for x in -2..=2 {
                    px(self, x, -1, roof);
                    px(self, x, 0, if x == 0 { DOOR } else { WALL });
                }
            } else {
                px(self, 0, -2, roof);
                for x in -1..=1 {
                    px(self, x, -1, roof);
                    px(self, x, 0, if x == 0 { DOOR } else { WALL });
                }
            }
            if capital {
                for y in -6..=-1 {
                    px(self, 3, y, [70, 52, 40, 255]);
                }
                px(self, 4, -6, roof);
                px(self, 5, -6, roof);
                px(self, 4, -5, roof);
                px(self, 5, -5, roof);
            }
            if siege && blink {
                for k in -3..=3 {
                    px(self, k, -5, [255, 60, 50, 230]);
                    px(self, k, 2, [255, 60, 50, 230]);
                }
            }
            // The age's buildings round the village, newest last (grey while dormant).
            self.draw_buildings(sx, sy, built, age, roof);
            // A rebel flag over a village that has risen against its own crown.
            if camp != 0 && side == 1 {
                for y in -8..=-4 {
                    px(self, -3, y, [60, 40, 30, 255]);
                }
                px(self, -2, -8, [220, 30, 40, 255]);
                px(self, -1, -8, [220, 30, 40, 255]);
                px(self, -2, -7, [20, 20, 20, 255]);
                px(self, -1, -7, [220, 30, 40, 255]);
            }
            if mourn {
                px(self, 2, -4, [20, 20, 24, 255]);
                px(self, 2, -5, [20, 20, 24, 255]);
            }
            if plague > 0.2 {
                let h = hash(vi as u32 ^ (self.time * 3.0) as u32);
                px(self, (h % 7) as i32 - 3, -(((h >> 4) % 3) as i32) - 1, [120, 200, 60, 220]);
                px(self, ((h >> 8) % 7) as i32 - 3, ((h >> 12) % 2) as i32, [150, 220, 70, 200]);
            }
            if fest {
                let t = (self.time * 8.0) as u32;
                for k in 0..4u32 {
                    let h = hash(vi as u32 * 31 + k + t);
                    let c = [[255, 220, 80, 255], [255, 120, 200, 255], [120, 220, 255, 255], [170, 255, 120, 255]][(h % 4) as usize];
                    px(self, ((h >> 3) % 9) as i32 - 4, -(((h >> 7) % 6) as i32) - 2, c);
                }
            }
        }
    }

    /// Up to eight of a village's buildings, 4x4 sprites in a ring round it.
    fn draw_buildings(&mut self, sx: i32, sy: i32, built: u64, age: u8, roof: Rgba) {
        const SLOTS: [(i32, i32); 8] = [(-8, -3), (4, -3), (-8, 2), (4, 2), (-2, 3), (-13, -1), (9, -1), (-2, -9)];
        if built == 0 {
            return;
        }
        let mut list = [0usize; 8];
        let mut n = 0;
        // The newest eight (highest bits), drawn oldest first.
        let mut bits = built;
        while bits != 0 && n < 8 {
            let k = 63 - bits.leading_zeros() as usize;
            bits &= !(1u64 << k);
            list[n] = k;
            n += 1;
        }
        for s in 0..n {
            let b = Building::ALL[list[n - 1 - s]];
            let dormant = b.unlock_age() > age;
            let (ox, oy) = SLOTS[s];
            let spr = binfo(b).sprite;
            for (ry, row) in spr.iter().enumerate() {
                for (rx, ch) in row.bytes().enumerate() {
                    let c: Rgba = match ch {
                        b'r' => roof,
                        b'w' => WALL,
                        b'd' => DOOR,
                        b's' => [150, 150, 160, 255],
                        b'f' => [255, 150, 40, 255],
                        b'y' => [240, 210, 90, 255],
                        b'g' => [90, 170, 70, 255],
                        b'b' => [80, 140, 220, 255],
                        b'k' => [50, 50, 60, 255],
                        b'l' => [255, 240, 170, 255],
                        b'm' => [200, 205, 215, 255],
                        b'o' => [190, 100, 60, 255],
                        b'p' => [150, 90, 190, 255],
                        b'h' => [240, 240, 240, 255],
                        b'c' => [90, 220, 230, 255],
                        b'n' => [120, 80, 45, 255],
                        b'R' => [210, 50, 40, 255],
                        _ => continue,
                    };
                    let c = if dormant {
                        let g = ((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3 / 2 + 40) as u8;
                        [g, g, g, 255]
                    } else {
                        c
                    };
                    self.view.px(sx + ox + rx as i32, sy + oy + ry as i32, c);
                }
            }
        }
    }

    fn draw_agents(&mut self) {
        let blink = (self.time * 10.0) as i32 % 2 == 0;
        for ai in 0..self.agents.len() {
            let a = &self.agents[ai];
            if !a.alive {
                continue;
            }
            let (sx, sy) = self.screen(a.x, a.y);
            if sx < -2 || sy < -3 || sx > VW as i32 + 1 || sy > VH as i32 + 1 {
                continue;
            }
            let c = self.factions[a.faction as usize].color;
            let (job, fight, carry, state, prisoner, side, camp) = (a.job, a.fight, a.carry, a.state, a.prisoner, a.side, a.camp);
            let body: Rgba = if prisoner > 0.0 {
                [120, 120, 120, 255]
            } else if camp != 0 && side == 1 {
                // Rebels: their people's colour, darkened.
                [c[0] / 2 + 20, c[1] / 2, c[2] / 2, 255]
            } else {
                [c[0], c[1], c[2], 255]
            };
            if state == State::Sleep {
                // Asleep: lying down.
                self.view.px(sx, sy, [body[0] / 2, body[1] / 2, body[2] / 2, 255]);
                self.view.px(sx + 1, sy, [SKIN[0] / 2, SKIN[1] / 2, SKIN[2] / 2, 255]);
                continue;
            }
            if state == State::Sail {
                for k in -1..=1 {
                    self.view.px(sx + k, sy + 1, [110, 72, 40, 255]);
                }
                self.view.px(sx, sy - 2, [240, 240, 240, 255]);
            }
            self.view.px(sx, sy, body);
            self.view.px(sx, sy - 1, if camp != 0 && side == 1 { [220, 40, 40, 255] } else { SKIN });
            let ji = info(job);
            let t = ji.tool;
            match ji.role {
                Role::Fight | Role::Hunt => self.view.px(sx + 1, sy - 1, [t[0], t[1], t[2], 255]),
                Role::Settle => self.view.px(sx - 1, sy, PACK),
                Role::Gather | Role::Haul => {
                    if carry > 0.0 {
                        self.view.px(sx + 1, sy, if ji.out == crate::jobs::Out::Food { GRAIN } else { [t[0], t[1], t[2], 255] });
                    }
                }
                Role::Work | Role::Lead => self.view.px(sx + 1, sy, [t[0], t[1], t[2], 255]),
            }
            if job == crate::jobs::Job::Chief {
                self.view.px(sx, sy - 2, [250, 210, 70, 255]);
            }
            if fight > 0.0 && blink {
                self.view.px(sx, sy - 2, [255, 255, 255, 255]);
            }
        }
    }

    fn ring(&mut self, x: f32, y: f32, r: f32, c: Rgba) {
        let (sx, sy) = self.cam.to_screen(x, y);
        let n = (r * 6.0 + 8.0) as i32;
        for k in 0..n {
            let ang = k as f32 / n as f32 * std::f32::consts::TAU;
            self.view.px((sx + ang.cos() * r).round() as i32, (sy + ang.sin() * r).round() as i32, c);
        }
    }

    fn draw_effects(&mut self) {
        for k in 0..self.effects.len() {
            let e = self.effects[k];
            let p = (e.t / e.ttl).clamp(0.0, 1.0);
            let fade = ((1.0 - p) * 255.0) as u8;
            let seed = (k as u32).wrapping_mul(97) ^ (self.time * 20.0) as u32;
            match e.kind {
                0 => {
                    let (sx, sy) = self.screen(e.x, e.y);
                    let h = hash(seed);
                    self.view.px(sx + (h % 3) as i32 - 1, sy - 1 - ((h >> 3) % 2) as i32, [255, 255, 200, 255]);
                    self.view.px(sx + ((h >> 5) % 3) as i32 - 1, sy - ((h >> 7) % 2) as i32, [255, 200, 80, 255]);
                }
                1 => {
                    self.ring(e.x, e.y, 1.0 + e.r * p, [255, 230, 160, fade]);
                    self.ring(e.x, e.y, 0.5 + e.r * p * 0.6, [255, 150, 60, fade]);
                }
                2 => {
                    let (sx, sy) = self.screen(e.x, e.y);
                    let r = e.r as i32;
                    for j in 0..(e.r * e.r * 0.8) as u32 {
                        let h = hash(seed ^ j.wrapping_mul(2_246_822_519));
                        let ox = (h % (2 * r as u32 + 1)) as i32 - r;
                        let oy = ((h >> 10) % (2 * r as u32 + 1)) as i32 - r;
                        if ox * ox + oy * oy <= r * r {
                            let c = if h & 0x8000_0000 != 0 && h & 0x4000_0000 != 0 { [230, 240, 255, fade] } else { [40, 44, 60, fade / 2] };
                            self.view.px(sx + ox, sy + oy, c);
                        }
                    }
                }
                3 | 4 => {
                    let (sx, sy) = self.screen(e.x, e.y);
                    let c: Rgba = if e.kind == 3 { [255, 230, 110, fade] } else { [150, 240, 110, fade] };
                    for j in 0..24u32 {
                        let h = hash(j.wrapping_mul(2_654_435_761) ^ k as u32);
                        let ox = (h % (2 * e.r as u32 + 1)) as i32 - e.r as i32;
                        let oy = ((h >> 12) % (2 * e.r as u32 + 1)) as i32 - e.r as i32 - (p * 6.0) as i32;
                        self.view.px(sx + ox, sy + oy, c);
                    }
                }
                5 => self.ring(e.x, e.y, 2.0 + e.r * p, [200, 250, 255, fade]),
                6 => {
                    self.ring(e.x, e.y, 2.0 + e.r * p, [255, 240, 200, fade]);
                }
                _ => {
                    self.ring(e.x, e.y, 2.0 + e.r * p, [255, 70, 60, fade]);
                    self.ring(e.x, e.y, 1.0 + e.r * p * 0.7, [255, 160, 60, fade]);
                }
            }
        }
    }

    /// The founding flow (#98): waiting homelands glow in their people's colour; the spot being
    /// chosen shows green (yes) or red (no); seeding fills a ring round it.
    fn draw_setup(&mut self) {
        use crate::setup::Phase;
        let s = self.setup.clone();
        if matches!(s.phase, Phase::Off | Phase::Done) {
            return;
        }
        let pulse = 0.5 + 0.5 * (self.time * 4.0).sin();
        for &f in &s.slots {
            let fac = &self.factions[f as usize];
            if !fac.pending {
                continue;
            }
            let c = fac.color;
            let a = if f == s.slot { 200 + (55.0 * pulse) as u8 } else { 110 };
            let (x, y) = (fac.start.0 as f32 + 0.5, fac.start.1 as f32 + 0.5);
            self.ring(x, y, 6.0 + if f == s.slot { pulse } else { 0.0 }, [c[0], c[1], c[2], a]);
        }
        if matches!(s.phase, Phase::Place | Phase::Seed) {
            let (x, y) = (s.spot.0 as f32 + 0.5, s.spot.1 as f32 + 0.5);
            let c: Rgba = if s.phase == Phase::Seed {
                let k = self.factions[s.slot as usize].color;
                [k[0], k[1], k[2], 255]
            } else if s.spot_ok {
                [120, 255, 140, 255]
            } else {
                [255, 80, 70, 255]
            };
            self.ring(x, y, 3.0, c);
            let (sx, sy) = self.cam.to_screen(x, y);
            let (sx, sy) = (sx.floor() as i32, sy.floor() as i32);
            for k in -1..=1 {
                self.view.px(sx + k, sy, c);
                self.view.px(sx, sy + k, c);
            }
            if s.phase == Phase::Seed {
                // The seeding ring fills clockwise from the top.
                let p = (s.t / crate::setup::SEED_SECONDS).clamp(0.0, 1.0);
                let n = 64;
                for k in 0..(p * n as f32) as i32 {
                    let ang = -std::f32::consts::FRAC_PI_2 + k as f32 / n as f32 * std::f32::consts::TAU;
                    let (rx, ry) = (sx as f32 + ang.cos() * 8.0, sy as f32 + ang.sin() * 8.0);
                    self.view.px(rx.round() as i32, ry.round() as i32, [255, 230, 120, 255]);
                }
            }
        }
    }

    fn draw_rain(&mut self) {
        let a = (self.rain_t.min(2.0) / 2.0 * 150.0) as u8;
        let t = self.time;
        for k in 0..360u32 {
            let h = hash(k.wrapping_mul(2_654_435_761) ^ 0x5bd1_e995);
            let x = (h % (VW as u32 + 40)) as f32 - (t * 30.0) % 40.0;
            let y = ((h >> 12) % VH as u32) as f32 + t * 140.0 * (0.8 + (h & 7) as f32 * 0.05);
            let (x, y) = (x as i32, (y as i32).rem_euclid(VH as i32));
            self.view.px(x, y, [170, 200, 255, a]);
            self.view.px(x - 1, y - 1, [170, 200, 255, a / 2]);
        }
    }

    /// The whole planet shrunk to `MINI_W`×`MINI_H` RGBA, with territory, for an overview map.
    pub fn render_minimap(&mut self) {
        let d = self.d;
        if self.minimap.len() != MINI_W * MINI_H * 4 {
            self.minimap = vec![0; MINI_W * MINI_H * 4];
        }
        for my in 0..MINI_H {
            let y = (my * d.h + d.h / (2 * MINI_H)) / MINI_H;
            for mx in 0..MINI_W {
                let x = (mx * d.w + d.w / (2 * MINI_W)) / MINI_W;
                let i = y * d.w + x;
                let o = (my * MINI_W + mx) * 4;
                let px = &mut self.minimap[o..o + 4];
                px.copy_from_slice(&self.terrain.rgba[i * 4..i * 4 + 4]);
                let f = self.owner[i];
                if f != NO_F {
                    blend(&mut px[0..3], self.factions[f as usize].color, 170);
                }
            }
        }
        self.draw_deposits_minimap();
    }

    /// Owner of the tile at a planet point (NO_F = nobody).
    pub fn owner_at(&self, x: f32, y: f32) -> u8 {
        self.owner[self.d.idx(x.floor() as i32, y.floor() as i32)]
    }
}
