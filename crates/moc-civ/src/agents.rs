//! What agents do each step: needs (food, health, age, sleep, plague), movement on the
//! wrapping planet, and the jobs' behaviours (`jobs::Role`: gather, hunt, work, haul, fight,
//! lead, settle); then fights between agents who are at odds, whether of two civilisations
//! or of one (`conflict`).

use std::f32::consts::PI;

use crate::ages_table::{terrain_mul, HEALERS_FROM_AGE};
use crate::conflict::{formation_goal, inner_stance, is_flank, is_leader, is_reserve, take_captive, RAID};
use crate::jobs::{has, info, Building, Job, Out, Role, Site};
use crate::life;
use crate::planet::{Dims, Paths};
use crate::politics;
use crate::terrain::{defence, speed_mul, Terrain, FOREST, GRASS, HILLS, MOUNTAIN, RIVER_SPEED};
use crate::world::*;

const DIRS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

pub const SPEED_GATHER: f32 = 6.5;
pub const SPEED_WARRIOR: f32 = 7.5;
pub const SPEED_SETTLER: f32 = 7.0;
pub const SPEED_FLEE: f32 = 9.0;
pub const SPEED_HAUL: f32 = 6.0;
pub const SPEED_RAIL: f32 = 9.5;
pub const SPEED_SAIL: f32 = 6.0;

#[inline]
pub fn tile(a: &Agent, d: Dims) -> (i32, i32) {
    (d.wrap_x(a.x.floor() as i32), d.wrap_y(a.y.floor() as i32))
}

/// Point an agent at a new goal tile (keeps its progress if the goal is unchanged).
pub fn set_goal(a: &mut Agent, d: Dims, x: i32, y: i32) {
    let (x, y) = (d.wrap_x(x), d.wrap_y(y));
    if a.gx != x || a.gy != y {
        a.gx = x;
        a.gy = y;
        a.best = i32::MAX;
        a.stuck = 0;
    }
}

/// Is the straight line between two tiles all walkable land? (Settlers and gatherers only
/// pick targets they can see, so simple steering always gets them there.)
pub fn los(t: &Terrain, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let dx = t.d.dxi(x0, x1);
    let dy = t.d.dyi(y0, y1);
    let n = dx.abs().max(dy.abs());
    for s in 1..=n {
        let k = s as f32 / n as f32;
        let x = (x0 as f32 + dx as f32 * k).round() as i32;
        let y = (y0 as f32 + dy as f32 * k).round() as i32;
        if !t.walkable(x, y) {
            return false;
        }
    }
    true
}

/// One step toward the goal, preferring the straight direction, sliding round obstacles,
/// and avoiding stepping straight back.
pub fn greedy(t: &Terrain, cx: i32, cy: i32, gx: i32, gy: i32, px: i32, py: i32) -> Option<(i32, i32)> {
    let dx = t.d.dxi(cx, gx);
    let dy = t.d.dyi(cy, gy);
    if dx == 0 && dy == 0 {
        return None;
    }
    let ang = (dy as f32).atan2(dx as f32);
    let k = ((ang / (PI / 4.0)).round() as i32).rem_euclid(8);
    let mut fallback = None;
    for off in [0, 1, -1, 2, -2, 3, -3] {
        let (ox, oy) = DIRS[(k + off).rem_euclid(8) as usize];
        let (nx, ny) = (t.d.wrap_x(cx + ox), t.d.wrap_y(cy + oy));
        if !t.walkable(nx, ny) {
            continue;
        }
        if nx == px && ny == py {
            fallback.get_or_insert((nx, ny));
            continue;
        }
        return Some((nx, ny));
    }
    fallback
}

/// How a walker finds its way: straight-line steering only, distance fields once steering
/// gets stuck (trips home), or distance fields for any long trip (marches).
pub const ROUTE_GREEDY: u8 = 0;
pub const ROUTE_STUCK: u8 = 1;
pub const ROUTE_FIELD: u8 = 2;

fn next_tile(a: &Agent, cx: i32, cy: i32, t: &Terrain, paths: &mut Paths, route: u8) -> Option<(i32, i32)> {
    let far = t.d.cheb(cx, cy, a.gx, a.gy);
    let want = match route {
        ROUTE_FIELD => far > 8 || a.stuck > 6,
        ROUTE_STUCK => a.stuck > 6,
        _ => false,
    };
    if want && far < crate::planet::FIELD_RADIUS {
        if let Some(n) = paths.get(t.d, &t.walk, a.gx, a.gy).and_then(|f| f.next_step(cx, cy)) {
            return Some(n);
        }
    }
    greedy(t, cx, cy, a.gx, a.gy, a.px, a.py)
}

/// Move toward the goal at `speed` tiles/s. Returns true when standing on the goal tile.
/// `route`: the goal is a village, so wrap-aware distance fields may route round coasts and
/// mountains (see `ROUTE_*`). Rivers are fords: slow going.
pub fn walk(a: &mut Agent, t: &Terrain, paths: &mut Paths, speed: f32, dt: f32, route: u8) -> bool {
    let d = t.d;
    if a.nx == NO_TILE {
        let (cx, cy) = tile(a, d);
        if cx == a.gx && cy == a.gy {
            return true;
        }
        match next_tile(a, cx, cy, t, paths, route) {
            Some((nx, ny)) => {
                a.px = cx;
                a.py = cy;
                a.nx = nx;
                a.ny = ny;
            }
            None => {
                a.stuck = a.stuck.saturating_add(4);
                return false;
            }
        }
    }
    let (tx, ty) = (a.nx as f32 + 0.5, a.ny as f32 + 0.5);
    let ddx = d.dx(a.x, tx);
    let ddy = d.dy(a.y, ty);
    let len = (ddx * ddx + ddy * ddy).sqrt();
    let hurt = if a.wound == Wound::Wounded { 0.6 } else { 1.0 };
    let ford = if t.river_at(a.nx, a.ny) { RIVER_SPEED } else { 1.0 };
    let step = speed * hurt * ford * speed_mul(t.kind_at(a.nx, a.ny)) * dt;
    if len <= step.max(1e-4) {
        a.x = d.wrap_xf(tx);
        a.y = ty;
        let (nx, ny) = (a.nx, a.ny);
        a.nx = NO_TILE;
        let dg = d.cheb(nx, ny, a.gx, a.gy);
        if dg < a.best {
            a.best = dg;
            a.stuck = 0;
        } else {
            a.stuck = a.stuck.saturating_add(1);
        }
        return dg == 0;
    }
    a.x = d.wrap_xf(a.x + ddx / len * step);
    a.y = d.wrap_yf(a.y + ddy / len * step);
    false
}

/// Sail (or row) straight toward the goal over water and land alike. True on arrival.
pub fn sail(a: &mut Agent, d: Dims, speed: f32, dt: f32) -> bool {
    let (tx, ty) = (a.gx as f32 + 0.5, a.gy as f32 + 0.5);
    let ddx = d.dx(a.x, tx);
    let ddy = d.dy(a.y, ty);
    let len = (ddx * ddx + ddy * ddy).sqrt();
    let step = speed * dt;
    a.nx = NO_TILE;
    if len <= step.max(0.05) {
        a.x = d.wrap_xf(tx);
        a.y = ty;
        return true;
    }
    a.x = d.wrap_xf(a.x + ddx / len * step);
    a.y = d.wrap_yf(a.y + ddy / len * step);
    false
}

/// Nearest living village of `f`, preferring the agent's own landmass.
pub fn nearest_own(villages: &[Village], t: &Terrain, f: u8, x: f32, y: f32) -> Option<u16> {
    let land = t.land(x as i32, y as i32);
    let mut best: Option<(bool, f32, u16)> = None;
    for (i, v) in villages.iter().enumerate() {
        if !v.alive || v.owner != f {
            continue;
        }
        let same = land != 0 && t.land(v.x, v.y) == land;
        let d = t.d.dist(x, y, v.x as f32 + 0.5, v.y as f32 + 0.5);
        let better = match best {
            None => true,
            Some((bs, bd, _)) => (same && !bs) || (same == bs && d < bd),
        };
        if better {
            best = Some((same, d, i as u16));
        }
    }
    best.map(|b| b.2)
}

/// Is (x, y) a place a gatherer of this kind works?
fn site_fits(t: &Terrain, site: Site, x: i32, y: i32) -> bool {
    match site {
        Site::Forest => t.kind_at(x, y) == FOREST,
        Site::Hills => matches!(t.kind_at(x, y), HILLS) || (-1..=1).any(|oy| (-1..=1).any(|ox| t.kind_at(x + ox, y + oy) == MOUNTAIN)),
        Site::Coast => t.near_water(x, y, 1),
        Site::Field => t.kind_at(x, y) == GRASS || t.river_at(x, y),
        Site::Wild => matches!(t.kind_at(x, y), FOREST | GRASS),
        Site::Any | Site::Home => true,
    }
}

/// An arrival at the end of a haul, settled after the agent loop.
struct Arrival {
    agent: usize,
    village: u16,
}

/// Needs, movement and job behaviour for every agent.
pub fn update(w: &mut World, dt: f32) {
    let now = w.time;
    let dm = w.d;
    let regen = if w.rain_t > 0.0 { 3.0 } else { 1.0 };
    let focus = w.inputs.focus.clamp(0.0, 1.0);
    let yield_mul = 1.0 + 0.6 * focus;
    let night = w.night;
    let mut deaths: Vec<(usize, Cause)> = Vec::new();
    let mut settle: Vec<usize> = Vec::new();
    let mut loot: Vec<(usize, u16, u8)> = Vec::new();
    let mut arrivals: Vec<Arrival> = Vec::new();
    {
        let World { agents, terrain, paths, villages, factions, food, burning, rng, marches, owner, strifes, rel, busy, fight_level, res, .. } = w;
        for i in 0..agents.len() {
            let a = &mut agents[i];
            if !a.alive {
                continue;
            }
            // ---- needs ----
            a.age += dt;
            a.fight = (a.fight - dt).max(0.0);
            a.brawl = (a.brawl - dt).max(0.0);
            let asleep_now = a.state == State::Sleep;
            a.food = (a.food - EAT_RATE * dt * if asleep_now { 0.6 } else { 1.0 }).max(0.0);
            if a.food <= 0.0 {
                a.hp -= dt * 0.1;
            } else if a.hp < 1.0 {
                a.hp = (a.hp + dt * 0.03).min(1.0);
            }
            if a.hp < WOUND_HP {
                a.wound = Wound::Wounded;
            } else if a.wound == Wound::Wounded && a.hp >= HEALED_HP {
                a.wound = Wound::Healthy;
            }
            if a.prisoner > 0.0 {
                // Forced labour ends; the captive stays among its captors.
                a.prisoner = (a.prisoner - dt).max(0.0);
            }
            let (cx, cy) = tile(a, dm);
            let on_fire = burning[dm.idx(cx, cy)] != 0;
            if on_fire {
                a.hp -= dt * 0.8;
                if a.state != State::Flee {
                    a.state = State::Flee;
                    a.timer = 1.2;
                    let (ox, oy) = DIRS[rng.below(8)];
                    set_goal(a, dm, cx + ox * 5, cy + oy * 5);
                }
            }
            let mut sick = false;
            if let Some(v) = villages.get(a.home as usize) {
                if v.alive && v.plague > 0.2 {
                    a.hp -= dt * 0.012 * v.plague * if a.age > 60.0 { 2.0 } else { 1.0 };
                    sick = true;
                }
            }
            if a.hp <= 0.0 {
                deaths.push((i, if on_fire { Cause::Fire } else if sick && a.food > 0.0 { Cause::Plague } else { Cause::Starved }));
                continue;
            }
            if a.age >= a.life {
                deaths.push((i, Cause::OldAge));
                continue;
            }

            // ---- home ----
            let home_ok = (a.home as usize) < villages.len() && {
                let v = &villages[a.home as usize];
                v.alive && v.owner == a.faction
            };
            if !home_ok {
                a.home = nearest_own(villages, terrain, a.faction, a.x, a.y).unwrap_or(NO_V);
                let role = a.job.role();
                if a.home == NO_V {
                    if role != Role::Settle {
                        a.job = Job::Settle;
                        a.state = State::Idle;
                        a.tries = 0;
                        a.timer = 0.5;
                    }
                } else if role == Role::Settle && !matches!(a.state, State::ToSite | State::Sail) {
                    a.job = Job::Forage;
                    a.state = State::ToHome;
                } else if !matches!(a.state, State::March | State::Siege | State::ToSite | State::Sail) {
                    a.state = State::ToHome;
                }
            }
            let (hx, hy) = if a.home != NO_V {
                let v = &villages[a.home as usize];
                (v.x, v.y)
            } else {
                (cx, cy)
            };
            let fac = &factions[a.faction as usize];
            let age = fac.age;
            let job = a.job;
            let ji = info(job);
            let near_home = a.home != NO_V && dm.dist(a.x, a.y, hx as f32 + 0.5, hy as f32 + 0.5) <= 1.6;
            // Home restores the spirit and mends the wounded (faster with healers and wells).
            a.morale = (a.morale + dt * if near_home { 0.1 } else { 0.012 }).min(1.0);
            if near_home && a.wound == Wound::Wounded && a.food > 0.3 {
                let heal = villages.get(a.home as usize).map_or(0.0, |v| v.fx.heal * 3.0 + if age >= HEALERS_FROM_AGE { 0.02 } else { 0.0 });
                a.hp = (a.hp + dt * (0.05 + heal)).min(1.0);
            }
            if near_home && ji.role != Role::Settle {
                let v = &mut villages[a.home as usize];
                if a.carry > 0.0 {
                    match if a.state == State::ToHome && job.fights() { Out::Food } else { ji.out } {
                        Out::Goods => {
                            v.goods += a.carry;
                            factions[a.faction as usize].prod_acc += a.carry;
                        }
                        Out::Goats => a.goats += a.carry,
                        _ => v.food += a.carry,
                    }
                    a.carry = 0.0;
                }
                if a.food < 0.7 {
                    // Cooked meals at the hearth and the market go further.
                    let meal = 1.0 + 0.15 * (v.built & (1 << Building::Campfire as u8) != 0) as u8 as f32 + 0.1 * (v.built & (1 << Building::Market as u8) != 0) as u8 as f32;
                    let eat = ((1.0 - a.food) / meal).min(v.food).max(0.0);
                    v.food -= eat;
                    a.food = (a.food + eat * meal).min(1.0);
                }
            }
            let fac = &factions[a.faction as usize];

            // ---- fleeing (fire, lightning, warriors) ----
            if a.state == State::Flee {
                a.timer -= dt;
                let arrived = walk(a, terrain, paths, SPEED_FLEE, dt, ROUTE_GREEDY);
                if a.timer <= 0.0 || arrived || a.stuck > 8 {
                    a.state = if ji.role == Role::Settle { State::Idle } else { State::ToHome };
                }
                continue;
            }

            // ---- fighters in a fight chase their enemy (or stand and shoot) ----
            if let Some((ex, ey)) = a.engage {
                set_goal(a, dm, ex, ey);
                walk(a, terrain, paths, SPEED_WARRIOR, dt, ROUTE_GREEDY);
                continue;
            }

            // ---- night: from the Hearth Clans on, people sleep at home ----
            let lights = villages.get(a.home as usize).map_or(false, |v| v.fx.lights);
            let marching = matches!(a.state, State::March | State::Siege | State::ToSite | State::Sail | State::Haul);
            let fighting = busy[a.faction as usize] && fight_level[a.faction as usize] >= 2;
            if life::asleep(age, night, lights) && !marching && !(job.fights() && fighting) {
                if near_home || a.home == NO_V {
                    a.state = State::Sleep;
                    continue;
                }
                a.state = State::ToHome;
                set_goal(a, dm, hx, hy);
                walk(a, terrain, paths, SPEED_GATHER, dt, ROUTE_STUCK);
                continue;
            } else if a.state == State::Sleep {
                a.state = State::Idle;
                a.timer = rng.range(0.0, 1.0);
            }

            let fert = fac.traits.fertility;
            // Hunters fight when there is a march for them; otherwise they hunt.
            let my_marches = |ms: &[March], a: &Agent| -> bool {
                let in_strife = a.camp != 0 && strifes.get(a.camp as usize - 1).map_or(false, |s| s.alive && s.level >= RAID);
                ms.iter().any(|m| if in_strife { m.camp == a.camp && m.side == a.side } else { m.camp == 0 })
            };
            let role = match ji.role {
                Role::Hunt if my_marches(&marches[a.faction as usize], a) || matches!(a.state, State::March | State::Siege) => Role::Fight,
                Role::Hunt => Role::Gather,
                r => r,
            };
            let festival = villages.get(a.home as usize).map_or(false, |v| v.festival_t > 0.0) && !job.fights();
            match role {
                Role::Gather => match a.state {
                    State::ToFood => {
                        if walk(a, terrain, paths, SPEED_GATHER, dt, ROUTE_GREEDY) {
                            a.state = State::Gather;
                            a.timer = GATHER_TIME;
                        } else if a.stuck > STUCK_LIMIT {
                            a.state = State::ToHome;
                        }
                    }
                    State::Gather => {
                        a.timer -= dt;
                        if a.timer <= 0.0 {
                            let ti = dm.idx(cx, cy);
                            let dry: f32 = if fac.drought_t > 0.0 { 0.5 } else { 1.0 };
                            let got = match ji.out {
                                Out::Food if ji.site == Site::Coast => ji.rate * dry.max(0.8),
                                Out::Food => food.take(ti, terrain.kind[ti], now, regen, CARRY_MAX) * ji.rate * dry,
                                _ => ji.rate * if fac.strike_t > 0.0 { 0.3 } else { 1.0 } * (1.0 + 0.5 * focus * (age == 3 || age == 8) as u8 as f32),
                            };
                            // #165: a deposit of the kind this job works pays more (and drains).
                            let got = got * crate::resources::harvest(res, &mut food.fert, terrain, ti, cx, cy, job, age);
                            a.carry += got * yield_mul * (0.8 + 0.4 * fert);
                            a.state = State::ToHome;
                        }
                    }
                    State::ToHome => {
                        if a.home == NO_V {
                            a.state = State::Idle;
                            continue;
                        }
                        set_goal(a, dm, hx, hy);
                        let arrived = walk(a, terrain, paths, SPEED_GATHER, dt, ROUTE_STUCK);
                        if arrived || near_home {
                            a.state = State::Idle;
                            a.timer = rng.range(0.2, 0.8);
                        } else if a.stuck > STUCK_LIMIT * 3 {
                            a.state = State::Idle;
                            a.timer = 1.0;
                            a.stuck = 0;
                        }
                    }
                    State::Wander => {
                        a.timer -= dt;
                        if walk(a, terrain, paths, 4.0, dt, ROUTE_GREEDY) || a.timer <= 0.0 || a.stuck > 6 {
                            a.state = State::ToHome;
                        }
                    }
                    _ => {
                        a.timer -= dt;
                        if a.timer > 0.0 || a.home == NO_V {
                            continue;
                        }
                        if !near_home {
                            a.state = State::ToHome;
                            continue;
                        }
                        if festival {
                            a.state = State::Work;
                            a.timer = rng.range(2.0, 4.0);
                            continue;
                        }
                        let v = &villages[a.home as usize];
                        let site = ji.site;
                        let (rmin, r) = match site {
                            Site::Wild => (6, v.radius + 10),
                            Site::Field => (1, v.radius),
                            Site::Forest | Site::Coast => (1, v.radius + 6),
                            Site::Hills => (1, v.radius + 9),
                            _ => (1, v.radius + 2),
                        };
                        let mut best = None;
                        let mut bs = f32::MIN;
                        // #165: the deposit the village works for this job comes first, if it is fit to work.
                        if let Some((x, y)) = crate::resources::gather_spot(res, a.home, site, rng, dm) {
                            let ti = dm.idx(x, y);
                            let o = owner[ti];
                            if terrain.walk[ti] && site_fits(terrain, site, x, y) && o == a.faction && burning[ti] == 0 && los(terrain, cx, cy, x, y) {
                                let lvl = if ji.out == Out::Food && site != Site::Coast { food.level(ti, terrain.kind[ti], now, regen) } else { 0.5 };
                                if lvl >= 0.12 {
                                    bs = lvl.min(CARRY_MAX) + 0.25;
                                    best = Some((x, y));
                                }
                            }
                        }
                        for _ in 0..8 {
                            let ox = rng.below((2 * r + 1) as usize) as i32 - r;
                            let oy = rng.below((2 * r + 1) as usize) as i32 - r;
                            let d2 = ox * ox + oy * oy;
                            if d2 > r * r || d2 < rmin * rmin || (ox == 0 && oy == 0) {
                                continue;
                            }
                            let (x, y) = (dm.wrap_x(v.x + ox), v.y + oy);
                            if !terrain.walkable(x, y) || !site_fits(terrain, site, x, y) {
                                continue;
                            }
                            let ti = dm.idx(x, y);
                            let o = owner[ti];
                            if (o != NO_F && o != a.faction) || burning[ti] != 0 || (site == Site::Field && o != a.faction) {
                                continue;
                            }
                            let lvl = if ji.out == Out::Food && site != Site::Coast { food.level(ti, terrain.kind[ti], now, regen) } else { 0.5 };
                            if lvl < 0.12 {
                                continue;
                            }
                            let s = lvl.min(CARRY_MAX) - 0.015 * ox.abs().max(oy.abs()) as f32;
                            if s > bs && los(terrain, cx, cy, x, y) {
                                bs = s;
                                best = Some((x, y));
                            }
                        }
                        if let Some((x, y)) = best {
                            set_goal(a, dm, x, y);
                            a.state = State::ToFood;
                        } else {
                            let (x, y) = (dm.wrap_x(cx + rng.below(7) as i32 - 3), cy + rng.below(7) as i32 - 3);
                            if terrain.walkable(x, y) {
                                set_goal(a, dm, x, y);
                                a.state = State::Wander;
                                a.timer = 2.0;
                            } else {
                                a.timer = 0.4;
                            }
                        }
                    }
                },
                Role::Work | Role::Lead => {
                    if a.home == NO_V {
                        continue;
                    }
                    let at_home = dm.dist(a.x, a.y, hx as f32 + 0.5, hy as f32 + 0.5) <= 3.2;
                    if !at_home {
                        a.state = State::ToHome;
                        set_goal(a, dm, hx, hy);
                        if walk(a, terrain, paths, SPEED_GATHER, dt, ROUTE_STUCK) || a.stuck > STUCK_LIMIT * 3 {
                            a.state = State::Work;
                            a.stuck = 0;
                        }
                        continue;
                    }
                    a.state = State::Work;
                    if !festival {
                        // Output while at work.
                        let v = &mut villages[a.home as usize];
                        let bonus = match job {
                            Job::Knap => has(v, Building::Workshop, age),
                            Job::Scholar => has(v, Building::University, age),
                            Job::Programmer => has(v, Building::DataCentre, age),
                            Job::Artist => has(v, Building::Theatre, age),
                            Job::Engineer => has(v, Building::Aqueduct, age),
                            _ => false,
                        };
                        let k = dt * if bonus { 1.5 } else { 1.0 };
                        let lore_k = (1.0 + focus) * if fac.censor_t > 0.0 { 0.5 } else { 1.0 };
                        let goods_k = (if fac.strike_t > 0.0 { 0.3 } else { 1.0 }) * if fac.blackout_t > 0.0 { 0.5 } else { 1.0 } * (1.0 + 0.5 * focus * (age == 3 || age == 8) as u8 as f32);
                        for (out, rate) in [(ji.out, ji.rate), ji.side] {
                            let r = rate * k;
                            match out {
                                Out::Food => v.food += r,
                                Out::Goods => {
                                    v.goods += r * goods_k;
                                    factions[a.faction as usize].prod_acc += r * goods_k;
                                }
                                Out::Lore => factions[a.faction as usize].metrics.knowledge += r * lore_k,
                                Out::Goats => a.goats += r,
                                Out::Faith => v.faith = (v.faith + r).min(1.0),
                                Out::Order => v.unrest = (v.unrest - r).max(0.0),
                                Out::Heal => {
                                    v.plague = (v.plague - r * (1.0 + focus)).max(0.0);
                                }
                                Out::None => {}
                            }
                        }
                    }
                    // Mill about the village.
                    a.timer -= dt;
                    if a.timer <= 0.0 {
                        let (x, y) = (dm.wrap_x(hx + rng.below(5) as i32 - 2), hy + rng.below(5) as i32 - 2);
                        if terrain.walkable(x, y) {
                            set_goal(a, dm, x, y);
                        }
                        a.timer = rng.range(1.5, 4.0);
                    }
                    walk(a, terrain, paths, 2.5, dt, ROUTE_GREEDY);
                }
                Role::Haul => match a.state {
                    State::Haul => {
                        let tv = a.target;
                        let Some(v) = villages.get(tv as usize).filter(|v| v.alive) else {
                            a.state = State::ToHome;
                            continue;
                        };
                        let speed = if job == Job::Rail { SPEED_RAIL } else { SPEED_HAUL };
                        set_goal(a, dm, v.x, v.y);
                        let arrived = walk(a, terrain, paths, speed, dt, ROUTE_STUCK);
                        if arrived || dm.dist(a.x, a.y, v.x as f32 + 0.5, v.y as f32 + 0.5) <= 1.6 {
                            arrivals.push(Arrival { agent: i, village: tv });
                            a.state = State::ToHome;
                        } else if a.stuck > STUCK_LIMIT * 3 {
                            a.state = State::ToHome;
                        }
                    }
                    State::Sail => {
                        if sail(a, dm, SPEED_SAIL, dt) {
                            if a.target != NO_V {
                                arrivals.push(Arrival { agent: i, village: a.target });
                            }
                            // Home by sea.
                            a.target = NO_V;
                            set_goal(a, dm, hx, hy);
                            a.state = if dm.dist(a.x, a.y, hx as f32, hy as f32) > 3.0 && job == Job::Sailor { State::Wander } else { State::ToHome };
                            a.timer = 30.0;
                        }
                    }
                    State::Wander if job == Job::Sailor => {
                        // The voyage home.
                        if sail(a, dm, SPEED_SAIL, dt) {
                            a.state = State::Idle;
                            a.timer = 1.0;
                        }
                    }
                    State::Wander => {
                        a.timer -= dt;
                        if walk(a, terrain, paths, SPEED_HAUL, dt, ROUTE_GREEDY) || a.timer <= 0.0 || a.stuck > 10 {
                            if job == Job::Explorer {
                                factions[a.faction as usize].metrics.knowledge += ji.rate * (1.0 + focus);
                            }
                            a.state = State::ToHome;
                        }
                    }
                    State::ToHome => {
                        if a.home == NO_V {
                            a.state = State::Idle;
                            continue;
                        }
                        set_goal(a, dm, hx, hy);
                        let speed = if job == Job::Rail { SPEED_RAIL } else { SPEED_HAUL };
                        if walk(a, terrain, paths, speed, dt, ROUTE_STUCK) || near_home || a.stuck > STUCK_LIMIT * 3 {
                            a.state = State::Idle;
                            a.timer = rng.range(1.0, 3.0);
                            a.stuck = 0;
                        }
                    }
                    _ => {
                        a.timer -= dt;
                        if a.timer > 0.0 || a.home == NO_V {
                            continue;
                        }
                        if !near_home {
                            a.state = State::ToHome;
                            continue;
                        }
                        let home = &villages[a.home as usize];
                        let land = terrain.land(home.x, home.y);
                        let me = a.faction;
                        // Where to: by job.
                        let mut best: Option<(f32, u16)> = None;
                        for (vi, v) in villages.iter().enumerate() {
                            if !v.alive || vi as u16 == a.home {
                                continue;
                            }
                            let d = dm.dist(home.x as f32, home.y as f32, v.x as f32, v.y as f32);
                            let same_land = terrain.land(v.x, v.y) == land;
                            let foreign = v.owner != me;
                            let friendly = foreign && {
                                let r = rel[me as usize * MAX_FACTIONS + v.owner as usize];
                                r.met && r.war == 0 && r.tension < 0.25 && rel[v.owner as usize * MAX_FACTIONS + me as usize].war == 0
                            };
                            let score = match job {
                                Job::Carry if !foreign && same_land && d < 50.0 => Some(home.food - v.food - d * 0.05),
                                Job::Trade if same_land && d < 70.0 && (friendly || !foreign) => Some(if friendly { 10.0 } else { 0.0 } - d * 0.05),
                                Job::Pilgrim if !foreign && same_land && d < 80.0 => Some(v.faith * 10.0 + if factions[me as usize].capital as usize == vi { 5.0 } else { 0.0 } - d * 0.03),
                                Job::Rail if !foreign && same_land && d < 90.0 => Some(d * 0.1 + rng.f32()),
                                Job::Sailor if (friendly || !foreign) && v.coast && d > 20.0 && d < 110.0 => Some(if friendly { 5.0 } else { 0.0 } + if same_land { 0.0 } else { 4.0 } - d * 0.02 + rng.f32()),
                                _ => None,
                            };
                            if let Some(s) = score {
                                if best.map_or(true, |b| s > b.0) {
                                    best = Some((s, vi as u16));
                                }
                            }
                        }
                        match (job, best) {
                            (Job::Carry, Some((s, _))) if s < 3.0 => {
                                a.timer = 3.0;
                            }
                            (Job::Explorer, _) => {
                                let ang = rng.range(0.0, std::f32::consts::TAU);
                                let reach = rng.range(30.0, 60.0);
                                let (x, y) = (hx + (ang.cos() * reach) as i32, hy + (ang.sin() * reach) as i32);
                                if terrain.walkable(x, y) {
                                    set_goal(a, dm, x, y);
                                    a.state = State::Wander;
                                    a.timer = 25.0;
                                } else {
                                    a.timer = 1.0;
                                }
                            }
                            (_, Some((_, tv))) => {
                                a.target = tv;
                                let v = &villages[tv as usize];
                                set_goal(a, dm, v.x, v.y);
                                if job == Job::Carry {
                                    let take = (villages[a.home as usize].food - 3.0).clamp(0.0, ji.rate);
                                    villages[a.home as usize].food -= take;
                                    a.carry = take;
                                }
                                a.state = if job == Job::Sailor { State::Sail } else { State::Haul };
                            }
                            _ => a.timer = 4.0,
                        }
                    }
                },
                Role::Fight => {
                    if a.food < 0.25 && a.state != State::ToHome && a.home != NO_V {
                        a.state = State::ToHome;
                    }
                    let ms = &marches[a.faction as usize];
                    let (acamp, aside) = (a.camp, a.side);
                    let in_strife = acamp != 0 && strifes.get(acamp as usize - 1).map_or(false, |s| s.alive && s.level >= RAID);
                    let mine = |m: &&March| if in_strife { m.camp == acamp && m.side == aside } else { m.camp == 0 };
                    match a.state {
                        State::ToHome => {
                            if a.home == NO_V {
                                a.state = State::Idle;
                                continue;
                            }
                            set_goal(a, dm, hx, hy);
                            if walk(a, terrain, paths, SPEED_WARRIOR, dt, ROUTE_STUCK) || near_home || a.stuck > STUCK_LIMIT * 3 {
                                a.state = State::Idle;
                                a.timer = 0.5;
                            }
                        }
                        State::March | State::Siege => {
                            let m = ms.iter().filter(mine).find(|m| (m.village != NO_V && m.village == a.target) || (m.village == NO_V && a.target == NO_V && m.field)).copied();
                            let Some(m) = m else {
                                a.state = State::ToHome;
                                a.target = NO_V;
                                continue;
                            };
                            let (gx, gy) = formation_goal(age, a.id, &m);
                            let d = dm.dist(a.x, a.y, m.x as f32 + 0.5, m.y as f32 + 0.5);
                            if a.state == State::March {
                                set_goal(a, dm, gx, gy);
                                let arrived = walk(a, terrain, paths, SPEED_WARRIOR, dt, ROUTE_FIELD);
                                let at = dm.dist(a.x, a.y, gx as f32 + 0.5, gy as f32 + 0.5);
                                if arrived || at < 1.8 || (!m.field && d < 2.2) {
                                    a.state = State::Siege;
                                    a.timer = 0.0;
                                } else if a.stuck > STUCK_LIMIT * 3 {
                                    a.state = State::ToHome;
                                }
                            } else if m.raid && !m.field {
                                if a.carry <= 0.0 {
                                    loot.push((i, m.village, m.enemy));
                                }
                                a.state = State::ToHome;
                            } else {
                                // Hold the line (or the siege, or the field).
                                a.timer -= dt;
                                if a.timer <= 0.0 {
                                    let (x, y) = (gx + rng.below(3) as i32 - 1, gy + rng.below(3) as i32 - 1);
                                    if terrain.walkable(x, y) {
                                        set_goal(a, dm, x, y);
                                    }
                                    a.timer = 0.8;
                                }
                                walk(a, terrain, paths, 3.0, dt, ROUTE_GREEDY);
                                let back = dm.dist(a.x, a.y, gx as f32 + 0.5, gy as f32 + 0.5);
                                if back > 5.5 {
                                    a.state = State::March;
                                }
                            }
                        }
                        _ => {
                            let mut mine_it = ms.iter().filter(mine);
                            let count = ms.iter().filter(mine).count();
                            if (!a.garrison || in_strife) && count > 0 {
                                let m = *mine_it.nth(a.id as usize % count).unwrap();
                                a.target = m.village;
                                a.state = State::March;
                                continue;
                            }
                            // Peacetime: the garrison stands guard; the rest scout the land
                            // round home, which is how civilisations first meet.
                            a.timer -= dt;
                            if a.timer <= 0.0 {
                                let reach = if a.garrison { 4.0 } else { rng.range(10.0, 40.0) };
                                let ang = rng.range(0.0, std::f32::consts::TAU);
                                let (x, y) = (hx + (ang.cos() * reach) as i32, hy + (ang.sin() * reach) as i32);
                                if terrain.walkable(x, y) {
                                    set_goal(a, dm, x, y);
                                }
                                a.timer = if a.garrison { rng.range(1.5, 3.5) } else { rng.range(4.0, 8.0) };
                            }
                            let speed = if a.garrison { 3.5 } else { 5.0 };
                            walk(a, terrain, paths, speed, dt, ROUTE_GREEDY);
                        }
                    }
                }
                Role::Settle => match a.state {
                    State::ToSite => {
                        if walk(a, terrain, paths, SPEED_SETTLER, dt, ROUTE_GREEDY) || a.stuck > STUCK_LIMIT {
                            settle.push(i);
                        }
                    }
                    State::Sail => {
                        if sail(a, dm, SPEED_SAIL, dt) {
                            a.state = State::ToSite;
                            settle.push(i);
                        }
                    }
                    _ => {
                        a.timer -= dt;
                        if a.timer <= 0.0 {
                            settle.push(i);
                        }
                    }
                },
                Role::Hunt => {}
            }
        }
    }
    for (i, c) in deaths {
        w.kill(i, c);
    }
    for (i, v, enemy) in loot {
        let take = w.villages[v as usize].food.clamp(0.0, 1.5);
        w.villages[v as usize].food -= take;
        w.villages[v as usize].raided_t = w.time;
        w.agents[i].carry += take.max(0.01);
        let f = w.agents[i].faction;
        // Raiders also drive off goats and burn what will burn.
        let goats = (w.villages[v as usize].goats * 0.3).min(2.0);
        w.villages[v as usize].goats -= goats;
        w.agents[i].goats += goats;
        if f != enemy && !w.rel(f, enemy).raid_looted {
            w.rel_mut(f, enemy).raid_looted = true;
            let (x, y) = (w.villages[v as usize].x, w.villages[v as usize].y);
            w.log(ev::RAID, f, enemy, x, y, take.round() as i32);
            w.rel_mut(enemy, f).tension += 0.2;
            w.rel_mut(enemy, f).grudge += 0.05;
            let built = w.villages[v as usize].built;
            if built != 0 && w.rng.chance(0.35) {
                // A building burns: the newest one.
                let top = 63 - built.leading_zeros();
                w.villages[v as usize].built &= !(1u64 << top);
                let age = w.factions[enemy as usize].age;
                w.villages[v as usize].fx = crate::jobs::village_fx(w.villages[v as usize].built, age);
                let i = w.d.idx(x + 1, y);
                w.ignite_tile(i);
            }
        }
    }
    for arr in arrivals {
        arrive(w, arr.agent, arr.village);
    }
    for i in settle {
        politics::settle(w, i);
    }
}

/// A hauler reaches its destination: food delivered, goods traded, a pilgrimage made, a link
/// kept alive.
fn arrive(w: &mut World, i: usize, v: u16) {
    let (f, job, home, carry) = {
        let a = &w.agents[i];
        (a.faction, a.job, a.home, a.carry)
    };
    if !w.villages[v as usize].alive {
        return;
    }
    let now = w.time;
    let owner = w.villages[v as usize].owner;
    let rate = info(job).rate;
    w.villages[v as usize].linked_t = now;
    if let Some(h) = w.villages.get_mut(home as usize) {
        h.linked_t = now;
    }
    match job {
        Job::Carry => {
            w.villages[v as usize].food += carry;
            w.agents[i].carry = 0.0;
        }
        Job::Trade | Job::Sailor => {
            let foreign = owner != f;
            w.agents[i].goats += rate * if foreign { 1.0 } else { 0.5 };
            w.villages[v as usize].goats += rate * 0.3;
            if foreign {
                w.rel_mut(f, owner).traded_t = 0.0;
                w.rel_mut(owner, f).traded_t = 0.0;
                w.rel_mut(f, owner).tension -= 0.01;
                w.rel_mut(owner, f).tension -= 0.01;
                // Plague travels the trade routes.
                let (hp, vp) = (w.villages.get(home as usize).map_or(0.0, |h| h.plague), w.villages[v as usize].plague);
                if hp > 0.3 && vp < 0.05 && w.rng.chance(0.2) {
                    w.villages[v as usize].plague = 0.2;
                } else if vp > 0.3 && hp < 0.05 && w.rng.chance(0.2) {
                    if let Some(h) = w.villages.get_mut(home as usize) {
                        h.plague = 0.2;
                    }
                }
            }
        }
        Job::Pilgrim => {
            if let Some(h) = w.villages.get_mut(home as usize) {
                h.faith = (h.faith + rate).min(1.0);
            }
        }
        Job::Rail => {
            w.villages[v as usize].goods += rate;
            w.factions[f as usize].prod_acc += rate;
        }
        _ => {}
    }
}

/// Agents at odds near each other fight, one attacker against one defender at a time.
///
/// Who fights whom (`World::stance`, `conflict::inner_stance`): at a scuffle, agents brawl
/// without killing; at a feud, fighters kill fighters (violent civilisations ambush civilians);
/// at a raid, battle, war or civil war, anyone. Fighters chase enemies within 6 tiles, or within
/// their weapon's reach, `1.3 + 2·ln(range)` tiles of the age's weapon (`AgeDef::weapon`).
/// Civilians run from fighters, and so do fighters whose morale breaks.
///
/// Damage = job strength × prowess (0.6..1.4 per agent) × (0.55 + 0.9·violence) × (0.6 + 0.4·morale) × ½ if wounded
/// × power^0.35 (the attacker's weapon) ÷ power^0.25 (the defender's) ÷ cover, where cover is
/// the defender's terrain (hills 1.3, forest 1.2), or walls (1.8) at its own walled village,
/// × 1.25 in a shield wall (age 11+, a fighter at its place in the line), and an attack across
/// a river does × 0.7. From age 11 flankers hit an engaged enemy × 1.3.
///
/// Morale falls with hurt, with a comrade's death nearby (more for a leader's), with being
/// outnumbered and with a rout next to you; it rises with a kill and at home. Below
/// `ROUT_MORALE` a fighter runs. From age 4, merciful victors (violence < 0.75) take the beaten
/// alive as captives.
pub fn combat(w: &mut World, dt: f32) {
    if !w.busy.iter().any(|&b| b) {
        for a in w.agents.iter_mut() {
            a.engage = None;
        }
        return;
    }
    let n = w.agents.len();
    let dm = w.d;
    let tick = w.tick as usize;
    // Alternate the order each step, so nobody always strikes first.
    for k in 0..n {
        let i = if tick % 2 == 0 { k } else { n - 1 - k };
        let (ax, ay, fa, job, alive, morale, wound, prisoner, id, state, brawl, engaged) = {
            let a = &w.agents[i];
            (a.x, a.y, a.faction, a.job, a.alive, a.morale, a.wound, a.prisoner, a.id, a.state, a.brawl, a.engage.is_some() || a.fight > 0.0 || a.morale < ROUT_MORALE)
        };
        if !alive || !w.busy[fa as usize] || prisoner > 0.0 || state == State::Sail {
            w.agents[i].engage = None;
            continue;
        }
        // Those already in a fight look every step; the rest look round every other step
        // (fighters) or every third (civilians), which is plenty at these speeds.
        if !engaged && (id.wrapping_mul(2_654_435_761) as usize >> 7).wrapping_add(tick) % if job.fights() { 2 } else { 3 } != 0 {
            continue;
        }
        let fac = &w.factions[fa as usize];
        let age = fac.age;
        let violence = fac.traits.violence;
        let weapon = crate::ages::def(age).weapon;
        let fighter = job.fights();
        let reach = if fighter { 1.3 + 2.0 * weapon.range.max(1.0).ln() } else { 1.3 };
        let routed = fighter && (morale < ROUT_MORALE || (wound == Wound::Wounded && morale < 0.45));
        let reserve = fighter && age >= crate::ages_table::FORMATIONS_FROM_AGE && is_reserve(id) && matches!(state, State::March | State::Siege);
        let lethal_here = w.fight_level[fa as usize] >= 2;
        // Scuffles are rare and close: try one now and then.
        let want_brawl = brawl > 0.0 || w.rng.chance(dt * 0.35);
        if !fighter && !lethal_here && !want_brawl {
            w.agents[i].engage = None;
            continue;
        }
        let range = if fighter && !routed {
            if reserve {
                3.5
            } else {
                6.0f32.max(reach + 0.5).min(12.0)
            }
        } else {
            2.5
        };
        let mut best: Option<(usize, u8)> = None;
        let mut bd = range;
        let (mut foes, mut friends) = (0u32, 0u32);
        let range2 = range * range;
        {
            let agents = &w.agents;
            let stance = &w.stance;
            let me = &agents[i];
            let wr = &*w;
            w.grid.near(ax, ay, range, |j| {
                if j == i {
                    return;
                }
                let b = &agents[j];
                let (ddx, ddy) = (dm.dx(ax, b.x), dm.dy(ay, b.y));
                let d2 = ddx * ddx + ddy * ddy;
                if d2 > range2 || !b.alive || b.prisoner > 0.0 || b.state == State::Sail {
                    return;
                }
                let st = if b.faction == fa { inner_stance(wr, me, b) } else { stance[fa as usize * MAX_FACTIONS + b.faction as usize] };
                if st == 0 && !fighter {
                    return;
                }
                let d = d2.sqrt();
                if st == 0 {
                    if b.faction == fa && b.job.fights() && d <= 5.0 && (me.camp == 0 || b.camp == me.camp) {
                        friends += 1;
                    }
                    return;
                }
                if st >= 2 && b.job.fights() && d <= 5.0 {
                    foes += 1;
                }
                match st {
                    1 => {
                        if !want_brawl || d > 1.5 {
                            return;
                        }
                    }
                    2 => {
                        // A feud: fighters against fighters; only the violent ambush civilians.
                        if fighter && !b.job.fights() && (violence < 0.6 || d > 1.3) {
                            return;
                        }
                        if !fighter && !b.job.fights() {
                            return;
                        }
                    }
                    _ => {
                        if fighter && !b.job.fights() && violence < 0.45 && d > 1.3 {
                            return;
                        }
                    }
                }
                if d < bd {
                    bd = d;
                    best = Some((j, st));
                }
            });
        }
        if fighter && foes > 0 {
            // Outnumbered: the line wavers.
            let ratio = (foes + 1) as f32 / (friends + 1) as f32;
            let a = &mut w.agents[i];
            a.morale = (a.morale - dt * 0.06 * (ratio - 1.2).max(0.0)).max(0.0);
        }
        let Some((j, st)) = best else {
            w.agents[i].engage = None;
            continue;
        };
        let (bx, by, fb, bjob, bage_civ) = {
            let b = &w.agents[j];
            (b.x, b.y, b.faction, b.job, w.factions[b.faction as usize].age)
        };
        if ((!fighter && bjob.fights() && st >= 2) || routed) && st >= 2 {
            // Run home, away from the spears.
            let was = w.agents[i].state;
            {
                let a = &mut w.agents[i];
                a.engage = None;
                if a.state != State::Flee {
                    a.state = State::Flee;
                    a.timer = if routed { 3.0 } else { 1.5 };
                    let (cx, cy) = tile(a, dm);
                    let ox = -dm.dx(ax, bx).signum() as i32;
                    let oy = -dm.dy(ay, by).signum() as i32;
                    set_goal(a, dm, cx + ox * 6, cy + oy * 6);
                }
            }
            if routed && was != State::Flee {
                // A rout spreads down the line.
                w.stats.routs += 1;
                spread_morale(w, ax, ay, 3.0, fa, w.agents[i].camp, w.agents[i].side, -0.04);
            }
            continue;
        }
        if st == 1 {
            // A scuffle: fists and shoves; wounds, never deaths; a grudge.
            if bd <= 1.5 {
                if w.agents[i].brawl <= 0.0 {
                    w.stats.brawls += 1;
                    if fb != fa {
                        w.rel_mut(fb, fa).tension += 0.01;
                    } else if let Some(s) = (w.agents[i].camp as usize).checked_sub(1).and_then(|k| w.strifes.get_mut(k)) {
                        s.heat += 0.01;
                    }
                }
                let hurt = 0.18 * dt * w.rng.range(0.5, 1.5);
                for (k, t) in [(i, j), (j, i)] {
                    let x = &mut w.agents[k];
                    x.brawl = 2.0;
                    x.fight = 0.25;
                    if k == j || w.rng.chance(0.5) {
                        x.hp = (x.hp - hurt).max(0.35);
                    }
                    let _ = t;
                }
                w.agents[i].engage = Some((dm.wrap_x(bx.floor() as i32), dm.wrap_y(by.floor() as i32)));
            } else {
                w.agents[i].engage = None;
            }
            continue;
        }
        if bd <= reach.max(1.3) {
            let hurt = if wound == Wound::Wounded { 0.5 } else { 1.0 };
            let (btx, bty) = (dm.wrap_x(bx.floor() as i32), dm.wrap_y(by.floor() as i32));
            let mut cover = defence(w.terrain.kind_at(btx, bty));
            // Walls: a defender at its own walled village.
            if let Some(h) = w.villages.get(w.agents[j].home as usize) {
                if h.alive && h.owner == fb && h.fx.walls && dm.dist(bx, by, h.x as f32 + 0.5, h.y as f32 + 0.5) <= 3.0 {
                    cover = cover.max(terrain_mul::WALLS);
                }
            }
            // A shield wall (age 11+): a fighter at its place in the line.
            let mut steady = 1.0;
            {
                let b = &w.agents[j];
                if bage_civ >= crate::ages_table::FORMATIONS_FROM_AGE && bjob.fights() && matches!(b.state, State::March | State::Siege) && dm.dist(b.x, b.y, b.gx as f32 + 0.5, b.gy as f32 + 0.5) < 2.0 {
                    cover *= 1.25;
                    steady = 0.6;
                }
            }
            let (atx, aty) = (dm.wrap_x(ax.floor() as i32), ay.floor() as i32);
            let mx = dm.wrap_x((ax + dm.dx(ax, bx) * 0.5).floor() as i32);
            let my = dm.wrap_y((ay + dm.dy(ay, by) * 0.5).floor() as i32);
            let across = w.terrain.river_at(atx, aty) || (w.terrain.river_at(mx, my) && !w.terrain.river_at(btx, bty));
            let river = if across { terrain_mul::RIVER_CROSSING } else { 1.0 };
            let flank = if age >= crate::ages_table::FORMATIONS_FROM_AGE && is_flank(id) && w.agents[j].fight > 0.0 { 1.3 } else { 1.0 };
            let power = if fighter { weapon.power.powf(0.35) * crate::resources::arms(w, fa) } else { 1.0 };
            let armour = crate::ages::def(bage_civ).weapon.power.powf(0.25);
            let hit = job.strength() * prowess(id ^ w.seed) * (0.55 + 0.9 * violence) * (0.6 + 0.4 * morale) * hurt * w.rng.range(0.6, 1.4) * power / armour * river * flank / cover;
            let leader_hit;
            {
                let b = &mut w.agents[j];
                let was_ok = b.hp >= WOUND_HP;
                b.hp -= hit * dt;
                b.morale = (b.morale - hit * dt * 0.3 * steady).max(0.0);
                b.fight = 0.25;
                leader_hit = was_ok && b.hp < WOUND_HP && is_leader(b.job);
            }
            w.agents[i].fight = 0.25;
            if w.rng.chance(dt * 6.0) {
                w.effect(dm.wrap_xf(ax + dm.dx(ax, bx) * 0.5), dm.wrap_yf(ay + dm.dy(ay, by) * 0.5), 0, 0.3, 1.0);
            }
            let (bcamp, bside) = (w.agents[j].camp, w.agents[j].side);
            if leader_hit {
                spread_morale(w, bx, by, 8.0, fb, bcamp, bside, -0.05);
            }
            // The beaten may be taken alive (age 4+, a merciful victor, another people).
            let beaten = w.agents[j].hp < 0.3 && w.agents[j].hp > 0.0 && (!bjob.fights() || w.agents[j].morale < ROUT_MORALE);
            if beaten && fighter && fb != fa && age >= 4 && violence < 0.75 && w.rng.chance(dt * 1.5) {
                take_captive(w, j, fa);
                w.agents[i].engage = None;
                continue;
            }
            if w.agents[j].hp <= 0.0 && w.agents[j].alive {
                let leader = is_leader(bjob);
                w.kill(j, Cause::Combat);
                w.factions[fa as usize].kills += 1;
                w.factions[fb as usize].last_attacker = fa;
                w.agents[i].morale = (w.agents[i].morale + 0.1).min(1.0);
                if fb == fa && bcamp != 0 {
                    if let Some(s) = w.strifes.get_mut(bcamp as usize - 1) {
                        s.deaths[bside as usize & 1] += 1;
                    }
                } else if fb != fa {
                    w.rel_mut(fb, fa).grudge += 0.01;
                }
                // A fallen comrade shakes those who saw it; a fallen leader, the whole army.
                spread_morale(w, bx, by, if leader { 10.0 } else { 4.0 }, fb, bcamp, bside, if leader { -0.2 } else { -0.08 });
            }
        }
        let ranged = reach > 2.5 && bd <= reach * 0.9;
        w.agents[i].engage = if !fighter {
            None
        } else if ranged {
            // Archers, gunners and drones stand and shoot.
            Some(tile(&w.agents[i], dm))
        } else {
            Some((dm.wrap_x(bx.floor() as i32), dm.wrap_y(by.floor() as i32)))
        };
    }
}

/// Some fight better than others: 0.6..1.4, fixed per agent (`key`: its id mixed with the run seed).
#[inline]
pub fn prowess(key: u32) -> f32 {
    let mut h = key.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    0.6 + 0.8 * (h & 0xffff) as f32 / 65535.0
}

/// Change the morale of `f`'s people (on the same strife side) within `r` of (x, y).
fn spread_morale(w: &mut World, x: f32, y: f32, r: f32, f: u8, camp: u8, side: u8, by: f32) {
    let dm = w.d;
    let World { grid, agents, .. } = w;
    grid.near(x, y, r, |k| {
        let c = &mut agents[k];
        if c.alive && c.faction == f && (camp == 0 || (c.camp == camp && c.side == side)) && dm.dist(x, y, c.x, c.y) <= r {
            c.morale = (c.morale + by).clamp(0.0, 1.0);
        }
    });
}

/// Lightning, fire and storms break a charge: fighters near a strike lose heart, the side that
/// was hit most of all.
pub fn shock(w: &mut World, x: f32, y: f32, r: f32, struck: f32, others: f32) {
    let dm = w.d;
    // The struck side: whoever's fighters stood nearest the blow.
    let side = w
        .agents
        .iter()
        .filter(|a| a.alive && a.job.fights())
        .map(|a| (dm.dist(x, y, a.x, a.y), a.faction as usize))
        .filter(|&(d, _)| d <= r)
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, f)| f);
    for a in w.agents.iter_mut().filter(|a| a.alive && a.job.fights()) {
        if dm.dist(x, y, a.x, a.y) <= r {
            let k = if Some(a.faction as usize) == side { struck } else { others };
            a.morale = (a.morale - k).max(0.0);
            a.engage = None;
        }
    }
}

/// Force all in a radius to stop fighting (the peace cast).
pub fn calm_round(w: &mut World, x: f32, y: f32, r: f32) {
    let dm = w.d;
    for a in w.agents.iter_mut().filter(|a| a.alive && dm.dist(x, y, a.x, a.y) <= r) {
        a.engage = None;
        a.brawl = 0.0;
        if matches!(a.state, State::March | State::Siege) {
            a.state = State::ToHome;
        }
    }
}
