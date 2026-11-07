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

    #[test]
    fn test_seven_spells_and_keys() {
        assert_eq!(SPELLS.len(), 7);
        let keys: Vec<u8> = SPELLS.iter().map(|s| s.key).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(spell_for_key_code("Digit2").unwrap().id, SpellId::Fireball);
        assert_eq!(spell_for_key_code("Numpad7").unwrap().id, SpellId::Peace);
        assert!(spell_for_key_code("Digit8").is_none());
        assert!(spell_for_key_code("KeyF").is_none());
    }

    #[test]
    fn test_gesture_to_cast_mapping() {
        let c = CastController::default();
        assert_eq!(c.spell_for("Pointing_Up"), Some(SpellId::Point));
        assert_eq!(c.spell_for("Closed_Fist"), Some(SpellId::Fireball));
        assert_eq!(c.spell_for("Open_Palm"), Some(SpellId::Spawn));
        assert_eq!(c.spell_for("Victory"), Some(SpellId::Rain));
        assert_eq!(c.spell_for("Thumb_Up"), Some(SpellId::Bless));
        assert_eq!(c.spell_for("Thumb_Down"), Some(SpellId::Disaster));
        assert_eq!(c.spell_for("ILoveYou"), Some(SpellId::Peace));
        assert_eq!(c.spell_for("None"), None);
    }

    #[test]
    fn test_hold_debounce_and_cast_once() {
        let mut c = CastController::default();
        // 250 ms: not enough
        for t in (0..=250).step_by(33) {
            let u = c.update("Closed_Fist", 0.9, t as f64);
            assert!(u.cast.is_none());
            if t == 250 {
                assert!(u.progress > 0.7 && u.progress < 1.0);
            }
        }

        // At 300 ms: fires fireball!
        let r = c.update("Closed_Fist", 0.9, 300.0);
        assert_eq!(r.cast, Some(SpellId::Fireball));

        // Long hold (up to 20 seconds): casts once per hold
        for t in (330..=20000).step_by(33) {
            let u = c.update("Closed_Fist", 0.9, t as f64);
            assert!(u.cast.is_none());
        }
    }

    #[test]
    fn test_tracking_dropouts_and_grace() {
        let mut c = CastController::new(300.0, 0.55, 0.45, 150.0);
        c.update("Victory", 0.9, 0.0);
        c.update("Victory", 0.9, 150.0);
        c.update("None", 0.0, 200.0); // 50 ms dropout
        assert_eq!(c.update("Victory", 0.9, 310.0).cast, Some(SpellId::Rain));

        let mut d = CastController::new(300.0, 0.55, 0.45, 150.0);
        d.update("Victory", 0.9, 0.0);
        d.update("Victory", 0.9, 150.0);
        d.update("None", 0.0, 200.0);
        d.update("None", 0.0, 400.0); // 250 ms dropout > grace: hold dropped
        assert_eq!(d.update("Victory", 0.9, 420.0).cast, None);
        assert_eq!(d.update("Victory", 0.9, 720.0).cast, Some(SpellId::Rain));
    }

    #[test]
    fn test_keyboard_trigger_and_cooldowns() {
        let mut c = CastController::default();
        assert!(c.trigger(SpellId::Peace, 0.0));
        assert!(!c.trigger(SpellId::Peace, 1000.0));
        assert!((c.cooldown_frac(SpellId::Peace, 0.0) - 1.0).abs() < 1e-9);
        assert_eq!(c.cooldown_left(SpellId::Peace, 15000.0), 0.0);
        assert!(c.trigger(SpellId::Peace, 15000.0));
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
