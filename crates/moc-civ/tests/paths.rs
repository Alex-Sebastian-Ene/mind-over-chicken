//! Path-field cache (#217): reused BFS scratch buffers, a cache that scales with the world's
//! settlements, and adaptive queuing so a step's build budget never turns into agent `stuck`.
//!
//! None of this changes the rules: given the same seed and inputs two runs still produce the
//! same world, paths and metrics.

use moc_civ::agents::{set_goal, walk, ROUTE_FIELD};
use moc_civ::planet::{Dims, LocalField, Paths, FIELD_RADIUS};
use moc_civ::terrain::GRASS;
use moc_civ::*;

const DT: f32 = 1.0 / 30.0;

/// Deterministic, deliberately awkward passability: walls and gaps in both directions.
fn pass(x: i32, y: i32) -> bool {
    !(x.rem_euclid(23) == 0 && (y / 5).rem_euclid(3) != 0) && !(y.rem_euclid(31) == 0 && (x / 7).rem_euclid(2) == 0)
}

/// A cached/recycled field must be tile-for-tile identical to one built from scratch.
fn assert_same_field(cached: &LocalField, fresh: &LocalField, d: Dims) {
    assert_eq!((cached.tx, cached.ty), (fresh.tx, fresh.ty));
    for y in (0..d.h as i32).step_by(7) {
        for x in (0..d.w as i32).step_by(7) {
            assert_eq!(cached.at(x, y), fresh.at(x, y), "distance at ({x}, {y})");
        }
    }
    // Routes agree wherever a route exists.
    for y in (5..d.h as i32).step_by(29) {
        for x in (5..d.w as i32).step_by(37) {
            assert_eq!(cached.next_step(x, y), fresh.next_step(x, y), "next step at ({x}, {y})");
        }
    }
}

#[test]
fn recycled_fields_match_fresh_builds_across_many_evictions() {
    let d = Dims::SMALL;
    let mut paths = Paths::new();
    paths.set_settlements(0);
    let cap = paths.capacity();
    assert_eq!(cap, 128);
    // More targets than the cache holds: every build after the cache fills evicts and reuses
    // the previous field's distance buffer.
    for i in 0..(cap as i32 + 64) {
        let (x, y) = ((i * 2).rem_euclid(d.w as i32), (i * 3).rem_euclid(d.h as i32));
        paths.refill(1);
        let cached = paths.get_by(d, x, y, pass).expect("fresh field within budget");
        let fresh = LocalField::build(d, x, y, FIELD_RADIUS, pass);
        assert_same_field(cached, &fresh, d);
    }
    assert_eq!(paths.builds, cap as u32 + 64);
}

#[test]
fn scratch_buffers_are_not_reallocated_after_warm_up() {
    let d = Dims::SMALL;
    let mut paths = Paths::new();
    paths.set_settlements(0);
    let cap = paths.capacity();
    for i in 0..cap as i32 {
        let (x, y) = (i, 0);
        paths.refill(1);
        assert!(paths.get_by(d, x, y, pass).is_some());
    }
    let warm = paths.dist_capacity();
    assert!(warm >= cap * 161 * 161, "fields own their distance buffers ({warm})");
    for i in 0..64i32 {
        let (x, y) = (200 + i, 1);
        assert!(!paths.has(d, x, y), "target must force a build");
        paths.refill(1);
        assert!(paths.get_by(d, x, y, pass).is_some());
    }
    assert_eq!(paths.dist_capacity(), warm, "evicted buffers are recycled, not reallocated");
}

#[test]
fn cache_capacity_scales_with_settlements() {
    let mut land = Paths::new();
    land.set_settlements(0);
    assert_eq!(land.capacity(), 128);
    land.set_settlements(10);
    assert_eq!(land.capacity(), 208);
    land.set_settlements(1_000);
    assert_eq!(land.capacity(), 256, "clamped so memory stays bounded");

    let mut sea = Paths::sea();
    sea.set_settlements(0);
    assert_eq!(sea.capacity(), 24);
    sea.set_settlements(10);
    assert_eq!(sea.capacity(), 34);
    sea.set_settlements(1_000);
    assert_eq!(sea.capacity(), 48);
}

#[test]
fn fields_used_this_step_are_never_evicted() {
    let d = Dims::SMALL;
    let mut paths = Paths::new();
    paths.set_settlements(0);
    let cap = paths.capacity();
    let targets: Vec<(i32, i32)> = (0..cap as i32)
        .map(|i| ((i * 97 + 3).rem_euclid(d.w as i32), (i * 53 + 7).rem_euclid(d.h as i32)))
        .collect();
    for &(x, y) in &targets {
        paths.refill(1);
        assert!(paths.get_by(d, x, y, pass).is_some());
    }
    // Touch every field in one step, then ask for a new one: none may be evicted while hot.
    paths.refill(1);
    for &(x, y) in &targets {
        assert!(paths.has(d, x, y));
        assert!(paths.get_by(d, x, y, pass).is_some(), "cache hit");
    }
    assert!(paths.get_by(d, 1, 1, pass).is_none(), "deferred instead of evicting a field in use");
    assert!(paths.is_pending(d, 1, 1));
    assert!(paths.deferred > 0);
    for &(x, y) in &targets {
        assert!(paths.has(d, x, y), "hot field at ({x}, {y}) was evicted");
    }
    // Next step the coldest field can go, so the request is served.
    paths.refill(1);
    assert!(paths.get_by(d, 1, 1, pass).is_some());
    assert!(paths.has(d, 1, 1));
}

#[test]
fn requests_over_the_budget_are_queued_and_built_next_step() {
    let d = Dims::SMALL;
    let mut paths = Paths::new();
    paths.refill(2);
    assert!(paths.get_by(d, 10, 10, pass).is_some());
    assert!(paths.get_by(d, 40, 40, pass).is_some());
    assert!(paths.get_by(d, 70, 70, pass).is_none(), "third build waits");
    assert!(paths.is_pending(d, 70, 70));
    assert_eq!(paths.pending_len(), 1);
    let deferred = paths.deferred;
    assert!(deferred > 0);
    // The same request is deduped, not queued again.
    assert!(paths.get_by(d, 70, 70, pass).is_none());
    assert_eq!(paths.pending_len(), 1);
    assert_eq!(paths.deferred, deferred + 1);
    // Next step's budget serves it.
    paths.refill(2);
    assert!(paths.get_by(d, 70, 70, pass).is_some());
    assert!(!paths.is_pending(d, 70, 70));
    assert_eq!(paths.pending_len(), 0);
}

#[test]
fn an_agent_with_a_queued_field_waits_without_getting_stuck() {
    let d = Dims::SMALL;
    let mut w = World::with_terrain(1, Terrain::uniform(d, GRASS));
    let f = w.add_faction(5, SeedSource::Run, Traits::from_seed(5), NO_F, None);
    let i = w.spawn_agent(f, NO_V, Job::Forage, 100, 100).unwrap();
    let mut a = w.agents[i].clone();
    set_goal(&mut a, d, 140, 100);
    let mut paths = Paths::new();
    // The step's budget is spent: the request is queued and the walker holds still.
    paths.refill(0);
    let before = (a.x, a.y, a.stuck, a.nx);
    assert!(!walk(&mut a, &w.terrain, &mut paths, 7.0, DT, ROUTE_FIELD));
    assert_eq!((a.x, a.y, a.stuck, a.nx), before, "waiting changes nothing and costs no stuck");
    assert!(paths.is_pending(d, a.gx, a.gy));
    assert!(paths.deferred > 0);
    // Every later step rebuilds the budget; the queued field is served and the walker arrives.
    let mut arrived = false;
    for _ in 0..300 {
        paths.refill(2);
        if walk(&mut a, &w.terrain, &mut paths, 7.0, DT, ROUTE_FIELD) {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "never arrived; at ({}, {})", a.x, a.y);
    assert!(paths.builds >= 1);
}

/// Same seed and inputs still make the same world, including path-cache telemetry.
fn signature(seed: u32) -> (u64, u32, u32, u32, usize, usize) {
    let mut w = World::new_sized(seed, Dims::DEFAULT);
    w.inputs = Inputs { calm: 0.3, focus: 0.4, stress: 0.7, anger: 0.1, ..Default::default() };
    for _ in 0..(40.0 / DT) as usize {
        w.step(DT);
    }
    let stuck = w.agents.iter().filter(|a| a.stuck > 0).count();
    (w.checksum(), w.paths.builds, w.paths.deferred, w.sea.builds, w.agents.len(), stuck)
}

#[test]
fn deferred_paths_keep_runs_deterministic() {
    let a = signature(21);
    let b = signature(21);
    assert_eq!(a, b, "same seed, same world, same path-cache behaviour");
    assert!(a.1 > 0, "the run exercised the field cache");
}
