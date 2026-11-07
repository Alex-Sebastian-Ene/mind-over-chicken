//! Staggered starts (#98): civilisations wait until the player chooses, places and seeds them
//! from the mind; then they are founded, one at a time, while the others already live.

use moc_civ::planet::Dims;
use moc_civ::setup::{unpack_traits, Phase, SEED_SECONDS, SLOTS};
use moc_civ::terrain::{DEEP, WATER};
use moc_civ::*;

const FRAME: f32 = 1.0 / 60.0;

fn staged() -> World {
    World::new_staged(21, Dims::SMALL)
}

/// Run `secs` of real time at the world's speed.
fn play(w: &mut World, secs: f32) {
    for _ in 0..(secs / FRAME).round() as usize {
        w.advance(FRAME);
    }
}

/// Choose `f`, keep its homeland, and seed it for the full window with these inputs.
fn seed(w: &mut World, f: u8, inputs: Inputs, blinks: u32) {
    assert!(w.setup_select(f));
    assert_eq!(w.setup.phase, Phase::Place);
    assert_eq!(w.setup_enter(), Phase::Seed, "its homeland is a fine place to begin: {}", w.setup.reason);
    w.inputs = inputs;
    for _ in 0..blinks {
        w.setup_blink();
    }
    play(w, SEED_SECONDS + 0.1);
}

#[test]
fn a_staged_world_waits_for_the_player() {
    let mut w = staged();
    assert_eq!(w.alive_factions().count(), 0, "nobody lives here yet");
    assert_eq!(w.factions.iter().filter(|f| f.pending).count(), SLOTS);
    assert_eq!(w.setup.phase, Phase::Choose);
    play(&mut w, 30.0);
    assert!(w.agents.is_empty() && w.villages.is_empty());
    assert!(w.events.is_empty(), "no contact, no ages, nothing: {:?}", w.events.first());
    for f in w.factions.iter().filter(|f| f.pending) {
        assert_eq!(f.age, 1);
        assert_eq!(f.metrics, Metrics::default());
    }
    assert!(w.setup_text().starts_with("Choose your first civilisation"));
}

#[test]
fn civilisations_are_founded_one_at_a_time_in_the_players_order() {
    let mut w = staged();
    let [s0, _, s2, _] = [w.setup.slots[0], w.setup.slots[1], w.setup.slots[2], w.setup.slots[3]];
    // The third slot first, calm; the world runs on without it meanwhile.
    seed(&mut w, s2, Inputs::with_stress(0.95, 0.4, 0.0), 0);
    assert!(w.factions[s2 as usize].alive && !w.factions[s2 as usize].pending);
    assert!(w.factions[s2 as usize].is_player, "the first founded is the player's own");
    assert_eq!(w.alive_factions().count(), 1);
    assert_eq!(w.agents.len(), START_AGENTS);
    let t1 = w.factions[s2 as usize].born;
    assert!((t1 - SEED_SECONDS).abs() < 0.3, "founded after the seeding window ({t1})");
    play(&mut w, 20.0);
    assert!(w.agents.iter().all(|a| a.faction == s2), "only the founded people live");
    // Then the first slot, seeded in a temper.
    seed(&mut w, s0, Inputs { calm: 0.0, focus: 0.1, stress: 1.0, anger: 0.9, joy: 0.0 }, 6);
    let t2 = w.factions[s0 as usize].born;
    assert!(t2 > t1 + 20.0, "staggered: {t1} then {t2}");
    assert!(!w.factions[s0 as usize].is_player);
    assert_eq!(w.alive_factions().count(), 2);
    let founded: Vec<Event> = w.events.iter().filter(|e| e.kind == ev::CIV_FOUNDED).copied().collect();
    assert_eq!(founded.len(), 2);
    assert_eq!((founded[0].a, founded[0].n), (s2 as i32, 1), "founding order in the log");
    assert_eq!((founded[1].a, founded[1].n), (s0 as i32, 2));
    assert!(founded[1].t_ms > founded[0].t_ms);
    assert_eq!(w.setup.founded.iter().map(|x| x.0).collect::<Vec<_>>(), vec![s2, s0]);
    assert_eq!(w.setup.phase, Phase::Choose, "two more to go");
}

#[test]
fn the_seeding_window_sets_the_traits() {
    let mut w = staged();
    let (a, b) = (w.setup.slots[0], w.setup.slots[1]);
    seed(&mut w, a, Inputs { calm: 0.95, focus: 0.8, stress: 0.0, anger: 0.0, joy: 0.7 }, 0);
    seed(&mut w, b, Inputs { calm: 0.0, focus: 0.0, stress: 1.0, anger: 0.9, joy: 0.0 }, 8);
    let (ta, tb) = (w.factions[a as usize].traits, w.factions[b as usize].traits);
    assert!(ta.friendliness > 0.6 && tb.friendliness < 0.3, "calm and joy make friends: {ta:?} / {tb:?}");
    assert!(tb.aggressiveness > 0.6 && ta.aggressiveness < 0.35, "stress and anger make them fierce");
    assert!(tb.violence > ta.violence + 0.3, "anger and blinks make them violent");
    assert!(ta.fertility > tb.fertility + 0.3, "focus makes them fertile");
    assert!(ta.cohesion > tb.cohesion, "blinks and stress pull them apart");
    // The log carries them.
    let e = w.events.iter().find(|e| e.kind == ev::CIV_FOUNDED && e.a == a as i32).unwrap();
    let logged = unpack_traits(e.extra);
    assert!((logged.friendliness - ta.friendliness).abs() < 0.02);
    assert!((logged.cohesion - ta.cohesion).abs() < 0.02);
    // Seeded from the same mind, two civilisations still differ a little (seeded variation).
    let mut w = staged();
    let (c, d) = (w.setup.slots[2], w.setup.slots[3]);
    let calm = Inputs::with_stress(0.5, 0.3, 0.0);
    seed(&mut w, c, calm, 0);
    seed(&mut w, d, calm, 0);
    assert_ne!(w.factions[c as usize].traits, w.factions[d as usize].traits);
}

#[test]
fn bad_spots_are_refused_with_a_reason() {
    let mut w = staged();
    let f = w.setup.slots[0];
    w.setup_select(f);
    let sea = (0..w.d.n()).find(|&i| matches!(w.terrain.kind[i], DEEP | WATER) && i / w.d.w > 40 && i / w.d.w < w.d.h - 40).unwrap();
    let (sx, sy) = ((sea % w.d.w) as f32, (sea / w.d.w) as f32);
    assert!(!w.setup_hover(sx, sy));
    assert!(w.setup.reason.contains("sea"), "{}", w.setup.reason);
    assert_eq!(w.setup_click(sx, sy), Phase::Place, "a refused click doesn't start seeding");
    // Next to a founded people: too close. (Enter begins it in its homeland, wherever it aimed.)
    let home = w.factions[f as usize].start;
    assert_eq!(w.setup_enter(), Phase::Seed);
    play(&mut w, SEED_SECONDS + 0.1);
    let g = w.setup.slots[1];
    w.setup_select(g);
    let (hx, hy) = (home.0 as f32 + 0.5, home.1 as f32 + 12.0);
    let near = (-15..=15).flat_map(|dy| (-15..=15).map(move |dx| (hx + dx as f32, hy + dy as f32))).find(|&(x, y)| {
        let k = w.terrain.kind_at(x as i32, y as i32);
        w.terrain.walkable(x as i32, y as i32) && w.owner_at(x, y) == NO_F && k != DEEP
    });
    if let Some((x, y)) = near {
        assert!(!w.setup_hover(x, y), "({x}, {y}) is {} tiles from the {}", w.d.dist(x, y, home.0 as f32, home.1 as f32), w.factions[f as usize].name);
        assert!(w.setup.reason.contains("too close") || w.setup.reason.contains("belongs"), "{}", w.setup.reason);
    }
    // Its own homeland is fine.
    let s = w.factions[g as usize].start;
    assert!(w.setup_hover(s.0 as f32, s.1 as f32), "{}", w.setup.reason);
}

#[test]
fn nobody_meets_a_civilisation_before_it_is_founded() {
    let mut w = staged();
    let (a, b) = (w.setup.slots[0], w.setup.slots[1]);
    seed(&mut w, a, Inputs::with_stress(0.2, 0.2, 0.0), 0);
    // Walk one of a's people right onto b's waiting homeland.
    let hb = w.factions[b as usize].start;
    w.agents[0].x = hb.0 as f32 + 0.5;
    w.agents[0].y = hb.1 as f32 + 0.5;
    play(&mut w, 40.0);
    assert!(!w.rel(a, b).met && !w.rel(b, a).met);
    assert_eq!(w.stats.contacts, 0);
    assert!(!w.factions[b as usize].alive);
    assert_eq!(w.factions[b as usize].metrics, Metrics::default(), "no metrics");
    assert_eq!(w.factions[b as usize].age, 1, "no ages");
    // A rebellion never takes a waiting civilisation's slot.
    let c = w.add_faction(99, SeedSource::Rebellion, Traits::default(), a, None);
    assert!(!w.setup.slots.contains(&c));
}

#[test]
fn seeding_runs_on_real_time_whatever_the_speed() {
    let mut w = staged();
    w.set_speed(16.0);
    let f = w.setup.slots[0];
    w.setup_select(f);
    w.setup_enter();
    play(&mut w, SEED_SECONDS * 0.5);
    assert_eq!(w.setup.phase, Phase::Seed, "half the window: still seeding");
    play(&mut w, SEED_SECONDS * 0.5 + 0.1);
    assert!(w.factions[f as usize].alive);
    assert!(w.time > 16.0 * SEED_SECONDS * 0.9, "while the world ran at ×16 ({})", w.time);
}
