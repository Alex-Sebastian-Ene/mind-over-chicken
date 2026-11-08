//! Rivals on the home world (#212): the map, the rivals' places, the mirrored player faction,
//! what the rivals do to the player in a scripted Stone Age, difficulty, and determinism.

use moc_civ::homeworld::{self, he, tile, Continent, Difficulty, HomeWorld, MirrorGuard};
use moc_civ::politics::{Decision, Policy};
use moc_civ::terrain;
use moc_civ::world::*;

/// The game's home world (the same generator the browser runs), as the JS World hands it over.
fn home_world(seed: u32, w: usize, h: usize) -> (Vec<u8>, Vec<f32>, Vec<Continent>) {
    let mut p = moc_terrain::TerrainParams::home_world_sized(seed, w, h);
    p.set_biome("temperate");
    let m = moc_terrain::generate(&p);
    let conts = m.game_continents.iter().map(|c| Continent { cx: c.cx, cy: c.cy, rx: c.rx, ry: c.ry, village: c.village }).collect();
    (m.types, m.heights, conts)
}

fn rivals_world(seed: u32) -> HomeWorld {
    let (types, heights, conts) = home_world(seed, 640, 360);
    HomeWorld::new(seed, 640, 360, &types, &heights, &conts, 0, 0)
}

#[test]
fn tiles_map_to_civ_kinds() {
    assert_eq!(homeworld::civ_kind(tile::DEEP, 0.1), terrain::DEEP);
    assert_eq!(homeworld::civ_kind(tile::SHALLOW, 0.3), terrain::WATER);
    assert_eq!(homeworld::civ_kind(tile::SAND, 0.35), terrain::SAND);
    assert_eq!(homeworld::civ_kind(tile::GRASS, 0.5), terrain::GRASS);
    assert_eq!(homeworld::civ_kind(tile::GRASS, 0.9), terrain::HILLS);
    assert_eq!(homeworld::civ_kind(tile::FOREST, 0.5), terrain::FOREST);
    assert_eq!(homeworld::civ_kind(tile::HILL, 0.6), terrain::HILLS);
    assert_eq!(homeworld::civ_kind(tile::MOUNTAIN, 0.9), terrain::MOUNTAIN);
    assert_eq!(homeworld::civ_kind(tile::SNOW, 0.95), terrain::ICE);
    for t in [tile::FARM, tile::ROAD, tile::PAVED] {
        assert_eq!(homeworld::civ_kind(t, 0.4), terrain::GRASS, "land use is still land");
    }
}

#[test]
fn same_map_same_land_and_sea() {
    let (types, heights, conts) = home_world(5, 640, 360);
    let t = homeworld::home_terrain(640, 360, &types, &heights, &conts);
    assert_eq!((t.d.w, t.d.h), (640, 360), "same size, no padding");
    for i in 0..640 * 360 {
        assert_eq!(t.walk[i], tile::walkable(types[i]), "walkable at tile {i}");
        let sea = types[i] <= tile::SHALLOW;
        assert_eq!(sea, matches!(t.kind[i], terrain::DEEP | terrain::WATER), "sea at tile {i}");
    }
    // The campaign continents keep their order: home first, its start on its own land.
    assert_eq!(t.regions.len(), conts.len());
    let (vx, vy) = conts[0].village;
    assert!(t.walkable(vx, vy) && t.land(vx, vy) != 0);
}

#[test]
fn the_map_wraps_both_ways() {
    // A ring of land across the east–west seam and a strip across the north–south one: each
    // is one landmass, because the planet is a torus.
    let (w, h) = (640, 360);
    let mut types = vec![tile::DEEP; w * h];
    for x in (0..20).chain(w - 20..w) {
        for y in 100..110 {
            types[y * w + x] = tile::GRASS;
        }
    }
    for y in (0..15).chain(h - 15..h) {
        for x in 300..310 {
            types[y * w + x] = tile::FOREST;
        }
    }
    let t = homeworld::home_terrain(w, h, &types, &vec![0.5; w * h], &[]);
    assert_eq!(t.land(0, 105), t.land(w as i32 - 1, 105));
    assert_ne!(t.land(0, 105), 0);
    assert_eq!(t.land(305, 0), t.land(305, h as i32 - 1));
    assert_eq!(t.d.dist(1.0, 105.0, w as f32 - 2.0, 105.0), 3.0, "short way round the seam");
}

#[test]
fn rival_count_scales_with_size() {
    assert_eq!(homeworld::rival_count(480, 270), 3);
    assert_eq!(homeworld::rival_count(640, 360), 5);
    assert_eq!(homeworld::rival_count(960, 540), 8);
    assert_eq!(homeworld::rival_count(1280, 720), 12);
    assert_eq!(homeworld::rival_count(320, 180), 3);
}

#[test]
fn rivals_start_on_other_continents() {
    for seed in [3, 9, 21] {
        let hw = rivals_world(seed);
        let n = hw.rival_total();
        assert!((3..=6).contains(&n), "seed {seed}: {n} rivals on a medium world");
        let t = &hw.w.terrain;
        let home = t.land(hw.conts[0].village.0, hw.conts[0].village.1);
        let starts: Vec<(i32, i32)> = hw.rivals().map(|f| hw.w.factions[f as usize].start).collect();
        for (i, &(x, y)) in starts.iter().enumerate() {
            assert!(t.walkable(x, y), "seed {seed}: rival {i} on land");
            assert_ne!(t.land(x, y), home, "seed {seed}: rival {i} not on the player's continent");
            for c in &hw.conts {
                assert!(hw.w.d.dist(x as f32, y as f32, c.village.0 as f32, c.village.1 as f32) >= homeworld::COLONY_GAP, "seed {seed}: clear of the player's spots");
            }
            for &(ox, oy) in &starts[..i] {
                assert!(hw.w.d.dist(x as f32, y as f32, ox as f32, oy as f32) >= homeworld::RIVAL_GAP);
            }
        }
        // Waiting, then arriving one by one.
        assert_eq!(hw.rivals_alive(), 0);
    }
}

#[test]
fn rivals_arrive_staggered_and_grow() {
    let mut hw = rivals_world(9);
    let mut arrived = Vec::new();
    for _ in 0..240 {
        hw.advance(1.0);
        arrived.push(hw.rivals_alive());
    }
    assert!(arrived[10] < arrived[239], "they arrive over time: {:?}", &arrived[..20]);
    assert_eq!(arrived[239], hw.rival_total().max(arrived[239]), "all arrived");
    let pop: u32 = hw.rivals().map(|f| hw.w.factions[f as usize].pop).sum();
    assert!(pop > 40, "the rivals grow: {pop}");
    assert!(hw.lines.iter().filter(|l| l.kind == he::ARRIVE).count() >= 3);
}

#[test]
fn the_mirror_tracks_the_scene() {
    let mut hw = rivals_world(9);
    let m = hw.mirror;
    let camp = hw.conts[0].village;
    let colony = hw.conts[1].village;
    hw.player.sites = vec![camp, colony];
    hw.player.pop = 14;
    hw.player.food = 3.0;
    for _ in 0..60 {
        hw.advance(1.0);
    }
    let f = &hw.w.factions[m as usize];
    assert!(f.alive && f.is_player);
    assert_eq!(f.age, homeworld::SCENE_AGES[0].1, "the Stone Age scene's top age");
    let mine: Vec<&Village> = hw.w.villages.iter().filter(|v| v.alive && v.owner == m).collect();
    assert_eq!(mine.len(), 2, "a village at the camp and at the colony");
    assert!(mine.iter().any(|v| (v.x, v.y) == camp) && mine.iter().any(|v| (v.x, v.y) == colony));
    // Its people are only the guards standing in for the tribe's fighters.
    hw.pin();
    let agents: Vec<&Agent> = hw.w.agents.iter().filter(|a| a.alive && a.faction == m).collect();
    assert!(!agents.is_empty() && agents.len() <= 24);
    assert!(agents.iter().all(|a| a.job == Job::Guard && a.garrison));
    // Never ages, splits or starves on its own.
    for _ in 0..200 {
        hw.advance(1.0);
    }
    let f = &hw.w.factions[m as usize];
    assert_eq!(f.age, homeworld::SCENE_AGES[0].1);
    assert_eq!(f.dark_t, 0.0);
    assert!(!hw.w.factions.iter().any(|o| o.alive && o.parent == m), "no rebels split from the mirror");
    // A new stage: the mirror takes its top age; a dropped colony leaves the sim.
    hw.set_stage(13, 15, false);
    hw.player.sites = vec![camp];
    hw.advance(1.0);
    assert_eq!(hw.w.factions[m as usize].age, 15);
    assert_eq!(hw.w.factions[m as usize].name, "Chicken Republic");
    assert_eq!(hw.w.villages.iter().filter(|v| v.alive && v.owner == m).count(), 1);
}

#[test]
fn a_sacked_settlement_comes_back() {
    let mut hw = rivals_world(9);
    for _ in 0..120 {
        hw.advance(1.0);
    }
    let m = hw.mirror;
    let r = hw.rivals().find(|&f| hw.w.factions[f as usize].alive).expect("a rival");
    let v = hw.w.villages.iter().position(|v| v.alive && v.owner == m).unwrap() as u16;
    moc_civ::politics::capture(&mut hw.w, v, r);
    hw.advance(0.2);
    assert!(hw.lines.iter().any(|l| l.kind == he::SACK && l.rival == r as i32), "the sack is told");
    assert!(!hw.w.villages[v as usize].alive || hw.w.villages[v as usize].owner == m, "not left in the rival's hands");
    for _ in 0..70 {
        hw.advance(1.0);
    }
    assert!(hw.w.villages.iter().any(|v| v.alive && v.owner == m && (v.x, v.y) == hw.conts[0].village), "rebuilt");
}

/// A policy that wants war with everyone, to see what the guard lets through.
struct Warmonger;
impl Policy for Warmonger {
    fn decide(&mut self, _w: &World, civ: u8, out: &mut Vec<Decision>) {
        let other = if civ == 0 { 1 } else { 0 };
        out.extend([Decision::Raid(other), Decision::DeclareWar(other), Decision::OfferAlliance(other), Decision::Fortify, Decision::SueForPeace { with: other, vassal: true }]);
    }
}

#[test]
fn the_councils_never_decide_for_the_player() {
    let hw = rivals_world(3);
    let mut g = MirrorGuard { inner: Some(Box::new(Warmonger)), mirror: hw.mirror };
    let mut out = Vec::new();
    g.decide(&hw.w, hw.mirror, &mut out);
    assert_eq!(out, vec![Decision::Fortify, Decision::SueForPeace { with: if hw.mirror == 0 { 1 } else { 0 }, vassal: false }]);
    out.clear();
    let rival = if hw.mirror == 0 { 1 } else { 0 };
    g.decide(&hw.w, rival, &mut out);
    assert_eq!(out.len(), 5, "rivals decide as before");
}

/// The scripted Stone Age: rivals arrive, the tribe's raft reaches a friendly people and an
/// aggressive one, and a colony lands. Returns the world after `secs` simulated seconds.
fn stone_age(seed: u32, k: Difficulty, secs: usize) -> HomeWorld {
    let (types, heights, conts) = home_world(seed, 640, 360);
    let mut hw = HomeWorld::new(seed, 640, 360, &types, &heights, &conts, 0, 0);
    hw.difficulty = k;
    let camp = hw.conts[0].village;
    hw.player.sites = vec![camp];
    hw.player.pop = 12;
    hw.player.food = 6.0;
    let mut t = 0;
    while t < secs {
        if t == 200 {
            // A friendly people and a fierce one (the dice decide the rest).
            let alive: Vec<u8> = hw.rivals().filter(|&f| hw.w.factions[f as usize].alive).collect();
            if let Some(&a) = alive.first() {
                hw.w.factions[a as usize].traits = Traits { friendliness: 0.9, aggressiveness: 0.1, violence: 0.2, fertility: 0.6, cohesion: 0.6 };
            }
            if let Some(&b) = alive.get(1) {
                hw.w.factions[b as usize].traits = Traits { friendliness: 0.15, aggressiveness: 0.85, violence: 0.7, fertility: 0.6, cohesion: 0.6 };
            }
            // The raft sails past both.
            let boats: Vec<(f32, f32)> = alive.iter().take(2).map(|&f| {
                let s = hw.w.factions[f as usize].start;
                (s.0 as f32 + 3.0, s.1 as f32)
            }).collect();
            hw.player.boats = boats;
        }
        if t == 206 {
            hw.player.boats.clear();
        }
        if t == 260 && hw.conts.len() > 1 {
            hw.player.sites.push(hw.conts[1].village);
        }
        // The larder refills (mammoths).
        hw.player.food = (hw.player.food + 0.05).min(8.0);
        hw.advance(1.0);
        t += 1;
    }
    hw
}

#[test]
fn stone_age_contact_trade_and_raids() {
    let hw = stone_age(9, Difficulty::default(), 600);
    let t = hw.tally;
    assert!(t[he::CONTACT as usize] >= 2, "the raft met them: {t:?}");
    assert!(t[he::TRADE as usize] >= 1, "the friendly people brought food: {t:?}");
    assert!(t[he::RAID as usize] >= 1, "the fierce people raided: {t:?}");
    assert!(hw.lines.iter().filter(|l| l.kind == he::TRADE).all(|l| l.food >= 0.0));
    let raid = hw.lines.iter().find(|l| l.kind == he::RAID).expect("a raid line");
    assert!(raid.food < 0.0 || raid.hurt > 0, "a raid takes food or hurts people");
    assert!(!raid.text.is_empty() && !raid.say.is_empty(), "told on the ticker and by the advisor");
}

/// Contact and raid pressure on the player: tension toward the mirror from every rival that
/// met it, plus raids, sacks and the dead.
fn pressure(hw: &HomeWorld) -> f32 {
    let m = hw.mirror;
    let tension: f32 = (0..MAX_FACTIONS as u8).filter(|&f| f != m && hw.w.factions[f as usize].alive && hw.w.rel(f, m).met).map(|f| hw.w.rel(f, m).tension.max(0.0)).sum();
    let hits = (hw.tally[he::RAID as usize] + hw.tally[he::SACK as usize] + hw.tally[he::CLASH as usize] + hw.tally[he::WAR as usize]) as f32;
    let hurt: i32 = hw.lines.iter().map(|l| l.hurt).sum();
    let lost: f32 = hw.lines.iter().map(|l| (-l.food).max(0.0)).sum();
    tension + hits + hurt as f32 + lost * 0.5
}

#[test]
fn hard_presses_harder_than_easy() {
    let easy = Difficulty { aggression: -0.15, raids: 0.6, damage: 0.6, growth: 0.85 };
    let hard = Difficulty { aggression: 0.25, raids: 1.6, damage: 1.5, growth: 1.2 };
    for seed in [9, 21] {
        let e = pressure(&stone_age(seed, easy, 500));
        let h = pressure(&stone_age(seed, hard, 500));
        assert!(h > e, "seed {seed}: hard {h} > easy {e}");
    }
}

#[test]
fn the_rival_count_follows_the_difficulty() {
    let (types, heights, conts) = home_world(9, 640, 360);
    let easy = HomeWorld::new(9, 640, 360, &types, &heights, &conts, 0, -1).rival_total();
    let normal = HomeWorld::new(9, 640, 360, &types, &heights, &conts, 0, 0).rival_total();
    let hard = HomeWorld::new(9, 640, 360, &types, &heights, &conts, 0, 2).rival_total();
    assert!(easy < normal && normal <= hard, "{easy} {normal} {hard}");
}

#[test]
fn deterministic_for_a_seed() {
    let a = stone_age(21, Difficulty::default(), 320);
    let b = stone_age(21, Difficulty::default(), 320);
    assert_eq!(a.w.checksum(), b.w.checksum());
    let la: Vec<String> = a.lines.iter().map(|l| l.text.clone()).collect();
    let lb: Vec<String> = b.lines.iter().map(|l| l.text.clone()).collect();
    assert_eq!(la, lb);
    assert!(!la.is_empty());
}

#[test]
fn stages_lift_rivals_that_fall_behind() {
    let mut hw = rivals_world(9);
    for _ in 0..240 {
        hw.advance(1.0);
    }
    hw.set_stage(13, 15, false);
    let lifted = hw.catch_up();
    assert!(lifted > 0);
    for f in hw.rivals() {
        let fac = &hw.w.factions[f as usize];
        if fac.alive {
            assert!(fac.age >= 11, "{} at {}", fac.name, fac.age);
        }
    }
    // They hold the age they were lifted to for a while (people enough).
    for _ in 0..60 {
        hw.advance(1.0);
    }
    let held = hw.rivals().filter(|&f| hw.w.factions[f as usize].alive && hw.w.factions[f as usize].age >= 11).count();
    assert!(held >= lifted.min(hw.rivals_alive()).saturating_sub(1), "held {held} of {lifted}");
}

#[test]
fn space_age_launch_race() {
    let mut hw = rivals_world(9);
    for _ in 0..200 {
        hw.advance(1.0);
    }
    hw.set_stage(19, 20, false);
    hw.catch_up();
    for _ in 0..400 {
        hw.advance(1.0);
    }
    let ages: Vec<u8> = hw.rivals().map(|f| hw.w.factions[f as usize].age).collect();
    assert!(hw.tally[he::LAUNCH as usize] > 0, "the rivals race to orbit: ages {ages:?}");
    let launches: Vec<&homeworld::HomeLine> = hw.lines.iter().filter(|l| l.kind == he::LAUNCH).collect();
    assert!(launches.iter().any(|l| l.n >= 100), "someone reaches orbit");
    assert_eq!(launches.iter().filter(|l| l.text.contains("first")).count(), 1, "only one is first");
}
