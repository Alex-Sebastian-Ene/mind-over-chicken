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
//!   The god's mood is +0.7·pulse + 0.6·anger − 0.75·calm (full weight for the player's
//!   civilisation, 0.6 for the rest).
//! - **Peace:** weariness grows with time and losses; peace when
//!   weariness · (0.4 + mean friendliness) · (1 + 1.6·calm − 0.6·pulse − 0.6·anger) > 1
//!   after at least 20 s of war.
//! - **Capture:** hostile warriors within 3 tiles of a village with no defending warriors
//!   within 5 build siege at 0.35/s each; at `SIEGE_TO_CAPTURE` the village changes owner.
//! - **Unrest** per village: 0.03 · (crowding + 1.3·hunger + distance from capital
//!   + 2·war losses + god's anger) · (1.35 − cohesion), minus 0.012 · (0.4 + cohesion + 0.8·calm).
//!   A non-capital village at `REBEL_UNREST` splits off with nearby restless villages (up to
//!   half the civilisation) as a new civilisation with drifted traits, which usually declares
//!   war on its parent.

use std::f32::consts::TAU;

use crate::agents::{los, set_goal, tile};
use crate::terrain::{food_max, SAND};
use crate::world::*;

pub const TENSION_RATE: f32 = 0.04;
pub const UNREST_RATE: f32 = 0.03;
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
    w.recount();
    scan_borders(w);
    update_metrics(w);
    diplomacy(w, 1.0);
    w.refresh_hostility();
    plan_marches(w);
    trade(w);
    if rebels {
        rebellion(w);
    }
    w.refresh_hostility();
}

fn too_close(w: &World, x: i32, y: i32, spacing: f32) -> bool {
    w.villages.iter().any(|v| v.alive && w.d.dist(v.x as f32, v.y as f32, x as f32, y as f32) < spacing)
}

fn site_ok(w: &World, f: u8, x: i32, y: i32) -> bool {
    if y < 3 || y > w.d.h as i32 - 4 || !w.terrain.walkable(x, y) {
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
        let sy = (oy as f32 + ang.sin() * d).round() as i32;
        if sy < 3 || sy > w.d.h as i32 - 4 || !w.terrain.walkable(sx, sy) {
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
    if state == State::ToSite && w.d.cheb(cx, cy, gx, gy) <= 1 && site_ok(w, f, cx, cy) && alive_villages < MAX_VILLAGES {
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
                if a.alive && a.faction == f && a.home == origin && a.job == Job::Gatherer && a.carry <= 0.0 {
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
        let a = &mut w.agents[i];
        a.job = Job::Gatherer;
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
        a.job = Job::Gatherer;
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
    let mood = w.inputs.anger.clamp(0.0, 1.0) * 1.2 + w.inputs.pulse() * 0.6;
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
        let (pop, warriors, food) = (w.villages[vi].pop, w.villages[vi].warriors, w.villages[vi].food);
        if w.villages[vi].spawn_t >= SPAWN_EVERY && pop < cap && food >= SPAWN_COST + 0.5 && w.agents.len() < MAX_AGENTS {
            w.villages[vi].spawn_t = 0.0;
            w.villages[vi].food -= SPAWN_COST;
            let at_war = w.at_war[f as usize];
            let want = if at_war { 0.3 + 0.4 * tr.aggressiveness } else { 0.06 + 0.16 * tr.aggressiveness };
            let job = if (warriors as f32) < want * (pop as f32 + 1.0) { Job::Warrior } else { Job::Gatherer };
            if let Some(a) = w.spawn_agent(f, v16, job, vx, vy) {
                if job == Job::Warrior {
                    let g = w.rng.chance(if at_war { 0.3 } else { 0.5 });
                    w.agents[a].garrison = g;
                    w.villages[vi].warriors += 1;
                }
                w.villages[vi].pop += 1;
            }
        }
        // ---- settlers ----
        let (pop, food) = (w.villages[vi].pop, w.villages[vi].food);
        if w.villages[vi].settler_t <= 0.0
            && pop as f32 >= (cap as f32 * 0.45).max(3.0)
            && food >= SETTLER_COST + 1.0
            && alive_villages < MAX_VILLAGES
            && fac.villages < 48
            && w.agents.len() < MAX_AGENTS
        {
            match pick_site(w, f, vx, vy, vx, vy) {
                Some((sx, sy)) => {
                    w.villages[vi].food -= SETTLER_COST;
                    w.villages[vi].settler_t = 7.0 / (0.5 + tr.fertility);
                    if let Some(a) = w.spawn_agent(f, v16, Job::Settler, vx, vy) {
                        let ag = &mut w.agents[a];
                        ag.carry = SETTLER_COST - SPAWN_COST;
                        set_goal(ag, w.d, sx, sy);
                        ag.state = State::ToSite;
                    }
                }
                None => w.villages[vi].settler_t = 4.0,
            }
        }
        // ---- territory grows with the village ----
        let max_r = if fac.capital == v16 { 12 } else { 10 };
        let r = w.villages[vi].radius;
        if r < max_r && w.villages[vi].pop as i32 >= r - 2 && w.rng.chance(dt * 0.2) {
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
            let v = &mut w.villages[vi];
            let dist_term = ((d - 18.0) / 45.0).clamp(0.0, 1.5);
            let crowd = ((v.pop as f32 / v.cap.max(1) as f32) - 0.8).max(0.0) * 2.5;
            let hunger = v.hungry as f32 / v.pop.max(1) as f32 + if v.food < 1.0 { 0.15 } else { 0.0 };
            let losses = (fac.recent_losses / fac.pop.max(8) as f32).min(1.0);
            let pressure = crowd + hunger * 1.3 + dist_term + losses * 2.0 + mood;
            let rise = UNREST_RATE * pressure * (1.35 - tr.cohesion);
            let fall = 0.012 * (0.4 + tr.cohesion + calm * 0.8);
            v.unrest = (v.unrest + (rise - fall) * dt).clamp(0.0, 1.5);
            if is_cap {
                v.unrest = (v.unrest - dt * 0.05).max(0.0);
            }
        }

        // ---- siege: hostile warriors at the gates and nobody to hold them ----
        let mut att = 0u32;
        let mut def = 0u32;
        let mut by_f = [0u16; MAX_FACTIONS];
        {
            let agents = &w.agents;
            let hostile = &w.hostile;
            let (cxf, cyf) = (vx as f32 + 0.5, vy as f32 + 0.5);
            w.grid.near(cxf, cyf, 5.0, |j| {
                let b = &agents[j];
                if !b.alive || b.job != Job::Warrior {
                    return;
                }
                let d = w.d.dist(cxf, cyf, b.x, b.y);
                if b.faction == f {
                    if d <= 5.0 {
                        def += 1;
                    }
                } else if hostile[f as usize * MAX_FACTIONS + b.faction as usize] && d <= 3.0 {
                    att += 1;
                    by_f[b.faction as usize] += 1;
                }
            });
        }
        if att > 0 && def == 0 {
            let by = (0..MAX_FACTIONS).max_by_key(|&k| (by_f[k], MAX_FACTIONS - k)).unwrap_or(0) as u8;
            let raid_only = w.rel(by, f).war != 2 && w.rel(f, by).war != 2;
            if !raid_only {
                w.villages[vi].siege += dt * att as f32 * 0.35;
                w.villages[vi].besieger = by;
                if w.villages[vi].siege >= SIEGE_TO_CAPTURE {
                    capture(w, v16, by);
                }
            }
        } else {
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
        let vil = &mut w.villages[v as usize];
        vil.owner = by;
        vil.siege = 0.0;
        vil.unrest = 0.35;
        vil.food *= 0.5;
        vil.hp = vil.hp.max(0.5);
        vil.besieger = NO_F;
    }
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
    let (owner, x, y) = {
        let vil = &mut w.villages[v as usize];
        vil.alive = false;
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
                if d.in_y(y + 1) {
                    let s = w.owner[i + d.w];
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

/// The `data/ages.json` metrics the age ladder (#69) will read (see `Metrics`).
fn update_metrics(w: &mut World) {
    let land = w.land_tiles;
    for f in 0..MAX_FACTIONS {
        if !w.factions[f].alive {
            continue;
        }
        let cap_land = w.villages.get(w.factions[f].capital as usize).map(|v| w.terrain.land(v.x, v.y)).unwrap_or(0);
        let (mut n, mut unrest, mut coastal, mut colonies) = (0u32, 0.0f32, 0u32, 0u32);
        for v in w.villages.iter().filter(|v| v.alive && v.owner as usize == f) {
            n += 1;
            unrest += v.unrest;
            let sea = (-2..=2).any(|oy| (-2..=2).any(|ox| matches!(w.terrain.kind_at(v.x + ox, v.y + oy), crate::terrain::WATER | crate::terrain::DEEP)));
            coastal += sea as u32;
            colonies += (cap_land != 0 && w.terrain.land(v.x, v.y) != cap_land) as u32;
        }
        let (mut contact, mut links) = (0u32, 0u32);
        for o in 0..MAX_FACTIONS {
            if o == f || !w.factions[o].alive {
                continue;
            }
            let r = w.rel[f * MAX_FACTIONS + o];
            contact += r.met as u32;
            links += (r.met && r.near && r.war == 0 && r.tension < -0.05) as u32;
        }
        let fac = &mut w.factions[f];
        let m = &mut fac.metrics;
        m.population = fac.pop;
        m.settlements = n;
        m.food_days = fac.food / (fac.pop.max(1) as f32 * EAT_RATE * DAY_SECONDS);
        m.stability = if n > 0 { (1.0 - unrest / n as f32).clamp(0.0, 1.0) } else { 0.0 };
        m.territory = w.owned_tiles[f] as f32 / land.max(1) as f32;
        m.contact = contact > 0;
        m.links = links;
        m.coastal = coastal > 0;
        m.colonies = colonies;
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
    for (p, q) in [(a, b), (b, a)] {
        let r = w.rel_mut(p, q);
        r.war = 0;
        r.war_t = 0.0;
        r.weariness = 0.0;
        r.tension = -0.4;
        r.cooldown = 30.0;
    }
    if was_war {
        w.stats.peaces += 1;
        let p = w.near_pairs[a as usize * MAX_FACTIONS + b as usize];
        let (x, y) = if p.1 != NO_V { (w.villages[p.1 as usize].x, w.villages[p.1 as usize].y) } else { w.factions[a as usize].start };
        w.log(ev::PEACE, a, b, x, y, 0);
    }
    w.refresh_hostility();
}

fn diplomacy(w: &mut World, dt: f32) {
    let hostility = w.inputs.hostility();
    let calm = w.inputs.calm.clamp(0.0, 1.0);
    let pulse = w.inputs.pulse();
    let anger = w.inputs.anger.clamp(0.0, 1.0);
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
            let r = w.rel_mut(a, b);
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
    // Decisions.
    for &a in &alive {
        for &b in &alive {
            if a == b {
                continue;
            }
            let ta = w.factions[a as usize].traits;
            let hungry = w.factions[a as usize].hungry;
            let (r, rb) = (*w.rel(a, b), *w.rel(b, a));
            if r.war == 0 && rb.war != 2 && r.cooldown <= 0.0 && r.near && r.met {
                if r.tension >= WAR_TENSION {
                    declare_war(w, a, b);
                    continue;
                }
                if r.tension >= RAID_TENSION && ta.aggressiveness >= 0.45 && rb.war == 0 && (hungry > 0.2 || w.rng.chance(0.1 * ta.aggressiveness + 0.1 * pulse)) {
                    let rr = w.rel_mut(a, b);
                    rr.war = 1;
                    rr.war_t = 0.0;
                    rr.raid_looted = false;
                    w.stats.raids += 1;
                    continue;
                }
            }
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
                let fr = (fa.traits.friendliness + fb.traits.friendliness) * 0.5;
                let mood = (1.0 + 1.6 * calm - 0.6 * pulse - 0.6 * anger).max(0.1);
                let rr = w.rel_mut(a, b);
                rr.war_t += dt;
                rr.weariness += dt * (0.01 + 0.4 * losses);
                let (wt, wear) = (rr.war_t, rr.weariness);
                w.rel_mut(b, a).war_t = wt;
                if wt > 20.0 && wear * (0.4 + fr) * mood > 1.0 {
                    make_peace(w, a, b);
                }
            }
        }
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
            let (_, _, vb) = w.near_pairs[a * MAX_FACTIONS + b];
            if vb == NO_V {
                continue;
            }
            let (x, y) = (w.villages[vb as usize].x, w.villages[vb as usize].y);
            w.marches[a].push(March { enemy: b as u8, village: vb, x, y, raid: war == 1 });
        }
    }
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
    for &v in &set {
        let vil = &mut w.villages[v as usize];
        vil.owner = c;
        vil.unrest = 0.15;
        vil.siege = 0.0;
        w.transfer_land(v, c);
    }
    for a in w.agents.iter_mut() {
        if a.alive && a.faction == parent && set.contains(&a.home) {
            a.faction = c;
            a.target = NO_V;
            if matches!(a.state, State::March | State::Siege) {
                a.state = State::ToHome;
            }
        }
    }
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
    let fight = w.rng.chance(0.45 + 0.5 * t.aggressiveness);
    if fight {
        declare_war(w, c, parent);
    }
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
