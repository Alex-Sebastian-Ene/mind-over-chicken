//! The endgame (#166): tiers after the last age, colonies feeding strength, coalitions against
//! the leader, galactic dominance.

use moc_cosmos::endgame::{self, TIERS};
use moc_cosmos::war::*;

fn world(pop: f32, era: u8, temper: f32) -> WorldStats {
    WorldStats { alive: true, pop, era, tier: if era >= 3 { 3 } else { 1 }, progress: 1.0, stock: 2500.0, stability: 0.8, temper, hand_played: false }
}

const NEUTRAL: Mind = Mind { calm: 0.3, focus: 0.3, stress: 0.25 };

/// Step a war for `secs` with no launches (councils, tiers and defences only).
fn quiet(w: &mut SpaceWar, secs: usize) {
    for _ in 0..secs * 2 {
        for p in w.powers.iter_mut() {
            p.cooldown = 99.0;
        }
        w.step(0.5, NEUTRAL);
    }
}

fn war(worlds: &[(&str, WorldStats)]) -> SpaceWar {
    let mut w = SpaceWar::new(21);
    for (i, (n, s)) in worlds.iter().enumerate() {
        w.sync_body(i, n, *s);
    }
    w
}

#[test]
fn the_tier_table_climbs() {
    for k in 1..TIERS.len() {
        let (a, b) = (&TIERS[k - 1], &TIERS[k]);
        assert!(b.xp > a.xp && b.colonies >= a.colonies);
        assert!(b.fx.fleet > a.fx.fleet && b.fx.shields > a.fx.shields && b.fx.econ > a.fx.econ, "{} must be stronger than {}", b.name, a.name);
    }
    assert_eq!(endgame::earned_tier(0, 1e9, 0), 1, "one step at a time");
    assert_eq!(endgame::earned_tier(1, 1e9, 0), 1, "interplanetary colonies need a colony");
    assert_eq!(endgame::earned_tier(1, 1e9, 1), 2);
}

#[test]
fn endgame_tiers_raise_strength() {
    let mut w = war(&[("Thrum", world(12000.0, 3, 0.5)), ("Pecktopia", world(400.0, 1, 0.5))]);
    quiet(&mut w, 3);
    let (cap0, shields0, econ0) = (w.powers[0].fleet_cap, w.caps(0).shields, w.powers[0].econ.index());
    assert_eq!(w.powers[0].tier, 0);
    // Orbital industry comes with time in space.
    quiet(&mut w, 60);
    assert_eq!(w.powers[0].tier, 1, "xp {}", w.powers[0].tier_xp);
    assert!(w.news.iter().any(|n| n.text.contains("orbital industry")), "{:?}", w.news);
    // Without a colony it stops there, however long it waits.
    quiet(&mut w, 200);
    assert_eq!(w.powers[0].tier, 1);
    let (cap1, shields1, econ1) = (w.powers[0].fleet_cap, w.caps(0).shields, w.powers[0].econ.index());
    assert!(cap1 > cap0 * 1.3 && shields1 > shields0 * 1.3 && econ1 > econ0 * 1.1, "tier 1 is stronger: fleet {cap0}→{cap1}, shields {shields0}→{shields1}, economy {econ0}→{econ1}");
    // Take a colony: the colony's economy adds in, and the next tiers open.
    w.powers[0].fleet = 200.0;
    w.order(0, 1, Order::Invade).unwrap();
    quiet(&mut w, 10);
    assert_eq!(w.owner_of(1), Some(0));
    assert!(w.powers[0].econ.index() > econ1, "the colony feeds its ruler");
    quiet(&mut w, 5);
    assert_eq!(w.powers[0].tier, 2, "interplanetary colonies");
    assert!(w.powers[0].fleet_cap > cap1 && w.caps(0).shields > shields1);
    // A stellar empire needs a second colony, however much experience it has.
    quiet(&mut w, 120);
    assert_eq!(w.powers[0].tier, 2);
    // Without a player world in space the HUD tells of the galaxy instead.
    let rows = w.hud_rows();
    assert!(rows.contains("Leader\tThrum") && rows.contains("interplanetary colonies"), "{rows}");
}

/// A galaxy where Thrum rules three worlds and the rest one each.
fn lopsided() -> SpaceWar {
    let mut w = war(&[
        ("Thrum", world(12000.0, 3, 0.6)),
        ("Lumen", world(5000.0, 3, 0.5)),
        ("Flicker", world(4000.0, 3, 0.5)),
        ("Stillwater", world(4500.0, 3, 0.5)),
        ("Yolkos", world(8000.0, 2, 0.5)),
        ("Bok Prime", world(8000.0, 2, 0.5)),
    ]);
    quiet(&mut w, 3);
    for target in [4, 5] {
        w.powers[0].fleet = 400.0;
        w.order(0, target, Order::Invade).unwrap();
        quiet(&mut w, 10);
        assert_eq!(w.owner_of(target), Some(0));
    }
    w.powers[0].fleet = w.powers[0].fleet_cap;
    w
}

#[test]
fn coalitions_form_against_the_leader() {
    let mut w = lopsided();
    quiet(&mut w, 2);
    assert_eq!(w.leader, Some(0));
    assert_eq!(w.coalition_against, Some(0), "dominance {}", w.powers[0].dominance);
    for p in 1..4u16 {
        assert!(w.powers[p as usize].coalition, "{} joined", w.powers[p as usize].name);
        assert!(w.friendly(1, p), "coalition members are at peace");
    }
    assert!(!w.powers[0].coalition && !w.friendly(0, 1));
    assert!(w.news.iter().any(|n| n.text.starts_with("A coalition forms against Thrum")), "{:?}", w.news);
    // Members build faster and dig in.
    let cap_before = w.caps(1).shields;
    // The coalition's fleets go for the leader.
    let mut at_leader = 0;
    let mut elsewhere = 0;
    let mut seen = std::collections::BTreeSet::new();
    for p in w.powers.iter_mut() {
        p.cooldown = 0.0;
    }
    for _ in 0..(120 * 4) {
        w.step(0.25, NEUTRAL);
        for f in w.fleets.iter() {
            let m = f.mission.unwrap();
            if m.owner != 0 && seen.insert(m.id) {
                let ruler = w.owner_of(f.to_planet);
                if ruler == Some(0) {
                    at_leader += 1;
                } else {
                    elsewhere += 1;
                }
            }
        }
        if w.coalition_against.is_none() {
            break;
        }
    }
    assert!(at_leader > 0 && at_leader > elsewhere * 3, "coalition fleets at the leader {at_leader}, elsewhere {elsewhere}");
    let _ = cap_before;
}

#[test]
fn the_coalition_breaks_up_when_the_leader_falls_back() {
    let mut w = lopsided();
    quiet(&mut w, 2);
    assert_eq!(w.coalition_against, Some(0));
    // Thrum loses its colonies and its fleet.
    for i in [4, 5] {
        w.sync_body(i, if i == 4 { "Yolkos" } else { "Bok Prime" }, WorldStats { alive: true, pop: 0.0, ..world(0.0, 2, 0.5) });
    }
    w.powers[0].fleet = 0.0;
    w.sync_body(0, "Thrum", world(3000.0, 3, 0.6));
    for _ in 0..(endgame::COALITION_MIN as usize + 30) {
        quiet(&mut w, 1);
        w.powers[0].fleet = 0.0;
        if w.coalition_against.is_none() {
            break;
        }
    }
    assert_eq!(w.coalition_against, None, "dominance {}", w.powers[0].dominance);
    assert!(w.powers.iter().all(|p| !p.coalition));
    assert!(w.news.iter().any(|n| n.text.contains("breaks up")));
}

#[test]
fn galactic_dominance_is_the_goal() {
    let mut w = war(&[("Thrum", world(12000.0, 3, 0.6)), ("Lumen", world(800.0, 2, 0.5)), ("Flicker", world(600.0, 2, 0.5)), ("Pecktopia", world(500.0, 1, 0.5))]);
    quiet(&mut w, 3);
    for target in [1, 2, 3] {
        w.powers[0].fleet = 300.0;
        w.order(0, target, Order::Invade).unwrap();
        quiet(&mut w, 10);
    }
    assert!(w.powers[0].dominance > endgame::DOMINANCE_AT, "{}", w.powers[0].dominance);
    quiet(&mut w, endgame::DOMINANCE_HOLD as usize + 2);
    assert_eq!(w.dominant, Some(0));
    assert!(w.news.iter().any(|n| n.text.starts_with("Galactic dominance: Thrum")));
}

#[test]
fn the_player_sees_its_tier_goal_and_coalition() {
    let mut home = world(40.0, 3, 0.5);
    home.hand_played = true;
    let mut w = SpaceWar::new(31);
    w.set_player_home(Some(0));
    let worlds = [("Home", home), ("Thrum", world(3000.0, 3, 0.5)), ("Lumen", world(3000.0, 3, 0.5)), ("Yolkos", world(9000.0, 2, 0.5)), ("Bok Prime", world(9000.0, 2, 0.5))];
    for (i, (n, s)) in worlds.iter().enumerate() {
        w.sync_body(i, n, *s);
    }
    quiet(&mut w, 3);
    let rows = w.hud_rows();
    assert!(rows.contains("Fleet\t") && rows.contains("Tier\tSpacefaring") && rows.contains("Goal\tdominance"), "{rows}");
    // The player conquers two worlds and becomes the one everyone fears.
    for t in [3, 4] {
        w.powers[0].fleet = 500.0;
        w.order(0, t, Order::Invade).unwrap();
        quiet(&mut w, 10);
    }
    w.powers[0].fleet = w.powers[0].fleet_cap;
    quiet(&mut w, 2);
    assert_eq!(w.coalition_against, Some(0), "dominance {}", w.powers[0].dominance);
    assert!(w.hud_rows().contains("Coalition\tagainst you!"), "{}", w.hud_rows());
    assert!(w.news.iter().any(|n| n.player && n.text.starts_with("A coalition forms against your world")), "{:?}", w.news);
}

#[test]
fn deterministic_with_the_endgame() {
    let run = || {
        let mut w = SpaceWar::new(77);
        let worlds = [("Thrum", world(12000.0, 3, 0.8)), ("Lumen", world(10000.0, 3, 0.4)), ("Flicker", world(3000.0, 2, 0.5)), ("Stillwater", world(11000.0, 3, 0.3)), ("Yolkos", world(2000.0, 1, 0.5))];
        for _ in 0..(400 * 4) {
            for (i, (n, s)) in worlds.iter().enumerate() {
                w.sync_body(i, n, *s);
            }
            w.step(0.25, Mind { calm: 0.1, focus: 0.5, stress: 0.7 });
            for i in 0..worlds.len() {
                w.take_fx(i);
            }
        }
        (w.news.iter().map(|n| n.text.clone()).collect::<Vec<_>>(), w.powers.iter().map(|p| (p.tier, p.tier_xp.to_bits(), p.coalition)).collect::<Vec<_>>(), w.coalition_against)
    };
    let a = run();
    assert_eq!(a, run());
    assert!(a.1.iter().any(|t| t.0 >= 1), "someone climbed a tier in 400 s: {:?}", a.1);
}
