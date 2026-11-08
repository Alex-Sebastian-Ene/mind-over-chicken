//! Camera casts debounce and cooldown controller (#33, #108, #150, #159).
//! The browser runs this one (RustCastController); src/camera/casts.js mirrors it for Node.
//!
//! Hold moves (Calm) cast once the gesture has been held for `hold_ms`, once per hold: to cast
//! again, change or drop the gesture. Short tracking dropouts (under `grace_ms`) don't break a
//! hold. A held gesture casts as soon as its cooldown ends.
//!
//! Charge moves (Fireball, #150) never cast while held. Holding the charge gesture (a fist) for
//! `hold_ms` arms the move; its release gesture (an open hand) within `release_ms` of the last
//! charged frame casts it exactly once. The open hand that released it is spent: it has to leave
//! Open_Palm before it can hold-cast Calm, so one fist-to-open never casts both.
//! Keyboard casts (`trigger`) share the same cooldowns.

use super::spells::{spell_for_gesture, spell_for_id, SpellId};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastUpdate {
    pub held: Option<SpellId>,
    pub progress: f64,
    pub cast: Option<SpellId>,
    pub aiming: bool,
    /// A charged move waiting for its release gesture.
    pub armed: Option<SpellId>,
}

#[derive(Debug, Clone)]
pub struct CastController {
    pub hold_ms: f64,
    pub min_score: f64,
    pub aim_score: f64,
    pub grace_ms: f64,
    pub release_ms: f64,
    pub ready_at: HashMap<SpellId, f64>,
    pub held: Option<SpellId>,
    pub since: f64,
    pub last_seen: f64,
    pub latched: bool,
    /// (charged move, last time its charge gesture was seen)
    pub armed: Option<(SpellId, f64)>,
}

impl Default for CastController {
    fn default() -> Self {
        Self::new(300.0, 0.55, 0.45, 150.0)
    }
}

impl CastController {
    pub const RELEASE_MS: f64 = 600.0;

    pub fn new(hold_ms: f64, min_score: f64, aim_score: f64, grace_ms: f64) -> Self {
        Self {
            hold_ms,
            min_score,
            aim_score,
            grace_ms,
            release_ms: Self::RELEASE_MS,
            ready_at: HashMap::new(),
            held: None,
            since: 0.0,
            last_seen: 0.0,
            latched: false,
            armed: None,
        }
    }

    pub fn reset(&mut self) {
        self.drop_hold();
        self.armed = None;
    }

    fn drop_hold(&mut self) {
        self.held = None;
        self.since = 0.0;
        self.last_seen = 0.0;
        self.latched = false;
    }

    pub fn spell_for(&self, gesture: &str) -> Option<SpellId> {
        spell_for_gesture(gesture).map(|s| s.id)
    }

    pub fn update(&mut self, gesture: &str, score: f64, now: f64) -> CastUpdate {
        let confident = score >= self.min_score;
        let id = if confident { self.spell_for(gesture) } else { None };
        let aiming = gesture == "Pointing_Up" && score >= self.aim_score;
        let mut cast = None;

        // A charged move fires on its release gesture, before that gesture can start a hold.
        if let Some((armed_id, seen)) = self.armed {
            let release = spell_for_id(armed_id).and_then(|s| s.release);
            if now - seen > self.release_ms {
                self.armed = None;
            } else if confident && release == Some(gesture) {
                self.armed = None;
                if self.ready(armed_id, now) {
                    cast = Some(armed_id);
                    self.start_cooldown(armed_id, now);
                }
                // The opening hand is spent on the release (no Calm from the same motion).
                self.held = id;
                self.since = now;
                self.last_seen = now;
                self.latched = true;
                return CastUpdate { held: self.held, progress: 0.0, cast, aiming, armed: None };
            }
        }

        if let Some(spell_id) = id {
            if self.held != Some(spell_id) {
                self.held = Some(spell_id);
                self.since = now;
                self.latched = false;
            }
            self.last_seen = now;
        } else if self.held.is_some() && now - self.last_seen > self.grace_ms {
            self.drop_hold();
        }

        let held_for = if self.held.is_some() { now - self.since } else { 0.0 };
        if let Some(spell_id) = self.held {
            let charge = spell_for_id(spell_id).map_or(false, |s| s.release.is_some());
            if charge {
                // Charging never casts; once charged it stays armed for `release_ms` after the
                // charge gesture was last seen.
                if held_for >= self.hold_ms {
                    self.armed = Some((spell_id, self.last_seen));
                }
            } else if !self.latched && held_for >= self.hold_ms && self.ready(spell_id, now) {
                cast = Some(spell_id);
                self.latched = true;
                self.start_cooldown(spell_id, now);
            }
        }

        CastUpdate {
            held: self.held,
            progress: if self.held.is_some() {
                (held_for / self.hold_ms).clamp(0.0, 1.0)
            } else {
                0.0
            },
            cast,
            aiming,
            armed: self.armed.map(|(id, _)| id),
        }
    }

    pub fn trigger(&mut self, id: SpellId, now: f64) -> bool {
        if !self.ready(id, now) {
            return false;
        }
        self.start_cooldown(id, now);
        true
    }

    pub fn ready(&self, id: SpellId, now: f64) -> bool {
        let ready_time = self.ready_at.get(&id).copied().unwrap_or(f64::NEG_INFINITY);
        now >= ready_time
    }

    pub fn start_cooldown(&mut self, id: SpellId, now: f64) {
        let cd = spell_for_id(id).map(|s| s.cooldown_ms).unwrap_or(0.0);
        self.ready_at.insert(id, now + cd);
    }

    pub fn cooldown_left(&self, id: SpellId, now: f64) -> f64 {
        let ready_time = self.ready_at.get(&id).copied().unwrap_or(f64::NEG_INFINITY);
        (ready_time - now).max(0.0)
    }

    pub fn cooldown_frac(&self, id: SpellId, now: f64) -> f64 {
        let cd = spell_for_id(id).map(|s| s.cooldown_ms).unwrap_or(0.0);
        if cd > 0.0 {
            (self.cooldown_left(id, now) / cd).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}
