use super::industry::IndustryEra;
use super::space::SpaceAge;
use super::stone::StoneAge;
use super::{BrainInput, Era, EraBillboard, EraEvent, EraLight};

#[derive(Clone, Copy, Debug, Default)]
pub struct GameStats {
    pub lightning: u32,
    pub smitten: u32,
    pub births: u32,
    pub deaths: u32,
    pub recessions: u32,
    pub restarts: u32,
    pub runtime: f32,
}

pub struct GameFlow {
    pub era_index: usize,
    pub era: Box<dyn Era>,
    pub stats: GameStats,
    pub finished: bool,
    pub pending_era: Option<usize>,
    pub pending_timer: f32,
}

impl Default for GameFlow {
    fn default() -> Self {
        Self::new(0)
    }
}

impl GameFlow {
    pub fn new(start_era: usize) -> Self {
        let era: Box<dyn Era> = match start_era {
            1 => Box::new(IndustryEra::new()),
            2 => Box::new(SpaceAge::new()),
            _ => Box::new(StoneAge::new()),
        };

        Self {
            era_index: start_era,
            era,
            stats: GameStats::default(),
            finished: false,
            pending_era: None,
            pending_timer: 0.0,
        }
    }

    pub fn start_era(&mut self, index: usize) {
        self.era_index = index;
        self.era = match index {
            1 => Box::new(IndustryEra::new()),
            2 => Box::new(SpaceAge::new()),
            _ => Box::new(StoneAge::new()),
        };
        self.pending_era = None;
        self.pending_timer = 0.0;
    }

    pub fn restart_current_era(&mut self) {
        self.stats.restarts += 1;
        self.start_era(self.era_index);
    }

    pub fn tick(&mut self, dt: f32, brain: &BrainInput) -> Vec<EraEvent> {
        self.stats.runtime += dt;

        if let Some(next_idx) = self.pending_era {
            self.pending_timer -= dt;
            if self.pending_timer <= 0.0 {
                self.start_era(next_idx);
            }
            return Vec::new();
        }

        let events = self.era.update(dt, brain);

        for ev in &events {
            match ev {
                EraEvent::StatAddLightning => self.stats.lightning += 1,
                EraEvent::StatAddSmitten => self.stats.smitten += 1,
                EraEvent::StatAddBirth => self.stats.births += 1,
                EraEvent::StatAddDeath => self.stats.deaths += 1,
                EraEvent::StatAddRecession => self.stats.recessions += 1,
                _ => {}
            }
        }

        if self.era.is_complete() {
            if self.era_index < 2 {
                self.pending_era = Some(self.era_index + 1);
                self.pending_timer = 2.0;
            } else {
                self.finished = true;
            }
        }

        events
    }

    pub fn on_blink(&mut self, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        let events = self.era.on_blink(aim_x, aim_y);
        for ev in &events {
            if let EraEvent::StatAddLightning = ev {
                self.stats.lightning += 1;
            }
        }
        events
    }

    pub fn on_cast(&mut self, spell: &str, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        self.era.on_cast(spell, aim_x, aim_y)
    }

    pub fn billboards_3d(&self) -> Vec<EraBillboard> {
        self.era.billboards_3d()
    }

    pub fn lights_3d(&self) -> Vec<EraLight> {
        self.era.lights_3d()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scripted_full_run_under_3_minutes() {
        let mut game = GameFlow::new(0);
        let dt = 0.1;

        // Simulated focused/calm player (low stress)
        let good_brain = BrainInput {
            calm: 0.85,
            focus: 0.85,
            stress: 0.15,
            ..Default::default()
        };

        // ERA I: Stone Age
        // Strike fire and mammoth
        game.on_blink(150.0, 100.0);
        game.on_blink(60.0, 110.0);
        game.on_blink(110.0, 122.0);

        // Advance until Stone Age completes
        while game.era_index == 0 {
            game.tick(dt, &good_brain);
            if game.stats.runtime > 80.0 {
                panic!("Stone Age took too long");
            }
        }
        assert_eq!(game.era_index, 1);

        // ERA II: Industry
        // Advance until Industry completes
        while game.era_index == 1 {
            game.tick(dt, &good_brain);
            if game.stats.runtime > 140.0 {
                panic!("Industry took too long");
            }
        }
        assert_eq!(game.era_index, 2);

        // ERA III: Space Age
        // Advance until Space Age completes
        while !game.finished {
            game.tick(dt, &good_brain);
            if game.stats.runtime > 180.0 {
                panic!("Full game exceeded 3 minutes limit (180s)");
            }
        }

        assert!(game.finished);
        assert!(
            game.stats.runtime < 180.0,
            "Scripted full run must finish in under 3:00 (took {:.1}s)",
            game.stats.runtime
        );
    }
}
