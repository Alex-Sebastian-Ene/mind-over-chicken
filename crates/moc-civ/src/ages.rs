//! The twenty ages (#69): reached, not researched.
//!
//! Every second each civilisation's metrics (`data/ages.json` `metrics`) are measured from the
//! simulation, and the next age's entry conditions (`ages_table::AGES`, generated from the
//! JSON, with its `"any"` alternatives) are checked. When they have held for
//! `READY_SECONDS` and the civilisation has spent `min_dwell` in its age, it enters the next
//! one: new jobs and buildings become possible and its capital holds a festival.
//!
//! **Collapse** (`data/ages.json` `collapse`): stability below 0.15 for 30 s, or population
//! below 60 % of the age's entry population (the population the ladder asks for at that age:
//! the nearest `population` condition at or below it) for `POP_GRACE` s, drops a civilisation
//! back one age. That is a Dark Age: the lost age's buildings go dormant and its jobs stop
//! (`jobs::rebalance` moves their workers to older jobs), some knowledge is lost, and it cannot
//! climb for `DARK_SECONDS`. Then it can earn the age back.
//!
//! Where the metrics come from:
//! - population, settlements, territory, contact, coastal, colonies: counted;
//! - food_days: stored food over a day's eating (`world::DAY_SECONDS`);
//! - stability: 1 − mean village unrest (hunger, crowding, distance, war losses,
//!   inequality, agitators, the player's anger and stress; calm, festivals, leaders, law,
//!   faith and parliaments lower it);
//! - knowledge: accumulated from knappers, smiths, scribes, monks, scholars, programmers…,
//!   elders' stories and libraries, faster with the player's focus;
//! - wealth: goats held by people, villages and the treasury (herders, traders, bankers,
//!   tribute and ransom add them; raids, crashes and famine take them);
//! - faith: mean village devotion (priests, temples, shrines, festivals, mourning, miracles);
//! - production: goods made per minute (smoothed);
//! - links: friendly trade partners, recent trade, and settlements linked by haulers, roads,
//!   docks, ports, railways and networks;
//! - vassals: civilisations paying it tribute;
//! - inequality: Gini of people's goats (taxes and bankers concentrate them; parliaments,
//!   reformers and revolutions spread them).

use crate::ages_table::{Building, Cond, Metric, AGES, AGE_COUNT, COLLAPSE_FOR_SECONDS, COLLAPSE_POP_SHARE, COLLAPSE_STABILITY_BELOW, CRISES};
use crate::conflict::{crisis_strife, StrifeKind};
use crate::jobs::{has, info, village_fx, Job};
use crate::world::*;

/// Seconds the next age's conditions must hold before it is entered.
pub const READY_SECONDS: f32 = 3.0;
/// Seconds of population below its share before it collapses an age (stability has its own 30 s).
pub const POP_GRACE: f32 = 8.0;
/// A Dark Age lasts at least this long.
pub const DARK_SECONDS: f32 = 45.0;
/// No second collapse within this long of the first.
pub const COLLAPSE_GRACE: f32 = 30.0;

/// Least time spent in age `n` before the next: the ladder takes about half an hour at ×1.
pub fn min_dwell(n: u8) -> f32 {
    10.0 + 3.0 * n as f32
}

pub fn def(n: u8) -> &'static crate::ages_table::AgeDef {
    &AGES[(n.clamp(1, AGE_COUNT as u8) - 1) as usize]
}

pub fn name(n: u8) -> &'static str {
    def(n).name
}

impl Metrics {
    /// A metric as a number (true/false as 1/0).
    pub fn value(&self, m: Metric) -> f32 {
        match m {
            Metric::Population => self.population as f32,
            Metric::Settlements => self.settlements as f32,
            Metric::FoodDays => self.food_days,
            Metric::Stability => self.stability,
            Metric::Territory => self.territory,
            Metric::Knowledge => self.knowledge,
            Metric::Wealth => self.wealth,
            Metric::Faith => self.faith,
            Metric::Production => self.production,
            Metric::Links => self.links as f32,
            Metric::Contact => self.contact as u8 as f32,
            Metric::Coastal => self.coastal as u8 as f32,
            Metric::Vassals => self.vassals as f32,
            Metric::Colonies => self.colonies as f32,
            Metric::Inequality => self.inequality,
        }
    }

    fn meets(&self, conds: &[Cond]) -> bool {
        conds.iter().all(|c| self.value(c.metric) >= c.at_least)
    }

    /// Does this civilisation meet age `n`'s entry conditions (`all`, and one `any` group)?
    pub fn can_enter(&self, n: u8) -> bool {
        if n < 1 || n as usize > AGE_COUNT {
            return false;
        }
        let a = def(n);
        self.meets(a.all) && (a.any.is_empty() || a.any.iter().any(|g| self.meets(g)))
    }
}

/// The population age `n` asks for: its own `population` condition, or the nearest earlier one.
pub fn entry_population(n: u8) -> f32 {
    for k in (1..=n.clamp(1, AGE_COUNT as u8)).rev() {
        let a = def(k);
        let p = a.all.iter().chain(a.any.iter().flat_map(|g| g.iter())).find(|c| c.metric == Metric::Population);
        if let Some(c) = p {
            return c.at_least;
        }
    }
    0.0
}

/// The highest age at or below `max` a civilisation of `pop` people can hold (rebels start here).
pub fn sustainable_age(max: u8, pop: u32) -> u8 {
    let mut n = max.max(1);
    while n > 1 && (pop as f32) < COLLAPSE_POP_SHARE * entry_population(n) {
        n -= 1;
    }
    n
}

/// Measure every civilisation (once a second, after the borders scan).
pub fn update_metrics(w: &mut World, dt: f32) {
    let land = w.land_tiles;
    let mut goats_buf = std::mem::take(&mut w.scratch);
    for f in 0..MAX_FACTIONS {
        if !w.factions[f].alive {
            continue;
        }
        let fac = &w.factions[f];
        let age = fac.age;
        let cap_land = w.villages.get(fac.capital as usize).map(|v| w.terrain.land(v.x, v.y)).unwrap_or(0);
        let (mut n, mut unrest, mut coastal, mut colonies) = (0u32, 0.0f32, 0u32, 0u32);
        let (mut faith, mut fw, mut vgoats, mut routes) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for v in w.villages.iter().filter(|v| v.alive && v.owner as usize == f) {
            n += 1;
            unrest += v.unrest;
            coastal += v.coast as u32;
            colonies += (cap_land != 0 && w.terrain.land(v.x, v.y) != cap_land) as u32;
            faith += v.faith * (v.pop as f32 + 1.0);
            fw += v.pop as f32 + 1.0;
            vgoats += v.goats;
            // A settlement linked to the rest by recent haulers, or by roads, docks, rails, wires.
            let hauled = if w.time - v.linked_t < 60.0 { 0.5 } else { 0.0 };
            routes += hauled + v.fx.links;
        }
        let (mut contact, mut partners, mut vassals) = (0u32, 0u32, 0u32);
        for o in 0..MAX_FACTIONS {
            if o == f || !w.factions[o].alive {
                continue;
            }
            let r = w.rel[f * MAX_FACTIONS + o];
            contact += r.met as u32;
            partners += (r.met && r.war == 0 && (r.traded_t < 60.0 || (r.near && r.tension < -0.05))) as u32;
            vassals += (w.factions[o].overlord == f as u8) as u32;
        }
        goats_buf.clear();
        let mut people_goats = 0.0;
        for a in w.agents.iter().filter(|a| a.alive && a.faction as usize == f) {
            // Everyone owns a little (a goat's worth of tools and pots); the Gini is of the rest.
            goats_buf.push(a.goats.max(0.0) + 1.0);
            people_goats += a.goats.max(0.0);
        }
        let gini = gini(&mut goats_buf);
        let fac = &mut w.factions[f];
        let rate = fac.prod_acc / dt.max(1e-3);
        fac.prod_acc = 0.0;
        fac.prod_rate += (rate - fac.prod_rate) * (dt / 20.0).min(1.0);
        let m = &mut fac.metrics;
        m.population = fac.pop;
        m.settlements = n;
        m.food_days = fac.food / (fac.pop.max(1) as f32 * EAT_RATE * DAY_SECONDS);
        m.stability = if n > 0 { (1.0 - unrest / n as f32).clamp(0.0, 1.0) } else { 0.0 };
        m.territory = w.owned_tiles[f] as f32 / land.max(1) as f32;
        m.contact = contact > 0;
        m.links = partners * 2 + routes.floor() as u32;
        m.coastal = coastal > 0;
        m.colonies = colonies;
        m.faith = if fw > 0.0 { faith / fw } else { 0.0 };
        m.wealth = people_goats + vgoats + fac.treasury;
        m.production = fac.prod_rate * 60.0;
        m.vassals = vassals;
        m.inequality = gini;
        let _ = age;
    }
    w.scratch = goats_buf;
}

/// Gini coefficient of non-negative values (sorts in place).
pub fn gini(v: &mut [f32]) -> f32 {
    let n = v.len();
    if n < 2 {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let total: f32 = v.iter().sum();
    if total <= 1e-3 {
        return 0.0;
    }
    let mut acc = 0.0;
    for (i, &x) in v.iter().enumerate() {
        acc += (2.0 * (i as f32 + 1.0) - n as f32 - 1.0) * x;
    }
    (acc / (n as f32 * total)).clamp(0.0, 1.0)
}

/// Enter ages, fall into Dark Ages, climb back.
pub fn climb(w: &mut World, dt: f32) {
    for f in 0..MAX_FACTIONS {
        if !w.factions[f].alive {
            continue;
        }
        let fac = &mut w.factions[f];
        fac.age_t += dt;
        fac.dark_t = (fac.dark_t - dt).max(0.0);
        fac.collapse_cd = (fac.collapse_cd - dt).max(0.0);
        let age = fac.age;
        let m = fac.metrics;
        // ---- collapse ----
        fac.low_stab_t = if m.stability < COLLAPSE_STABILITY_BELOW && m.settlements > 0 { fac.low_stab_t + dt } else { 0.0 };
        let floor = COLLAPSE_POP_SHARE * entry_population(age);
        fac.low_pop_t = if age > 1 && (m.population as f32) < floor { fac.low_pop_t + dt } else { 0.0 };
        let why = if fac.low_stab_t >= COLLAPSE_FOR_SECONDS {
            1
        } else if fac.low_pop_t >= POP_GRACE {
            2
        } else {
            0
        };
        if why != 0 && age > 1 && fac.collapse_cd <= 0.0 {
            collapse(w, f as u8, why);
            continue;
        }
        // ---- the next age ----
        let fac = &mut w.factions[f];
        if age as usize >= AGE_COUNT || fac.dark_t > 0.0 {
            fac.ready_t = 0.0;
            continue;
        }
        if m.can_enter(age + 1) {
            fac.ready_t += dt;
        } else {
            fac.ready_t = 0.0;
        }
        if fac.ready_t >= READY_SECONDS && fac.age_t >= min_dwell(age) {
            enter(w, f as u8, age + 1);
        }
    }
}

/// A civilisation enters age `n`.
pub fn enter(w: &mut World, f: u8, n: u8) {
    let fac = &mut w.factions[f as usize];
    let recovered = n <= fac.peak_age;
    fac.age = n;
    fac.age_t = 0.0;
    fac.ready_t = 0.0;
    fac.low_pop_t = 0.0;
    fac.peak_age = fac.peak_age.max(n);
    fac.ups += 1;
    let cap = fac.capital;
    w.stats.age_ups += 1;
    if recovered {
        w.stats.recoveries += 1;
    }
    w.stats.max_age = w.stats.max_age.max(n as u32);
    refresh_fx(w, f);
    // The capital celebrates; the realm takes heart.
    for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == f) {
        v.unrest = (v.unrest - 0.08).max(0.0);
    }
    let (x, y) = match w.villages.get_mut(cap as usize) {
        Some(v) if v.alive => {
            v.festival_t = v.festival_t.max(10.0);
            (v.x, v.y)
        }
        _ => w.factions[f as usize].start,
    };
    w.stats.festivals += 1;
    w.log_x(ev::AGE_UP, f, NO_F, x, y, n as i32, recovered as i32);
}

/// A Dark Age: back one age.
pub fn collapse(w: &mut World, f: u8, why: i32) {
    let fac = &mut w.factions[f as usize];
    if fac.age <= 1 {
        return;
    }
    fac.age -= 1;
    fac.age_t = 0.0;
    fac.ready_t = 0.0;
    fac.low_stab_t = 0.0;
    fac.low_pop_t = 0.0;
    fac.dark_t = DARK_SECONDS;
    fac.collapse_cd = COLLAPSE_GRACE;
    fac.falls += 1;
    fac.metrics.knowledge *= 0.9;
    let n = fac.age;
    w.stats.collapses += 1;
    refresh_fx(w, f);
    let (x, y) = w.villages.get(w.factions[f as usize].capital as usize).filter(|v| v.alive).map(|v| (v.x, v.y)).unwrap_or(w.factions[f as usize].start);
    w.log_x(ev::COLLAPSE, f, NO_F, x, y, n as i32, why);
}

/// Buildings of ages the civilisation has lost go dormant (and wake when it climbs back).
pub fn refresh_fx(w: &mut World, f: u8) {
    let age = w.factions[f as usize].age;
    for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == f) {
        v.fx = village_fx(v.built, age);
    }
}

/// Knowledge a civilisation gains passively each second: elders' stories, libraries.
fn passive_lore(w: &World, f: &Faction) -> f32 {
    let elders = f.pop as f32 * 0.35;
    let mut l = 0.0012 * elders;
    for v in w.villages.iter().filter(|v| v.alive && v.owner == f.id) {
        l += v.fx.lore;
    }
    l
}

/// Taxes, banks, reform, faith, knowledge and the crisis timers. Once a second.
pub fn economy(w: &mut World, dt: f32) {
    let focus = w.inputs.focus.clamp(0.0, 1.0);
    let calm = w.inputs.calm.clamp(0.0, 1.0);
    let agitation = w.inputs.agitation();
    let anger = w.inputs.anger.clamp(0.0, 1.0);
    for fi in 0..MAX_FACTIONS {
        if !w.factions[fi].alive {
            continue;
        }
        let f = w.factions[fi].clone();
        let age = f.age;
        let wgt = if f.is_player { 1.0 } else { 0.6 };
        // Knowledge: libraries and elders (workers add theirs as they work).
        let censor = if f.censor_t > 0.0 { 0.5 } else { 1.0 };
        let lore = passive_lore(w, &f) * (1.0 + focus) * censor;
        // Tally the elite and the taxmen.
        let (mut taxmen, mut elite, mut reformers, mut agitators, mut bankers) = (0u32, 0u32, 0u32, 0u32, 0u32);
        for a in w.agents.iter().filter(|a| a.alive && a.faction as usize == fi) {
            match a.job {
                Job::Tax => taxmen += 1,
                Job::Reformer => reformers += 1,
                Job::Agitator => agitators += 1,
                Job::Banker => bankers += 1,
                _ => {}
            }
            elite += info(a.job).elite as u32;
        }
        let fac = &mut w.factions[fi];
        fac.metrics.knowledge += lore * dt;
        fac.drought_t = (fac.drought_t - dt).max(0.0);
        fac.fortify_t = (fac.fortify_t - dt).max(0.0);
        fac.strike_t = (fac.strike_t - dt).max(0.0);
        fac.censor_t = (fac.censor_t - dt).max(0.0);
        fac.blackout_t = (fac.blackout_t - dt).max(0.0);
        fac.crisis_cd -= dt;
        fac.tax_t += dt;
        // Taxes (Kingdoms on): every 5 s the taxmen take a share of the commoners' goats; the
        // elite keep most of it, the treasury the rest. Parliaments halve the rate.
        let parliament = w.villages.get(f.capital as usize).map_or(false, |v| v.alive && has(v, Building::Parliament, age));
        if fac.tax_t >= 5.0 {
            fac.tax_t = 0.0;
            if taxmen > 0 {
                let reach = (taxmen as f32 * 25.0 / f.pop.max(1) as f32).min(1.0);
                let rate = (0.06 + 0.08 * f.traits.aggressiveness) * reach * if parliament { 0.5 } else { 1.0 };
                let mut pot = 0.0;
                for a in w.agents.iter_mut().filter(|a| a.alive && a.faction as usize == fi && !info(a.job).elite) {
                    let t = a.goats * rate;
                    a.goats -= t;
                    pot += t;
                }
                let to_elite = if elite > 0 { pot * 0.6 } else { 0.0 };
                w.factions[fi].treasury += pot - to_elite;
                if elite > 0 {
                    let each = to_elite / elite as f32;
                    for a in w.agents.iter_mut().filter(|a| a.alive && a.faction as usize == fi && info(a.job).elite) {
                        a.goats += each;
                    }
                }
            }
            // Rents: from the Chiefdoms the elite draw goats from the land they hold.
            if age >= Job::Chief.unlock_age() {
                for a in w.agents.iter_mut().filter(|a| a.alive && a.faction as usize == fi && info(a.job).elite) {
                    a.goats += match a.job {
                        Job::Knight | Job::Banker | Job::Chief => 1.2,
                        _ => 0.6,
                    };
                }
            }
            // Credit: bankers make the rich richer (and the realm wealthier).
            if bankers > 0 {
                let k = (0.015 * bankers as f32).min(0.1);
                for a in w.agents.iter_mut().filter(|a| a.alive && a.faction as usize == fi && a.goats > 4.0) {
                    a.goats *= 1.0 + k;
                }
            }
            // Reform: parliaments and reformers spread the goats back out.
            if parliament || reformers > 0 {
                let share = (0.04 + 0.02 * reformers as f32).min(0.2);
                let mut pot = 0.0;
                let mut n = 0.0;
                for a in w.agents.iter_mut().filter(|a| a.alive && a.faction as usize == fi) {
                    if info(a.job).elite || a.goats > 8.0 {
                        let t = a.goats * share;
                        a.goats -= t;
                        pot += t;
                    } else {
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    for a in w.agents.iter_mut().filter(|a| a.alive && a.faction as usize == fi && !info(a.job).elite && a.goats <= 8.0) {
                        a.goats += pot / n;
                    }
                }
            }
        }
        // Agitators fan the resentment of inequality (which itself feeds unrest's pressure).
        let ineq = f.metrics.inequality;
        let stir = agitators as f32 * 0.002 * ineq * dt;
        // Faith drifts toward what the people feel: cohesion, calm, hard times, kingship.
        let target = (0.27 + 0.25 * f.traits.cohesion + 0.2 * calm * wgt + 0.12 * (age >= 9) as u8 as f32 + 0.1 * (age >= 10) as u8 as f32).min(0.95);
        for v in w.villages.iter_mut().filter(|v| v.alive && v.owner as usize == fi) {
            v.unrest = (v.unrest + stir).min(1.5);
            v.faith = (v.faith + (target - v.faith) * 0.01 * dt + v.fx.faith * dt).clamp(0.0, 1.0);
            v.unrest = (v.unrest - v.fx.order * dt).max(0.0);
            v.food += v.fx.food * dt;
            v.goods += v.fx.goods * dt + 0.01 * dt;
        }
        let made: f32 = w.villages.iter().filter(|v| v.alive && v.owner as usize == fi).map(|v| v.fx.goods * dt).sum();
        w.factions[fi].prod_acc += made;
        let _ = (agitation, anger);
    }
}

/// The crises of each civilisation's age (`ages_table::AgeDef::crises`), emergent: each has a
/// trigger in the simulation that the player's mind tilts. Checked once a second.
pub fn crises(w: &mut World, dt: f32) {
    let calm = w.inputs.calm.clamp(0.0, 1.0);
    let agitation = w.inputs.agitation();
    let anger = w.inputs.anger.clamp(0.0, 1.0);
    let focus = w.inputs.focus.clamp(0.0, 1.0);
    for fi in 0..MAX_FACTIONS as u8 {
        let f = w.factions[fi as usize].clone();
        if !f.alive || f.crisis_cd > 0.0 || f.villages == 0 {
            continue;
        }
        let wgt = if f.is_player { 1.0 } else { 0.6 };
        let m = f.metrics;
        let list = crate::ages::def(f.age).crises;
        for &key in list {
            let (p, hit): (f32, bool) = match key {
                "predators" => (0.004, true),
                "cold_snap" => (if w.night > 0.6 { 0.006 * (1.0 + wgt * agitation - 0.7 * calm) } else { 0.0 }, true),
                "lost_at_sea" => (0.003, true),
                "drought" => (0.004 * (1.0 + 2.0 * wgt * agitation - calm), true),
                "locusts" => (0.003, true),
                "granary_fire" => (0.003, true),
                "flood" => (0.002, true),
                "bandits" => (0.004 * (1.0 - f.traits.friendliness * 0.5), m.wealth > 10.0),
                "ore_war" => (0.004, true),
                "ore_depletion" => (0.002, true),
                "tax_revolt" => (0.006 * (m.inequality + (1.0 - m.stability)), true),
                "schism" => (0.006 * (1.0 - m.faith) * (1.0 + wgt * anger), true),
                "barbarians" => (0.003, true),
                "plague" => (0.004 * (1.0 + m.links as f32 * 0.05), true),
                "shipwreck_storms" => (0.003, m.coastal),
                "censorship" => (0.01 * wgt * anger, true),
                "market_crash" => (0.003 * (1.0 + 3.0 * wgt * agitation - calm), m.wealth > 50.0),
                "general_strike" => (0.004 * m.inequality, true),
                "blackout" => (0.004 * (1.0 + 3.0 * wgt * agitation - focus), true),
                "world_war" => (0.002, true),
                "panic" => (0.003 * (1.0 + 4.0 * wgt * agitation), true),
                "cyber_war" => (0.002, true),
                "launch_failure" => (0.004 * (1.0 + 2.0 * wgt * agitation - focus), true),
                "colony_collapse" => (0.003, m.colonies > 0),
                // Famine, succession splits, province and peasant revolts and civil wars come out
                // of the simulation itself (hunger, chiefs' deaths, unrest, inequality).
                _ => (0.0, false),
            };
            if !hit || p <= 0.0 || !w.rng.chance(p.max(0.0) * dt) {
                continue;
            }
            if strike_crisis(w, fi, key) {
                let idx = CRISES.iter().position(|&c| c == key).unwrap_or(0) as i32;
                w.stats.crises += 1;
                w.factions[fi as usize].crisis_cd = 25.0;
                let (x, y) = w.villages.get(f.capital as usize).filter(|v| v.alive).map(|v| (v.x, v.y)).unwrap_or(f.start);
                w.log_x(ev::CRISIS, fi, NO_F, x, y, f.age as i32, idx);
                break;
            }
        }
    }
}

/// A random living village of civilisation `f` (optionally only those passing `ok`).
fn some_village(w: &mut World, f: u8, ok: impl Fn(&Village) -> bool) -> Option<usize> {
    let c: Vec<usize> = w.villages.iter().enumerate().filter(|(_, v)| v.alive && v.owner == f && ok(v)).map(|(i, _)| i).collect();
    if c.is_empty() {
        None
    } else {
        Some(c[w.rng.below(c.len())])
    }
}

/// Kill up to `n` agents of `f` doing `job` (any job if None).
fn lose_people(w: &mut World, f: u8, job: Option<Job>, n: usize, cause: Cause) -> usize {
    let mut k = 0;
    for i in 0..w.agents.len() {
        if k >= n {
            break;
        }
        let a = &w.agents[i];
        if a.alive && a.faction == f && job.map_or(true, |j| a.job == j) {
            w.kill(i, cause);
            k += 1;
        }
    }
    k
}

fn strike_crisis(w: &mut World, f: u8, key: &str) -> bool {
    match key {
        "predators" => lose_people(w, f, Some(Job::Hunt), 1, Cause::Disaster) + lose_people(w, f, Some(Job::Forage), 1, Cause::Disaster) > 0,
        "lost_at_sea" => lose_people(w, f, Some(Job::Fish), 2, Cause::Disaster) > 0,
        "shipwreck_storms" => lose_people(w, f, Some(Job::Sailor), 2, Cause::Disaster) + lose_people(w, f, Some(Job::Colonist), 1, Cause::Disaster) > 0,
        "cold_snap" => {
            let mut hit = false;
            for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == f && v.built & (1 << Building::Hut as u8) == 0) {
                v.food *= 0.7;
                v.unrest = (v.unrest + 0.1).min(1.5);
                hit = true;
            }
            hit
        }
        "drought" => {
            w.factions[f as usize].drought_t = 30.0;
            true
        }
        "locusts" | "granary_fire" | "flood" => {
            let Some(v) = some_village(w, f, |v| v.food > 3.0) else { return false };
            let vil = &mut w.villages[v];
            vil.food *= if key == "locusts" { 0.5 } else { 0.3 };
            vil.unrest = (vil.unrest + 0.1).min(1.5);
            if key == "granary_fire" {
                let i = w.d.idx(vil.x + 1, vil.y);
                w.ignite_tile(i);
            }
            true
        }
        "bandits" | "market_crash" => {
            let k = if key == "bandits" { 0.85 } else { 0.6 };
            for a in w.agents.iter_mut().filter(|a| a.alive && a.faction == f) {
                a.goats *= k;
            }
            w.factions[f as usize].treasury *= k;
            for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == f) {
                v.unrest = (v.unrest + if key == "market_crash" { 0.15 } else { 0.03 }).min(1.5);
            }
            true
        }
        "ore_war" | "world_war" | "cyber_war" => {
            let mut hit = false;
            for o in 0..MAX_FACTIONS as u8 {
                if o != f && w.factions[o as usize].alive && w.rel(f, o).met && w.rel(f, o).near {
                    w.rel_mut(f, o).tension += if key == "world_war" { 0.5 } else { 0.25 };
                    hit = true;
                    if key == "ore_war" {
                        break;
                    }
                }
            }
            hit
        }
        "ore_depletion" | "general_strike" | "blackout" | "launch_failure" => {
            let fac = &mut w.factions[f as usize];
            fac.strike_t = fac.strike_t.max(if key == "ore_depletion" { 30.0 } else { 20.0 });
            if key == "launch_failure" {
                lose_people(w, f, Some(Job::Astronaut), 2, Cause::Disaster);
            }
            if key == "blackout" {
                w.factions[f as usize].blackout_t = 20.0;
            }
            true
        }
        "tax_revolt" => crisis_strife(w, f, StrifeKind::TaxRevolt),
        "schism" => crisis_strife(w, f, StrifeKind::Schism),
        "barbarians" => {
            let Some(v) = some_village(w, f, |v| v.pop > 3) else { return false };
            w.villages[v].food *= 0.5;
            w.villages[v].unrest = (w.villages[v].unrest + 0.15).min(1.5);
            let home = v as u16;
            let mut k = 0;
            for i in 0..w.agents.len() {
                if k < 2 && w.agents[i].alive && w.agents[i].home == home {
                    w.kill(i, Cause::Combat);
                    k += 1;
                }
            }
            true
        }
        "plague" => {
            let Some(v) = some_village(w, f, |v| v.pop >= 6) else { return false };
            if w.villages[v].plague < 0.3 {
                w.villages[v].plague = 0.45;
                w.stats.plagues += 1;
                let (x, y) = (w.villages[v].x, w.villages[v].y);
                w.log(ev::PLAGUE, f, NO_F, x, y, 0);
            }
            true
        }
        "censorship" => {
            w.factions[f as usize].censor_t = 30.0;
            true
        }
        "panic" => {
            for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == f) {
                v.unrest = (v.unrest + 0.15).min(1.5);
            }
            true
        }
        "colony_collapse" => {
            let cap_land = w.villages.get(w.factions[f as usize].capital as usize).map(|v| w.terrain.land(v.x, v.y)).unwrap_or(0);
            let t = &w.terrain;
            let colony = w.villages.iter().position(|v| v.alive && v.owner == f && t.land(v.x, v.y) != cap_land);
            match colony {
                Some(v) => {
                    w.villages[v].food = 0.0;
                    w.villages[v].unrest = 1.0;
                    true
                }
                None => false,
            }
        }
        _ => false,
    }
}
