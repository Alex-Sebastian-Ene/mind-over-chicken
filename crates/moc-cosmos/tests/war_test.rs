//! The interplanetary war (#166): raids, bombardments, invasions, the AI's councils, the
//! player's mind, the news and determinism.

use moc_cosmos::galaxy::Galaxy;
use moc_cosmos::planet_sim::BrainEffects;
use moc_cosmos::war::*;

fn world(pop: f32, era: u8, temper: f32) -> WorldStats {
    WorldStats { alive: true, pop, era, tier: if era >= 3 { 3 } else { 1 }, progress: 1.0, stock: 2500.0, stability: 0.8, temper, hand_played: false }
}

const NEUTRAL: Mind = Mind { calm: 0.3, focus: 0.3, stress: 0.25 };

/// A war over `worlds`, run for a few seconds so every world has its power and full defences.
fn war_of(seed: u32, worlds: &[(&str, WorldStats)]) -> SpaceWar {
    let mut w = SpaceWar::new(seed);
    for (i, (name, s)) in worlds.iter().enumerate() {
        w.sync_body(i, name, *s);
    }
    // Long enough for shields, defences and garrisons to grow to their caps, no fleets yet, and
    // no endgame tiers (endgame_test.rs has those).
    for _ in 0..60 {
        w.step(0.5, NEUTRAL);
        for p in w.powers.iter_mut() {
            p.fleet = 0.0;
            p.cooldown = 99.0;
            p.tier_xp = -1e6;
        }
    }
    w.fleets.clear();
    w
}

/// Fly every fleet home (several seconds, no new launches).
fn settle(w: &mut SpaceWar) {
    for _ in 0..40 {
        for p in w.powers.iter_mut() {
            p.cooldown = 99.0;
        }
        w.step(0.25, NEUTRAL);
    }
}

#[test]
fn space_worlds_get_powers_fleets_and_defences() {
    let mut w = war_of(1, &[("Thrum", world(12000.0, 3, 0.7)), ("Lumen", world(9000.0, 3, 0.4)), ("Pecktopia", world(200.0, 1, 0.5))]);
    assert_eq!(w.powers.len(), 3, "every inhabited world has a power");
    for (i, p) in w.powers.iter().enumerate() {
        assert_eq!(w.owner_of(i), Some(p.id));
    }
    assert!(w.powers[0].space && w.powers[1].space && !w.powers[2].space, "only Space Age worlds are space-capable");
    let (t, p) = (w.caps(0), w.caps(2));
    assert!(t.shields > 10.0 && t.defences > 10.0, "a space world has shields and orbital defences: {t:?}");
    assert!(p.shields == 0.0 && p.defences == 0.0 && p.garrison > 0.0, "a farming world has only its garrison: {p:?}");
    assert!((w.bodies[0].shields - t.shields).abs() < 1.0, "defences grow to their caps");
    // Fleets build from the economy: a richer world builds the bigger fleet.
    for _ in 0..10 {
        w.step(1.0, NEUTRAL);
    }
    assert!(w.powers[0].fleet_cap > w.powers[1].fleet_cap && w.powers[1].fleet_cap > 0.0);
    assert_eq!(w.powers[2].fleet_cap, 0.0, "no fleet before the Space Age");
}

#[test]
fn a_raid_steals_only_from_the_weaker() {
    // A strong fleet raids a world: it steals stores and knowledge.
    let mut w = war_of(2, &[("Thrum", world(12000.0, 3, 0.5)), ("Lumen", world(9000.0, 3, 0.5))]);
    w.powers[0].fleet = 200.0;
    let id = w.order(0, 1, Order::Raid).expect("raid ordered");
    assert!(id > 0);
    settle(&mut w);
    let fx = w.take_fx(1);
    assert!(fx.stock_mul < 0.95 && fx.progress_loss > 0.0, "the raid stole stores and knowledge: {fx:?}");
    assert!(w.powers[0].wins == 1 && w.powers[0].results.loot > 0.0);
    // A weak one is driven off and steals nothing.
    let mut w = war_of(2, &[("Thrum", world(12000.0, 3, 0.5)), ("Lumen", world(9000.0, 3, 0.5))]);
    w.powers[0].fleet = 6.0;
    w.order(0, 1, Order::Raid).expect("raid ordered");
    settle(&mut w);
    assert!(w.take_fx(1).is_empty(), "a weak raid steals nothing");
    assert_eq!(w.powers[0].losses, 1);
    assert!(w.news.iter().any(|n| n.text.contains("driven off")), "{:?}", w.news);
}

#[test]
fn a_bombardment_breaks_shields_first_then_people() {
    let base = [("Thrum", world(12000.0, 3, 0.5)), ("Lumen", world(9000.0, 3, 0.5))];
    let mut dmg = Vec::new();
    for fleet in [10.0, 60.0, 250.0] {
        let mut w = war_of(3, &base);
        let shields = w.bodies[1].shields;
        w.powers[0].fleet = fleet / PLAYER_COMMIT;
        w.order(0, 1, Order::Bombard).unwrap();
        let mut low = shields;
        for _ in 0..40 {
            w.powers[1].cooldown = 99.0;
            w.step(0.25, NEUTRAL);
            low = low.min(w.bodies[1].shields);
        }
        let fx = w.take_fx(1);
        assert!(low < shields * 0.9, "shields take the first hits: {shields} → {low}");
        dmg.push(1.0 - fx.pop_mul);
    }
    assert_eq!(dmg[0], 0.0, "a small fleet only dents the shields");
    assert!(dmg[2] > dmg[1] && dmg[2] > 0.1, "a bigger fleet kills more: {dmg:?}");
}

#[test]
fn an_invasion_can_take_a_planet() {
    let mut w = war_of(4, &[("Thrum", world(12000.0, 3, 0.5)), ("Lumen", world(9000.0, 3, 0.5)), ("Pecktopia", world(400.0, 1, 0.5))]);
    // Too weak: broken in orbit.
    w.powers[0].fleet = 30.0;
    w.order(0, 1, Order::Invade).unwrap();
    settle(&mut w);
    assert_eq!(w.owner_of(1), Some(1), "Lumen's orbit held");
    // Strong enough: the farming world, with no shields, falls.
    w.powers[0].fleet = 120.0;
    w.order(0, 2, Order::Invade).unwrap();
    settle(&mut w);
    assert_eq!(w.owner_of(2), Some(0), "Thrum took Pecktopia");
    assert!(!w.powers[2].alive, "Pecktopia's own civilisation has fallen");
    assert_eq!(w.take_fx(2).captured_by, 0);
    assert!(w.news.iter().any(|n| n.text.contains("invaded Pecktopia") && n.text.contains("Why:")), "{:?}", w.news);
    // The colony feeds its ruler's economy.
    let before = w.powers[0].econ.index();
    w.step(1.0, NEUTRAL);
    assert!(w.powers[0].worlds == 2 && w.powers[0].econ.index() >= before);
    // A space world falls to a fleet that beats its orbit and its garrison.
    w.powers[0].fleet = 600.0;
    w.order(0, 1, Order::Invade).unwrap();
    settle(&mut w);
    assert_eq!(w.owner_of(1), Some(0), "Lumen fell to an overwhelming fleet");
}

#[test]
fn the_hand_played_home_is_never_taken() {
    let mut home = world(40.0, 3, 0.5);
    home.hand_played = true;
    let mut w = SpaceWar::new(5);
    w.set_player_home(Some(0));
    w.sync_body(0, "Home", home);
    w.sync_body(1, "Thrum", world(12000.0, 3, 0.9));
    for _ in 0..40 {
        w.step(0.5, NEUTRAL);
    }
    let player = w.player().expect("the player has a power");
    assert_eq!(w.powers[player as usize].kind, PowerKind::Player);
    assert!(w.powers[player as usize].space, "the campaign home is space-capable in the Space era");
    w.powers[1].fleet = 2000.0;
    w.powers[1].cooldown = 0.0;
    // The host forces an invasion through the external path.
    w.launch_external(1, 0, Order::Invade, 2000.0, Motive::Expansion);
    settle(&mut w);
    assert_eq!(w.owner_of(0), Some(player), "a beachhead, never a conquest");
    assert!(w.news.iter().any(|n| n.player && n.text.contains("beachhead")), "{:?}", w.news);
}

#[test]
fn ai_worlds_attack_each_other_and_the_player() {
    let mut home = world(40.0, 3, 0.5);
    home.hand_played = true;
    let mut w = SpaceWar::new(6);
    w.set_player_home(Some(0));
    let worlds = [("Home", home), ("Thrum", world(12000.0, 3, 0.8)), ("Lumen", world(10000.0, 3, 0.45)), ("Flicker", world(8000.0, 3, 0.55)), ("Stillwater", world(11000.0, 3, 0.3))];
    let mut seen_player = false;
    for _ in 0..(240 * 4) {
        for (i, (n, s)) in worlds.iter().enumerate() {
            w.sync_body(i, n, *s);
        }
        w.step(0.25, NEUTRAL);
        seen_player |= w.fleets.iter().any(|f| f.to_planet == 0);
        for i in 0..worlds.len() {
            w.take_fx(i);
        }
    }
    let attacks: u32 = w.powers.iter().map(|p| p.wins + p.losses).sum();
    assert!(attacks >= 6, "AI worlds fight each other: {attacks} battles");
    assert!(seen_player, "and someone came for the player's world");
    assert!(w.news.iter().any(|n| n.text.contains("Why:")), "the news says why");
}

/// Launches over four minutes of a five-world galaxy at a given mind.
fn launches_with(mind: Mind) -> u32 {
    let mut w = SpaceWar::new(7);
    let worlds = [("Thrum", world(12000.0, 3, 0.5)), ("Lumen", world(10000.0, 3, 0.5)), ("Flicker", world(8000.0, 3, 0.5)), ("Stillwater", world(11000.0, 3, 0.5)), ("Roostia", world(9000.0, 3, 0.5))];
    let mut ids = 0;
    for _ in 0..(240 * 4) {
        for (i, (n, s)) in worlds.iter().enumerate() {
            w.sync_body(i, n, *s);
        }
        w.step(0.25, mind);
        for f in w.fleets.iter() {
            ids = ids.max(f.mission.unwrap().id);
        }
        for i in 0..worlds.len() {
            w.take_fx(i);
        }
    }
    ids
}

#[test]
fn stress_makes_leaders_aggressive_and_calm_makes_them_defensive() {
    let stressed = launches_with(Mind { calm: 0.0, focus: 0.3, stress: 0.9 });
    let calm = launches_with(Mind { calm: 0.9, focus: 0.3, stress: 0.0 });
    assert!(stressed > calm * 2, "stress {stressed} launches, calm {calm}");
    // Calm shields are thicker.
    let mut a = war_of(8, &[("Thrum", world(12000.0, 3, 0.5))]);
    let mut b = a.clone();
    for _ in 0..20 {
        a.step(1.0, Mind { calm: 0.9, focus: 0.3, stress: 0.0 });
        b.step(1.0, Mind { calm: 0.0, focus: 0.3, stress: 0.9 });
    }
    assert!(a.bodies[0].shields > b.bodies[0].shields * 1.1, "calm digs in: {} vs {}", a.bodies[0].shields, b.bodies[0].shields);
}

#[test]
fn calm_brings_truces() {
    let mut w = war_of(9, &[("Thrum", world(12000.0, 3, 0.5)), ("Lumen", world(10000.0, 3, 0.5))]);
    w.rel_mut(0, 1).grudge = 1.0;
    w.rel_mut(1, 0).grudge = 1.0;
    let mut truce = false;
    for _ in 0..200 {
        w.step(1.0, Mind { calm: 1.0, focus: 0.3, stress: 0.0 });
        truce |= w.rel(0, 1).truce > 0.0;
    }
    assert!(truce, "a calm god talks them into a truce");
    assert!(w.news.iter().any(|n| n.text.contains("truce")));
}

fn run(seed: u32) -> (Vec<String>, Vec<f32>, Vec<Option<u16>>) {
    let mut w = SpaceWar::new(seed);
    let worlds = [("Thrum", world(12000.0, 3, 0.8)), ("Lumen", world(10000.0, 3, 0.4)), ("Flicker", world(3000.0, 2, 0.5)), ("Stillwater", world(11000.0, 3, 0.3))];
    for k in 0..(200 * 4) {
        for (i, (n, s)) in worlds.iter().enumerate() {
            w.sync_body(i, n, *s);
        }
        let stress = ((k / 40) % 3) as f32 * 0.4;
        w.step(0.25, Mind { calm: 0.2, focus: 0.3, stress });
        for i in 0..worlds.len() {
            w.take_fx(i);
        }
    }
    (
        w.news.iter().map(|n| n.text.clone()).collect(),
        w.powers.iter().map(|p| p.fleet).collect(),
        (0..worlds.len()).map(|i| w.owner_of(i)).collect(),
    )
}

#[test]
fn deterministic_for_a_seed_and_inputs() {
    let (a, b) = (run(11), run(11));
    assert_eq!(a, b);
    assert!(!a.0.is_empty());
    assert_ne!(run(11).0, run(12).0, "another seed, another war");
}

#[test]
fn the_rust_galaxy_runs_the_war() {
    let mut g = Galaxy::new(4242);
    let fx = BrainEffects { calm: 0.2, focus: 0.9, stress: 0.6, blink_rate: 0.0 };
    // Mind worlds form after 15 s; focus brings them to the Space Age in about a minute.
    let mut logged = false;
    for _ in 0..(360 * 5) {
        g.step(0.2, &fx, 0);
        logged |= g.log.iter().any(|(_, t)| t.contains("Why:"));
    }
    let space = g.planets.iter().filter(|p| p.alive && p.state.era_index == 3).count();
    assert!(space >= 2, "worlds reached space: {space}");
    let battles: u32 = g.war.powers.iter().map(|p| p.wins + p.losses).sum();
    assert!(battles > 0, "space worlds went to war");
    assert!(logged, "the galaxy log carries the war news");
}

#[test]
fn a_galaxy_step_stays_under_a_millisecond() {
    let mut w = SpaceWar::new(13);
    let worlds: Vec<(String, WorldStats)> = (0..10).map(|i| (format!("W{i}"), world(3000.0 + 1000.0 * i as f32, if i % 4 == 0 { 2 } else { 3 }, 0.1 * i as f32))).collect();
    // Warm up into a busy war.
    let step = |w: &mut SpaceWar| {
        for (i, (n, s)) in worlds.iter().enumerate() {
            w.sync_body(i, n, *s);
        }
        w.step(1.0 / 60.0, Mind { calm: 0.0, focus: 0.3, stress: 1.0 });
        for i in 0..worlds.len() {
            w.take_fx(i);
        }
    };
    for _ in 0..3600 {
        step(&mut w);
    }
    let t0 = std::time::Instant::now();
    let n = 3600;
    for _ in 0..n {
        step(&mut w);
    }
    let per = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
    eprintln!("war step: {per:.4} ms ({} fleets, {} powers)", w.fleets.len(), w.powers.len());
    assert!(per < 1.0, "{per} ms per step");
}

#[test]
fn civilisations_on_a_shared_planet_fight_in_space() {
    let mut w = SpaceWar::new(15);
    let mut home = world(40.0, 1, 0.5);
    home.hand_played = true;
    let worlds = [("Home", home), ("Thrum", world(12000.0, 3, 0.9)), ("Pecktopia", world(400.0, 1, 0.5))];
    for (i, (n, s)) in worlds.iter().enumerate() {
        w.sync_body(i, n, *s);
    }
    w.set_shared(0, true);
    let econ = Economy { production: 8.0, knowledge: 9.0, resources: 6.0 };
    let defence = Defence { shields: 20.0, defences: 12.0, garrison: 10.0 };
    let a = w.upsert_external(1001, "Ashkai", [232, 70, 62], 0, econ, 120.0, defence, 0, 0.6, true).unwrap();
    assert_eq!(w.upsert_external(1001, "Ashkai", [232, 70, 62], 0, econ, 120.0, defence, 0, 0.6, true), Some(a), "same key, same power");
    for _ in 0..8 {
        w.step(0.5, NEUTRAL);
    }
    assert_eq!(w.player(), None, "the civ sandbox's planet has no player power");
    let mut t = Vec::new();
    w.targets_for(a, &mut t);
    let bodies: Vec<u32> = t.chunks(moc_cosmos::host::TARGET_OUT).map(|c| c[0] as u32).collect();
    assert_eq!(bodies, vec![1, 2], "it may strike the other planets, never its own");
    // It takes the farming world as a colony; the colony feeds it.
    w.launch_external(a, 2, Order::Invade, 120.0, Motive::Expansion).unwrap();
    for _ in 0..40 {
        w.step(0.25, NEUTRAL);
        for i in 0..3 {
            w.sync_body(i, worlds[i].0, worlds[i].1);
        }
        w.upsert_external(1001, "Ashkai", [232, 70, 62], 0, econ, 0.0, defence, 0, 0.6, true);
    }
    assert_eq!(w.owner_of(2), Some(a));
    let r = w.take_results_packed(a);
    assert_eq!(r[4], 1.0, "one colony gained: {r:?}");
    assert!(r[6] > 0.0, "and its economy: {r:?}");
    // The galaxy strikes back at the civilisation on its shared planet.
    let mut hit = false;
    for _ in 0..(240 * 4) {
        for i in 0..3 {
            w.sync_body(i, worlds[i].0, worlds[i].1);
        }
        w.upsert_external(1001, "Ashkai", [232, 70, 62], 0, econ, 60.0, defence, 0, 0.6, true);
        w.step(0.25, Mind { calm: 0.0, focus: 0.3, stress: 0.9 });
        hit |= w.fleets.iter().any(|f| f.to_planet == 0 && f.mission.unwrap().target_power == Some(a));
    }
    assert!(hit, "Thrum came for Ashkai");
    let r = w.take_results_packed(a);
    assert!(r[2] > 0.0 || r[3] > 0.0 || r[0] < 0.0, "and it hurt: {r:?}");
    assert!(w.news.iter().any(|n| n.civ && n.text.contains("Ashkai")), "the news knows the civilisation");
    // Out of space: retired, and its colony goes free.
    w.upsert_external(1001, "", [0, 0, 0], 0, econ, 0.0, defence, 0, 0.6, false);
    assert!(!w.powers[a as usize].alive);
    w.step(1.0, NEUTRAL);
    assert_ne!(w.owner_of(2), Some(a));
}
