//! The training environment for the war councils (#100), in Rust: worlds of `n_civs`
//! civilisations on a `w`×`h` planet, stepped at full speed, with some civilisations driven by
//! the learner (actions handed in), the rest by a league (past council networks run here, or
//! the rules). `examples/civ_gym.rs` speaks it as newline-delimited JSON to the Python trainer;
//! `examples/civ_eval.rs` and the tests use it directly.
//!
//! **Council mode:** `learners` civilisations per world each fill one slot; a step hands in one
//! MultiDiscrete action per slot and runs `COUNCIL_DT` (4 s) of the world. A slot whose
//! civilisation dies ends (terminated) and takes over another living civilisation not yet
//! driven by the learner, if there is one; otherwise it idles (zero observation, only the
//! first option legal, no reward) until the world ends. Worlds run `max_seconds` after a
//! warm-up under the rules, then all slots end (truncated) and a new world begins.
//!
//! **Commander mode:** one slot per world: the army of a civilisation at war, one action per
//! second while its war lasts. Between wars the world runs on until somebody fights.
//!
//! The player's mind is part of the world: by default it wanders between neutral, calm,
//! stressed and angry spells (60–240 s each), so the policy learns to read it.
//!
//! Rewards: [`council_reward`] and [`commander_reward`].

use std::cell::RefCell;
use std::rc::Rc;

use moc_planet::rng::Rng;

use crate::commander::{self, CmdBrain};
use crate::council::{self, Brain, Council, COUNCIL_DT, N_LOGITS, OBS_SIZE};
use crate::jobs::{Building, Job};
use crate::planet::Dims;
use crate::terrain::{Terrain, FOREST, GRASS};
use crate::world::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Council,
    Commander,
}

#[derive(Clone, Copy, Debug)]
pub enum Mood {
    /// Spells of neutral, calm, stress and anger.
    Wander,
    Fixed(Inputs),
}

pub fn mood_named(name: &str) -> Mood {
    match name {
        "calm" => Mood::Fixed(Inputs { calm: 0.85, focus: 0.2, stress: 0.0, anger: 0.0, joy: 0.0 }),
        "stress" => Mood::Fixed(Inputs { calm: 0.05, focus: 0.1, stress: 0.75, anger: 0.2, joy: 0.0 }),
        "anger" => Mood::Fixed(Inputs { calm: 0.05, focus: 0.1, stress: 0.5, anger: 0.8, joy: 0.0 }),
        "neutral" => Mood::Fixed(Inputs { calm: 0.2, focus: 0.15, stress: 0.0, anger: 0.0, joy: 0.0 }),
        _ => Mood::Wander,
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub seed: u32,
    pub n_civs: usize,
    pub w: usize,
    pub h: usize,
    pub mode: Mode,
    /// Learner slots per world (council mode).
    pub learners: usize,
    /// Seconds under the rules before the councils take over (drawn from this range).
    pub warmup: (f32, f32),
    /// Seconds of council play per world.
    pub max_seconds: f32,
    pub mood: Mood,
    /// League: chance an opponent keeps the rules (the rest get a random loaded network,
    /// the newest one half of the time).
    pub p_rules: f32,
    /// Training seeds for the worlds after the first: drawn from [lo, hi).
    pub seeds: (u32, u32),
}

impl Default for Config {
    fn default() -> Config {
        Config { seed: 1, n_civs: 2, w: 640, h: 320, mode: Mode::Council, learners: 1, warmup: (150.0, 450.0), max_seconds: 900.0, mood: Mood::Wander, p_rules: 0.35, seeds: (0, 500_000) }
    }
}

/// What a civilisation had last step (for rewards).
#[derive(Clone, Copy, Debug, Default)]
pub struct Snap {
    pub pop: u32,
    pub owned: u32,
    pub kills: u32,
    pub losses: u32,
}

pub fn snap(w: &World, c: u8) -> Snap {
    let f = &w.factions[c as usize];
    Snap { pop: f.pop, owned: w.owned_tiles[c as usize], kills: f.kills, losses: f.losses }
}

/// Events of one step, per civilisation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tally {
    pub captured: u32,
    pub lost: u32,
    pub battles_won: u32,
    pub battles_lost: u32,
    pub age_ups: u32,
    pub collapses: u32,
    pub wiped: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SlotInfo {
    pub civ: i32,
    pub ret: f32,
    pub pop: u32,
    pub villages: u32,
    pub age: u8,
    pub alive: bool,
    pub idle: bool,
}

#[derive(Clone, Debug, Default)]
struct Slot {
    civ: u8,
    prev: Snap,
    ret: f32,
    len: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Step {
    pub obs: Vec<Vec<f32>>,
    pub masks: Vec<Vec<bool>>,
    pub rewards: Vec<f32>,
    pub dones: Vec<bool>,
    /// Ended by the clock (true) or by the civilisation's death / the war's end (false).
    pub truncated: Vec<bool>,
    /// For ended slots: the last observation of the episode that ended.
    pub terminal_obs: Vec<Option<Vec<f32>>>,
    pub infos: Vec<SlotInfo>,
    /// Finished episodes this step: (return, length, slot info).
    pub episodes: Vec<(f32, u32, SlotInfo)>,
}

pub struct Gym {
    pub cfg: Config,
    pub world: World,
    pub council: Rc<RefCell<Council>>,
    /// Council networks (MOCP bytes) loaded into every new world: [0] newest.
    pub league: Vec<Vec<u8>>,
    pub cmd_league: Vec<Vec<u8>>,
    slots: Vec<Slot>,
    rng: Rng,
    end_t: f32,
    mood_t: f32,
    tally: [Tally; MAX_FACTIONS],
    pub worlds: u32,
}

/// A world of `n` civilisations on a `d` planet: the first four on the four home continents
/// (as `World::populate` does), more on free land of the same landmass, 38+ tiles apart.
pub fn new_world(seed: u32, n: usize, d: Dims) -> World {
    let mut w = World::with_terrain(seed, Terrain::generate(seed, d));
    let mut run = Rng::new(seed ^ 0x2545_f491);
    let mut starts: Vec<(i32, i32)> = Vec::new();
    let home: Vec<(i32, i32)> = w.terrain.regions.iter().take(4).map(|r| r.start).collect();
    let lands: Vec<u16> = home.iter().map(|&(x, y)| w.terrain.land(x, y)).collect();
    for i in 0..n.min(MAX_FACTIONS) {
        let s = if i < home.len() {
            Some(home[i])
        } else {
            let mut found = None;
            for t in 0..6000 {
                let spacing = if t < 3000 { 38.0 } else { 24.0 };
                let x = run.below(w.d.w) as i32;
                let y = (w.d.h as f32 * run.range(0.2, 0.8)) as i32;
                let k = w.terrain.kind_at(x, y);
                if !(k == GRASS || k == FOREST) || !w.terrain.walkable(x, y) || !lands.contains(&w.terrain.land(x, y)) {
                    continue;
                }
                if starts.iter().all(|&(sx, sy)| w.d.dist(sx as f32, sy as f32, x as f32, y as f32) >= spacing) {
                    found = Some((x, y));
                    break;
                }
            }
            found
        };
        let Some(start) = s else { break };
        starts.push(start);
        let fs = run.next_u32();
        let f = w.add_faction(fs, SeedSource::Run, Traits::from_seed(fs), NO_F, None);
        w.factions[f as usize].region = (i % 4) as u8;
        w.factions[f as usize].start = start;
        w.factions[f as usize].is_player = false;
        let v = w.found_village(f, start.0, start.1, 6.0, false);
        w.factions[f as usize].capital = v;
        w.villages[v as usize].built |= 1u64 << Building::Campfire as u8;
        for k in 0..START_AGENTS {
            let (ox, oy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][k % 4];
            let (x, y) = if w.terrain.walkable(start.0 + ox, start.1 + oy) { (start.0 + ox, start.1 + oy) } else { start };
            let job = if k % 2 == 0 { Job::Forage } else { Job::Hunt };
            if let Some(a) = w.spawn_agent(f, v, job, x, y) {
                let age = w.rng.range(0.0, 20.0);
                w.agents[a].age = age;
                w.agents[a].clan = (k / 2) as u8;
            }
        }
    }
    w.recount();
    w.refresh_info();
    w
}

/// Run the world `secs` seconds of sim time (steps of 0.1 s).
pub fn run(w: &mut World, secs: f32) {
    let mut left = secs;
    while left > 1e-4 {
        let chunk = left.min(2.0);
        w.speed = chunk / 0.1;
        w.advance(0.1);
        left -= chunk;
    }
    w.speed = 1.0;
}

/// Tally the events since the last call (and clear them).
pub fn tally_events(w: &mut World, t: &mut [Tally; MAX_FACTIONS]) {
    for e in w.events.drain(..) {
        let (a, b) = (e.a, e.b);
        let ok = |x: i32| x >= 0 && (x as usize) < MAX_FACTIONS;
        match e.kind {
            ev::CAPTURE if ok(a) && ok(b) && a != b => {
                t[a as usize].captured += 1;
                t[b as usize].lost += 1;
            }
            ev::RAZED if ok(a) && ok(b) && a != b => {
                t[a as usize].lost += 1;
            }
            ev::BATTLE if ok(a) && ok(b) => {
                t[a as usize].battles_won += 1;
                t[b as usize].battles_lost += 1;
            }
            ev::AGE_UP if ok(a) => t[a as usize].age_ups += 1,
            ev::COLLAPSE if ok(a) => t[a as usize].collapses += 1,
            ev::WIPED if ok(a) => t[a as usize].wiped = true,
            _ => {}
        }
    }
}

/// The council's reward for one step: survival, growth (people, land, ages), minus deaths,
/// collapses and lost villages; land weighs more for aggressive civilisations, trade and
/// allies more for friendly ones; battles count a little.
pub fn council_reward(w: &World, c: &Council, civ: u8, prev: &Snap, t: &Tally) -> f32 {
    let f = &w.factions[civ as usize];
    if !f.alive || t.wiped {
        return -3.0;
    }
    let tr = f.traits;
    let w_land = 0.3 + 0.7 * tr.aggressiveness;
    let w_trade = 0.3 + 0.7 * tr.friendliness;
    let ln = |x: u32| (x as f32).ln_1p();
    let partners = (0..MAX_FACTIONS as u8).filter(|&b| b != civ && w.factions[b as usize].alive && w.rel(civ, b).traded_t < COUNCIL_DT).count() as f32;
    let allies = (0..MAX_FACTIONS as u8).filter(|&b| c.allied(civ, b)).count() as f32;
    let deaths = f.losses.saturating_sub(prev.losses) as f32;
    0.02 + 1.0 * (ln(f.pop) - ln(prev.pop))
        + w_land * 0.5 * (ln(w.owned_tiles[civ as usize]) - ln(prev.owned))
        + w_land * 0.3 * t.captured as f32
        - 0.4 * t.lost as f32
        + 0.6 * t.age_ups as f32
        - 1.0 * t.collapses as f32
        - 0.004 * deaths
        + w_trade * 0.005 * (partners + allies)
        + 0.15 * t.battles_won as f32
        - 0.15 * t.battles_lost as f32
}

/// The commander's reward for one second: villages taken and lost, fighters killed and lost,
/// battles won and lost (sparse), and −2 if the civilisation falls.
pub fn commander_reward(w: &World, civ: u8, prev: &Snap, t: &Tally) -> f32 {
    let f = &w.factions[civ as usize];
    if !f.alive || t.wiped {
        return -2.0;
    }
    let kills = f.kills.saturating_sub(prev.kills) as f32;
    let losses = f.losses.saturating_sub(prev.losses) as f32;
    1.0 * t.captured as f32 - 1.0 * t.lost as f32 + 0.03 * kills - 0.03 * losses + 0.5 * t.battles_won as f32 - 0.5 * t.battles_lost as f32
}

impl Gym {
    pub fn new(cfg: Config) -> Gym {
        let world = new_world(cfg.seed, cfg.n_civs, Dims::new(cfg.w, cfg.h));
        let council = Rc::new(RefCell::new(Council::new(cfg.seed)));
        let rng = Rng::new(cfg.seed ^ 0x6a09_e667);
        let mut g = Gym { cfg, world, council, league: Vec::new(), cmd_league: Vec::new(), slots: Vec::new(), rng, end_t: 0.0, mood_t: 0.0, tally: [Tally::default(); MAX_FACTIONS], worlds: 0 };
        g.begin(None);
        g
    }

    pub fn n_slots(&self) -> usize {
        match self.cfg.mode {
            Mode::Council => self.cfg.learners.max(1),
            Mode::Commander => 1,
        }
    }

    fn obs_size(&self) -> usize {
        match self.cfg.mode {
            Mode::Council => OBS_SIZE,
            Mode::Commander => commander::OBS_SIZE,
        }
    }

    fn n_logits(&self) -> usize {
        match self.cfg.mode {
            Mode::Council => N_LOGITS,
            Mode::Commander => commander::N_LOGITS,
        }
    }

    /// Start a new world (`seed`, or one drawn from the training range), warm it up and assign
    /// the slots.
    pub fn begin(&mut self, seed: Option<u32>) {
        let seed = seed.unwrap_or_else(|| self.cfg.seeds.0 + (self.rng.next_u32() % (self.cfg.seeds.1 - self.cfg.seeds.0).max(1)));
        self.cfg.seed = seed;
        self.world = new_world(seed, self.cfg.n_civs, Dims::new(self.cfg.w, self.cfg.h));
        self.worlds += 1;
        let mut c = Council::new(seed);
        c.spare_player = false;
        // The league samples its policies: more varied opponents than their greedy play.
        c.greedy = false;
        for b in &self.league {
            let _ = c.load_net(b);
        }
        for b in &self.cmd_league {
            let _ = c.load_cmd_net(b);
        }
        // Newcomers (rebels) follow the newest network if there is one, else the rules.
        c.new_brain = if c.nets.is_empty() { Brain::Rules } else { Brain::Net(0) };
        c.new_cmd = if c.cmd_nets.is_empty() { CmdBrain::Scripted } else { CmdBrain::Net(0) };
        self.council = council::install(&mut self.world, c);
        // Warm-up under the rules (the councils don't exist yet).
        {
            let mut c = self.council.borrow_mut();
            c.new_brain = Brain::Rules;
            c.new_cmd = CmdBrain::Rules;
        }
        let warm = self.rng.range(self.cfg.warmup.0, self.cfg.warmup.1.max(self.cfg.warmup.0));
        self.mood_t = 0.0;
        self.step_mood(warm);
        run(&mut self.world, warm);
        self.world.events.clear();
        self.end_t = self.world.time + self.cfg.max_seconds;
        // Hand over: the league for everyone, then the learner's slots.
        {
            let mut c = self.council.borrow_mut();
            c.sync(&self.world);
            let n_nets = c.nets.len();
            let n_cmd = c.cmd_nets.len();
            for f in 0..MAX_FACTIONS as u8 {
                if !self.world.factions[f as usize].alive {
                    continue;
                }
                let brain = if n_nets == 0 || self.rng.chance(self.cfg.p_rules) {
                    Brain::Rules
                } else if self.rng.chance(0.5) {
                    Brain::Net(0)
                } else {
                    Brain::Net(self.rng.below(n_nets) as u8)
                };
                c.set_brain(f, brain);
                let cmd = if n_cmd > 0 { CmdBrain::Net(0) } else if brain == Brain::Rules && self.cfg.mode == Mode::Council { CmdBrain::Rules } else { CmdBrain::Scripted };
                c.set_cmd_brain(f, cmd);
            }
            c.new_brain = if n_nets == 0 { Brain::Rules } else { Brain::Net(0) };
            c.new_cmd = if n_cmd == 0 { CmdBrain::Scripted } else { CmdBrain::Net(0) };
            if self.cfg.mode == Mode::Commander {
                // Every council follows the newest council network (or the rules).
                for f in 0..MAX_FACTIONS as u8 {
                    if self.world.factions[f as usize].alive {
                        c.set_brain(f, if n_nets > 0 { Brain::Net(0) } else { Brain::Rules });
                        c.set_cmd_brain(f, if n_cmd > 0 && self.rng.chance(0.5) { CmdBrain::Net(self.rng.below(n_cmd) as u8) } else { CmdBrain::Scripted });
                    }
                }
            }
        }
        self.tally = [Tally::default(); MAX_FACTIONS];
        self.slots = vec![Slot { civ: NO_F, ..Slot::default() }; self.n_slots()];
        match self.cfg.mode {
            Mode::Council => {
                let mut alive: Vec<u8> = self.world.alive_factions().map(|f| f.id).collect();
                for k in 0..self.slots.len() {
                    if alive.is_empty() {
                        break;
                    }
                    let i = self.rng.below(alive.len());
                    let civ = alive.swap_remove(i);
                    self.adopt(k, civ);
                }
            }
            Mode::Commander => {
                self.find_war();
            }
        }
    }

    fn adopt(&mut self, k: usize, civ: u8) {
        let mut c = self.council.borrow_mut();
        match self.cfg.mode {
            Mode::Council => {
                c.set_brain(civ, Brain::External);
                if c.cmd_brain(civ) == CmdBrain::Rules {
                    let cmd = if c.cmd_nets.is_empty() { CmdBrain::Scripted } else { CmdBrain::Net(0) };
                    c.set_cmd_brain(civ, cmd);
                }
            }
            Mode::Commander => c.set_cmd_brain(civ, CmdBrain::External),
        }
        drop(c);
        self.slots[k] = Slot { civ, prev: snap(&self.world, civ), ret: 0.0, len: 0 };
    }

    /// The player's mind: spells of neutral, calm, stress and anger (or a fixed one).
    fn step_mood(&mut self, dt: f32) {
        match self.cfg.mood {
            Mood::Fixed(i) => self.world.inputs = i,
            Mood::Wander => {
                self.mood_t -= dt;
                if self.mood_t <= 0.0 {
                    self.mood_t = self.rng.range(60.0, 240.0);
                    let names = ["neutral", "neutral", "calm", "stress", "anger"];
                    if let Mood::Fixed(i) = mood_named(names[self.rng.below(names.len())]) {
                        self.world.inputs = i;
                    }
                }
            }
        }
    }

    fn is_learner(&self, civ: u8) -> bool {
        self.slots.iter().any(|s| s.civ == civ)
    }

    /// Commander mode: run on until some civilisation is at war (up to 300 s, else a new
    /// world), and put the slot on its army.
    fn find_war(&mut self) {
        let mut waited = 0.0;
        loop {
            let at_war: Vec<u8> = (0..MAX_FACTIONS as u8).filter(|&f| self.world.factions[f as usize].alive && (0..MAX_FACTIONS).any(|b| self.world.rel[f as usize * MAX_FACTIONS + b].war == 2)).collect();
            if !at_war.is_empty() {
                let civ = at_war[self.rng.below(at_war.len())];
                self.adopt(0, civ);
                return;
            }
            if waited > 300.0 || self.world.time >= self.end_t {
                self.begin(None);
                return;
            }
            self.step_mood(1.0);
            run(&mut self.world, 1.0);
            self.world.events.clear();
            waited += 1.0;
        }
    }

    /// Observation and mask of slot `k`.
    pub fn observe(&self, k: usize) -> (Vec<f32>, Vec<bool>) {
        let civ = self.slots[k].civ;
        let mut idle_mask = vec![false; self.n_logits()];
        let heads: &[usize] = match self.cfg.mode {
            Mode::Council => &council::HEADS,
            Mode::Commander => &commander::HEADS,
        };
        let mut at = 0;
        for &n in heads {
            idle_mask[at] = true;
            at += n;
        }
        if civ == NO_F || !self.world.factions[civ as usize].alive {
            return (vec![0.0; self.obs_size()], idle_mask);
        }
        let c = self.council.borrow();
        match self.cfg.mode {
            Mode::Council => council::observe_civ(&self.world, &c, civ),
            Mode::Commander => {
                let goal = c.orders(civ).goal;
                let t = commander::targets(&self.world, &c.agg, civ, goal, &c);
                let mut o = vec![0.0; commander::OBS_SIZE];
                let mut m = vec![true; commander::N_LOGITS];
                commander::observe(&self.world, &c.agg, civ, &t, goal, c.cmd_orders(civ), &mut o);
                commander::masks(&t, &mut m);
                (o, m)
            }
        }
    }

    pub fn observe_all(&mut self) -> (Vec<Vec<f32>>, Vec<Vec<bool>>) {
        // Between steps the neighbour grid still indexes agents removed at the end of the last
        // step: rebuild it before counting fighters.
        self.world.grid_rebuild();
        self.council.borrow_mut().refresh(&self.world);
        (0..self.slots.len()).map(|k| self.observe(k)).unzip()
    }

    pub fn info(&self, k: usize) -> SlotInfo {
        let s = &self.slots[k];
        if s.civ == NO_F {
            return SlotInfo { civ: -1, ret: s.ret, idle: true, ..SlotInfo::default() };
        }
        let f = &self.world.factions[s.civ as usize];
        SlotInfo { civ: s.civ as i32, ret: s.ret, pop: f.pop, villages: f.villages, age: f.age, alive: f.alive, idle: false }
    }

    /// One step: hand in the actions (one per slot), run the world, score, observe.
    pub fn step(&mut self, actions: &[Vec<u8>]) -> Step {
        let n = self.slots.len();
        {
            let mut c = self.council.borrow_mut();
            for (k, s) in self.slots.iter().enumerate() {
                if s.civ == NO_F || !self.world.factions[s.civ as usize].alive {
                    continue;
                }
                let a = actions.get(k).cloned().unwrap_or_default();
                match self.cfg.mode {
                    Mode::Council => c.set_action(s.civ, std::array::from_fn(|i| a.get(i).copied().unwrap_or(0))),
                    Mode::Commander => c.set_cmd_action(s.civ, std::array::from_fn(|i| a.get(i).copied().unwrap_or(0))),
                }
            }
        }
        let dt = if self.cfg.mode == Mode::Council { COUNCIL_DT } else { 1.0 };
        self.step_mood(dt);
        run(&mut self.world, dt);
        self.tally = [Tally::default(); MAX_FACTIONS];
        tally_events(&mut self.world, &mut self.tally);
        self.world.grid_rebuild();
        self.council.borrow_mut().refresh(&self.world);
        let mut out = Step { rewards: vec![0.0; n], dones: vec![false; n], truncated: vec![false; n], terminal_obs: vec![None; n], ..Step::default() };
        let clock = self.world.time >= self.end_t;
        let mut ended: Vec<usize> = Vec::new();
        for k in 0..n {
            let civ = self.slots[k].civ;
            if civ == NO_F {
                continue;
            }
            let t = self.tally[civ as usize];
            let r = {
                let c = self.council.borrow();
                match self.cfg.mode {
                    Mode::Council => council_reward(&self.world, &c, civ, &self.slots[k].prev, &t),
                    Mode::Commander => commander_reward(&self.world, civ, &self.slots[k].prev, &t),
                }
            };
            out.rewards[k] = r;
            self.slots[k].ret += r;
            self.slots[k].len += 1;
            self.slots[k].prev = snap(&self.world, civ);
            let dead = !self.world.factions[civ as usize].alive;
            let war_over = self.cfg.mode == Mode::Commander && !(0..MAX_FACTIONS).any(|b| self.world.rel[civ as usize * MAX_FACTIONS + b].war == 2 || self.world.rel[b * MAX_FACTIONS + civ as usize].war == 2);
            if dead || war_over || clock {
                out.dones[k] = true;
                out.truncated[k] = !dead && !war_over;
                ended.push(k);
            }
        }
        for &k in &ended {
            out.terminal_obs[k] = Some(self.observe(k).0);
            let info = self.info(k);
            out.episodes.push((self.slots[k].ret, self.slots[k].len, info));
        }
        if clock {
            self.begin(None);
        } else {
            for &k in &ended {
                let civ = self.slots[k].civ;
                match self.cfg.mode {
                    Mode::Council => {
                        // Take over a living civilisation the learner doesn't drive yet.
                        let free: Vec<u8> = self.world.alive_factions().map(|f| f.id).filter(|&f| !self.is_learner(f)).collect();
                        if free.is_empty() {
                            self.slots[k] = Slot { civ: NO_F, ..Slot::default() };
                        } else {
                            let civ = free[self.rng.below(free.len())];
                            self.adopt(k, civ);
                        }
                    }
                    Mode::Commander => {
                        if civ != NO_F && self.world.factions[civ as usize].alive {
                            self.council.borrow_mut().set_cmd_brain(civ, CmdBrain::Scripted);
                        }
                        self.slots[k] = Slot { civ: NO_F, ..Slot::default() };
                        self.find_war();
                    }
                }
            }
        }
        let (obs, masks) = self.observe_all();
        out.obs = obs;
        out.masks = masks;
        out.infos = (0..n).map(|k| self.info(k)).collect();
        out
    }

    /// Fresh world (after `league` changes, or at the trainer's request).
    pub fn reset(&mut self, seed: Option<u32>) -> (Vec<Vec<f32>>, Vec<Vec<bool>>) {
        self.begin(seed);
        self.observe_all()
    }
}
