//! The twenty ages and agent-vs-agent conflict (#69): the generated ladder, age entry and
//! collapse, the escalation ladder, fights inside one civilisation and its civil wars, morale
//! and routs, terrain and weapons, the player's effects, the speed control and night.

use moc_civ::ages::{self, entry_population, min_dwell, READY_SECONDS};
use moc_civ::ages_table::{self, Metric, AGES, AGE_COUNT, FORMATIONS_FROM_AGE, INSIDE_ONE_CIV, WAR_FROM_AGE};
use moc_civ::conflict::{self, StrifeKind, BATTLE, CIVIL_WAR, FEUD, NONE, SCUFFLE};
use moc_civ::jobs::{self, Building};
use moc_civ::planet::Dims;
use moc_civ::politics::{self, declare_war, decide, Decision, Policy};
use moc_civ::powers::{cast, lightning, CAST_PEACE};
use moc_civ::terrain::{GRASS, HILLS};
use moc_civ::*;

const DT: f32 = 1.0 / 30.0;

fn flat(seed: u32, kind: u8) -> World {
    World::with_terrain(seed, Terrain::uniform(Dims::SMALL, kind))
}

fn traits(aggr: f32, friend: f32) -> Traits {
    Traits { friendliness: friend, aggressiveness: aggr, violence: 0.6, fertility: 0.6, cohesion: 0.6 }
}

fn civ(w: &mut World, x: i32, y: i32, n: usize, job: Job, t: Traits) -> (u8, u16) {
    let f = w.add_faction(1000 + x as u32, SeedSource::Run, t, NO_F, None);
    let v = w.found_village(f, x, y, 6.0, false);
    w.factions[f as usize].capital = v;
    for _ in 0..n {
        w.spawn_agent(f, v, job, x, y);
    }
    w.recount();
    (f, v)
}

fn events(w: &World, kind: i32) -> Vec<Event> {
    w.events.iter().filter(|e| e.kind == kind).copied().collect()
}

/// Fight with nobody walking: only the combat step, for `secs`.
fn melee(w: &mut World, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        w.grid_rebuild();
        agents::combat(w, DT);
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        if b == b'\r' {
            continue;
        }
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[test]
fn the_ladder_is_generated_from_data_ages_json() {
    let json = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/ages.json"));
    assert_eq!(fnv1a(json), ages_table::SOURCE_FNV, "data/ages.json changed: run python crates/moc-civ/gen_ages.py");
    assert_eq!(AGE_COUNT, 20);
    assert_eq!((AGES[0].name, AGES[19].name), ("Wanderers", "Interstellar"));
    for (i, a) in AGES.iter().enumerate() {
        assert_eq!(a.n as usize, i + 1);
        assert!(!a.all.is_empty() || !a.any.is_empty(), "age {} has entry conditions", a.n);
        assert!(a.weapon.power > 0.0 && a.weapon.range > 0.0);
        for j in a.jobs {
            assert!(j.unlock_age() <= a.n);
        }
    }
    // Empires: territory 0.25 OR two vassals (the "any" alternatives).
    let e = &AGES[10];
    assert_eq!(e.any.len(), 2);
    assert_eq!(e.any[0][0].metric, Metric::Territory);
    assert_eq!(e.any[1][0].metric, Metric::Vassals);
    assert_eq!((WAR_FROM_AGE, FORMATIONS_FROM_AGE), (7, 11));
    assert_eq!((INSIDE_ONE_CIV[0].from_age, INSIDE_ONE_CIV[0].to_age), (1, 6));
    assert_eq!((INSIDE_ONE_CIV[4].from_age, INSIDE_ONE_CIV[4].to_age), (16, 20));
    assert_eq!(Job::Scribe.unlock_age(), 9);
    assert_eq!(Building::Palace.unlock_age(), 9);
}

#[test]
fn ages_are_entered_when_their_conditions_hold() {
    let mut w = flat(1, GRASS);
    let (f, _) = civ(&mut w, 100, 160, 4, Job::Forage, traits(0.3, 0.6));
    let fi = f as usize;
    assert_eq!(w.factions[fi].age, 1);
    // Hearth Clans need 8 people and a day of food: 5 people never get there.
    w.factions[fi].metrics = Metrics { population: 5, settlements: 1, food_days: 3.0, stability: 1.0, ..Default::default() };
    for _ in 0..60 {
        ages::climb(&mut w, 1.0);
    }
    assert_eq!(w.factions[fi].age, 1, "not enough people");
    w.factions[fi].metrics.population = 8;
    let mut t = 0.0;
    while w.factions[fi].age == 1 && t < 30.0 {
        ages::climb(&mut w, 1.0);
        t += 1.0;
    }
    assert_eq!(w.factions[fi].age, 2);
    assert!(t >= READY_SECONDS && t <= READY_SECONDS + 1.0, "after the conditions held {READY_SECONDS} s ({t})");
    let up = events(&w, ev::AGE_UP);
    assert_eq!((up.len(), up[0].n, up[0].extra), (1, 2, 0));
    // An age takes time to settle: the next one waits for min_dwell even if its
    // conditions already hold.
    w.factions[fi].metrics = Metrics { population: 20, settlements: 1, food_days: 3.0, stability: 1.0, ..Default::default() };
    let mut t = 0.0;
    while w.factions[fi].age == 2 && t < 100.0 {
        ages::climb(&mut w, 1.0);
        t += 1.0;
    }
    assert_eq!(w.factions[fi].age, 3);
    assert!(t >= min_dwell(2), "dwelt {t} s");
    // Empires by either road: wide land, or two vassals.
    let mut m = Metrics::default();
    assert!(!m.can_enter(11));
    m.vassals = 2;
    assert!(m.can_enter(11), "friendly road: vassals");
    m.vassals = 0;
    m.territory = 0.3;
    assert!(m.can_enter(11), "aggressive road: conquest");
    // Not a tech tree: nothing to pick, only the civilisation's own state.
    assert_eq!(entry_population(10), 80.0, "Temples inherit Kingdoms' population");
}

#[test]
fn low_stability_brings_a_dark_age_and_the_age_is_won_back() {
    let mut w = flat(2, GRASS);
    let (f, cap) = civ(&mut w, 100, 160, 2, Job::Forage, traits(0.3, 0.6));
    let fi = f as usize;
    w.factions[fi].age = 9;
    w.factions[fi].peak_age = 9;
    w.villages[cap as usize].built |= 1 << Building::Palace as u8;
    ages::refresh_fx(&mut w, f);
    assert!(jobs::has(&w.villages[cap as usize], Building::Palace, 9));
    let scribe = w.spawn_agent(f, cap, Job::Scribe, 100, 160).unwrap();
    let id = w.agents[scribe].id;
    w.recount();
    let healthy = Metrics { population: 200, settlements: 5, knowledge: 30.0, stability: 0.7, food_days: 2.0, ..Default::default() };
    w.factions[fi].metrics = Metrics { stability: 0.1, ..healthy };
    for _ in 0..29 {
        ages::climb(&mut w, 1.0);
    }
    assert_eq!(w.factions[fi].age, 9, "29 s of misery is survivable");
    ages::climb(&mut w, 1.0);
    assert_eq!(w.factions[fi].age, 8, "30 s below 0.15 is a Dark Age");
    let c = events(&w, ev::COLLAPSE);
    assert_eq!((c.len(), c[0].n, c[0].extra), (1, 8, 1));
    assert!(w.factions[fi].dark_t > 0.0);
    // The lost age's buildings go dormant and its jobs stop.
    assert!(!jobs::has(&w.villages[cap as usize], Building::Palace, 8));
    assert!(w.villages[cap as usize].built & (1 << Building::Palace as u8) != 0, "dormant, not destroyed");
    jobs::rebalance(&mut w);
    let a = w.agents.iter().find(|a| a.id == id).unwrap();
    assert!(a.job.unlock_age() <= 8, "the scribe took an older job: {:?}", a.job);
    // Climbing back: no sooner than the Dark Age allows, then as soon as the age is earned.
    w.factions[fi].metrics = healthy;
    let mut t = 0.0;
    while w.factions[fi].age == 8 && t < 200.0 {
        ages::climb(&mut w, 1.0);
        t += 1.0;
    }
    assert_eq!(w.factions[fi].age, 9);
    assert!(t >= ages::DARK_SECONDS, "after the Dark Age ({t} s)");
    let up = events(&w, ev::AGE_UP);
    assert_eq!((up.last().unwrap().n, up.last().unwrap().extra), (9, 1), "logged as won back");
    assert!(jobs::has(&w.villages[cap as usize], Building::Palace, 9), "the palace wakes");
    assert_eq!(w.stats.collapses, 1);
    assert_eq!(w.stats.recoveries, 1);
}

#[test]
fn losing_too_many_people_drops_an_age() {
    let mut w = flat(3, GRASS);
    let (f, _) = civ(&mut w, 100, 160, 2, Job::Forage, traits(0.3, 0.6));
    let fi = f as usize;
    w.factions[fi].age = 12;
    w.factions[fi].peak_age = 12;
    // Medieval Realms ask for 150 people: below 60 % (90) the realm can't hold.
    w.factions[fi].metrics = Metrics { population: 95, stability: 0.8, settlements: 8, ..Default::default() };
    for _ in 0..20 {
        ages::climb(&mut w, 1.0);
    }
    assert_eq!(w.factions[fi].age, 12);
    w.factions[fi].metrics.population = 85;
    for _ in 0..(ages::POP_GRACE as usize + 1) {
        ages::climb(&mut w, 1.0);
    }
    assert_eq!(w.factions[fi].age, 11);
    assert_eq!(events(&w, ev::COLLAPSE)[0].extra, 2, "by population");
    // A rebel people starts in the highest age it can hold.
    assert_eq!(ages::sustainable_age(12, 85), 11);
    assert_eq!(ages::sustainable_age(9, 20), 6);
}

/// Two met neighbours, tension set by hand, levels read back.
fn neighbours() -> (World, u8, u8) {
    let mut w = flat(4, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.5, 0.5));
    let (b, _) = civ(&mut w, 130, 160, 0, Job::Forage, traits(0.5, 0.5));
    for (p, q) in [(a, b), (b, a)] {
        w.rel_mut(p, q).met = true;
        w.rel_mut(p, q).near = true;
    }
    (w, a, b)
}

fn at_tension(w: &mut World, a: u8, b: u8, t: f32) -> u8 {
    w.rel_mut(a, b).tension = t;
    w.rel_mut(b, a).tension = t - 0.3;
    conflict::update_levels(w, 1.0);
    conflict::refresh(w);
    w.rel(a, b).level
}

#[test]
fn conflicts_climb_the_ladder_and_calm_back_down() {
    let (mut w, a, b) = neighbours();
    assert_eq!(at_tension(&mut w, a, b, 0.1), NONE);
    assert_eq!(at_tension(&mut w, a, b, 0.2), SCUFFLE);
    assert_eq!(w.stance[a as usize * MAX_FACTIONS + b as usize], 1, "a scuffle: no killing");
    assert!(!w.is_hostile(a, b));
    assert_eq!(at_tension(&mut w, a, b, 0.4), FEUD);
    assert_eq!(w.stance[a as usize * MAX_FACTIONS + b as usize], 2);
    assert_eq!(at_tension(&mut w, a, b, 0.85), BATTLE);
    assert!(w.is_hostile(a, b), "a battle is lethal");
    assert_eq!(w.rel(b, a).level, BATTLE, "the same both ways");
    // Hysteresis: a little cooling keeps the level, more brings it down a step at a time.
    assert_eq!(at_tension(&mut w, a, b, 0.75), BATTLE);
    assert_eq!(at_tension(&mut w, a, b, 0.3), FEUD);
    assert_eq!(at_tension(&mut w, a, b, 0.0), NONE);
    let up: Vec<i32> = events(&w, ev::ESCALATE).iter().map(|e| e.n).collect();
    let down: Vec<i32> = events(&w, ev::DEESCALATE).iter().map(|e| e.n).collect();
    assert_eq!(up, vec![FEUD as i32, BATTLE as i32]);
    assert_eq!(down, vec![FEUD as i32, NONE as i32]);
    // War: declared from age 7, or after two battles.
    assert!(!conflict::may_declare(&w, a, b));
    w.rel_mut(a, b).battles = 2;
    assert!(conflict::may_declare(&w, a, b), "several battles");
    w.rel_mut(a, b).battles = 0;
    w.factions[a as usize].age = 7;
    assert!(conflict::may_declare(&w, a, b), "a declaration (age 7+)");
}

#[test]
fn a_battle_runs_its_course_and_counts() {
    let (mut w, a, b) = neighbours();
    at_tension(&mut w, a, b, 0.95);
    for _ in 0..26 {
        w.rel_mut(a, b).tension = 0.95;
        conflict::update_levels(&mut w, 1.0);
    }
    assert_eq!(w.rel(a, b).battles, 1);
    assert_eq!(events(&w, ev::BATTLE).len(), 1);
}

#[test]
fn a_scuffle_wounds_but_never_kills() {
    let (mut w, a, b) = neighbours();
    for k in 0..6 {
        w.spawn_agent(a, 0, Job::Forage, 115, 155 + k);
        w.spawn_agent(b, 1, Job::Forage, 116, 155 + k);
    }
    at_tension(&mut w, a, b, 0.2);
    melee(&mut w, 20.0);
    assert!(w.stats.brawls > 0, "they brawled");
    assert_eq!(w.stats.killed, 0);
    assert!(w.agents.iter().all(|x| x.alive && x.hp >= 0.35));
    assert!(w.agents.iter().any(|x| x.hp < 0.9), "and hurt each other");
}

#[test]
fn a_civilisation_alone_fights_itself() {
    let mut w = flat(5, GRASS);
    let (f, v) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.5, 0.5));
    assert_eq!(w.alive_factions().count(), 1);
    for k in 0..6 {
        let i = w.spawn_agent(f, v, Job::Warrior, 110, 155 + k).unwrap();
        w.agents[i].clan = 0;
        let j = w.spawn_agent(f, v, Job::Warrior, 111, 155 + k).unwrap();
        w.agents[j].clan = 1;
    }
    let k = conflict::begin(&mut w, f, StrifeKind::ClanFeud, FEUD, [0, 1], &[]).unwrap();
    assert!(w.agents.iter().all(|a| a.camp == k as u8 + 1), "both clans are in it");
    melee(&mut w, 20.0);
    let dead = w.agents.iter().filter(|a| !a.alive).count();
    assert!(dead >= 2, "a feud kills ({dead})");
    let s = &w.strifes[k];
    assert_eq!((s.deaths[0] + s.deaths[1]) as usize, dead);
    assert_eq!(w.stats.killed as usize, dead);
    assert_eq!(events(&w, ev::STRIFE).len(), 1);
}

/// A civilisation of `n` villages in a row, with rebels in the last two.
fn realm(n: usize, rebels_armed: bool, loyal_armed: bool) -> (World, u8, Vec<u16>) {
    let mut w = flat(6, GRASS);
    let (f, cap) = civ(&mut w, 100, 160, 3, Job::Forage, traits(0.5, 0.5));
    let mut vs = vec![cap];
    for k in 1..n {
        let x = 100 + 20 * k as i32;
        let v = w.found_village(f, x, 160, 20.0, false);
        w.spawn_agent(f, v, Job::Forage, x, 160);
        vs.push(v);
    }
    let rebels = vs[n - 2..].to_vec();
    for &v in &rebels {
        w.villages[v as usize].unrest = 1.2;
        if rebels_armed {
            for _ in 0..3 {
                let (x, y) = (w.villages[v as usize].x, w.villages[v as usize].y);
                w.spawn_agent(f, v, Job::Warrior, x, y);
            }
        }
    }
    if loyal_armed {
        for _ in 0..3 {
            w.spawn_agent(f, cap, Job::Warrior, 100, 160);
        }
    }
    w.recount();
    (w, f, rebels)
}

#[test]
fn a_civil_war_inside_one_civilisation_splits_it() {
    let (mut w, f, rebels) = realm(5, true, false);
    w.inputs = Inputs::with_stress(0.0, 0.0, 1.0);
    let k = conflict::begin(&mut w, f, StrifeKind::ProvinceRevolt, CIVIL_WAR, [0, 0], &rebels).unwrap();
    assert_eq!(events(&w, ev::CIVIL_WAR).len(), 1, "a civil war inside one civilisation");
    assert!(rebels.iter().all(|&v| w.villages[v as usize].side == 1));
    let [loyal, rebel] = w.strifes[k].fighters;
    assert!(rebel >= 6 && rebel > loyal, "the rebels rose in arms ({loyal} vs {rebel})");
    // Nobody wins outright here (no fighting is simulated), so the crown concedes
    // (`politics::decide`): the rebels keep what they hold.
    for _ in 0..(conflict::CIVIL_WAR_SETTLES as usize + 3) {
        conflict::strife_tick(&mut w, 1.0);
        for d in decide(&w, f) {
            politics::apply(&mut w, f, d);
        }
    }
    assert!(!w.strifes[k].alive, "decided");
    assert_eq!(w.alive_factions().count(), 2, "the civilisation split");
    let child = w.alive_factions().find(|c| c.id != f).unwrap().id;
    assert!(rebels.iter().all(|&v| w.villages[v as usize].owner == child), "the rebels took their villages");
    assert_eq!(w.factions[child as usize].parent, f);
    assert_eq!(events(&w, ev::REBELLION).len(), 1);
}

#[test]
fn rebels_without_an_army_are_crushed() {
    let (mut w, f, rebels) = realm(5, false, true);
    w.inputs = Inputs::with_stress(0.0, 0.0, 1.0);
    let k = conflict::begin(&mut w, f, StrifeKind::PeasantRevolt, CIVIL_WAR, [0, 0], &rebels).unwrap();
    // Whatever arms they took up are lost at once.
    for a in w.agents.iter_mut().filter(|a| a.side == 1 && a.job.fights()) {
        a.job = Job::Forage;
    }
    for _ in 0..20 {
        conflict::strife_tick(&mut w, 1.0);
    }
    assert!(!w.strifes[k].alive);
    assert_eq!(events(&w, ev::CRUSHED).len(), 1);
    assert_eq!(w.alive_factions().count(), 1, "no split");
    assert!(rebels.iter().all(|&v| w.villages[v as usize].owner == f && w.villages[v as usize].unrest < 0.2), "calmed");
}

#[test]
fn a_revolution_takes_over_rather_than_splitting() {
    let (mut w, f, rebels) = realm(5, true, false);
    w.inputs = Inputs::with_stress(0.0, 0.0, 1.0);
    let cap = w.factions[f as usize].capital;
    let rich = w.spawn_agent(f, cap, Job::Scribe, 100, 160).unwrap();
    w.agents[rich].goats = 50.0;
    let id = w.agents[rich].id;
    conflict::begin(&mut w, f, StrifeKind::Revolution, CIVIL_WAR, [0, 0], &rebels).unwrap();
    for _ in 0..12 {
        conflict::strife_tick(&mut w, 1.0);
    }
    assert_eq!(events(&w, ev::REVOLUTION).len(), 1);
    assert_eq!(w.alive_factions().count(), 1, "one civilisation, new rulers");
    assert!(rebels.contains(&w.factions[f as usize].capital), "the capital moved to the rebels");
    assert!(w.agents.iter().find(|a| a.id == id).unwrap().goats < 20.0, "the elite lost their goats");
}

#[test]
fn calm_cools_a_strife_and_anger_heats_it() {
    let mut heat = Vec::new();
    for inputs in [Inputs::with_stress(0.95, 0.0, 0.0), Inputs::with_stress(0.0, 0.0, 0.9)] {
        let (mut w, f, rebels) = realm(4, true, true);
        w.inputs = inputs;
        let k = conflict::begin(&mut w, f, StrifeKind::ProvinceRevolt, FEUD, [0, 0], &rebels).unwrap();
        for _ in 0..20 {
            conflict::strife_tick(&mut w, 1.0);
        }
        heat.push((w.strifes[k].heat, w.strifes[k].alive, w.strifes[k].level));
    }
    assert!(heat[0].0 < heat[1].0, "calm {heat:?}");
    assert!(heat[1].2 > FEUD, "anger escalates: {heat:?}");
}

#[test]
fn the_peace_cast_forces_a_truce_inside_and_between() {
    let (mut w, f, rebels) = realm(4, true, true);
    conflict::begin(&mut w, f, StrifeKind::ProvinceRevolt, BATTLE, [0, 0], &rebels).unwrap();
    let (b, _) = civ(&mut w, 200, 160, 2, Job::Warrior, traits(0.9, 0.1));
    declare_war(&mut w, b, f);
    cast(&mut w, CAST_PEACE, 140.0, 160.0);
    assert!(w.strifes.iter().all(|s| !s.alive), "the rising ends");
    assert!(events(&w, ev::TRUCE).iter().any(|e| e.a == e.b));
    assert_eq!(w.rel(b, f).war, 0);
    assert!(!w.busy[f as usize]);
}

#[test]
fn outnumbered_fighters_lose_heart_and_rout() {
    let mut w = flat(7, GRASS);
    let (a, va) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.8, 0.2));
    let (b, vb) = civ(&mut w, 140, 160, 0, Job::Forage, traits(0.5, 0.3));
    let lone = w.spawn_agent(b, vb, Job::Warrior, 120, 160).unwrap();
    let lone_id = w.agents[lone].id;
    for k in 0..6 {
        let i = w.spawn_agent(a, va, Job::Warrior, 119, 157 + k).unwrap();
        w.agents[i].morale = 0.9;
    }
    w.agents[lone].morale = 0.9;
    w.agents[lone].hp = 50.0; // it can take a beating, so we see its nerve go first
    declare_war(&mut w, a, b);
    melee(&mut w, 2.0);
    let l = w.agents.iter().find(|x| x.id == lone_id).unwrap();
    assert!(l.morale < ROUT_MORALE, "one against six breaks ({})", l.morale);
    assert_eq!(l.state, State::Flee, "and runs");
    assert!(w.stats.routs >= 1);
    let a_morale = w.agents.iter().filter(|x| x.faction == a).map(|x| x.morale).fold(1.0, f32::min);
    assert!(a_morale > 0.6, "the six hold ({a_morale})");
}

#[test]
fn lightning_breaks_a_charge() {
    let mut w = flat(8, GRASS);
    let (a, va) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.8, 0.2));
    let (b, vb) = civ(&mut w, 160, 160, 0, Job::Forage, traits(0.5, 0.3));
    for k in 0..8 {
        let i = w.spawn_agent(a, va, Job::Warrior, 126, 156 + k).unwrap();
        w.agents[i].morale = 1.0;
        let j = w.spawn_agent(b, vb, Job::Warrior, 132, 156 + k).unwrap();
        w.agents[j].morale = 1.0;
    }
    declare_war(&mut w, a, b);
    lightning(&mut w, 120.5, 160.0);
    let mean = |f: u8| {
        let v: Vec<f32> = w.agents.iter().filter(|x| x.alive && x.faction == f).map(|x| x.morale).collect();
        v.iter().sum::<f32>() / v.len() as f32
    };
    assert!(mean(a) < 0.6, "the struck side loses heart ({})", mean(a));
    assert!(mean(b) > mean(a) + 0.2, "the other side much less ({} vs {})", mean(b), mean(a));
}

/// One warrior of `a` hits one of `b` for half a second; how hurt is the defender?
fn one_blow(defender_ground: u8, attacker_in_river: bool, walls: bool, attacker_age: u8) -> f32 {
    let mut kinds = vec![GRASS; Dims::SMALL.n()];
    for y in 150..170 {
        kinds[y * Dims::SMALL.w + 121] = defender_ground;
    }
    let regions = Terrain::uniform(Dims::SMALL, GRASS).regions;
    let mut t = Terrain::from_kinds(Dims::SMALL, kinds, regions);
    if attacker_in_river {
        t.set_river(120, 160, true);
    }
    let mut w = World::with_terrain(9, t);
    let (a, va) = civ(&mut w, 60, 160, 0, Job::Forage, traits(0.8, 0.2));
    let (b, vb) = civ(&mut w, 122, 160, 0, Job::Forage, traits(0.5, 0.3));
    w.factions[a as usize].age = attacker_age;
    if walls {
        w.villages[vb as usize].built |= 1 << Building::Palisade as u8;
        w.factions[b as usize].age = 7;
        ages::refresh_fx(&mut w, b);
    }
    let i = w.spawn_agent(a, va, Job::Warrior, 120, 160).unwrap();
    let j = w.spawn_agent(b, vb, Job::Warrior, 121, 160).unwrap();
    w.agents[j].hp = 100.0;
    w.agents[j].morale = 1.0;
    w.agents[i].morale = 1.0;
    w.agents[i].hp = 100.0;
    declare_war(&mut w, a, b);
    w.agents[j].state = State::Siege; // stand still
    melee(&mut w, 0.5);
    100.0 - w.agents[j].hp
}

#[test]
fn terrain_walls_rivers_and_weapons_change_the_blows() {
    let open = one_blow(GRASS, false, false, 1);
    let hills = one_blow(HILLS, false, false, 1);
    let forest = one_blow(moc_civ::terrain::FOREST, false, false, 1);
    let ford = one_blow(GRASS, true, false, 1);
    let walled = one_blow(GRASS, false, true, 1);
    let bronze = one_blow(GRASS, false, false, 8);
    assert!(open > 0.0);
    let r = |x: f32| x / open;
    assert!((r(hills) - 1.0 / 1.3).abs() < 0.02, "hills ×1.3 ({})", r(hills));
    assert!((r(forest) - 1.0 / 1.2).abs() < 0.02, "forest ×1.2 ({})", r(forest));
    assert!((r(ford) - 0.7).abs() < 0.02, "across a river ×0.7 ({})", r(ford));
    assert!(r(walled) < 1.0 / 1.7, "walls ×1.8 ({})", r(walled));
    assert!(r(bronze) > 1.4, "bronze swords hit harder than clubs ({})", r(bronze));
}

#[test]
fn the_speed_control_runs_the_world_faster() {
    let mut w = World::new_sized(10, Dims::SMALL);
    w.set_speed(16.0);
    let t0 = w.time;
    for _ in 0..60 {
        w.advance(1.0 / 60.0);
    }
    assert!((w.time - t0 - 16.0).abs() < 0.05, "16 s of world in a second ({})", w.time - t0);
    w.set_speed(4.0);
    let t1 = w.time;
    w.advance(0.5);
    assert!((w.time - t1 - 2.0).abs() < 0.01);
    w.set_speed(1.0);
    let t2 = w.time;
    w.advance(1.0 / 60.0);
    assert!((w.time - t2 - 1.0 / 60.0).abs() < 1e-4);
}

#[test]
fn hearth_clans_sleep_at_night() {
    let mut w = flat(11, GRASS);
    let (f, _) = civ(&mut w, 100, 160, 6, Job::Forage, traits(0.3, 0.6));
    w.factions[f as usize].age = 2;
    // Midnight is halfway through the day.
    w.time = DAY_SECONDS * 0.5 - 1.0;
    for _ in 0..30 {
        w.step(DT);
    }
    assert!(w.night > SLEEP_NIGHT);
    assert!(w.agents.iter().filter(|a| a.faction == f).all(|a| a.state == State::Sleep), "all abed");
    // Wanderers (age 1) keep no hours.
    let mut w = flat(11, GRASS);
    let (f, _) = civ(&mut w, 100, 160, 6, Job::Forage, traits(0.3, 0.6));
    w.time = DAY_SECONDS * 0.5 - 1.0;
    for _ in 0..30 {
        w.step(DT);
    }
    assert!(w.agents.iter().filter(|a| a.faction == f).all(|a| a.state != State::Sleep));
}

/// Every war-and-peace decision goes through `politics::decide` (the rules), or a plugged-in
/// `Policy` (#100) in its place.
#[test]
fn war_and_peace_go_through_one_decision_seam() {
    let (mut w, a, b) = neighbours();
    w.factions[a as usize].age = 7;
    w.rel_mut(a, b).tension = 1.2;
    assert_eq!(decide(&w, a), vec![Decision::DeclareWar(b)], "hot and old enough: war");
    w.factions[a as usize].age = 1;
    assert!(!decide(&w, a).contains(&Decision::DeclareWar(b)), "too young to declare");
    // A policy that keeps the peace whatever happens.
    struct Dove;
    impl Policy for Dove {
        fn decide(&mut self, _w: &World, _civ: u8, out: &mut Vec<Decision>) {
            out.push(Decision::KeepPeace);
        }
    }
    let (mut w, a, b) = neighbours();
    w.factions[a as usize].age = 7;
    w.policy = Some(Box::new(Dove));
    for _ in 0..30 {
        w.rel_mut(a, b).tension = 1.5;
        w.rel_mut(b, a).tension = 1.5;
        w.step(0.1);
    }
    assert_eq!(w.stats.wars + w.stats.raids, 0, "the policy decides, not the rules");
    // And one that goes to war at once.
    struct Hawk;
    impl Policy for Hawk {
        fn decide(&mut self, w: &World, civ: u8, out: &mut Vec<Decision>) {
            for o in w.alive_factions().filter(|o| o.id != civ) {
                if w.rel(civ, o.id).war == 0 {
                    out.push(Decision::DeclareWar(o.id));
                }
            }
        }
    }
    let (mut w, a, b) = neighbours();
    w.policy = Some(Box::new(Hawk));
    for _ in 0..12 {
        w.step(0.1);
    }
    assert!(w.at_war(a, b));
}
