//! Trees and foliage on the civ planet (#205): forests are textured, not flat green, they stay
//! readable as land by the 3D view's height-from-colour, and the paint follows the runtime
//! felling and regrowth (#265).
use std::collections::HashSet;

use moc_civ::planet::Dims;
use moc_civ::terrain::{FOREST, GRASS};
use moc_civ::*;

/// The colour tests in src/civ/sandbox.js `getVisibleHeightmap` that a tree must pass as
/// plains/forest: green-dominant, never read as water, hills or mountain.
fn reads_as_green_land(c: [u8; 4]) -> bool {
    let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
    let water = b > r + 15 && b > 70;
    let snow = r > 200 && g > 200 && b > 200;
    let mountain = r > 100 && g > 95 && b > 85 && (r - g).abs() < 18;
    let hills = r > 110 && g > 105 && b < 100;
    !water && !snow && !mountain && !hills && g > r && g > b
}

#[test]
fn forests_have_trees_not_one_flat_green() {
    let t = Terrain::generate(7, Dims::DEFAULT);
    let d = t.d;
    let mut forest = HashSet::new();
    let mut n = 0;
    for y in 0..d.h as i32 {
        for x in 0..d.w as i32 {
            let i = d.idx(x, y);
            if t.kind[i] != FOREST && t.kind[i] != GRASS || t.river[i] {
                continue;
            }
            let c = t.base_colour(x, y);
            assert_eq!(&t.rgba[i * 4..i * 4 + 4], &c, "the painted planet matches base_colour");
            assert!(reads_as_green_land(c), "tile ({x},{y}) kind {} colour {c:?} would not read as land in 3D", t.kind[i]);
            if t.kind[i] == FOREST {
                n += 1;
                forest.insert([c[0] / 8, c[1] / 8, c[2] / 8]);
            }
        }
    }
    assert!(n > 1000, "the planet has forests ({n} tiles)");
    // Shadows, canopy and lit crowns of three kinds of tree, not a flat fill with jitter.
    assert!(forest.len() > 20, "forest colours: {} distinct", forest.len());
}

#[test]
fn foliage_is_deterministic_and_follows_felling_and_regrowth() {
    let a = Terrain::generate(11, Dims::DEFAULT);
    let b = Terrain::generate(11, Dims::DEFAULT);
    assert!(a.rgba == b.rgba, "same seed, same paint");

    let d = Dims::DEFAULT;
    let regions = Terrain::uniform(d, FOREST).regions;
    let mut w = World::with_terrain(3, Terrain::from_kinds(d, vec![FOREST; d.n()], regions));
    w.agents.clear();
    let (x, y) = ((d.w / 2) as i32, (d.h / 2) as i32);
    let i = d.idx(x, y);
    let before = w.terrain.base_colour(x, y);
    w.fell_tree(i);
    assert_eq!(w.terrain.kind[i], GRASS);
    assert_eq!(&w.terrain.rgba[i * 4..i * 4 + 4], &w.terrain.base_colour(x, y), "a felled tree repaints as grass");
    for _ in 0..(110 * 10) {
        w.step(0.1);
        if w.terrain.kind[i] == FOREST {
            break;
        }
    }
    assert_eq!(w.terrain.kind[i], FOREST, "the clearing grows back");
    assert_eq!(&w.terrain.rgba[i * 4..i * 4 + 4], &before, "and is painted with its tree again");
}

#[test]
fn big_planets_keep_small_biome_patches() {
    // A 2560×1280 planet: without the fixed-scale detail octave its woods and meadows come out
    // as blobs four times the size of the default planet's. Count forest/grass boundaries per
    // land tile on each and expect the big one to be at least comparably broken up.
    let edges = |t: &Terrain| {
        let d = t.d;
        let (mut land, mut e) = (0usize, 0usize);
        for y in 0..d.h as i32 {
            for x in 0..d.w as i32 {
                let k = t.kind[d.idx(x, y)];
                if k != FOREST && k != GRASS {
                    continue;
                }
                land += 1;
                let k2 = t.kind[d.idx(x + 1, y)];
                if (k2 == FOREST || k2 == GRASS) && k2 != k {
                    e += 1;
                }
            }
        }
        e as f32 / land.max(1) as f32
    };
    let small = edges(&Terrain::generate(5, Dims::DEFAULT));
    let big = edges(&Terrain::generate(5, Dims::new(2560, 1280)));
    assert!(big > small * 0.8, "biome edges per tile: big {big:.4} vs default {small:.4}");
}
