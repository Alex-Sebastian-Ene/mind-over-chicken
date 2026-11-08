use super::{BrainInput, Era, EraBillboard, EraEvent, EraLight, Objective, SoundKind, STRESS_LIMIT};

const GDP_TARGET: f32 = 620.0;
const IPO_HOLD_SECONDS: f32 = 8.0;

pub struct IndustryEra {
    pub time: f32,
    pub price: f32,
    pub value: f32,
    pub momentum: f32,
    pub gdp: f32,
    pub gdp_reached: bool,
    pub confidence: f32,
    pub recessions: u32,
    pub crash_cooldown: f32,
    pub general_strike: f32,
    pub ipo_progress: f32,
    pub ipo_done: bool,
    pub done: bool,
    pub failed: Option<String>,
    pub strikes: u32,
}

impl Default for IndustryEra {
    fn default() -> Self {
        Self::new()
    }
}

impl IndustryEra {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            price: 100.0,
            value: 100.0,
            momentum: 0.0,
            gdp: 50.0,
            gdp_reached: false,
            confidence: 1.0,
            recessions: 0,
            crash_cooldown: 0.0,
            general_strike: 0.0,
            ipo_progress: 0.0,
            ipo_done: false,
            done: false,
            failed: None,
            strikes: 0,
        }
    }
}

impl Era for IndustryEra {
    fn id(&self) -> &str {
        "industry"
    }

    fn label(&self) -> &str {
        "Era II · Industry"
    }

    fn powers(&self) -> &[&str] {
        // Blink strikes the factory, the picket line or the exchange (on_blink), so its tile is on (#178).
        &["blink", "stress", "calm"]
    }

    fn darkness(&self) -> f32 {
        0.15
    }

    fn objectives(&self) -> Vec<Objective> {
        let gdp_done = self.gdp >= GDP_TARGET || self.gdp_reached;
        let display_gdp = if gdp_done {
            GDP_TARGET as i32
        } else {
            self.gdp.round() as i32
        };
        let ipo_seconds = (IPO_HOLD_SECONDS - self.ipo_progress).max(0.0).ceil() as i32;
        vec![
            Objective {
                label: "GDP".to_string(),
                value: format!("{}/{}", display_gdp, GDP_TARGET as i32),
                done: gdp_done,
                bad: false,
                plain: false,
            },
            Objective {
                label: "Goat IPO".to_string(),
                value: if self.ipo_done {
                    "priced".to_string()
                } else if gdp_done {
                    format!("{}/{}s", (IPO_HOLD_SECONDS as i32) - ipo_seconds, IPO_HOLD_SECONDS as i32)
                } else {
                    "soon".to_string()
                },
                done: self.ipo_done,
                bad: false,
                plain: false,
            },
            Objective {
                label: "Goat index".to_string(),
                value: format!("{:.1}", self.price),
                done: false,
                bad: self.price < 50.0,
                plain: true,
            },
            Objective {
                label: "Striking".to_string(),
                value: format!("{}", if self.general_strike > 0.0 { 1 } else { 0 }),
                done: false,
                bad: self.recessions >= 3,
                plain: true,
            },
        ]
    }

    fn update(&mut self, dt: f32, brain: &BrainInput) -> Vec<EraEvent> {
        let mut events = Vec::new();
        if self.done {
            return events;
        }
        if self.price < 12.0 {
            self.done = true;
            self.failed = Some("The Great Goat Depression. A goat is now worth less than its hat.".to_string());
            events.push(EraEvent::PlaySound(SoundKind::Crash));
            return events;
        }
        if self.general_strike > 8.0 {
            self.done = true;
            self.failed = Some("General strike. The economy has stopped. The goats have unionised.".to_string());
            events.push(EraEvent::PlaySound(SoundKind::Bad));
            return events;
        }
        self.time += dt;

        if self.crash_cooldown > 0.0 {
            self.crash_cooldown -= dt;
        }

        // Arousal: the god's stress, as in src/eras/industry.js.
        let arousal = brain.stress.clamp(0.0, 1.0);
        let herding = 1.0 + 1.6 * arousal;

        // Fundamental economy value grows with GDP
        self.value = 100.0 * (1.0 + (self.gdp / GDP_TARGET) * 0.7);

        // Market price dynamics: fundamental pull + chartist momentum + stress herding
        let fundamental_pull = 0.4 * (self.value / self.price).ln();
        let chart_momentum = herding * self.momentum * 0.8;

        let return_rate = fundamental_pull + chart_momentum;
        self.momentum = (self.momentum * 0.85 + return_rate * 0.15).clamp(-0.5, 0.5);
        self.price = (self.price * (1.0 + return_rate * dt)).clamp(5.0, 500.0);

        // Failure condition 1: The Great Goat Depression (price drops below 12.0)
        if self.price < 12.0 {
            self.done = true;
            self.failed = Some("The Great Goat Depression. A goat is now worth less than its hat.".to_string());
            events.push(EraEvent::PlaySound(SoundKind::Crash));
            return events;
        }

        // GDP grows when confidence is healthy and factories aren't on strike
        if self.general_strike <= 0.0 {
            let productivity = (self.confidence * 15.0).max(1.0);
            self.gdp += productivity * dt;
        } else {
            if arousal > 0.6 {
                self.general_strike += dt;
            } else if brain.calm < 0.4 {
                self.general_strike += dt * 0.2;
            } else {
                self.general_strike -= dt;
            }
        }

        // Failure condition 2: General strike lasts too long (> 8 seconds)
        if self.general_strike > 8.0 {
            self.done = true;
            self.failed = Some("General strike. The economy has stopped. The goats have unionised.".to_string());
            events.push(EraEvent::PlaySound(SoundKind::Bad));
            return events;
        }
        if self.gdp >= GDP_TARGET {
            self.gdp_reached = true;
        }

        // Emergent crash condition: high stress + rapid price plunge
        let is_plunging = self.momentum < -0.05 || (self.price < self.value * 0.6);
        if arousal > 0.6 && is_plunging && self.crash_cooldown <= 0.0 {
            self.recessions += 1;
            self.crash_cooldown = 12.0;
            self.confidence = (self.confidence - 0.4).max(0.1);
            self.general_strike = 6.0;
            events.push(EraEvent::PlaySound(SoundKind::Crash));
            events.push(EraEvent::StatAddRecession);
            events.push(EraEvent::AdvisorSay {
                key: "market-crash".to_string(),
                text: "MARKET CRASH! The goats are stampeding! Calm down, my lord!".to_string(),
                mood: "panic".to_string(),
                priority: 3,
                cooldown: 8.0,
            });
        }

        // Calm brings recovery: restores confidence, stops strikes, pulls price to fundamental
        if brain.calm >= 0.6 {
            self.confidence = (self.confidence + dt * 0.3).min(1.0);
            self.general_strike = (self.general_strike - dt * 2.0).max(0.0);
            self.momentum *= 0.5;
        }

        // IPO hold: steady nerves (stress at or below STRESS_LIMIT) complete the IPO
        if self.gdp >= GDP_TARGET && !self.ipo_done {
            let steady = brain.stress <= STRESS_LIMIT;
            if steady {
                self.ipo_progress += dt;
                if self.ipo_progress >= IPO_HOLD_SECONDS {
                    self.ipo_done = true;
                    self.done = true;
                    events.push(EraEvent::PlaySound(SoundKind::Fanfare));
                    events.push(EraEvent::AdvisorSay {
                        key: "ipo-success".to_string(),
                        text: "THE GOAT IPO HAS LANDED! Wealth for all! Onward to the Space Age!"
                            .to_string(),
                        mood: "happy".to_string(),
                        priority: 4,
                        cooldown: 999.0,
                    });
                }
            } else {
                self.ipo_progress = (self.ipo_progress - dt * 1.0).max(0.0);
            }
        }

        events
    }

    fn on_blink(&mut self, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        let mut events = Vec::new();
        self.strikes += 1;
        events.push(EraEvent::StatAddLightning);
        events.push(EraEvent::PlaySound(SoundKind::Thunder));

        // 1. Strike Factory (turbines / production): divine boost
        let df = ((aim_x - 76.0).powi(2) + (aim_y - 130.0).powi(2))
            .sqrt()
            .min(((aim_x - 112.0).powi(2) + (aim_y - 92.0).powi(2)).sqrt());
        if df < 24.0 {
            self.gdp += 15.0;
            events.push(EraEvent::PlaySound(SoundKind::Good));
            events.push(EraEvent::AdvisorSay {
                key: "factory-shock".to_string(),
                text: "Divine lightning struck the factory turbines! Production surged.".to_string(),
                mood: "happy".to_string(),
                priority: 2,
                cooldown: 4.0,
            });
        }

        // 2. Strike Picket Line / Strikers: breaks strike and restores worker confidence
        let dp = ((aim_x - 80.0).powi(2) + (aim_y - 128.0).powi(2)).sqrt();
        if dp < 22.0 && self.general_strike > 0.0 {
            self.general_strike = (self.general_strike - 0.5).max(0.0);
            self.confidence = (self.confidence + 0.15).min(1.0);
            events.push(EraEvent::PlaySound(SoundKind::Good));
            events.push(EraEvent::AdvisorSay {
                key: "strike-break".to_string(),
                text: "Lightning scattered the picket line! The workers are scurrying back to work."
                    .to_string(),
                mood: "happy".to_string(),
                priority: 2,
                cooldown: 4.0,
            });
        }

        // 3. Strike Goat Exchange / Trading Floor: market volatility spike
        let de = ((aim_x - 130.0).powi(2) + (aim_y - 95.0).powi(2)).sqrt();
        if de < 24.0 {
            self.momentum += 0.15;
            events.push(EraEvent::PlaySound(SoundKind::Coin));
            events.push(EraEvent::AdvisorSay {
                key: "exchange-shock".to_string(),
                text: "Lightning struck the Goat Exchange! Traders are scrambling in volatile excitement."
                    .to_string(),
                mood: "panic".to_string(),
                priority: 2,
                cooldown: 4.0,
            });
        }

        events
    }

    fn on_cast(&mut self, spell: &str, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        let mut events = Vec::new();
        if spell == "fireball" {
            events.extend(self.on_blink(aim_x, aim_y));
        } else if spell == "peace" {
            self.confidence = 1.0;
            self.general_strike = 0.0;
            events.push(EraEvent::PlaySound(SoundKind::Good));
        }
        events
    }

    fn is_complete(&self) -> bool {
        self.done && self.failed.is_none()
    }

    fn is_failed(&self) -> Option<String> {
        self.failed.clone()
    }

    fn billboards_3d(&self) -> Vec<EraBillboard> {
        let mut list = Vec::new();

        // 1. Goats: sprite 2 (grazing around exchange pastures)
        let goat_coords = [
            (145.0, 84.0),
            (152.0, 92.0),
            (138.0, 96.0),
            (160.0, 80.0),
        ];
        for (i, &(gx, gz)) in goat_coords.iter().enumerate() {
            let bob = ((self.time * 2.0 + i as f32) as u32) % 2;
            list.push(EraBillboard {
                sprite_id: 2,
                x: gx,
                z: gz,
                facing: if i % 2 == 0 { 1 } else { -1 },
                frame: bob,
                variant: 0,
            });
        }

        // 2. Workers, traders, strikers: sprite 0
        let worker_coords = [
            (76.0, 130.0, 0),  // factory picket
            (82.0, 126.0, 0),
            (130.0, 95.0, 1),  // exchange floor
            (136.0, 88.0, 1),
            (142.0, 102.0, 1),
        ];
        for (i, &(wx, wz, role)) in worker_coords.iter().enumerate() {
            let pose = if self.general_strike > 0.0 && role == 0 {
                1 // strike / protest
            } else if self.recessions > 0 && self.crash_cooldown > 0.0 {
                2 // panic
            } else {
                0 // walk
            };
            list.push(EraBillboard {
                sprite_id: 0,
                x: wx,
                z: wz,
                facing: if i % 2 == 0 { 1 } else { -1 },
                frame: (((self.time * 4.0) as u32) + i as u32) % 2,
                variant: pose,
            });
        }

        // 3. Chicken: sprite 3 (mascot of the exchange)
        list.push(EraBillboard {
            sprite_id: 3,
            x: 132.0,
            z: 88.0,
            facing: 1,
            frame: ((self.time * 4.0) as u32) % 2,
            variant: 0,
        });

        list
    }

    fn lights_3d(&self) -> Vec<EraLight> {
        let mut list = Vec::new();
        // Factory furnace light
        let smog_flicker = (self.time * 5.0).sin() * 2.0;
        list.push(EraLight {
            x: 70.0,
            y: 8.0,
            z: 126.0,
            radius: 30.0 + smog_flicker,
            intensity: 1.4,
        });
        // Exchange market lantern
        list.push(EraLight {
            x: 134.0,
            y: 8.0,
            z: 90.0,
            radius: 22.0,
            intensity: 1.2,
        });
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_industry_emergent_crash_and_calm_recovery() {
        let mut ind = IndustryEra::new();
        assert_eq!(ind.recessions, 0);

        // Normal market operations
        let calm_brain = BrainInput {
            calm: 0.8,
            stress: 0.2,
            ..Default::default()
        };
        for _ in 0..30 {
            ind.update(0.1, &calm_brain);
        }
        assert!(ind.gdp > 50.0);
        assert_eq!(ind.recessions, 0);

        // Stress triggers an emergent crash
        let panic_brain = BrainInput {
            calm: 0.0,
            stress: 1.0,
            ..Default::default()
        };
        // Introduce downward momentum
        ind.momentum = -0.08;
        let mut crashed = false;
        for _ in 0..20 {
            let events = ind.update(0.1, &panic_brain);
            if events.contains(&EraEvent::StatAddRecession) {
                crashed = true;
                break;
            }
        }
        assert!(crashed, "Stress must trigger an emergent market crash");
        assert!(ind.recessions >= 1);
        assert!(ind.general_strike > 0.0);

        // Serene calm brings recovery
        for _ in 0..50 {
            ind.update(0.1, &calm_brain);
        }
        assert_eq!(ind.general_strike, 0.0);
        assert!(ind.confidence >= 0.8);
    }

    #[test]
    fn test_industry_depression_failure() {
        let mut ind = IndustryEra::new();
        let brain = BrainInput::default();
        ind.price = 11.5;
        let events = ind.update(0.1, &brain);
        assert!(ind.is_failed().is_some());
        assert_eq!(
            ind.is_failed().unwrap(),
            "The Great Goat Depression. A goat is now worth less than its hat."
        );
        assert!(!ind.is_complete());
        assert!(events.contains(&EraEvent::PlaySound(SoundKind::Crash)));
    }

    #[test]
    fn test_industry_general_strike_failure() {
        let mut ind = IndustryEra::new();
        let panic_brain = BrainInput {
            calm: 0.0,
            stress: 1.0,
            ..Default::default()
        };
        ind.general_strike = 8.1;
        let events = ind.update(0.1, &panic_brain);
        assert!(ind.is_failed().is_some());
        assert_eq!(
            ind.is_failed().unwrap(),
            "General strike. The economy has stopped. The goats have unionised."
        );
        assert!(!ind.is_complete());
        assert!(events.contains(&EraEvent::PlaySound(SoundKind::Bad)));
    }

    #[test]
    fn test_industry_lightning_strike_interactions() {
        let mut ind = IndustryEra::new();
        assert_eq!(ind.strikes, 0);

        // 1. Strike random empty space
        let events = ind.on_blink(0.0, 0.0);
        assert_eq!(ind.strikes, 1);
        assert!(events.contains(&EraEvent::StatAddLightning));
        assert!(events.contains(&EraEvent::PlaySound(SoundKind::Thunder)));

        // 2. Strike factory site -> boosts GDP
        let initial_gdp = ind.gdp;
        let factory_events = ind.on_blink(76.0, 130.0);
        assert_eq!(ind.strikes, 2);
        assert!(ind.gdp > initial_gdp);
        assert!(factory_events.contains(&EraEvent::PlaySound(SoundKind::Good)));

        // 3. Strike picket line while on strike -> reduces strike
        ind.general_strike = 4.0;
        let picket_events = ind.on_blink(80.0, 128.0);
        assert_eq!(ind.strikes, 3);
        assert!(ind.general_strike < 4.0);
        assert!(picket_events.contains(&EraEvent::PlaySound(SoundKind::Good)));

        // 4. Fireball cast triggers lightning strike
        let cast_events = ind.on_cast("fireball", 130.0, 95.0);
        assert_eq!(ind.strikes, 4);
        assert!(cast_events.contains(&EraEvent::StatAddLightning));
    }
}
