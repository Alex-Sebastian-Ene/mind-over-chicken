//! What the ladder's jobs and buildings do (#69). The jobs and buildings themselves, and the
//! age each one arrives in, come from `data/ages.json` through the generated
//! [`crate::ages_table`]; this module gives each a behaviour and a yield.
//!
//! Jobs are cumulative: a Kingdoms civilisation still forages, hunts, farms and trades. An
//! agent's job is chosen when it is born, and changed later when the village's needs shift
//! (`rebalance`), by **need** (hunger, war, unrest, faith, ...), the civilisation's **traits**
//! and what its **age** has unlocked (and what its village has built). When a civilisation
//! falls into a Dark Age, the jobs of the lost age stop and their workers take older ones.
//!
//! Every job has one of a handful of behaviours ([`Role`]): gather on the land and carry home,
//! work at home, haul between settlements, fight, lead, or settle. Its output ([`Out`]) feeds
//! the metrics the ages are entered by: food, goods (production), lore (knowledge), goats
//! (wealth), faith, order (stability) and healing.

use moc_planet::rng::Rng;

pub use crate::ages_table::{Building, Job};
use crate::world::*;

/// How a job spends its day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Walks to a resource tile, works it, carries the yield home.
    Gather,
    /// Gathers food far afield and fights when there is fighting (the first ages' fighters).
    Hunt,
    /// Works in and around its village; output accrues while it is there and awake.
    Work,
    /// Walks (or sails) to another settlement and back: food, trade, pilgrimage, links.
    Haul,
    /// Guards, scouts, marches and fights.
    Fight,
    /// The chief: stays at the capital, keeps order, leads.
    Lead,
    /// Founds new settlements (by sea for colonists).
    Settle,
}

/// What a job makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    None,
    Food,
    Goods,
    Lore,
    Goats,
    Faith,
    Order,
    Heal,
}

/// Where a gatherer works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Home,
    /// Any land with food.
    Any,
    /// Forest and grass well beyond the village (hunting grounds).
    Wild,
    Forest,
    Hills,
    /// Land next to open water.
    Coast,
    /// The village's own grass (needs a field).
    Field,
}

/// The village's need a job answers (job choice balances these).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cat {
    Food = 0,
    Fight = 1,
    Goods = 2,
    Lore = 3,
    Goats = 4,
    Faith = 5,
    Order = 6,
    Haul = 7,
    Settle = 8,
}
pub const CATS: usize = 9;

#[derive(Clone, Copy, Debug)]
pub struct JobInfo {
    pub role: Role,
    pub cat: Cat,
    pub out: Out,
    /// Per second of work (Work), or per trip (Gather, Haul).
    pub rate: f32,
    /// A second output, per second (knappers learn, scribes keep the law, ...).
    pub side: (Out, f32),
    /// Fighting strength (combat multiplies it by the age's weapon).
    pub strength: f32,
    pub site: Site,
    /// Only chosen where the village has this building (or, for `capital_only`, the capital).
    pub needs: Option<Building>,
    /// Gets a cut of the taxes (inequality).
    pub elite: bool,
    /// The tool pixel drawn beside the agent.
    pub tool: [u8; 3],
}

const fn j(role: Role, cat: Cat, out: Out, rate: f32, strength: f32, site: Site, tool: [u8; 3]) -> JobInfo {
    JobInfo { role, cat, out, rate, side: (Out::None, 0.0), strength, site, needs: None, elite: false, tool }
}

impl JobInfo {
    const fn needs(mut self, b: Building) -> JobInfo {
        self.needs = Some(b);
        self
    }
    const fn elite(mut self) -> JobInfo {
        self.elite = true;
        self
    }
    const fn also(mut self, out: Out, rate: f32) -> JobInfo {
        self.side = (out, rate);
        self
    }
}

const GRAIN: [u8; 3] = [242, 220, 96];
const WOOD: [u8; 3] = [140, 92, 52];
const STONE: [u8; 3] = [170, 170, 180];
const STEEL: [u8; 3] = [214, 214, 226];
const GOLD: [u8; 3] = [250, 210, 70];
const WHITE: [u8; 3] = [245, 245, 245];
const BLUE: [u8; 3] = [90, 150, 240];
const RED: [u8; 3] = [230, 60, 50];
const GREEN: [u8; 3] = [110, 210, 90];
const PURPLE: [u8; 3] = [170, 100, 220];
const CYAN: [u8; 3] = [90, 230, 230];
const DARK: [u8; 3] = [60, 60, 70];

/// The behaviour and yield of every job.
pub fn info(job: Job) -> JobInfo {
    use Building as B;
    use Cat as C;
    use Job::*;
    use Out as O;
    use Role as R;
    use Site as S;
    match job {
        Forage => j(R::Gather, C::Food, O::Food, 1.0, 0.3, S::Any, GRAIN),
        Hunt => j(R::Hunt, C::Fight, O::Food, 1.3, 0.75, S::Wild, WOOD),
        Woodcut => j(R::Gather, C::Goods, O::Goods, 0.06, 0.4, S::Forest, WOOD),
        Cook => j(R::Work, C::Food, O::Food, 0.05, 0.2, S::Home, [255, 150, 60]).needs(B::Campfire),
        Knap => j(R::Work, C::Goods, O::Goods, 0.006, 0.3, S::Home, STONE).also(O::Lore, 0.004),
        Fish => j(R::Gather, C::Food, O::Food, 0.65, 0.3, S::Coast, BLUE),
        Farm => j(R::Gather, C::Food, O::Food, 2.4, 0.3, S::Field, GRAIN).needs(B::Field),
        Herd => j(R::Work, C::Goats, O::Goats, 0.012, 0.35, S::Home, WHITE).needs(B::GoatPen).also(O::Food, 0.03),
        Build => j(R::Work, C::Goods, O::Goods, 0.008, 0.35, S::Home, WOOD),
        Carry => j(R::Haul, C::Haul, O::Food, 2.0, 0.3, S::Home, WOOD),
        Settle => j(R::Settle, C::Settle, O::None, 0.0, 0.3, S::Home, WOOD),
        Trade => j(R::Haul, C::Goats, O::Goats, 0.5, 0.35, S::Home, GOLD).needs(B::Market),
        Guard => j(R::Fight, C::Fight, O::None, 0.0, 1.0, S::Home, STEEL),
        Chief => j(R::Lead, C::Order, O::Order, 0.004, 0.9, S::Home, GOLD).elite(),
        Warrior => j(R::Fight, C::Fight, O::None, 0.0, 1.1, S::Home, STEEL),
        Raid => j(R::Fight, C::Fight, O::None, 0.0, 1.0, S::Home, RED),
        Mine => j(R::Gather, C::Goods, O::Goods, 0.12, 0.45, S::Hills, STONE).needs(B::Mine),
        Smith => j(R::Work, C::Goods, O::Goods, 0.015, 0.55, S::Home, STEEL).needs(B::Forge).also(O::Lore, 0.003),
        Scribe => j(R::Work, C::Lore, O::Lore, 0.03, 0.2, S::Home, WHITE).also(O::Order, 0.002).elite(),
        Tax => j(R::Work, C::Order, O::None, 0.0, 0.3, S::Home, GOLD).elite(),
        Priest => j(R::Work, C::Faith, O::Faith, 0.012, 0.2, S::Home, WHITE).also(O::Order, 0.002).needs(B::Temple),
        Pilgrim => j(R::Haul, C::Faith, O::Faith, 0.08, 0.2, S::Home, WHITE),
        General => j(R::Fight, C::Fight, O::None, 0.0, 1.3, S::Home, GOLD).elite(),
        Engineer => j(R::Work, C::Goods, O::Goods, 0.025, 0.4, S::Home, STEEL).also(O::Lore, 0.004),
        Soldier => j(R::Fight, C::Fight, O::None, 0.0, 1.2, S::Home, STEEL),
        Knight => j(R::Fight, C::Fight, O::None, 0.0, 1.6, S::Home, [255, 255, 255]).elite(),
        Monk => j(R::Work, C::Faith, O::Heal, 0.03, 0.2, S::Home, [150, 110, 70]).also(O::Lore, 0.012).needs(B::Monastery),
        Miller => j(R::Work, C::Food, O::Food, 0.12, 0.3, S::Home, GRAIN).needs(B::Mill),
        Sailor => j(R::Haul, C::Goats, O::Goats, 0.7, 0.4, S::Coast, BLUE).needs(B::Dock),
        Explorer => j(R::Haul, C::Haul, O::Lore, 0.5, 0.5, S::Home, GREEN),
        Colonist => j(R::Settle, C::Settle, O::None, 0.0, 0.3, S::Coast, GREEN).needs(B::Shipyard),
        Scholar => j(R::Work, C::Lore, O::Lore, 0.055, 0.2, S::Home, PURPLE),
        Artist => j(R::Work, C::Order, O::Order, 0.006, 0.2, S::Home, PURPLE).also(O::Faith, 0.003),
        Banker => j(R::Work, C::Goats, O::Goats, 0.03, 0.2, S::Home, GOLD).needs(B::Bank).elite(),
        Factory => j(R::Work, C::Goods, O::Goods, 0.06, 0.4, S::Home, DARK).needs(B::Factory),
        Rail => j(R::Haul, C::Haul, O::Goods, 0.2, 0.35, S::Home, DARK).needs(B::Railway),
        Miner => j(R::Gather, C::Goods, O::Goods, 0.3, 0.45, S::Hills, DARK).needs(B::Mine),
        Agitator => j(R::Work, C::Order, O::None, 0.0, 0.5, S::Home, RED),
        Reformer => j(R::Work, C::Order, O::Order, 0.008, 0.2, S::Home, WHITE).needs(B::Parliament),
        Electrician => j(R::Work, C::Goods, O::Goods, 0.08, 0.4, S::Home, [255, 240, 120]).needs(B::PowerPlant),
        Clerk => j(R::Work, C::Goats, O::Goats, 0.02, 0.2, S::Home, WHITE).also(O::Lore, 0.012),
        Programmer => j(R::Work, C::Lore, O::Lore, 0.14, 0.2, S::Home, CYAN),
        Broadcaster => j(R::Work, C::Order, O::Order, 0.01, 0.2, S::Home, CYAN).needs(B::Tower),
        Astronaut => j(R::Work, C::Goods, O::Goods, 0.08, 0.5, S::Home, WHITE).also(O::Lore, 0.05).needs(B::LaunchPad),
        Terraformer => j(R::Work, C::Food, O::Food, 0.2, 0.4, S::Home, GREEN).also(O::Goods, 0.04).needs(B::Starport),
    }
}

/// The need a job answers in a civilisation of `age`: hunters are its fighters until guards
/// arrive (Trade Routes), and food-getters after.
pub fn cat_of(job: Job, age: u8) -> Cat {
    if job == Job::Hunt {
        if age < Job::Guard.unlock_age() {
            Cat::Fight
        } else {
            Cat::Food
        }
    } else {
        info(job).cat
    }
}

impl Job {
    /// Base fighting strength (combat scales it by morale, wounds, terrain and the weapon).
    pub fn strength(self) -> f32 {
        info(self).strength
    }

    /// Does this job march and fight?
    pub fn fights(self) -> bool {
        matches!(info(self).role, Role::Fight | Role::Hunt)
    }

    pub fn role(self) -> Role {
        info(self).role
    }
}

/// The job a civilisation of `age` gives its fighters (the ages' soldiers).
pub fn fighter(age: u8, rng: &mut Rng, castle: bool, raiding: bool) -> Job {
    let ok = |j: Job| j.unlock_age() <= age;
    if ok(Job::Knight) && castle && rng.chance(0.25) {
        Job::Knight
    } else if ok(Job::Soldier) {
        Job::Soldier
    } else if ok(Job::Raid) && raiding && rng.chance(0.4) {
        Job::Raid
    } else if ok(Job::Warrior) {
        Job::Warrior
    } else if ok(Job::Guard) {
        Job::Guard
    } else {
        Job::Hunt
    }
}

/// A village building's effects while active (built, and not dormant in a Dark Age).
#[derive(Clone, Copy, Debug, Default)]
pub struct Fx {
    /// Extra people the village can hold.
    pub cap: f32,
    /// Extra food it can store before it spoils.
    pub store: f32,
    pub food: f32,
    pub goods: f32,
    pub lore: f32,
    pub faith: f32,
    pub order: f32,
    pub links: f32,
    pub heal: f32,
    /// Defenders here are behind walls (`terrain_mul::WALLS`).
    pub walls: bool,
    /// Agents work through the night.
    pub lights: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Any,
    Capital,
    Coast,
    Hills,
    /// A sizeable village (pop ≥ 8) or the capital.
    Town,
}

pub struct BInfo {
    pub fx: Fx,
    pub place: Place,
    /// 4×4 pixel sprite, top row first: '.' clear, 'r' the owner's colour, the rest a palette.
    pub sprite: [&'static str; 4],
}

const fn fx() -> Fx {
    Fx { cap: 0.0, store: 0.0, food: 0.0, goods: 0.0, lore: 0.0, faith: 0.0, order: 0.0, links: 0.0, heal: 0.0, walls: false, lights: false }
}

/// What every building does, where it may stand and what it looks like.
pub fn binfo(b: Building) -> BInfo {
    use Building::*;
    use Place as P;
    let (f, place, sprite): (Fx, Place, [&str; 4]) = match b {
        Campfire => (Fx { store: 8.0, order: 0.002, ..fx() }, P::Any, ["....", ".f..", "fyf.", "nnn."]),
        Hut => (Fx { cap: 2.0, store: 10.0, ..fx() }, P::Any, ["....", ".r..", "rrr.", "wdw."]),
        Workshop => (Fx { goods: 0.002, cap: 1.0, ..fx() }, P::Any, ["....", "kkkk", "wwww", "wdwn"]),
        Raft => (Fx { food: 0.01, ..fx() }, P::Coast, ["....", "....", "nnnn", "bbbb"]),
        Field => (Fx { food: 0.03, store: 6.0, ..fx() }, P::Any, ["....", "y.y.", "gygy", "ygyg"]),
        GoatPen => (Fx { food: 0.015, ..fx() }, P::Any, ["....", "n..n", "nhhn", "nnnn"]),
        Granary => (Fx { store: 45.0, ..fx() }, P::Any, [".rr.", "rrrr", "wyyw", "wwdw"]),
        Well => (Fx { cap: 2.0, heal: 0.01, ..fx() }, P::Any, ["....", "nnn.", "sbs.", "sss."]),
        Market => (Fx { links: 0.5, order: 0.002, ..fx() }, P::Town, ["....", "RhRh", "n..n", "nyyn"]),
        Dock => (Fx { links: 0.5, food: 0.015, ..fx() }, P::Coast, ["....", "....", "nnnn", "nbnb"]),
        Longhouse => (Fx { cap: 3.0, order: 0.003, ..fx() }, P::Town, ["....", "rrrr", "rrrr", "wdww"]),
        Palisade => (Fx { walls: true, ..fx() }, P::Any, ["n.n.", "nnnn", "nnnn", "nnnn"]),
        Mine => (Fx { goods: 0.003, ..fx() }, P::Hills, ["....", ".ss.", "sks.", "sks."]),
        Forge => (Fx { goods: 0.004, ..fx() }, P::Town, [".k..", ".kf.", "ssss", "sfds"]),
        Palace => (Fx { order: 0.01, cap: 3.0, ..fx() }, P::Capital, ["y..y", "rrrr", "hhhh", "hdhh"]),
        Road => (Fx { links: 0.5, order: 0.002, ..fx() }, P::Any, ["....", "....", "s.s.", ".s.s"]),
        Temple => (Fx { faith: 0.006, order: 0.004, ..fx() }, P::Town, [".y..", "rrr.", "hkh.", "hhh."]),
        Shrine => (Fx { faith: 0.004, ..fx() }, P::Any, ["....", ".y..", ".s..", "sss."]),
        Barracks => (Fx { order: 0.002, ..fx() }, P::Town, ["....", "RRRR", "wwww", "wdwk"]),
        Aqueduct => (Fx { cap: 4.0, heal: 0.01, food: 0.02, ..fx() }, P::Town, ["....", "bbbb", "s.s.", "s.s."]),
        Fort => (Fx { walls: true, ..fx() }, P::Any, ["R..s", "s..s", "ssss", "sdss"]),
        Castle => (Fx { walls: true, order: 0.004, cap: 2.0, ..fx() }, P::Town, ["s.Rs", "ssss", "s.ss", "sdss"]),
        Monastery => (Fx { lore: 0.006, heal: 0.01, faith: 0.003, ..fx() }, P::Any, [".h..", "oooo", "hkhh", "hdhh"]),
        Mill => (Fx { food: 0.06, store: 20.0, ..fx() }, P::Any, ["h..h", ".hh.", ".wn.", ".wd."]),
        Shipyard => (Fx { goods: 0.003, ..fx() }, P::Coast, ["....", "n..n", "nnnn", "bbbb"]),
        Port => (Fx { links: 1.5, ..fx() }, P::Coast, ["k...", "kn..", "nnnn", "bbbb"]),
        University => (Fx { lore: 0.03, ..fx() }, P::Capital, ["..y.", "rrrr", "h.h.", "hdhh"]),
        Bank => (Fx { order: 0.001, ..fx() }, P::Town, ["....", "ssss", "yhyh", "sdss"]),
        Theatre => (Fx { order: 0.006, faith: 0.002, ..fx() }, P::Town, ["....", ".pp.", "pppp", "pdpp"]),
        Factory => (Fx { goods: 0.02, cap: 3.0, ..fx() }, P::Any, ["k.k.", "k.k.", "oooo", "odoo"]),
        Railway => (Fx { links: 1.0, ..fx() }, P::Any, ["....", "....", "nnnn", "kmkm"]),
        Parliament => (Fx { order: 0.015, ..fx() }, P::Capital, ["..h.", ".hhh", "hhhh", "hdhh"]),
        PowerPlant => (Fx { goods: 0.03, ..fx() }, P::Town, ["mk..", "mk..", "mmmm", "mlml"]),
        StreetLights => (Fx { lights: true, order: 0.002, ..fx() }, P::Any, ["l..l", "k..k", "k..k", "k..k"]),
        Tower => (Fx { links: 1.5, order: 0.002, ..fx() }, P::Town, [".R..", ".m..", ".m..", "mmm."]),
        DataCentre => (Fx { lore: 0.08, links: 1.0, ..fx() }, P::Town, ["....", "kkkk", "kckc", "kkkk"]),
        LaunchPad => (Fx { goods: 0.02, lore: 0.03, ..fx() }, P::Capital, [".h..", ".h..", "hRh.", "ssss"]),
        Satellite => (Fx { links: 2.0, lore: 0.04, ..fx() }, P::Capital, ["m..m", ".mm.", ".m..", "...."]),
        Starport => (Fx { goods: 0.03, cap: 4.0, ..fx() }, P::Capital, [".c..", "cmc.", "mmmm", "smms"]),
    };
    BInfo { fx: f, place, sprite }
}

/// What it costs (in goods) to build.
pub fn cost(b: Building) -> f32 {
    1.0 + 0.6 * b.unlock_age() as f32
}

/// Is a building of this kind standing and active in a village of a civilisation in `age`?
#[inline]
pub fn has(v: &Village, b: Building, age: u8) -> bool {
    v.built & (1u64 << b as u8) != 0 && b.unlock_age() <= age
}

/// Sum the active buildings' effects.
pub fn village_fx(built: u64, age: u8) -> Fx {
    let mut t = Fx::default();
    let mut bits = built;
    while bits != 0 {
        let k = bits.trailing_zeros() as usize;
        bits &= bits - 1;
        let b = Building::ALL[k];
        if b.unlock_age() > age {
            continue;
        }
        let f = binfo(b).fx;
        t.cap += f.cap;
        t.store += f.store;
        t.food += f.food;
        t.goods += f.goods;
        t.lore += f.lore;
        t.faith += f.faith;
        t.order += f.order;
        t.links += f.links;
        t.heal += f.heal;
        t.walls |= f.walls;
        t.lights |= f.lights;
    }
    t
}

/// Food a village keeps before it spoils.
pub fn storage(fx: &Fx, pop: u16) -> f32 {
    20.0 + fx.store + pop as f32 * 6.0
}

/// The next building a village puts up, if it can afford one: the oldest unlocked one it lacks
/// (that may stand there).
pub fn next_building(w: &World, v: usize) -> Option<Building> {
    let vil = &w.villages[v];
    let f = &w.factions[vil.owner as usize];
    let age = f.age;
    let capital = f.capital as usize == v;
    for b in Building::ALL {
        if b.unlock_age() > age {
            break;
        }
        if vil.built & (1u64 << b as u8) != 0 {
            continue;
        }
        let ok = match binfo(b).place {
            Place::Any => true,
            Place::Capital => capital,
            Place::Coast => vil.coast,
            Place::Hills => vil.hills,
            Place::Town => capital || vil.pop >= 8,
        };
        if ok {
            return Some(b);
        }
    }
    None
}

/// Are the village (or for `Place::Capital` buildings, the capital) able to employ this job?
fn job_ok(w: &World, f: &Faction, v: &Village, job: Job) -> bool {
    let i = info(job);
    if job.unlock_age() > f.age {
        return false;
    }
    if let Some(b) = i.needs {
        if !has(v, b, f.age) {
            let cap = w.villages.get(f.capital as usize);
            let civ_wide = binfo(b).place == Place::Capital || matches!(b, Building::Temple | Building::Market | Building::Mine);
            if !(civ_wide && cap.map_or(false, |c| c.alive && has(c, b, f.age))) {
                return false;
            }
        }
    }
    match i.site {
        Site::Coast => v.coast,
        Site::Hills => v.hills,
        _ => true,
    }
}

/// How much a civilisation wants each kind of work, as shares of its people.
pub fn wants(w: &World, f: &Faction) -> [f32; CATS] {
    let t = f.traits;
    let m = &f.metrics;
    let age = f.age as f32;
    let calmish = w.inputs.calm.clamp(0.0, 1.0);
    let mut s = [0.0f32; CATS];
    let fight_level = w.fight_level[f.id as usize] as f32;
    s[Cat::Food as usize] = 0.55 + 0.3 * f.hungry + if m.food_days < 2.0 { 0.15 } else if m.food_days > 5.0 { -0.1 } else { 0.0 };
    s[Cat::Fight as usize] = if w.at_war[f.id as usize] {
        0.3 + 0.4 * t.aggressiveness
    } else {
        0.06 + 0.14 * t.aggressiveness + 0.02 * fight_level
    } + if f.fortify_t > 0.0 { 0.15 } else { 0.0 };
    s[Cat::Goods as usize] = if f.age >= 2 { 0.08 + 0.018 * age } else { 0.0 };
    s[Cat::Lore as usize] = 0.03 + 0.012 * age + 0.08 * w.inputs.focus.clamp(0.0, 1.0);
    s[Cat::Goats as usize] = 0.04 + 0.08 * t.friendliness;
    s[Cat::Faith as usize] = 0.03 + 0.12 * (1.0 - m.faith) + 0.04 * calmish;
    s[Cat::Order as usize] = 0.03 + 0.2 * (1.0 - m.stability);
    s[Cat::Haul as usize] = if f.villages > 1 { 0.05 } else { 0.01 };
    s
}

/// A job for a new agent of village `v`: the most-wanted kind of work the village can offer,
/// then one of that kind's unlocked jobs, newer ones more often.
pub fn choose(w: &mut World, v: u16) -> Job {
    let (fid, age) = {
        let vil = &w.villages[v as usize];
        (vil.owner, w.factions[vil.owner as usize].age)
    };
    let f = &w.factions[fid as usize];
    let want = wants(w, f);
    let have = f.cat_counts;
    let total: f32 = want.iter().sum::<f32>().max(1e-3);
    let pop = f.pop.max(1) as f32 + 1.0;
    let vil = &w.villages[v as usize];
    let mut cands: [Option<Job>; 8] = [None; 8];
    let mut order: [(f32, usize); CATS] = [(0.0, 0); CATS];
    for c in 0..CATS {
        order[c] = (want[c] / total * pop - have[c] as f32, c);
    }
    order.sort_by(|a, b| b.0.total_cmp(&a.0));
    let castle = w.villages.iter().any(|x| x.alive && x.owner == fid && has(x, Building::Castle, age));
    let raiding = (0..MAX_FACTIONS).any(|b| w.rel[fid as usize * MAX_FACTIONS + b].war == 1);
    // A village at war with its own crown (or its rebels) raises fighters.
    let rising = vil.camp != 0 && w.strifes.get(vil.camp as usize - 1).map_or(false, |s| s.alive && s.level >= crate::conflict::RAID);
    if rising && w.rng.chance(0.5) {
        return fighter(age, &mut w.rng, castle, false);
    }
    for &(_, c) in &order {
        if c == Cat::Settle as usize {
            continue;
        }
        if c == Cat::Fight as usize {
            return fighter(age, &mut w.rng, castle, raiding);
        }
        let mut n = 0;
        for job in Job::ALL {
            if info(job).cat as usize == c && info(job).role != Role::Fight && job_ok(w, f, vil, job) && n < cands.len() {
                cands[n] = Some(job);
                n += 1;
            }
        }
        if c == Cat::Food as usize && n < cands.len() && cat_of(Job::Hunt, age) == Cat::Food {
            cands[n] = Some(Job::Hunt);
            n += 1;
        }
        if n == 0 {
            continue;
        }
        let mut weights = [0.0f32; 8];
        let mut sum = 0.0;
        for k in 0..n {
            let ua = cands[k].unwrap().unlock_age() as f32;
            weights[k] = (ua + 2.0).powf(1.2);
            sum += weights[k];
        }
        let mut r = w.rng.f32() * sum;
        for k in 0..n {
            r -= weights[k];
            if r <= 0.0 {
                return cands[k].unwrap();
            }
        }
        return cands[n - 1].unwrap();
    }
    Job::Forage
}

/// Shift a few agents of each civilisation toward the work it lacks, retire jobs its age no
/// longer allows (a Dark Age), demobilise fighters in peace, appoint chiefs and generals.
pub fn rebalance(w: &mut World) {
    for fi in 0..MAX_FACTIONS {
        if !w.factions[fi].alive {
            continue;
        }
        let f = &w.factions[fi];
        let age = f.age;
        let peak = f.peak_age;
        let want = wants(w, f);
        let total: f32 = want.iter().sum::<f32>().max(1e-3);
        let pop = f.pop.max(1) as f32;
        let mut deficit = [0.0f32; CATS];
        for c in 0..CATS {
            deficit[c] = want[c] / total * pop - f.cat_counts[c] as f32;
        }
        let (most, need) = (0..CATS).filter(|&c| c != Cat::Settle as usize).map(|c| (c, deficit[c])).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        let (least, extra) = (0..CATS).filter(|&c| c != Cat::Settle as usize).map(|c| (c, deficit[c])).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        let shift = need > 1.5 && extra < -1.5;
        let mut moved = 0;
        let budget = ((pop / 40.0).ceil() as usize).clamp(1, 4);
        let need_chief = Job::Chief.unlock_age() <= age && !w.agents.iter().any(|a| a.alive && a.faction == fi as u8 && a.job == Job::Chief);
        let mut chief_done = !need_chief;
        let capital = f.capital;
        for i in 0..w.agents.len() {
            let a = &w.agents[i];
            if !a.alive || a.faction != fi as u8 || a.prisoner > 0.0 || a.home == NO_V {
                continue;
            }
            let job = a.job;
            let role = job.role();
            // A Dark Age: the lost age's jobs stop. (Jobs beyond anything the civilisation ever
            // reached were handed out by hand, by a test or a tool, and are left alone.)
            if job.unlock_age() > age && job.unlock_age() <= peak && role != Role::Settle {
                let nj = if job.fights() {
                    let mut r = std::mem::replace(&mut w.rng, Rng::new(0));
                    let j = fighter(age, &mut r, false, false);
                    w.rng = r;
                    j
                } else {
                    choose(w, w.agents[i].home)
                };
                set_job(w, i, nj);
                continue;
            }
            if !chief_done && a.home == capital && !job.fights() && a.age > 20.0 {
                set_job(w, i, Job::Chief);
                chief_done = true;
                continue;
            }
            if shift && moved < budget && cat_of(job, age) as usize == least && matches!(a.state, State::Idle | State::Sleep | State::Work) && role != Role::Lead {
                // Fighters only stand down in peace (and not in the middle of a rising).
                let rising = a.camp != 0 && w.strifes.get(a.camp as usize - 1).map_or(false, |s| s.alive && s.level >= crate::conflict::RAID);
                if job.fights() && (w.at_war[fi] || rising) {
                    continue;
                }
                let nj = choose_in(w, a.home, most);
                if let Some(nj) = nj {
                    set_job(w, i, nj);
                    moved += 1;
                }
            }
        }
    }
}

/// A job of category `c` for village `v`, if the village offers one.
fn choose_in(w: &mut World, v: u16, c: usize) -> Option<Job> {
    let fid = w.villages[v as usize].owner;
    let age = w.factions[fid as usize].age;
    if c == Cat::Fight as usize {
        let castle = w.villages.iter().any(|x| x.alive && x.owner == fid && has(x, Building::Castle, age));
        return Some(fighter(age, &mut w.rng, castle, false));
    }
    let f = &w.factions[fid as usize];
    let vil = &w.villages[v as usize];
    let mut best: Option<(f32, Job)> = None;
    for job in Job::ALL {
        if info(job).cat as usize == c && info(job).role != Role::Fight && job_ok(w, f, vil, job) {
            let s = job.unlock_age() as f32 + w.rng.f32() * 4.0;
            if best.map_or(true, |b| s > b.0) {
                best = Some((s, job));
            }
        }
    }
    best.map(|b| b.1)
}

/// Change an agent's job (and reset what it was doing).
pub fn set_job(w: &mut World, i: usize, job: Job) {
    let a = &mut w.agents[i];
    if a.job == job {
        return;
    }
    a.job = job;
    a.state = State::Idle;
    a.target = NO_V;
    a.timer = 0.3;
    a.garrison = job == Job::Guard;
}

/// Let villages put up buildings from the goods their workers make (the capital also draws
/// on the treasury).
pub fn construct(w: &mut World) {
    for v in 0..w.villages.len() {
        if !w.villages[v].alive {
            continue;
        }
        let owner = w.villages[v].owner as usize;
        if w.villages[v].goods < 3.0 && w.factions[owner].capital as usize != v {
            continue;
        }
        let Some(b) = next_building(w, v) else { continue };
        let c = cost(b);
        if w.factions[owner].capital as usize == v && w.villages[v].goods < c && w.factions[owner].treasury > 4.0 {
            let take = (c - w.villages[v].goods).min(w.factions[owner].treasury * 0.5);
            w.factions[owner].treasury -= take;
            w.villages[v].goods += take;
        }
        if w.villages[v].goods >= c {
            w.villages[v].goods -= c;
            w.villages[v].built |= 1u64 << b as u8;
            w.stats.buildings += 1;
            let age = w.factions[owner].age;
            w.villages[v].fx = village_fx(w.villages[v].built, age);
        }
    }
}
