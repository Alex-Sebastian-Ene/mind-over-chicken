//! The planet generator: bigger planets get bigger lands, and the home continents stay joined
//! on foot by their isthmuses (which bend instead of running ruler-straight, Alex).

use moc_civ::terrain::HOME_REGIONS;
use moc_civ::{Dims, Terrain};

fn land_share(t: &Terrain) -> f32 {
    t.walk.iter().filter(|&&w| w).count() as f32 / t.d.n() as f32
}

#[test]
fn bigger_planets_have_bigger_lands_not_more_ocean() {
    for seed in [3, 11, 29] {
        let t = Terrain::generate(seed, Dims::new(2560, 1280));
        let land = land_share(&t);
        assert!((0.2..0.45).contains(&land), "seed {seed}: land {land:.2} on 2560x1280");
        assert!(t.regions.len() >= 8, "seed {seed}: only {} continents", t.regions.len());
    }
}

#[test]
fn home_continents_are_joined_on_foot() {
    for (seed, w, h) in [(1, 1280, 640), (7, 1920, 960), (11, 2560, 1280), (5, 3072, 1536)] {
        let t = Terrain::generate(seed, Dims::new(w, h));
        let homes: Vec<u16> = t.regions[..HOME_REGIONS].iter().map(|r| t.land(r.start.0, r.start.1)).collect();
        assert!(homes.iter().all(|&l| l != 0 && l == homes[0]), "seed {seed} {w}x{h}: home landmasses {homes:?}");
    }
}

#[test]
fn the_same_seed_and_size_make_the_same_planet() {
    let a = Terrain::generate(42, Dims::new(1920, 960));
    let b = Terrain::generate(42, Dims::new(1920, 960));
    assert_eq!(a.kind, b.kind);
}
