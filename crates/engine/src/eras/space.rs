use super::{BrainInput, Era, EraEvent, Objective, SoundKind};

const MIN_PASSENGERS: u32 = 8;
const COUNT_FROM: f32 = 10.0;
const SCRUB_AFTER: f32 = 5.0;

pub struct SpaceAge {
    pub time: f32,
    pub progress: f32,
    pub passengers: u32,
    pub board_timer: f32,
    pub countdown: f32,
    pub hold_time: f32,
    pub scrubs: u32,
    pub phase: String, // "build", "board", "countdown", "launch"
    pub done: bool,
    pub failed: Option<String>,
}

impl Default for SpaceAge {
    fn default() -> Self {
        Self::new()
    }
}

impl SpaceAge {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            progress: 0.0,
            passengers: 0,
            board_timer: 0.0,
            countdown: COUNT_FROM,
            hold_time: 0.0,
            scrubs: 0,
            phase: "build".to_string(),
            done: false,
            failed: None,
        }
    }
}

impl Era for SpaceAge {
    fn id(&self) -> &str {
        "space"
    }

    fn label(&self) -> &str {
        "Era III · Space Age"
    }

    fn powers(&self) -> &[&str] {
        &["focus", "calm"]
    }

    fn darkness(&self) -> f32 {
        0.0
    }

    fn objectives(&self) -> Vec<Objective> {
        vec![
            Objective {
                label: "Rocket".to_string(),
                value: format!("{}%", (self.progress * 100.0).round() as i32),
                done: self.progress >= 1.0,
                bad: false,
                plain: false,
            },
            Objective {
                label: "Boarding".to_string(),
                value: format!("{}/{}", self.passengers, MIN_PASSENGERS),
                done: self.passengers >= MIN_PASSENGERS,
                bad: false,
                plain: false,
            },
            Objective {
                label: "Countdown".to_string(),
                value: if self.phase == "launch" {
                    "Liftoff!".to_string()
                } else if self.phase == "countdown" {
                    format!("T-{:.1}s", self.countdown)
                } else {
                    "Pending".to_string()
                },
                done: self.phase == "launch",
                bad: self.hold_time > 3.0,
                plain: false,
            },
            Objective {
                label: "Scrubs".to_string(),
                value: format!("{}", self.scrubs),
                done: false,
                bad: self.scrubs >= 2,
                plain: true,
            },
        ]
    }

    fn update(&mut self, dt: f32, brain: &BrainInput) -> Vec<EraEvent> {
        let mut events = Vec::new();
        self.time += dt;

        match self.phase.as_str() {
            "build" => {
                // Focus accelerates rocket construction
                let build_rate = 0.05 + 0.15 * brain.focus;
                self.progress = (self.progress + build_rate * dt).min(1.0);

                // Stress/agitation drops parts (pulse contributes optionally if present)
                let stress = brain.panic.max((1.0 - brain.calm) * 0.8).max(if brain.hr_delta > 0.0 { brain.hr_delta / 25.0 } else { 0.0 });
                if stress > 0.6 && self.progress > 0.05 {
                    self.progress = (self.progress - dt * 0.04).max(0.0);
                }

                if self.progress >= 1.0 {
                    self.phase = "board".to_string();
                    events.push(EraEvent::PlaySound(SoundKind::Good));
                    events.push(EraEvent::AdvisorSay {
                        key: "rocket-ready".to_string(),
                        text: "The rocket is built! Calm your mind so the crew has courage to board."
                            .to_string(),
                        mood: "happy".to_string(),
                        priority: 3,
                        cooldown: 8.0,
                    });
                }
            }
            "board" => {
                // Calm encourages citizens to board
                if brain.calm >= 0.45 {
                    self.board_timer += dt;
                    if self.board_timer >= 0.5 && self.passengers < MIN_PASSENGERS {
                        self.board_timer = 0.0;
                        self.passengers += 1;
                        events.push(EraEvent::PlaySound(SoundKind::Pop));
                    }
                }

                if self.passengers >= MIN_PASSENGERS {
                    self.phase = "countdown".to_string();
                    events.push(EraEvent::PlaySound(SoundKind::Bell));
                    events.push(EraEvent::AdvisorSay {
                        key: "countdown-start".to_string(),
                        text: "All passengers aboard! Hold focus and maintain calm for the final 10s countdown!"
                            .to_string(),
                        mood: "nervous".to_string(),
                        priority: 4,
                        cooldown: 999.0,
                    });
                }
            }
            "countdown" => {
                let focus_ok = brain.focus >= 0.45;
                let steady_ok = brain.calm >= 0.35 && brain.panic <= 0.35 && (brain.hr_delta <= 0.0 || brain.hr_delta <= 14.0);

                if focus_ok && steady_ok {
                    self.countdown -= dt;
                    self.hold_time = 0.0;
                    if (self.countdown % 1.0) < dt {
                        events.push(EraEvent::PlaySound(SoundKind::Tick));
                    }
                    if self.countdown <= 0.0 {
                        self.phase = "launch".to_string();
                        self.done = true;
                        events.push(EraEvent::PlaySound(SoundKind::Rumble));
                        events.push(EraEvent::PlaySound(SoundKind::Fanfare));
                        events.push(EraEvent::AdvisorSay {
                            key: "liftoff".to_string(),
                            text: "WE HAVE LIFTOFF! To the stars and a new world!".to_string(),
                            mood: "happy".to_string(),
                            priority: 5,
                            cooldown: 999.0,
                        });
                    }
                } else {
                    // Hold countdown
                    self.hold_time += dt;
                    if self.hold_time >= SCRUB_AFTER {
                        // Scrub countdown back to 10
                        self.scrubs += 1;
                        self.countdown = COUNT_FROM;
                        self.hold_time = 0.0;
                        events.push(EraEvent::PlaySound(SoundKind::Bad));
                        events.push(EraEvent::AdvisorSay {
                            key: "count-scrub".to_string(),
                            text: "Countdown scrubbed due to stress! Resetting to T-10s.".to_string(),
                            mood: "panic".to_string(),
                            priority: 3,
                            cooldown: 6.0,
                        });
                    }
                }
            }
            _ => {}
        }

        events
    }

    fn on_blink(&mut self, _aim_x: f32, _aim_y: f32) -> Vec<EraEvent> {
        let mut events = Vec::new();
        events.push(EraEvent::PlaySound(SoundKind::Thunder));
        events
    }

    fn on_cast(&mut self, _spell: &str, _aim_x: f32, _aim_y: f32) -> Vec<EraEvent> {
        Vec::new()
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
    fn test_space_age_flow_and_countdown() {
        let mut space = SpaceAge::new();

        // Build with high focus and calm, zero pulse dependency
        let focus_brain = BrainInput {
            focus: 0.9,
            calm: 0.8,
            hr_delta: 0.0,
            ..Default::default()
        };
        while space.phase == "build" {
            space.update(0.1, &focus_brain);
        }
        assert!(space.progress >= 1.0);
        assert_eq!(space.phase, "board");

        // Board citizens with calm
        while space.phase == "board" {
            space.update(0.1, &focus_brain);
        }
        assert!(space.passengers >= MIN_PASSENGERS);
        assert_eq!(space.phase, "countdown");

        // Countdown with focus & low pulse
        while space.phase == "countdown" {
            space.update(0.1, &focus_brain);
        }
        assert_eq!(space.phase, "launch");
        assert!(space.is_complete());
    }
}
