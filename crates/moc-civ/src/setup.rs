//! Staggered starts: each civilisation is chosen, placed and seeded from the player's mind
//! before it is founded (#98).
//!
//! A staged world (`World::new_staged`) starts with an empty planet and four civilisation slots
//! waiting, each with a name, a colour and a seeded homeland. The flow is a small state
//! machine ([`Phase`]):
//!
//! 1. **Choose** a waiting civilisation: click near its homeland, Tab to the next, or Enter
//!    for the one offered.
//! 2. **Place** it: aim and click where it begins (Enter keeps its homeland). Sea, mountains,
//!    ice, another people's land and anywhere within `MIN_START_GAP` tiles of a founded people
//!    are refused, with the reason.
//! 3. **Seed** it: for `SEED_SECONDS` of real time the player's calm, focus, stress, camera
//!    mood (anger, joy) and blinks are averaged into its five traits, which fill in live (the
//!    calibration formula of #54, with stress in place of pulse; a small seeded variation keeps
//!    two civilisations seeded by an idle player apart).
//! 4. **Found** it: its first village and four people appear and it starts living. Civilisations
//!    already founded keep running while the next is set up, so the starts are staggered.
//!
//! **Rivals** (#101): a staged world can also hold rival civilisations the player doesn't set
//! up. They arrive on their own at staggered times, with their seeded traits, at sites spread
//! over the planet's big landmasses (several to a continent), and play by the rules of
//! `politics::decide` (or a `Policy`). Start sites come from whatever `Terrain::generate`
//! made: its regions' starts for the player's slots, then the biggest landmasses in proportion
//! to their area for the rivals, at least `MIN_SITE_GAP` tiles apart.
//!
//! Until it is founded a civilisation is not alive: it takes no part in contact, diplomacy,
//! metrics, ages or anything else. Founding order, times and traits go in the event log
//! (`ev::CIV_FOUNDED`: n = order, extra = the five traits, 6 bits each).

use std::fmt::Write;

use moc_planet::rng::Rng;

use crate::terrain::{Terrain, DEEP, ICE, MOUNTAIN, WATER};
use crate::world::*;

/// Seconds of real time a civilisation is seeded from the player's mind.
pub const SEED_SECONDS: f32 = 5.0;
/// A new civilisation must begin at least this far from any founded people's village.
pub const MIN_START_GAP: f32 = 40.0;
/// Civilisations the player sets up.
pub const SLOTS: usize = 4;
/// Start sites are at least this far apart.
pub const MIN_SITE_GAP: f32 = 60.0;
/// The first rival arrives after this long, the rest every `RIVAL_EVERY` (±50 %) after it.
pub const RIVAL_FIRST: f32 = 15.0;
pub const RIVAL_EVERY: f32 = 30.0;
/// A rival arriving late may begin this close to someone's village.
pub const RIVAL_GAP: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    /// Not a staged world (every civilisation was founded at the start).
    Off = 0,
    Choose = 1,
    Place = 2,
    Seed = 3,
    /// Every civilisation is founded.
    Done = 4,
}

#[derive(Clone, Debug)]
pub struct Setup {
    pub phase: Phase,
    /// The civilisation (faction id) being set up.
    pub slot: u8,
    /// Where it would begin, and whether it may.
    pub spot: (i32, i32),
    pub spot_ok: bool,
    /// Why not (empty when it may).
    pub reason: String,
    /// Seconds of seeding so far.
    pub t: f32,
    /// Calm, focus, stress, anger, joy, summed over the seeding seconds.
    pub sum: [f32; 5],
    pub blinks: u32,
    /// The traits as they fill in.
    pub traits: Traits,
    /// The civilisations the player sets up, in slot order.
    pub slots: Vec<u8>,
    /// (civilisation, world time) in founding order.
    pub founded: Vec<(u8, f32)>,
    /// Rival civilisations and the world time each arrives (#101).
    pub rivals: Vec<(u8, f32)>,
}

impl Default for Setup {
    fn default() -> Setup {
        Setup {
            phase: Phase::Off,
            slot: NO_F,
            spot: (0, 0),
            spot_ok: false,
            reason: String::new(),
            t: 0.0,
            sum: [0.0; 5],
            blinks: 0,
            traits: Traits::default(),
            slots: Vec::new(),
            founded: Vec::new(),
            rivals: Vec::new(),
        }
    }
}

/// The five traits from a seeding window: the calibration formula of #54 (stress in place of
/// pulse) for the README's three, focus for fertility, steadiness for cohesion; plus up to
/// ±0.08 of the civilisation's own seeded variation.
pub fn mind_traits(calm: f32, focus: f32, stress: f32, anger: f32, joy: f32, blinks: u32, secs: f32, seed: u32) -> Traits {
    let c = |v: f32| v.clamp(0.0, 1.0);
    let (calm, focus, stress, anger, joy) = (c(calm), c(focus), c(stress), c(anger), c(joy));
    // About one blink a second is a lot.
    let wild = c(blinks as f32 / secs.max(1.0));
    let mut r = Rng::new(seed ^ 0x5eed_0098);
    let mut j = || r.range(-0.08, 0.08);
    Traits {
        friendliness: 0.55 * calm + 0.35 * joy + 0.1 * (1.0 - anger) + j(),
        aggressiveness: 0.5 * stress + 0.35 * anger + 0.15 * focus + j(),
        violence: 0.45 * anger + 0.35 * stress * (1.0 - calm) + 0.2 * wild + j(),
        fertility: 0.3 + 0.6 * focus + j(),
        cohesion: 0.8 - 0.4 * wild - 0.2 * stress + 0.1 * calm + j(),
    }
    .clamped()
}

fn word(v: f32) -> String {
    format!("{v:.2}")
}

/// "friendly 0.62 · fierce 0.31 · violent 0.20 · fertile 0.55 · cohesive 0.70"
pub fn traits_line(t: &Traits) -> String {
    format!(
        "friendly {} · fierce {} · violent {} · fertile {} · cohesive {}",
        word(t.friendliness),
        word(t.aggressiveness),
        word(t.violence),
        word(t.fertility),
        word(t.cohesion)
    )
}

/// Traits packed 6 bits each (friendliness lowest) for the event log.
pub fn pack_traits(t: &Traits) -> i32 {
    let q = |v: f32| ((v.clamp(0.0, 1.0) * 63.0).round() as i32) & 63;
    q(t.friendliness) | q(t.aggressiveness) << 6 | q(t.violence) << 12 | q(t.fertility) << 18 | q(t.cohesion) << 24
}

pub fn unpack_traits(p: i32) -> Traits {
    let g = |s: i32| ((p >> s) & 63) as f32 / 63.0;
    Traits { friendliness: g(0), aggressiveness: g(6), violence: g(12), fertility: g(18), cohesion: g(24) }
}

impl World {
    /// A run whose civilisations are founded one at a time by the player (#98): the planet is
    /// generated with four civilisation slots waiting and nobody living on it yet.
    pub fn new_staged(seed: u32, d: crate::planet::Dims) -> World {
        World::new_staged_with(seed, d, SLOTS, 0)
    }

    /// A staged run with `players` civilisations for the player to found and `rivals` that
    /// arrive on their own (#101: about 8–12 in all).
    pub fn new_staged_with(seed: u32, d: crate::planet::Dims, players: usize, rivals: usize) -> World {
        let mut w = World::with_terrain(seed, Terrain::generate(seed, d));
        let total = (players + rivals).min(MAX_FACTIONS - 4);
        let sites = w.start_sites(total);
        w.prepare_slots_at(players.min(total), &sites);
        w.prepare_rivals(&sites[players.min(total)..]);
        w
    }

    /// Start sites for `n` civilisations on whatever planet this is: the terrain's regions'
    /// starts first, then the biggest landmasses in proportion to their area (so a big continent
    /// holds several peoples), each site at least `MIN_SITE_GAP` from the others, on grass or
    /// forest where it can.
    pub fn start_sites(&self, n: usize) -> Vec<(i32, i32)> {
        let d = self.d;
        let t = &self.terrain;
        let gap = |a: (i32, i32), b: (i32, i32)| d.dist(a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32);
        let mut sites: Vec<(i32, i32)> = Vec::new();
        for r in &t.regions {
            if sites.len() >= n.min(SLOTS) {
                break;
            }
            if t.walkable(r.start.0, r.start.1) && sites.iter().all(|&s| gap(s, r.start) >= MIN_SITE_GAP) {
                sites.push(r.start);
            }
        }
        if sites.len() >= n {
            return sites;
        }
        // Good tiles on the big landmasses (sampled), then farthest-point picks: the sites
        // spread evenly over the land, so a big continent holds several peoples.
        let mut count = std::collections::BTreeMap::<u16, u32>::new();
        for &l in &t.land_id {
            if l != 0 {
                *count.entry(l).or_insert(0) += 1;
            }
        }
        let big = |l: u16| count.get(&l).map_or(false, |&a| a >= 1500);
        let mut rng = Rng::new(self.seed ^ 0x0101_51e5);
        let mut cands: Vec<(i32, i32)> = Vec::new();
        for _ in 0..20_000 {
            if cands.len() >= 4000 {
                break;
            }
            let i = rng.below(d.n());
            let (x, y) = ((i % d.w) as i32, (i / d.w) as i32);
            if !matches!(t.kind[i], crate::terrain::GRASS | crate::terrain::FOREST) || !big(t.land_id[i]) {
                continue;
            }
            cands.push((x, y));
        }
        while sites.len() < n {
            let best = cands
                .iter()
                .map(|&c| (sites.iter().map(|&s| gap(s, c)).fold(f32::MAX, f32::min), c))
                .filter(|&(near, _)| near >= MIN_SITE_GAP)
                .max_by(|a, b| a.0.total_cmp(&b.0));
            match best {
                Some((_, c)) => sites.push(c),
                None => break,
            }
        }
        sites
    }

    /// Make `n` civilisations waiting to be founded: their seeds, names, colours and homelands
    /// come from the run seed, as `populate` makes them.
    pub fn prepare_slots(&mut self, n: usize) {
        let sites: Vec<(i32, i32)> = (0..n)
            .map(|i| {
                let region = i % self.terrain.regions.len().max(1);
                self.terrain.regions.get(region).map(|r| r.start).unwrap_or((80 + 160 * i as i32, self.d.h as i32 / 2))
            })
            .collect();
        self.prepare_slots_at(n, &sites);
    }

    /// Player slots at these homelands.
    pub fn prepare_slots_at(&mut self, n: usize, sites: &[(i32, i32)]) {
        let seeds = self.civ_seeds(n);
        for (i, &s) in seeds.iter().enumerate() {
            let start = sites.get(i).copied().unwrap_or((80 + 160 * i as i32, self.d.h as i32 / 2));
            let f = self.add_faction(s, SeedSource::Mind, Traits::from_seed(s), NO_F, None);
            let region = self.terrain.regions.iter().position(|r| r.start == start).unwrap_or(255) as u8;
            let fac = &mut self.factions[f as usize];
            fac.alive = false;
            fac.pending = true;
            fac.region = region;
            fac.start = start;
            self.setup.slots.push(f);
        }
        self.setup.phase = Phase::Choose;
        self.setup.slot = self.setup.slots.first().copied().unwrap_or(NO_F);
        if let Some(&f) = self.setup.slots.first() {
            let s = self.factions[f as usize].start;
            self.cam = crate::planet::Cam::centred_on(self.d, s.0 as f32, s.1 as f32);
        }
        self.refresh_info();
    }

    /// Rival civilisations waiting at these sites, arriving at staggered times.
    pub fn prepare_rivals(&mut self, sites: &[(i32, i32)]) {
        let mut r = Rng::new(self.seed ^ 0x7e71_0101);
        let mut at = RIVAL_FIRST;
        for &site in sites {
            let s = r.next_u32();
            let f = self.add_faction(s, SeedSource::Run, Traits::from_seed(s), NO_F, None);
            let fac = &mut self.factions[f as usize];
            fac.alive = false;
            fac.pending = true;
            fac.start = site;
            self.setup.rivals.push((f, at));
            at += RIVAL_EVERY * r.range(0.5, 1.5);
        }
        self.refresh_info();
    }

    /// Rivals whose time has come are founded (near their site if it has been taken).
    pub(crate) fn rivals_tick(&mut self) {
        let now = self.time;
        for k in 0..self.setup.rivals.len() {
            let (f, at) = self.setup.rivals[k];
            if at > now || !self.factions[f as usize].pending {
                continue;
            }
            let s = self.factions[f as usize].start;
            let spot = self.nearest_start(s, 80);
            match spot {
                Some((x, y)) => {
                    let t = self.factions[f as usize].traits;
                    self.found_civ(f, x, y, t);
                }
                // Nowhere free: try again in a while.
                None => self.setup.rivals[k].1 = now + 20.0,
            }
        }
    }

    /// The nearest place to `s` (within `r` tiles) where a rival may begin: free land, not
    /// within `RIVAL_GAP` of anyone's village (rivals squeeze in closer than the player may).
    fn nearest_start(&self, s: (i32, i32), r: i32) -> Option<(i32, i32)> {
        let ok = |x: i32, y: i32| {
            self.terrain.walkable(x, y)
                && self.owner[self.d.idx(x, y)] == NO_F
                && self.villages.iter().filter(|v| v.alive).all(|v| self.d.dist(x as f32, y as f32, v.x as f32, v.y as f32) >= RIVAL_GAP)
        };
        if ok(s.0, s.1) {
            return Some(s);
        }
        for ring in (2..=r).step_by(2) {
            for k in 0..(4 * ring) {
                let a = k as f32 / (4 * ring) as f32 * std::f32::consts::TAU;
                let (x, y) = (s.0 + (a.cos() * ring as f32) as i32, s.1 + (a.sin() * ring as f32) as i32);
                if ok(x, y) {
                    return Some((self.d.wrap_x(x), self.d.wrap_y(y)));
                }
            }
        }
        None
    }

    fn pending(&self) -> Vec<u8> {
        self.setup.slots.iter().copied().filter(|&f| self.factions[f as usize].pending).collect()
    }

    /// May a civilisation begin at (x, y)? Err(reason) if not.
    pub fn start_ok(&self, x: i32, y: i32) -> Result<(), String> {
        match self.terrain.kind_at(x, y) {
            DEEP | WATER => return Err("that is sea".into()),
            MOUNTAIN => return Err("mountains: nobody can live there".into()),
            ICE => return Err("polar ice: nobody can live there".into()),
            _ => {}
        }
        let o = self.owner[self.d.idx(x, y)];
        if o != NO_F {
            return Err(format!("that land belongs to the {}", self.factions[o as usize].name));
        }
        for v in self.villages.iter().filter(|v| v.alive) {
            let d = self.d.dist(x as f32, y as f32, v.x as f32, v.y as f32);
            if d < MIN_START_GAP {
                return Err(format!("too close to the {} ({} tiles; keep {} away)", self.factions[v.owner as usize].name, d.round() as i32, MIN_START_GAP as i32));
            }
        }
        Ok(())
    }

    fn set_spot(&mut self, x: i32, y: i32) {
        let (x, y) = (self.d.wrap_x(x), self.d.wrap_y(y));
        self.setup.spot = (x, y);
        match self.start_ok(x, y) {
            Ok(()) => {
                self.setup.spot_ok = true;
                self.setup.reason.clear();
            }
            Err(r) => {
                self.setup.spot_ok = false;
                self.setup.reason = r;
            }
        }
    }

    /// Choose a waiting civilisation (then place it).
    pub fn setup_select(&mut self, f: u8) -> bool {
        if !matches!(self.setup.phase, Phase::Choose | Phase::Place) || (f as usize) >= MAX_FACTIONS || !self.factions[f as usize].pending {
            return false;
        }
        self.setup.slot = f;
        self.setup.phase = Phase::Place;
        let s = self.factions[f as usize].start;
        self.set_spot(s.0, s.1);
        self.cam.centre(s.0 as f32, s.1 as f32);
        self.log(ev::SETUP, f, NO_F, s.0, s.1, 0);
        true
    }

    /// Offer the next waiting civilisation (Tab).
    pub fn setup_next(&mut self) {
        let p = self.pending();
        if p.is_empty() {
            return;
        }
        let i = p.iter().position(|&f| f == self.setup.slot).map(|i| (i + 1) % p.len()).unwrap_or(0);
        let f = p[i];
        if self.setup.phase == Phase::Choose {
            self.setup.slot = f;
            let s = self.factions[f as usize].start;
            self.cam.centre(s.0 as f32, s.1 as f32);
        } else if self.setup.phase == Phase::Place {
            self.setup_select(f);
        }
    }

    /// Where the player aims while placing: shows whether a civilisation may begin there.
    pub fn setup_hover(&mut self, x: f32, y: f32) -> bool {
        if self.setup.phase != Phase::Place {
            return false;
        }
        self.set_spot(x.floor() as i32, y.floor() as i32);
        self.setup.spot_ok
    }

    /// A click (or blink) on the planet: choose the civilisation whose homeland is nearest, or
    /// begin the one being placed here, if it may.
    pub fn setup_click(&mut self, x: f32, y: f32) -> Phase {
        match self.setup.phase {
            Phase::Choose => {
                let near = self.pending().into_iter().min_by(|&a, &b| {
                    let (sa, sb) = (self.factions[a as usize].start, self.factions[b as usize].start);
                    let da = self.d.dist(x, y, sa.0 as f32, sa.1 as f32);
                    let db = self.d.dist(x, y, sb.0 as f32, sb.1 as f32);
                    da.total_cmp(&db)
                });
                if let Some(f) = near {
                    self.setup_select(f);
                }
            }
            Phase::Place => {
                if self.setup_hover(x, y) {
                    self.begin_seeding();
                }
            }
            Phase::Seed => self.setup.blinks += 1,
            _ => {}
        }
        self.setup.phase
    }

    /// Enter: take the civilisation offered, or begin it in its homeland.
    pub fn setup_enter(&mut self) -> Phase {
        match self.setup.phase {
            Phase::Choose => {
                let f = self.setup.slot;
                if !self.setup_select(f) {
                    if let Some(&f) = self.pending().first() {
                        self.setup_select(f);
                    }
                }
            }
            Phase::Place => {
                // Enter keeps its homeland (a click takes the spot aimed at).
                let (x, y) = self.factions[self.setup.slot as usize].start;
                self.set_spot(x, y);
                if self.setup.spot_ok {
                    self.begin_seeding();
                }
            }
            _ => {}
        }
        self.setup.phase
    }

    /// A blink while seeding (it makes the civilisation wilder).
    pub fn setup_blink(&mut self) {
        if self.setup.phase == Phase::Seed {
            self.setup.blinks += 1;
        }
    }

    fn begin_seeding(&mut self) {
        self.setup.phase = Phase::Seed;
        self.setup.t = 0.0;
        self.setup.sum = [0.0; 5];
        self.setup.blinks = 0;
        let f = self.setup.slot;
        let (x, y) = self.setup.spot;
        self.log(ev::SETUP, f, NO_F, x, y, 1);
    }

    /// Seeding runs on real time (whatever the world's speed): average the mind, fill in the
    /// traits, and found the civilisation when the window closes.
    pub fn setup_tick(&mut self, real_dt: f32) {
        if self.setup.phase != Phase::Seed || real_dt <= 0.0 {
            return;
        }
        let i = self.inputs;
        let s = &mut self.setup;
        let dt = real_dt.min(0.25);
        s.t += dt;
        for (k, v) in [i.calm, i.focus, i.stress, i.anger, i.joy].into_iter().enumerate() {
            s.sum[k] += v.clamp(0.0, 1.0) * dt;
        }
        let n = s.t.max(1e-3);
        let seed = self.factions[s.slot as usize].seed;
        s.traits = mind_traits(s.sum[0] / n, s.sum[1] / n, s.sum[2] / n, s.sum[3] / n, s.sum[4] / n, s.blinks, s.t, seed);
        if s.t >= SEED_SECONDS {
            let (f, (x, y), t) = (s.slot, s.spot, s.traits);
            self.found_civ(f, x, y, t);
        }
    }

    /// Found a waiting civilisation at (x, y) with these traits: its first village and four
    /// people. The first one founded is the player's own (★).
    pub fn found_civ(&mut self, f: u8, x: i32, y: i32, traits: Traits) -> bool {
        if (f as usize) >= MAX_FACTIONS || !self.factions[f as usize].pending {
            return false;
        }
        // The player's own people (★): the first of the player's slots to be founded.
        let mine = self.setup.slots.contains(&f);
        let first = mine && !self.setup.founded.iter().any(|(g, _)| self.setup.slots.contains(g));
        {
            let fac = &mut self.factions[f as usize];
            fac.pending = false;
            fac.alive = true;
            fac.traits = traits.clamped();
            if mine {
                fac.seed_source = SeedSource::Mind;
            }
            fac.born = self.time;
            fac.start = (x, y);
            fac.is_player = first;
        }
        let v = self.found_village(f, x, y, 6.0, false);
        self.factions[f as usize].capital = v;
        self.villages[v as usize].built |= 1u64 << Building::Campfire as u8;
        for k in 0..START_AGENTS {
            let (ox, oy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][k % 4];
            let (ax, ay) = if self.terrain.walkable(x + ox, y + oy) { (x + ox, y + oy) } else { (x, y) };
            let job = if k % 2 == 0 { Job::Forage } else { Job::Hunt };
            if let Some(a) = self.spawn_agent(f, v, job, ax, ay) {
                let age = self.rng.range(0.0, 20.0);
                self.agents[a].age = age;
                self.agents[a].clan = (k / 2) as u8;
            }
        }
        self.setup.founded.push((f, self.time));
        let order = self.setup.founded.len() as i32;
        let packed = pack_traits(&self.factions[f as usize].traits);
        self.log_x(ev::CIV_FOUNDED, f, NO_F, x, y, order, packed);
        if mine {
            self.setup.phase = if self.pending().is_empty() { Phase::Done } else { Phase::Choose };
            if let Some(&next) = self.pending().first() {
                self.setup.slot = next;
            }
        }
        self.recount();
        self.refresh_hostility();
        self.refresh_info();
        true
    }

    /// The setup's words: what to do now, then a line per civilisation (founded, being seeded
    /// with its traits filling in, or waiting).
    pub fn setup_text(&self) -> String {
        let s = &self.setup;
        let mut out = String::new();
        let name = |f: u8| self.factions.get(f as usize).map(|x| x.name.as_str()).unwrap_or("?");
        match s.phase {
            Phase::Off | Phase::Done => return out,
            Phase::Choose => {
                let first = s.founded.is_empty();
                let _ = writeln!(
                    out,
                    "Choose {} civilisation: click its homeland, Tab for the next, Enter for the {}.",
                    if first { "your first" } else { "the next" },
                    name(s.slot)
                );
            }
            Phase::Place => {
                let _ = writeln!(out, "Where do the {} begin? Aim and click, or Enter for their homeland.", name(s.slot));
                if !s.spot_ok {
                    let _ = writeln!(out, "Not there: {}.", s.reason);
                }
            }
            Phase::Seed => {
                let left = (SEED_SECONDS - s.t).max(0.0).ceil() as i32;
                let _ = writeln!(out, "Seeding the {} from your mind: {left} s. Calm makes them kind, stress and anger fierce, focus fertile, blinks wild.", name(s.slot));
            }
        }
        if !s.rivals.is_empty() {
            let arrived = s.rivals.iter().filter(|r| !self.factions[r.0 as usize].pending).count();
            let next = s.rivals.iter().filter(|r| self.factions[r.0 as usize].pending).map(|r| r.1).fold(f32::MAX, f32::min);
            if arrived < s.rivals.len() {
                let _ = writeln!(out, "Rivals: {arrived} of {} have arrived; the next in {} s.", s.rivals.len(), (next - self.time).max(0.0).ceil() as i32);
            } else {
                let _ = writeln!(out, "Rivals: all {} have arrived.", s.rivals.len());
            }
        }
        for &f in &s.slots {
            let fac = &self.factions[f as usize];
            let line = if !fac.pending {
                format!("{}{}: founded · {}", if fac.is_player { "★ " } else { "" }, fac.name, traits_line(&fac.traits))
            } else if s.phase == Phase::Seed && f == s.slot {
                format!("{}: seeding · {}", fac.name, traits_line(&s.traits))
            } else {
                format!("{}: waiting{}", fac.name, if f == s.slot { " (next)" } else { "" })
            };
            let _ = writeln!(out, "{line}");
        }
        out
    }

    /// Seeding progress, 0..1.
    pub fn setup_progress(&self) -> f32 {
        if self.setup.phase == Phase::Seed {
            (self.setup.t / SEED_SECONDS).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The four civilisation seeds of a run (as `populate` draws them): from the run seed,
    /// redrawn until the temperaments differ.
    pub(crate) fn civ_seeds(&self, n: usize) -> Vec<u32> {
        let mut run = Rng::new(self.seed ^ 0x2545_f491);
        let mut seeds = vec![0u32; n];
        for _ in 0..64 {
            for s in seeds.iter_mut() {
                *s = run.next_u32();
            }
            let tr: Vec<Traits> = seeds.iter().map(|&s| Traits::from_seed(s)).collect();
            let amax = tr.iter().map(|t| t.aggressiveness).fold(0.0, f32::max);
            let amin = tr.iter().map(|t| t.aggressiveness).fold(1.0, f32::min);
            let fmax = tr.iter().map(|t| t.friendliness).fold(0.0, f32::max);
            if amax >= 0.62 && amin <= 0.4 && fmax >= 0.55 {
                break;
            }
        }
        seeds
    }
}
