//! Camera casts debounce and cooldown controller (#33, #108).
//! Pure logic port of src/camera/casts.js.

use std::collections::HashMap;
use super::spells::{spell_for_gesture, spell_for_id, SpellId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastUpdate {
    pub held: Option<SpellId>,
    pub progress: f64,
    pub cast: Option<SpellId>,
    pub aiming: bool,
}

#[derive(Debug, Clone)]
pub struct CastController {
    pub hold_ms: f64,
    pub min_score: f64,
    pub aim_score: f64,
    pub grace_ms: f64,
    pub ready_at: HashMap<SpellId, f64>,
    pub held: Option<SpellId>,
    pub since: f64,
    pub last_seen: f64,
    pub latched: bool,
}

impl Default for CastController {
    fn default() -> Self {
        Self::new(300.0, 0.55, 0.45, 150.0)
    }
}

impl CastController {
    pub fn new(hold_ms: f64, min_score: f64, aim_score: f64, grace_ms: f64) -> Self {
        Self {
            hold_ms,
            min_score,
            aim_score,
            grace_ms,
            ready_at: HashMap::new(),
            held: None,
            since: 0.0,
            last_seen: 0.0,
            latched: false,
        }
    }

    pub fn reset(&mut self) {
        self.held = None;
        self.since = 0.0;
        self.last_seen = 0.0;
        self.latched = false;
    }

    pub fn spell_for(&self, gesture: &str) -> Option<SpellId> {
        spell_for_gesture(gesture).map(|s| s.id)
    }

    pub fn update(&mut self, gesture: &str, score: f64, now: f64) -> CastUpdate {
        let id = if score >= self.min_score {
            self.spell_for(gesture)
        } else {
            None
        };
        let aiming = gesture == "Pointing_Up" && score >= self.aim_score;

        if let Some(spell_id) = id {
            if self.held != Some(spell_id) {
                self.held = Some(spell_id);
                self.since = now;
                self.latched = false;
            }
            self.last_seen = now;
        } else if self.held.is_some() && now - self.last_seen > self.grace_ms {
            self.reset();
        }

        let held_for = if self.held.is_some() { now - self.since } else { 0.0 };
        let mut cast = None;

        if let Some(spell_id) = self.held {
            if !self.latched && held_for >= self.hold_ms && self.ready(spell_id, now) {
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
