//! Boats keep to the water (#197): a voyage between two coasts goes round the land between
//! them instead of gliding over it, and still arrives.

use moc_civ::agents::{sail, sailable, set_goal, DOCK, SPEED_SAIL};
use moc_civ::planet::{Dims, Paths};
use moc_civ::terrain::{GRASS, MOUNTAIN, WATER};
use moc_civ::*;

/// Open sea with two islands (west 40..=60, east 140..=160) and a mountain isthmus between
/// them (x 98..=102, y 60..=160), plus a landlocked lake on a third island.
fn seas(d: Dims) -> Terrain {
    let mut kind = vec![WATER; d.n()];
    let mut fill = |x0: i32, x1: i32, y0: i32, y1: i32, k: u8| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                kind[d.idx(x, y)] = k;
            }
        }
    };
    fill(40, 60, 90, 130, GRASS);
    fill(140, 160, 90, 130, GRASS);
    fill(98, 102, 60, 160, MOUNTAIN);
    fill(280, 330, 80, 130, GRASS);
    fill(300, 310, 100, 110, WATER);
    let regions = Terrain::uniform(d, GRASS).regions;
    Terrain::from_kinds(d, kind, regions)
}

fn boat(w: &mut World, x: i32, y: i32) -> Agent {
    let f = w.add_faction(5, SeedSource::Run, Traits::from_seed(5), NO_F, None);
    let i = w.spawn_agent(f, NO_V, Job::Sailor, x, y).unwrap();
    let mut a = w.agents[i].clone();
    a.state = State::Sail;
    a
}

#[test]
fn boats_sail_round_the_land_between_two_coasts() {
    let d = Dims::SMALL;
    let t = seas(d);
    let mut w = World::with_terrain(1, seas(d));
    let (sx, sy, gx, gy) = (60, 110, 140, 110);
    let mut a = boat(&mut w, sx, sy);
    set_goal(&mut a, d, gx, gy);
    let mut sea = Paths::sea();
    let (mut arrived, mut round) = (false, false);
    for _ in 0..20_000 {
        sea.refill(1);
        if sail(&mut a, &t, &mut sea, SPEED_SAIL, 0.1) {
            arrived = true;
            break;
        }
        let (x, y) = (d.wrap_x(a.x.floor() as i32), d.wrap_y(a.y.floor() as i32));
        let ashore = d.cheb(x, y, sx, sy) <= 3 || d.cheb(x, y, gx, gy) <= DOCK;
        assert!(sailable(&t, x, y) || ashore, "the boat crossed land at ({x}, {y})");
        round |= !(60..=160).contains(&y);
    }
    assert!(arrived, "the boat never arrived (at {}, {})", a.x, a.y);
    assert!(round, "the boat should have gone round the isthmus");
    assert_eq!(d.cheb(a.x.floor() as i32, a.y.floor() as i32, gx, gy), 0);
}

#[test]
fn a_boat_with_no_way_by_sea_still_gets_there() {
    let d = Dims::SMALL;
    let t = seas(d);
    let mut w = World::with_terrain(1, seas(d));
    let mut a = boat(&mut w, 200, 105);
    set_goal(&mut a, d, 305, 105);
    let mut sea = Paths::sea();
    let arrived = (0..20_000).any(|_| {
        sea.refill(1);
        sail(&mut a, &t, &mut sea, SPEED_SAIL, 0.1)
    });
    assert!(arrived, "a landlocked lake is still reached (straight, as before)");
}
