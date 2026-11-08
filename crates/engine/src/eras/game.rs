use super::industry::IndustryEra;
use super::space::SpaceAge;
use super::stone::StoneAge;
use super::{BrainInput, Era, EraBillboard, EraEvent, EraLight, Objective};

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
    /// The JS scene's objectives while the browser game mirrors it (#191), else None.
    ///
    /// In the browser the JS scenes (src/eras/*.js) are what the player sees and plays; these
    /// Rust eras ran beside them on the old fixed 320x180 map (a fire pit at (116, 100), prey
    /// marching x 45..270 over water). The HUD showed "Fire out" next to a lit fire, the Rust
    /// advisor answered blinks on invisible prey, and completion and failure read the wrong
    /// state. While mirrored the era stands still: no update, no events from blinks or casts,
    /// no completion or failure of its own, and the HUD shows these objectives and the JS clock.
    pub mirrored: Option<Vec<Objective>>,
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
            mirrored: None,
        }
    }

    /// Show the JS scene's objectives and clock (#191); the Rust era stands still until unmirror().
    pub fn mirror(&mut self, run_time: f32, objectives: Vec<Objective>) {
        self.stats.runtime = run_time;
        self.mirrored = Some(objectives);
    }

    pub fn unmirror(&mut self) {
        self.mirrored = None;
    }

    /// What the HUD lists: the mirrored JS objectives, or the Rust era's own.
    pub fn objectives(&self) -> Vec<Objective> {
        match &self.mirrored {
            Some(o) => o.clone(),
            None => self.era.objectives(),
        }
    }

    pub fn era_complete(&self) -> bool {
        self.mirrored.is_none() && self.era.is_complete()
    }

    pub fn era_failed(&self) -> Option<String> {
        if self.mirrored.is_some() {
            None
        } else {
            self.era.is_failed()
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
        if self.mirrored.is_some() {
            return Vec::new();
        }
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
        if self.mirrored.is_some() {
            return Vec::new();
        }
        let events = self.era.on_blink(aim_x, aim_y);
        for ev in &events {
            if let EraEvent::StatAddLightning = ev {
                self.stats.lightning += 1;
            }
        }
        events
    }

    pub fn on_cast(&mut self, spell: &str, aim_x: f32, aim_y: f32) -> Vec<EraEvent> {
        if self.mirrored.is_some() {
            return Vec::new();
        }
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

    fn row(label: &str, value: &str, done: bool) -> Objective {
        Objective {
            label: label.into(),
            value: value.into(),
            done,
            bad: false,
            plain: false,
        }
    }

    #[test]
    fn mirrored_hud_shows_the_js_scene_and_the_rust_era_stands_still() {
        let mut game = GameFlow::new(0);
        assert_eq!(game.objectives()[0].value, "out");
        // The JS Stone Age on the 640x360 world: fire lit, far from the Rust pit at (116, 100).
        let js = vec![
            row("Fire", "lit", true),
            row("Tribe", "8/15", false),
            row("Continents", "1/5", false),
        ];
        game.mirror(12.5, js.clone());
        assert_eq!(game.objectives(), js);
        assert_eq!(game.stats.runtime, 12.5);

        let calm = BrainInput {
            calm: 1.0,
            ..Default::default()
        };
        // A blink on the old Rust pit, a cast and a minute of ticks: nothing happens, nothing is said.
        assert!(game.on_blink(116.0, 100.0).is_empty());
        assert!(game.on_cast("fireball", 116.0, 100.0).is_empty());
        for _ in 0..600 {
            assert!(game.tick(0.1, &calm).is_empty());
        }
        assert_eq!(game.stats.runtime, 12.5, "the clock is the JS run time");
        assert_eq!(
            game.era.objectives()[0].value,
            "out",
            "the Rust fire was not lit behind the scenes"
        );
        assert!(!game.era_complete());
        assert_eq!(game.era_failed(), None);
        assert_eq!(game.era_index, 0);

        // Unmirrored (the all-Rust app), the era plays as before.
        game.unmirror();
        assert!(!game.on_blink(116.0, 100.0).is_empty());
        assert_eq!(game.objectives()[0].value, "lit");
    }

    #[test]
    fn objective_rows_decode_the_flags() {
        let o = super::super::objectives_from_rows(
            vec!["Fire".into(), "Food".into(), "Tribe".into()],
            vec!["lit".into(), "3".into(), "2/15".into()],
            &[1, 4, 2],
        );
        assert_eq!((o[0].done, o[0].bad, o[0].plain), (true, false, false));
        assert_eq!((o[1].done, o[1].bad, o[1].plain), (false, false, true));
        assert_eq!((o[2].done, o[2].bad, o[2].plain), (false, true, false));
        assert_eq!(o[2].value, "2/15");
    }

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
