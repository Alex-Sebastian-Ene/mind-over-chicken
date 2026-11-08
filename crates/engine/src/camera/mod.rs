//! Camera & Input Module (#33, #67, #108).
//! Pure Rust implementations of gestures, finger tracking, face emotion analysis, and telemetry meters.

pub mod casts;
pub mod emotion;
pub mod fingers;
pub mod meter;
pub mod spells;

#[cfg(test)]
mod tests {
    use super::casts::*;
    use super::emotion::*;
    use super::fingers::*;
    use super::meter::*;
    use super::spells::*;
    use std::collections::HashMap;

    // -----------------------------------------------------------------------
    // Spells and Casts tests
    // -----------------------------------------------------------------------

    /// Feed one gesture at 30 fps over [t0, t1]; return the casts that fired.
    fn feed(c: &mut CastController, gesture: &str, t0: f64, t1: f64) -> Vec<(SpellId, f64)> {
        let mut out = Vec::new();
        let mut t = t0;
        while t <= t1 {
            if let Some(id) = c.update(gesture, 0.9, t).cast {
                out.push((id, t));
            }
            t += 1000.0 / 30.0;
        }
        out
    }

    #[test]
    fn test_three_core_moves_and_keys() {
        assert_eq!(SPELLS.len(), 3);
        let keys: Vec<u8> = SPELLS.iter().map(|s| s.key).collect();
        assert_eq!(keys, vec![1, 2, 3]);
        assert_eq!(spell_for_key_code("Digit1").unwrap().id, SpellId::Pinch);
        assert_eq!(spell_for_key_code("Digit2").unwrap().id, SpellId::Palm);
        assert_eq!(spell_for_key_code("Numpad3").unwrap().id, SpellId::Fireball);
        assert!(spell_for_key_code("Digit4").is_none());
        assert!(spell_for_key_code("KeyF").is_none());
        for s in SPELLS.iter() {
            assert_eq!(SpellId::from_str(s.id.as_str()), Some(s.id));
        }
        // No Strike move (#159), and the legacy ids are gone.
        for legacy in ["fist", "point", "spawn", "rain", "bless", "disaster", "peace"] {
            assert!(SpellId::from_str(legacy).is_none(), "{legacy}");
        }
    }

    #[test]
    fn test_gesture_to_cast_mapping() {
        let c = CastController::default();
        assert_eq!(c.spell_for("Open_Palm"), Some(SpellId::Palm));
        assert_eq!(c.spell_for("Closed_Fist"), Some(SpellId::Fireball));
        for g in ["Pointing_Up", "Victory", "Thumb_Up", "Thumb_Down", "ILoveYou", "None"] {
            assert_eq!(c.spell_for(g), None, "{g} casts nothing");
        }
        assert_eq!(spell_for_id(SpellId::Fireball).unwrap().release, Some("Open_Palm"));
        assert_eq!(spell_for_id(SpellId::Palm).unwrap().release, None);
    }

    #[test]
    fn test_held_fist_charges_but_never_casts() {
        let mut c = CastController::default();
        assert!(feed(&mut c, "Closed_Fist", 0.0, 20_000.0).is_empty());
        let u = c.update("Closed_Fist", 0.9, 20_010.0);
        assert_eq!(u.held, Some(SpellId::Fireball));
        assert_eq!(u.progress, 1.0);
        assert_eq!(u.armed, Some(SpellId::Fireball));
    }

    #[test]
    fn test_fist_then_open_releases_exactly_one_fireball() {
        let mut c = CastController::default();
        assert!(feed(&mut c, "Closed_Fist", 0.0, 500.0).is_empty());
        // Two transition frames the recognizer often reports between the poses.
        c.update("None", 0.0, 533.0);
        c.update("None", 0.0, 566.0);
        let casts = feed(&mut c, "Open_Palm", 600.0, 3_000.0);
        assert_eq!(casts, vec![(SpellId::Fireball, 600.0)], "one fireball, and the opening hand does not also calm");
        // Leave the palm, then a fresh palm hold calms as usual.
        feed(&mut c, "None", 3_033.0, 3_400.0);
        let calm = feed(&mut c, "Open_Palm", 3_433.0, 4_000.0);
        assert_eq!(calm.len(), 1);
        assert_eq!(calm[0].0, SpellId::Palm);
    }

    #[test]
    fn test_palm_alone_calms_once_and_never_fires() {
        let mut c = CastController::default();
        let casts = feed(&mut c, "Open_Palm", 0.0, 20_000.0);
        assert_eq!(casts.len(), 1);
        assert_eq!(casts[0].0, SpellId::Palm);
        assert!(casts[0].1 >= 300.0);
    }

    #[test]
    fn test_short_fist_or_late_open_does_not_fire() {
        // A fist shorter than the hold time never charges.
        let mut c = CastController::default();
        feed(&mut c, "Closed_Fist", 0.0, 200.0);
        let casts = feed(&mut c, "Open_Palm", 233.0, 2_000.0);
        assert!(casts.iter().all(|(id, _)| *id == SpellId::Palm), "{casts:?}");

        // Opening the hand long after the charge was lost does nothing either.
        let mut d = CastController::default();
        feed(&mut d, "Closed_Fist", 0.0, 500.0);
        feed(&mut d, "None", 533.0, 1_400.0);
        let late = d.update("Open_Palm", 0.9, 1_433.0);
        assert_eq!(late.cast, None);
        assert_eq!(late.armed, None);
    }

    #[test]
    fn test_fireball_cooldown_eats_the_release() {
        let mut c = CastController::default();
        feed(&mut c, "Closed_Fist", 0.0, 400.0);
        assert_eq!(c.update("Open_Palm", 0.9, 433.0).cast, Some(SpellId::Fireball));
        feed(&mut c, "None", 466.0, 700.0);
        feed(&mut c, "Closed_Fist", 733.0, 1_200.0);
        // Still cooling (1.5 s): the release is consumed, nothing fires, no Calm either.
        assert!(feed(&mut c, "Open_Palm", 1_233.0, 2_000.0).is_empty());
        assert!(c.cooldown_left(SpellId::Fireball, 1_233.0) > 0.0);
    }

    #[test]
    fn test_tracking_dropouts_and_grace() {
        let mut c = CastController::new(300.0, 0.55, 0.45, 150.0);
        c.update("Open_Palm", 0.9, 0.0);
        c.update("Open_Palm", 0.9, 150.0);
        c.update("None", 0.0, 200.0); // 50 ms dropout
        assert_eq!(c.update("Open_Palm", 0.9, 310.0).cast, Some(SpellId::Palm));

        let mut d = CastController::new(300.0, 0.55, 0.45, 150.0);
        d.update("Open_Palm", 0.9, 0.0);
        d.update("Open_Palm", 0.9, 150.0);
        d.update("None", 0.0, 200.0);
        d.update("None", 0.0, 400.0); // 250 ms dropout > grace: hold dropped
        assert_eq!(d.update("Open_Palm", 0.9, 420.0).cast, None);
        assert_eq!(d.update("Open_Palm", 0.9, 720.0).cast, Some(SpellId::Palm));
    }

    #[test]
    fn test_keyboard_trigger_and_cooldowns() {
        let mut c = CastController::default();
        assert!(c.trigger(SpellId::Palm, 0.0));
        assert!(!c.trigger(SpellId::Palm, 1000.0));
        assert!((c.cooldown_frac(SpellId::Palm, 0.0) - 1.0).abs() < 1e-9);
        assert_eq!(c.cooldown_left(SpellId::Palm, 5000.0), 0.0);
        assert!(c.trigger(SpellId::Palm, 5000.0));
        assert!(c.trigger(SpellId::Fireball, 0.0));
        assert!(!c.trigger(SpellId::Fireball, 1000.0));
        assert!(c.trigger(SpellId::Fireball, 1500.0));
    }

    // -----------------------------------------------------------------------
    // Fingers tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_one_euro_filter_jitter_and_responsiveness() {
        let mut f = OneEuroFilter::default();
        assert_eq!(f.filter(0.3, 0.0), 0.3);
        let a = f.filter(0.5, 33.0);
        assert_eq!(f.filter(0.9, 33.0), a); // duplicate time changes nothing

        // Long gap re-seeds
        assert_eq!(f.filter(0.8, 2000.0), 0.8);
    }

    #[test]
    fn test_soft_dead_zone() {
        let o = (0.5, 0.5);
        assert_eq!(soft_dead_zone(Some(o), (0.503, 0.5), 0.004), o);
        let moved = soft_dead_zone(Some(o), (0.51, 0.5), 0.004);
        assert!((moved.0 - 0.506).abs() < 1e-9 && moved.1 == 0.5);
    }

    #[test]
    fn test_pinch_detector_cycle() {
        let mut p = PinchDetector::default();
        // Ratio entering pinch: < 0.3 for 50 ms
        let ev1 = p.update(Some(0.25), 0.0, true);
        assert!(!ev1.active && !ev1.started);
        let ev2 = p.update(Some(0.25), 60.0, true);
        assert!(ev2.active && ev2.started);

        // Still pinching
        let ev3 = p.update(Some(0.25), 200.0, true);
        assert!(ev3.active && !ev3.started && ev3.held_ms >= 140.0);

        // Opening: > 0.45 for 60 ms
        p.update(Some(0.5), 220.0, true);
        let ev4 = p.update(Some(0.5), 290.0, true);
        assert!(!ev4.active && ev4.released);
    }

    // -----------------------------------------------------------------------
    // Emotion tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_emotion_blendshape_mapping() {
        let mut smile = HashMap::new();
        smile.insert("mouthSmileLeft".to_string(), 0.8);
        smile.insert("mouthSmileRight".to_string(), 0.8);
        let em = emotions_from_blendshapes(&smile, None);
        assert!(em.joy > 0.8);
        assert!(em.anger < 0.1);

        let mut tracker = EmotionTracker::new(0.0);
        let (mood, ev) = tracker.update(Some(&smile), 1.5);
        assert_eq!(mood.dominant, Emotion::Joy);
        assert_eq!(ev, Some(EmotionEvent::Change));
    }

    // -----------------------------------------------------------------------
    // Meter tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_rate_meter_and_frame_meter() {
        let mut rm = RateMeter::new(1000.0);
        rm.tick(0.0);
        rm.tick(500.0);
        rm.tick(1000.0);
        let hz = rm.hz(1000.0);
        assert!((hz - 2.0).abs() < 0.1);

        let mut fm = FrameMeter::default();
        fm.frame(0.0);
        fm.frame(16.6);
        fm.frame(33.3);
        assert!(fm.fps(33.3) > 55.0);
    }
}
