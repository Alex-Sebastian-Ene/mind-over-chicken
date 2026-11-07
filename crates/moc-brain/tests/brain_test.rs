use moc_brain::brain::Brain;
use moc_brain::calibration::{random_seed_traits, traits_from_features, SensorFeatures};
use moc_brain::model::{features_from_bands, Bands, Class, PersonalModel};
use moc_brain::sim_source::{BrainFrame, SimFingerprint, SimSource};

#[test]
fn test_scripted_player_following_cues_achieves_target_accuracy() {
    // Issue #20: "a Rust test with a scripted player following cues reaches ≥ 85% CV accuracy (JS reference: 88%)"
    let mut model = PersonalModel::new();
    let mut sim = SimSource::with_fingerprint(SimFingerprint::default_preset(), 42);

    let cues = [
        (Class::Rest, 70),  // 70 frames of rest
        (Class::Calm, 70),  // 70 frames of calm
        (Class::Focus, 70), // 70 frames of focus
        (Class::Rest, 70),
        (Class::Calm, 70),
        (Class::Focus, 70),
    ];

    for (cue, frame_count) in cues {
        sim.input.calm = cue == Class::Calm;
        sim.input.focus = cue == Class::Focus;
        sim.input.panic = false;

        for _ in 0..frame_count {
            sim.update(0.1, |frame| {
                let f = features_from_bands(&frame.bands);
                model.add(cue, f);
            });
        }
    }

    assert!(model.count(Class::Rest) >= 50);
    assert!(model.count(Class::Calm) >= 50);
    assert!(model.count(Class::Focus) >= 50);

    let trained = model.train();
    assert!(trained, "Model should successfully train");

    println!(
        "Model trained with accuracy: {:.2}% (target >= 85%, JS reference: 88%), last_train_ms: {:.2}ms",
        model.accuracy * 100.0,
        model.last_train_ms
    );

    assert!(
        model.accuracy >= 0.85,
        "Expected accuracy >= 85%, got {:.2}%",
        model.accuracy * 100.0
    );

    // Predict rest, calm, focus states
    // A calm brain state should yield high calm probability
    sim.input.calm = true;
    sim.input.focus = false;
    for _ in 0..30 {
        sim.update(0.1, |_| {});
    }
    let mut calm_frame = None;
    sim.update(0.1, |f| calm_frame = Some(f));
    let cf = features_from_bands(&calm_frame.unwrap().bands);
    let pred_calm = model.predict(&cf).expect("Prediction available");
    assert!(
        pred_calm.calm.unwrap() > pred_calm.focus.unwrap(),
        "Calm prob should exceed focus prob"
    );

    // A focus brain state should yield high focus probability
    sim.input.calm = false;
    sim.input.focus = true;
    for _ in 0..30 {
        sim.update(0.1, |_| {});
    }
    let mut focus_frame = None;
    sim.update(0.1, |f| focus_frame = Some(f));
    let ff = features_from_bands(&focus_frame.unwrap().bands);
    let pred_focus = model.predict(&ff).expect("Prediction available");
    assert!(
        pred_focus.focus.unwrap() > pred_focus.calm.unwrap(),
        "Focus prob should exceed calm prob"
    );
}

#[test]
fn test_retrain_time_benchmark() {
    // Issue #20: "and a retrain takes ≤ 5 ms in release wasm"
    let mut model = PersonalModel::new();
    let mut sim = SimSource::new(999);

    for &cue in &[Class::Rest, Class::Calm, Class::Focus] {
        sim.input.calm = cue == Class::Calm;
        sim.input.focus = cue == Class::Focus;
        for _ in 0..120 {
            sim.update(0.1, |frame| {
                let f = features_from_bands(&frame.bands);
                model.add(cue, f);
            });
        }
    }

    let start = std::time::Instant::now();
    let trained = model.train();
    let duration = start.elapsed();

    assert!(trained);
    println!("Retrain duration in test: {:?}", duration);
    if cfg!(debug_assertions) {
        assert!(duration.as_millis() < 150, "Debug mode should be < 150ms");
    } else {
        assert!(
            duration.as_millis() <= 5,
            "Release retrain should be <= 5ms (got {:?})",
            duration
        );
    }
}

#[test]
fn test_calibration_traits_reproducibility() {
    // Dummy / random traits must be exactly reproducible from a seed
    let traits1 = random_seed_traits(42);
    let traits2 = random_seed_traits(42);
    assert_eq!(traits1, traits2);

    let traits3 = random_seed_traits(100);
    assert_ne!(traits1, traits3);
}

#[test]
fn test_calibration_sensor_formula() {
    // Test the formula from docs/calibration.md and PR #63:
    // friendliness = clamp(0.55 * calm + 0.35 * joy + 0.1 * (1 - anger))
    // aggressiveness = clamp(0.5 * pulse + 0.35 * anger + 0.15 * focus)
    // violence = clamp(0.45 * anger + 0.35 * pulse * (1 - calm) + 0.2 * blinks)
    let features = SensorFeatures {
        calm: 0.8,
        focus: 0.2,
        hr_delta: 0.0,
        blink_rate: 1.0,
        joy: 0.9,
        anger: 0.0,
        surprise: 0.0,
        sadness: 0.0,
        face_seen: true,
    };
    let traits = traits_from_features(&features);
    // friendliness: 0.55 * 0.8 + 0.35 * 0.9 + 0.1 * 1.0 = 0.44 + 0.315 + 0.1 = 0.855
    assert!((traits.friendliness - 0.855).abs() < 1e-3);
    // aggressiveness: 0.5 * 0 + 0.35 * 0 + 0.15 * 0.2 = 0.03
    assert!((traits.aggressiveness - 0.03).abs() < 1e-3);
}

#[test]
fn test_brain_blink_debounce() {
    let mut brain = Brain::new();

    let mut frame = BrainFrame::default();
    frame.bands = Bands {
        delta: 0.2,
        theta: 0.2,
        alpha: 0.2,
        beta: 0.2,
        gamma: 0.2,
    };
    frame.blink = true;

    // First blink at t=0
    assert!(brain.ingest(&frame));
    assert_eq!(brain.stats.blinks, 1);

    // Second blink 0.2s later (should be debounced / rejected)
    brain.update(0.2);
    assert!(!brain.ingest(&frame));
    assert_eq!(brain.stats.blinks, 1);

    // Third blink 0.5s later (t=0.7s, > 0.45s lockout, should register)
    brain.update(0.5);
    assert!(brain.ingest(&frame));
    assert_eq!(brain.stats.blinks, 2);
}

#[test]
fn test_resting_hr_estimation() {
    let mut brain = Brain::new();
    let mut frame = BrainFrame::default();
    frame.bands = Bands {
        delta: 0.2,
        theta: 0.2,
        alpha: 0.2,
        beta: 0.2,
        gamma: 0.2,
    };

    // Feed 25 heart rate values from 60 to 84 bpm
    for i in 0..25 {
        brain.update(1.0);
        frame.hr = Some(60.0 + i as f32);
        brain.ingest(&frame);
    }

    assert!(brain.rest_hr.is_some());
    let rest = brain.rest_hr.unwrap();
    // 25th percentile of 60..84 is around 66 bpm
    assert!(rest >= 64.0 && rest <= 68.0, "rest_hr = {}", rest);
    assert!(brain.hr_delta() > 0.0);
}
