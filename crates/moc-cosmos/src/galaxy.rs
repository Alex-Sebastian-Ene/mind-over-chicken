//! Galaxy simulation, orbits, and multi-world coordination.
//!
//! Orchestrates the home world and 4 brain-seeded worlds in an orbital star system.
//! Reference implementation matches `src/cosmos/galaxy.js`.

use crate::math::Mulberry32;
use crate::planet_sim::{
    arrive_ship, blink_planet, step_planet, BrainEffects, Channel, EventKind, MindSnapshot,
    PlanetState, SimEvent,
};

pub const MAX_PLANETS: usize = 7;
pub const STAR_X: f32 = 132.0;
pub const STAR_Y: f32 = 92.0;
pub const ORBIT_FIRST: f32 = 34.0;
pub const ORBIT_STEP: f32 = 15.0;
pub const ORBIT_SQUASH: f32 = 0.42;

pub const PLANET_NAMES: [&str; 12] = [
    "Henhaven",
    "Cluckturn",
    "Roostia",
    "Yolkos",
    "Featherfall",
    "Eggsodus",
    "Pecktopia",
    "Gizzardia",
    "Coopernicus",
    "Bok Prime",
    "Omelettia",
    "Wattle IV",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Temperate,
    Desert,
    Ice,
    Jungle,
    Alien,
    Forming,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Temperate => "temperate",
            Biome::Desert => "desert",
            Biome::Ice => "ice",
            Biome::Jungle => "jungle",
            Biome::Alien => "alien",
            Biome::Forming => "forming",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Planet {
    pub id: usize,
    pub name: String,
    pub biome: Biome,
    pub size: f32,
    pub state: PlanetState,
    pub home: bool,
    pub autonomous: bool,
    pub alive: bool,
    pub forming: bool,
    pub mind_slot: bool,
    pub orbit: f32,
    pub phase: f32,
    pub speed: f32,
    pub spin: f32,
}

impl Planet {
    pub fn position(&self, time: f32) -> (f32, f32, bool) {
        let a = self.phase + time * self.speed;
        let x = STAR_X + a.cos() * self.orbit;
        let y = STAR_Y + a.sin() * self.orbit * ORBIT_SQUASH;
        let behind = a.sin() < 0.0;
        (x, y, behind)
    }

    pub fn hit_test(&self, x: f32, y: f32, time: f32) -> bool {
        let (px, py, _) = self.position(time);
        let dx = px - x;
        let dy = py - y;
        (dx * dx + dy * dy).sqrt() < self.size + 5.0
    }
}

#[derive(Debug, Clone)]
pub struct Ship {
    pub from_planet: usize,
    pub to_planet: usize,
    pub t: f32,
    pub duration: f32,
}

#[derive(Debug, Clone)]
pub struct Galaxy {
    pub planets: Vec<Planet>,
    pub ships: Vec<Ship>,
    pub time: f32,
    pub mind: MindSnapshot,
    pub attended: Option<usize>,
    pub log: Vec<(f32, String)>,
    rng: Mulberry32,
}

impl Galaxy {
    pub fn new(seed: u32) -> Self {
        let rng = Mulberry32::new(seed);
        let mut galaxy = Self {
            planets: Vec::new(),
            ships: Vec::new(),
            time: 0.0,
            mind: MindSnapshot::new(),
            attended: None,
            log: Vec::new(),
            rng,
        };

        // Add home planet
        galaxy.add_planet(
            "Home".to_string(),
            Biome::Temperate,
            12.0,
            Channel::Mixed,
            0.0,
            0,
            true,
            false,
            true,
        );

        // Add 3 forming mind worlds that will wake up after the initial 10s mind snapshot
        for i in 0..3 {
            let sizes = [11.0, 10.0, 9.0];
            galaxy.add_planet(
                "Forming…".to_string(),
                Biome::Forming,
                sizes[i],
                Channel::Mixed,
                0.0,
                0,
                false,
                false,
                false,
            );
            if let Some(p) = galaxy.planets.last_mut() {
                p.mind_slot = true;
                p.forming = true;
            }
        }

        galaxy
    }

    fn add_planet(
        &mut self,
        name: String,
        biome: Biome,
        size: f32,
        channel: Channel,
        pop: f32,
        era_index: usize,
        home: bool,
        autonomous: bool,
        alive: bool,
    ) -> usize {
        let idx = self.planets.len();
        let orbit = ORBIT_FIRST + idx as f32 * ORBIT_STEP;
        let phase = self.rng.range_f32(0.0, std::f32::consts::PI * 2.0);
        let speed = 0.2 / (1.0 + idx as f32 * 0.45);
        let spin = self.rng.range_f32(0.25, 0.5);

        let state = PlanetState::new(channel, pop, era_index, &mut self.rng);
        let planet = Planet {
            id: idx,
            name,
            biome,
            size,
            state,
            home,
            autonomous,
            alive,
            forming: false,
            mind_slot: false,
            orbit,
            phase,
            speed,
            spin,
        };
        self.planets.push(planet);
        idx
    }

    pub fn note(&mut self, text: String) {
        self.log.insert(0, (self.time, text));
        if self.log.len() > 8 {
            self.log.pop();
        }
    }

    pub fn step(&mut self, dt: f32, fx: &BrainEffects, blinks_total: u32) -> Vec<(usize, SimEvent)> {
        self.time += dt;
        let mut all_events = Vec::new();

        // 1. Advance the 10-second mind snapshot
        if !self.mind.done {
            if let Some(ranked) = self.mind.update(dt, fx.calm, fx.focus, fx.hr_delta, blinks_total) {
                // Form the 3 mind worlds from the top 3 channels!
                let top_3: Vec<Channel> = ranked.iter().take(3).map(|&(ch, _)| ch).collect();
                let mut mind_slot_indices = Vec::new();
                for (i, p) in self.planets.iter().enumerate() {
                    if p.mind_slot && p.forming {
                        mind_slot_indices.push(i);
                    }
                }

                for (slot_idx, &ch) in top_3.iter().enumerate() {
                    if slot_idx < mind_slot_indices.len() {
                        let p_idx = mind_slot_indices[slot_idx];
                        let p = &mut self.planets[p_idx];
                        let (name, biome) = match ch {
                            Channel::Calm => ("Stillwater", Biome::Temperate),
                            Channel::Focus => ("Lumen", Biome::Alien),
                            Channel::Pulse => ("Thrum", Biome::Desert),
                            Channel::Blink => ("Flicker", Biome::Ice),
                            Channel::Mixed => ("Roostia", Biome::Jungle),
                        };
                        p.name = name.to_string();
                        p.biome = biome;
                        p.state.channel = ch;
                        p.forming = false;
                        p.alive = true;
                        p.state.pop = 10.0;
                        p.state.era_index = 0;
                        p.state.progress = 0.0;

                        let note_msg = format!("{}: formed from your {}.", name, ch.name());
                        self.log.insert(0, (self.time, note_msg));
                    }
                }
            }
        }

        // 2. Advance all planets
        let planet_count = self.planets.len();
        for i in 0..planet_count {
            let attended = self.attended == Some(i);
            let p = &mut self.planets[i];
            if !p.alive || (p.home && !p.autonomous) {
                continue;
            }

            let res = step_planet(&mut p.state, dt, fx, &mut self.rng, attended);
            for ev in res.events {
                if ev.kind == EventKind::LaunchReady {
                    self.launch_ship_from(i);
                }
                all_events.push((i, ev));
            }
        }

        // 3. Update ships travelling through space
        let mut arriving = Vec::new();
        self.ships.retain_mut(|s| {
            s.t += dt;
            if s.t >= s.duration {
                arriving.push(s.to_planet);
                false
            } else {
                true
            }
        });

        for target_idx in arriving {
            if target_idx < self.planets.len() {
                let outcome = arrive_ship(&mut self.planets[target_idx].state);
                let target_name = self.planets[target_idx].name.clone();
                self.note(format!("Ship arrived at {}: {}.", target_name, outcome));
            }
        }

        all_events
    }

    fn launch_ship_from(&mut self, from_idx: usize) {
        // Find best target world: prefer barren world (pop <= 0), then least advanced
        let mut target_idx = None;
        for (i, p) in self.planets.iter().enumerate() {
            if i != from_idx && p.alive && p.state.pop <= 0.0 {
                target_idx = Some(i);
                break;
            }
        }

        if target_idx.is_none() {
            let mut min_era = usize::MAX;
            for (i, p) in self.planets.iter().enumerate() {
                if i != from_idx && p.alive && p.state.era_index < min_era {
                    min_era = p.state.era_index;
                    target_idx = Some(i);
                }
            }
        }

        if let Some(to_idx) = target_idx {
            let from_name = self.planets[from_idx].name.clone();
            let to_name = self.planets[to_idx].name.clone();
            let duration = self.rng.range_f32(5.0, 7.0);

            self.ships.push(Ship {
                from_planet: from_idx,
                to_planet: to_idx,
                t: 0.0,
                duration,
            });
            self.note(format!("{} launched a colony ship to {}.", from_name, to_name));
        }
    }

    pub fn on_blink(&mut self) -> Vec<(usize, SimEvent)> {
        let mut events = Vec::new();
        for (i, p) in self.planets.iter_mut().enumerate() {
            let attended = self.attended == Some(i);
            if !p.alive || (p.home && !p.autonomous) {
                continue;
            }

            if let Some(ev) = blink_planet(&mut p.state, &mut self.rng, attended) {
                events.push((i, ev));
            }
        }
        events
    }
}
