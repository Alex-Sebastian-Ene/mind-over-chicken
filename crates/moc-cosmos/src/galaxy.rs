//! Galaxy simulation, orbits, and multi-world coordination.
//!
//! Orchestrates the home world and 4 brain-seeded worlds in an orbital star system.
//! Reference implementation matches `src/cosmos/galaxy.js`.

use crate::math::Mulberry32;
use crate::planet_sim::{
    arrive_ship, blink_planet, step_planet, BrainEffects, Channel, EventKind, MindSnapshot,
    PlanetState, SimEvent, MIN_POP, POP_CAP, SETTLE_POP,
};
use crate::war::{Mind, Mission, SpaceWar, WorldStats};

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
    /// A war fleet's orders (#166); None for a colony ship.
    pub mission: Option<Mission>,
}

#[derive(Debug, Clone)]
pub struct Galaxy {
    pub planets: Vec<Planet>,
    pub ships: Vec<Ship>,
    pub time: f32,
    pub mind: MindSnapshot,
    pub attended: Option<usize>,
    pub log: Vec<(f32, String)>,
    /// The interplanetary war (#166): fleets, raids, bombardments, invasions.
    pub war: SpaceWar,
    /// Run the war from these planets in `step` (true). The browser's galaxy has more planets
    /// than this one and syncs them itself, so `RustGalaxy` turns this off.
    pub war_auto: bool,
    war_news_seen: u32,
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
            war: SpaceWar::new(seed),
            war_auto: true,
            war_news_seen: 0,
            rng,
        };
        galaxy.war.set_player_home(Some(0));

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

        // 1. Advance sequential 5-second mind readings for world formation (#108)
        if !self.mind.done {
            if let Some(ch) = self.mind.update(dt, fx.calm, fx.focus, fx.stress, blinks_total) {
                // Find next forming planet slot
                if let Some(p) = self.planets.iter_mut().find(|p| p.mind_slot && p.forming) {
                    let (name, biome) = match ch {
                        Channel::Calm => ("Stillwater", Biome::Temperate),
                        Channel::Focus => ("Lumen", Biome::Alien),
                        Channel::Stress => ("Thrum", Biome::Desert),
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

        // 4. The interplanetary war (#166).
        if self.war_auto {
            self.step_war(dt, fx);
        }

        all_events
    }

    /// What the war needs to know about planet `i`.
    pub fn world_stats(&self, i: usize) -> WorldStats {
        let p = &self.planets[i];
        let s = &p.state;
        // This galaxy's planets have no settlement or stores: tier from the people, stores
        // from the goat market.
        let tier = if s.pop >= 1800.0 { 3 } else if s.pop >= 220.0 { 2 } else if s.pop >= 25.0 { 1 } else { 0 };
        let temper = match s.channel {
            Channel::Stress => 0.75,
            Channel::Calm => 0.25,
            Channel::Focus => 0.45,
            Channel::Blink => 0.55,
            Channel::Mixed => 0.5,
        };
        WorldStats {
            alive: p.alive && !p.forming,
            pop: s.pop.max(0.0),
            era: s.era_index as u8,
            tier,
            progress: if s.era_index + 1 >= POP_CAP.len() { 1.0 } else { s.progress },
            stock: s.market.max(0.0) * 10.0,
            stability: 0.7,
            temper,
            hand_played: p.home && !p.autonomous,
        }
    }

    /// Sync the planets into the war, step it, and apply what it did.
    pub fn step_war(&mut self, dt: f32, fx: &BrainEffects) {
        for i in 0..self.planets.len() {
            let stats = self.world_stats(i);
            self.war.sync_body(i, &self.planets[i].name, stats);
        }
        self.war.step(dt, Mind { calm: fx.calm, focus: fx.focus, stress: fx.stress });
        for i in 0..self.planets.len() {
            let w = self.war.take_fx(i);
            if w.is_empty() {
                continue;
            }
            let p = &mut self.planets[i];
            if p.home && !p.autonomous {
                continue; // the hand-played home's scene isn't ours to hurt
            }
            let s = &mut p.state;
            if w.settled && s.pop <= 0.0 {
                s.pop = SETTLE_POP;
                s.era_index = 0;
                s.progress = 0.0;
            }
            if s.pop > 0.0 {
                s.pop = (s.pop * w.pop_mul).max(MIN_POP);
            }
            s.progress = (s.progress - w.progress_loss).max(0.0);
            s.market *= w.stock_mul.max(0.2);
        }
        let news: Vec<String> = self.war.news_since(self.war_news_seen).map(|n| n.text.clone()).collect();
        self.war_news_seen = self.war.next_news;
        for text in news {
            self.note(text);
        }
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
                mission: None,
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
