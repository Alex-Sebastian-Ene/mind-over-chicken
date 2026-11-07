use moc_ai::observe::StoneAgeRawState;
use moc_ai::policy::{MLPPolicy, NUM_ACTIONS, NUM_INPUTS};

#[test]
fn test_embedded_policy_self_check() {
    let policy = MLPPolicy::from_embedded();
    let max_diff = policy.self_check();
    println!("MLP policy self-check max diff vs PyTorch reference: {:.2e}", max_diff);
    assert!(
        max_diff < 1e-5,
        "Policy output must match PyTorch reference export within 1e-5, got {:.2e}",
        max_diff
    );
}

#[test]
fn test_forward_pass_speed() {
    let policy = MLPPolicy::from_embedded();
    let obs = [0.5f32; NUM_INPUTS];

    // Warm-up
    for _ in 0..100 {
        policy.probs(&obs);
    }

    let iterations = 10_000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        policy.probs(&obs);
    }
    let elapsed = start.elapsed();
    let per_pass_micros = elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64;

    println!(
        "Forward pass speed: {:.3} µs per inference ({} iterations in {:?})",
        per_pass_micros, iterations, elapsed
    );

    // In release mode this is typically ~0.3 µs, in debug mode ~5 µs
    if !cfg!(debug_assertions) {
        assert!(
            per_pass_micros < 5.0,
            "Release mode inference should take < 5 µs per pass"
        );
    }
}

#[test]
fn test_action_distribution() {
    let policy = MLPPolicy::from_embedded();
    let obs = [0.5f32; NUM_INPUTS];
    let probs = policy.probs(&obs);

    let sum: f32 = probs.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-4,
        "Probabilities must sum to 1.0 (got {})",
        sum
    );

    let greedy_action = policy.act(&obs, true, 0.0);
    assert!(greedy_action < NUM_ACTIONS);
}

#[test]
fn test_observation_vector_encoding() {
    let raw = StoneAgeRawState {
        fire_lit: true,
        fire_heat: 0.8,
        food: 12.0,
        living_people: 18,
        avg_hunger: 0.15,
        starving_count: 0,
        avg_fear: 0.05,
        hold_active: true,
        hold_progress: 0.4,
        hold_successes: 1,
        nearest_mammoth_dist: 80.0,
        living_mammoths: 2,
        colonies: 1,
        rafts_in_transit: 0,
        people_overseas: 4,
        elapsed_time: 45.0,
        calm: 0.75,
        calm_threshold: 0.60,
        bolt_ready: true,
        meat_count: 5.0,
    };

    let obs = raw.to_observation_vector();
    assert_eq!(obs.len(), NUM_INPUTS);
    for (i, &val) in obs.iter().enumerate() {
        assert!(
            val >= 0.0 && val <= 1.0,
            "Feature {} is out of bounds [0, 1]: {}",
            i,
            val
        );
    }
}
