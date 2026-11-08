//! Pinch-grab in the civ sandbox (#188): find the person under the hand and set them down.
use moc_civ::*;

#[test]
fn a_person_can_be_picked_up_and_set_down_on_land_only() {
    let mut w = World::new(3);
    for _ in 0..20 {
        w.step(0.1);
    }
    let i = w.agents.iter().position(|a| a.alive).expect("someone alive");
    let (x, y) = (w.agents[i].x, w.agents[i].y);
    assert_eq!(w.agent_near(x + 0.5, y, 6.0), Some(i));

    // Somewhere else on land a few tiles away.
    let d = w.d;
    let land = (1..30)
        .flat_map(|r| [(r, 0), (-r, 0), (0, r), (0, -r)])
        .map(|(ox, oy)| (x as i32 + ox, y as i32 + oy))
        .find(|&(tx, ty)| w.terrain.walkable(tx, ty) && (tx, ty) != (x as i32, y as i32))
        .expect("land nearby");
    assert!(w.move_agent(i, land.0 as f32 + 0.5, land.1 as f32 + 0.5));
    assert_eq!((w.agents[i].x.floor() as i32, w.agents[i].y.floor() as i32), (d.wrap_x(land.0), d.wrap_y(land.1)));

    // Never into the sea.
    let sea = (0..d.n()).find(|&j| !w.terrain.walk[j]).expect("some sea");
    let (sx, sy) = ((sea % d.w) as f32 + 0.5, (sea / d.w) as f32 + 0.5);
    assert!(!w.move_agent(i, sx, sy));
    w.step(0.1); // the sim carries on from the new spot
}
