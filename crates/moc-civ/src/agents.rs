//! What agents do each step: needs (food, health, age), movement on the wrapping planet, and
//! the job behaviours (gatherer, warrior, settler); then fights between hostile agents.

use std::f32::consts::PI;

use crate::planet::{Dims, Paths};
use crate::politics;
use crate::terrain::{defence, speed_mul, Terrain};
use crate::world::*;

const DIRS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

pub const SPEED_GATHER: f32 = 6.5;
pub const SPEED_WARRIOR: f32 = 7.5;
pub const SPEED_SETTLER: f32 = 7.0;
pub const SPEED_FLEE: f32 = 9.0;

#[inline]
pub fn tile(a: &Agent, d: Dims) -> (i32, i32) {
    (d.wrap_x(a.x.floor() as i32), (a.y.floor() as i32).clamp(0, d.h as i32 - 1))
}

/// Point an agent at a new goal tile (keeps its progress if the goal is unchanged).
pub fn set_goal(a: &mut Agent, d: Dims, x: i32, y: i32) {
    let (x, y) = (d.wrap_x(x), y.clamp(0, d.h as i32 - 1));
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
    let dy = y1 - y0;
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
    let dy = gy - cy;
    if dx == 0 && dy == 0 {
        return None;
    }
    let ang = (dy as f32).atan2(dx as f32);
    let k = ((ang / (PI / 4.0)).round() as i32).rem_euclid(8);
    let mut fallback = None;
    for off in [0, 1, -1, 2, -2, 3, -3] {
        let (ox, oy) = DIRS[(k + off).rem_euclid(8) as usize];
        let (nx, ny) = (t.d.wrap_x(cx + ox), cy + oy);
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
/// mountains (see `ROUTE_*`).
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
    let ddy = ty - a.y;
    let len = (ddx * ddx + ddy * ddy).sqrt();
    let hurt = if a.wound == Wound::Wounded { 0.6 } else { 1.0 };
    let step = speed * hurt * speed_mul(t.kind_at(a.nx, a.ny)) * dt;
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
    a.y = (a.y + ddy / len * step).clamp(0.0, d.h as f32 - 0.01);
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

/// Needs, movement and job behaviour for every agent.
pub fn update(w: &mut World, dt: f32) {
    let now = w.time;
    let dm = w.d;
    let regen = if w.rain_t > 0.0 { 3.0 } else { 1.0 };
    let yield_mul = 1.0 + 0.6 * w.inputs.focus.clamp(0.0, 1.0);
    let mut deaths: Vec<(usize, Cause)> = Vec::new();
    let mut settle: Vec<usize> = Vec::new();
    let mut loot: Vec<(usize, u16, u8)> = Vec::new();
    {
        let World { agents, terrain, paths, villages, factions, food, burning, rng, marches, owner, .. } = w;
        for i in 0..agents.len() {
            let a = &mut agents[i];
            if !a.alive {
                continue;
            }
            // ---- needs ----
            a.age += dt;
            a.fight = (a.fight - dt).max(0.0);
            a.food = (a.food - EAT_RATE * dt).max(0.0);
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
            if a.hp <= 0.0 {
                deaths.push((i, if on_fire { Cause::Fire } else { Cause::Starved }));
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
                if a.home == NO_V {
                    if a.job != Job::Settler {
                        a.job = Job::Settler;
                        a.state = State::Idle;
                        a.tries = 0;
                        a.timer = 0.5;
                    }
                } else if a.job == Job::Settler && a.state != State::ToSite {
                    a.job = Job::Gatherer;
                    a.state = State::ToHome;
                } else if !matches!(a.state, State::March | State::Siege | State::ToSite) {
                    a.state = State::ToHome;
                }
            }
            let (hx, hy) = if a.home != NO_V {
                let v = &villages[a.home as usize];
                (v.x, v.y)
            } else {
                (cx, cy)
            };
            let near_home = a.home != NO_V && dm.dist(a.x, a.y, hx as f32 + 0.5, hy as f32 + 0.5) <= 1.6;
            // Home restores the spirit and mends the wounded.
            a.morale = (a.morale + dt * if near_home { 0.1 } else { 0.012 }).min(1.0);
            if near_home && a.wound == Wound::Wounded && a.food > 0.3 {
                a.hp = (a.hp + dt * 0.05).min(1.0);
            }
            if near_home && a.job != Job::Settler {
                let v = &mut villages[a.home as usize];
                if a.carry > 0.0 {
                    v.food += a.carry;
                    a.carry = 0.0;
                }
                if a.food < 0.7 {
                    let eat = (1.0 - a.food).min(v.food).max(0.0);
                    v.food -= eat;
                    a.food += eat;
                }
            }

            // ---- fleeing (fire, lightning, warriors) ----
            if a.state == State::Flee {
                a.timer -= dt;
                let arrived = walk(a, terrain, paths, SPEED_FLEE, dt, ROUTE_GREEDY);
                if a.timer <= 0.0 || arrived || a.stuck > 8 {
                    a.state = if a.job == Job::Settler { State::Idle } else { State::ToHome };
                }
                continue;
            }

            // ---- warriors in a fight chase their enemy ----
            if let Some((ex, ey)) = a.engage {
                set_goal(a, dm, ex, ey);
                walk(a, terrain, paths, SPEED_WARRIOR, dt, ROUTE_GREEDY);
                continue;
            }

            let fert = factions[a.faction as usize].traits.fertility;
            match a.job {
                Job::Gatherer => match a.state {
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
                            let got = food.take(ti, terrain.kind[ti], now, regen, CARRY_MAX);
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
                        let v = &villages[a.home as usize];
                        let r = v.radius + 2;
                        let mut best = None;
                        let mut bs = 0.0f32;
                        for _ in 0..8 {
                            let ox = rng.below((2 * r + 1) as usize) as i32 - r;
                            let oy = rng.below((2 * r + 1) as usize) as i32 - r;
                            if ox * ox + oy * oy > r * r || (ox == 0 && oy == 0) {
                                continue;
                            }
                            let (x, y) = (dm.wrap_x(v.x + ox), v.y + oy);
                            if !terrain.walkable(x, y) {
                                continue;
                            }
                            let ti = dm.idx(x, y);
                            let o = owner[ti];
                            if (o != NO_F && o != a.faction) || burning[ti] != 0 {
                                continue;
                            }
                            let lvl = food.level(ti, terrain.kind[ti], now, regen);
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
                Job::Warrior => {
                    if a.food < 0.25 && a.state != State::ToHome && a.home != NO_V {
                        a.state = State::ToHome;
                    }
                    let ms = &marches[a.faction as usize];
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
                            let m = ms.iter().find(|m| m.village == a.target).copied();
                            let Some(m) = m else {
                                a.state = State::ToHome;
                                a.target = NO_V;
                                continue;
                            };
                            let d = dm.dist(a.x, a.y, m.x as f32 + 0.5, m.y as f32 + 0.5);
                            if a.state == State::March {
                                set_goal(a, dm, m.x, m.y);
                                let arrived = walk(a, terrain, paths, SPEED_WARRIOR, dt, ROUTE_FIELD);
                                if arrived || d < 2.2 {
                                    a.state = State::Siege;
                                    a.timer = 0.0;
                                } else if a.stuck > STUCK_LIMIT * 3 {
                                    a.state = State::ToHome;
                                }
                            } else if m.raid {
                                if a.carry <= 0.0 {
                                    loot.push((i, m.village, m.enemy));
                                }
                                a.state = State::ToHome;
                            } else {
                                a.timer -= dt;
                                if a.timer <= 0.0 {
                                    let (x, y) = (m.x + rng.below(3) as i32 - 1, m.y + rng.below(3) as i32 - 1);
                                    if terrain.walkable(x, y) {
                                        set_goal(a, dm, x, y);
                                    }
                                    a.timer = 0.8;
                                }
                                walk(a, terrain, paths, 3.0, dt, ROUTE_GREEDY);
                                if d > 4.5 {
                                    a.state = State::March;
                                }
                            }
                        }
                        _ => {
                            if !a.garrison && !ms.is_empty() {
                                let m = ms[a.id as usize % ms.len()];
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
                Job::Settler => match a.state {
                    State::ToSite => {
                        if walk(a, terrain, paths, SPEED_SETTLER, dt, ROUTE_GREEDY) || a.stuck > STUCK_LIMIT {
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
            }
        }
    }
    for (i, c) in deaths {
        w.kill(i, c);
    }
    for (i, v, enemy) in loot {
        let take = w.villages[v as usize].food.clamp(0.0, 1.5);
        w.villages[v as usize].food -= take;
        w.agents[i].carry += take.max(0.01);
        let f = w.agents[i].faction;
        if !w.rel(f, enemy).raid_looted {
            w.rel_mut(f, enemy).raid_looted = true;
            let (x, y) = (w.villages[v as usize].x, w.villages[v as usize].y);
            w.log(ev::RAID, f, enemy, x, y, take.round() as i32);
            w.rel_mut(enemy, f).tension += 0.2;
        }
    }
    for i in settle {
        politics::settle(w, i);
    }
}

/// Hostile agents near each other fight, one attacker against one defender at a time.
/// Warriors chase enemies within 6 tiles; civilians run from warriors, and so do warriors
/// whose morale breaks. Damage depends on the attacker's job, its civilisation's violence,
/// its morale and wounds, and the defender's terrain; getting hurt costs morale, a kill
/// raises it and shakes the victim's comrades. Merciful civilisations (low violence) leave
/// civilians alone. Anyone below `WOUND_HP` is wounded before they die.
pub fn combat(w: &mut World, dt: f32) {
    if !w.at_war.iter().any(|&b| b) {
        for a in w.agents.iter_mut() {
            a.engage = None;
        }
        return;
    }
    let n = w.agents.len();
    let dm = w.d;
    for i in 0..n {
        let (ax, ay, fa, job, alive, morale, wound) = {
            let a = &w.agents[i];
            (a.x, a.y, a.faction, a.job, a.alive, a.morale, a.wound)
        };
        if !alive || !w.at_war[fa as usize] {
            w.agents[i].engage = None;
            continue;
        }
        let fighter = job.fights();
        let routed = fighter && (morale < ROUT_MORALE || (wound == Wound::Wounded && morale < 0.45));
        let violence = w.factions[fa as usize].traits.violence;
        let range = if fighter && !routed { 6.0 } else { 2.5 };
        let mut best: Option<usize> = None;
        let mut bd = range;
        {
            let agents = &w.agents;
            let hostile = &w.hostile;
            w.grid.near(ax, ay, range, |j| {
                if j == i {
                    return;
                }
                let b = &agents[j];
                if !b.alive || b.faction == fa || !hostile[fa as usize * MAX_FACTIONS + b.faction as usize] {
                    return;
                }
                let d = dm.dist(ax, ay, b.x, b.y);
                if fighter && !b.job.fights() && violence < 0.45 && d > 1.3 {
                    return;
                }
                if d < bd {
                    bd = d;
                    best = Some(j);
                }
            });
        }
        let Some(j) = best else {
            w.agents[i].engage = None;
            continue;
        };
        let (bx, by, fb, bjob) = {
            let b = &w.agents[j];
            (b.x, b.y, b.faction, b.job)
        };
        if (!fighter && bjob.fights()) || routed {
            // Run home, away from the spears.
            let a = &mut w.agents[i];
            a.engage = None;
            if a.state != State::Flee {
                a.state = State::Flee;
                a.timer = if routed { 3.0 } else { 1.5 };
                let (cx, cy) = tile(a, dm);
                let ox = -dm.dx(ax, bx).signum() as i32;
                let oy = -(by - ay).signum() as i32;
                set_goal(a, dm, cx + ox * 6, cy + oy * 6);
            }
            continue;
        }
        if bd <= 1.3 {
            let hurt = if wound == Wound::Wounded { 0.5 } else { 1.0 };
            let cover = defence(w.terrain.kind_at(bx.floor() as i32, by.floor() as i32));
            let hit = job.strength() * (0.55 + 0.9 * violence) * (0.6 + 0.4 * morale) * hurt * w.rng.range(0.6, 1.4) / cover;
            {
                let b = &mut w.agents[j];
                b.hp -= hit * dt;
                b.morale = (b.morale - hit * dt * 0.6).max(0.0);
                b.fight = 0.25;
            }
            w.agents[i].fight = 0.25;
            if w.rng.chance(dt * 6.0) {
                w.effect(ax + dm.dx(ax, bx) * 0.5, (ay + by) * 0.5, 0, 0.3, 1.0);
            }
            if w.agents[j].hp <= 0.0 && w.agents[j].alive {
                w.kill(j, Cause::Combat);
                w.factions[fa as usize].kills += 1;
                w.factions[fb as usize].last_attacker = fa;
                w.agents[i].morale = (w.agents[i].morale + 0.1).min(1.0);
                // A fallen comrade shakes those who saw it.
                let mut near = Vec::new();
                w.grid.near(bx, by, 4.0, |k| near.push(k));
                for k in near {
                    let c = &mut w.agents[k];
                    if c.alive && c.faction == fb && dm.dist(bx, by, c.x, c.y) <= 4.0 {
                        c.morale = (c.morale - 0.08).max(0.0);
                    }
                }
            }
        }
        w.agents[i].engage = if fighter { Some((dm.wrap_x(bx.floor() as i32), by.floor() as i32)) } else { None };
    }
}
