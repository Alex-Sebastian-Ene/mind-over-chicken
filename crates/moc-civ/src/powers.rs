//! The god's hand: lightning from a blink, and the camera casts (#33/#49). Coordinates are
//! planet pixels.

use crate::agents::{set_goal, tile};
use crate::politics::{make_peace, raze};
use crate::terrain::flammable;
use crate::world::*;

pub const CAST_FIREBALL: u32 = 1;
pub const CAST_FOOD: u32 = 2;
pub const CAST_PEACE: u32 = 3;
pub const CAST_DISASTER: u32 = 4;
pub const CAST_BLESS: u32 = 5;
pub const CAST_RAIN: u32 = 6;

fn ignite_round(w: &mut World, x: f32, y: f32, r: i32) {
    let (cx, cy) = (x.floor() as i32, y.floor() as i32);
    for oy in -r..=r {
        for ox in -r..=r {
            let yy = cy + oy;
            if ox * ox + oy * oy > r * r || yy < 0 || yy >= w.d.h as i32 {
                continue;
            }
            let i = w.d.idx(cx + ox, yy);
            if flammable(w.terrain.kind[i]) && w.rng.chance(0.75) {
                w.ignite_tile(i);
            }
        }
    }
}

/// Strike agents round (x, y): kill within `kill_r` (with probability `p`), scatter within `scare_r`.
fn strike(w: &mut World, x: f32, y: f32, kill_r: f32, p: f32, scare_r: f32, cause: Cause) -> i32 {
    let mut killed = 0;
    for i in 0..w.agents.len() {
        if !w.agents[i].alive {
            continue;
        }
        let d = w.d.dist(x, y, w.agents[i].x, w.agents[i].y);
        if d <= kill_r && w.rng.chance(p) {
            w.kill(i, cause);
            killed += 1;
        } else if d <= scare_r {
            let dm = w.d;
            let a = &mut w.agents[i];
            let (cx, cy) = tile(a, dm);
            let ox = dm.dx(x, a.x).signum() as i32;
            let oy = (a.y - y).signum() as i32;
            a.state = State::Flee;
            a.timer = 1.4;
            a.engage = None;
            set_goal(a, dm, cx + ox * 7, cy + oy * 7);
        }
    }
    killed
}

fn villages_within(w: &World, x: f32, y: f32, r: f32) -> Vec<u16> {
    w.villages
        .iter()
        .enumerate()
        .filter(|(_, v)| v.alive && w.d.dist(x, y, v.x as f32 + 0.5, v.y as f32 + 0.5) <= r)
        .map(|(i, _)| i as u16)
        .collect()
}

/// A blink: lightning at a planet point. Kills whoever it hits, scatters the rest, starts a
/// fire, and frightens nearby villages back into line (unrest falls).
pub fn lightning(w: &mut World, x: f32, y: f32) -> i32 {
    let x = w.d.wrap_x(x.floor() as i32) as f32 + 0.5;
    let y = y.clamp(0.0, w.d.h as f32 - 1.0);
    let killed = strike(w, x, y, 2.0, 1.0, 9.0, Cause::Smitten);
    ignite_round(w, x, y, 1);
    for v in villages_within(w, x, y, 2.5) {
        let vil = &mut w.villages[v as usize];
        vil.hp -= 0.35;
        vil.food = (vil.food - 2.0).max(0.0);
    }
    for v in villages_within(w, x, y, 16.0) {
        let vil = &mut w.villages[v as usize];
        vil.unrest = (vil.unrest - 0.15).max(0.0);
    }
    w.effect(x, y, 1, 0.6, 5.0);
    w.log(ev::SMITE, NO_F, NO_F, x as i32, y as i32, killed);
    killed
}

/// A camera cast at a planet point. Returns how many it affected (agents or villages).
pub fn cast(w: &mut World, kind: u32, x: f32, y: f32) -> i32 {
    let x = w.d.wrap_x(x.floor() as i32) as f32 + 0.5;
    let y = y.clamp(0.0, w.d.h as f32 - 1.0);
    let n = match kind {
        CAST_FIREBALL => {
            let k = strike(w, x, y, 4.0, 1.0, 11.0, Cause::Smitten);
            ignite_round(w, x, y, 3);
            for v in villages_within(w, x, y, 4.5) {
                w.villages[v as usize].hp -= 0.6;
                if w.villages[v as usize].hp <= 0.0 {
                    raze(w, v, NO_F);
                }
            }
            w.effect(x, y, 1, 0.9, 9.0);
            k
        }
        CAST_FOOD => {
            let (cx, cy) = (x as i32, y as i32);
            for oy in -7..=7 {
                for ox in -7..=7 {
                    let yy = cy + oy;
                    if ox * ox + oy * oy <= 49 && yy >= 0 && yy < w.d.h as i32 && w.terrain.walkable(cx + ox, yy) {
                        let i = w.d.idx(cx + ox, yy);
                        w.food.set(i, 1.5, w.time);
                    }
                }
            }
            let vs = villages_within(w, x, y, 25.0);
            for &v in &vs {
                w.villages[v as usize].food += 6.0;
            }
            w.effect(x, y, 4, 1.5, 8.0);
            vs.len() as i32
        }
        CAST_PEACE => {
            // Every war touching a village within 60 tiles ends.
            let vs = villages_within(w, x, y, 60.0);
            let mut sides: Vec<u8> = vs.iter().map(|&v| w.villages[v as usize].owner).collect();
            sides.sort_unstable();
            sides.dedup();
            let mut ended = 0;
            for &a in &sides {
                for b in 0..MAX_FACTIONS as u8 {
                    if a != b && (w.rel(a, b).war > 0 || w.rel(b, a).war > 0) {
                        make_peace(w, a, b);
                        ended += 1;
                    }
                    if a != b && w.rel(a, b).met {
                        let r = w.rel_mut(a, b);
                        r.tension = r.tension.min(0.0) - 0.3;
                        r.cooldown = r.cooldown.max(45.0);
                    }
                }
            }
            for a in w.agents.iter_mut() {
                a.engage = None;
            }
            w.effect(x, y, 5, 2.0, 30.0);
            ended
        }
        CAST_DISASTER => {
            let k = strike(w, x, y, 14.0, 0.6, 20.0, Cause::Disaster);
            ignite_round(w, x, y, 2);
            for v in villages_within(w, x, y, 14.0) {
                let vil = &mut w.villages[v as usize];
                vil.food *= 0.4;
                vil.hp -= 0.5;
                vil.unrest = (vil.unrest + 0.3).min(1.5);
                if vil.hp <= 0.0 {
                    raze(w, v, NO_F);
                }
            }
            w.effect(x, y, 2, 2.5, 16.0);
            k
        }
        CAST_BLESS => {
            let vs = villages_within(w, x, y, 40.0);
            for &v in &vs {
                let vil = &mut w.villages[v as usize];
                vil.unrest = (vil.unrest - 0.5).max(0.0);
                vil.food += 3.0;
                vil.hp = 1.0;
            }
            for a in w.agents.iter_mut().filter(|a| a.alive && w.d.dist(x, y, a.x, a.y) <= 40.0) {
                a.hp = 1.0;
                a.food = 1.0;
            }
            w.effect(x, y, 3, 2.0, 20.0);
            vs.len() as i32
        }
        CAST_RAIN => {
            w.rain_t = w.rain_t.max(18.0);
            let (cx, cy) = (x as i32, y as i32);
            for oy in -30..=30 {
                for ox in -30..=30 {
                    let yy = cy + oy;
                    if yy >= 0 && yy < w.d.h as i32 && ox * ox + oy * oy <= 900 && w.terrain.walkable(cx + ox, yy) {
                        let i = w.d.idx(cx + ox, yy);
                        let k = w.terrain.kind[i];
                        let lvl = w.food.level(i, k, w.time, 1.0);
                        w.food.set(i, (lvl + 0.4).min(crate::terrain::food_max(k).max(lvl)), w.time);
                    }
                }
            }
            w.effect(x, y, 2, 1.0, 30.0);
            1
        }
        _ => 0,
    };
    w.log(ev::CAST, NO_F, NO_F, x as i32, y as i32, kind as i32 * 1000 + n.clamp(0, 999));
    n
}

/// Calibration hook (#54): set the player's civilisation from a payload of the README's
/// three traits. The dummy run doesn't call it; all four civilisations stay random.
pub fn set_player_traits(w: &mut World, friendliness: f32, aggressiveness: f32, violence: f32) -> bool {
    let Some(p) = w.player() else { return false };
    let f = &mut w.factions[p as usize];
    f.traits = Traits { friendliness, aggressiveness, violence, ..f.traits }.clamped();
    f.seed_source = SeedSource::Calibration;
    true
}

/// The mind read: the player's first ten seconds of brain data shape their civilisation.
/// Calm → friendliness, pulse → aggressiveness, blinks → chaos (violence up, cohesion down),
/// focus → fertility (industry).
pub fn seed_player_mind(w: &mut World, calm: f32, focus: f32, hr_delta: f32, blinks: f32) -> bool {
    let Some(p) = w.player() else { return false };
    let pulse = (hr_delta / 25.0).clamp(0.0, 1.0);
    let chaos = (blinks / 8.0).clamp(0.0, 1.0);
    let f = &mut w.factions[p as usize];
    f.traits = Traits {
        friendliness: 0.2 + 0.7 * calm.clamp(0.0, 1.0),
        aggressiveness: 0.15 + 0.75 * pulse,
        violence: 0.2 + 0.6 * chaos,
        fertility: 0.3 + 0.6 * focus.clamp(0.0, 1.0),
        cohesion: 0.8 - 0.5 * chaos,
    }
    .clamped();
    f.seed_source = SeedSource::Mind;
    true
}
