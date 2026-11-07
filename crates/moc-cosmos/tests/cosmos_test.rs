use moc_cosmos::galaxy::Galaxy;
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
    // Simulate 10 seconds of high calm and moderate focus
    for _ in 0..55 {
        let finished = mind.update(0.2, 0.85, 0.40, 0.15, 1);
        if finished.is_some() {
            break;
        }
    }

    assert!(mind.done, "Mind snapshot should complete after 10s");
    let ranked = mind.ranked.as_ref().unwrap();
    assert_eq!(
        ranked[0].0,
        Channel::Calm,
        "Calm should be ranked #1 channel"
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

    // Step galaxy through the 10s mind snapshot
    for _ in 0..55 {
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
