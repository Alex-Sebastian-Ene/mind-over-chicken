use super::{BrainInput, Era, EraEvent, Objective, SoundKind};

const POP_TARGET: usize = 15;
const CALM_TARGET: u32 = 2;
const HOLD_SECONDS: f32 = 5.0;

#[derive(Clone, Debug)]
pub struct StonePerson {
    pub x: f32,
    pub y: f32,
    pub alive: bool,
    pub state: String,
    pub hunger: f32,
    pub fear: f32,
    pub faith: f32,
    pub facing: i32,
    pub frame: u32,
}

#[derive(Clone, Debug)]
pub struct StoneMammoth {
    pub x: f32,
    pub y: f32,
    pub alive: bool,
    pub facing: i32,
    pub frame: u32,
}

#[derive(Clone, Debug)]
pub struct FirePit {
    pub x: f32,
    pub y: f32,
    pub lit: bool,
    pub ever_lit: bool,
    pub heat: f32,
}

#[derive(Clone, Debug)]
pub struct CalmHold {
    pub active: bool,
    pub progress: f32,
    pub successes: u32,
}

pub struct StoneAge {
    pub time: f32,
    pub fire: FirePit,
    pub hold: CalmHold,
    pub food: f32,
    pub people: Vec<StonePerson>,
    pub mammoths: Vec<StoneMammoth>,
    pub colonies: u32,
    pub strikes: u32,
    pub done: bool,
    pub failed: Option<String>,
}

impl Default for StoneAge {
    fn default() -> Self {
        Self::new()
    }
}

impl StoneAge {
    pub fn new() -> Self {
        let mut people = Vec::with_capacity(10);
        for i in 0..10 {
            people.push(StonePerson {
                x: 130.0 + (i as f32 % 5.0) * 8.0,
                y: 85.0 + (i as f32 / 5.0) * 8.0,
                alive: true,
                state: "idle".to_string(),
                hunger: 0.2,
                fear: 0.4,
                faith: 0.1,
                facing: 1,
                frame: 0,
            });
        }

        let mut mammoths = Vec::with_capacity(4);
        for i in 0..4 {
            mammoths.push(StoneMammoth {
                x: 60.0 + (i as f32) * 50.0,
                y: 110.0 + (i as f32) * 12.0,
                alive: true,
                facing: 1,
                frame: 0,
            });
        }

        Self {
            time: 0.0,
            fire: FirePit {
                x: 150.0,
                y: 100.0,
                lit: false,
                ever_lit: false,
                heat: 0.0,
            },
            hold: CalmHold {
                active: false,
                progress: 0.0,
                successes: 0,
            },
            food: 1.0,
            people,
            mammoths,
            colonies: 1,
            strikes: 0,
            done: false,
            failed: None,
        }
    }

    pub fn living_count(&self) -> usize {
        self.people.iter().filter(|p| p.alive).count()
    }
}

impl Era for StoneAge {
    fn id(&self) -> &str {
        "stone"
    }

    fn label(&self) -> &str {
        "Era I · Stone Age"
    }

    fn powers(&self) -> &[&str] {
        &["blink", "calm"]
    }

    fn darkness(&self) -> f32 {
        if self.fire.lit {
            0.55
        } else {
            0.75
        }
    }

    fn objectives(&self) -> Vec<Objective> {
        let living = self.living_count();
        vec![
            Objective {
                label: "Fire".to_string(),
                value: if self.fire.lit {
                    "lit".to_string()
                } else {
                    "out".to_string()
                },
                done: self.fire.lit,
                bad: !self.fire.lit && self.fire.ever_lit,
                plain: false,
            },
            Objective {
                label: "Tribe".to_string(),
                value: format!("{}/{}", living, POP_TARGET),
                done: living >= POP_TARGET,
                bad: living <= 3,
                plain: false,
            },
            Objective {
                label: "Calm holds".to_string(),
                value: format!("{}/{}", self.hold.successes, CALM_TARGET),
                done: self.hold.successes >= CALM_TARGET,
                bad: false,
                plain: false,
            },
            Objective {
                label: "Food".to_string(),
                value: format!("{}", self.food.floor() as i32),
                done: false,
                bad: false,
                plain: true,
            },
            Objective {
                label: "Continents".to_string(),
                value: format!("{}/4", self.colonies),
                done: self.colonies >= 4,
                bad: false,
                plain: true,
            },
        ]
    }

    fn update(&mut self, dt: f32, brain: &BrainInput) -> Vec<EraEvent> {
        let mut events = Vec::new();
        self.time += dt;

        // Fire heat decays if lit
        if self.fire.lit {
            let decay_rate = if self.hold.active { 0.025 } else { 0.04 };
            self.fire.heat = (self.fire.heat - dt * decay_rate).max(0.05);

            // Trigger calm hold when fire gets low
            if self.fire.heat < 0.55 && !self.hold.active && self.hold.successes < CALM_TARGET {
                self.hold.active = true;
                self.hold.progress = 0.0;
                events.push(EraEvent::AdvisorSay {
                    key: "hold-fire".to_string(),
                    text: "The flame fades! Hold calm for 5 seconds to stoke it.".to_string(),
                    mood: "nervous".to_string(),
                    priority: 2,
                    cooldown: 8.0,
                });
            }
        }

        // Calm hold processing
        if self.hold.active {
            if brain.calm >= 0.5 {
                self.hold.progress += dt;
                if self.hold.progress >= HOLD_SECONDS {
                    self.hold.active = false;
                    self.hold.successes += 1;
                    self.fire.heat = 0.85;
                    events.push(EraEvent::PlaySound(SoundKind::Good));
                    events.push(EraEvent::AdvisorSay {
                        key: "calm-success".to_string(),
                        text: "Your serenity has fed the flame! The tribe rejoices.".to_string(),
                        mood: "happy".to_string(),
                        priority: 3,
                        cooldown: 0.0,
                    });
                }
            } else {
                self.hold.progress = (self.hold.progress - dt * 1.5).max(0.0);
            }
        }

        // People behaviors & growth
        let living = self.living_count();
        if self.food >= 2.0 && living < POP_TARGET {
            self.food -= 1.0;
            self.people.push(StonePerson {
                x: self.fire.x + 10.0,
                y: self.fire.y + 4.0,
                alive: true,
                state: "idle".to_string(),
                hunger: 0.2,
                fear: 0.1,
                faith: 0.4,
                facing: 1,
                frame: 0,
            });
            events.push(EraEvent::PlaySound(SoundKind::Pop));
            events.push(EraEvent::StatAddBirth);
        }

        // Expeditions to continents
        if self.fire.lit && self.food >= 3.0 && living >= 12 && self.colonies < 4 {
            self.colonies += 1;
            self.food -= 1.5;
            events.push(EraEvent::PlaySound(SoundKind::Good));
            events.push(EraEvent::AdvisorSay {
                key: "new-colony".to_string(),
                text: "Settlers sailed across the straits! A new colony is founded.".to_string(),
                mood: "happy".to_string(),
                priority: 3,
                cooldown: 5.0,
            });
        }

        // Win condition
        if !self.done
            && living >= POP_TARGET
            && self.hold.successes >= CALM_TARGET
            && self.fire.lit
        {
            self.done = true;
            events.push(EraEvent::PlaySound(SoundKind::Fanfare));
            events.push(EraEvent::AdvisorSay {
                key: "win-stone".to_string(),
                text: "The tribe is warm, fed, and thriving. Era complete!".to_string(),
                mood: "happy".to_string(),
                priority: 4,
                cooldown: 999.0,
            });
        }

        // Lose condition
        if !self.done && living <= 2 {
            self.done = true;
            let reason = if !self.fire.ever_lit {
                "The tribe froze in the dark: blink directly on the fire pit to light it."
            } else if self.food <= 0.0 {
                "The tribe starved: blink to strike mammoths and feed your people."
            } else if !self.fire.lit {
                "The fire went out: stay calm when the flame fades to kindle it."
            } else {
                "The tribe has perished: keep the fire lit and hunt mammoths for dinner."
            };
            self.failed = Some(reason.to_string());
            events.push(EraEvent::PlaySound(SoundKind::Bad));
        }

        events
    }

    fn on_blink(&mut self, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        let mut events = Vec::new();
        self.strikes += 1;
        events.push(EraEvent::PlaySound(SoundKind::Thunder));
        events.push(EraEvent::StatAddLightning);

        // Check fire strike (either fire pit or label area)
        let df = ((aim_x - self.fire.x).powi(2) + (aim_y - self.fire.y).powi(2)).sqrt();
        let df_label = ((aim_x - self.fire.x).powi(2) + (aim_y - (self.fire.y - 12.0)).powi(2)).sqrt();
        if df < 18.0 || df_label < 14.0 {
            let first = !self.fire.ever_lit;
            self.fire.lit = true;
            self.fire.ever_lit = true;
            self.fire.heat = self.fire.heat.max(0.8);
            events.push(EraEvent::PlaySound(SoundKind::Ignite));
            events.push(EraEvent::AdvisorSay {
                key: "fire-lit".to_string(),
                text: if first {
                    "FIRE! You have invented fire! Please do not invent it on Grug.".to_string()
                } else {
                    "The flame lives again.".to_string()
                },
                mood: "happy".to_string(),
                priority: 3,
                cooldown: 4.0,
            });
        }

        // Check mammoth strike (generous hit testing)
        for m in &mut self.mammoths {
            if m.alive {
                let dm = ((aim_x - m.x).powi(2) + (aim_y - m.y).powi(2)).sqrt();
                if dm < 18.0 {
                    m.alive = false;
                    self.food += 5.0;
                    events.push(EraEvent::PlaySound(SoundKind::Coin));
                    events.push(EraEvent::AdvisorSay {
                        key: "cooked-mammoth".to_string(),
                        text: "Cooked mammoth! Barbecue time for the tribe.".to_string(),
                        mood: "happy".to_string(),
                        priority: 2,
                        cooldown: 5.0,
                    });
                    break;
                }
            }
        }

        // Direct lightning strike kills person
        for p in &mut self.people {
            if p.alive {
                let dp = ((aim_x - p.x).powi(2) + (aim_y - p.y).powi(2)).sqrt();
                if dp < 9.0 {
                    p.alive = false;
                    events.push(EraEvent::StatAddSmitten);
                    events.push(EraEvent::AdvisorSay {
                        key: "smitten-person".to_string(),
                        text: "A direct lightning strike! You smote a villager. The tribe trembles in fear.".to_string(),
                        mood: "nervous".to_string(),
                        priority: 3,
                        cooldown: 3.0,
                    });
                    break;
                }
            }
        }

        events
    }

    fn on_cast(&mut self, spell: &str, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        match spell {
            "fireball" => self.on_blink(aim_x, aim_y),
            "spawn" => {
                self.food += 2.0;
                let mut events = Vec::new();
                events.push(EraEvent::PlaySound(SoundKind::Coin));
                events
            }
            "bless" => {
                for p in &mut self.people {
                    p.faith = (p.faith + 0.3).min(1.0);
                    p.fear = (p.fear - 0.1).max(0.0);
                }
                let mut events = Vec::new();
                events.push(EraEvent::PlaySound(SoundKind::Good));
                events.push(EraEvent::StatAddSmitten);
                events
            }
            _ => Vec::new(),
        }
    }

    fn is_complete(&self) -> bool {
        self.done && self.failed.is_none()
    }

    fn is_failed(&self) -> Option<String> {
        self.failed.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stone_age_lightning_and_calm() {
        let mut stone = StoneAge::new();
        assert!(!stone.fire.lit);

        // Strike fire
        let events = stone.on_blink(150.0, 100.0);
        assert!(stone.fire.lit);
        assert!(events.contains(&EraEvent::PlaySound(SoundKind::Ignite)));

        // Strike mammoth
        let m_x = stone.mammoths[0].x;
        let m_y = stone.mammoths[0].y;
        stone.on_blink(m_x, m_y);
        assert!(!stone.mammoths[0].alive);
        assert!(stone.food >= 4.0);

        // Simulate calm hold
        stone.hold.active = true;
        let brain = BrainInput {
            calm: 0.8,
            ..Default::default()
        };
        for _ in 0..60 {
            stone.update(0.1, &brain);
        }
        assert!(stone.hold.successes >= 1);
    }

    #[test]
    fn test_stone_age_label_strike_and_failure_explanation() {
        let mut stone = StoneAge::new();
        assert!(!stone.fire.lit);

        // Strike 12px above the fire pit (where the tutorial text is drawn)
        let label_y = stone.fire.y - 12.0;
        let events = stone.on_blink(stone.fire.x, label_y);
        assert!(stone.fire.lit, "Blinking the tutorial label must ignite the fire");
        assert!(events.contains(&EraEvent::PlaySound(SoundKind::Ignite)));

        // Test specific failure explanation when tribe perishes without food
        stone.food = 0.0;
        stone.people.retain(|_| false); // All perish
        let brain = BrainInput::default();
        stone.update(0.1, &brain);
        assert!(stone.is_failed().is_some());
        let reason = stone.is_failed().unwrap();
        assert!(reason.contains("starved"), "Failure must explain starvation: {}", reason);
    }

    #[test]
    fn test_stone_age_lightning_kills_person() {
        let mut stone = StoneAge::new();
        assert!(stone.people[0].alive);
        let px = stone.people[0].x;
        let py = stone.people[0].y;
        let events = stone.on_blink(px, py);
        assert!(!stone.people[0].alive, "Direct lightning strike must smite/kill villager");
        assert!(events.contains(&EraEvent::StatAddSmitten));
    }
}
