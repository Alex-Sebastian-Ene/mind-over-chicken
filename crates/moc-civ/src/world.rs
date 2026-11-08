//! World state, construction and the step loop. Agent behaviour lives in [`crate::agents`],
//! villages, diplomacy and rebellion in [`crate::politics`], god powers in [`crate::powers`].

use moc_planet::fb::Framebuffer;
use moc_planet::rng::Rng;

pub use crate::jobs::{Building, Job};
use crate::conflict::Strife;
use crate::jobs::{Fx, CATS};
use crate::planet::{Cam, Dims, Paths};
use crate::terrain::{self, food_max, Terrain};

/// Civilisation slots: up to 12 at the start (#101) and room for rebels.
pub const MAX_FACTIONS: usize = 24;
pub const NO_F: u8 = 255;
pub const NO_V: u16 = u16::MAX;
pub const NO_TILE: i32 = i32::MIN;
/// Agent and village limits on the default planet; a bigger planet gets more (`World::cap_agents`).
pub const MAX_AGENTS: usize = 2400;
pub const MAX_VILLAGES: usize = 480;
/// Civilisations and agents per civilisation at the start (#52).
pub const START_CIVS: usize = 4;
pub const START_AGENTS: usize = 4;

/// Village bookkeeping (spawning, unrest, sieges) runs on this period; diplomacy every second
/// (see `politics::phase_tick`).
pub const SLOW_DT: f32 = 0.5;
pub const SPAWN_COST: f32 = 3.0;
pub const SPAWN_EVERY: f32 = 2.0;
pub const SETTLER_COST: f32 = 5.0;
/// Agent food (0..1) eaten per second; a full belly lasts 50 s.
pub const EAT_RATE: f32 = 1.0 / 50.0;
pub const GATHER_TIME: f32 = 1.8;
pub const CARRY_MAX: f32 = 0.6;
/// Tile food regrowth per second, as a fraction of the tile's maximum.
pub const REGEN: f32 = 0.012;
pub const MIN_SPACING: f32 = 14.0;
pub const START_RADIUS: i32 = 6;
pub const WAR_TENSION: f32 = 1.0;
pub const RAID_TENSION: f32 = 0.55;
pub const REBEL_UNREST: f32 = 1.0;
pub const SIEGE_TO_CAPTURE: f32 = 2.5;
pub const STUCK_LIMIT: u16 = 40;

const GRID: i32 = 8;

/// Event kinds in the log JS polls.
pub mod ev {
    pub const CONTACT: i32 = 1;
    pub const WAR: i32 = 2;
    pub const PEACE: i32 = 3;
    pub const CAPTURE: i32 = 4;
    pub const REBELLION: i32 = 5;
    pub const WIPED: i32 = 6;
    pub const FOUNDED: i32 = 7;
    pub const RAID: i32 = 8;
    pub const FAMINE: i32 = 9;
    pub const SMITE: i32 = 10;
    pub const TRADE: i32 = 11;
    pub const CAPITAL: i32 = 12;
    pub const RAZED: i32 = 13;
    pub const CAST: i32 = 14;
    // The ages and conflict (#69).
    /// a enters age n (extra 1: climbing back after a Dark Age).
    pub const AGE_UP: i32 = 15;
    /// a falls back to age n, a Dark Age (extra 1: stability, 2: population).
    pub const COLLAPSE: i32 = 16;
    /// A conflict climbs to level n (`conflict::LEVELS`); a == b inside one civilisation
    /// (extra: the strife kind + 1, 0 between civilisations).
    pub const ESCALATE: i32 = 17;
    pub const DEESCALATE: i32 = 18;
    /// A fight inside civilisation a begins at level n (extra: `conflict::StrifeKind`).
    pub const STRIFE: i32 = 19;
    /// A strife inside a reaches civil war (n rebel villages, extra the kind).
    pub const CIVIL_WAR: i32 = 20;
    /// The rebels inside a are crushed.
    pub const CRUSHED: i32 = 21;
    /// The rebels inside a take over: a revolution (no split).
    pub const REVOLUTION: i32 = 22;
    /// a becomes b's vassal (extra 0 defeat, 1 treaty, 2 rebel province).
    pub const VASSAL: i32 = 23;
    /// A truce between a and b (a == b: inside one civilisation).
    pub const TRUCE: i32 = 24;
    /// A crisis of a's age strikes a (extra: index into `ages_table::CRISES`).
    pub const CRISIS: i32 = 25;
    pub const PLAGUE: i32 = 26;
    /// a takes n captives from b.
    pub const CAPTIVES: i32 = 27;
    /// a battle between a and b ends: a holds the field (n dead).
    pub const BATTLE: i32 = 28;
    pub const FESTIVAL: i32 = 29;
    /// a founds a colony across the sea.
    pub const COLONY: i32 = 30;
    /// Vassal a breaks free of b.
    pub const INDEPENDENCE: i32 = 31;
    /// a's chief dies; extra 1 if the succession is disputed.
    pub const SUCCESSION: i32 = 32;
    // Staggered starts (#98).
    /// Civilisation a is founded, n-th, with traits `setup::unpack_traits(extra)`.
    pub const CIV_FOUNDED: i32 = 33;
    /// The player picks civilisation a to set up (n 0) or starts seeding it (n 1).
    pub const SETUP: i32 = 34;
}

/// One log entry: what happened, between which civilisations, where, how much.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Event {
    pub kind: i32,
    pub a: i32,
    pub b: i32,
    pub x: i32,
    pub y: i32,
    pub n: i32,
    pub t_ms: i32,
    pub extra: i32,
}

/// An agent's injuries: fighting wounds before it kills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Wound {
    Healthy = 0,
    /// Below `WOUND_HP`: slower, hits softer, loses heart; heals at home.
    Wounded = 1,
}

/// Health below which an agent is wounded, and above which (at home) it recovers.
pub const WOUND_HP: f32 = 0.45;
pub const HEALED_HP: f32 = 0.75;
/// Morale below which a fighter breaks and runs home.
pub const ROUT_MORALE: f32 = 0.22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Idle,
    ToFood,
    Gather,
    ToHome,
    Flee,
    Wander,
    March,
    Siege,
    ToSite,
    /// Asleep at home (night, from the Hearth Clans on).
    Sleep,
    /// Working in and around the village.
    Work,
    /// On the road to another settlement (`Agent::target`).
    Haul,
    /// At sea, in a boat (sailors, colonists).
    Sail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cause {
    Alive,
    Starved,
    OldAge,
    Combat,
    Fire,
    Smitten,
    Disaster,
    Plague,
}

/// Where a civilisation's traits came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SeedSource {
    /// Derived from the run seed (the dummy run: all four are random).
    Run = 0,
    /// Set from a calibration payload (`civ_set_player_traits`, #54).
    Calibration = 1,
    /// Read from the player's brain (`civ_seed_player_mind`).
    Mind = 2,
    /// A rebel civilisation: the parent's traits, drifted.
    Rebellion = 3,
}

/// Behavioural traits, all 0..1. `friendliness`, `aggressiveness` and `violence` are the
/// README's three; fertility and cohesion are extra.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Traits {
    /// Trade, first impressions, peace-making.
    pub friendliness: f32,
    /// How readily tension turns into raids and war; how many warriors are kept.
    pub aggressiveness: f32,
    /// How lethal fighting is; whether captured villagers are spared.
    pub violence: f32,
    /// Spawn rate and village size.
    pub fertility: f32,
    /// Resistance to unrest and rebellion.
    pub cohesion: f32,
}

impl Traits {
    /// Traits from a civilisation seed alone.
    pub fn from_seed(seed: u32) -> Traits {
        let mut r = Rng::new(seed);
        Traits {
            friendliness: r.range(0.1, 0.9),
            aggressiveness: r.range(0.1, 0.92),
            violence: r.range(0.15, 0.9),
            fertility: r.range(0.35, 0.8),
            cohesion: r.range(0.4, 0.8),
        }
    }

    pub fn clamped(self) -> Traits {
        let c = |v: f32| v.clamp(0.02, 0.98);
        Traits {
            friendliness: c(self.friendliness),
            aggressiveness: c(self.aggressiveness),
            violence: c(self.violence),
            fertility: c(self.fertility),
            cohesion: c(self.cohesion),
        }
    }
}

/// How a civilisation is doing: the metrics of `data/ages.json` (#70), for the age ladder
/// (#69) to read. Computed now (`politics::update_metrics`): population, settlements,
/// food_days, stability, territory, links, contact, coastal, colonies. The rest stay 0 until
/// their systems exist.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub population: u32,
    pub settlements: u32,
    /// Food stock divided by daily consumption (a day is `DAY_SECONDS`).
    pub food_days: f32,
    /// 0..1: 1 − mean village unrest (hunger, crowding, distance, war losses, god's anger).
    pub stability: f32,
    /// Share of the planet's land this civilisation owns, 0..1.
    pub territory: f32,
    pub knowledge: f32,
    pub wealth: f32,
    pub faith: f32,
    pub production: f32,
    /// Friendly neighbours at peace it trades food with.
    pub links: u32,
    /// Has met at least one other civilisation.
    pub contact: bool,
    /// Owns a coastal settlement.
    pub coastal: bool,
    pub vassals: u32,
    /// Settlements on another landmass than the capital's.
    pub colonies: u32,
    pub inequality: f32,
}

/// Seconds in a day (the sandbox's day–night cycle).
pub const DAY_SECONDS: f32 = 80.0;
/// Night above which agents (from the Hearth Clans on) sleep: about a quarter of each day.
pub const SLEEP_NIGHT: f32 = 0.82;
/// The longest single step `World::advance` takes when the speed control runs it faster.
pub const MAX_SUBSTEP: f32 = 0.1;
/// At most this many steps per `advance` (a frame): ×16 at 60 fps needs 3; a slow frame lets the
/// world fall a little behind rather than pile up work.
pub const MAX_STEPS_PER_ADVANCE: usize = 4;

#[derive(Clone, Debug)]
pub struct Faction {
    pub id: u8,
    /// Emergent age (1 = first). The twenty ages of #69 advance this; it starts at 1.
    pub age: u8,
    pub metrics: Metrics,
    /// Slot has ever held a civilisation.
    pub used: bool,
    pub alive: bool,
    pub name: String,
    pub stem: String,
    pub color: [u8; 3],
    pub seed: u32,
    pub seed_source: SeedSource,
    pub traits: Traits,
    pub capital: u16,
    pub parent: u8,
    /// Continent it started on and its first village.
    pub region: u8,
    pub start: (i32, i32),
    pub is_player: bool,
    pub born: f32,
    // Live tallies (refreshed every bookkeeping tick).
    pub pop: u32,
    pub villages: u32,
    pub warriors: u32,
    pub hungry: f32,
    pub food: f32,
    pub kills: u32,
    pub losses: u32,
    /// War deaths, decaying (feeds unrest and war weariness).
    pub recent_losses: f32,
    pub starved_recent: f32,
    pub rebel_cd: f32,
    pub famine_cd: f32,
    pub homeless_t: f32,
    pub last_attacker: u8,
    // ---- the ages (#69) ----
    /// Seconds in the current age.
    pub age_t: f32,
    /// Seconds the next age's entry conditions have held.
    pub ready_t: f32,
    /// Seconds of stability below the collapse line, and of population below its share.
    pub low_stab_t: f32,
    pub low_pop_t: f32,
    /// A Dark Age: no climbing until it runs out.
    pub dark_t: f32,
    pub collapse_cd: f32,
    pub peak_age: u8,
    pub ups: u32,
    pub falls: u32,
    /// Goats in the treasury (taxes, tribute, ransom).
    pub treasury: f32,
    /// Goods made since the last metrics tick, and their smoothed rate (per second).
    pub prod_acc: f32,
    pub prod_rate: f32,
    /// Overlord this civilisation pays tribute to (NO_F = free).
    pub overlord: u8,
    /// Agents per kind of work (`jobs::Cat`), refreshed with the tallies.
    pub cat_counts: [u16; CATS],
    /// Clans (family lines) among its people.
    pub clans: u8,
    /// Crisis timers: drought, strike, censorship, blackout.
    pub drought_t: f32,
    pub strike_t: f32,
    pub censor_t: f32,
    pub blackout_t: f32,
    pub tax_t: f32,
    pub strife_cd: f32,
    pub crisis_cd: f32,
    /// The chief died and nobody has taken the chair yet.
    pub succession: bool,
    /// Seconds of manning the walls left (`politics::Decision::Fortify`).
    pub fortify_t: f32,
    /// Waiting to be founded by the player (#98): not alive yet, and its slot is kept.
    pub pending: bool,
}

impl Faction {
    fn empty(id: u8) -> Faction {
        Faction {
            id,
            age: 1,
            metrics: Metrics::default(),
            used: false,
            alive: false,
            name: String::new(),
            stem: String::new(),
            color: [200, 200, 200],
            seed: 0,
            seed_source: SeedSource::Run,
            traits: Traits::default(),
            capital: NO_V,
            parent: NO_F,
            region: 255,
            start: (0, 0),
            is_player: false,
            born: 0.0,
            pop: 0,
            villages: 0,
            warriors: 0,
            hungry: 0.0,
            food: 0.0,
            kills: 0,
            losses: 0,
            recent_losses: 0.0,
            starved_recent: 0.0,
            rebel_cd: 0.0,
            famine_cd: 0.0,
            homeless_t: 0.0,
            last_attacker: NO_F,
            age_t: 0.0,
            ready_t: 0.0,
            low_stab_t: 0.0,
            low_pop_t: 0.0,
            dark_t: 0.0,
            collapse_cd: 0.0,
            peak_age: 1,
            ups: 0,
            falls: 0,
            treasury: 0.0,
            prod_acc: 0.0,
            prod_rate: 0.0,
            overlord: NO_F,
            cat_counts: [0; CATS],
            clans: 2,
            drought_t: 0.0,
            strike_t: 0.0,
            censor_t: 0.0,
            blackout_t: 0.0,
            tax_t: 0.0,
            strife_cd: 20.0,
            crisis_cd: 30.0,
            succession: false,
            fortify_t: 0.0,
            pending: false,
        }
    }
}

/// How civilisation `a` stands toward `b` (tension is a's hostility toward b).
#[derive(Clone, Copy, Debug, Default)]
pub struct Rel {
    pub met: bool,
    pub border: u32,
    pub near: bool,
    pub tension: f32,
    /// 0 peace, 1 a is raiding b, 2 war (set on both sides).
    pub war: u8,
    pub war_t: f32,
    pub weariness: f32,
    pub cooldown: f32,
    pub trade_t: f32,
    pub trade_news_t: f32,
    pub raid_looted: bool,
    /// Conflict level between the two (`conflict::LEVELS`: 0 none … 5 war), same both ways.
    pub level: u8,
    /// Battles fought since the last peace (several battles make a war, #69).
    pub battles: u8,
    /// Seconds of the current battle.
    pub battle_t: f32,
    /// Old wrongs: losses, captures, conquest. Decays slowly; restarts feuds.
    pub grudge: f32,
    /// Seconds since a's people last traded with b's (a link while recent).
    pub traded_t: f32,
}

#[derive(Clone, Debug)]
pub struct Village {
    pub x: i32,
    pub y: i32,
    pub owner: u8,
    pub alive: bool,
    pub food: f32,
    pub pop: u16,
    pub warriors: u16,
    pub hungry: u16,
    pub cap: u16,
    pub radius: i32,
    pub unrest: f32,
    pub hp: f32,
    pub spawn_t: f32,
    pub settler_t: f32,
    pub founded: f32,
    pub siege: f32,
    pub besieger: u8,
    pub empty_t: f32,
    /// Buildings standing (bit `Building as u8`); those of a lost age stay but go dormant.
    pub built: u64,
    /// Effects of the active buildings (`jobs::village_fx`).
    pub fx: Fx,
    /// Goods made here, spent on buildings.
    pub goods: f32,
    /// Goats left by the dead, inherited by the newborn.
    pub goats: f32,
    /// Devotion 0..1.
    pub faith: f32,
    /// Plague 0..1.
    pub plague: f32,
    /// When a hauler last arrived (a route is a link while it is recent).
    pub linked_t: f32,
    /// Strife it is in (index + 1, 0 none) and its side (0 loyal, 1 rebel).
    pub camp: u8,
    pub side: u8,
    pub mourn_t: f32,
    pub festival_t: f32,
    /// The founding clan.
    pub clan: u8,
    /// When it died (dead slots are reused after a while).
    pub died: f32,
    pub coast: bool,
    pub hills: bool,
    pub raided_t: f32,
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    /// Tile being walked to (NO_TILE when standing on a tile centre).
    pub nx: i32,
    pub ny: i32,
    /// Previous tile, so greedy steering doesn't step straight back.
    pub px: i32,
    pub py: i32,
    /// Goal tile.
    pub gx: i32,
    pub gy: i32,
    pub best: i32,
    pub stuck: u16,
    pub faction: u8,
    pub job: Job,
    pub state: State,
    pub home: u16,
    pub target: u16,
    pub food: f32,
    pub hp: f32,
    pub age: f32,
    pub life: f32,
    pub carry: f32,
    pub timer: f32,
    pub fight: f32,
    /// Fighting spirit 0..1: falls with wounds and lost fights, recovers near home and allies.
    pub morale: f32,
    pub wound: Wound,
    pub garrison: bool,
    pub engage: Option<(i32, i32)>,
    pub alive: bool,
    pub cause: Cause,
    pub tries: u8,
    /// Family line within the civilisation (clan feuds), and household.
    pub clan: u8,
    pub family: u32,
    /// Strife it is in (index + 1, 0 none) and its side.
    pub camp: u8,
    pub side: u8,
    /// Goats it owns (wealth, inequality).
    pub goats: f32,
    /// Seconds of a non-lethal scuffle left.
    pub brawl: f32,
    /// Seconds of forced labour left as a captive (0 free).
    pub prisoner: f32,
    /// Civilisation it was born into (ransom).
    pub origin: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Effect {
    pub x: f32,
    pub y: f32,
    pub t: f32,
    pub ttl: f32,
    pub r: f32,
    /// 0 fight spark, 1 blast, 2 storm, 3 blessing, 4 bounty, 5 peace, 6 capture, 7 rebellion.
    pub kind: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Fire {
    pub i: u32,
    pub ttl: f32,
    /// How many tiles the fire has passed through to get here: each step spreads less (it burns
    /// out on its own instead of sweeping whole forests).
    pub gen: u8,
}

/// Fire spread attempts per burning tile per second, before the per-generation fall-off.
pub const FIRE_SPREAD: f32 = 0.45;
/// Each generation of a fire spreads this much less, so fires die out after a dozen steps or so.
pub const FIRE_FALLOFF: f32 = 0.8;
/// A woodcutter's harvest fells the tree this often (the forest near villages thins out).
pub const FELL_CHANCE: f32 = 0.3;

/// A war or raid target for one civilisation.
#[derive(Clone, Copy, Debug)]
pub struct March {
    pub enemy: u8,
    pub village: u16,
    pub x: i32,
    pub y: i32,
    pub raid: bool,
    /// A pitched battle on open ground (no capture), not a march on a village.
    pub field: bool,
    /// For a strife inside the civilisation: its index + 1 and the side marching.
    pub camp: u8,
    pub side: u8,
    /// Unit vector from home toward the target (formations face along it).
    pub dir: (f32, f32),
}

/// What the player's brain and face are doing this step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    pub calm: f32,
    pub focus: f32,
    /// 0..1, 0 at rest: the brain's `stress` (neutral at rest), or, without one, the
    /// formula `max(1 − calm, anger)` measured from its resting level (`Inputs::with_stress`).
    pub stress: f32,
    pub anger: f32,
    /// A smile on camera (festivals).
    pub joy: f32,
}

/// Where `max(1 − calm, anger)` sits when the player is doing nothing (resting calm is low).
pub const STRESS_REST: f32 = 0.8;

impl Inputs {
    /// Inputs from calm, focus and anger alone, with stress from the formula
    /// `max(1 − calm, anger)` measured from its resting level (0.8 → 0, 1 → 1).
    pub fn with_stress(calm: f32, focus: f32, anger: f32) -> Inputs {
        let raw = (1.0 - calm).max(anger);
        Inputs { calm, focus, stress: ((raw - STRESS_REST) / (1.0 - STRESS_REST)).clamp(0.0, 1.0), anger, joy: 0.0 }
    }
    /// Stress, 0..1: the player is tense rather than merely not calm.
    pub fn agitation(&self) -> f32 {
        self.stress.clamp(0.0, 1.0)
    }
    /// The god's mood pushed onto every chief: + agitation and anger, − calm.
    pub fn hostility(&self) -> f32 {
        0.7 * self.agitation() + 0.6 * self.anger.clamp(0.0, 1.0) - 0.75 * self.calm.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub births: u32,
    pub deaths: u32,
    pub starved: u32,
    pub old_age: u32,
    pub killed: u32,
    pub burned: u32,
    pub smitten: u32,
    pub wars: u32,
    pub peaces: u32,
    pub raids: u32,
    pub captures: u32,
    pub rebellions: u32,
    pub contacts: u32,
    pub wiped: u32,
    pub founded: u32,
    pub trades: u32,
    pub field_builds: u32,
    pub peak_agents: u32,
    pub age_ups: u32,
    pub collapses: u32,
    pub recoveries: u32,
    pub strifes: u32,
    pub civil_wars: u32,
    pub crushed: u32,
    pub revolutions: u32,
    pub escalations: u32,
    pub battles: u32,
    pub captives: u32,
    pub vassals: u32,
    pub plagues: u32,
    pub crises: u32,
    pub festivals: u32,
    pub brawls: u32,
    pub routs: u32,
    pub max_age: u32,
    pub buildings: u32,
}

/// Lazily regrowing food on every tile.
pub struct FoodMap {
    pub amt: Vec<f32>,
    pub t: Vec<f32>,
}

impl FoodMap {
    fn new(t: &Terrain) -> FoodMap {
        FoodMap { amt: t.kind.iter().map(|&k| food_max(k)).collect(), t: vec![0.0; t.d.n()] }
    }

    #[inline]
    pub fn level(&self, i: usize, kind: u8, now: f32, regen: f32) -> f32 {
        let max = food_max(kind);
        (self.amt[i] + (now - self.t[i]) * REGEN * regen * max).min(max.max(self.amt[i]))
    }

    pub fn take(&mut self, i: usize, kind: u8, now: f32, regen: f32, want: f32) -> f32 {
        let have = self.level(i, kind, now, regen);
        let got = have.min(want).max(0.0);
        self.amt[i] = have - got;
        self.t[i] = now;
        got
    }

    pub fn set(&mut self, i: usize, v: f32, now: f32) {
        self.amt[i] = v;
        self.t[i] = now;
    }
}

/// Uniform grid over the planet for neighbour queries (fights, contacts, sieges).
pub struct Grid {
    gw: i32,
    gh: i32,
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    fn new(d: Dims) -> Grid {
        let (gw, gh) = ((d.w as i32 + GRID - 1) / GRID, (d.h as i32 + GRID - 1) / GRID);
        Grid { gw, gh, start: vec![0; (gw * gh + 1) as usize], items: Vec::new() }
    }

    #[inline]
    fn cell(&self, x: f32, y: f32) -> usize {
        let cx = (x as i32 / GRID).clamp(0, self.gw - 1);
        let cy = (y as i32 / GRID).clamp(0, self.gh - 1);
        (cy * self.gw + cx) as usize
    }

    fn build(&mut self, agents: &[Agent]) {
        self.start.iter_mut().for_each(|s| *s = 0);
        for a in agents.iter().filter(|a| a.alive) {
            let c = self.cell(a.x, a.y);
            self.start[c + 1] += 1;
        }
        for i in 1..self.start.len() {
            self.start[i] += self.start[i - 1];
        }
        self.items.clear();
        self.items.resize(*self.start.last().unwrap() as usize, 0);
        let mut fill = self.start.clone();
        for (i, a) in agents.iter().enumerate() {
            if a.alive {
                let c = self.cell(a.x, a.y);
                self.items[fill[c] as usize] = i as u32;
                fill[c] += 1;
            }
        }
    }

    /// Call `f` with every agent index in cells overlapping the square of radius `r` round (x, y).
    pub fn near(&self, x: f32, y: f32, r: f32, mut f: impl FnMut(usize)) {
        let (gw, gh) = (self.gw, self.gh);
        // Wraps both ways (the planet is a torus). The last cell row or column may be partial.
        let x0 = ((x - r).floor() as i32).div_euclid(GRID);
        let x1 = ((x + r).floor() as i32).div_euclid(GRID);
        let y0 = ((y - r).floor() as i32).div_euclid(GRID);
        let y1 = ((y + r).floor() as i32).div_euclid(GRID);
        let span = (x1 - x0 + 1).min(gw);
        let rows = (y1 - y0 + 1).min(gh);
        for j in 0..rows {
            let cy = (y0 + j).rem_euclid(gh);
            for k in 0..span {
                let cx = (x0 + k).rem_euclid(gw);
                let c = (cy * gw + cx) as usize;
                for &i in &self.items[self.start[c] as usize..self.start[c + 1] as usize] {
                    f(i as usize);
                }
            }
        }
    }
}

pub struct World {
    pub seed: u32,
    /// Planet size.
    pub d: Dims,
    pub time: f32,
    pub terrain: Terrain,
    pub rng: Rng,
    pub factions: Vec<Faction>,
    pub rel: Vec<Rel>,
    pub villages: Vec<Village>,
    pub agents: Vec<Agent>,
    /// Civilisation owning each tile (NO_F = nobody).
    pub owner: Vec<u8>,
    /// Village claiming each tile (NO_V = none).
    pub claim: Vec<u16>,
    pub food: FoodMap,
    /// 1 where a tile is burning.
    pub burning: Vec<u8>,
    pub fires: Vec<Fire>,
    pub scorched: Vec<(u32, f32)>,
    /// Cleared forest (burnt or felled) waiting to grow back: (tile, when to try). A tile regrows
    /// only with trees beside it, so forests heal from their edges and big clearings stay open.
    pub regrow: Vec<(u32, f32)>,
    pub effects: Vec<Effect>,
    pub events: Vec<Event>,
    pub stats: Stats,
    pub inputs: Inputs,
    pub rain_t: f32,
    pub cam: Cam,
    pub view: Framebuffer,
    pub paths: Paths,
    /// Sea routes toward ports and landing sites (#197): boats keep to the water.
    pub sea: Paths,
    pub grid: Grid,
    pub marches: Vec<Vec<March>>,
    pub hostile: Vec<bool>,
    pub at_war: Vec<bool>,
    /// Nearest village pair (distance, a's village, b's village) on one landmass, per pair.
    pub near_pairs: Vec<(f32, u16, u16)>,
    pub(crate) slow_acc: f32,
    pub(crate) slow_n: u32,
    pub(crate) next_id: u32,
    // Buffers JS reads.
    pub info_f: Vec<f32>,
    pub info_v: Vec<f32>,
    pub names: Vec<u8>,
    pub stats_buf: Vec<f32>,
    pub minimap: Vec<u8>,
    /// Walkable tiles on the planet, and tiles owned per civilisation (refreshed every second).
    pub land_tiles: u32,
    pub owned_tiles: Vec<u32>,
    // ---- the ages and conflict (#69) ----
    /// Fights inside civilisations (clan feuds, revolts, civil wars).
    pub strifes: Vec<Strife>,
    /// Time speed (1, 4 or 16): `advance` runs this many seconds per real second.
    pub speed: f32,
    /// How a's people treat b's (MAX_FACTIONS²): 0 leave alone, 1 scuffle (no killing),
    /// 2 feud (fighters kill), 3 raid, battle or war (`hostile`).
    pub stance: Vec<u8>,
    /// Highest conflict level each civilisation is in (with others or itself).
    pub fight_level: Vec<u8>,
    /// Any fighting at all (combat runs for its agents).
    pub busy: Vec<bool>,
    /// 0 midday … 1 midnight (the sandbox's day–night cycle).
    pub night: f32,
    pub(crate) scratch: Vec<f32>,
    /// Steps taken (combat staggers its searches by it; the rules' dice roll on it).
    pub tick: u32,
    /// A war-and-peace policy replacing the rules of `politics::decide` (#100); None = rules.
    pub policy: Option<Box<dyn crate::politics::Policy>>,
    /// Headlines, log lines and advisor lines for the sandbox (`news`).
    pub news: crate::news::News,
    /// Text handed to JS (`civ_hud_*`, `civ_label_*`).
    pub text_buf: String,
    /// Steps the last `advance` took (for per-step timing).
    pub last_steps: u32,
    /// The player's founding of civilisations one by one (#98; `Phase::Off` when all start at once).
    pub setup: crate::setup::Setup,
    /// Agent and village limits for this planet: the defaults, more on a bigger one (#101).
    pub cap_agents: usize,
    pub cap_villages: usize,
    #[cfg(not(target_arch = "wasm32"))]
    pub prof: [f64; 8],
    /// The politics tick's parts (native profiling only).
    #[cfg(not(target_arch = "wasm32"))]
    pub prof2: [f64; 10],
}

pub const FSTRIDE: usize = 46;
pub const VSTRIDE: usize = 12;
pub const NAME_LEN: usize = 24;
pub const STATS_LEN: usize = 44;

const PALETTE: [[u8; 3]; 24] = [
    [232, 70, 62],   // red
    [66, 136, 246],  // blue
    [246, 198, 54],  // gold
    [186, 92, 226],  // violet
    [250, 140, 40],  // orange
    [56, 212, 222],  // cyan
    [244, 108, 172], // pink
    [238, 238, 238], // white
    [150, 96, 52],   // brown
    [120, 230, 120], // mint
    [40, 60, 160],   // navy
    [200, 30, 110],  // crimson
    [170, 200, 40],  // lime
    [255, 210, 160], // peach
    [110, 110, 120], // slate
    [140, 40, 40],   // maroon
    [255, 120, 90],  // coral
    [0, 150, 130],   // teal
    [210, 170, 255], // lavender
    [180, 140, 20],  // ochre
    [90, 200, 255],  // sky
    [255, 80, 200],  // magenta
    [200, 230, 160], // sage
    [100, 70, 160],  // indigo
];

const SYL_A: [&str; 24] = [
    "Ash", "Bor", "Cal", "Dra", "El", "Fen", "Gor", "Hal", "Ir", "Jun", "Kar", "Lun", "Mor", "Nar", "Os", "Pel", "Ruk", "Sel",
    "Tam", "Ul", "Var", "Wen", "Yr", "Zan",
];
const SYL_B: [&str; 12] = ["ka", "en", "ar", "oth", "um", "ira", "ost", "an", "el", "ok", "un", "is"];
const SUFFIX: [&str; 6] = ["i", "ans", "ites", "folk", "ari", "en"];
const REBEL_PREFIX: [&str; 8] = ["Free", "New", "True", "Red", "Hill", "River", "Iron", "Dawn"];

fn make_name(seed: u32) -> (String, String) {
    let mut r = Rng::new(seed ^ 0x00c0_ffee);
    let stem = format!("{}{}", r.pick(&SYL_A), r.pick(&SYL_B));
    let name = format!("{}{}", stem, r.pick(&SUFFIX));
    (name, stem)
}

impl World {
    /// A fresh run on the default (1280×640) planet: four civilisations of four agents each.
    pub fn new(seed: u32) -> World {
        World::new_sized(seed, Dims::DEFAULT)
    }

    /// A fresh run on a planet of size `d`.
    pub fn new_sized(seed: u32, d: Dims) -> World {
        let mut w = World::with_terrain(seed, Terrain::generate(seed, d));
        w.populate();
        w
    }

    /// An empty world on a given terrain (for tests and custom scenarios).
    pub fn with_terrain(seed: u32, terrain: Terrain) -> World {
        let d = terrain.d;
        let n = d.n();
        let food = FoodMap::new(&terrain);
        let mut w = World {
            seed,
            d,
            time: 0.0,
            rng: Rng::new(seed.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b),
            factions: (0..MAX_FACTIONS as u8).map(Faction::empty).collect(),
            rel: vec![Rel::default(); MAX_FACTIONS * MAX_FACTIONS],
            villages: Vec::new(),
            agents: Vec::new(),
            owner: vec![NO_F; n],
            claim: vec![NO_V; n],
            food,
            burning: vec![0; n],
            fires: Vec::new(),
            scorched: Vec::new(),
            regrow: Vec::new(),
            effects: Vec::new(),
            events: Vec::new(),
            stats: Stats::default(),
            inputs: Inputs::default(),
            rain_t: 0.0,
            cam: Cam::centred_on(d, d.w as f32 / 2.0, d.h as f32 / 2.0),
            view: Framebuffer::default(),
            paths: Paths::new(),
            sea: Paths::sea(),
            grid: Grid::new(d),
            marches: vec![Vec::new(); MAX_FACTIONS],
            hostile: vec![false; MAX_FACTIONS * MAX_FACTIONS],
            at_war: vec![false; MAX_FACTIONS],
            near_pairs: vec![(f32::MAX, NO_V, NO_V); MAX_FACTIONS * MAX_FACTIONS],
            slow_acc: 0.0,
            slow_n: 0,
            next_id: 1,
            info_f: vec![0.0; MAX_FACTIONS * FSTRIDE],
            info_v: Vec::new(),
            names: vec![0; MAX_FACTIONS * NAME_LEN],
            stats_buf: vec![0.0; STATS_LEN],
            minimap: Vec::new(),
            land_tiles: terrain.walk.iter().filter(|&&b| b).count() as u32,
            owned_tiles: vec![0; MAX_FACTIONS],
            strifes: Vec::new(),
            speed: 1.0,
            stance: vec![0; MAX_FACTIONS * MAX_FACTIONS],
            fight_level: vec![0; MAX_FACTIONS],
            busy: vec![false; MAX_FACTIONS],
            night: 0.0,
            scratch: Vec::new(),
            tick: 0,
            policy: None,
            news: crate::news::News::default(),
            text_buf: String::new(),
            last_steps: 0,
            setup: crate::setup::Setup::default(),
            cap_agents: MAX_AGENTS,
            cap_villages: MAX_VILLAGES,
            #[cfg(not(target_arch = "wasm32"))]
            prof: [0.0; 8],
            #[cfg(not(target_arch = "wasm32"))]
            prof2: [0.0; 10],
            terrain,
        };
        // More land, more people: the limits grow with the land (about 50 tiles a person).
        let land = w.land_tiles as usize;
        w.cap_agents = (land / 50).clamp(MAX_AGENTS, 9000);
        w.cap_villages = (land / 250).clamp(MAX_VILLAGES, 2000);
        w.refresh_info();
        w
    }

    /// Four civilisations, one per continent, four agents each. Their seeds come from the run
    /// seed; each civilisation's traits come from its own seed alone. Seeds are redrawn until
    /// the four temperaments differ enough to make a story (one hot head, one cool head).
    pub fn populate(&mut self) {
        let seeds = self.civ_seeds(START_CIVS);
        for (i, &s) in seeds.iter().enumerate() {
            let region = i % self.terrain.regions.len().max(1);
            let start = self.terrain.regions.get(region).map(|r| r.start).unwrap_or((80 + 160 * i as i32, self.d.h as i32 / 2));
            let f = self.add_faction(s, SeedSource::Run, Traits::from_seed(s), NO_F, None);
            self.factions[f as usize].region = region as u8;
            self.factions[f as usize].start = start;
            self.factions[f as usize].is_player = i == 0;
            let v = self.found_village(f, start.0, start.1, 6.0, false);
            self.factions[f as usize].capital = v;
            self.villages[v as usize].built |= 1u64 << Building::Campfire as u8;
            for k in 0..START_AGENTS {
                let (ox, oy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][k % 4];
                let (x, y) = if self.terrain.walkable(start.0 + ox, start.1 + oy) { (start.0 + ox, start.1 + oy) } else { start };
                // Two foragers and two hunters, of two clans.
                let job = if k % 2 == 0 { Job::Forage } else { Job::Hunt };
                if let Some(a) = self.spawn_agent(f, v, job, x, y) {
                    // The founders are adults of mixed ages.
                    let age = self.rng.range(0.0, 20.0);
                    self.agents[a].age = age;
                    self.agents[a].clan = (k / 2) as u8;
                }
            }
        }
        if let Some(p) = self.factions.iter().find(|f| f.is_player) {
            self.cam = Cam::centred_on(self.d, p.start.0 as f32, p.start.1 as f32);
        }
        self.recount();
        self.refresh_info();
    }

    /// Create a civilisation in the first free slot. Returns its id.
    pub fn add_faction(&mut self, seed: u32, source: SeedSource, traits: Traits, parent: u8, name: Option<(String, String)>) -> u8 {
        // Never a slot kept for a civilisation waiting to be founded (#98).
        let slot = self
            .factions
            .iter()
            .position(|f| !f.used)
            .or_else(|| self.factions.iter().position(|f| !f.alive && !f.pending))
            .unwrap_or(MAX_FACTIONS - 1);
        let (name, stem) = name.unwrap_or_else(|| make_name(seed));
        let used: Vec<[u8; 3]> = self.factions.iter().filter(|f| f.alive).map(|f| f.color).collect();
        let color = if slot < START_CIVS && !used.contains(&PALETTE[slot]) {
            PALETTE[slot]
        } else {
            *PALETTE.iter().skip(START_CIVS).find(|c| !used.contains(c)).unwrap_or(&PALETTE[slot % PALETTE.len()])
        };
        let f = &mut self.factions[slot];
        *f = Faction::empty(slot as u8);
        f.used = true;
        f.alive = true;
        f.name = name;
        f.stem = stem;
        f.color = color;
        f.seed = seed;
        f.seed_source = source;
        f.traits = traits.clamped();
        f.parent = parent;
        f.born = self.time;
        // A reused slot starts with a clean slate of relations.
        for o in 0..MAX_FACTIONS {
            self.rel[slot * MAX_FACTIONS + o] = Rel::default();
            self.rel[o * MAX_FACTIONS + slot] = Rel::default();
        }
        slot as u8
    }

    #[inline]
    pub fn rel(&self, a: u8, b: u8) -> &Rel {
        &self.rel[a as usize * MAX_FACTIONS + b as usize]
    }

    #[inline]
    pub fn rel_mut(&mut self, a: u8, b: u8) -> &mut Rel {
        &mut self.rel[a as usize * MAX_FACTIONS + b as usize]
    }

    /// Are a and b fighting (war either way, or either raiding the other)?
    #[inline]
    pub fn is_hostile(&self, a: u8, b: u8) -> bool {
        a != b && (a as usize) < MAX_FACTIONS && (b as usize) < MAX_FACTIONS && self.hostile[a as usize * MAX_FACTIONS + b as usize]
    }

    pub fn at_war(&self, a: u8, b: u8) -> bool {
        self.rel(a, b).war == 2
    }

    pub(crate) fn refresh_hostility(&mut self) {
        crate::conflict::refresh(self);
    }

    pub fn log(&mut self, kind: i32, a: u8, b: u8, x: i32, y: i32, n: i32) {
        self.log_x(kind, a, b, x, y, n, 0);
    }

    /// Log an event with an `extra` value (an age's sub-kind, a crisis, a strife kind).
    pub fn log_x(&mut self, kind: i32, a: u8, b: u8, x: i32, y: i32, n: i32, extra: i32) {
        let e = Event {
            kind,
            a: if a == NO_F { -1 } else { a as i32 },
            b: if b == NO_F { -1 } else { b as i32 },
            x,
            y,
            n,
            t_ms: (self.time * 1000.0) as i32,
            extra,
        };
        if self.events.len() >= 1024 {
            self.events.drain(..256);
        }
        self.events.push(e);
        self.news_of(&e);
    }

    /// The living person nearest (x, y) within `r` tiles (#188: a pinch picks people up).
    pub fn agent_near(&self, x: f32, y: f32, r: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = r * r;
        for (i, a) in self.agents.iter().enumerate() {
            if !a.alive {
                continue;
            }
            let (dx, dy) = (self.d.dx(x, a.x), self.d.dy(y, a.y));
            let d = dx * dx + dy * dy;
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    /// Set a living person down at (x, y) if it is walkable land; they drop what they were doing
    /// and carry on from there (#188). Returns whether they moved.
    pub fn move_agent(&mut self, i: usize, x: f32, y: f32) -> bool {
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        if i >= self.agents.len() || !self.agents[i].alive || !self.terrain.walkable(tx, ty) {
            return false;
        }
        let (wx, wy) = (self.d.wrap_xf(x), self.d.wrap_yf(y));
        let (cx, cy) = (self.d.wrap_x(tx), self.d.wrap_y(ty));
        let a = &mut self.agents[i];
        a.x = wx;
        a.y = wy;
        a.nx = NO_TILE;
        a.ny = NO_TILE;
        a.px = cx;
        a.py = cy;
        a.gx = cx;
        a.gy = cy;
        a.stuck = 0;
        a.state = State::Idle;
        a.timer = 0.5;
        true
    }

    pub fn alive_factions(&self) -> impl Iterator<Item = &Faction> {
        self.factions.iter().filter(|f| f.alive)
    }

    /// Found a village and claim the land round it.
    pub fn found_village(&mut self, f: u8, x: i32, y: i32, food: f32, log: bool) -> u16 {
        let (x, y) = (self.d.wrap_x(x), self.d.wrap_y(y));
        // Reuse a slot that has been dead a while (nothing points at it any more), so long
        // runs don't grow the village list without bound.
        let reuse = self.villages.iter().position(|v| !v.alive && v.camp == 0 && self.time - v.died > 30.0);
        let id = reuse.unwrap_or(self.villages.len()) as u16;
        let coast = self.terrain.near_water(x, y, 3);
        let hills = (-8..=8).any(|oy| (-8..=8).any(|ox| matches!(self.terrain.kind_at(x + ox, y + oy), terrain::HILLS | terrain::MOUNTAIN)));
        let clan = self.factions[f as usize].clans.saturating_sub(1).min(7);
        let village = Village {
            x,
            y,
            owner: f,
            alive: true,
            food,
            pop: 0,
            warriors: 0,
            hungry: 0,
            cap: 8,
            radius: START_RADIUS,
            unrest: 0.0,
            hp: 1.0,
            spawn_t: 0.0,
            settler_t: 8.0,
            founded: self.time,
            siege: 0.0,
            besieger: NO_F,
            empty_t: 0.0,
            built: 0,
            fx: Fx::default(),
            goods: 0.0,
            goats: 0.0,
            faith: 0.2,
            plague: 0.0,
            linked_t: -1e9,
            camp: 0,
            side: 0,
            mourn_t: 0.0,
            festival_t: 0.0,
            clan,
            died: 0.0,
            coast,
            hills,
            raided_t: -1e9,
        };
        if reuse.is_some() {
            self.villages[id as usize] = village;
        } else {
            self.villages.push(village);
        }
        self.villages[id as usize].cap = self.village_cap(id);
        self.claim_land(id, START_RADIUS);
        self.stats.founded += 1;
        if log {
            self.log(ev::FOUNDED, f, NO_F, x, y, self.factions[f as usize].villages as i32 + 1);
        }
        id
    }

    pub fn village_cap(&self, v: u16) -> u16 {
        let vil = &self.villages[v as usize];
        let f = &self.factions[vil.owner as usize];
        let capital = f.capital == v;
        (6.0 + 4.0 * f.traits.fertility + if capital { 4.0 } else { 0.0 } + (vil.radius - START_RADIUS) as f32 * 0.5 + vil.fx.cap).round() as u16
    }

    /// Claim unowned land (and land of this civilisation's dead villages) within `r` tiles.
    pub fn claim_land(&mut self, v: u16, r: i32) {
        let (vx, vy, f) = {
            let vil = &self.villages[v as usize];
            (vil.x, vil.y, vil.owner)
        };
        for oy in -r..=r {
            let y = vy + oy;
            if !self.d.in_y(y) {
                continue;
            }
            for ox in -r..=r {
                if ox * ox + oy * oy > r * r + r {
                    continue;
                }
                let i = self.d.idx(vx + ox, y);
                if !self.terrain.walk[i] {
                    continue;
                }
                let c = self.claim[i];
                let free = c == NO_V || !self.villages[c as usize].alive;
                if free {
                    self.claim[i] = v;
                    self.owner[i] = f;
                }
            }
        }
    }

    /// Hand all of a village's land to a new owner (capture, rebellion).
    pub(crate) fn transfer_land(&mut self, v: u16, to: u8) {
        let (vx, vy, r) = {
            let vil = &self.villages[v as usize];
            (vil.x, vil.y, vil.radius + 1)
        };
        for oy in -r..=r {
            let y = vy + oy;
            if !self.d.in_y(y) {
                continue;
            }
            for ox in -r..=r {
                let i = self.d.idx(vx + ox, y);
                if self.claim[i] == v {
                    self.owner[i] = to;
                }
            }
        }
    }

    /// Clear a dead village's land.
    pub(crate) fn release_land(&mut self, v: u16) {
        let (vx, vy, r) = {
            let vil = &self.villages[v as usize];
            (vil.x, vil.y, vil.radius + 1)
        };
        for oy in -r..=r {
            let y = vy + oy;
            if !self.d.in_y(y) {
                continue;
            }
            for ox in -r..=r {
                let i = self.d.idx(vx + ox, y);
                if self.claim[i] == v {
                    self.owner[i] = NO_F;
                    self.claim[i] = NO_V;
                }
            }
        }
    }

    pub fn spawn_agent(&mut self, f: u8, home: u16, job: Job, x: i32, y: i32) -> Option<usize> {
        if self.agents.len() >= self.cap_agents {
            return None;
        }
        let life = self.rng.range(75.0, 115.0);
        let timer = self.rng.range(0.0, 0.6);
        let x = self.d.wrap_x(x);
        let y = self.d.wrap_y(y);
        // Born into a family of the village: mostly its founding clan, sometimes another
        // (marriage); with a share of the goats the village's dead left behind.
        let (clan, family, goats) = match self.villages.get(home as usize) {
            Some(v) if v.alive => {
                let clans = self.factions[f as usize].clans.max(1);
                let clan = if self.rng.chance(0.72) { v.clan } else { self.rng.below(clans as usize) as u8 };
                let family = ((home as u32) << 3) | self.rng.below(4) as u32;
                (clan, family, (v.goats * 0.1).min(3.0))
            }
            _ => (0, 0, 0.0),
        };
        if goats > 0.0 {
            self.villages[home as usize].goats -= goats;
        }
        self.agents.push(Agent {
            id: self.next_id,
            x: x as f32 + 0.5,
            y: y as f32 + 0.5,
            nx: NO_TILE,
            ny: NO_TILE,
            px: x,
            py: y,
            gx: x,
            gy: y,
            best: i32::MAX,
            stuck: 0,
            faction: f,
            job,
            state: State::Idle,
            home,
            target: NO_V,
            food: 1.0,
            hp: 1.0,
            age: 0.0,
            life,
            carry: 0.0,
            timer,
            fight: 0.0,
            morale: 0.6 + 0.3 * self.factions[f as usize].traits.cohesion,
            wound: Wound::Healthy,
            garrison: false,
            engage: None,
            alive: true,
            cause: Cause::Alive,
            tries: 0,
            clan,
            family,
            camp: 0,
            side: 0,
            goats,
            brawl: 0.0,
            prisoner: 0.0,
            origin: f,
        });
        self.next_id += 1;
        self.stats.births += 1;
        Some(self.agents.len() - 1)
    }

    pub fn effect(&mut self, x: f32, y: f32, kind: u8, ttl: f32, r: f32) {
        if self.effects.len() < 400 {
            self.effects.push(Effect { x, y, t: 0.0, ttl, r, kind });
        }
    }

    /// Run `real_dt` seconds of real time at the world's speed (×1, ×4, ×16), in steps of at
    /// most `MAX_SUBSTEP`; refresh the buffers JS reads once at the end.
    pub fn advance(&mut self, real_dt: f32) {
        // Seeding a civilisation from the mind runs on real time (#98).
        self.setup_tick(real_dt.max(0.0));
        let total = (real_dt.max(0.0) * self.speed).min(MAX_SUBSTEP * MAX_STEPS_PER_ADVANCE as f32);
        if total <= 0.0 {
            self.refresh_info();
            return;
        }
        let n = (total / MAX_SUBSTEP).ceil().max(1.0);
        for _ in 0..n as usize {
            self.step_core(total / n);
        }
        self.last_steps = n as u32;
        self.refresh_info();
    }

    /// Set the time speed (clamped to 0.25..32; the sandbox offers 1, 4 and 16).
    pub fn set_speed(&mut self, k: f32) {
        self.speed = if k.is_finite() { k.clamp(0.25, 32.0) } else { 1.0 };
    }

    /// Advance the world by `dt` seconds (clamped to 0.1).
    pub fn step(&mut self, dt: f32) {
        self.step_core(dt);
        self.refresh_info();
    }

    fn step_core(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        if dt <= 0.0 {
            return;
        }
        self.time += dt;
        self.tick = self.tick.wrapping_add(1);
        // Rival civilisations arrive on their own, at staggered times (#101).
        if !self.setup.rivals.is_empty() {
            self.rivals_tick();
        }
        let phase = (self.time / DAY_SECONDS).fract();
        self.night = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
        self.rain_t = (self.rain_t - dt).max(0.0);
        self.paths.refill(2);
        self.sea.refill(1);
        self.update_fires(dt);
        self.update_effects(dt);
        #[cfg(not(target_arch = "wasm32"))]
        let t0 = std::time::Instant::now();
        crate::agents::update(self, dt);
        #[cfg(not(target_arch = "wasm32"))]
        let t1 = std::time::Instant::now();
        self.grid.build(&self.agents);
        crate::agents::combat(self, dt);
        #[cfg(not(target_arch = "wasm32"))]
        let t2 = std::time::Instant::now();
        self.slow_acc += dt;
        while self.slow_acc >= SLOW_DT / 2.0 {
            self.slow_acc -= SLOW_DT / 2.0;
            #[cfg(not(target_arch = "wasm32"))]
            let p0 = std::time::Instant::now();
            #[cfg(not(target_arch = "wasm32"))]
            let ph = (self.slow_n % 4) as usize;
            crate::politics::phase_tick(self);
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.prof[3 + ph] += p0.elapsed().as_secs_f64() * 1000.0;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.prof[0] += (t1 - t0).as_secs_f64() * 1000.0;
            self.prof[1] += (t2 - t1).as_secs_f64() * 1000.0;
        }
        self.agents.retain(|a| a.alive);
        self.stats.peak_agents = self.stats.peak_agents.max(self.agents.len() as u32);
    }

    /// Rebuild the neighbour grid from the agents (tests and tools that run `agents::combat` alone).
    pub fn grid_rebuild(&mut self) {
        self.grid.build(&self.agents);
    }

    fn update_effects(&mut self, dt: f32) {
        for e in self.effects.iter_mut() {
            e.t += dt;
        }
        self.effects.retain(|e| e.t < e.ttl);
    }

    fn update_fires(&mut self, dt: f32) {
        self.update_regrowth();
        if self.fires.is_empty() && self.scorched.is_empty() {
            return;
        }
        let rain = self.rain_t > 0.0;
        let mut new: Vec<(u32, u8)> = Vec::new();
        for k in 0..self.fires.len() {
            self.fires[k].ttl -= dt * if rain { 5.0 } else { 1.0 };
            if rain || self.fires.len() + new.len() >= 700 {
                continue;
            }
            let gen = self.fires[k].gen;
            if self.rng.chance(dt * FIRE_SPREAD * FIRE_FALLOFF.powi(gen as i32)) {
                let fi = self.fires[k].i as usize;
                let (x, y) = ((fi % self.d.w) as i32, (fi / self.d.w) as i32);
                let k = self.rng.below(8);
                let (ox, oy) = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)][k];
                let ny = y + oy;
                if self.d.in_y(ny) {
                    let j = self.d.idx(x + ox, ny);
                    if self.burning[j] == 0 && terrain::flammable(self.terrain.kind[j]) && self.food.amt[j] > 0.05 {
                        new.push((j as u32, gen.saturating_add(1)));
                    }
                }
            }
        }
        for (j, gen) in new {
            if self.fires.len() < 700 && self.burning[j as usize] == 0 {
                self.ignite_tile_gen(j as usize, gen);
            }
        }
        let now = self.time;
        let mut out = Vec::new();
        self.fires.retain(|f| {
            if f.ttl <= 0.0 {
                out.push(f.i);
                false
            } else {
                true
            }
        });
        for i in out {
            self.burning[i as usize] = 0;
            self.scorched.push((i, now + 25.0));
            // A burnt forest is a clearing until trees beside it seed it again.
            if self.terrain.kind[i as usize] == terrain::FOREST {
                self.terrain.kind[i as usize] = terrain::GRASS;
                let due = now + self.rng.range(30.0, 50.0);
                self.regrow.push((i, due));
            }
        }
        // Burnt land greens up again.
        if !self.scorched.is_empty() {
            let mut back = Vec::new();
            self.scorched.retain(|&(i, until)| {
                if now >= until {
                    back.push(i);
                    false
                } else {
                    true
                }
            });
            for i in back {
                let i = i as usize;
                if self.burning[i] == 0 {
                    let c = self.terrain.base_colour((i % self.d.w) as i32, (i / self.d.w) as i32);
                    self.terrain.rgba[i * 4..i * 4 + 4].copy_from_slice(&c);
                }
            }
        }
    }

    pub fn ignite_tile(&mut self, i: usize) {
        self.ignite_tile_gen(i, 0);
    }

    fn ignite_tile_gen(&mut self, i: usize, gen: u8) {
        if self.burning[i] != 0 || !terrain::flammable(self.terrain.kind[i]) {
            return;
        }
        self.burning[i] = 1;
        // Trees smoulder a while; grass flashes over.
        let ttl = if self.terrain.kind[i] == terrain::FOREST { self.rng.range(4.0, 7.0) } else { self.rng.range(1.5, 3.0) };
        self.fires.push(Fire { i: i as u32, ttl, gen });
        self.food.set(i, 0.0, self.time);
        let c = [44u8, 34, 30, 255];
        self.terrain.rgba[i * 4..i * 4 + 4].copy_from_slice(&c);
    }

    /// A woodcutter fells the tree on tile `i`: the forest becomes a clearing, which grows back
    /// slowly from the trees around it (unless the village cuts it again first).
    pub fn fell_tree(&mut self, i: usize) {
        if self.terrain.kind[i] != terrain::FOREST || self.burning[i] != 0 {
            return;
        }
        self.terrain.kind[i] = terrain::GRASS;
        self.repaint(i);
        let due = self.time + self.rng.range(60.0, 100.0);
        self.regrow.push((i as u32, due));
    }

    fn repaint(&mut self, i: usize) {
        let c = self.terrain.base_colour((i % self.d.w) as i32, (i / self.d.w) as i32);
        self.terrain.rgba[i * 4..i * 4 + 4].copy_from_slice(&c);
    }

    /// Cleared forest grows back where trees stand beside it: two or more neighbouring trees and
    /// it regrows; one, and it tries again later; none, and it stays open land.
    fn update_regrowth(&mut self) {
        if self.regrow.is_empty() {
            return;
        }
        let now = self.time;
        if !self.regrow.iter().any(|&(_, t)| now >= t) {
            return;
        }
        let due: Vec<(u32, f32)> = self.regrow.iter().copied().filter(|&(_, t)| now >= t).collect();
        self.regrow.retain(|&(_, t)| now < t);
        for (i, _) in due {
            let i = i as usize;
            if self.burning[i] != 0 || self.terrain.kind[i] != terrain::GRASS {
                continue;
            }
            let (x, y) = ((i % self.d.w) as i32, (i / self.d.w) as i32);
            let mut trees = 0;
            for (ox, oy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                if self.terrain.kind_at(x + ox, y + oy) == terrain::FOREST {
                    trees += 1;
                }
            }
            if trees >= 2 {
                self.terrain.kind[i] = terrain::FOREST;
                self.repaint(i);
            } else if trees == 1 {
                let again = now + self.rng.range(30.0, 60.0);
                self.regrow.push((i as u32, again));
            }
        }
    }

    /// Tally villages and civilisations from the agents.
    pub fn recount(&mut self) {
        for v in self.villages.iter_mut() {
            v.pop = 0;
            v.warriors = 0;
            v.hungry = 0;
        }
        let mut hungry = [0u32; MAX_FACTIONS];
        for f in self.factions.iter_mut() {
            f.pop = 0;
            f.villages = 0;
            f.warriors = 0;
            f.food = 0.0;
            f.cat_counts = [0; CATS];
        }
        for a in self.agents.iter().filter(|a| a.alive) {
            let f = &mut self.factions[a.faction as usize];
            f.pop += 1;
            let fights = a.job.fights() && a.job != Job::Hunt;
            if a.job.fights() {
                f.warriors += 1;
            }
            f.cat_counts[crate::jobs::cat_of(a.job, f.age) as usize] += 1;
            if a.food < 0.25 {
                hungry[a.faction as usize] += 1;
            }
            if let Some(v) = self.villages.get_mut(a.home as usize) {
                if v.alive && v.owner == a.faction {
                    v.pop += 1;
                    if fights {
                        v.warriors += 1;
                    }
                    if a.food < 0.25 {
                        v.hungry += 1;
                    }
                }
            }
        }
        for v in self.villages.iter().filter(|v| v.alive) {
            let f = &mut self.factions[v.owner as usize];
            f.villages += 1;
            f.food += v.food;
        }
        for (i, f) in self.factions.iter_mut().enumerate() {
            f.hungry = hungry[i] as f32 / f.pop.max(1) as f32;
        }
    }

    /// Nearest living village of civilisation `f` to (x, y), on the same landmass if possible.
    pub fn nearest_village(&self, f: u8, x: f32, y: f32) -> Option<u16> {
        let land = self.terrain.land(x as i32, y as i32);
        let mut best: Option<(bool, f32, u16)> = None;
        for (i, v) in self.villages.iter().enumerate() {
            if !v.alive || v.owner != f {
                continue;
            }
            let same = land != 0 && self.terrain.land(v.x, v.y) == land;
            let d = self.d.dist(x, y, v.x as f32 + 0.5, v.y as f32 + 0.5);
            let better = match best {
                None => true,
                Some((bs, bd, _)) => (same && !bs) || (same == bs && d < bd),
            };
            if better {
                best = Some((same, d, i as u16));
            }
        }
        best.map(|b| b.2)
    }

    /// Kill an agent (it is removed at the end of the step).
    pub fn kill(&mut self, i: usize, cause: Cause) {
        let a = &mut self.agents[i];
        if !a.alive {
            return;
        }
        a.alive = false;
        a.cause = cause;
        let f = a.faction as usize;
        let (home, goats, chief) = (a.home, a.goats, a.job == Job::Chief);
        self.stats.deaths += 1;
        if let Some(v) = self.villages.get_mut(home as usize) {
            if v.alive {
                v.goats += goats;
                // The village mourns a violent or untimely death.
                if !matches!(cause, Cause::OldAge) {
                    v.mourn_t = v.mourn_t.max(8.0);
                    v.faith = (v.faith + 0.01).min(1.0);
                }
            }
        }
        if chief {
            self.factions[f].succession = true;
        }
        match cause {
            Cause::Starved => {
                self.stats.starved += 1;
                self.factions[f].starved_recent += 1.0;
            }
            Cause::OldAge => self.stats.old_age += 1,
            Cause::Combat => {
                self.stats.killed += 1;
                self.factions[f].losses += 1;
                self.factions[f].recent_losses += 1.0;
            }
            Cause::Fire => self.stats.burned += 1,
            Cause::Smitten | Cause::Disaster => self.stats.smitten += 1,
            Cause::Plague => {}
            Cause::Alive => {}
        }
    }

    pub fn centre_camera(&mut self, x: f32, y: f32) {
        self.cam.centre(x, y);
    }

    pub fn pan_camera(&mut self, dx: f32, dy: f32) {
        self.cam.pan(dx, dy);
    }

    /// The four starting civilisations' camera point: the player's capital.
    pub fn player(&self) -> Option<u8> {
        self.factions.iter().find(|f| f.is_player).map(|f| f.id)
    }

    /// Refresh the flat buffers JS reads.
    pub fn refresh_info(&mut self) {
        for (i, f) in self.factions.iter().enumerate() {
            let o = i * FSTRIDE;
            let cap = self.villages.get(f.capital as usize).filter(|v| v.alive && v.owner == f.id);
            let (cx, cy) = cap.map(|v| (v.x as f32, v.y as f32)).unwrap_or((-1.0, -1.0));
            let wars = (0..MAX_FACTIONS).filter(|&b| self.rel[i * MAX_FACTIONS + b].war == 2).count();
            let raids = (0..MAX_FACTIONS).filter(|&b| self.rel[i * MAX_FACTIONS + b].war == 1).count();
            let unrest = {
                let (mut s, mut n) = (0.0, 0.0);
                for v in self.villages.iter().filter(|v| v.alive && v.owner == f.id) {
                    s += v.unrest;
                    n += 1.0;
                }
                if n > 0.0 {
                    s / n
                } else {
                    0.0
                }
            };
            let vals = [
                if f.alive {
                    1.0
                } else if f.pending {
                    2.0
                } else if f.used {
                    -1.0
                } else {
                    0.0
                },
                f.pop as f32,
                f.villages as f32,
                f.color[0] as f32,
                f.color[1] as f32,
                f.color[2] as f32,
                cx,
                cy,
                f.traits.friendliness,
                f.traits.aggressiveness,
                f.traits.violence,
                f.traits.fertility,
                f.traits.cohesion,
                if f.parent == NO_F { -1.0 } else { f.parent as f32 },
                wars as f32,
                raids as f32,
                f.food,
                f.kills as f32,
                f.losses as f32,
                unrest,
                f.seed_source as u8 as f32,
                f.region as f32,
                if f.is_player { 1.0 } else { 0.0 },
                f.warriors as f32,
                f.age as f32,
                f.age_t,
                f.metrics.stability,
                f.metrics.knowledge,
                f.metrics.wealth,
                f.metrics.faith,
                f.metrics.production,
                f.metrics.links as f32,
                f.metrics.territory,
                f.metrics.inequality,
                f.metrics.vassals as f32,
                f.metrics.colonies as f32,
                f.dark_t,
                if f.overlord == NO_F { -1.0 } else { f.overlord as f32 },
                self.strifes.iter().filter(|s| s.alive && s.civ == f.id).map(|s| s.level).max().unwrap_or(0) as f32,
                self.fight_level[i] as f32,
                f.peak_age as f32,
                f.metrics.food_days,
                f.metrics.coastal as u8 as f32,
                f.metrics.contact as u8 as f32,
                f.falls as f32,
                f.ready_t,
            ];
            self.info_f[o..o + FSTRIDE].copy_from_slice(&vals);
            let nb = &mut self.names[i * NAME_LEN..(i + 1) * NAME_LEN];
            nb.iter_mut().for_each(|b| *b = 0);
            for (k, b) in f.name.bytes().take(NAME_LEN - 1).enumerate() {
                nb[k] = b;
            }
        }
        self.info_v.clear();
        for v in &self.villages {
            let capital = v.alive && self.factions[v.owner as usize].capital as usize == self.info_v.len() / VSTRIDE;
            self.info_v.extend_from_slice(&[
                v.x as f32,
                v.y as f32,
                v.owner as f32,
                v.pop as f32,
                v.food,
                v.unrest,
                if capital { 1.0 } else { 0.0 },
                if !v.alive { 0.0 } else if v.siege > 0.0 { 2.0 } else { 1.0 },
                v.built.count_ones() as f32,
                v.plague,
                if v.camp == 0 { 0.0 } else { 1.0 + v.side as f32 },
                if v.festival_t > 0.0 { 1.0 } else if v.mourn_t > 0.0 { -1.0 } else { 0.0 },
            ]);
        }
        let s = &self.stats;
        let vals = [
            self.time,
            self.agents.len() as f32,
            self.villages.iter().filter(|v| v.alive).count() as f32,
            s.wars as f32,
            s.peaces as f32,
            s.raids as f32,
            s.captures as f32,
            s.rebellions as f32,
            s.contacts as f32,
            s.wiped as f32,
            s.founded as f32,
            s.births as f32,
            s.deaths as f32,
            s.starved as f32,
            s.old_age as f32,
            s.killed as f32,
            s.trades as f32,
            self.paths.builds as f32,
            self.alive_factions().count() as f32,
            self.fires.len() as f32,
            self.rain_t,
            s.peak_agents as f32,
            s.burned as f32 + s.smitten as f32,
            self.agents.iter().filter(|a| a.job.fights()).count() as f32,
            s.age_ups as f32,
            s.collapses as f32,
            s.recoveries as f32,
            s.strifes as f32,
            s.civil_wars as f32,
            s.crushed as f32,
            s.revolutions as f32,
            s.escalations as f32,
            s.battles as f32,
            s.captives as f32,
            s.vassals as f32,
            s.plagues as f32,
            s.crises as f32,
            s.festivals as f32,
            s.brawls as f32,
            s.routs as f32,
            s.max_age as f32,
            self.speed,
            self.night,
            s.buildings as f32,
        ];
        self.stats_buf.copy_from_slice(&vals);
    }

    /// A checksum of the state, for determinism tests.
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 1469598103934665603;
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(1099511628211);
        };
        for a in &self.agents {
            mix(a.x.to_bits() as u64);
            mix(a.y.to_bits() as u64);
            mix(a.faction as u64);
            mix(a.job as u64);
        }
        for v in &self.villages {
            mix(v.owner as u64);
            mix(v.food.to_bits() as u64);
        }
        for e in &self.events {
            mix(e.kind as u64);
            mix(e.t_ms as u64);
        }
        mix(self.time.to_bits() as u64);
        h
    }

    /// Distance between two villages, the short way round.
    pub fn village_dist(&self, a: u16, b: u16) -> f32 {
        let (va, vb) = (&self.villages[a as usize], &self.villages[b as usize]);
        self.d.dist(va.x as f32, va.y as f32, vb.x as f32, vb.y as f32)
    }

    /// Rebel name for a split from `parent`.
    pub(crate) fn rebel_name(&mut self, parent: u8) -> (String, String) {
        let stem = self.factions[parent as usize].stem.clone();
        for _ in 0..12 {
            let p = *self.rng.pick(&REBEL_PREFIX);
            let name = format!("{} {}", p, stem);
            if !self.factions.iter().any(|f| f.alive && f.name == name) {
                return (name, stem);
            }
        }
        let seed = self.rng.next_u32();
        make_name(seed)
    }
}
