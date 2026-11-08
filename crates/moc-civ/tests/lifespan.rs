//! How long people live and how fast they starve (#298), and that longer lives do not make
//! a population run away.

use moc_civ::terrain::{GRASS, SAND};
use moc_civ::*;

const DT: f32 = 1.0 / 30.0;

fn traits(fertility: f32) -> Traits {
    Traits { friendliness: 0.6, aggressiveness: 0.3, violence: 0.6, fertility, cohesion: 0.6 }
}

/// A civilisation with one village (its capital) and `n` foragers there.
fn civ(w: &mut World, x: i32, y: i32, n: usize, fertility: f32) -> (u8, u16) {
    let f = w.add_faction(1000 + x as u32, SeedSource::Run, traits(fertility), NO_F, None);
    let v = w.found_village(f, x, y, 6.0, false);
    w.factions[f as usize].capital = v;
    for _ in 0..n {
        w.spawn_agent(f, v, Job::Forage, x, y);
    }
    w.recount();
    (f, v)
}

fn run(w: &mut World, secs: f32, dt: f32) {
    for _ in 0..(secs / dt).round() as usize {
        w.step(dt);
    }
}

/// A barren world: no food on the land and none growing back.
fn barren(seed: u32) -> World {
    let mut w = World::with_terrain(seed, Terrain::uniform(Dims::SMALL, SAND));
    w.food.amt.iter_mut().for_each(|a| *a = 0.0);
    w.food.t.iter_mut().for_each(|t| *t = 1e9);
    w
}

/// Run with every village's larder kept empty (soil slowly refills it otherwise).
fn run_hungry(w: &mut World, secs: f32) {
    for _ in 0..(secs / DT).round() as usize {
        w.villages.iter_mut().for_each(|v| v.food = 0.0);
        w.step(DT);
    }
}

fn alive(w: &World, id: u32) -> bool {
    w.agents.iter().any(|a| a.id == id && a.alive)
}

#[test]
fn people_live_three_to_five_days() {
    assert_eq!(lifespan::MIN, 3.0 * DAY_SECONDS);
    assert_eq!(lifespan::MAX, 5.0 * DAY_SECONDS);
    assert_eq!((lifespan::MIN, lifespan::MAX), (240.0, 400.0));
    let mut w = World::with_terrain(1, Terrain::uniform(Dims::SMALL, GRASS));
    civ(&mut w, 100, 160, 300, 0.5);
    let lives: Vec<f32> = w.agents.iter().map(|a| a.life).collect();
    assert_eq!(lives.len(), 300);
    assert!(lives.iter().all(|l| (lifespan::MIN..lifespan::MAX).contains(l)), "every life in 240..400 s");
    let (lo, hi) = lives.iter().fold((f32::MAX, 0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
    assert!(lo < 260.0 && hi > 380.0, "the whole band is used ({lo:.0}..{hi:.0})");
    // The founders of a new world start as young adults, far from old age.
    let w = World::new_sized(42, Dims::SMALL);
    assert!(w.agents.iter().all(|a| a.age < lifespan::START_AGE_MAX && a.life - a.age >= lifespan::MIN - lifespan::START_AGE_MAX));
}

#[test]
fn a_newborn_outlives_three_days_and_then_dies_of_old_age() {
    let mut w = World::with_terrain(2, Terrain::uniform(Dims::SMALL, GRASS));
    let (_, v) = civ(&mut w, 100, 160, 1, 0.0);
    let (id, life) = (w.agents[0].id, w.agents[0].life);
    w.villages[v as usize].food = 1e6; // never hungry
    run(&mut w, lifespan::MIN - 1.0, 0.1);
    assert!(alive(&w, id), "alive after {} s", lifespan::MIN - 1.0);
    assert_eq!(w.stats.old_age, 0);
    run(&mut w, life - (lifespan::MIN - 1.0) + 1.0, 0.1);
    assert!(!alive(&w, id), "dead of old age at {life:.0} s");
    assert!(w.stats.old_age >= 1, "old age is a cause of death: nobody is immortal");
}

#[test]
fn an_empty_belly_kills_in_forty_seconds_a_full_one_in_ninety() {
    assert_eq!(EAT_RATE, 1.0 / 50.0, "the food economy is unchanged");
    assert_eq!(STARVE_HP_RATE, 1.0 / 40.0);
    let mut w = barren(3);
    let (_, v) = civ(&mut w, 100, 160, 2, 0.0);
    w.villages[v as usize].food = 0.0;
    let (empty, full) = (w.agents[0].id, w.agents[1].id);
    w.agents[0].food = 0.0;
    w.agents[1].food = 1.0;
    run_hungry(&mut w, 38.0);
    assert!(alive(&w, empty), "an empty belly holds out for 38 s");
    run_hungry(&mut w, 4.0);
    assert!(!alive(&w, empty), "and is dead by 42 s");
    assert_eq!(w.stats.starved, 1);
    run_hungry(&mut w, 45.0);
    assert!(alive(&w, full), "a full belly and no food last 87 s");
    run_hungry(&mut w, 6.0);
    assert!(!alive(&w, full), "and kill by 93 s");
    assert_eq!(w.stats.starved, 2);
}

#[test]
fn plague_hits_the_old_by_their_own_lifespan_not_a_fixed_age() {
    let mut w = barren(4);
    let (_, v) = civ(&mut w, 100, 160, 2, 0.0);
    w.villages[v as usize].food = 0.0;
    w.villages[v as usize].plague = 1.0;
    for a in w.agents.iter_mut() {
        a.food = 0.0;
        a.life = 300.0;
    }
    // 100 s was "old" under the fixed 60 s threshold; now old is past 0.6 of a life (180 s).
    w.agents[0].age = 100.0;
    w.agents[1].age = 200.0;
    run_hungry(&mut w, 5.0);
    let lost = |i: usize| 1.0 - w.agents[i].hp;
    let starve = 5.0 * STARVE_HP_RATE;
    let (young, old) = (lost(0) - starve, lost(1) - starve);
    assert!(young > 0.0 && old > 0.0, "both sicken ({young:.3}, {old:.3})");
    assert!((old / young - 2.0).abs() < 0.05, "the old sicken twice as fast ({old:.3} vs {young:.3})");
}

#[test]
fn a_population_stays_bounded_over_many_lifetimes() {
    // A small island of good land, one fertile civilisation, plenty of food: births are held
    // back only by village caps, the agent cap and food, so this is where a long life would
    // make the population run away.
    let mut w = World::with_terrain(5, Terrain::uniform(Dims::new(96, 48), GRASS));
    let (f, _) = civ(&mut w, 48, 24, 6, 1.0);
    let mut peak = 0usize;
    let mut late: Vec<usize> = Vec::new();
    let secs = 3.0 * lifespan::MAX;
    for k in 0..(secs / 10.0) as usize {
        run(&mut w, 10.0, 0.1);
        let pop = w.agents.iter().filter(|a| a.alive).count();
        let cap: usize = w.villages.iter().filter(|v| v.alive).map(|v| v.cap as usize).sum();
        assert!(pop <= w.cap_agents, "within the agent cap at {} s", k * 10);
        assert!(pop <= cap + 8, "within the village caps at {} s: {pop} people, room for {cap}", k * 10);
        peak = peak.max(pop);
        if (k * 10) as f32 >= secs - 2.0 * lifespan::MIN {
            late.push(pop);
        }
    }
    assert!(w.factions[f as usize].alive, "the civilisation lives on");
    assert!(w.stats.old_age > 0, "people die of old age");
    assert!(w.stats.births > w.stats.old_age, "and are replaced");
    let (lo, hi) = (*late.iter().min().unwrap(), *late.iter().max().unwrap());
    assert!(lo > 0 && lo * 2 >= hi, "the population holds steady over the last two lifetimes ({lo}..{hi}, peak {peak})");
}
