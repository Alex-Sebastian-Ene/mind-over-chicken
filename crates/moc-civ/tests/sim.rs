//! Rules of the civilisation sim: start, growth, food, ageing, combat, capture, territory,
//! contact, war vs peace under the player's mood, rebellion, determinism and wrapping.

use moc_civ::agents::{set_goal, walk, ROUTE_FIELD};
use moc_civ::planet::{Dims, Paths};
use moc_civ::politics::{declare_war, slow_tick};
use moc_civ::powers::{lightning, set_player_traits};
use moc_civ::terrain::{GRASS, SAND};
use moc_civ::*;

const DT: f32 = 1.0 / 30.0;

fn flat(seed: u32, kind: u8) -> World {
    World::with_terrain(seed, Terrain::uniform(Dims::SMALL, kind))
}

fn traits(aggr: f32, friend: f32) -> Traits {
    Traits { friendliness: friend, aggressiveness: aggr, violence: 0.6, fertility: 0.6, cohesion: 0.6 }
}

/// A civilisation with one village (its capital) and `n` agents of `job` there.
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

fn run(w: &mut World, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        w.step(DT);
    }
}

fn count(w: &World, kind: i32) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

#[test]
fn starts_with_four_civilisations_of_four_agents_on_separate_regions() {
    let w = World::new_sized(42, Dims::SMALL);
    let civs: Vec<&Faction> = w.alive_factions().collect();
    assert_eq!(civs.len(), START_CIVS);
    let mut regions: Vec<u8> = civs.iter().map(|f| f.region).collect();
    regions.dedup();
    assert_eq!(regions.len(), 4, "each civilisation starts on its own continent");
    for f in &civs {
        assert_eq!(w.agents.iter().filter(|a| a.faction == f.id).count(), START_AGENTS);
        assert_eq!(f.seed_source, SeedSource::Run);
        assert!(!f.name.is_empty());
        assert_eq!(f.traits, Traits::from_seed(f.seed).clamped(), "traits come from the civilisation's own seed");
        for v in [f.traits.friendliness, f.traits.aggressiveness, f.traits.violence] {
            assert!((0.0..=1.0).contains(&v));
        }
        assert!(w.terrain.walkable(f.start.0, f.start.1));
    }
    let mut seeds: Vec<u32> = civs.iter().map(|f| f.seed).collect();
    seeds.sort_unstable();
    seeds.dedup();
    assert_eq!(seeds.len(), 4);
    assert!(civs[0].is_player && civs.iter().filter(|f| f.is_player).count() == 1);
    // Same run seed, same four civilisations.
    let again = World::new_sized(42, Dims::SMALL);
    assert_eq!(again.factions[2].seed, w.factions[2].seed);
    assert_eq!(again.factions[2].name, w.factions[2].name);
}

#[test]
fn the_default_planet_is_bigger_with_more_continents() {
    let w = World::new(3);
    assert_eq!(w.d, Dims::DEFAULT);
    assert!(w.d.w > moc_planet::PW && w.d.h > moc_planet::PH);
    assert!(w.terrain.regions.len() > 4, "{} continents", w.terrain.regions.len());
    assert_eq!(w.alive_factions().count(), 4);
    let land = w.terrain.walk.iter().filter(|&&b| b).count() as f32 / w.d.n() as f32;
    assert!(land > 0.08 && land < 0.6, "land fraction {land}");
}

#[test]
fn same_seed_same_history_different_seed_different_world() {
    let mut a = World::new_sized(9, Dims::SMALL);
    let mut b = World::new_sized(9, Dims::SMALL);
    let mut c = World::new_sized(10, Dims::SMALL);
    for w in [&mut a, &mut b, &mut c] {
        w.inputs = Inputs { calm: 0.3, focus: 0.4, stress: 0.7, anger: 0.1, ..Default::default() };
        run(w, 40.0);
    }
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(a.agents.len(), b.agents.len());
    assert_ne!(a.checksum(), c.checksum());
    assert_ne!(a.terrain.kind, c.terrain.kind);
}

#[test]
fn villages_grow_from_food_surplus_and_settle_new_villages() {
    let mut w = flat(1, GRASS);
    let (f, _) = civ(&mut w, 100, 160, 4, Job::Forage, traits(0.3, 0.6));
    run(&mut w, 75.0);
    w.recount();
    let fac = &w.factions[f as usize];
    assert!(fac.pop > 8, "pop {}", fac.pop);
    assert!(fac.villages >= 2, "villages {}", fac.villages);
    assert!(count(&w, ev::FOUNDED) >= 1);
}

#[test]
fn gatherers_bring_food_home() {
    let mut w = flat(2, GRASS);
    let (_, v) = civ(&mut w, 100, 160, 4, Job::Forage, traits(0.3, 0.6));
    w.villages[v as usize].food = 0.0;
    run(&mut w, 12.0);
    // Passive income alone is ~0.4 food in 12 s; four gatherers bring in much more.
    assert!(w.villages[v as usize].food > 1.0, "food {}", w.villages[v as usize].food);
}

#[test]
fn agents_starve_without_food_and_the_civilisation_dies_out() {
    let mut w = flat(3, SAND);
    w.food.amt.iter_mut().for_each(|a| *a = 0.0);
    w.food.t.iter_mut().for_each(|t| *t = 1e9); // no regrowth
    let (f, v) = civ(&mut w, 100, 160, 4, Job::Forage, traits(0.3, 0.6));
    w.villages[v as usize].food = 0.0;
    // A full belly lasts 50 s and an empty one kills in 40 s (#298): most are dead by ~100 s;
    // the sand's trickle of food into the larder keeps the last alive to ~200 s, no longer.
    run(&mut w, 300.0);
    assert!(w.agents.iter().all(|a| a.faction != f), "everyone starved");
    assert!(w.stats.starved >= 4, "starved {}", w.stats.starved);
    assert!(!w.factions[f as usize].alive);
    assert_eq!(count(&w, ev::WIPED), 1);
}

#[test]
fn agents_die_of_old_age() {
    let mut w = flat(4, GRASS);
    civ(&mut w, 100, 160, 2, Job::Forage, traits(0.3, 0.6));
    let life = w.agents[0].life;
    w.agents[0].age = life - 0.05;
    run(&mut w, 0.5);
    assert_eq!(w.stats.old_age, 1);
}

#[test]
fn combat_has_casualties_and_the_stronger_side_wins() {
    let mut w = flat(5, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 0, Job::Warrior, traits(0.8, 0.2));
    let (b, _) = civ(&mut w, 130, 160, 0, Job::Warrior, traits(0.5, 0.3));
    for k in 0..6 {
        w.spawn_agent(a, 0, Job::Warrior, 113, 158 + k);
    }
    for k in 0..2 {
        w.spawn_agent(b, 1, Job::Warrior, 116, 160 + k);
    }
    declare_war(&mut w, a, b);
    assert!(w.is_hostile(a, b) && w.is_hostile(b, a));
    run(&mut w, 12.0);
    let alive = |f: u8| w.agents.iter().filter(|x| x.faction == f && x.job == Job::Warrior).count();
    assert_eq!(alive(b), 0, "outnumbered warriors fall");
    assert!(alive(a) >= 3, "most of the larger band survives ({})", alive(a));
    assert!(w.factions[a as usize].kills >= 2);
    assert!(w.factions[b as usize].losses >= 2);
    assert!(w.stats.killed >= 2);
}

#[test]
fn an_undefended_village_is_captured_and_its_land_changes_hands() {
    let mut w = flat(6, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.8, 0.2));
    let (b, vb) = civ(&mut w, 125, 160, 0, Job::Forage, traits(0.3, 0.5));
    let (bx, by) = (w.villages[vb as usize].x, w.villages[vb as usize].y);
    let claimed: Vec<usize> = (0..w.d.n()).filter(|&i| w.claim[i] == vb).collect();
    assert!(claimed.len() > 50);
    assert!(claimed.iter().all(|&i| w.owner[i] == b));
    for k in 0..4 {
        let i = w.spawn_agent(a, 0, Job::Warrior, bx - 2, by - 1 + k).unwrap();
        w.agents[i].garrison = false;
    }
    declare_war(&mut w, a, b);
    run(&mut w, 15.0);
    assert_eq!(w.villages[vb as usize].owner, a, "village captured");
    assert!(claimed.iter().all(|&i| w.owner[i] == a), "its land changed owner");
    assert!(count(&w, ev::CAPTURE) >= 1);
}

#[test]
fn villages_claim_free_land_but_not_their_neighbours() {
    let mut w = flat(7, GRASS);
    let (a, va) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.3, 0.5));
    let r = START_RADIUS;
    for (ox, oy) in [(0, 0), (r - 1, 0), (0, -(r - 1)), (-3, 3)] {
        let i = w.d.idx(100 + ox, 160 + oy);
        assert_eq!((w.owner[i], w.claim[i]), (a, va));
    }
    assert_eq!(w.owner[w.d.idx(100 + r + 3, 160)], NO_F);
    // A neighbour founded 8 tiles away gets only the free land.
    let (b, vb) = civ(&mut w, 108, 160, 0, Job::Forage, traits(0.3, 0.5));
    assert_eq!(w.owner[w.d.idx(103, 160)], a, "a's land stays a's");
    assert_eq!((w.owner[w.d.idx(110, 160)], w.claim[w.d.idx(110, 160)]), (b, vb));
    // Growing the radius claims the ring.
    w.villages[va as usize].radius = r + 3;
    w.claim_land(va, r + 3);
    assert_eq!(w.owner[w.d.idx(100, 160 - r - 2)], a);
}

#[test]
fn meeting_another_civilisation_is_an_event() {
    let mut w = flat(8, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.3, 0.5));
    let (b, _) = civ(&mut w, 160, 160, 0, Job::Forage, traits(0.3, 0.5));
    w.spawn_agent(a, 0, Job::Warrior, 130, 160);
    w.spawn_agent(b, 1, Job::Warrior, 131, 161);
    w.step(DT);
    slow_tick(&mut w);
    assert!(w.rel(a, b).met && w.rel(b, a).met);
    run(&mut w, 2.0);
    assert_eq!(count(&w, ev::CONTACT), 1, "contact is logged once");
    assert!(!w.is_hostile(a, b), "meeting is not fighting");
}

/// Two neighbours, met, same temperament: a calm god keeps the peace, stress and an
/// angry face start a war.
#[test]
fn the_players_mood_tilts_war_and_peace() {
    let mut outcomes = Vec::new();
    for inputs in [Inputs::with_stress(0.95, 0.0, 0.0), Inputs::with_stress(0.0, 0.0, 0.8)] {
        let mut w = flat(9, GRASS);
        let (a, _) = civ(&mut w, 100, 160, 3, Job::Forage, traits(0.55, 0.45));
        let (b, _) = civ(&mut w, 125, 160, 3, Job::Forage, traits(0.55, 0.45));
        w.factions[a as usize].is_player = true;
        w.rel_mut(a, b).met = true;
        w.rel_mut(b, a).met = true;
        w.inputs = inputs;
        run(&mut w, 60.0);
        outcomes.push((w.stats.wars + w.stats.raids, w.rel(a, b).tension, w.stats.trades));
    }
    let (calm, angry) = (outcomes[0], outcomes[1]);
    assert_eq!(calm.0, 0, "calm: no raids or wars ({calm:?})");
    assert!(angry.0 >= 1, "angry: raids or war ({angry:?})");
    assert!(calm.1 < angry.1);
}

#[test]
fn restless_far_villages_split_off_as_a_new_civilisation() {
    let mut w = flat(10, GRASS);
    let (p, cap) = civ(&mut w, 100, 160, 4, Job::Forage, traits(0.5, 0.4));
    let far: Vec<u16> = (0..4).map(|k| w.found_village(p, 180 + 16 * k, 160, 5.0, false)).collect();
    for &v in &far {
        w.spawn_agent(p, v, Job::Forage, w.villages[v as usize].x, 160);
        w.villages[v as usize].unrest = 1.2;
    }
    w.time = 50.0;
    run(&mut w, 2.5);
    assert_eq!(w.stats.rebellions, 1);
    let e = w.events.iter().find(|e| e.kind == ev::REBELLION).copied().unwrap();
    assert_eq!(e.a, p as i32);
    let c = e.b as u8;
    let child = &w.factions[c as usize];
    assert!(child.alive && child.parent == p && child.seed_source == SeedSource::Rebellion);
    assert_ne!(child.color, w.factions[p as usize].color, "new colour");
    assert_ne!(child.traits, w.factions[p as usize].traits, "drifted traits");
    assert_eq!(w.villages[cap as usize].owner, p, "the capital stays loyal");
    let rebels = far.iter().filter(|&&v| w.villages[v as usize].owner == c).count();
    assert!(rebels >= 1 && rebels <= 2, "up to half the villages go ({rebels})");
    let rebel_village = far.iter().copied().find(|&v| w.villages[v as usize].owner == c).unwrap();
    let (rx, ry) = (w.villages[rebel_village as usize].x, w.villages[rebel_village as usize].y);
    assert_eq!(w.owner[w.d.idx(rx + 2, ry)], c, "its land goes with it");
    assert!(w.agents.iter().any(|a| a.faction == c), "and its people");
    assert!(w.rel(c, p).met && w.rel(c, p).tension > 0.5, "and it bears a grudge");
}

#[test]
fn walkers_take_the_short_way_across_the_date_line() {
    for d in [Dims::SMALL, Dims::DEFAULT] {
        let t = Terrain::uniform(d, GRASS);
        let mut w = World::with_terrain(1, t);
        let (f, v) = civ(&mut w, 40, 100, 0, Job::Forage, traits(0.3, 0.5));
        let i = w.spawn_agent(f, v, Job::Warrior, d.w as i32 - 3, 100).unwrap();
        let mut paths = Paths::new();
        let mut a = w.agents[i].clone();
        set_goal(&mut a, d, 4, 104);
        let mut arrived = false;
        for _ in 0..600 {
            paths.refill(2);
            arrived = walk(&mut a, &w.terrain, &mut paths, 7.0, DT, ROUTE_FIELD);
            assert!(a.x < 30.0 || a.x > d.w as f32 - 30.0, "went the long way: x = {}", a.x);
            if arrived {
                break;
            }
        }
        assert!(arrived, "never arrived; at ({}, {})", a.x, a.y);
        assert_eq!((a.x.floor() as i32, a.y.floor() as i32), (4, 104));
    }
}

#[test]
fn lightning_smites_scatters_and_burns() {
    let mut w = flat(11, GRASS);
    let (f, _) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.3, 0.5));
    let hit = w.spawn_agent(f, 0, Job::Forage, 120, 160).unwrap();
    let near = w.spawn_agent(f, 0, Job::Forage, 125, 160).unwrap();
    let id_near = w.agents[near].id;
    let killed = lightning(&mut w, 120.5, 160.5);
    assert_eq!(killed, 1);
    assert!(!w.agents[hit].alive);
    assert_eq!(w.agents[near].state, State::Flee);
    assert!(!w.fires.is_empty());
    run(&mut w, 0.5);
    assert!(w.agents.iter().any(|a| a.id == id_near));
}

#[test]
fn the_calibration_hook_sets_the_players_civilisation() {
    let mut w = World::new_sized(12, Dims::SMALL);
    let before: Vec<Traits> = w.factions.iter().map(|f| f.traits).collect();
    assert!(set_player_traits(&mut w, 0.9, 0.1, 0.2));
    let p = w.player().unwrap() as usize;
    let t = w.factions[p].traits;
    assert_eq!((t.friendliness, t.aggressiveness, t.violence), (0.9, 0.1, 0.2));
    assert_eq!(w.factions[p].seed_source, SeedSource::Calibration);
    for (i, f) in w.factions.iter().enumerate() {
        if i != p {
            assert_eq!(f.traits, before[i], "only the player's civilisation changes");
        }
    }
}

#[test]
fn the_bookkeeping_tick_runs_without_agents() {
    let mut w = flat(13, GRASS);
    civ(&mut w, 100, 160, 0, Job::Forage, traits(0.3, 0.5));
    slow_tick(&mut w);
    run(&mut w, 1.0);
}

#[test]
fn fighters_are_wounded_before_they_die_and_break_when_morale_fails() {
    let mut w = flat(14, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.8, 0.2));
    let (b, _) = civ(&mut w, 130, 160, 0, Job::Forage, traits(0.5, 0.3));
    w.spawn_agent(a, 0, Job::Warrior, 115, 160);
    let victim = w.spawn_agent(b, 1, Job::Warrior, 116, 160).unwrap();
    let id = w.agents[victim].id;
    declare_war(&mut w, a, b);
    let mut seen_wounded = false;
    for _ in 0..(20.0 / DT) as usize {
        w.step(DT);
        match w.agents.iter().find(|x| x.id == id) {
            Some(v) if v.wound == Wound::Wounded => {
                seen_wounded = true;
                assert!(v.hp < HEALED_HP, "wounded until healed");
            }
            Some(_) => {}
            None => break,
        }
    }
    assert!(seen_wounded, "wounded before dying or fleeing");
    // A fighter whose morale has broken runs instead of fighting.
    let mut w = flat(15, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 0, Job::Forage, traits(0.8, 0.2));
    let (b, _) = civ(&mut w, 130, 160, 0, Job::Forage, traits(0.5, 0.3));
    w.spawn_agent(a, 0, Job::Warrior, 115, 160);
    let coward = w.spawn_agent(b, 1, Job::Warrior, 117, 160).unwrap();
    w.agents[coward].morale = 0.1;
    declare_war(&mut w, a, b);
    w.step(DT);
    assert_eq!(w.agents[coward].state, State::Flee);
}

#[test]
fn a_lone_unstable_civilisation_splits_without_any_rival() {
    let mut w = flat(16, GRASS);
    let (p, _) = civ(&mut w, 100, 160, 4, Job::Forage, traits(0.5, 0.4));
    for k in 0..4 {
        let v = w.found_village(p, 130 + 16 * k, 160, 5.0, false);
        w.spawn_agent(p, v, Job::Forage, 130 + 16 * k, 160);
        w.villages[v as usize].unrest = 0.8;
    }
    assert_eq!(w.alive_factions().count(), 1, "alone on the planet");
    w.time = 50.0;
    // The first politics tick (1 s in) measures stability, then checks for rebellion. No
    // village is past REBEL_UNREST, so only the low-stability rule can split it.
    run(&mut w, 1.1);
    assert!(w.villages.iter().all(|v| v.unrest < REBEL_UNREST));
    assert!(w.factions[p as usize].metrics.stability < 0.45);
    assert_eq!(w.stats.rebellions, 1, "split with no one else in contact");
    assert_eq!(w.alive_factions().count(), 2);
}

#[test]
fn civilisations_start_in_the_first_age_and_report_metrics() {
    let mut w = World::new_sized(17, Dims::SMALL);
    run(&mut w, 3.0);
    for f in w.alive_factions() {
        assert_eq!(f.age, 1);
        let m = f.metrics;
        assert!(m.population >= START_AGENTS as u32 && m.population <= f.pop, "{} vs {}", m.population, f.pop);
        assert_eq!(m.settlements, 1);
        assert!(m.territory > 0.0 && m.territory < 0.05);
        assert!(m.stability > 0.9);
        assert!(m.food_days > 0.0);
        assert!(!m.contact);
    }
}
