//! The planet is a torus (#111): it wraps north–south as well as east–west, so distances,
//! neighbours, contact, paths and the camera all take the short way round across the top and
//! bottom seam.

use moc_civ::agents::{set_goal, walk, ROUTE_FIELD};
use moc_civ::planet::{blit, Cam, Dims, LocalField, Paths};
use moc_civ::politics::slow_tick;
use moc_civ::terrain::GRASS;
use moc_civ::*;
use moc_planet::fb::Framebuffer;
use moc_planet::VH;

/// All grass, no ice anywhere: nothing stops a walker crossing the seam.
fn meadow(d: Dims) -> Terrain {
    let regions = Terrain::uniform(d, GRASS).regions;
    Terrain::from_kinds(d, vec![GRASS; d.n()], regions)
}

#[test]
fn distances_wrap_north_south() {
    let d = Dims::SMALL;
    let h = d.h as i32;
    assert!((d.dist(50.0, 1.0, 50.0, d.h as f32 - 2.0) - 3.0).abs() < 1e-4);
    assert_eq!(d.cheb(50, 0, 51, h - 1), 1);
    assert_eq!(d.dyi(h - 1, 1), 2);
    assert_eq!(d.idx(5, -1), d.idx(5, h - 1));
    assert_eq!(d.idx(5, h), d.idx(5, 0));
    assert_eq!(d.wrap_yf(-0.5), d.h as f32 - 0.5);
}

#[test]
fn walkers_and_fields_take_the_short_way_across_the_top_and_bottom() {
    let d = Dims::SMALL;
    let h = d.h as i32;
    let mut w = World::with_terrain(1, meadow(d));
    let f = w.add_faction(5, SeedSource::Run, Traits::from_seed(5), NO_F, None);
    let v = w.found_village(f, 100, 4, 6.0, false);
    let i = w.spawn_agent(f, v, Job::Warrior, 100, h - 4).unwrap();
    let mut a = w.agents[i].clone();
    set_goal(&mut a, d, 100, 4);
    let mut paths = Paths::new();
    let mut arrived = false;
    for _ in 0..300 {
        paths.refill(2);
        arrived = walk(&mut a, &w.terrain, &mut paths, 7.0, 1.0 / 30.0, ROUTE_FIELD);
        assert!(a.y < 12.0 || a.y > (h - 12) as f32, "went the long way: y = {}", a.y);
        if arrived {
            break;
        }
    }
    assert!(arrived, "never arrived; at ({}, {})", a.x, a.y);
    // A distance field round a target near the top reaches below the bottom edge.
    let field = LocalField::build(d, 100, 2, 20, |_, _| true);
    assert_eq!(field.at(100, h - 3), 5);
    let (x, y) = field.next_step(100, h - 3).unwrap();
    assert_eq!((y, field.at(x, y)), (h - 2, 4), "the next step goes down toward the seam");
}

#[test]
fn neighbours_meet_across_the_seam() {
    let d = Dims::SMALL;
    let h = d.h as i32;
    let mut w = World::with_terrain(2, meadow(d));
    let a = w.add_faction(11, SeedSource::Run, Traits::from_seed(11), NO_F, None);
    let b = w.add_faction(12, SeedSource::Run, Traits::from_seed(12), NO_F, None);
    let va = w.found_village(a, 60, 20, 6.0, false);
    let vb = w.found_village(b, 160, h - 20, 6.0, false);
    w.factions[a as usize].capital = va;
    w.factions[b as usize].capital = vb;
    w.spawn_agent(a, va, Job::Forage, 110, 1);
    w.spawn_agent(b, vb, Job::Forage, 110, h - 2);
    w.step(0.03);
    slow_tick(&mut w);
    assert!(w.rel(a, b).met, "three tiles apart across the seam");
    assert_eq!(w.events.iter().filter(|e| e.kind == ev::CONTACT).count(), 1);
}

#[test]
fn the_camera_and_view_cross_the_seam_both_ways() {
    let d = Dims::SMALL;
    let h = d.h as f32;
    // Looking at the top row: the view starts above it, across the seam.
    let mut c = Cam::centred_on(d, 100.0, 1.0);
    assert!(c.y > h - VH as f32);
    let (_, sy) = c.to_screen(100.0, 1.0);
    assert!((sy - (VH as f32 / 2.0)).abs() < 1.01, "the top row sits mid-screen ({sy})");
    let (_, sy) = c.to_screen(100.0, h - 2.0);
    assert!(sy > 0.0 && sy < VH as f32, "so does the bottom of the planet ({sy})");
    // Panning down across the seam and back up again.
    let y0 = c.y;
    c.pan(0.0, VH as f32);
    assert!(c.y < VH as f32, "wrapped past the bottom ({})", c.y);
    c.pan(0.0, -(VH as f32));
    assert!((c.y - y0).abs() < 1e-3);
    let (_, py) = c.to_planet(0.0, 0.0);
    assert!(py > h - VH as f32 && py < h);
    // The blit takes its rows from both sides of the seam.
    let mut layer = vec![0u8; d.n() * 4];
    let put = |l: &mut Vec<u8>, y: usize, c: [u8; 4]| {
        let i = (y * d.w + 100) * 4;
        l[i..i + 4].copy_from_slice(&c);
    };
    put(&mut layer, d.h - 1, [200, 0, 0, 255]);
    put(&mut layer, 0, [0, 200, 0, 255]);
    let cam = Cam { x: 90.0, y: h - 10.0, d };
    let mut fb = Framebuffer::default();
    blit(&mut fb, &layer, d, &cam);
    assert_eq!(fb.get(10, 9), Some([200, 0, 0, 255]), "the last row, just above the seam");
    assert_eq!(fb.get(10, 10), Some([0, 200, 0, 255]), "the first row, just below it");
}
