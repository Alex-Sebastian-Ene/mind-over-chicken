//! Forests (Alex: "slow down how quickly trees burn … civs cut down trees in proximity … fires
//! stop burning and trees regrow when trees are nearby").
use moc_civ::planet::Dims;
use moc_civ::terrain::{FOREST, GRASS};
use moc_civ::*;

fn forest_world(seed: u32) -> World {
    let d = Dims::DEFAULT;
    let regions = Terrain::uniform(d, FOREST).regions;
    let mut w = World::with_terrain(seed, Terrain::from_kinds(d, vec![FOREST; d.n()], regions));
    w.agents.clear(); // just the land: no one to fight the fire or chop the trees
    w
}

fn count(w: &World, k: u8) -> usize {
    w.terrain.kind.iter().filter(|&&x| x == k).count()
}

#[test]
fn a_forest_fire_burns_out_instead_of_sweeping_the_forest() {
    for seed in 1..6 {
        let mut w = forest_world(seed);
        let (cx, cy) = (w.d.w / 2, w.d.h / 2);
        w.ignite_tile(cy * w.d.w + cx);
        for _ in 0..(120 * 10) {
            w.step(0.1);
        }
        assert!(w.fires.is_empty(), "seed {seed}: the fire should have burnt out ({} burning)", w.fires.len());
        let cleared = count(&w, GRASS);
        assert!(cleared < 400, "seed {seed}: one spark cleared {cleared} tiles of forest");
    }
}

#[test]
fn felled_trees_grow_back_only_beside_trees() {
    let mut w = forest_world(7);
    let d = w.d;
    // A single felled tree inside the forest regrows.
    let inside = (d.h / 2) * d.w + d.w / 2;
    w.fell_tree(inside);
    assert_eq!(w.terrain.kind[inside], GRASS);
    // A felled tree in the middle of open land stays cleared.
    let lone = (d.h / 2) * d.w + d.w / 4;
    for oy in -2i32..=2 {
        for ox in -2i32..=2 {
            let j = d.idx(lone as i32 % d.w as i32 + ox, (lone / d.w) as i32 + oy);
            w.terrain.kind[j] = GRASS;
        }
    }
    w.terrain.kind[lone] = FOREST;
    w.fell_tree(lone);
    for _ in 0..(200 * 10) {
        w.step(0.1);
    }
    assert_eq!(w.terrain.kind[inside], FOREST, "a gap in the forest grows back");
    assert_eq!(w.terrain.kind[lone], GRASS, "a clearing with no trees beside it stays open");
}
