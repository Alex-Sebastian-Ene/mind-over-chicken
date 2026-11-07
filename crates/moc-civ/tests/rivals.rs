//! More opponents (#101, the civilisation side): 8–12 civilisations, several to a continent;
//! the player's founded one by one (#98), rivals arriving on their own at staggered times with
//! their seeded traits; start sites from whatever planet `Terrain::generate` makes.

use moc_civ::planet::{Cam, Dims};
use moc_civ::setup::{MIN_SITE_GAP, RIVAL_FIRST, SEED_SECONDS};
use moc_civ::*;

fn play(w: &mut World, secs: f32) {
    for _ in 0..(secs * 10.0) as usize {
        w.advance(0.1);
    }
}

#[test]
fn rivals_arrive_on_their_own_at_staggered_times() {
    let mut w = World::new_staged_with(31, Dims::DEFAULT, 4, 6);
    assert_eq!(w.setup.slots.len(), 4);
    assert_eq!(w.setup.rivals.len(), 6);
    assert_eq!(w.alive_factions().count(), 0);
    w.set_speed(4.0);
    play(&mut w, 70.0); // 280 s of world time
    let rivals: Vec<&Faction> = w.setup.rivals.iter().map(|r| &w.factions[r.0 as usize]).collect();
    assert!(rivals.iter().all(|f| f.alive), "all six arrived by now");
    let mut born: Vec<f32> = rivals.iter().map(|f| f.born).collect();
    born.sort_by(|a, b| a.total_cmp(b));
    assert!(born[0] >= RIVAL_FIRST - 0.2, "the first after {RIVAL_FIRST} s ({})", born[0]);
    assert!(born.windows(2).all(|p| p[1] - p[0] >= 10.0), "staggered: {born:?}");
    for (f, &(id, _)) in rivals.iter().zip(&w.setup.rivals) {
        assert!(!f.is_player, "rivals aren't the player's");
        assert_eq!(f.traits, Traits::from_seed(f.seed).clamped(), "seeded traits");
        assert_eq!(f.seed_source, SeedSource::Run);
        assert!(w.agents.iter().any(|a| a.faction == id) || f.villages > 0 || f.pop == 0);
    }
    // The player's slots are still waiting for the player.
    assert!(w.setup.slots.iter().all(|&f| w.factions[f as usize].pending));
    // Founding one of them makes it the player's own, rivals or not.
    let mine = w.setup.slots[0];
    w.setup_select(mine);
    if w.setup_enter() == moc_civ::setup::Phase::Seed {
        for _ in 0..((SEED_SECONDS + 0.2) * 10.0) as usize {
            w.advance(0.1);
        }
        assert!(w.factions[mine as usize].is_player);
        assert_eq!(w.alive_factions().filter(|f| f.is_player).count(), 1);
    }
}

#[test]
fn start_sites_spread_over_the_land_several_to_a_continent() {
    let w = World::new_staged_with(32, Dims::DEFAULT, 4, 8);
    let sites = w.start_sites(12);
    assert_eq!(sites.len(), 12, "twelve peoples fit on the default planet");
    for (i, &a) in sites.iter().enumerate() {
        assert!(w.terrain.walkable(a.0, a.1));
        for &b in &sites[i + 1..] {
            assert!(w.d.dist(a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32) >= MIN_SITE_GAP - 0.01, "{a:?} and {b:?} too close");
        }
    }
    let mut per_land = std::collections::BTreeMap::new();
    for &s in &sites {
        *per_land.entry(w.terrain.land(s.0, s.1)).or_insert(0) += 1;
    }
    assert!(per_land.values().any(|&n| n >= 3), "several peoples share a continent: {per_land:?}");
    // Every waiting civilisation has its site; the player's homelands come first.
    let homes: Vec<(i32, i32)> = w.setup.slots.iter().map(|&f| w.factions[f as usize].start).collect();
    assert_eq!(&homes[..], &sites[..4]);
}

#[test]
fn a_bigger_planet_takes_more_peoples_and_more_people() {
    let d = Dims::new(2560, 1280);
    let w = World::new_staged_with(33, d, 4, 8);
    assert_eq!(w.d, d);
    assert_eq!(w.setup.slots.len() + w.setup.rivals.len(), 12);
    assert!(w.cap_agents > MAX_AGENTS && w.cap_villages > MAX_VILLAGES, "{} agents, {} villages", w.cap_agents, w.cap_villages);
    // The camera reaches every part of it: clamped at the poles, wrapping east–west.
    let mut c = Cam::centred_on(d, 10.0, 0.0);
    assert_eq!(c.y, 0.0);
    c.pan(0.0, 1e7);
    assert_eq!(c.y, (d.h - moc_planet::VH) as f32);
    c.pan(d.w as f32 - 5.0, 0.0);
    assert!(c.x >= 0.0 && c.x < d.w as f32);
    let (px, _) = c.to_planet(moc_planet::VW as f32 - 1.0, 0.0);
    assert!(px >= 0.0 && px < d.w as f32);
}
