//! Villages, territory, contact, diplomacy, war, rebellion and collapse. Runs on the
//! bookkeeping tick (every `SLOW_DT` seconds); diplomacy every second.
//!
//! The rules, in numbers:
//! - **Growth:** a village spawns an agent every `SPAWN_EVERY` / (0.6 + 0.8·fertility) s
//!   (faster with the player's focus) while it has `SPAWN_COST` + 0.5 food and is under its cap
//!   (6 + 4·fertility, +4 for a capital, +½ per tile of radius over 5). At 55 % of cap with
//!   `SETTLER_COST` + 1 food it sends a settler 11–26 tiles out to found a village (≥ 10 tiles
//!   from any other). Villages claim land within 6 tiles and grow to 10 (capital 12).
//! - **Tension** (a's hostility toward a neighbour b, per second):
//!   0.04 · (aggressiveness + 0.8·hunger + border pressure + rebel grudge + god's mood
//!   − 0.8·friendliness − 0.25). Raid at 0.55 (if aggressiveness ≥ 0.45), war at 1.0.
//!   The god's mood is +0.7·agitation + 0.6·anger − 0.75·calm (full weight for the player's
//!   civilisation, 0.6 for the rest), where agitation is stress above resting (`Inputs`).
//! - **Peace:** weariness grows with time and losses; peace when
//!   weariness · (0.4 + mean friendliness) · (1 + 1.6·calm − 0.6·agitation − 0.6·anger) > 1
//!   after at least 20 s of war.
//! - **Capture:** hostile warriors within 3 tiles of a village with no defending warriors
//!   within 5 build siege at 0.35/s each; at `SIEGE_TO_CAPTURE` the village changes owner.
//! - **Unrest** per village settles (4 %/s) toward 0.5 · (crowding + 1.3·hunger + distance
//!   from capital + 2·war losses + the god's anger and agitation) · (1.35 − cohesion) − 0.25·calm.
//!   A non-capital village at `REBEL_UNREST` splits off with nearby restless villages (up to
//!   half the civilisation) as a new civilisation with drifted traits, which usually declares
//!   war on its parent. Unrest also rises with inequality and mourning and falls with
//!   festivals, chiefs, law, faith and parliaments (`ages::economy`, `life`), and roads shorten
//!   the distance that feeds it; an empire's far provinces feel it more (overextension).
//!
//! With the ages (#69): new agents take jobs by need (`jobs::choose`); villages put up their
//! age's buildings (`jobs::construct`); settlers go further and land claims grow from the
//! Villages age (5) on; colonists sail from the Age of Sail (13). Each second the ladder is
//! measured and climbed (`ages`), conflict levels and strifes move (`conflict`), and wars are
//! declared from age 7 (or after two battles). A civilisation fighting a civil war inside
//! itself is not also split by the unrest rule; the war decides it.

use std::f32::consts::TAU;

use crate::agents::{los, set_goal, tile};
use crate::conflict::{self, BATTLE, CIVIL_WAR, RAID};
use crate::jobs::{self, Job};
use crate::terrain::{food_max, SAND};
use crate::world::*;

pub const TENSION_RATE: f32 = 0.04;
/// How fast unrest settles toward its target (per second), and the target per unit of pressure.
pub const UNREST_RATE: f32 = 0.04;
pub const UNREST_GAIN: f32 = 0.5;
pub const NEAR_DIST: f32 = 42.0;

/// Bookkeeping, spread over four phases a quarter-second apart so no single frame carries it
/// all: villages (spawning, settlers, territory, unrest, sieges) at phases 0 and 2, first
/// contacts at 1, politics (borders, metrics, diplomacy, marches, trade; rebellion every
/// other time) at 3. So villages tick every `SLOW_DT`, politics every second.
pub fn phase_tick(w: &mut World) {
    let phase = w.slow_n % 4;
    w.slow_n += 1;
    match phase {
        0 | 2 => village_tick(w),
        1 => contacts(w),
        _ => politics_tick(w, (w.slow_n / 4) % 2 == 1),
    }
}

/// Every phase at once (tests and tools).
pub fn slow_tick(w: &mut World) {
    village_tick(w);
    contacts(w);
    politics_tick(w, true);
}

fn village_tick(w: &mut World) {
    let dt = SLOW_DT;
    w.recount();
    crate::life::villages(w, dt);
    jobs::construct(w);
    for f in w.factions.iter_mut() {
        f.recent_losses *= 0.97;
        f.starved_recent *= 0.96;
        f.rebel_cd -= dt;
        f.famine_cd -= dt;
    }
    update_villages(w, dt);
    check_factions(w, dt);
    w.refresh_hostility();
}

fn politics_tick(w: &mut World, rebels: bool) {
    let dt = 1.0;
    w.recount();
    scan_borders(w);
    crate::ages::economy(w, dt);
    crate::ages::update_metrics(w, dt);
    crate::ages::climb(w, dt);
    diplomacy(w, dt);
    conflict::update_levels(w, dt);
    conflict::strife_tick(w, dt);
    conflict::vassals(w, dt);
    crate::ages::crises(w, dt);
    crate::life::spread_plague(w, dt);
    jobs::rebalance(w);
    w.refresh_hostility();
    plan_marches(w);
    trade(w);
    if rebels {
        rebellion(w);
    }
    w.refresh_hostility();
}

/// Days of food a village keeps in store before it feeds another mouth or sends settlers: the
/// storage habit, which grows with the first ages (hunter-gatherers barely keep any).
/// Settlements one civilisation can hold together: more with each age (roads, law, empire).
pub fn max_villages(age: u8) -> u32 {
    32 + 4 * age as u32
}

pub fn reserve_days(age: u8) -> f32 {
    match age {
        0 | 1 => 0.0,
        2 => 2.1,
        3 => 3.1,
        // Granaries, trade and carriers carry the load from the Villages age on.
        _ => 1.5,
    }
}

fn too_close(w: &World, x: i32, y: i32, spacing: f32) -> bool {
    w.villages.iter().any(|v| v.alive && w.d.dist(v.x as f32, v.y as f32, x as f32, y as f32) < spacing)
}

fn site_ok(w: &World, f: u8, x: i32, y: i32) -> bool {
    if !w.terrain.walkable(x, y) {
        return false;
    }
    let o = w.owner[w.d.idx(x, y)];
    (o == NO_F || o == f) && !too_close(w, x, y, MIN_SPACING * 0.8)
}

/// A place for a new village 11–26 tiles from (ox, oy), in sight of (fx, fy).
pub fn pick_site(w: &mut World, f: u8, ox: i32, oy: i32, fx: i32, fy: i32) -> Option<(i32, i32)> {
    let mut best = None;
    let mut bs = f32::MIN;
    for _ in 0..16 {
        let ang = w.rng.range(0.0, TAU);
        let d = w.rng.range(13.0, 30.0);
        let sx = w.d.wrap_x((ox as f32 + ang.cos() * d).round() as i32);
        let sy = w.d.wrap_y((oy as f32 + ang.sin() * d).round() as i32);
        if !w.terrain.walkable(sx, sy) {
            continue;
        }
        if w.terrain.kind_at(sx, sy) == SAND && w.rng.chance(0.6) {
            continue;
        }
        let o = w.owner[w.d.idx(sx, sy)];
        if (o != NO_F && o != f) || too_close(w, sx, sy, MIN_SPACING) || !los(&w.terrain, fx, fy, sx, sy) {
            continue;
        }
        // Good land nearby, and a frontier: settlers push into land nobody has claimed.
        let mut s = 0.0;
        for k in 0..9 {
            let (kx, ky) = (sx + (k % 3 - 1) * 3, sy + (k / 3 - 1) * 3);
            s += food_max(w.terrain.kind_at(kx, ky)) * 0.5;
        }
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            let (kx, ky) = (sx + (a.cos() * 9.0) as i32, sy + (a.sin() * 9.0) as i32);
            if ky >= 0 && ky < w.d.h as i32 && w.terrain.walkable(kx, ky) && w.owner[w.d.idx(kx, ky)] == NO_F {
                s += 0.5;
            }
        }
        s += w.rng.f32() * 0.6 - d * 0.01;
        if s > bs {
            bs = s;
            best = Some((sx, sy));
        }
    }
    best
}

/// A settler has arrived (or needs a new site): found a village, or look again.
pub fn settle(w: &mut World, i: usize) {
    let (cx, cy) = tile(&w.agents[i], w.d);
    let (f, gx, gy, state) = {
        let a = &w.agents[i];
        (a.faction, a.gx, a.gy, a.state)
    };
    let alive_villages = w.villages.iter().filter(|v| v.alive).count();
    if state == State::ToSite && w.d.cheb(cx, cy, gx, gy) <= 1 && site_ok(w, f, cx, cy) && alive_villages < w.cap_villages {
        let food = w.agents[i].carry + 1.0;
        let origin = w.agents[i].home;
        let v = w.found_village(f, cx, cy, food, true);
        // Two villagers follow the settler from the old village, if it can spare them.
        let spare = (origin as usize) < w.villages.len() && w.villages[origin as usize].alive && w.villages[origin as usize].pop >= 4;
        if spare {
            let mut moved = 0;
            for a in w.agents.iter_mut() {
                if moved >= 2 {
                    break;
                }
                if a.alive && a.faction == f && a.home == origin && !a.job.fights() && a.job.role() != jobs::Role::Settle && a.carry <= 0.0 {
                    a.home = v;
                    a.state = State::ToHome;
                    moved += 1;
                }
            }
            w.villages[origin as usize].pop -= moved;
        }
        let fac = &mut w.factions[f as usize];
        let cap_ok = (fac.capital as usize) < w.villages.len() && w.villages[fac.capital as usize].alive && w.villages[fac.capital as usize].owner == f;
        if !cap_ok {
            fac.capital = v;
        }
        // A settlement founded far away starts a new clan; one across the sea is a colony.
        let (ox, oy) = (w.villages.get(origin as usize).map(|o| (o.x, o.y))).unwrap_or((cx, cy));
        let far = w.d.dist(ox as f32, oy as f32, cx as f32, cy as f32) > 24.0;
        if far && w.factions[f as usize].clans < 8 && w.rng.chance(0.25) {
            w.factions[f as usize].clans += 1;
            w.villages[v as usize].clan = w.factions[f as usize].clans - 1;
        } else if let Some(o) = w.villages.get(origin as usize) {
            let clan = o.clan;
            w.villages[v as usize].clan = clan;
        }
        if w.agents[i].job == Job::Colonist || w.terrain.land(ox, oy) != w.terrain.land(cx, cy) {
            w.log(ev::COLONY, f, NO_F, cx, cy, 0);
        }
        let age = w.factions[f as usize].age;
        w.villages[v as usize].fx = jobs::village_fx(w.villages[v as usize].built, age);
        let a = &mut w.agents[i];
        a.job = Job::Forage;
        a.home = v;
        a.state = State::Idle;
        a.carry = 0.0;
        a.timer = 0.5;
        a.tries = 0;
        return;
    }
    w.agents[i].tries = w.agents[i].tries.saturating_add(1);
    let home = w.agents[i].home;
    if w.agents[i].tries > 4 && home != NO_V {
        let a = &mut w.agents[i];
        a.job = Job::Forage;
        a.state = State::ToHome;
        return;
    }
    match pick_site(w, f, cx, cy, cx, cy) {
        Some((sx, sy)) => {
            let a = &mut w.agents[i];
            set_goal(a, w.d, sx, sy);
            a.state = State::ToSite;
        }
        None => {
            let a = &mut w.agents[i];
            a.state = State::Idle;
            a.timer = 1.5;
            if a.tries > 8 {
                a.tries = 0;
            }
        }
    }
}

fn update_villages(w: &mut World, dt: f32) {
    let focus = w.inputs.focus.clamp(0.0, 1.0);
    let calm = w.inputs.calm.clamp(0.0, 1.0);
    let mood = w.inputs.anger.clamp(0.0, 1.0) * 1.2 + w.inputs.agitation() * 0.6;
    let alive_villages = w.villages.iter().filter(|v| v.alive).count();
    for vi in 0..w.villages.len() {
        if !w.villages[vi].alive {
            continue;
        }
        let v16 = vi as u16;
        let (vx, vy, f) = (w.villages[vi].x, w.villages[vi].y, w.villages[vi].owner);
        let fac = w.factions[f as usize].clone();
        let tr = fac.traits;

        // Fire next door burns the village; it slowly mends.
        let mut burn = 0;
        for oy in -1..=1 {
            for ox in -1..=1 {
                let y = vy + oy;
                if y >= 0 && y < w.d.h as i32 && w.burning[w.d.idx(vx + ox, y)] != 0 {
                    burn += 1;
                }
            }
        }
        let soil = food_max(w.terrain.kind_at(vx, vy));
        {
            let v = &mut w.villages[vi];
            v.hp = (v.hp - dt * 0.12 * burn as f32 + dt * 0.01).min(1.0);
            v.food += dt * 0.03 * soil * (v.radius as f32 / START_RADIUS as f32);
            v.settler_t -= dt;
        }
        if w.villages[vi].hp <= 0.0 {
            raze(w, v16, NO_F);
            continue;
        }
        // Empty villages fall to ruin.
        if w.villages[vi].pop == 0 {
            w.villages[vi].empty_t += dt;
            if w.villages[vi].empty_t > 15.0 && w.villages[vi].food < SPAWN_COST {
                raze(w, v16, NO_F);
                continue;
            }
        } else {
            w.villages[vi].empty_t = 0.0;
        }

        // ---- growth: food surplus becomes people ----
        let cap = w.village_cap(v16);
        w.villages[vi].cap = cap;
        w.villages[vi].spawn_t += dt * (0.6 + 0.8 * tr.fertility) * (1.0 + 0.5 * focus);
        let (pop, food) = (w.villages[vi].pop, w.villages[vi].food);
        let reserve = pop as f32 * EAT_RATE * DAY_SECONDS * reserve_days(fac.age);
        if w.villages[vi].spawn_t >= SPAWN_EVERY && pop < cap && food >= SPAWN_COST + 0.5 + reserve && w.agents.len() < w.cap_agents {
            w.villages[vi].spawn_t = 0.0;
            w.villages[vi].food -= SPAWN_COST;
            let at_war = w.at_war[f as usize];
            let job = jobs::choose(w, v16);
            if let Some(a) = w.spawn_agent(f, v16, job, vx, vy) {
                if job.fights() && job != Job::Hunt {
                    let g = job == Job::Guard || w.rng.chance(if fac.fortify_t > 0.0 { 0.8 } else if at_war { 0.3 } else { 0.5 });
                    w.agents[a].garrison = g;
                    w.villages[vi].warriors += 1;
                }
                w.villages[vi].pop += 1;
            }
        }
        // ---- settlers (from the Villages age, more often; colonists cross the sea) ----
        let (pop, food) = (w.villages[vi].pop, w.villages[vi].food);
        if fac.age >= Job::Colonist.unlock_age()
            && w.villages[vi].settler_t <= 0.0
            && w.villages[vi].coast
            && jobs::has(&w.villages[vi], Building::Shipyard, fac.age)
            && food >= SETTLER_COST + 2.0
            && w.rng.chance(0.15)
            && alive_villages < w.cap_villages
            && w.agents.len() < w.cap_agents
        {
            if let Some((sx, sy)) = pick_overseas(w, f, vx, vy) {
                w.villages[vi].food -= SETTLER_COST;
                w.villages[vi].settler_t = 20.0;
                if let Some(a) = w.spawn_agent(f, v16, Job::Colonist, vx, vy) {
                    let ag = &mut w.agents[a];
                    ag.carry = SETTLER_COST - SPAWN_COST;
                    set_goal(ag, w.d, sx, sy);
                    ag.state = State::Sail;
                }
            }
        }
        if w.villages[vi].settler_t <= 0.0
            && pop as f32 >= (cap as f32 * 0.45).max(3.0)
            && food >= SETTLER_COST + 1.0 + reserve
            && alive_villages < w.cap_villages
            && fac.villages < max_villages(fac.age)
            && w.agents.len() < w.cap_agents
        {
            match pick_site(w, f, vx, vy, vx, vy) {
                Some((sx, sy)) => {
                    w.villages[vi].food -= SETTLER_COST;
                    w.villages[vi].settler_t = 7.0 / (0.5 + tr.fertility) * if fac.age >= Job::Settle.unlock_age() { 0.75 } else { 1.0 };
                    if let Some(a) = w.spawn_agent(f, v16, Job::Settle, vx, vy) {
                        let ag = &mut w.agents[a];
                        ag.carry = SETTLER_COST - SPAWN_COST;
                        set_goal(ag, w.d, sx, sy);
                        ag.state = State::ToSite;
                    }
                }
                None => w.villages[vi].settler_t = 4.0,
            }
        }
        // ---- territory grows with the village (and, from the Villages age, with the age) ----
        let territory = fac.age >= Job::Settle.unlock_age();
        let bonus = if territory { ((fac.age as i32 - 5) / 3).min(5) } else { -1 };
        let max_r = if fac.capital == v16 { 12 } else { 10 } + bonus;
        let r = w.villages[vi].radius;
        if r < max_r && w.villages[vi].pop as i32 >= r - 2 - bonus.max(0) && w.rng.chance(dt * if territory { 0.3 } else { 0.2 }) {
            w.villages[vi].radius = r + 1;
            w.claim_land(v16, r + 1);
        }

        // ---- unrest ----
        {
            let capital = fac.capital;
            let is_cap = capital == v16;
            let d = if (capital as usize) < w.villages.len() && w.villages[capital as usize].alive && !is_cap {
                w.village_dist(v16, capital)
            } else {
                0.0
            };
            let roads = w.villages[vi].fx.links >= 0.5;
            let overextended = if fac.age >= 11 { 1.0 + 2.0 * fac.metrics.territory } else { 1.0 };
            let v = &mut w.villages[vi];
            let dist_term = ((d - 18.0) / 45.0).clamp(0.0, 1.5) * if roads { 0.6 } else { 1.0 } * overextended;
            let crowd = ((v.pop as f32 / v.cap.max(1) as f32) - 0.8).max(0.0) * 2.5;
            let hunger = v.hungry as f32 / v.pop.max(1) as f32 + if v.food < 1.0 { 0.15 } else { 0.0 };
            let losses = (fac.recent_losses / fac.pop.max(8) as f32).min(1.0);
            // Inequality rankles once there are kings and taxes to resent.
            let unequal = (fac.metrics.inequality - 0.35).max(0.0) * if fac.age >= 9 { 1.2 } else { 0.3 };
            let pressure = crowd + hunger * 1.3 + dist_term + losses * 2.0 + mood + unequal;
            // Unrest settles toward what the village's lot warrants (so stability measures
            // today's conditions, not every grievance since the founding).
            let target = (UNREST_GAIN * pressure * (1.35 - tr.cohesion) - 0.25 * calm).clamp(0.0, 1.5);
            v.unrest = (v.unrest + (target - v.unrest) * UNREST_RATE * dt).clamp(0.0, 1.5);
            if is_cap {
                v.unrest = (v.unrest - dt * 0.05).max(0.0);
            }
        }

        // ---- siege: hostile warriors at the gates and nobody to hold them ----
        let mut att = 0u32;
        let mut def = 0u32;
        let mut inner = 0u32;
        let mut inner_def = 0u32;
        let mut by_f = [0u16; MAX_FACTIONS];
        let (vcamp, vside) = (w.villages[vi].camp, w.villages[vi].side);
        {
            let agents = &w.agents;
            let hostile = &w.hostile;
            let (cxf, cyf) = (vx as f32 + 0.5, vy as f32 + 0.5);
            w.grid.near(cxf, cyf, 5.0, |j| {
                let b = &agents[j];
                if !b.alive || !b.job.fights() || b.prisoner > 0.0 {
                    return;
                }
                let d = w.d.dist(cxf, cyf, b.x, b.y);
                if b.faction == f {
                    if vcamp != 0 && b.camp == vcamp && b.side != vside {
                        if d <= 3.0 {
                            inner += 1;
                        }
                    } else if d <= 5.0 {
                        def += 1;
                        if vcamp != 0 && b.camp == vcamp {
                            inner_def += 1;
                        }
                    }
                } else if hostile[f as usize * MAX_FACTIONS + b.faction as usize] && d <= 3.0 {
                    att += 1;
                    by_f[b.faction as usize] += 1;
                }
            });
        }
        // A civil war inside the civilisation: the other side takes the village over.
        if inner > 0 && inner_def == 0 && vcamp != 0 {
            let level = w.strifes[vcamp as usize - 1].level;
            if level >= CIVIL_WAR {
                w.villages[vi].siege += dt * inner as f32 * 0.35;
                if w.villages[vi].siege >= SIEGE_TO_CAPTURE {
                    conflict::turn_village(w, v16, 1 - vside);
                }
            } else if level >= RAID {
                let v = &mut w.villages[vi];
                v.food = (v.food - dt * 0.3 * inner as f32).max(0.0);
                v.raided_t = w.time;
            }
        }
        let walls = w.villages[vi].fx.walls;
        if att > 0 && def == 0 {
            let by = (0..MAX_FACTIONS).max_by_key(|&k| (by_f[k], MAX_FACTIONS - k)).unwrap_or(0) as u8;
            let raid_only = w.rel(by, f).war != 2 && w.rel(f, by).war != 2;
            w.villages[vi].raided_t = w.time;
            if !raid_only {
                // Walls hold a village twice as long.
                w.villages[vi].siege += dt * att as f32 * 0.35 * if walls { 0.5 } else { 1.0 };
                w.villages[vi].besieger = by;
                if w.villages[vi].siege >= SIEGE_TO_CAPTURE {
                    capture(w, v16, by);
                }
            }
        } else if inner == 0 {
            let v = &mut w.villages[vi];
            v.siege = (v.siege - dt * 0.5).max(0.0);
        }
    }
}

/// A village changes hands. Its people are spared, scattered or killed depending on the
/// captor's violence.
pub fn capture(w: &mut World, v: u16, by: u8) {
    let (old, x, y) = {
        let vil = &w.villages[v as usize];
        (vil.owner, vil.x, vil.y)
    };
    if old == by {
        return;
    }
    {
        let age = w.factions[by as usize].age;
        let vil = &mut w.villages[v as usize];
        vil.owner = by;
        vil.siege = 0.0;
        vil.unrest = 0.35;
        vil.food *= 0.5;
        vil.hp = vil.hp.max(0.5);
        vil.besieger = NO_F;
        vil.camp = 0;
        vil.side = 0;
        vil.fx = jobs::village_fx(vil.built, age);
    }
    w.rel_mut(old, by).grudge += 0.3;
    w.transfer_land(v, by);
    let violence = w.factions[by as usize].traits.violence;
    let has_other = w.villages.iter().enumerate().any(|(i, o)| i != v as usize && o.alive && o.owner == old);
    let mut taken = 0;
    for i in 0..w.agents.len() {
        let (alive, fa, home, job) = {
            let a = &w.agents[i];
            (a.alive, a.faction, a.home, a.job)
        };
        if !alive || fa != old || home != v {
            continue;
        }
        if job == Job::Warrior {
            w.agents[i].home = NO_V;
        } else if w.rng.chance(violence * 0.6) {
            w.kill(i, Cause::Combat);
        } else if has_other && w.rng.chance(0.5) {
            let a = &mut w.agents[i];
            a.home = NO_V;
            a.state = State::ToHome;
        } else {
            let a = &mut w.agents[i];
            a.faction = by;
            a.state = State::Idle;
            a.carry = 0.0;
            a.camp = 0;
            taken += 1;
        }
    }
    w.stats.captures += 1;
    w.factions[old as usize].last_attacker = by;
    w.log(ev::CAPTURE, by, old, x, y, taken);
    w.effect(x as f32 + 0.5, y as f32 + 0.5, 6, 1.5, 6.0);
    if w.factions[old as usize].capital == v {
        new_capital(w, old, true);
    }
}

/// A village is destroyed (fire, disaster, abandonment).
pub fn raze(w: &mut World, v: u16, by: u8) {
    let now = w.time;
    let (owner, x, y) = {
        let vil = &mut w.villages[v as usize];
        vil.alive = false;
        vil.died = now;
        vil.camp = 0;
        (vil.owner, vil.x, vil.y)
    };
    w.release_land(v);
    w.log(ev::RAZED, owner, by, x, y, 0);
    if w.factions[owner as usize].capital == v {
        new_capital(w, owner, true);
    }
}

fn new_capital(w: &mut World, f: u8, shaken: bool) {
    let best = w
        .villages
        .iter()
        .enumerate()
        .filter(|(_, v)| v.alive && v.owner == f)
        .max_by_key(|(i, v)| (v.pop, usize::MAX - *i))
        .map(|(i, _)| i as u16);
    w.factions[f as usize].capital = best.unwrap_or(NO_V);
    if let Some(c) = best {
        let (x, y) = (w.villages[c as usize].x, w.villages[c as usize].y);
        w.log(ev::CAPITAL, f, NO_F, x, y, 0);
        if shaken {
            for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == f) {
                v.unrest = (v.unrest + 0.25).min(1.5);
            }
        }
    }
}

fn meet(w: &mut World, a: u8, b: u8, x: i32, y: i32) {
    if a == b || w.rel(a, b).met {
        return;
    }
    w.rel_mut(a, b).met = true;
    w.rel_mut(b, a).met = true;
    for (p, q) in [(a, b), (b, a)] {
        let t = w.factions[p as usize].traits;
        w.rel_mut(p, q).tension += 0.3 * (t.aggressiveness - t.friendliness);
    }
    w.stats.contacts += 1;
    w.log(ev::CONTACT, a, b, x, y, 0);
}

/// First contact: two agents of civilisations that have never met come within 3 tiles, or
/// an agent walks onto another civilisation's land.
fn contacts(w: &mut World) {
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    let unmet = alive.iter().any(|&a| alive.iter().any(|&b| a != b && !w.rel(a, b).met));
    if !unmet {
        return;
    }
    let mut found: Vec<(u8, u8, i32, i32)> = Vec::new();
    {
        let agents = &w.agents;
        let rel = &w.rel;
        for (i, a) in agents.iter().enumerate() {
            if !a.alive {
                continue;
            }
            let o = w.owner[w.d.idx(a.x as i32, a.y as i32)];
            if o != NO_F && o != a.faction && !rel[a.faction as usize * MAX_FACTIONS + o as usize].met && !found.iter().any(|f| (f.0, f.1) == (a.faction, o) || (f.1, f.0) == (a.faction, o)) {
                found.push((a.faction, o, a.x as i32, a.y as i32));
            }
            w.grid.near(a.x, a.y, 3.0, |j| {
                if j <= i {
                    return;
                }
                let b = &agents[j];
                if !b.alive || b.faction == a.faction || rel[a.faction as usize * MAX_FACTIONS + b.faction as usize].met {
                    return;
                }
                if w.d.dist(a.x, a.y, b.x, b.y) <= 3.0 && !found.iter().any(|f| (f.0, f.1) == (a.faction, b.faction) || (f.1, f.0) == (a.faction, b.faction)) {
                    found.push((a.faction, b.faction, a.x as i32, a.y as i32));
                }
            });
        }
    }
    // Coastal exploration (from the Toolmakers' rafts): a coastal village with a raft or a dock
    // may find a people across up to 70 tiles of water.
    let mut sea: Vec<(u8, u8, i32, i32)> = Vec::new();
    for v in w.villages.iter().filter(|v| v.alive && v.coast) {
        let f = &w.factions[v.owner as usize];
        let boats = v.built & ((1u64 << Building::Raft as u8) | (1u64 << Building::Dock as u8) | (1u64 << Building::Port as u8)) != 0;
        if !boats || f.age < Building::Raft.unlock_age() {
            continue;
        }
        for o in w.villages.iter().filter(|o| o.alive && o.owner != v.owner && o.coast) {
            if !w.rel(v.owner, o.owner).met && w.d.dist(v.x as f32, v.y as f32, o.x as f32, o.y as f32) < 70.0 && !sea.iter().any(|s| s.0 == v.owner && s.1 == o.owner) {
                sea.push((v.owner, o.owner, o.x, o.y));
            }
        }
    }
    for (a, b, x, y) in sea {
        if w.rng.chance(0.05) {
            found.push((a, b, x, y));
        }
    }
    for (a, b, x, y) in found {
        meet(w, a, b, x, y);
    }
}

/// Shared border lengths and the nearest village pair between every two civilisations.
fn scan_borders(w: &mut World) {
    // Only claimed land can be on a border, so walk each village's own tiles rather than the
    // whole planet (every owned tile is claimed by exactly one village).
    let mut border = [0u32; MAX_FACTIONS * MAX_FACTIONS];
    let mut owned = [0u32; MAX_FACTIONS];
    let d = w.d;
    for (vi, v) in w.villages.iter().enumerate() {
        if !v.alive {
            continue;
        }
        let r = v.radius + 1;
        for oy in -r..=r {
            let y = v.y + oy;
            if !d.in_y(y) {
                continue;
            }
            for ox in -r..=r {
                let i = d.idx(v.x + ox, y);
                if w.claim[i] != vi as u16 {
                    continue;
                }
                let o = w.owner[i];
                owned[o as usize] += 1;
                let e = w.owner[d.idx(v.x + ox + 1, y)];
                if e != NO_F && e != o {
                    border[o as usize * MAX_FACTIONS + e as usize] += 1;
                    border[e as usize * MAX_FACTIONS + o as usize] += 1;
                }
                {
                    let s = w.owner[d.idx(v.x + ox, y + 1)];
                    if s != NO_F && s != o {
                        border[o as usize * MAX_FACTIONS + s as usize] += 1;
                        border[s as usize * MAX_FACTIONS + o as usize] += 1;
                    }
                }
            }
        }
    }
    w.owned_tiles.copy_from_slice(&owned);
    let mut lists: Vec<Vec<u16>> = vec![Vec::new(); MAX_FACTIONS];
    for (i, v) in w.villages.iter().enumerate() {
        if v.alive {
            lists[v.owner as usize].push(i as u16);
        }
    }
    for a in 0..MAX_FACTIONS {
        for b in 0..MAX_FACTIONS {
            let k = a * MAX_FACTIONS + b;
            w.rel[k].border = border[k];
            w.near_pairs[k] = (f32::MAX, NO_V, NO_V);
            if a >= b || lists[a].is_empty() || lists[b].is_empty() {
                continue;
            }
            let mut best = (f32::MAX, NO_V, NO_V);
            for &va in &lists[a] {
                let (ax, ay) = (w.villages[va as usize].x, w.villages[va as usize].y);
                let la = w.terrain.land(ax, ay);
                for &vb in &lists[b] {
                    let (bx, by) = (w.villages[vb as usize].x, w.villages[vb as usize].y);
                    if w.terrain.land(bx, by) != la {
                        continue;
                    }
                    let d = w.d.dist(ax as f32, ay as f32, bx as f32, by as f32);
                    if d < best.0 {
                        best = (d, va, vb);
                    }
                }
            }
            w.near_pairs[k] = best;
            w.near_pairs[b * MAX_FACTIONS + a] = (best.0, best.2, best.1);
        }
    }
    for a in 0..MAX_FACTIONS {
        for b in 0..MAX_FACTIONS {
            let k = a * MAX_FACTIONS + b;
            w.rel[k].near = a != b && (w.rel[k].border > 0 || w.near_pairs[k].0 < NEAR_DIST);
        }
    }
    // Touching borders means they have met, even if no one was looking.
    for a in 0..MAX_FACTIONS as u8 {
        for b in (a + 1)..MAX_FACTIONS as u8 {
            if w.rel(a, b).border > 0 && !w.rel(a, b).met && w.factions[a as usize].alive && w.factions[b as usize].alive {
                let p = w.near_pairs[a as usize * MAX_FACTIONS + b as usize];
                let (x, y) = if p.1 != NO_V { (w.villages[p.1 as usize].x, w.villages[p.1 as usize].y) } else { (0, 0) };
                meet(w, a, b, x, y);
            }
        }
    }
}

pub fn declare_war(w: &mut World, a: u8, b: u8) {
    if a == b || w.rel(a, b).war == 2 {
        return;
    }
    for (p, q) in [(a, b), (b, a)] {
        let r = w.rel_mut(p, q);
        r.war = 2;
        r.war_t = 0.0;
        r.weariness = 0.0;
        r.met = true;
        r.tension = r.tension.max(0.6);
    }
    w.stats.wars += 1;
    let p = w.near_pairs[a as usize * MAX_FACTIONS + b as usize];
    let (x, y) = if p.2 != NO_V { (w.villages[p.2 as usize].x, w.villages[p.2 as usize].y) } else { w.factions[a as usize].start };
    w.log(ev::WAR, a, b, x, y, 0);
    w.refresh_hostility();
}

pub fn make_peace(w: &mut World, a: u8, b: u8) {
    if w.rel(a, b).war == 0 && w.rel(b, a).war == 0 {
        return;
    }
    let was_war = w.rel(a, b).war == 2;
    let battles = w.rel(a, b).battles;
    for (p, q) in [(a, b), (b, a)] {
        let r = w.rel_mut(p, q);
        r.war = 0;
        r.war_t = 0.0;
        r.weariness = 0.0;
        r.tension = -0.4;
        r.cooldown = 30.0;
    }
    if was_war {
        conflict::on_peace(w, a, b, battles);
    }
    if was_war {
        w.stats.peaces += 1;
        let p = w.near_pairs[a as usize * MAX_FACTIONS + b as usize];
        let (x, y) = if p.1 != NO_V { (w.villages[p.1 as usize].x, w.villages[p.1 as usize].y) } else { w.factions[a as usize].start };
        w.log(ev::PEACE, a, b, x, y, 0);
    }
    w.refresh_hostility();
}

/// Tension drifts and wars wear on; then every civilisation decides (`decide`, or a `Policy`).
fn diplomacy(w: &mut World, dt: f32) {
    let hostility = w.inputs.hostility();
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    // Tension drifts.
    for &a in &alive {
        let fa = w.factions[a as usize].clone();
        let tr = fa.traits;
        let weight = if fa.is_player { 1.0 } else { 0.6 };
        for &b in &alive {
            if a == b {
                continue;
            }
            let grudge = if fa.parent == b || w.factions[b as usize].parent == a { 0.35 } else { 0.0 };
            // A vassal and its lord don't come to blows lightly.
            let bound = fa.overlord == b || w.factions[b as usize].overlord == a;
            let r = w.rel_mut(a, b);
            let grudge = grudge + 0.5 * r.grudge.min(1.0) - if bound { 0.4 } else { 0.0 };
            r.cooldown -= dt;
            r.trade_t -= dt;
            r.trade_news_t -= dt;
            if !r.met {
                continue;
            }
            if r.near {
                let pressure = tr.aggressiveness + fa.hungry * 0.8 + (r.border as f32 / 60.0).min(0.6) + grudge + hostility * weight
                    - tr.friendliness * 0.8
                    - 0.25;
                r.tension += TENSION_RATE * pressure * dt;
            } else {
                r.tension *= 0.99;
            }
            if r.war == 2 {
                r.tension = r.tension.max(0.5);
            }
            r.tension = r.tension.clamp(-1.0, 2.0);
        }
    }
    // War bookkeeping: raids run their course; wars wear both sides down.
    for &a in &alive {
        for &b in &alive {
            if a == b {
                continue;
            }
            let r = *w.rel(a, b);
            if r.war == 1 {
                let rr = w.rel_mut(a, b);
                rr.war_t += dt;
                if rr.war_t > 25.0 || (rr.raid_looted && rr.war_t > 10.0) {
                    rr.war = 0;
                    rr.cooldown = 12.0;
                    w.rel_mut(b, a).tension += 0.2;
                }
            }
            if r.war == 2 && a < b {
                let (fa, fb) = (&w.factions[a as usize], &w.factions[b as usize]);
                let losses = (fa.recent_losses + fb.recent_losses) / (fa.pop + fb.pop + 10) as f32;
                let rr = w.rel_mut(a, b);
                rr.war_t += dt;
                rr.weariness += dt * (0.01 + 0.4 * losses);
                let (wt, wear) = (rr.war_t, rr.weariness);
                let rb = w.rel_mut(b, a);
                rb.war_t = wt;
                rb.weariness = wear;
            }
        }
    }
    // Decisions: every civilisation's war-and-peace policy (`decide`, unless one is plugged in).
    let mut out = Vec::new();
    for &a in &alive {
        if !w.factions[a as usize].alive {
            continue;
        }
        out.clear();
        match w.policy.take() {
            Some(mut p) => {
                p.decide(w, a, &mut out);
                w.policy = Some(p);
            }
            None => decide_into(w, a, &mut out),
        }
        for &d in &out {
            apply(w, a, d);
        }
    }
}

// ---------------------------------------------------------------------------------------
// War and peace: the policy seam (#100).
//
// Every decision a civilisation makes about war and peace, with others or with its own rebels,
// goes through one function: `decide(&World, civ) -> Vec<Decision>`. It reads the world and
// returns what the civilisation does this second; `apply` carries it out. The rules below
// (tension thresholds, weariness, numbers) are the default. A trained policy (#100: PPO war
// councils) replaces them by implementing `Policy` and setting `World::policy`; it sees the
// same `World` and returns the same `Decision`s, so nothing else changes. Decisions are made
// once a second (the politics tick), for every living civilisation in id order.
//
// What stays outside the seam is the world's own physics: tension drifting with traits,
// hunger, borders, grudges and the player's mind; raids ending once looted; war weariness;
// conflict levels (scuffle, feud, battle) following tension; strife heating and its civil wars
// being won or lost in the field.

/// One thing a civilisation decides to do this second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decision {
    /// Carry on as before (the policy may also just return nothing).
    KeepPeace,
    /// Court civilisation `k`: warmer relations; a much weaker friendly neighbour of a
    /// kingdom (age 9+) accepts its protection and pays tribute.
    OfferAlliance(u8),
    /// Send war bands to steal from `k` (no conquest; ends once looted or after 25 s).
    Raid(u8),
    /// Open war on `k`: armies march, villages change hands (age 7+, or after two battles).
    DeclareWar(u8),
    /// Man the walls for 30 s: more fighters, kept at home.
    Fortify,
    /// End the war with `with`; `vassal`: accept being its tributary.
    SueForPeace { with: u8, vassal: bool },
    /// Commit the crown to putting down the rebels of strife `s` (`World::strifes` index).
    CrushRebels(u16),
    /// Let the rebels of strife `s` go with what they hold (a truce before civil war).
    ConcedeToRebels(u16),
}

/// A war-and-peace policy: given the world, what civilisation `civ` does this second.
pub trait Policy {
    fn decide(&mut self, w: &World, civ: u8, out: &mut Vec<Decision>);
}

/// The default policy: today's rules, as a pure function of the world.
pub fn decide(w: &World, civ: u8) -> Vec<Decision> {
    let mut out = Vec::new();
    decide_into(w, civ, &mut out);
    out
}

/// A deterministic die for the rules (so `decide` needs no mutable state): 0..1, different for
/// every second, pair and question.
fn roll(w: &World, a: u8, b: u8, salt: u32) -> f32 {
    let mut h = w.seed ^ w.tick.wrapping_mul(0x9e37_79b9) ^ ((a as u32) << 8 | b as u32).wrapping_mul(0x85eb_ca6b) ^ salt.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The rules (`decide`), into a buffer.
pub fn decide_into(w: &World, a: u8, out: &mut Vec<Decision>) {
    let fa = &w.factions[a as usize];
    if !fa.alive {
        return;
    }
    let ta = fa.traits;
    let calm = w.inputs.calm.clamp(0.0, 1.0);
    let agitation = w.inputs.agitation();
    let anger = w.inputs.anger.clamp(0.0, 1.0);
    let mut threatened = false;
    for b in 0..MAX_FACTIONS as u8 {
        let fb = &w.factions[b as usize];
        if b == a || !fb.alive {
            continue;
        }
        let (r, rb) = (*w.rel(a, b), *w.rel(b, a));
        threatened |= rb.met && rb.near && (rb.tension >= 0.8 || rb.war > 0);
        let bound = fa.overlord == b || fb.overlord == a;
        if r.war == 0 && rb.war != 2 && r.cooldown <= 0.0 && r.near && r.met && !bound {
            if r.tension >= WAR_TENSION && conflict::may_declare(w, a, b) {
                out.push(Decision::DeclareWar(b));
                continue;
            }
            if r.tension >= RAID_TENSION && ta.aggressiveness >= 0.45 && rb.war == 0 && (fa.hungry > 0.2 || roll(w, a, b, 1) < 0.1 * ta.aggressiveness + 0.1 * agitation) {
                out.push(Decision::Raid(b));
                continue;
            }
        }
        if r.war == 2 && r.war_t > 20.0 {
            // Beaten: sue for peace and pay tribute.
            if fa.pop * 10 < fb.pop * 4 && fa.recent_losses > fb.recent_losses && fa.overlord == NO_F && fb.overlord != a && roll(w, a, b, 2) < 0.1 {
                out.push(Decision::SueForPeace { with: b, vassal: true });
                continue;
            }
            // Weary: both sides have had enough (decided once per pair).
            let fr = (ta.friendliness + fb.traits.friendliness) * 0.5;
            let mood = (1.0 + 1.6 * calm - 0.6 * agitation - 0.6 * anger).max(0.1);
            if a < b && r.weariness * (0.4 + fr) * mood > 1.0 {
                out.push(Decision::SueForPeace { with: b, vassal: false });
                continue;
            }
        }
        // A friendly kingdom takes a much weaker friendly neighbour under its protection.
        if fa.age >= 9
            && ta.friendliness >= 0.45
            && fa.overlord == NO_F
            && fb.overlord == NO_F
            && rb.met
            && rb.near
            && r.war == 0
            && rb.war == 0
            && rb.tension <= 0.1
            && fb.pop * 3 <= fa.pop * 2
            && fb.traits.friendliness >= 0.3
            && !w.factions.iter().any(|x| x.alive && x.overlord == b)
            && roll(w, a, b, 3) < 0.03 * (ta.friendliness + fb.traits.friendliness)
        {
            out.push(Decision::OfferAlliance(b));
        }
    }
    if threatened && fa.fortify_t <= 0.0 && fa.age >= 5 {
        out.push(Decision::Fortify);
    }
    // Rebels: a civil war nobody can win is conceded; a rising the crown outnumbers two to
    // one, it sets out to crush.
    for (k, s) in w.strifes.iter().enumerate() {
        if !s.alive || s.civ != a {
            continue;
        }
        if s.level >= CIVIL_WAR && s.civil_t > conflict::CIVIL_WAR_SETTLES {
            out.push(Decision::ConcedeToRebels(k as u16));
        } else if s.level >= RAID && !s.committed && s.fighters[0] >= 2 * s.fighters[1].max(1) {
            out.push(Decision::CrushRebels(k as u16));
        }
    }
}

/// Carry out a decision of civilisation `a` (checked against the world as it is now).
pub fn apply(w: &mut World, a: u8, d: Decision) {
    let ok = |w: &World, b: u8| (b as usize) < MAX_FACTIONS && b != a && w.factions[b as usize].alive;
    match d {
        Decision::KeepPeace => {}
        Decision::OfferAlliance(b) => {
            if ok(w, b) && w.rel(a, b).war == 0 && w.rel(b, a).war == 0 {
                w.rel_mut(a, b).tension -= 0.05;
                w.rel_mut(b, a).tension -= 0.05;
                if w.factions[a as usize].age >= 9 && w.factions[b as usize].pop * 3 <= w.factions[a as usize].pop * 2 {
                    conflict::make_vassal(w, b, a, 1);
                }
            }
        }
        Decision::Raid(b) => {
            if ok(w, b) && w.rel(a, b).war == 0 && w.rel(b, a).war != 2 {
                let rr = w.rel_mut(a, b);
                rr.war = 1;
                rr.war_t = 0.0;
                rr.raid_looted = false;
                w.stats.raids += 1;
            }
        }
        Decision::DeclareWar(b) => {
            if ok(w, b) {
                declare_war(w, a, b);
            }
        }
        Decision::Fortify => {
            w.factions[a as usize].fortify_t = 30.0;
        }
        Decision::SueForPeace { with, vassal } => {
            if ok(w, with) && (w.rel(a, with).war > 0 || w.rel(with, a).war > 0) {
                make_peace(w, a, with);
                if vassal {
                    conflict::make_vassal(w, a, with, 0);
                }
            }
        }
        Decision::CrushRebels(k) => conflict::crush_rebels(w, a, k as usize),
        Decision::ConcedeToRebels(k) => conflict::concede(w, a, k as usize),
    }
}

/// Where each civilisation's warriors march: the enemy village nearest its own, reachable on foot.
fn plan_marches(w: &mut World) {
    for a in 0..MAX_FACTIONS {
        w.marches[a].clear();
        if !w.factions[a].alive {
            continue;
        }
        for b in 0..MAX_FACTIONS {
            let war = w.rel[a * MAX_FACTIONS + b].war;
            if a == b || war == 0 || !w.factions[b].alive {
                continue;
            }
            let (_, va, vb) = w.near_pairs[a * MAX_FACTIONS + b];
            if vb == NO_V {
                continue;
            }
            let (x, y) = (w.villages[vb as usize].x, w.villages[vb as usize].y);
            let dir = heading(w, va, x, y);
            w.marches[a].push(March { enemy: b as u8, village: vb, x, y, raid: war == 1, field: false, camp: 0, side: 0, dir });
        }
        // A battle: both armies take the field between them.
        for b in 0..MAX_FACTIONS {
            let r = w.rel[a * MAX_FACTIONS + b];
            if a == b || r.war != 0 || w.rel[b * MAX_FACTIONS + a].war != 0 || r.level != BATTLE || !w.factions[b].alive {
                continue;
            }
            let (_, va, _) = w.near_pairs[a * MAX_FACTIONS + b];
            if va == NO_V {
                continue;
            }
            let (x, y) = conflict::battle_field(w, a as u8, b as u8);
            let dir = heading(w, va, x, y);
            w.marches[a].push(March { enemy: b as u8, village: NO_V, x, y, raid: false, field: true, camp: 0, side: 0, dir });
        }
    }
    conflict::strife_marches(w);
}

/// Unit vector from village `from` toward (x, y).
fn heading(w: &World, from: u16, x: i32, y: i32) -> (f32, f32) {
    let Some(v) = w.villages.get(from as usize) else { return (1.0, 0.0) };
    let (dx, dy) = (w.d.dxi(v.x, x) as f32, w.d.dyi(v.y, y) as f32);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-3 {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

/// A coast across the sea for colonists: walkable, free, on another landmass, 25–90 tiles out.
pub fn pick_overseas(w: &mut World, f: u8, ox: i32, oy: i32) -> Option<(i32, i32)> {
    let home = w.terrain.land(ox, oy);
    for _ in 0..24 {
        let ang = w.rng.range(0.0, TAU);
        let d = w.rng.range(25.0, 90.0);
        let sx = w.d.wrap_x((ox as f32 + ang.cos() * d).round() as i32);
        let sy = w.d.wrap_y((oy as f32 + ang.sin() * d).round() as i32);
        if !w.terrain.walkable(sx, sy) || w.terrain.land(sx, sy) == home {
            continue;
        }
        let o = w.owner[w.d.idx(sx, sy)];
        if (o == NO_F || o == f) && !too_close(w, sx, sy, MIN_SPACING) && food_max(w.terrain.kind_at(sx, sy)) >= 0.45 {
            return Some((sx, sy));
        }
    }
    None
}

/// Friendly neighbours at peace share food: the richest village feeds the hungriest.
fn trade(w: &mut World) {
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    for &a in &alive {
        for &b in &alive {
            if a == b {
                continue;
            }
            let (r, rb) = (*w.rel(a, b), *w.rel(b, a));
            if !r.met || !r.near || r.war != 0 || rb.war != 0 || r.trade_t > 0.0 || r.tension > -0.05 || rb.tension > -0.05 {
                continue;
            }
            let rich = w.villages.iter().enumerate().filter(|(_, v)| v.alive && v.owner == a && v.food >= 8.0).max_by(|x, y| x.1.food.total_cmp(&y.1.food)).map(|(i, _)| i);
            let poor = w.villages.iter().enumerate().filter(|(_, v)| v.alive && v.owner == b && v.food < 4.0).min_by(|x, y| x.1.food.total_cmp(&y.1.food)).map(|(i, _)| i);
            let (Some(ri), Some(pi)) = (rich, poor) else { continue };
            w.villages[ri].food -= 2.0;
            w.villages[pi].food += 2.0;
            w.stats.trades += 1;
            w.rel_mut(a, b).traded_t = 0.0;
            w.rel_mut(b, a).traded_t = 0.0;
            // Disease travels with the grain.
            if w.villages[ri].plague > 0.3 && w.villages[pi].plague < 0.05 && w.rng.chance(0.3) {
                w.villages[pi].plague = 0.2;
            }
            w.rel_mut(a, b).trade_t = 6.0;
            w.rel_mut(a, b).tension -= 0.03;
            w.rel_mut(b, a).tension -= 0.05;
            if r.trade_news_t <= 0.0 {
                w.rel_mut(a, b).trade_news_t = 40.0;
                w.rel_mut(b, a).trade_news_t = 40.0;
                let (x, y) = (w.villages[pi].x, w.villages[pi].y);
                w.log(ev::TRADE, a, b, x, y, 2);
            }
        }
    }
}

fn rebellion(w: &mut World) {
    if w.time < 40.0 {
        return;
    }
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    if alive.len() >= MAX_FACTIONS {
        return;
    }
    for f in alive {
        let fac = &w.factions[f as usize];
        if fac.rebel_cd > 0.0 || fac.villages < 3 {
            continue;
        }
        // A civilisation already fighting its rebels in the field lets the fight decide.
        if w.strifes.iter().any(|s| s.alive && s.civ == f && s.by_village && s.level >= RAID) {
            continue;
        }
        let capital = fac.capital;
        let stability = fac.metrics.stability;
        let seed = w
            .villages
            .iter()
            .enumerate()
            .filter(|(i, v)| v.alive && v.owner == f && *i as u16 != capital)
            .max_by(|x, y| x.1.unrest.total_cmp(&y.1.unrest).then(y.0.cmp(&x.0)))
            .map(|(i, v)| (i as u16, v.unrest));
        if let Some((s, u)) = seed {
            // A restless village breaks away; in a shaky civilisation (low stability, with or
            // without any rival in sight) a merely unhappy one is enough.
            if u >= REBEL_UNREST || (stability < 0.45 && u >= 0.7) {
                split(w, f, s);
                return;
            }
        }
    }
}

/// Villages round `seed` break away from `parent` as a new civilisation.
pub fn split(w: &mut World, parent: u8, seed: u16) -> u8 {
    let pf = w.factions[parent as usize].clone();
    let max_n = (pf.villages as usize / 2).max(1);
    let mut cands: Vec<(u16, f32)> = w
        .villages
        .iter()
        .enumerate()
        .filter(|(i, v)| {
            let i = *i as u16;
            v.alive && v.owner == parent && i != pf.capital && i != seed && v.unrest >= 0.45 && w.village_dist(i, seed) <= 45.0
        })
        .map(|(i, v)| (i as u16, v.unrest))
        .collect();
    cands.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
    let mut set = vec![seed];
    for (v, _) in cands {
        if set.len() >= max_n {
            break;
        }
        set.push(v);
    }
    let c = split_set(w, parent, &set, true);
    let t = w.factions[c as usize].traits;
    // A small province of a big, cohesive realm may settle for paying tribute.
    let (pp, pc) = (w.factions[parent as usize].pop, w.factions[c as usize].pop);
    if pp > 2 * pc.max(1) && w.factions[parent as usize].overlord == NO_F && w.rng.chance(0.25 + 0.3 * pf.traits.cohesion) {
        conflict::make_vassal(w, c, parent, 2);
    } else if w.rng.chance(0.45 + 0.5 * t.aggressiveness) {
        declare_war(w, c, parent);
    }
    w.recount();
    c
}

/// Villages `set` (the first is the new capital) leave `parent` as a new civilisation with
/// the parent's traits drifted, its knowledge of the world, a grudge, and the highest age its
/// people can hold. `rebellion`: logged as a REBELLION (a civil war's split is logged by
/// `conflict`, also as a REBELLION).
pub fn split_set(w: &mut World, parent: u8, set: &[u16], rebellion: bool) -> u8 {
    let pf = w.factions[parent as usize].clone();
    let seed = set[0];
    let _ = rebellion;
    let child_seed = w.rng.next_u32();
    let p = pf.traits;
    let mut g = || w.rng.gauss() * 0.12;
    let t = Traits {
        aggressiveness: p.aggressiveness + 0.15 + g(),
        friendliness: p.friendliness - 0.1 + g(),
        violence: p.violence + 0.05 + g(),
        fertility: p.fertility + g() * 0.8,
        cohesion: 0.5 + g() * 0.8,
    }
    .clamped();
    let name = w.rebel_name(parent);
    let c = w.add_faction(child_seed, SeedSource::Rebellion, t, parent, Some(name));
    let (sx, sy) = (w.villages[seed as usize].x, w.villages[seed as usize].y);
    {
        let cf = &mut w.factions[c as usize];
        cf.region = pf.region;
        cf.start = (sx, sy);
        cf.capital = seed;
        cf.rebel_cd = 30.0;
    }
    for &v in set {
        let vil = &mut w.villages[v as usize];
        vil.owner = c;
        vil.unrest = 0.15;
        vil.siege = 0.0;
        vil.camp = 0;
        vil.side = 0;
        w.transfer_land(v, c);
    }
    let mut pop = 0;
    for a in w.agents.iter_mut() {
        if a.alive && a.faction == parent && set.contains(&a.home) {
            a.faction = c;
            a.target = NO_V;
            a.camp = 0;
            pop += 1;
            if matches!(a.state, State::March | State::Siege) {
                a.state = State::ToHome;
            }
        }
    }
    // The rebels keep their parent's ways, as far as their numbers can hold them, and a share
    // of its learning.
    {
        let age = crate::ages::sustainable_age(pf.age, pop);
        let cf = &mut w.factions[c as usize];
        cf.age = age;
        cf.peak_age = age;
        cf.clans = pf.clans;
        cf.metrics.knowledge = pf.metrics.knowledge * 0.6;
        cf.metrics.faith = pf.metrics.faith;
        // It remembers the ages it can't hold: their jobs stop (`jobs::rebalance`).
        cf.peak_age = pf.peak_age;
    }
    crate::ages::refresh_fx(w, c);
    // The rebels know everyone their parent knew, and hate the parent.
    for o in 0..MAX_FACTIONS as u8 {
        if o != c && o != parent && w.rel(parent, o).met {
            w.rel_mut(c, o).met = true;
            w.rel_mut(o, c).met = true;
        }
    }
    {
        let r = w.rel_mut(c, parent);
        r.met = true;
        r.near = true;
        r.tension = 0.6 + 0.4 * t.aggressiveness;
    }
    {
        let r = w.rel_mut(parent, c);
        r.met = true;
        r.near = true;
        r.tension = 0.5 + 0.3 * p.aggressiveness;
    }
    w.factions[parent as usize].rebel_cd = 40.0;
    for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == parent) {
        v.unrest *= 0.5;
    }
    w.stats.rebellions += 1;
    w.log(ev::REBELLION, parent, c, sx, sy, set.len() as i32);
    w.effect(sx as f32 + 0.5, sy as f32 + 0.5, 7, 2.0, 14.0);
    w.recount();
    c
}

fn wipe(w: &mut World, f: u8) {
    let by = w.factions[f as usize].last_attacker;
    let by = if by != NO_F && w.factions[by as usize].alive { by } else { NO_F };
    w.factions[f as usize].alive = false;
    for o in 0..MAX_FACTIONS as u8 {
        for (p, q) in [(f, o), (o, f)] {
            let r = w.rel_mut(p, q);
            r.war = 0;
            r.tension = 0.0;
        }
    }
    for i in 0..w.agents.len() {
        if w.agents[i].alive && w.agents[i].faction == f {
            if by != NO_F {
                let a = &mut w.agents[i];
                a.faction = by;
                a.home = NO_V;
                a.state = State::Idle;
            } else {
                w.kill(i, Cause::Starved);
            }
        }
    }
    let (x, y) = w.factions[f as usize].start;
    w.stats.wiped += 1;
    w.log(ev::WIPED, f, by, x, y, 0);
    for o in w.factions.iter_mut() {
        if o.overlord == f {
            o.overlord = NO_F;
        }
    }
    for k in 0..w.strifes.len() {
        if w.strifes[k].alive && w.strifes[k].civ == f {
            conflict::end(w, k, None);
        }
    }
    w.refresh_hostility();
}

fn check_factions(w: &mut World, dt: f32) {
    for f in 0..MAX_FACTIONS as u8 {
        if !w.factions[f as usize].alive {
            continue;
        }
        let cap = w.factions[f as usize].capital;
        let cap_ok = (cap as usize) < w.villages.len() && w.villages[cap as usize].alive && w.villages[cap as usize].owner == f;
        if !cap_ok && w.factions[f as usize].villages > 0 {
            new_capital(w, f, false);
        }
        let fac = &mut w.factions[f as usize];
        if fac.villages == 0 {
            fac.homeless_t += dt;
            if fac.pop == 0 || fac.homeless_t > 30.0 {
                wipe(w, f);
                continue;
            }
        } else {
            fac.homeless_t = 0.0;
        }
        let fac = &w.factions[f as usize];
        if fac.starved_recent >= 3.0 && fac.famine_cd <= 0.0 {
            let (x, y) = w.villages.get(fac.capital as usize).map(|v| (v.x, v.y)).unwrap_or(fac.start);
            w.factions[f as usize].famine_cd = 30.0;
            w.log(ev::FAMINE, f, NO_F, x, y, w.factions[f as usize].starved_recent.round() as i32);
        }
    }
}
