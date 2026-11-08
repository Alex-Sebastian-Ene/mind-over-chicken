use moc_cosmos::galaxy::{Galaxy, Ship, PLANET_METRIC_STRIDE, SHIP_METRIC_STRIDE};
use moc_cosmos::math::Mulberry32;
use moc_cosmos::planet_sim::{
    arrive_ship, blink_planet, step_planet, BrainEffects, Channel, EventKind, MindSnapshot,
    PlanetState,
};

#[test]
fn test_calm_world_grows_with_calm_input() {
    let mut rng = Mulberry32::new(42);
    let mut planet = PlanetState::new(Channel::Calm, 20.0, 0, &mut rng);
    let fx_calm = BrainEffects {
        calm: 0.9,
        focus: 0.1,
        stress: 0.0,
        blink_rate: 0.0,
    };

    let mut had_harvest = false;
    for _ in 0..100 {
        let res = step_planet(&mut planet, 0.2, &fx_calm, &mut rng, false);
        for ev in res.events {
            if ev.kind == EventKind::Good && ev.text == "bumper harvest" {
                had_harvest = true;
            }
        }
    }

    assert!(planet.pop > 25.0, "Population should grow under high calm");
    assert!(had_harvest, "High calm should trigger bumper harvest events");
}

#[test]
fn test_focus_world_advances_eras() {
    let mut rng = Mulberry32::new(101);
    let mut planet = PlanetState::new(Channel::Focus, 50.0, 0, &mut rng);
    let fx_focus = BrainEffects {
        calm: 0.1,
        focus: 0.95,
        stress: 0.0,
        blink_rate: 0.0,
    };

    let start_era = planet.era_index;
    for _ in 0..200 {
        step_planet(&mut planet, 0.2, &fx_focus, &mut rng, false);
    }

    assert!(
        planet.era_index > start_era,
        "Focus world should advance past Stone Age"
    );
}

#[test]
fn test_stress_world_goat_market_volatility() {
    let mut rng = Mulberry32::new(777);
    let mut planet = PlanetState::new(Channel::Stress, 30.0, 1, &mut rng);

    let fx_stress = BrainEffects {
        calm: 0.0,
        focus: 0.0,
        stress: 1.0,
        blink_rate: 0.0,
    };

    let mut market_shifted = false;
    for _ in 0..150 {
        step_planet(&mut planet, 0.1, &fx_stress, &mut rng, false);
        if (planet.market - 100.0).abs() > 10.0 {
            market_shifted = true;
        }
    }

    assert!(
        market_shifted,
        "Goat market should fluctuate significantly under high stress"
    );
}

#[test]
fn test_blink_strikes_and_storms() {
    let mut rng = Mulberry32::new(888);
    let mut blink_planet_state = PlanetState::new(Channel::Blink, 40.0, 0, &mut rng);
    let mut other_planet_state = PlanetState::new(Channel::Calm, 40.0, 0, &mut rng);

    // Blink on blink world
    let ev1 = blink_planet(&mut blink_planet_state, &mut rng, false);
    assert!(ev1.is_some(), "Blink world should always react to blinks");
    assert_eq!(ev1.unwrap().kind, EventKind::Storm);
    assert_eq!(blink_planet_state.storms, 1);

    // Attended other world gets lightning strike directly
    let ev2 = blink_planet(&mut other_planet_state, &mut rng, true);
    assert!(ev2.is_some());
    assert_eq!(ev2.unwrap().text, "lightning strike!");
}

#[test]
fn test_mind_snapshot_ranking() {
    let mut mind = MindSnapshot::new();
    let mut formed_channels = Vec::new();
    // Simulate sequential 5-second mind readings for 3 worlds (15s total)
    for _ in 0..80 {
        if let Some(ch) = mind.update(0.2, 0.85, 0.40, 0.15, 1) {
            formed_channels.push(ch);
        }
    }

    assert!(mind.done, "Sequential mind snapshot should complete all 3 worlds after 15s");
    assert_eq!(formed_channels.len(), 3);
    assert_eq!(
        formed_channels[0],
        Channel::Calm,
        "Calm should be formed as first world"
    );
}

#[test]
fn test_galaxy_orchestration_and_ships() {
    let mut galaxy = Galaxy::new(12345);
    assert_eq!(galaxy.planets.len(), 4); // Home + 3 forming mind worlds

    let fx = BrainEffects {
        calm: 0.9,
        focus: 0.5,
        stress: 0.0,
        blink_rate: 0.0,
    };

    // Step galaxy through the sequential 5s mind snapshots (15s total)
    for _ in 0..80 {
        galaxy.step(0.2, &fx, 0);
    }

    assert!(galaxy.mind.done);
    // Mind worlds should now be formed and alive!
    let alive_count = galaxy.planets.iter().filter(|p| p.alive).count();
    assert!(alive_count >= 3, "Formed mind worlds should become alive");

    // Test colony ship arrival
    let mut target = PlanetState::new(Channel::Mixed, 0.0, 0, &mut Mulberry32::new(1));
    let outcome = arrive_ship(&mut target);
    assert_eq!(outcome, "settled");
    assert!(target.pop > 0.0);
}

#[test]
fn test_packed_planet_and_ship_metric_buffers() {
    let mut galaxy = Galaxy::new(4242);
    let fx = BrainEffects {
        calm: 0.5,
        focus: 0.5,
        stress: 0.5,
        blink_rate: 0.0,
    };
    for _ in 0..80 {
        galaxy.step(0.1, &fx, 0);
    }
    galaxy.ships.push(Ship {
        from_planet: 0,
        to_planet: 1,
        t: 0.25,
        duration: 6.0,
        mission: None,
    });

    let mut planets = Vec::new();
    galaxy.fill_planet_metrics(&mut planets);
    assert_eq!(planets.len(), galaxy.planets.len() * PLANET_METRIC_STRIDE);
    for (i, p) in galaxy.planets.iter().enumerate() {
        let o = i * PLANET_METRIC_STRIDE;
        let (x, y, _) = p.position(galaxy.time);
        assert_eq!(planets[o], p.state.pop);
        assert_eq!(planets[o + 1], p.state.era_index as f32);
        assert_eq!(planets[o + 2], p.state.progress);
        assert_eq!(planets[o + 3], p.state.market);
        assert_eq!(planets[o + 4], p.state.storms as f32);
        assert_eq!(planets[o + 5], if p.alive { 1.0 } else { 0.0 });
        assert_eq!(planets[o + 6], if p.forming { 1.0 } else { 0.0 });
        assert_eq!(planets[o + 7], x);
        assert_eq!(planets[o + 8], y);
    }

    let mut ships = Vec::new();
    galaxy.fill_ship_metrics(&mut ships);
    assert_eq!(ships.len(), galaxy.ships.len() * SHIP_METRIC_STRIDE);
    for (i, s) in galaxy.ships.iter().enumerate() {
        let o = i * SHIP_METRIC_STRIDE;
        assert_eq!(ships[o], s.from_planet as f32);
        assert_eq!(ships[o + 1], s.to_planet as f32);
        assert_eq!(ships[o + 2], s.t / s.duration.max(0.001));
        assert_eq!(ships[o + 3], s.duration);
    }

    // Reusing the buffers must not reallocate: JS holds a view over the same memory (#216).
    let cap = planets.capacity();
    galaxy.fill_planet_metrics(&mut planets);
    assert_eq!(planets.capacity(), cap);
    assert_eq!(planets.len(), galaxy.planets.len() * PLANET_METRIC_STRIDE);
}
