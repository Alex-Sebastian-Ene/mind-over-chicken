//! Generation timings (from #112). Alex (#109): "remove the deterministic feature for now; use
//! the random with the diffusion model", so maps are random per run and these wall-clock
//! checks are informational: ignored by default (the shared build machine is too busy for
//! wall-clock asserts), they print the times when run with `--ignored`.

use moc_terrain::TerrainMap;
use std::time::Instant;

#[test]
#[ignore = "timing only: cargo test -p moc-terrain --release -- --ignored --nocapture"]
fn test_home_map_generation_speed() {
    let start = Instant::now();
    let map = TerrainMap::generate_home_world(101);
    println!("Home map ({}x{}) generated in: {:?}", map.width, map.height, start.elapsed());
}

#[test]
#[ignore = "timing only: cargo test -p moc-terrain --release -- --ignored --nocapture"]
fn test_civ_planet_generation_speed() {
    let start = Instant::now();
    let map = TerrainMap::generate_civ_planet(202, 2560, 1280);
    println!("Civ planet ({}x{}) generated in: {:?}", map.width, map.height, start.elapsed());
}
