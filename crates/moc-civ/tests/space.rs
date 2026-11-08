//! The space age of the civ sandbox (#166): programs at age 19+, fleets from the economy,
//! launches at the galaxy's planets, what the galaxy does back, endgame tiers, the mind's tilt,
//! determinism and cost.

use moc_civ::planet::Dims;
use moc_civ::space::{self, SpaceResult, Target, SPACE_AGE};
use moc_civ::terrain::GRASS;
use moc_civ::*;
use moc_cosmos::war::Order;

fn traits(aggr: f32) -> Traits {
    Traits { friendliness: 0.4, aggressiveness: aggr, violence: 0.6, fertility: 0.6, cohesion: 0.6 }
}

/// A world with `n` civilisations of 30 people at age `age`, with an age-19 economy.
fn spacefarers(seed: u32, n: usize, age: u8, aggr: f32) -> World {
    let mut w = World::with_terrain(seed, Terrain::uniform(Dims::SMALL, GRASS));
    for k in 0..n {
        let (x, y) = (20 + 40 * k as i32, 40);
        let f = w.add_faction(1000 + k as u32, SeedSource::Run, traits(aggr), NO_F, None);
        let v = w.found_village(f, x, y, 6.0, false);
        w.factions[f as usize].capital = v;
        for _ in 0..30 {
            w.spawn_agent(f, v, Job::Forage, x, y);
        }
        let fac = &mut w.factions[f as usize];
        fac.age = age;
        fac.metrics.production = 220.0;
        fac.metrics.knowledge = 480.0;
        fac.metrics.wealth = 900.0;
    }
    w.recount();
    w
}

fn galaxy_targets() -> Vec<Target> {
    vec![
        // A farming world with no shields, a space world with strong ones, a rival civilisation's colony.
        Target { body: 1, orbit: 0.0, garrison: 12.0, value: 5.0, kind: 1 },
        Target { body: 2, orbit: 80.0, garrison: 25.0, value: 11.0, kind: 1 },
        Target { body: 3, orbit: 10.0, garrison: 6.0, value: 4.0, kind: 2 },
    ]
}

fn secs(w: &mut World, n: usize, inputs: Inputs) {
    for _ in 0..n {
        w.inputs = inputs;
        space::tick(w, 1.0);
    }
}

const NEUTRAL: Inputs = Inputs { calm: 0.3, focus: 0.3, stress: 0.1, anger: 0.0, joy: 0.0 };

#[test]
fn the_space_program_starts_at_age_19() {
    let mut w = spacefarers(1, 2, SPACE_AGE - 1, 0.5);
    secs(&mut w, 5, NEUTRAL);
    assert!(w.space.programs.iter().all(|p| !p.active), "nobody in space at age 18");
    w.factions[0].age = SPACE_AGE;
    secs(&mut w, 30, NEUTRAL);
    let p = w.space.programs[0];
    assert!(p.active && !w.space.programs[1].active);
    assert!(p.fleet_cap > 30.0 && p.fleet > 0.5 * p.fleet_cap, "the yards build a fleet: {p:?}");
    assert!(p.shields > 10.0 && p.defences > 5.0 && p.garrison > 4.0, "and defences: {p:?}");
    // The buffer JS reads.
    let o = 0;
    assert_eq!(w.space.info[o], 1.0);
    assert!((w.space.info[o + 1] - p.fleet).abs() < 1e-3);
}

#[test]
fn a_richer_civilisation_and_its_starports_build_a_bigger_fleet() {
    let mut w = spacefarers(2, 2, SPACE_AGE, 0.5);
    w.factions[1].metrics.production = 800.0;
    w.factions[1].metrics.knowledge = 1800.0;
    secs(&mut w, 3, NEUTRAL);
    let (a, b) = (w.space.programs[0].fleet_cap, w.space.programs[1].fleet_cap);
    assert!(b > a * 1.3, "{a} vs {b}");
    let cap = w.factions[0].capital as usize;
    w.villages[cap].built |= 1u64 << Building::Starport as u8;
    secs(&mut w, 2, NEUTRAL);
    assert!(w.space.programs[0].fleet_cap > a * 1.2, "a starport: {} → {}", a, w.space.programs[0].fleet_cap);
}

#[test]
fn a_ready_fleet_launches_at_the_galaxy() {
    let mut w = spacefarers(3, 1, SPACE_AGE, 0.8);
    space::set_targets(&mut w, 0, &galaxy_targets());
    let mut launches = Vec::new();
    for _ in 0..120 {
        secs(&mut w, 1, NEUTRAL);
        launches.extend(w.space.launches.drain(..));
    }
    assert!(!launches.is_empty(), "it launched");
    let l = launches[0];
    assert_eq!(l.civ, 0);
    assert!(l.strength >= 4.0 && [1, 2, 3].contains(&l.body));
    assert_ne!(l.body, 2, "not into the strongest shields first");
    assert!(launches.iter().any(|l| l.order == Order::Invade), "it invades worlds it can take: {launches:?}");
    assert!(w.space.programs[0].launches as usize == launches.len());
}

#[test]
fn no_targets_no_launches() {
    let mut w = spacefarers(4, 1, SPACE_AGE, 0.9);
    secs(&mut w, 120, NEUTRAL);
    assert!(w.space.launches.is_empty());
}

#[test]
fn the_galaxy_hits_back_and_pays_out() {
    let mut w = spacefarers(5, 1, SPACE_AGE, 0.5);
    secs(&mut w, 30, NEUTRAL);
    let (treasury, knowledge, pop) = (w.factions[0].treasury, w.factions[0].metrics.knowledge, w.factions[0].pop);
    let shields = w.space.programs[0].shields;
    space::apply_result(&mut w, 0, &SpaceResult { loot: 10.0, knowledge: 5.0, ..Default::default() });
    assert!(w.factions[0].treasury > treasury && w.factions[0].metrics.knowledge > knowledge);
    space::apply_result(&mut w, 0, &SpaceResult { hits: 0.2, shield_loss: shields * 0.5, ..Default::default() });
    let dead = w.agents.iter().filter(|a| !a.alive && a.faction == 0).count();
    assert_eq!(dead, (pop as f32 * 0.2).round() as usize, "a fifth of its people died from orbit");
    assert!(w.space.programs[0].shields < shields * 0.8);
    assert!(w.effects.iter().any(|e| e.kind == 1), "fire from the sky over the capital");
    // A colony off-world counts for the ages (Interstellar needs one).
    space::apply_result(&mut w, 0, &SpaceResult { colonies: 1, colony_econ: 4.0, ..Default::default() });
    assert_eq!(space::colonies(&w, 0), 1);
    let economy = w.factions[0].metrics;
    moc_civ::ages::update_metrics(&mut w, 1.0);
    assert!(w.factions[0].metrics.colonies >= 1);
    // (This test world has no workers: put its economy back after the measuring.)
    let m = &mut w.factions[0].metrics;
    (m.production, m.knowledge, m.wealth) = (economy.production, economy.knowledge, economy.wealth);
    let cap = w.space.programs[0].fleet_cap;
    secs(&mut w, 2, NEUTRAL);
    assert!(w.space.programs[0].fleet_cap > cap, "the colony feeds its fleet");
}

#[test]
fn interstellar_civilisations_climb_the_endgame_tiers() {
    let mut w = spacefarers(6, 2, SPACE_AGE, 0.5);
    w.factions[1].age = 20;
    secs(&mut w, 120, NEUTRAL);
    assert_eq!(w.space.programs[0].tier, 0, "no tiers before Interstellar");
    assert_eq!(w.space.programs[1].tier, 1, "orbital industry: {:?}", w.space.programs[1]);
    let cap1 = w.space.programs[1].fleet_cap;
    secs(&mut w, 200, NEUTRAL);
    assert_eq!(w.space.programs[1].tier, 1, "interplanetary colonies need a colony");
    space::apply_result(&mut w, 1, &SpaceResult { colonies: 1, ..Default::default() });
    secs(&mut w, 2, NEUTRAL);
    assert_eq!(w.space.programs[1].tier, 2);
    assert!(w.space.programs[1].fleet_cap > cap1 && cap1 > w.space.programs[0].fleet_cap);
}

fn launches_with(inputs: Inputs) -> u32 {
    let mut w = spacefarers(7, 4, SPACE_AGE, 0.5);
    for f in 0..4 {
        space::set_targets(&mut w, f, &galaxy_targets());
    }
    secs(&mut w, 400, inputs);
    w.space.programs.iter().map(|p| p.launches).sum()
}

#[test]
fn stress_launches_more_and_calm_less() {
    let stressed = launches_with(Inputs { calm: 0.0, focus: 0.3, stress: 1.0, anger: 0.5, joy: 0.0 });
    let calm = launches_with(Inputs { calm: 1.0, focus: 0.3, stress: 0.0, anger: 0.0, joy: 0.0 });
    assert!(stressed > 2 * calm.max(1), "stress {stressed}, calm {calm}");
}

#[test]
fn deterministic_for_a_seed_and_inputs() {
    let run = || {
        let mut w = spacefarers(8, 3, SPACE_AGE, 0.6);
        for f in 0..3 {
            space::set_targets(&mut w, f, &galaxy_targets());
        }
        let mut all = Vec::new();
        for k in 0..300 {
            let stress = (k / 50 % 2) as f32;
            secs(&mut w, 1, Inputs { calm: 0.2, focus: 0.3, stress, anger: 0.0, joy: 0.0 });
            all.extend(w.space.launches.drain(..).map(|l| (l.civ, l.body, l.order as u8, l.strength.to_bits())));
        }
        all
    };
    let a = run();
    assert!(!a.is_empty());
    assert_eq!(a, run());
}

#[test]
fn the_space_council_costs_microseconds() {
    let mut w = spacefarers(9, 20, SPACE_AGE, 0.6);
    for f in 0..20 {
        space::set_targets(&mut w, f, &galaxy_targets());
    }
    let t0 = std::time::Instant::now();
    let n = 2000;
    for _ in 0..n {
        space::tick(&mut w, 1.0);
        w.space.launches.clear();
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
    eprintln!("space council, 20 civilisations: {us:.1} µs");
    // It runs once a second; the sandbox's frame budget is milliseconds.
    assert!(us < 500.0, "{us} µs");
}
