//! Interplanetary war (#166): space-capable worlds build fleets and attack other planets.
//!
//! **Bodies and powers.** Every planet in the galaxy is a [`Body`]; the host (the Rust
//! [`crate::galaxy::Galaxy`], or the browser's galaxy through `RustGalaxy`) tells the war what
//! each one is like every frame ([`WorldStats`]). Every inhabited planet is ruled by a [`Power`]:
//! its own civilisation (`PowerKind::World`), the player's campaign world (`PowerKind::Player`),
//! or, on the civ sandbox's shared planet, one of its civilisations (`PowerKind::Civ`, whose
//! fleet and defences come from `moc-civ`'s `space.rs` through the host). A power that conquers
//! a planet rules it as a colony; a power that loses every planet falls.
//!
//! **Fleets.** A space-capable power (Space Age, or a civilisation at age 19+) builds a fleet
//! from its economy: production, knowledge and resources ([`Economy`]), summed over its home
//! and colonies. Fleets fly between planets as [`Ship`]s carrying a [`Mission`] and fight on
//! arrival:
//! - **raid** ([`Order::Raid`]): slip past the defences, steal stores and knowledge;
//! - **orbital bombardment** ([`Order::Bombard`]): burn through shields and orbital defences,
//!   then hit the surface, its people and its progress;
//! - **invasion** ([`Order::Invade`]): win the orbit, land troops, beat the garrison and take the
//!   planet. The hand-played campaign home and the civ sandbox's shared planet are never taken:
//!   a won invasion there is a beachhead that wrecks and loots instead.
//!
//! Defenders have **shields** (from knowledge), **orbital defences** (from production) and a
//! **garrison** (from population); all three regrow toward caps set by the ruler's economy.
//! Outcomes follow strength against defence, with a little luck.
//!
//! **Councils.** Once a second every AI power decides through a [`SpacePolicy`] (the rules,
//! [`Rules`], by default; a trained policy can replace them without touching anything else).
//! **The player's mind tilts every council:** stress makes leaders reckless (more launches,
//! invasions, longer grudges); calm makes them defensive and diplomatic (shields, truces).
//! **Difficulty seam:** [`SpaceWar::set_player_pressure`] scales how often AI councils launch at
//! the player's world and how hard those fleets hit it (1 Normal; Easy about 0.5, Hard about 1.6).
//!
//! **News.** Every launch at the player and every battle writes a line saying who attacked whom
//! and why ([`Motive`], plus the mind's part), for the galaxy log.
//!
//! Deterministic for a seed and a sequence of syncs, steps and orders.

use crate::galaxy::Ship;
use crate::math::{clamp, Mulberry32};

/// The era from which a campaign world is space-capable (`planet_sim::ERA_TITLES[3]`).
pub const SPACE_ERA: u8 = 3;
/// Powers ever created (slots are never reused, so fleets in flight always know their owner).
pub const MAX_POWERS: usize = 64;
/// Fleets in flight at once.
pub const MAX_FLEETS: usize = 64;
/// Seconds between councils (decisions, ownership, economy).
pub const COUNCIL_EVERY: f32 = 1.0;
/// News lines kept for the host to read.
pub const NEWS_KEEP: usize = 48;
/// Seconds a battle stays on the galaxy view.
pub const BATTLE_SECONDS: f32 = 2.8;

/// Fleet cap per point of economy index (before tiers).
pub const FLEET_PER_ECON: f32 = 6.0;
/// Share of the fleet cap built per second.
pub const FLEET_BUILD: f32 = 0.035;
/// Shields per point of knowledge, orbital defences per point of production (before tiers).
pub const SHIELD_PER_KNOW: f32 = 2.5;
pub const DEFENCE_PER_PROD: f32 = 1.5;
/// Garrison: a base plus this many per √(thousand people).
pub const GARRISON_BASE: f32 = 4.0;
pub const GARRISON_PER_KPOP: f32 = 5.0;
/// The campaign home is hand-played: its garrison never drops below this.
pub const HAND_PLAYED_GARRISON: f32 = 15.0;
/// Shares of shields, defences and the regrowth rate per second.
pub const SHIELD_REGEN: f32 = 0.08;
pub const DEFENCE_REGEN: f32 = 0.04;
pub const GARRISON_REGEN: f32 = 0.03;
/// Colonies get this share of their ruler's shields and defences; the ruler gets this share
/// of a colony's economy.
pub const COLONY_DEFENCE: f32 = 0.5;
pub const COLONY_SHARE: f32 = 0.6;
/// Defenders fight harder on their own ground; harder still on the hand-played home.
pub const HOME_GROUND: f32 = 1.5;
pub const HAND_PLAYED_GROUND: f32 = 3.0;
/// The smallest fleet worth sending.
pub const MIN_LAUNCH: f32 = 4.0;
/// Share of the docked fleet the player sends on an order.
pub const PLAYER_COMMIT: f32 = 0.8;
/// Seconds a fleet takes between planets.
pub const TRAVEL_MIN: f32 = 5.0;
pub const TRAVEL_MAX: f32 = 8.0;
/// Seconds a truce lasts.
pub const TRUCE_SECONDS: f32 = 45.0;

/// What a fleet is sent to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Order {
    /// Steal stores and knowledge.
    Raid = 0,
    /// Burn shields, defences, people and progress from orbit.
    Bombard = 1,
    /// Land troops and take the planet.
    Invade = 2,
}

impl Order {
    pub const ALL: [Order; 3] = [Order::Raid, Order::Bombard, Order::Invade];

    pub fn from_u8(v: u8) -> Option<Order> {
        Self::ALL.get(v as usize).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Order::Raid => "raid",
            Order::Bombard => "orbital bombardment",
            Order::Invade => "invasion",
        }
    }

    /// Share of its docked fleet an AI power sends on this order.
    pub fn commit(self) -> f32 {
        match self {
            Order::Raid => 0.5,
            Order::Bombard => 0.6,
            Order::Invade => 0.85,
        }
    }
}

/// Who a power is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PowerKind {
    /// The player's campaign world: never attacks on its own, only on the player's orders.
    Player = 0,
    /// A galaxy world's own civilisation.
    World = 1,
    /// A civilisation of the civ sandbox (`moc-civ`), on the shared planet.
    Civ = 2,
}

/// Why a power attacked (for the news).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Motive {
    Plunder = 0,
    Rivalry = 1,
    Revenge = 2,
    Expansion = 3,
    Coalition = 4,
    Ambition = 5,
    /// The player ordered it.
    Command = 6,
}

impl Motive {
    pub fn from_u8(v: u8) -> Motive {
        match v {
            1 => Motive::Rivalry,
            2 => Motive::Revenge,
            3 => Motive::Expansion,
            4 => Motive::Coalition,
            5 => Motive::Ambition,
            6 => Motive::Command,
            _ => Motive::Plunder,
        }
    }

    /// "because …", about the attacker and the target.
    pub fn why(self, target: &str) -> String {
        match self {
            Motive::Plunder => format!("it wants {target}'s stores"),
            Motive::Rivalry => format!("it fears {target}'s growing fleet"),
            Motive::Revenge => format!("it remembers {target}'s attacks"),
            Motive::Expansion => "it wants more worlds".to_string(),
            Motive::Coalition => format!("the coalition is out to stop {target}"),
            Motive::Ambition => "it is reaching for galactic dominance".to_string(),
            Motive::Command => "you gave the order".to_string(),
        }
    }
}

/// News line tones (as `moc-civ`'s news): info, good, bad, warning.
pub const TONE_INFO: u8 = 0;
pub const TONE_GOOD: u8 = 1;
pub const TONE_BAD: u8 = 2;
pub const TONE_WARN: u8 = 3;

/// What the host says about a planet each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldStats {
    /// Formed and on the map (false while it forms).
    pub alive: bool,
    pub pop: f32,
    /// 0 Stone Age … 3 Space Age.
    pub era: u8,
    /// Settlement tier, 0 camp … 3 city.
    pub tier: u8,
    /// Research toward the next era, 0..1 (a Space Age world keeps it full).
    pub progress: f32,
    /// Stores: food + wood + stone + goats.
    pub stock: f32,
    /// 0..1.
    pub stability: f32,
    /// 0 dove … 1 hawk: how its leaders lean (a stress world runs hot, a calm one cool).
    pub temper: f32,
    /// The campaign's hand-played home: its scene can't be hurt from here, only its orbit.
    pub hand_played: bool,
}

/// A power's economy, in strength units: a Space Age city of 12k people is about 10 in each.
/// Seam (#165): a civilisation's resources feed `resources` (see `moc-civ` `space::economy`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Economy {
    pub production: f32,
    pub knowledge: f32,
    pub resources: f32,
}

/// The hand-played campaign home's economy per era (its scene has few people on screen, but it
/// stands for a whole civilisation).
const HAND_PLAYED_FLOOR: [Economy; 4] = [
    Economy { production: 1.0, knowledge: 1.0, resources: 1.0 },
    Economy { production: 2.0, knowledge: 2.5, resources: 2.0 },
    Economy { production: 5.0, knowledge: 5.0, resources: 4.0 },
    Economy { production: 11.0, knowledge: 10.0, resources: 8.0 },
];

impl Economy {
    /// A world's economy from what the host says about it.
    pub fn of_world(s: &WorldStats) -> Economy {
        if !s.alive || s.pop <= 0.0 {
            return Economy::default();
        }
        let kpop = (s.pop.max(0.0) / 1000.0).sqrt();
        let stab = clamp(s.stability, 0.0, 1.0);
        let mut e = Economy {
            production: kpop * 2.5 * (1.0 + 0.2 * s.tier as f32) * (0.5 + 0.5 * stab),
            knowledge: (s.era.min(SPACE_ERA) as f32 + clamp(s.progress, 0.0, 1.0)) * 2.5,
            resources: s.stock.max(0.0).sqrt() * 0.2,
        };
        if s.hand_played {
            let f = HAND_PLAYED_FLOOR[s.era.min(SPACE_ERA) as usize];
            e.production = e.production.max(f.production);
            e.knowledge = e.knowledge.max(f.knowledge);
            e.resources = e.resources.max(f.resources);
        }
        e
    }

    /// One number for strength: production counts most, then knowledge, then resources.
    pub fn index(&self) -> f32 {
        0.45 * self.production + 0.35 * self.knowledge + 0.2 * self.resources
    }

    pub fn add_scaled(&mut self, o: &Economy, k: f32) {
        self.production += o.production * k;
        self.knowledge += o.knowledge * k;
        self.resources += o.resources * k;
    }

    pub fn scaled(&self, k: f32) -> Economy {
        Economy { production: self.production * k, knowledge: self.knowledge * k, resources: self.resources * k }
    }
}

/// Multipliers an endgame tier gives (see `crate::endgame`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TierFx {
    pub fleet: f32,
    pub shields: f32,
    pub econ: f32,
}

/// The player's mind this step, all 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mind {
    pub calm: f32,
    pub focus: f32,
    pub stress: f32,
}

impl Mind {
    /// −1 (calm: defensive, diplomatic) … +1 (stress: reckless, aggressive).
    pub fn hostility(&self) -> f32 {
        clamp(1.1 * clamp(self.stress, 0.0, 1.0) - 0.9 * clamp(self.calm, 0.0, 1.0), -1.0, 1.0)
    }

    /// 0..1: how much the calm makes leaders dig in and talk.
    pub fn defensiveness(&self) -> f32 {
        clamp(-self.hostility(), 0.0, 1.0)
    }
}

/// Shields, orbital defences and garrison of one target.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Defence {
    pub shields: f32,
    pub defences: f32,
    pub garrison: f32,
}

impl Defence {
    pub fn orbit(&self) -> f32 {
        self.shields + self.defences
    }
    pub fn total(&self) -> f32 {
        self.shields + self.defences + self.garrison
    }
}

/// What the war did to a planet since the host last took it ([`SpaceWar::take_fx`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyFx {
    /// Multiply the population by this.
    pub pop_mul: f32,
    /// Take this much research progress (0..1 of an era).
    pub progress_loss: f32,
    /// Multiply the stores by this.
    pub stock_mul: f32,
    /// Power that took the planet (−1 none).
    pub captured_by: i32,
    /// Colonists landed on a barren world (the host settles it).
    pub settled: bool,
}

impl Default for BodyFx {
    fn default() -> Self {
        BodyFx { pop_mul: 1.0, progress_loss: 0.0, stock_mul: 1.0, captured_by: -1, settled: false }
    }
}

impl BodyFx {
    pub fn is_empty(&self) -> bool {
        *self == BodyFx::default()
    }
}

/// A planet in the war.
#[derive(Clone, Debug)]
pub struct Body {
    pub name: String,
    pub stats: WorldStats,
    pub econ: Economy,
    /// The power ruling it (None: barren, unclaimed, or the shared civ planet).
    pub owner: Option<u16>,
    /// Several powers live here (the civ sandbox's planet): fleets target one of them.
    pub shared: bool,
    pub shields: f32,
    pub defences: f32,
    pub garrison: f32,
    pub fx: BodyFx,
    /// Seconds since a battle here (for the view).
    pub battle_t: f32,
}

impl Body {
    fn new(name: &str) -> Body {
        Body {
            name: name.to_string(),
            stats: WorldStats::default(),
            econ: Economy::default(),
            owner: None,
            shared: false,
            shields: 0.0,
            defences: 0.0,
            garrison: 0.0,
            fx: BodyFx::default(),
            battle_t: 1e9,
        }
    }

    pub fn inhabited(&self) -> bool {
        self.stats.alive && self.stats.pop > 0.0
    }
}

/// What happened to an external (civ) power since the host last took it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerResults {
    /// Stores and knowledge won by raids and invasions (strength units).
    pub loot: f32,
    pub knowledge: f32,
    /// Share of its people lost to bombardment and beachheads.
    pub hits: f32,
    /// Shields and defences knocked out (strength units).
    pub shield_loss: f32,
    /// Planets gained (+) or lost (−).
    pub colonies: i32,
    /// Ships that came home (strength units), to dock again.
    pub returned: f32,
}

/// Strength and defence of an external power (a civ sandbox civilisation), set by the host.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct External {
    pub fleet: f32,
    pub defence: Defence,
    /// Its own economy (its colonies' share is added by the war).
    pub econ: Economy,
}

/// A space-faring civilisation.
#[derive(Clone, Debug)]
pub struct Power {
    pub id: u16,
    pub kind: PowerKind,
    pub name: String,
    pub color: [u8; 3],
    /// Its capital planet.
    pub home: usize,
    /// The host's id for it (a planet index, or a civilisation's key).
    pub key: u32,
    pub alive: bool,
    /// Can build and launch fleets.
    pub space: bool,
    /// Its economy: home plus a share of its colonies', times its tier.
    pub econ: Economy,
    /// Ships in dock (strength), and the most its economy supports.
    pub fleet: f32,
    pub fleet_cap: f32,
    /// 0 dove … 1 hawk, and that lean tilted by the player's mind now.
    pub temper: f32,
    pub stance: f32,
    /// Seconds until it may launch again.
    pub cooldown: f32,
    /// Endgame tier (`crate::endgame::TIERS`) and progress toward the next.
    pub tier: u8,
    pub tier_xp: f32,
    /// In the coalition against the galaxy's leader.
    pub coalition: bool,
    /// Share of the galaxy's strength, 0..1.
    pub dominance: f32,
    /// Planets it rules.
    pub worlds: u32,
    /// Host-driven fleet and defences (civ powers).
    pub external: Option<External>,
    pub results: PowerResults,
    pub wins: u32,
    pub losses: u32,
    pub captures: u32,
}

impl Power {
    pub fn tier_fx(&self) -> TierFx {
        crate::endgame::tier_fx(self.tier)
    }
}

/// How power a feels about power b.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rel {
    /// Old wrongs, 0..: raised by attacks, decays.
    pub grudge: f32,
    /// Seconds of truce left.
    pub truce: f32,
}

/// What a fleet carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mission {
    pub id: u32,
    pub owner: u16,
    /// On a shared planet: the civilisation it is sent against.
    pub target_power: Option<u16>,
    pub order: Order,
    pub strength: f32,
    pub motive: Motive,
    /// Curve of the flight path, −0.4..0.4 (the view's).
    pub bend: f32,
}

/// One launch decided by a policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub target: usize,
    pub target_power: Option<u16>,
    pub order: Order,
    /// Share of the docked fleet to send, 0..1.
    pub commit: f32,
    pub motive: Motive,
}

/// A battle on the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battle {
    pub id: u32,
    pub body: usize,
    pub attacker: u16,
    pub order: Order,
    pub won: bool,
    pub t: f32,
}

/// A line of galaxy news.
#[derive(Clone, Debug, PartialEq)]
pub struct NewsItem {
    pub seq: u32,
    pub t: f32,
    pub tone: u8,
    pub text: String,
    /// About the player's own world.
    pub player: bool,
    /// About a civ sandbox civilisation (for the sandbox's own news).
    pub civ: bool,
}

/// A space council: given the war, what power `p` launches this second. The rules ([`Rules`])
/// are the default; a trained policy implements this and is set with [`SpaceWar::set_policy`].
pub trait SpacePolicy {
    fn decide(&mut self, war: &SpaceWar, p: u16, out: &mut Vec<Launch>);
}

/// The interplanetary war.
pub struct SpaceWar {
    pub seed: u32,
    pub time: f32,
    pub bodies: Vec<Body>,
    pub powers: Vec<Power>,
    /// MAX_POWERS² relations, row = who feels.
    pub rel: Vec<Rel>,
    /// War fleets in flight (each `Ship` carries a `Mission`).
    pub fleets: Vec<Ship>,
    pub battles: Vec<Battle>,
    pub news: Vec<NewsItem>,
    pub next_news: u32,
    pub mind: Mind,
    /// The campaign's home planet (the player's power lives there).
    pub player_home: Option<usize>,
    /// The galaxy's leader and whether a coalition stands against it (`crate::endgame`).
    pub leader: Option<u16>,
    pub coalition_against: Option<u16>,
    /// Seconds the coalition has stood.
    pub coalition_t: f32,
    /// A power that has held galactic dominance long enough (`crate::endgame`).
    pub dominant: Option<u16>,
    pub dominance_t: f32,
    pub councils: u32,
    /// How hard the AI powers press the player (difficulty seam): ×how often they launch at the
    /// player's world and ×how hard those fleets hit it. 1 is the default; Easy about 0.5, Hard
    /// about 1.6 ([`SpaceWar::set_player_pressure`]).
    pub player_pressure: f32,
    council_acc: f32,
    next_fleet: u32,
    next_battle: u32,
    rng: Mulberry32,
    policy: Option<Box<dyn SpacePolicy>>,
    launches: Vec<Launch>,
}

impl Clone for SpaceWar {
    /// A copy runs on the rules (a trained policy is not copied).
    fn clone(&self) -> SpaceWar {
        SpaceWar {
            seed: self.seed,
            time: self.time,
            bodies: self.bodies.clone(),
            powers: self.powers.clone(),
            rel: self.rel.clone(),
            fleets: self.fleets.clone(),
            battles: self.battles.clone(),
            news: self.news.clone(),
            next_news: self.next_news,
            mind: self.mind,
            player_home: self.player_home,
            leader: self.leader,
            coalition_against: self.coalition_against,
            coalition_t: self.coalition_t,
            dominant: self.dominant,
            dominance_t: self.dominance_t,
            councils: self.councils,
            player_pressure: self.player_pressure,
            council_acc: self.council_acc,
            next_fleet: self.next_fleet,
            next_battle: self.next_battle,
            rng: self.rng.clone(),
            policy: None,
            launches: Vec::new(),
        }
    }
}

impl std::fmt::Debug for SpaceWar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpaceWar")
            .field("time", &self.time)
            .field("bodies", &self.bodies.len())
            .field("powers", &self.powers.len())
            .field("fleets", &self.fleets.len())
            .field("leader", &self.leader)
            .finish()
    }
}

impl SpaceWar {
    pub fn new(seed: u32) -> SpaceWar {
        SpaceWar {
            seed,
            time: 0.0,
            bodies: Vec::new(),
            powers: Vec::new(),
            rel: vec![Rel::default(); MAX_POWERS * MAX_POWERS],
            fleets: Vec::new(),
            battles: Vec::new(),
            news: Vec::new(),
            next_news: 0,
            mind: Mind::default(),
            player_home: None,
            leader: None,
            coalition_against: None,
            coalition_t: 0.0,
            dominant: None,
            dominance_t: 0.0,
            councils: 0,
            player_pressure: 1.0,
            council_acc: 0.0,
            next_fleet: 1,
            next_battle: 0,
            rng: Mulberry32::new(seed ^ 0x5eed_0166),
            policy: None,
            launches: Vec::new(),
        }
    }

    /// Replace the rules with a trained policy (None: the rules again).
    pub fn set_policy(&mut self, p: Option<Box<dyn SpacePolicy>>) {
        self.policy = p;
    }

    /// The difficulty seam: how hard the AI powers press the player, 1 by default (Easy about
    /// 0.5, Hard about 1.6), clamped to 0.1..=3. It scales how often councils launch at the
    /// player's world (any [`SpacePolicy`]) and the strength those fleets hit it with; the ships
    /// themselves, and every other war, are untouched.
    pub fn set_player_pressure(&mut self, x: f32) {
        self.player_pressure = if x.is_finite() { clamp(x, 0.1, 3.0) } else { 1.0 };
    }

    /// The pressure multiplier for `attacker` striking `defender` (1 unless an AI power strikes
    /// the player).
    pub fn pressure_on(&self, attacker: u16, defender: u16) -> f32 {
        if self.is_player(defender) && !self.is_player(attacker) {
            self.player_pressure
        } else {
            1.0
        }
    }

    // ------------------------------------------------------------------ host input

    /// Tell the war about planet `i` (the host's index). Planets are added as they appear.
    pub fn sync_body(&mut self, i: usize, name: &str, stats: WorldStats) {
        while self.bodies.len() <= i {
            self.bodies.push(Body::new(""));
        }
        let b = &mut self.bodies[i];
        if b.name != name {
            b.name.clear();
            b.name.push_str(name);
        }
        b.stats = stats;
        b.econ = Economy::of_world(&stats);
        if stats.hand_played {
            b.garrison = b.garrison.max(HAND_PLAYED_GARRISON);
        }
    }

    /// The campaign's home planet: the player's power rules it.
    pub fn set_player_home(&mut self, i: Option<usize>) {
        self.player_home = i;
    }

    /// Planet `i` hosts several powers (the civ sandbox's planet).
    pub fn set_shared(&mut self, i: usize, shared: bool) {
        while self.bodies.len() <= i {
            self.bodies.push(Body::new(""));
        }
        self.bodies[i].shared = shared;
        if shared {
            self.bodies[i].owner = None;
        }
    }

    // ------------------------------------------------------------------ queries

    #[inline]
    pub fn rel(&self, a: u16, b: u16) -> &Rel {
        &self.rel[a as usize * MAX_POWERS + b as usize]
    }

    #[inline]
    pub fn rel_mut(&mut self, a: u16, b: u16) -> &mut Rel {
        &mut self.rel[a as usize * MAX_POWERS + b as usize]
    }

    /// The player's power, if the campaign home has one.
    pub fn player(&self) -> Option<u16> {
        self.powers.iter().find(|p| p.alive && p.kind == PowerKind::Player).map(|p| p.id)
    }

    /// The power ruling planet `i`.
    pub fn owner_of(&self, i: usize) -> Option<u16> {
        self.bodies.get(i).and_then(|b| b.owner)
    }

    /// Powers that have their capital on planet `i` (the civilisations of a shared planet).
    pub fn hosted(&self, i: usize) -> impl Iterator<Item = &Power> {
        self.powers.iter().filter(move |p| p.alive && p.home == i && p.kind == PowerKind::Civ)
    }

    /// Are a and b at peace (a truce, the same power, or partners in the coalition)?
    pub fn friendly(&self, a: u16, b: u16) -> bool {
        a == b || self.rel(a, b).truce > 0.0 || (self.powers[a as usize].coalition && self.powers[b as usize].coalition)
    }

    /// The defence a fleet meets at planet `i` (on a shared planet: power `target`'s own).
    pub fn defence(&self, i: usize, target: Option<u16>) -> Defence {
        let b = &self.bodies[i];
        if b.shared {
            return target.and_then(|t| self.powers.get(t as usize)).and_then(|p| p.external).map(|e| e.defence).unwrap_or_default();
        }
        Defence { shields: b.shields, defences: b.defences, garrison: b.garrison }
    }

    /// The most shields, defences and garrison planet `i` can hold now.
    pub fn caps(&self, i: usize) -> Defence {
        let b = &self.bodies[i];
        if !b.inhabited() {
            return Defence::default();
        }
        let mut garrison = GARRISON_BASE + GARRISON_PER_KPOP * (b.stats.pop.max(0.0) / 1000.0).sqrt();
        if b.stats.hand_played {
            garrison = garrison.max(HAND_PLAYED_GARRISON);
        }
        let ruler = b.owner.and_then(|o| self.powers.get(o as usize)).filter(|p| p.alive);
        match ruler {
            Some(p) if p.space => {
                let fx = p.tier_fx();
                let share = if p.home == i { 1.0 } else { COLONY_DEFENCE };
                let dig_in = 1.0 + 0.25 * self.mind.defensiveness() + if p.coalition { 0.15 } else { 0.0 };
                Defence {
                    shields: SHIELD_PER_KNOW * p.econ.knowledge * fx.shields * share * dig_in,
                    defences: DEFENCE_PER_PROD * p.econ.production * fx.shields * share * dig_in,
                    garrison: garrison * (1.0 + 0.2 * p.tier as f32),
                }
            }
            _ => Defence { shields: 0.0, defences: 0.0, garrison },
        }
    }

    /// Can power `p` send a fleet against planet `i` (and, on a shared planet, power `tp`)?
    pub fn can_target(&self, p: u16, i: usize, tp: Option<u16>) -> bool {
        let Some(b) = self.bodies.get(i) else { return false };
        if !b.inhabited() {
            return false;
        }
        let target = if b.shared { tp } else { b.owner };
        let Some(t) = target else { return false };
        let tpw = &self.powers[t as usize];
        if !tpw.alive || self.friendly(p, t) {
            return false;
        }
        // The galaxy can't reach the hand-played home (or a civilisation) until it is in space.
        if (b.stats.hand_played || b.shared) && !tpw.space {
            return false;
        }
        true
    }

    // ------------------------------------------------------------------ orders

    /// The player (or the host for any power) orders an attack on planet `target`.
    /// Returns the fleet's id, or why not.
    pub fn order(&mut self, p: u16, target: usize, order: Order) -> Result<u32, &'static str> {
        let Some(pw) = self.powers.get(p as usize) else { return Err("No such power.") };
        if !pw.alive {
            return Err("That world has fallen.");
        }
        if !pw.space {
            return Err("Your world needs the Space Age to build a fleet.");
        }
        let Some(b) = self.bodies.get(target) else { return Err("No such world.") };
        if target == pw.home || b.owner == Some(p) {
            return Err("That world is already yours.");
        }
        if !b.stats.alive {
            return Err("That world is still forming.");
        }
        let fleet = pw.fleet;
        if fleet < MIN_LAUNCH {
            return Err("Your fleet is still being built.");
        }
        if !b.inhabited() {
            if order != Order::Invade {
                return Err("Nobody lives there: invade to land colonists.");
            }
        } else {
            let tp = if b.shared { self.hosted(target).map(|h| h.id).max_by(|&x, &y| self.powers[x as usize].dominance.total_cmp(&self.powers[y as usize].dominance)) } else { b.owner };
            if let Some(t) = tp {
                if self.friendly(p, t) {
                    return Err("You have a truce with them.");
                }
            }
        }
        let tp = if b.shared { self.hosted(target).map(|h| h.id).next() } else { None };
        Ok(self.launch(p, Launch { target, target_power: tp, order, commit: PLAYER_COMMIT, motive: Motive::Command }))
    }

    /// An external power (a civ sandbox civilisation) sends `strength` it has already taken
    /// from its own dock.
    pub fn launch_external(&mut self, p: u16, target: usize, order: Order, strength: f32, motive: Motive) -> Option<u32> {
        let pw = self.powers.get(p as usize)?;
        if !pw.alive || strength <= 0.0 || target >= self.bodies.len() || target == pw.home {
            return None;
        }
        let tp = if self.bodies[target].shared { None } else { self.bodies[target].owner };
        if tp.is_some_and(|t| self.friendly(p, t)) {
            return None;
        }
        Some(self.dispatch(p, target, tp, order, strength, motive))
    }

    fn launch(&mut self, p: u16, l: Launch) -> u32 {
        // Pressing the player harder (difficulty): back at it sooner after a strike on it.
        let at_player = l.target_power.or(self.bodies[l.target].owner).map_or(1.0, |d| self.pressure_on(p, d));
        let pw = &mut self.powers[p as usize];
        let strength = (pw.fleet * clamp(l.commit, 0.0, 1.0)).max(MIN_LAUNCH.min(pw.fleet));
        pw.fleet -= strength;
        pw.cooldown = (10.0 + 8.0 * (1.0 - pw.stance)) / at_player.max(1.0);
        self.dispatch(p, l.target, l.target_power, l.order, strength, l.motive)
    }

    fn dispatch(&mut self, p: u16, target: usize, tp: Option<u16>, order: Order, strength: f32, motive: Motive) -> u32 {
        let id = self.next_fleet;
        self.next_fleet += 1;
        let bend = self.rng.range_f32(-0.4, 0.4);
        let duration = self.rng.range_f32(TRAVEL_MIN, TRAVEL_MAX);
        let from = self.powers[p as usize].home;
        if self.fleets.len() >= MAX_FLEETS {
            self.fleets.remove(0);
        }
        self.fleets.push(Ship {
            from_planet: from,
            to_planet: target,
            t: 0.0,
            duration,
            mission: Some(Mission { id, owner: p, target_power: tp, order, strength, motive, bend }),
        });
        // Warn the player of an incoming fleet; tell them their own is away.
        let defender = tp.or(self.bodies[target].owner);
        let attacker = self.powers[p as usize].name.clone();
        let tname = self.bodies[target].name.clone();
        if defender.is_some_and(|d| self.powers[d as usize].kind == PowerKind::Player) {
            let why = motive.why("your world");
            self.say(TONE_WARN, true, format!("{attacker} launched {} at your world: {why}.", with_article(order)));
        } else if self.powers[p as usize].kind == PowerKind::Player {
            self.say(TONE_INFO, true, format!("Your fleet ({:.0}) is away: {} on {tname}.", strength, order.name()));
        }
        id
    }

    // ------------------------------------------------------------------ the step

    /// Advance the war by `dt` seconds with the player's mind as it is now.
    pub fn step(&mut self, dt: f32, mind: Mind) {
        if !(dt > 0.0) {
            return;
        }
        let dt = dt.min(0.5);
        self.time += dt;
        self.mind = mind;
        self.fly(dt);
        self.regrow(dt);
        for b in self.battles.iter_mut() {
            b.t += dt;
        }
        self.battles.retain(|b| b.t < BATTLE_SECONDS);
        for b in self.bodies.iter_mut() {
            b.battle_t += dt;
        }
        self.council_acc += dt;
        if self.council_acc >= COUNCIL_EVERY || self.councils == 0 {
            self.council_acc = (self.council_acc - COUNCIL_EVERY).max(0.0);
            self.council(COUNCIL_EVERY);
        }
    }

    fn fly(&mut self, dt: f32) {
        let mut arrived: Vec<(usize, Mission)> = Vec::new();
        self.fleets.retain_mut(|s| {
            s.t += dt;
            if s.t >= s.duration {
                if let Some(m) = s.mission {
                    arrived.push((s.to_planet, m));
                }
                false
            } else {
                true
            }
        });
        for (to, m) in arrived {
            self.arrive(to, m);
        }
    }

    /// Shields, defences and garrisons regrow; fleets are built.
    fn regrow(&mut self, dt: f32) {
        let calm = self.mind.defensiveness();
        for i in 0..self.bodies.len() {
            if self.bodies[i].shared {
                continue;
            }
            let cap = self.caps(i);
            let b = &mut self.bodies[i];
            let grow = |v: f32, cap: f32, rate: f32| if v < cap { (v + cap * rate * dt).min(cap) } else { cap + (v - cap) * (1.0 - 0.2 * dt).max(0.0) };
            b.shields = grow(b.shields, cap.shields, SHIELD_REGEN * (1.0 + calm));
            b.defences = grow(b.defences, cap.defences, DEFENCE_REGEN * (1.0 + 0.5 * calm));
            b.garrison = grow(b.garrison, cap.garrison, GARRISON_REGEN);
        }
        let h = self.mind.hostility();
        for p in self.powers.iter_mut() {
            if !p.alive || !p.space || p.external.is_some() {
                continue;
            }
            // Stress puts the yards on a war footing; calm spends on shields instead.
            let pace = (1.0 + 0.3 * h) * if p.coalition { 1.25 } else { 1.0 };
            if p.fleet < p.fleet_cap {
                p.fleet = (p.fleet + p.fleet_cap * FLEET_BUILD * pace * dt).min(p.fleet_cap);
            } else {
                p.fleet = p.fleet_cap + (p.fleet - p.fleet_cap) * (1.0 - 0.05 * dt).max(0.0);
            }
            p.cooldown = (p.cooldown - dt).max(0.0);
        }
        for p in self.powers.iter_mut().filter(|p| p.external.is_some()) {
            p.cooldown = (p.cooldown - dt).max(0.0);
        }
    }

    /// Once a second: who rules what, economies, moods, endgame, decisions, truces.
    fn council(&mut self, dt: f32) {
        self.councils += 1;
        self.refresh_owners();
        self.refresh_economies();
        crate::endgame::council(self, dt);
        self.diplomacy(dt);
        self.decide_all();
    }

    /// Inhabited planets without a ruler get their own power; dead rulers let go.
    fn refresh_owners(&mut self) {
        for i in 0..self.bodies.len() {
            let inhabited = self.bodies[i].inhabited();
            if let Some(o) = self.bodies[i].owner {
                if !inhabited || !self.powers[o as usize].alive {
                    self.bodies[i].owner = None;
                    if inhabited {
                        continue;
                    }
                }
            }
            if self.bodies[i].shared || !inhabited || self.bodies[i].owner.is_some() || self.powers.len() >= MAX_POWERS {
                continue;
            }
            let kind = if self.player_home == Some(i) { PowerKind::Player } else { PowerKind::World };
            let name = if kind == PowerKind::Player { "Your world".to_string() } else { self.bodies[i].name.clone() };
            let temper = self.bodies[i].stats.temper;
            let id = self.add_power(kind, name, [255, 255, 255], i, i as u32, temper, None);
            self.bodies[i].owner = Some(id);
            // A people that was already there has its warriors already.
            let g = self.caps(i).garrison;
            self.bodies[i].garrison = self.bodies[i].garrison.max(g);
        }
        // Who lives, and where its capital is.
        for p in 0..self.powers.len() {
            if !self.powers[p].alive || self.powers[p].external.is_some() {
                continue;
            }
            let held: Vec<usize> = (0..self.bodies.len()).filter(|&i| self.bodies[i].owner == Some(p as u16)).collect();
            let pw = &mut self.powers[p];
            pw.worlds = held.len() as u32;
            if held.is_empty() {
                pw.alive = false;
                pw.fleet = 0.0;
            } else if !held.contains(&pw.home) {
                pw.home = held[0];
            }
        }
        for p in 0..self.powers.len() {
            if self.powers[p].external.is_some() {
                let held = self.bodies.iter().filter(|b| b.owner == Some(p as u16)).count() as u32;
                self.powers[p].worlds = 1 + held;
            }
        }
    }

    /// Economy (home + colonies, times the tier), space capability and fleet caps.
    fn refresh_economies(&mut self) {
        let h = self.mind.hostility();
        for p in 0..self.powers.len() {
            if !self.powers[p].alive {
                continue;
            }
            let mut e = self.powers[p].external.map(|x| x.econ).unwrap_or_default();
            let external = self.powers[p].external.is_some();
            let home = self.powers[p].home;
            // An empire overstretches: each colony is worth less the more it holds.
            let colonies = self.bodies.iter().enumerate().filter(|(i, b)| b.owner == Some(p as u16) && (*i != home || external)).count();
            let share = COLONY_SHARE / (colonies.max(1) as f32).sqrt();
            for (i, b) in self.bodies.iter().enumerate() {
                if b.owner != Some(p as u16) {
                    continue;
                }
                e.add_scaled(&b.econ, if i == home && !external { 1.0 } else { share });
            }
            let pw = &mut self.powers[p];
            let fx = crate::endgame::tier_fx(pw.tier);
            if !external {
                pw.econ = e.scaled(fx.econ);
                pw.space = self.bodies.get(home).is_some_and(|b| b.stats.era >= SPACE_ERA && b.inhabited());
            } else {
                pw.econ = e;
            }
            pw.fleet_cap = if pw.space { FLEET_PER_ECON * pw.econ.index() * fx.fleet } else { 0.0 };
            pw.stance = clamp(pw.temper + 0.55 * h, 0.0, 1.0);
        }
    }

    /// Grudges fade; truces run out; a calm god talks warring leaders into truces.
    fn diplomacy(&mut self, dt: f32) {
        let calm = self.mind.defensiveness();
        let stress = clamp(self.mind.hostility(), 0.0, 1.0);
        let n = self.powers.len();
        for a in 0..n {
            for b in 0..n {
                let r = &mut self.rel[a * MAX_POWERS + b];
                r.truce = (r.truce - dt).max(0.0);
                // Stress keeps old wounds open.
                r.grudge = (r.grudge - dt * 0.012 * (1.0 + calm) * (1.0 - 0.6 * stress)).max(0.0);
            }
        }
        if calm < 0.2 {
            return;
        }
        for a in 0..n {
            for b in a + 1..n {
                let (pa, pb) = (&self.powers[a], &self.powers[b]);
                if !pa.alive || !pb.alive || !pa.space || !pb.space || pa.kind == PowerKind::Player || pb.kind == PowerKind::Player {
                    continue;
                }
                let (ra, rb) = (self.rel[a * MAX_POWERS + b], self.rel[b * MAX_POWERS + a]);
                if ra.truce > 0.0 || ra.grudge + rb.grudge < 0.3 || Some(a as u16) == self.coalition_against || Some(b as u16) == self.coalition_against {
                    continue;
                }
                if self.roll(a as u16, b as u16, 7) < 0.04 * calm {
                    for (x, y) in [(a, b), (b, a)] {
                        let r = &mut self.rel[x * MAX_POWERS + y];
                        r.truce = TRUCE_SECONDS;
                        r.grudge *= 0.5;
                    }
                    let text = format!("{} and {} sign a truce: your calm reached their councils.", self.powers[a].name, self.powers[b].name);
                    self.say(TONE_GOOD, false, text);
                }
            }
        }
    }

    fn decide_all(&mut self) {
        let mut policy = self.policy.take();
        let mut out = std::mem::take(&mut self.launches);
        for p in 0..self.powers.len() as u16 {
            let pw = &self.powers[p as usize];
            if !pw.alive || !pw.space || pw.external.is_some() || pw.kind == PowerKind::Player || pw.cooldown > 0.0 {
                continue;
            }
            out.clear();
            match policy.as_mut() {
                Some(pol) => pol.decide(self, p, &mut out),
                None => Rules.decide(self, p, &mut out),
            }
            for &l in out.iter() {
                let tp = l.target_power.or(self.bodies[l.target].owner);
                if self.powers[p as usize].fleet >= MIN_LAUNCH && self.can_target(p, l.target, tp) {
                    // Pressing the player more softly (difficulty): a council often stands its
                    // fleet down instead (for longer the softer it is).
                    let k = tp.map_or(1.0, |d| self.pressure_on(p, d));
                    if k < 1.0 && self.roll(p, tp.unwrap_or(p), 7) >= k {
                        let pw = &mut self.powers[p as usize];
                        pw.cooldown = (10.0 + 8.0 * (1.0 - pw.stance)) / k;
                        break;
                    }
                    self.launch(p, l);
                    if l.motive == Motive::Coalition {
                        self.joint_strike(p, l);
                    }
                    break;
                }
            }
        }
        self.launches = out;
        self.policy = policy;
    }

    /// The coalition strikes together: every member with ships ready joins a member's attack on
    /// the leader.
    fn joint_strike(&mut self, first: u16, l: Launch) {
        let mut joined = vec![self.powers[first as usize].name.clone()];
        for q in 0..self.powers.len() as u16 {
            let pw = &self.powers[q as usize];
            if q == first || !pw.alive || !pw.space || !pw.coalition || pw.external.is_some() || pw.kind == PowerKind::Player || pw.cooldown > 0.0 {
                continue;
            }
            if pw.fleet < MIN_LAUNCH || pw.fleet < 0.3 * pw.fleet_cap || !self.can_target(q, l.target, l.target_power.or(self.bodies[l.target].owner)) {
                continue;
            }
            joined.push(pw.name.clone());
            self.launch(q, Launch { commit: 0.6, ..l });
        }
        if joined.len() > 1 {
            let target = self.bodies[l.target].name.clone();
            let names = joined.join(" and ");
            self.say(TONE_WARN, false, format!("{names} strike {target} together: the coalition's fleets fly as one."));
        }
    }

    /// A deterministic die for councils: 0..1, different for every council, pair and question.
    pub fn roll(&self, a: u16, b: u16, salt: u32) -> f32 {
        let mut h = self.seed ^ self.councils.wrapping_mul(0x9e37_79b9) ^ ((a as u32) << 8 | b as u32).wrapping_mul(0x85eb_ca6b) ^ salt.wrapping_mul(0xc2b2_ae35);
        h ^= h >> 16;
        h = h.wrapping_mul(0x7feb_352d);
        h ^= h >> 15;
        h = h.wrapping_mul(0x846c_a68b);
        h ^= h >> 16;
        (h >> 8) as f32 / (1u32 << 24) as f32
    }

    // ------------------------------------------------------------------ battles

    fn arrive(&mut self, to: usize, m: Mission) {
        let a_id = m.owner;
        if !self.powers[a_id as usize].alive || to >= self.bodies.len() {
            return;
        }
        let b = &self.bodies[to];
        // Colonists on a barren world.
        if !b.inhabited() {
            if m.order == Order::Invade && b.stats.alive && !b.shared {
                self.bodies[to].owner = Some(a_id);
                self.bodies[to].fx.settled = true;
                self.bodies[to].fx.captured_by = a_id as i32;
                self.bodies[to].garrison = m.strength * 0.5;
                self.powers[a_id as usize].captures += 1;
                self.powers[a_id as usize].results.colonies += 1;
                let text = format!("{} landed colonists on {}: a new colony.", self.powers[a_id as usize].name, self.bodies[to].name);
                self.say(TONE_GOOD, self.is_player(a_id), text);
            } else {
                self.come_home(a_id, m.strength * 0.9);
            }
            return;
        }
        let target = if b.shared { m.target_power.or_else(|| self.hosted(to).map(|h| h.id).next()) } else { b.owner };
        let Some(d_id) = target else {
            self.come_home(a_id, m.strength * 0.9);
            return;
        };
        if d_id == a_id || !self.powers[d_id as usize].alive {
            // It changed hands on the way: reinforce, or come home.
            if d_id == a_id && !self.bodies[to].shared {
                self.bodies[to].garrison += m.strength * 0.5;
            } else {
                self.come_home(a_id, m.strength * 0.9);
            }
            return;
        }
        let shared = self.bodies[to].shared;
        let hand_played = self.bodies[to].stats.hand_played;
        let home_ground = self.powers[d_id as usize].home == to;
        let mut def = self.defence(to, Some(d_id));
        let before = def;
        let luck = self.rng.range_f32(0.85, 1.15);
        // Difficulty: an AI fleet hits the player's world ×player_pressure (its ships don't change).
        let a = m.strength * luck * self.pressure_on(a_id, d_id);
        let mut fx = BodyFx::default();
        let won;
        let mut survivors;
        let mut loot = 0.0;
        let mut taken = false;
        let mut hits = 0.0;
        let mut detail: String;
        let econ = self.bodies[to].econ;
        let its = if self.is_player(d_id) { "your" } else { "its" };
        match m.order {
            Order::Raid => {
                // Raiders slip past half the guns; the garrison guards the stores.
                let eff = 0.5 * def.orbit() + 0.25 * def.garrison;
                won = a > eff;
                let loss = m.strength * (eff / (a + eff + 1e-3)) * if won { 0.5 } else { 0.9 };
                survivors = (m.strength - loss).max(0.0);
                def.defences *= if won { 0.85 } else { 0.95 };
                if won {
                    let share = clamp((a - eff) / (a + eff + 1.0), 0.05, 0.35);
                    fx.stock_mul = 1.0 - share;
                    fx.progress_loss = 0.4 * share;
                    loot = share * (econ.resources + econ.knowledge) * 4.0;
                    detail = format!("stole {:.0}% of {its} stores", share * 100.0);
                } else {
                    detail = "the raiders were driven off".to_string();
                }
            }
            Order::Bombard => {
                let mut rem = a;
                let s_hit = rem.min(def.shields);
                def.shields -= s_hit;
                rem -= s_hit;
                let d_hit = (rem * 0.6).min(def.defences);
                def.defences -= d_hit;
                rem -= d_hit;
                let dmg = rem / (rem + 12.0 + 0.5 * def.garrison);
                fx.pop_mul = 1.0 - 0.35 * dmg;
                fx.progress_loss = 0.25 * dmg;
                fx.stock_mul = 1.0 - 0.25 * dmg;
                def.garrison *= 1.0 - 0.3 * dmg;
                hits = 0.35 * dmg;
                won = dmg > 0.05;
                let loss = m.strength * (before.defences / (a + before.defences + 1.0)) * 0.6;
                survivors = (m.strength - loss).max(0.0);
                detail = if won {
                    format!("{:.0}% of {its} people died", 35.0 * dmg)
                } else if s_hit > 0.0 {
                    format!("{its} shields held")
                } else {
                    "the bombs fell wide".to_string()
                };
            }
            Order::Invade => {
                let orbit = def.orbit();
                let ground_mult = if hand_played { HAND_PLAYED_GROUND } else if home_ground { HOME_GROUND } else { 1.0 };
                if a <= orbit {
                    // Lost in orbit.
                    won = false;
                    let k = a / (orbit + 1e-3);
                    def.shields *= 1.0 - 0.5 * k;
                    def.defences *= 1.0 - 0.4 * k;
                    survivors = m.strength * 0.15;
                    detail = "the fleet was broken in orbit".to_string();
                } else {
                    def.shields = 0.0;
                    def.defences *= 0.3;
                    let landed = (a - 0.7 * orbit) * 0.8;
                    let ground = def.garrison * ground_mult;
                    let roll = self.rng.range_f32(0.9, 1.1);
                    if landed * roll > ground {
                        won = true;
                        survivors = 0.0;
                        if hand_played || shared {
                            // A beachhead: they can't hold the world, but they wreck and loot it.
                            fx.pop_mul = 0.8;
                            fx.progress_loss = 0.2;
                            fx.stock_mul = 0.7;
                            hits = 0.2;
                            loot = (econ.resources + econ.knowledge) * 2.0;
                            def.garrison *= 0.5;
                            survivors = (landed - ground) * 0.5;
                            detail = "they held a beachhead, looted it and were driven off".to_string();
                        } else {
                            taken = true;
                            fx.pop_mul = 0.8;
                            def = Defence { shields: 0.0, defences: 0.0, garrison: (landed - ground).max(1.0) };
                            detail = "the planet fell".to_string();
                        }
                    } else {
                        won = false;
                        def.garrison = (def.garrison - landed / ground_mult).max(def.garrison * 0.2);
                        survivors = 0.0;
                        detail = "the garrison threw the landings back".to_string();
                    }
                }
            }
        }
        // The campaign's scene isn't the war's to hurt: an attack on the hand-played home takes
        // its toll in the player's orbit instead (docked ships and research).
        if hand_played && won {
            let pw = &mut self.powers[d_id as usize];
            let (ships, research) = match m.order {
                Order::Raid => (0.0, (1.0 - fx.stock_mul) * 0.6),
                Order::Bombard => ((1.0 - fx.pop_mul) * 1.5, 0.0),
                Order::Invade => (0.3, 0.1),
            };
            let lost = pw.fleet * ships.min(0.6);
            pw.fleet -= lost;
            let floor = crate::endgame::TIERS[(pw.tier as usize).min(crate::endgame::TIERS.len() - 1)].xp;
            pw.tier_xp = (pw.tier_xp - pw.tier_xp * research).max(floor);
            detail = match m.order {
                Order::Raid => format!("stole {:.0}% of your research", research * 100.0),
                Order::Bombard => format!("{lost:.0} of your docked ships burned in orbit"),
                Order::Invade => "they held a beachhead and wrecked your spaceport".to_string(),
            };
        }
        // Write the fight back.
        if shared {
            let pw = &mut self.powers[d_id as usize];
            pw.results.hits += hits;
            pw.results.shield_loss += (before.orbit() - def.orbit()).max(0.0);
            pw.results.loot -= loot;
            if let Some(e) = pw.external.as_mut() {
                e.defence = def;
            }
        } else {
            let b = &mut self.bodies[to];
            b.shields = def.shields;
            b.defences = def.defences;
            b.garrison = def.garrison;
            let f = &mut b.fx;
            f.pop_mul *= fx.pop_mul;
            f.progress_loss += fx.progress_loss;
            f.stock_mul *= fx.stock_mul;
        }
        self.bodies[to].battle_t = 0.0;
        // Grudges: the victim remembers.
        let wound = 0.25 + if won { 0.35 } else { 0.1 } + if taken { 0.6 } else { 0.0 };
        self.rel_mut(d_id, a_id).grudge += wound;
        self.rel_mut(d_id, a_id).truce = 0.0;
        self.rel_mut(a_id, d_id).truce = 0.0;
        {
            let at = &mut self.powers[a_id as usize];
            if won {
                at.wins += 1;
            } else {
                at.losses += 1;
            }
            at.results.loot += loot;
            at.results.knowledge += loot * 0.5;
            at.tier_xp += loot * 0.5;
        }
        if won {
            self.powers[d_id as usize].losses += 1;
        }
        self.next_battle += 1;
        self.battles.push(Battle { id: self.next_battle, body: to, attacker: a_id, order: m.order, won, t: 0.0 });
        if self.battles.len() > 32 {
            self.battles.remove(0);
        }
        // The news, before a capture renames anything.
        let an = if self.is_player(a_id) { "You".to_string() } else { self.powers[a_id as usize].name.clone() };
        let dn = self.power_label(d_id, to);
        let player_side = self.is_player(a_id) || self.is_player(d_id);
        let tone = if self.is_player(d_id) {
            if won { TONE_BAD } else { TONE_GOOD }
        } else if self.is_player(a_id) {
            if won { TONE_GOOD } else { TONE_BAD }
        } else if taken {
            TONE_WARN
        } else {
            TONE_INFO
        };
        let verb = match (m.order, won) {
            (Order::Raid, _) => "raided",
            (Order::Bombard, _) => "bombarded",
            (Order::Invade, true) => "invaded",
            (Order::Invade, false) => "tried to invade",
        };
        let why = m.motive.why(if self.is_player(d_id) { "your world" } else { &self.powers[d_id as usize].name });
        let mind = self.mind_clause(a_id, m.motive);
        self.say(tone, player_side, format!("{an} {verb} {dn}: {detail}. Why: {why}.{mind}"));
        if taken {
            self.capture(to, a_id, d_id);
        }
        self.come_home(a_id, survivors);
    }

    fn come_home(&mut self, p: u16, strength: f32) {
        if strength <= 0.0 {
            return;
        }
        let pw = &mut self.powers[p as usize];
        if pw.external.is_some() {
            pw.results.returned += strength;
        } else {
            pw.fleet += strength;
        }
    }

    /// Power `by` takes planet `i` from `from`.
    fn capture(&mut self, i: usize, by: u16, from: u16) {
        self.bodies[i].owner = Some(by);
        self.bodies[i].fx.captured_by = by as i32;
        self.powers[by as usize].captures += 1;
        self.powers[by as usize].results.colonies += 1;
        self.powers[from as usize].results.colonies -= 1;
        let held: Vec<usize> = (0..self.bodies.len()).filter(|&k| self.bodies[k].owner == Some(from)).collect();
        let name = self.powers[from as usize].name.clone();
        let world = self.bodies[i].name.clone();
        let by_name = self.powers[by as usize].name.clone();
        let fallen = held.is_empty() && self.powers[from as usize].external.is_none();
        let player = self.is_player(by);
        let now = if player { "your colony".to_string() } else { format!("ruled by {by_name}") };
        // A people driven from its homeworld that takes it back is home again.
        let homecoming = self.is_homeworld(by, i);
        if homecoming {
            self.powers[by as usize].home = i;
            self.powers[by as usize].results.colonies -= 1;
        }
        if homecoming && !fallen {
            let who = if player { "You".to_string() } else { by_name.clone() };
            let home = if self.powers[from as usize].home == i { held.first().copied() } else { None };
            if let Some(h) = home {
                self.powers[from as usize].home = h;
            }
            self.say(TONE_GOOD, player || self.is_player(from), format!("{who} took back {world}, its homeworld, from {name}."));
        } else if fallen {
            let pw = &mut self.powers[from as usize];
            pw.alive = false;
            pw.fleet = 0.0;
            self.say(TONE_WARN, player, format!("{name} has fallen: {world} is now {now}."));
        } else {
            if self.powers[from as usize].home == i {
                if let Some(&h) = held.first() {
                    self.powers[from as usize].home = h;
                }
            }
            let now = if player { "your colony".to_string() } else { format!("a colony of {by_name}") };
            self.say(TONE_WARN, player || self.is_player(from), format!("{world} is now {now}."));
        }
    }

    // ------------------------------------------------------------------ powers

    #[allow(clippy::too_many_arguments)]
    pub fn add_power(&mut self, kind: PowerKind, name: String, color: [u8; 3], home: usize, key: u32, temper: f32, external: Option<External>) -> u16 {
        let id = self.powers.len() as u16;
        self.powers.push(Power {
            id,
            kind,
            name,
            color,
            home,
            key,
            alive: true,
            space: false,
            econ: Economy::default(),
            fleet: 0.0,
            fleet_cap: 0.0,
            temper: clamp(temper, 0.0, 1.0),
            stance: clamp(temper, 0.0, 1.0),
            cooldown: 6.0,
            tier: 0,
            tier_xp: 0.0,
            coalition: false,
            dominance: 0.0,
            worlds: 1,
            external,
            results: PowerResults::default(),
            wins: 0,
            losses: 0,
            captures: 0,
        });
        id
    }

    /// Is planet `i` where power `p`'s people come from (a world's own civilisation)?
    pub fn is_homeworld(&self, p: u16, i: usize) -> bool {
        self.powers.get(p as usize).is_some_and(|x| x.kind != PowerKind::Civ && x.key == i as u32)
    }

    /// The living power whose people come from planet `i`, if it lives elsewhere now.
    pub fn exiles_of(&self, i: usize) -> Option<u16> {
        self.powers.iter().find(|x| x.alive && x.kind != PowerKind::Civ && x.key == i as u32).map(|x| x.id)
    }

    /// Register or update an external power, by the host's key: a civ sandbox civilisation in
    /// space, living on the shared planet `home`. Its fleet and defences are the host's
    /// (`moc-civ`'s `space.rs`); the war adds what its colonies bring. `alive: false` retires it
    /// (its colonies go free). Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_external(&mut self, key: u32, name: &str, color: [u8; 3], home: usize, econ: Economy, fleet: f32, defence: Defence, tier: u8, temper: f32, alive: bool) -> Option<u16> {
        while self.bodies.len() <= home {
            self.bodies.push(Body::new(""));
        }
        let found = self.powers.iter().position(|p| p.kind == PowerKind::Civ && p.key == key && p.alive);
        let id = match found {
            Some(i) => i as u16,
            None if alive && self.powers.len() < MAX_POWERS => self.add_power(PowerKind::Civ, name.to_string(), color, home, key, temper, Some(External { fleet, defence, econ })),
            None => return None,
        };
        if !alive {
            let pw = &mut self.powers[id as usize];
            pw.alive = false;
            pw.fleet = 0.0;
            for b in self.bodies.iter_mut() {
                if b.owner == Some(id) {
                    b.owner = None;
                }
            }
            return Some(id);
        }
        let pw = &mut self.powers[id as usize];
        if pw.name != name {
            pw.name = name.to_string();
        }
        pw.color = color;
        pw.home = home;
        pw.space = true;
        pw.fleet = fleet;
        pw.fleet_cap = pw.fleet_cap.max(fleet);
        pw.tier = tier;
        pw.temper = clamp(temper, 0.0, 1.0);
        pw.external = Some(External { fleet, defence, econ });
        Some(id)
    }

    pub fn is_player(&self, p: u16) -> bool {
        self.powers.get(p as usize).is_some_and(|x| x.kind == PowerKind::Player)
    }

    /// How the news names the defender of planet `i`.
    fn power_label(&self, d: u16, i: usize) -> String {
        let p = &self.powers[d as usize];
        if p.kind == PowerKind::Player {
            "your world".to_string()
        } else if p.home == i || self.bodies[i].shared {
            p.name.clone()
        } else {
            format!("{}'s colony {}", p.name, self.bodies[i].name)
        }
    }

    /// The mind's part in an attack, for the news.
    fn mind_clause(&self, a: u16, motive: Motive) -> &'static str {
        if motive == Motive::Command || self.powers[a as usize].kind != PowerKind::World && self.powers[a as usize].kind != PowerKind::Civ {
            return "";
        }
        let h = self.mind.hostility();
        if h > 0.3 {
            " Your stress has made its leaders reckless."
        } else if h < -0.3 {
            " Even your calm could not hold it back."
        } else {
            ""
        }
    }

    // ------------------------------------------------------------------ host output

    pub fn say(&mut self, tone: u8, player: bool, text: String) {
        let civ = self.powers.iter().any(|p| p.kind == PowerKind::Civ && !p.name.is_empty() && text.contains(p.name.as_str()));
        self.news.push(NewsItem { seq: self.next_news, t: self.time, tone, text, player, civ });
        self.next_news += 1;
        if self.news.len() > NEWS_KEEP {
            let k = self.news.len() - NEWS_KEEP;
            self.news.drain(..k);
        }
    }

    /// News lines numbered `from` and up.
    pub fn news_since(&self, from: u32) -> impl Iterator<Item = &NewsItem> {
        self.news.iter().filter(move |n| n.seq >= from)
    }

    /// What the war did to planet `i` since the last call (and forget it).
    pub fn take_fx(&mut self, i: usize) -> BodyFx {
        self.bodies.get_mut(i).map(|b| std::mem::take(&mut b.fx)).unwrap_or_default()
    }

    /// What happened to an external power since the last call (and forget it).
    pub fn take_results(&mut self, p: u16) -> PowerResults {
        self.powers.get_mut(p as usize).map(|x| std::mem::take(&mut x.results)).unwrap_or_default()
    }

    /// Where a fleet is, 0..1 along its flight.
    pub fn fleet_progress(s: &Ship) -> f32 {
        clamp(s.t / s.duration.max(1e-3), 0.0, 1.0)
    }
}

/// "a raid", "an orbital bombardment", "an invasion".
fn with_article(o: Order) -> &'static str {
    match o {
        Order::Raid => "a raid",
        Order::Bombard => "an orbital bombardment",
        Order::Invade => "an invasion",
    }
}

/// The default space council: strike when the fleet is ready, at the target that is worth most
/// and defended least, remembering grudges, following the coalition, tilted by the mind.
pub struct Rules;

impl SpacePolicy for Rules {
    fn decide(&mut self, war: &SpaceWar, p: u16, out: &mut Vec<Launch>) {
        let pw = &war.powers[p as usize];
        let h = war.mind.hostility();
        let ready = clamp(0.55 - 0.25 * h, 0.25, 0.85);
        if pw.fleet < MIN_LAUNCH || pw.fleet < ready * pw.fleet_cap {
            return;
        }
        // Not every council: hawks strike sooner, and calm makes doves of them all.
        if war.roll(p, p, 1) > (0.06 + 0.3 * pw.stance) * (1.0 - 0.8 * war.mind.defensiveness()) {
            return;
        }
        let mut best: Option<(f32, Launch)> = None;
        for (i, b) in war.bodies.iter().enumerate() {
            if !b.inhabited() || b.owner == Some(p) {
                continue;
            }
            // On a shared planet, its strongest civilisation (or the one it hates most).
            let t = if b.shared {
                war.hosted(i)
                    .filter(|x| x.space)
                    .map(|x| (x.id, war.rel(p, x.id).grudge * 2.0 + x.dominance))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|x| x.0)
            } else {
                b.owner
            };
            let Some(t) = t else { continue };
            if !war.can_target(p, i, Some(t)) {
                continue;
            }
            let tp = &war.powers[t as usize];
            let def = war.defence(i, Some(t));
            let value = b.econ.index() + if b.shared { tp.econ.index() } else { 0.0 } + 1.0;
            let weakness = pw.fleet / (pw.fleet + def.total() + 1.0);
            let grudge = war.rel(p, t).grudge;
            let mut score = value * weakness * (1.0 + 1.5 * grudge) * if tp.space { 1.0 } else { 0.35 };
            let against_leader = war.coalition_against == Some(t) && pw.coalition;
            if against_leader {
                score *= 3.0;
            } else if war.leader == Some(t) && tp.dominance > 0.35 {
                score *= 1.5;
            }
            if tp.kind == PowerKind::Player {
                score *= 1.0 + 0.4 * h.max(0.0);
            }
            // A little noise so equal targets don't always draw the same fleet.
            score *= 0.85 + 0.3 * war.roll(p, t, 2);
            // Order: invade what it can take, bombard who it hates (or the leader), else raid.
            let invade_needs = def.orbit() * 1.1 + def.garrison * if b.stats.hand_played { HAND_PLAYED_GROUND } else { HOME_GROUND } + 2.0;
            let order = if !b.stats.hand_played && !b.shared && pw.fleet * Order::Invade.commit() > invade_needs && pw.stance > 0.35 {
                Order::Invade
            } else if grudge > 0.45 || against_leader || (pw.stance > 0.65 && pw.fleet * Order::Bombard.commit() > def.shields) {
                Order::Bombard
            } else {
                Order::Raid
            };
            let motive = if against_leader {
                Motive::Coalition
            } else if war.leader == Some(p) && pw.dominance > 0.35 {
                Motive::Ambition
            } else if grudge > 0.45 {
                Motive::Revenge
            } else if order == Order::Invade {
                Motive::Expansion
            } else if tp.fleet > pw.fleet * 0.8 {
                Motive::Rivalry
            } else {
                Motive::Plunder
            };
            let l = Launch { target: i, target_power: if b.shared { Some(t) } else { None }, order, commit: order.commit(), motive };
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, l));
            }
        }
        if let Some((_, l)) = best {
            out.push(l);
        }
    }
}
