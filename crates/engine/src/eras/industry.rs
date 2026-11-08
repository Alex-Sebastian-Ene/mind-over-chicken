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
    pub strikes: u32,
    pub done: bool,
    pub failed: Option<String>,
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
            strikes: 0,
            done: false,
            failed: None,
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
        &["stress", "calm"]
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
        let chart_momentum = herding * self.momentum * 12.0;

        let return_rate = fundamental_pull + chart_momentum;
        self.momentum = self.momentum * 0.85 + return_rate * 0.15;
        self.price = (self.price * (1.0 + return_rate * dt * 3.0)).clamp(10.0, 500.0);

        // GDP grows when confidence is healthy and factories aren't on strike
        if self.general_strike <= 0.0 {
            let productivity = (self.confidence * 15.0).max(1.0);
            self.gdp += productivity * dt;
        } else {
            self.general_strike -= dt;
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
        events.push(EraEvent::PlaySound(SoundKind::Thunder));
        events.push(EraEvent::StatAddLightning);

        // Factory smokestacks / dynamos around (70.0, 126.0)
        let df_factory = ((aim_x - 70.0).powi(2) + (aim_y - 126.0).powi(2)).sqrt();
        if df_factory < 28.0 {
            self.gdp = (self.gdp + 15.0).min(GDP_TARGET + 50.0);
            events.push(EraEvent::AdvisorSay {
                key: "factory-hit".to_string(),
                text: "Direct strike on the smokestacks! The dynamos are supercharged!".to_string(),
                mood: "happy".to_string(),
                priority: 2,
                cooldown: 15.0,
            });
        }

        // Picket line around (79.0, 128.0)
        let df_picket = ((aim_x - 79.0).powi(2) + (aim_y - 128.0).powi(2)).sqrt();
        if df_picket < 24.0 && self.general_strike > 0.0 {
            self.general_strike = (self.general_strike - 4.0).max(0.0);
            events.push(EraEvent::AdvisorSay {
                key: "picket-struck".to_string(),
                text: "The picket line scattered before the divine thunderbolt! Back to work!".to_string(),
                mood: "shocked".to_string(),
                priority: 2,
                cooldown: 20.0,
            });
        }

        // Goat exchange around (134.0, 90.0)
        let df_exchange = ((aim_x - 134.0).powi(2) + (aim_y - 90.0).powi(2)).sqrt();
        if df_exchange < 22.0 {
            self.price = (self.price * 1.08).clamp(10.0, 500.0);
            self.momentum += 0.02;
            events.push(EraEvent::AdvisorSay {
                key: "exchange-hit".to_string(),
                text: "Lightning struck the exchange bell! Bulls are running wild on the trading floor!".to_string(),
                mood: "happy".to_string(),
                priority: 2,
                cooldown: 15.0,
            });
        }

        events
    }

    fn on_cast(&mut self, spell: &str, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        let mut events = Vec::new();
        match spell {
            "peace" => {
                self.confidence = 1.0;
                self.general_strike = 0.0;
                events.push(EraEvent::PlaySound(SoundKind::Good));
            }
            "fist" | "fireball" | "lightning" => {
                return self.on_blink(aim_x, aim_y);
            }
            _ => {}
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
    fn test_industry_lightning_on_blink_interactions() {
        let mut ind = IndustryEra::new();
        assert_eq!(ind.strikes, 0);

        // Strike factory
        let initial_gdp = ind.gdp;
        let events = ind.on_blink(70.0, 126.0);
        assert_eq!(ind.strikes, 1);
        assert!(events.contains(&EraEvent::StatAddLightning));
        assert!(events.contains(&EraEvent::PlaySound(SoundKind::Thunder)));
        assert!(ind.gdp > initial_gdp);

        // Strike picket line while on strike
        ind.general_strike = 6.0;
        let _ = ind.on_blink(79.0, 128.0);
        assert!(ind.general_strike < 6.0);

        // Strike goat exchange
        let initial_price = ind.price;
        let _ = ind.on_blink(134.0, 90.0);
        assert!(ind.price > initial_price);

        // on_cast with fireball or fist delegates to on_blink
        let _ = ind.on_cast("fireball", 70.0, 126.0);
        assert_eq!(ind.strikes, 4);
    }
}
