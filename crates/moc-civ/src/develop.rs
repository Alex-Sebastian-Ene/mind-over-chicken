//! Development (#165): civilisations improve their land, or wear it out.
//!
//! **Fertility.** Every land tile has a fertility byte (`FoodMap::fert`): 128 is the land as
//! the planet made it, and a tile's food scales with it ([`mul`]: 0 to 2×). Rich soil and
//! steppe deposits start above it (`resources::init`).
//!
//! **Care and strain.** Once a second ([`tick`]) every village looks after a few tiles of its
//! own land, chosen at random.
//! - **Care** comes from knowledge (the ages), stability, its farm buildings, cohesion and the
//!   player's calm. A village with care improves the tile:
//!   - fields on grass (First Farmers, more with a Field);
//!   - irrigation near rivers, wells and aqueducts (Villages);
//!   - terraces on hills (Kingdoms);
//!   - managed woods, and replanting at forest edges (Temples & Faith);
//!   - land reclamation, which turns desert and dust back into grass (Industrial Revolution).
//! - **Strain** comes from crowding, hunger, raids, a Dark Age, drought and the player's stress.
//!   A village under strain wears the tile down:
//!   - worn grass turns to sand;
//!   - worn forest is cleared to grass;
//!   - worn hills stay hills, but poorer.
//!
//!   Woodcutters thin the forest they cut (`resources::harvest`), so over-cutting strips
//!   forests to grass, and that grass erodes in turn.
//!
//! Land that is neither tended nor strained creeps back toward natural. A well-run civilisation
//! visibly greens its land: fields, canals and terraces are drawn on it. A badly run one strips
//! it to dust and sand. Poorer land grows less food, so famine follows, and a village on
//! stripped land is restless.

use moc_planet::rng::Rng;

use crate::jobs::Building;
use crate::resources::{has, rev};
use crate::terrain::{FOREST, GRASS, HILLS, SAND, WATER};
use crate::world::*;

/// Fertility of land as the planet made it.
pub const NAT: u8 = 128;
/// Forest thinned below this is cleared to grass.
pub const STRIP: u8 = 60;
/// Grass worn below this turns to sand.
pub const DUST: u8 = 56;

#[inline]
fn hash2(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 13)
}

/// Food multiplier of a fertility byte.
#[inline]
pub fn mul(f: u8) -> f32 {
    f as f32 * (1.0 / NAT as f32)
}

/// The works of development (`rev::GREEN`'s n).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Work {
    Fields = 0,
    Irrigation = 1,
    Terraces = 2,
    Reforest = 3,
    Reclaim = 4,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LandStats {
    pub improved: u32,
    pub worn: u32,
    pub stripped: u32,
    pub eroded: u32,
    pub irrigated: u32,
    pub terraced: u32,
    pub reforested: u32,
    pub reclaimed: u32,
}

pub struct Land {
    rng: Rng,
    /// Smoothed fertility of each village's land (by village index; sampled).
    pub vfert: Vec<f32>,
    /// Smoothed fertility of each people's land.
    pub cfert: [f32; MAX_FACTIONS],
    /// Works each people has begun (bit per `Work`), for their first headline.
    begun: [u8; MAX_FACTIONS],
    /// Tiles each people wore out since its last headline about it, and when that was.
    dust: [u32; MAX_FACTIONS],
    dust_t: [f32; MAX_FACTIONS],
    born: [f32; MAX_FACTIONS],
    pub stats: LandStats,
}

impl Land {
    pub fn new(seed: u32) -> Land {
        Land {
            rng: Rng::new(seed ^ 0x6eed_1a4d),
            vfert: Vec::new(),
            cfert: [NAT as f32; MAX_FACTIONS],
            begun: [0; MAX_FACTIONS],
            dust: [0; MAX_FACTIONS],
            dust_t: [-1e9; MAX_FACTIONS],
            born: [f32::NAN; MAX_FACTIONS],
            stats: LandStats::default(),
        }
    }
}

/// Change a tile's kind (always walkable to walkable, so landmasses stay) and repaint it.
pub fn set_kind(w: &mut World, i: usize, kind: u8) {
    if w.terrain.kind[i] == kind || !w.terrain.walk[i] {
        return;
    }
    w.terrain.kind[i] = kind;
    if w.burning[i] == 0 {
        let c = w.terrain.base_colour((i % w.d.w) as i32, (i / w.d.w) as i32);
        w.terrain.rgba[i * 4..i * 4 + 4].copy_from_slice(&c);
    }
}

/// How well a village of people `f` looks after its land (0..1.2).
fn care(w: &World, f: &Faction, v: &Village) -> f32 {
    let know = (0.12 * (f.age as f32 - 2.0)).clamp(0.0, 1.0);
    let wgt = if f.is_player { 1.0 } else { 0.6 };
    let mood = (0.35 * w.inputs.calm.clamp(0.0, 1.0) - 0.25 * w.inputs.agitation()) * wgt;
    let works = 0.1 * has(w, v, Building::Field) as u8 as f32
        + 0.1 * has(w, v, Building::Mill) as u8 as f32
        + 0.1 * (has(w, v, Building::Well) || has(w, v, Building::Aqueduct)) as u8 as f32
        + 0.05 * has(w, v, Building::Monastery) as u8 as f32;
    let war = if w.at_war[f.id as usize] { 0.15 } else { 0.0 };
    let hungry = v.hungry as f32 / v.pop.max(1) as f32;
    (know * (0.35 + 0.65 * f.metrics.stability) + works + 0.15 * f.traits.cohesion + mood - war - 0.3 * hungry).clamp(0.0, 1.2)
}

/// How hard a village is working its land (0..1.5): overcrowding, hunger, raids, war, a Dark
/// Age, drought and the god's stress.
fn strain(w: &World, f: &Faction, v: &Village) -> f32 {
    let wgt = if f.is_player { 1.0 } else { 0.6 };
    let crowd = v.pop as f32 / v.cap.max(1) as f32;
    let hunger = v.hungry as f32 / v.pop.max(1) as f32;
    let people = (v.pop as f32 / 8.0).min(1.0);
    let raided = if w.time - v.raided_t < 30.0 { 0.25 } else { 0.0 };
    let dark = if f.dark_t > 0.0 { 0.2 } else { 0.0 };
    let dry = if f.drought_t > 0.0 { 0.25 } else { 0.0 };
    let war = if w.at_war[f.id as usize] { 0.2 } else { 0.0 };
    // Foragers tread lightly; farmers and cities work the land hard.
    let footprint = 0.4 + 0.15 * (f.age.min(4) as f32);
    (footprint * people * (0.15 + 1.5 * (crowd - 0.85).max(0.0) + 0.9 * hunger) + raided + dark + dry + war + 0.3 * w.inputs.agitation() * wgt).clamp(0.0, 1.5)
}

fn river_or_water_near(w: &World, x: i32, y: i32, r: i32) -> bool {
    (-r..=r).any(|oy| (-r..=r).any(|ox| w.terrain.river_at(x + ox, y + oy) || w.terrain.kind_at(x + ox, y + oy) == WATER))
}

fn forest_round(w: &World, x: i32, y: i32) -> usize {
    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)].iter().filter(|(ox, oy)| w.terrain.kind_at(x + ox, y + oy) == FOREST).count()
}

/// A village tends tile i: returns the work done, if any.
fn improve(w: &mut World, i: usize, f: u8, v: &Village) -> Option<Work> {
    let age = w.factions[f as usize].age;
    let (x, y) = ((i % w.d.w) as i32, (i / w.d.w) as i32);
    let fe = w.food.fert[i];
    let kind = w.terrain.kind[i];
    let up = |w: &mut World, cap: u8, step: u8| -> bool {
        let fe = w.food.fert[i];
        if fe < cap {
            w.food.fert[i] = fe.saturating_add(step).min(cap);
            true
        } else {
            false
        }
    };
    match kind {
        GRASS => {
            // Replanting at a forest's edge (from the Temples & Faith age).
            if age >= 10 && fe <= NAT + 20 && forest_round(w, x, y) >= 4 && w.land.rng.chance(0.2) {
                set_kind(w, i, FOREST);
                w.food.fert[i] = NAT;
                return Some(Work::Reforest);
            }
            let irrigable = age >= 5 && (river_or_water_near(w, x, y, 2) || ((has(w, v, Building::Well) || has(w, v, Building::Aqueduct)) && river_or_water_near(w, x, y, 5)));
            let field = age >= 4 && has(w, v, Building::Field);
            let cap: u8 = if irrigable {
                212 + if has(w, v, Building::Aqueduct) { 16 } else { 0 }
            } else if field {
                170
            } else if age >= 4 {
                150
            } else {
                138
            };
            let cap = if age >= 15 { cap.saturating_add(16) } else { cap };
            if up(w, cap, 14) {
                return Some(if irrigable && w.food.fert[i] > 170 { Work::Irrigation } else { Work::Fields });
            }
            None
        }
        HILLS => {
            let terraces = age >= 9 && (has(w, v, Building::Field) || has(w, v, Building::Mill));
            if up(w, if terraces { 226 } else { 136 }, 10) {
                return Some(if terraces && w.food.fert[i] > 150 { Work::Terraces } else { Work::Fields });
            }
            None
        }
        FOREST => {
            if up(w, if age >= 10 { 156 } else { NAT }, 8) {
                return Some(if age >= 10 { Work::Reforest } else { Work::Fields });
            }
            None
        }
        SAND => {
            // Land reclamation: dust and desert made farmland again.
            if age >= 15 && w.land.rng.chance(0.35) {
                set_kind(w, i, GRASS);
                w.food.fert[i] = 112;
                return Some(Work::Reclaim);
            }
            up(w, NAT, 6).then_some(Work::Fields)
        }
        _ => None,
    }
}

/// A village overworks tile i: returns true if the tile turned to dust or was stripped.
fn wear(w: &mut World, i: usize) -> bool {
    let fe = w.food.fert[i];
    match w.terrain.kind[i] {
        GRASS => {
            let n = fe.saturating_sub(14);
            if n < DUST {
                set_kind(w, i, SAND);
                w.food.fert[i] = 100;
                w.land.stats.eroded += 1;
                return true;
            }
            w.food.fert[i] = n;
        }
        FOREST => {
            let n = fe.saturating_sub(12);
            if n < STRIP {
                set_kind(w, i, GRASS);
                w.food.fert[i] = 96;
                w.land.stats.stripped += 1;
                return true;
            }
            w.food.fert[i] = n;
        }
        HILLS => w.food.fert[i] = fe.saturating_sub(8).max(40),
        SAND => w.food.fert[i] = fe.saturating_sub(6).max(30),
        _ => {}
    }
    false
}

/// Once a second: villages tend or wear their land; clear-cut forest falls; headlines.
pub fn tick(w: &mut World, dt: f32) {
    #[cfg(not(target_arch = "wasm32"))]
    let t0 = std::time::Instant::now();
    for f in 0..MAX_FACTIONS {
        let fac = &w.factions[f];
        if !fac.alive || w.land.born[f].to_bits() != fac.born.to_bits() {
            w.land.born[f] = if fac.alive { fac.born } else { f32::NAN };
            w.land.begun[f] = 0;
            w.land.dust[f] = 0;
            w.land.cfert[f] = NAT as f32;
        }
    }
    // Forest the woodcutters cut too thin is cleared.
    let strip = std::mem::take(&mut w.res.strip);
    for &i in &strip {
        let i = i as usize;
        if w.terrain.kind[i] == FOREST && w.food.fert[i] < STRIP && w.burning[i] == 0 {
            set_kind(w, i, GRASS);
            w.food.fert[i] = 96;
            w.land.stats.stripped += 1;
            let o = w.owner[i];
            if o != NO_F {
                w.land.dust[o as usize] += 1;
            }
        }
    }
    w.res.strip = strip;
    w.res.strip.clear();
    if w.land.vfert.len() != w.villages.len() {
        w.land.vfert.resize(w.villages.len(), NAT as f32);
    }
    let mut csum = [0.0f32; MAX_FACTIONS];
    let mut cn = [0.0f32; MAX_FACTIONS];
    let mut done: [u8; MAX_FACTIONS] = [0; MAX_FACTIONS];
    let mut news_at: [(i32, i32); MAX_FACTIONS] = [(0, 0); MAX_FACTIONS];
    for vi in 0..w.villages.len() {
        if !w.villages[vi].alive {
            continue;
        }
        let v = w.villages[vi].clone();
        let f = v.owner;
        let fac = &w.factions[f as usize];
        let (c, s) = (care(w, fac, &v), strain(w, fac, &v));
        let samples = 3 + (v.radius / 2).max(0) as usize;
        let r = v.radius;
        for _ in 0..samples {
            let ox = w.land.rng.below((2 * r + 1) as usize) as i32 - r;
            let oy = w.land.rng.below((2 * r + 1) as usize) as i32 - r;
            let i = w.d.idx(v.x + ox, v.y + oy);
            if w.claim[i] != vi as u16 || !w.terrain.walk[i] || w.burning[i] != 0 {
                continue;
            }
            let roll = w.land.rng.f32();
            if roll < 0.3 * c {
                if let Some(work) = improve(w, i, f, &v) {
                    w.land.stats.improved += 1;
                    match work {
                        Work::Irrigation => w.land.stats.irrigated += 1,
                        Work::Terraces => w.land.stats.terraced += 1,
                        Work::Reforest => w.land.stats.reforested += 1,
                        Work::Reclaim => w.land.stats.reclaimed += 1,
                        Work::Fields => {}
                    }
                    let bit = 1u8 << work as u8;
                    if w.land.begun[f as usize] & bit == 0 && done[f as usize] == 0 {
                        w.land.begun[f as usize] |= bit;
                        done[f as usize] = work as u8 + 1;
                        news_at[f as usize] = ((i % w.d.w) as i32, (i / w.d.w) as i32);
                    }
                }
            } else if roll < 0.3 * c + 0.35 * s {
                w.land.stats.worn += 1;
                if wear(w, i) {
                    w.land.dust[f as usize] += 1;
                }
            } else if w.food.fert[i] < NAT && w.terrain.kind[i] != SAND {
                // Fallow land creeps back.
                w.food.fert[i] += 1;
            }
            let fe = w.food.fert[i] as f32;
            let vf = &mut w.land.vfert[vi];
            *vf += (fe - *vf) * 0.04;
        }
        let vf = w.land.vfert[vi];
        csum[f as usize] += vf;
        cn[f as usize] += 1.0;
        // Stripped land breeds discontent.
        let poor = ((104.0 - vf) / 104.0).max(0.0);
        if poor > 0.0 {
            let vil = &mut w.villages[vi];
            vil.unrest = (vil.unrest + 0.01 * dt * poor).min(1.5);
        }
    }
    for f in 0..MAX_FACTIONS {
        if cn[f] > 0.0 {
            w.land.cfert[f] = csum[f] / cn[f];
        }
    }
    // Headlines: a people's first irrigation, terraces, replanting, reclamation; land wearing out.
    let now = w.time;
    for f in 0..MAX_FACTIONS as u8 {
        if !w.factions[f as usize].alive {
            continue;
        }
        let d = done[f as usize];
        if d > 1 {
            let (x, y) = news_at[f as usize];
            w.log_x(rev::GREEN, f, NO_F, x, y, d as i32 - 1, w.land.stats.improved as i32);
        }
        if w.land.dust[f as usize] >= 25 && now - w.land.dust_t[f as usize] > 120.0 {
            let n = w.land.dust[f as usize];
            w.land.dust[f as usize] = 0;
            w.land.dust_t[f as usize] = now;
            let (x, y) = w.villages.get(w.factions[f as usize].capital as usize).filter(|v| v.alive).map(|v| (v.x, v.y)).unwrap_or(w.factions[f as usize].start);
            w.log(rev::ERODE, f, NO_F, x, y, n as i32);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        w.res.prof_ms += t0.elapsed().as_secs_f64() * 1000.0;
    }
}

impl World {
    /// Tint the view by fertility: fields, canals and terraces where the land is tended,
    /// dust where it is worn (drawn over the terrain, under everything else).
    pub fn draw_land(&mut self) {
        use moc_planet::{VH, VW};
        let (cx, cy) = (self.cam.ix(), self.cam.iy());
        let d = self.d;
        let fert = &self.food.fert;
        let kind = &self.terrain.kind;
        let walk = &self.terrain.walk;
        let data = &mut self.view.data;
        let blend = |px: &mut [u8], c: [u8; 3], a: u32| {
            for k in 0..3 {
                px[k] = ((c[k] as u32 * a + px[k] as u32 * (255 - a)) / 255) as u8;
            }
        };
        for sy in 0..VH {
            let py = d.wrap_y(cy + sy as i32);
            let row = py as usize * d.w;
            for sx in 0..VW {
                let px = d.wrap_x(cx + sx as i32);
                let i = row + px as usize;
                let fe = fert[i];
                if fe == NAT || !walk[i] {
                    continue;
                }
                let o = (sy * VW + sx) * 4;
                let p = &mut data[o..o + 3];
                let k = kind[i];
                let (gx, gy) = (px as u32, py as u32);
                if fe > NAT {
                    let t = ((fe - NAT) as u32).min(100);
                    match k {
                        GRASS if fe >= 156 => {
                            // Field plots of 4×3 tiles (lush, young and ripening crops) with
                            // furrows, and a grid of canals where it is irrigated.
                            let h = hash2(gx / 4, gy / 3);
                            let c = [[92, 182, 58], [140, 204, 74], [190, 190, 80]][(h % 3) as usize];
                            blend(p, c, 110 + t / 2);
                            if gy % 3 == 2 {
                                blend(p, [52, 118, 40], 60);
                            }
                            if fe >= 190 && (gx % 8 == 0 || gy % 6 == 0) {
                                blend(p, [74, 136, 204], 190);
                            }
                        }
                        GRASS => blend(p, [76, 178, 56], 30 + t * 4),
                        HILLS => {
                            // Terraces: green steps on the hillside.
                            if gy % 2 == 0 {
                                blend(p, [104, 166, 64], 60 + t * 150 / 100);
                            } else {
                                blend(p, [96, 86, 56], t * 90 / 100);
                            }
                        }
                        FOREST => blend(p, [24, 92, 36], t * 140 / 100),
                        _ => blend(p, [120, 170, 80], t),
                    }
                } else {
                    // Worn land: dust and bare earth.
                    let t = ((NAT - fe) as u32).min(90);
                    let speck = hash2(gx, gy) % 4 == 0;
                    match k {
                        FOREST => blend(p, [120, 98, 62], t * 160 / 90),
                        SAND => blend(p, [196, 168, 116], t * 120 / 90),
                        _ => blend(p, if speck { [140, 112, 74] } else { [184, 156, 104] }, t * 190 / 90),
                    }
                }
            }
        }
    }
}
