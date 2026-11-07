use moc_terrain::TerrainMap;
use std::time::Instant;

#[test]
fn test_home_map_generation_speed() {
    let start = Instant::now();
    let map = TerrainMap::generate_home_world(101);
    let elapsed = start.elapsed();

    println!("Home map (320x180) generated in: {:?}", elapsed);
    assert_eq!(map.width, 320);
    assert_eq!(map.height, 180);
    let max_ms = if cfg!(debug_assertions) { 500 } else { 80 };
    assert!(elapsed.as_millis() < max_ms, "Generation must be fast: {:?} elapsed (max: {}ms)", elapsed, max_ms);
}

#[test]
fn test_civ_planet_generation_speed() {
    let start = Instant::now();
    let map = TerrainMap::generate_civ_planet(202, 640, 320);
    let elapsed = start.elapsed();

    println!("Civ planet (640x320) generated in: {:?}", elapsed);
    assert_eq!(map.width, 640);
    assert_eq!(map.height, 320);
    let max_sec = if cfg!(debug_assertions) { 2.5 } else { 0.5 };
    assert!(elapsed.as_secs_f32() < max_sec, "Civ planet must generate under {}s: {:?} elapsed", max_sec, elapsed);
}
