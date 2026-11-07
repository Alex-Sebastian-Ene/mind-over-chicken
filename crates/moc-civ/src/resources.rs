//! Map resources (#165): deposits and fields that civilisations work, trade, covet and fight
//! over.
//!
//! **Placement** ([`Resources::place`]) reads tile kinds only, never the continent shapes:
//! - rich soil on river plains and wide grassland;
//! - timber in deep forest;
//! - stone in the hills;
//! - fish on coasts with shallow water;
//! - copper where hills meet mountains, iron in rugged hills, uranium deep in mountain ranges;
//! - coal in forested hills;
//! - horses on open steppe;
//! - oil in deserts and on sandy shelves.
//!
//! How many of each depends on the planet's land, and they are spread apart.
//!
//! **Kinds.** Soil, timber, stone and fish are worked from the first ages. The strategic kinds
//! come with the ages ([`Res::unlock`]): horses (Chiefdoms), copper (Bronze Age), iron (Empires),
//! coal (Industrial Revolution), oil (Electric Age) and uranium (Space Age).
//!
//! **Working them.** A deposit inside a civilisation's territory is held by it: the owner of
//! its centre tile, and the village claiming that tile works it.
//! - The village takes a steady share out every second (more with more hands). Food kinds feed
//!   it; the rest become goods, so production and the age conditions grow with them.
//! - Its gatherers go to the deposit first ([`gather_spot`]): farmers to the soil, woodcutters
//!   to the timber, miners to the ore, fishers to the fishing grounds. There they bring home
//!   more ([`harvest`]) and drain it faster.
//! - Mines, quarries and wells run dry. Soil, timber, fish and horses regrow slowly, and much
//!   more slowly once they have been run dry, so scarcity changes over a run.
//! - Strategic kinds also arm the people that has them ([`Resources::arms`]). A people that
//!   lacks the metal of its age fights worse.
//!
//! **Wanting and trading.**
//! - Each second every people measures what it needs ([`need`]): the strategic kind of its age
//!   (or the next one) above all, soil when hungry, timber and stone for building.
//! - It covets the best deposit a neighbour holds near its frontier ([`Resources::covet`]).
//!   Covetousness raises tension ([`pressure`]) and becomes a war goal: the rules declare
//!   `Decision::ClaimDeposit` instead of a bare war.
//! - Neighbours at ease with each other that each hold what the other lacks trade it.
//!   Both then have the kind (at a lower bonus), count as trade links, and cool toward each
//!   other. Allies share what they hold (`pacts`).
//!
//! **A war for a deposit ends with it changing hands.**
//! - Taking the village it lies in seizes it ([`on_capture`]); the victor then offers peace.
//! - A peace the claimant was winning makes the loser cede that village ([`on_peace`]).
//! - The news says why: "the Varostans march on the Elumans for the iron of the Red Hills".
//!
//! Determinism: placement has its own RNG from the run seed; the tick uses no randomness.

use moc_planet::rng::Rng;

use crate::jobs::{Building, Job, Site};
use crate::news::{NewsLine, TONE_BAD, TONE_GOOD, TONE_WARN};
use crate::terrain::{Terrain, DEEP, FOREST, GRASS, HILLS, MOUNTAIN, SAND, WATER};
use crate::world::*;

/// No deposit.
pub const NO_DEP: u16 = u16::MAX;

/// Event kinds of resources, pacts and the land (#165 holds 40–49; `ev` has 1–34).
pub mod rev {
    /// a takes deposit `extra` from b with the village it lies in.
    pub const SEIZE: i32 = 40;
    /// b cedes deposit `extra` (and its village) to a at the peace.
    pub const CEDE: i32 = 41;
    /// a and b swear a defensive pact.
    pub const PACT: i32 = 42;
    /// a breaks its pact with b to take what b holds.
    pub const BETRAY: i32 = 43;
    /// The pact between a and b lapses.
    pub const LAPSE: i32 = 44;
    /// Deposit `extra`, held by a, runs dry.
    pub const SPENT: i32 = 45;
    /// a learns to use kind n (`Res`) and holds none: extra is the nearest such deposit (-1 none).
    pub const LEARN: i32 = 46;
    /// a improves its land: n is the work (`develop::Work`), extra the tiles so far.
    pub const GREEN: i32 = 47;
    /// a's land wears out: n tiles stripped or turned to dust lately.
    pub const ERODE: i32 = 48;
    /// a and b trade: extra = what a gives | what b gives << 8 (`Res`).
    pub const BARTER: i32 = 49;
}

/// What a deposit holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Res {
    Soil = 0,
    Timber = 1,
    Stone = 2,
    Fish = 3,
    Copper = 4,
    Horses = 5,
    Iron = 6,
    Coal = 7,
    Oil = 8,
    Uranium = 9,
}

pub const KINDS: usize = 10;

impl Res {
    pub const ALL: [Res; KINDS] = [Res::Soil, Res::Timber, Res::Stone, Res::Fish, Res::Copper, Res::Horses, Res::Iron, Res::Coal, Res::Oil, Res::Uranium];

    pub fn from(i: u8) -> Res {
        Res::ALL[(i as usize).min(KINDS - 1)]
    }

    pub fn key(self) -> &'static str {
        ["soil", "timber", "stone", "fish", "copper", "horses", "iron", "coal", "oil", "uranium"][self as usize]
    }

    /// How a headline names it ("the iron").
    pub fn words(self) -> &'static str {
        ["the rich soil", "the timber", "the stone", "the fishing grounds", "the copper", "the horses", "the iron", "the coal", "the oil", "the uranium"][self as usize]
    }

    /// The age from which it is worked and wanted.
    pub fn unlock(self) -> u8 {
        [1, 2, 3, 3, 8, 7, 11, 15, 17, 19][self as usize]
    }

    /// The kinds the ages bring, worth a war.
    pub fn strategic(self) -> bool {
        self as u8 >= Res::Copper as u8
    }

    /// Grows back after use.
    pub fn renewable(self) -> bool {
        matches!(self, Res::Soil | Res::Timber | Res::Fish | Res::Horses)
    }

    /// Worked for food (the rest make goods).
    pub fn food(self) -> bool {
        matches!(self, Res::Soil | Res::Fish | Res::Horses)
    }

    /// Units in a fresh deposit.
    pub fn size(self) -> f32 {
        [300.0, 260.0, 900.0, 260.0, 420.0, 220.0, 560.0, 760.0, 720.0, 420.0][self as usize]
    }

    /// Units its village takes out per second with a full crew.
    pub fn rate(self) -> f32 {
        [0.06, 0.03, 0.025, 0.06, 0.04, 0.03, 0.05, 0.09, 0.11, 0.09][self as usize]
    }

    /// Regrowth per second, as a share of its size.
    pub fn regen(self) -> f32 {
        [0.0005, 0.0006, 0.0, 0.0008, 0.0, 0.0006, 0.0, 0.0, 0.0, 0.0][self as usize]
    }

    /// Deposits per 100 000 land tiles.
    pub fn density(self) -> f32 {
        [9.0, 8.0, 5.0, 7.0, 4.0, 3.5, 4.0, 3.2, 2.6, 2.0][self as usize]
    }

    /// Tiles round its centre it covers.
    pub fn radius(self) -> i32 {
        [4, 4, 2, 3, 2, 4, 2, 3, 2, 2][self as usize]
    }

    /// Fighting bonus while held.
    pub fn arms(self) -> f32 {
        [0.0, 0.0, 0.0, 0.0, 0.12, 0.08, 0.15, 0.0, 0.08, 0.06][self as usize]
    }

    /// Its colour on the overview map.
    pub fn colour(self) -> [u8; 3] {
        [[110, 210, 80], [40, 130, 50], [180, 180, 190], [120, 180, 230], [230, 130, 60], [200, 150, 90], [190, 196, 214], [30, 30, 36], [150, 80, 200], [150, 255, 90]][self as usize]
    }
}

/// One deposit or field on the map.
#[derive(Clone, Copy, Debug)]
pub struct Deposit {
    /// Centre tile (always walkable land).
    pub x: i32,
    pub y: i32,
    pub kind: Res,
    pub r: i32,
    /// Units left, and in a fresh one.
    pub amount: f32,
    pub size: f32,
    /// Run dry (a renewable then regrows a quarter as fast until it is a third full again).
    pub spent: bool,
    /// Holder (the owner of the centre tile; NO_F nobody) and the village whose land it is in.
    pub owner: u8,
    pub village: u16,
    /// Units taken out last second (a worked deposit sparkles).
    pub worked: f32,
    /// When it was last a war goal (its icon flashes while it is fought over).
    pub fought_t: f32,
    /// Seed of its place name (`place_name`).
    pub name: u32,
}

impl Deposit {
    pub fn share(&self) -> f32 {
        (self.amount / self.size.max(1e-3)).clamp(0.0, 1.0)
    }
}

/// Running counts (the sandbox's resource stats, tests).
#[derive(Clone, Copy, Debug, Default)]
pub struct ResStats {
    pub claims: u32,
    pub seized: u32,
    pub ceded: u32,
    pub spent: u32,
    pub trades: u32,
    pub goals_given_up: u32,
}

/// The map's deposits and who wants what.
pub struct Resources {
    pub deps: Vec<Deposit>,
    /// Deposits by 16-tile cell (fixed after placement).
    cw: i32,
    ch: i32,
    cell_start: Vec<u32>,
    cell_items: Vec<u16>,
    /// Per people and kind: 0 none, 1 by trade or pact, 2 held.
    pub access: [[u8; KINDS]; MAX_FACTIONS],
    /// Per people and kind: deposits held that are not run dry.
    pub held: [[u16; KINDS]; MAX_FACTIONS],
    /// Per people and kind: how much it wants more (0..1).
    pub need: [[f32; KINDS]; MAX_FACTIONS],
    /// a's want for what b holds (MAX_FACTIONS², 0..1) and the deposit it wants most.
    pub covet: Vec<f32>,
    pub want: Vec<u16>,
    /// a's war goal against b (NO_DEP none).
    pub goal: Vec<u16>,
    /// What a gets from b by trade (kind + 1; 0 none).
    pub trade: Vec<u8>,
    trade_news_t: Vec<f32>,
    trade_deal: Vec<u16>,
    /// Fighting strength from strategic kinds, per people (1 = none).
    pub arms: [f32; MAX_FACTIONS],
    /// Per village: the deposit its farmers, woodcutters, miners and fishers work (NO_DEP none).
    pub vdep: Vec<[u16; 4]>,
    /// Forest tiles woodcutters thinned below `develop::STRIP`, for `develop` to clear.
    pub strip: Vec<u32>,
    /// Which peoples have a village in each 32-tile cell (scratch for coveting).
    vmask: Vec<u32>,
    /// The highest age any living people has (what is worth showing).
    pub max_age: u8,
    /// The age each people last had (to tell when it learns to use a kind).
    seen_age: [u8; MAX_FACTIONS],
    born: [f32; MAX_FACTIONS],
    pub stats: ResStats,
    /// Milliseconds spent in `tick` (native profiling).
    pub prof_ms: f64,
}

#[inline]
fn hash(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

const CELL: i32 = 16;

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Share of tiles in the (2r+1)² square round (x, y) passing `f`.
fn share(t: &Terrain, x: i32, y: i32, r: i32, f: impl Fn(u8) -> bool) -> f32 {
    let mut n = 0;
    for oy in -r..=r {
        for ox in -r..=r {
            if f(t.kind_at(x + ox, y + oy)) {
                n += 1;
            }
        }
    }
    n as f32 / ((2 * r + 1) * (2 * r + 1)) as f32
}

fn river_near(t: &Terrain, x: i32, y: i32, r: i32) -> bool {
    (-r..=r).any(|oy| (-r..=r).any(|ox| t.river_at(x + ox, y + oy)))
}

/// Does tile (x, y) suit a deposit of `k`? Reads tile kinds only.
pub fn suits(t: &Terrain, k: Res, x: i32, y: i32) -> bool {
    let kind = t.kind_at(x, y);
    if !t.walkable(x, y) {
        return false;
    }
    let water = |c: u8| c == WATER || c == DEEP;
    let mount = |c: u8| c == MOUNTAIN;
    let high = |c: u8| c == HILLS || c == MOUNTAIN;
    match k {
        Res::Soil => kind == GRASS && (river_near(t, x, y, 2) || share(t, x, y, 3, |c| c == GRASS) >= 0.85),
        Res::Timber => kind == FOREST && share(t, x, y, 3, |c| c == FOREST) >= 0.75,
        Res::Stone => kind == HILLS && share(t, x, y, 2, high) >= 0.55,
        Res::Fish => kind != HILLS && t.near_water(x, y, 1) && share(t, x, y, 4, |c| c == WATER) >= 0.2 && share(t, x, y, 4, water) >= 0.3,
        Res::Copper => kind == HILLS && {
            let m = share(t, x, y, 3, mount);
            m >= 0.04 && m <= 0.45
        },
        Res::Horses => kind == GRASS && share(t, x, y, 4, |c| c == GRASS || c == SAND) >= 0.8 && share(t, x, y, 4, |c| c == FOREST) <= 0.05,
        Res::Iron => kind == HILLS && share(t, x, y, 3, mount) >= 0.12 && share(t, x, y, 3, high) >= 0.6,
        Res::Coal => (kind == HILLS || kind == FOREST) && share(t, x, y, 3, |c| c == HILLS) >= 0.25 && share(t, x, y, 3, |c| c == FOREST) >= 0.15,
        Res::Oil => kind == SAND && (share(t, x, y, 4, |c| c == SAND) >= 0.6 || share(t, x, y, 4, water) >= 0.35),
        Res::Uranium => kind == HILLS && share(t, x, y, 3, mount) >= 0.3,
    }
}

const ADJ: [&str; 24] = [
    "Red", "Black", "Grey", "White", "Green", "Golden", "Stag", "Raven", "Wolf", "Long", "High", "Old", "Bitter", "Silver", "Misty", "Windy", "Broken", "Bright", "Far",
    "Cold", "Amber", "Thorn", "Hollow", "Eagle",
];

/// A deposit's place name: "the Red Hills".
pub fn place_name(t: &Terrain, d: &Deposit) -> String {
    let h = hash(d.name);
    let adj = match d.kind {
        Res::Iron if h & 1 == 0 => "Red",
        Res::Coal if h & 1 == 0 => "Black",
        Res::Copper if h & 1 == 0 => "Amber",
        Res::Uranium if h & 1 == 0 => "Bright",
        _ => ADJ[(h >> 3) as usize % ADJ.len()],
    };
    let rugged = share(t, d.x, d.y, 3, |c| c == MOUNTAIN) >= 0.3;
    let pick = |xs: &[&'static str]| xs[(h >> 9) as usize % xs.len()];
    let land = match d.kind {
        Res::Soil if river_near(t, d.x, d.y, 2) => pick(&["Riverlands", "Water Meadows", "Vale"]),
        Res::Soil => pick(&["Plains", "Fields", "Meadows", "Vale"]),
        Res::Timber => pick(&["Woods", "Forest", "Weald", "Thicket"]),
        Res::Fish => pick(&["Banks", "Shallows", "Bay", "Strand"]),
        Res::Horses => pick(&["Steppe", "Downs", "Grasslands"]),
        Res::Oil => pick(&["Sands", "Flats", "Dunes"]),
        Res::Coal if t.kind_at(d.x, d.y) == FOREST => pick(&["Woods", "Hollows"]),
        _ if rugged => pick(&["Peaks", "Crags", "Fells"]),
        _ => pick(&["Hills", "Downs", "Fells", "Ridge"]),
    };
    format!("the {adj} {land}")
}

impl Resources {
    /// Place the deposits on a planet. Same terrain and seed, same deposits.
    pub fn place(t: &Terrain, seed: u32) -> Resources {
        let d = t.d;
        let mut rng = Rng::new(seed ^ 0x0165_7e5d);
        let land = t.walk.iter().filter(|&&b| b).count() as f32;
        let (cw, ch) = ((d.w as i32 + CELL - 1) / CELL, (d.h as i32 + CELL - 1) / CELL);
        // Placement's own spacing grid (cells of 32 tiles).
        let (gw, gh) = ((d.w as i32 + 31) / 32, (d.h as i32 + 31) / 32);
        let mut near: Vec<Vec<u16>> = vec![Vec::new(); (gw * gh) as usize];
        let mut deps: Vec<Deposit> = Vec::new();
        // Candidate spots: one land tile at random in each 3×3 block, pooled by tile kind (so
        // even rare kinds of ground are found, in a few milliseconds on the biggest planet).
        const STEP: usize = 3;
        let mut pools: [Vec<(u16, u16)>; 5] = Default::default();
        for by in (0..d.h).step_by(STEP) {
            for bx in (0..d.w).step_by(STEP) {
                let (x, y) = ((bx + rng.below(STEP)).min(d.w - 1), (by + rng.below(STEP)).min(d.h - 1));
                let i = y * d.w + x;
                if !t.walk[i] {
                    continue;
                }
                let p = (x as u16, y as u16);
                pools[0].push(p);
                match t.kind[i] {
                    GRASS => pools[1].push(p),
                    FOREST => pools[2].push(p),
                    HILLS => pools[3].push(p),
                    SAND => pools[4].push(p),
                    _ => {}
                }
            }
        }
        for k in Res::ALL {
            let want = (k.density() * land / 100_000.0).round().max(if land > 20_000.0 { if k.strategic() { 2.0 } else { 3.0 } } else { 0.0 }) as usize;
            let pool = match k {
                Res::Soil | Res::Horses => &pools[1],
                Res::Timber => &pools[2],
                Res::Stone | Res::Copper | Res::Iron | Res::Uranium => &pools[3],
                Res::Oil => &pools[4],
                Res::Coal | Res::Fish => &pools[0],
            };
            let mut made = 0;
            // Walk the pool in a random order (a random start and a stride coprime to its length).
            let n = pool.len();
            let start = if n > 0 { rng.below(n) } else { 0 };
            let stride = {
                let mut s = 7919 + rng.below(7919);
                while n > 1 && gcd(s, n) != 1 {
                    s += 1;
                }
                s
            };
            let mut at = start;
            for _ in 0..n {
                if made >= want {
                    break;
                }
                let (x, y) = pool[at];
                at = (at + stride) % n;
                let (x, y) = (x as i32, y as i32);
                if !suits(t, k, x, y) {
                    continue;
                }
                // Apart from other deposits (much further from one of its own kind).
                let (gx, gy) = (x / 32, y / 32);
                let mut crowded = false;
                'look: for oy in -1..=1 {
                    for ox in -1..=1 {
                        let c = ((gy + oy).rem_euclid(gh) * gw + (gx + ox).rem_euclid(gw)) as usize;
                        for &j in &near[c] {
                            let o = &deps[j as usize];
                            let dist = d.dist(x as f32, y as f32, o.x as f32, o.y as f32);
                            if dist < 10.0 || (o.kind == k && dist < 26.0) {
                                crowded = true;
                                break 'look;
                            }
                        }
                    }
                }
                if crowded || deps.len() >= NO_DEP as usize - 1 {
                    continue;
                }
                near[(gy * gw + gx) as usize].push(deps.len() as u16);
                deps.push(Deposit {
                    x,
                    y,
                    kind: k,
                    r: k.radius(),
                    amount: k.size(),
                    size: k.size(),
                    spent: false,
                    owner: NO_F,
                    village: NO_V,
                    worked: 0.0,
                    fought_t: -1e9,
                    name: rng.next_u32(),
                });
                made += 1;
            }
        }
        let mut r = Resources {
            deps,
            cw,
            ch,
            cell_start: Vec::new(),
            cell_items: Vec::new(),
            access: [[0; KINDS]; MAX_FACTIONS],
            held: [[0; KINDS]; MAX_FACTIONS],
            need: [[0.0; KINDS]; MAX_FACTIONS],
            covet: vec![0.0; MAX_FACTIONS * MAX_FACTIONS],
            want: vec![NO_DEP; MAX_FACTIONS * MAX_FACTIONS],
            goal: vec![NO_DEP; MAX_FACTIONS * MAX_FACTIONS],
            trade: vec![0; MAX_FACTIONS * MAX_FACTIONS],
            trade_news_t: vec![-1e9; MAX_FACTIONS * MAX_FACTIONS],
            trade_deal: vec![0; MAX_FACTIONS * MAX_FACTIONS],
            arms: [1.0; MAX_FACTIONS],
            vdep: Vec::new(),
            strip: Vec::new(),
            vmask: Vec::new(),
            max_age: 1,
            seen_age: [0; MAX_FACTIONS],
            born: [f32::NAN; MAX_FACTIONS],
            stats: ResStats::default(),
            prof_ms: 0.0,
        };
        r.index(d);
        r
    }

    /// Rebuild the lookup grid: every 16-tile cell a deposit's square touches.
    fn index(&mut self, d: crate::planet::Dims) {
        let (cw, ch) = (self.cw, self.ch);
        let mut lists: Vec<Vec<u16>> = vec![Vec::new(); (cw * ch) as usize];
        for (i, dp) in self.deps.iter().enumerate() {
            let mut cells: Vec<usize> = Vec::new();
            for oy in [-dp.r, 0, dp.r] {
                for ox in [-dp.r, 0, dp.r] {
                    let (x, y) = (d.wrap_x(dp.x + ox), d.wrap_y(dp.y + oy));
                    let c = ((y / CELL).min(ch - 1) * cw + (x / CELL).min(cw - 1)) as usize;
                    if !cells.contains(&c) {
                        cells.push(c);
                    }
                }
            }
            for c in cells {
                lists[c].push(i as u16);
            }
        }
        self.cell_start = vec![0u32; (cw * ch + 1) as usize];
        self.cell_items.clear();
        for (c, l) in lists.iter().enumerate() {
            self.cell_start[c] = self.cell_items.len() as u32;
            self.cell_items.extend_from_slice(l);
        }
        self.cell_start[(cw * ch) as usize] = self.cell_items.len() as u32;
    }

    /// Put a deposit of `kind` at (x, y) by hand (tests, scenarios). Returns its index.
    pub fn add(&mut self, d: crate::planet::Dims, kind: Res, x: i32, y: i32) -> u16 {
        self.deps.push(Deposit {
            x: d.wrap_x(x),
            y: d.wrap_y(y),
            kind,
            r: kind.radius(),
            amount: kind.size(),
            size: kind.size(),
            spent: false,
            owner: NO_F,
            village: NO_V,
            worked: 0.0,
            fought_t: -1e9,
            name: hash(self.deps.len() as u32 ^ 0x5eed),
        });
        self.index(d);
        (self.deps.len() - 1) as u16
    }

    /// The deposit of one of the kinds in `mask` (bit per `Res`) covering tile (x, y).
    pub fn at(&self, d: crate::planet::Dims, x: i32, y: i32, mask: u16) -> Option<usize> {
        let (x, y) = (d.wrap_x(x), d.wrap_y(y));
        let c = ((y / CELL).min(self.ch - 1) * self.cw + (x / CELL).min(self.cw - 1)) as usize;
        for &i in &self.cell_items[self.cell_start[c] as usize..self.cell_start[c + 1] as usize] {
            let dp = &self.deps[i as usize];
            if mask & (1 << dp.kind as u8) != 0 && d.dxi(dp.x, x).abs() <= dp.r && d.dyi(dp.y, y).abs() <= dp.r {
                return Some(i as usize);
            }
        }
        None
    }

    #[inline]
    pub fn covet_of(&self, a: u8, b: u8) -> f32 {
        self.covet[a as usize * MAX_FACTIONS + b as usize]
    }

    /// a's war goal against b, if any.
    #[inline]
    pub fn goal_of(&self, a: u8, b: u8) -> Option<u16> {
        let g = self.goal[a as usize * MAX_FACTIONS + b as usize];
        (g != NO_DEP).then_some(g)
    }
}

/// Soil and steppe start rich: their tiles grow more (`develop`).
pub fn init(w: &mut World) {
    for k in 0..w.res.deps.len() {
        let dp = w.res.deps[k];
        let boost = match dp.kind {
            Res::Soil => 176,
            Res::Horses => 150,
            _ => continue,
        };
        for oy in -dp.r..=dp.r {
            for ox in -dp.r..=dp.r {
                if ox * ox + oy * oy > dp.r * dp.r + 1 {
                    continue;
                }
                let i = w.d.idx(dp.x + ox, dp.y + oy);
                if w.terrain.kind[i] == GRASS {
                    w.food.fert[i] = w.food.fert[i].max(boost);
                }
            }
        }
    }
}

/// The kinds a job works (bit per `Res`) in a people of `age`.
fn job_kinds(job: Job, age: u8) -> u16 {
    let bit = |k: Res| if k.unlock() <= age { 1u16 << k as u8 } else { 0 };
    match job {
        Job::Forage | Job::Farm => bit(Res::Soil),
        Job::Woodcut => bit(Res::Timber),
        Job::Fish => bit(Res::Fish),
        Job::Hunt => bit(Res::Horses),
        Job::Mine | Job::Miner => bit(Res::Stone) | bit(Res::Copper) | bit(Res::Iron) | bit(Res::Coal) | bit(Res::Uranium),
        _ => 0,
    }
}

/// Which `Resources::vdep` slot a gatherer's site uses.
fn slot_of_site(site: Site) -> Option<usize> {
    match site {
        Site::Any | Site::Field => Some(0),
        Site::Forest => Some(1),
        Site::Hills => Some(2),
        Site::Coast => Some(3),
        _ => None,
    }
}

fn slot_of_kind(k: Res) -> Option<usize> {
    match k {
        Res::Soil => Some(0),
        Res::Timber => Some(1),
        Res::Stone | Res::Copper | Res::Iron | Res::Coal | Res::Uranium => Some(2),
        Res::Fish => Some(3),
        _ => None,
    }
}

/// A tile of the deposit village `v` works for this kind of site, jittered within it.
pub fn gather_spot(res: &Resources, v: u16, site: Site, rng: &mut Rng, d: crate::planet::Dims) -> Option<(i32, i32)> {
    let slot = slot_of_site(site)?;
    let k = *res.vdep.get(v as usize)?.get(slot)?;
    if k == NO_DEP {
        return None;
    }
    let dp = &res.deps[k as usize];
    let span = (2 * dp.r + 1) as usize;
    Some((d.wrap_x(dp.x + rng.below(span) as i32 - dp.r), d.wrap_y(dp.y + rng.below(span) as i32 - dp.r)))
}

/// A gatherer of `job` finished work on tile `ti` = (x, y): its yield multiplier. A deposit of
/// the kind it works pays more and drains; woodcutters thin the forest they cut (`develop`).
pub fn harvest(res: &mut Resources, fert: &mut [u8], terrain: &Terrain, ti: usize, x: i32, y: i32, job: Job, age: u8) -> f32 {
    if job == Job::Woodcut && terrain.kind[ti] == FOREST {
        fert[ti] = fert[ti].saturating_sub(9);
        if fert[ti] < crate::develop::STRIP && res.strip.len() < 4096 {
            res.strip.push(ti as u32);
        }
    }
    let mask = job_kinds(job, age);
    if mask == 0 {
        return 1.0;
    }
    let Some(k) = res.at(terrain.d, x, y, mask) else { return 1.0 };
    let dp = &mut res.deps[k];
    if dp.amount <= 0.0 {
        return 1.0;
    }
    let (bonus, cost) = if dp.kind.food() { (1.6, 0.5) } else { (2.0, 0.35) };
    dp.amount = (dp.amount - cost).max(0.0);
    bonus
}

/// How much people `a` wants more of each kind (0..1), from what it holds now.
pub fn need(w: &World, a: u8) -> [f32; KINDS] {
    let f = &w.factions[a as usize];
    let held = &w.res.held[a as usize];
    let mut n = [0.0f32; KINDS];
    if !f.alive {
        return n;
    }
    let age = f.age;
    for k in Res::ALL {
        let h = held[k as usize];
        let ki = k as usize;
        if k.strategic() {
            // The kind of its age, or of the next: wanted badly while it has none.
            if age + 1 < k.unlock() {
                continue;
            }
            let ahead = if age < k.unlock() { 0.6 } else { 1.0 };
            let latest = Res::ALL.iter().filter(|r| r.strategic() && r.unlock() <= age + 1).map(|r| r.unlock()).max() == Some(k.unlock());
            n[ki] = ahead
                * match h {
                    0 => 1.0,
                    1 => 0.35,
                    _ => 0.1,
                }
                * if latest { 1.0 } else { 0.7 };
        } else if age >= k.unlock() {
            n[ki] = match k {
                Res::Soil => ((f.hungry * 2.0 + if f.metrics.food_days < 2.0 { 0.35 } else { 0.0 } - 0.15 * h as f32).clamp(0.0, 1.0)) * 0.8,
                Res::Fish => {
                    if f.metrics.coastal && h == 0 {
                        (f.hungry * 1.5 + 0.1).min(0.6)
                    } else {
                        0.0
                    }
                }
                Res::Timber => {
                    if h == 0 {
                        0.45
                    } else {
                        0.1
                    }
                }
                Res::Stone => {
                    if h == 0 && age >= 5 {
                        0.35
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
        }
    }
    n
}

/// Extra tension per second of a toward b from what it wants of b's (and what they trade).
pub fn pressure(w: &World, a: u8, b: u8) -> f32 {
    let k = a as usize * MAX_FACTIONS + b as usize;
    0.6 * w.res.covet[k] - if w.res.trade[k] != 0 { 0.25 } else { 0.0 }
}

/// Once a second, before diplomacy: who holds what, what they take out, what they lack, trade
/// between neighbours, and who covets whose deposits.
pub fn tick(w: &mut World, dt: f32) {
    #[cfg(not(target_arch = "wasm32"))]
    let t0 = std::time::Instant::now();
    let now = w.time;
    // A slot whose people died or was reused starts clean.
    for f in 0..MAX_FACTIONS {
        let fac = &w.factions[f];
        let fresh = !fac.alive || w.res.born[f].to_bits() != fac.born.to_bits();
        if fresh {
            w.res.born[f] = if fac.alive { fac.born } else { f32::NAN };
            w.res.seen_age[f] = if fac.alive { fac.age } else { 0 };
            for o in 0..MAX_FACTIONS {
                for k in [f * MAX_FACTIONS + o, o * MAX_FACTIONS + f] {
                    w.res.goal[k] = NO_DEP;
                    w.res.covet[k] = 0.0;
                    w.res.want[k] = NO_DEP;
                    w.res.trade[k] = 0;
                    w.res.trade_deal[k] = 0;
                }
            }
        }
    }
    w.res.max_age = w.factions.iter().filter(|f| f.alive).map(|f| f.age).max().unwrap_or(1);
    // ---- holders, extraction, regrowth ----
    w.res.held = [[0; KINDS]; MAX_FACTIONS];
    if w.res.vdep.len() != w.villages.len() {
        w.res.vdep.resize(w.villages.len(), [NO_DEP; 4]);
    }
    w.res.vdep.iter_mut().for_each(|s| *s = [NO_DEP; 4]);
    let mut spent: Vec<usize> = Vec::new();
    for k in 0..w.res.deps.len() {
        let dp = w.res.deps[k];
        let i = w.d.idx(dp.x, dp.y);
        let (o, v) = (w.owner[i], w.claim[i]);
        let ok = o != NO_F && w.factions[o as usize].alive && (v as usize) < w.villages.len() && w.villages[v as usize].alive && w.villages[v as usize].owner == o;
        let mut taken = 0.0;
        if ok && w.factions[o as usize].age >= dp.kind.unlock() && dp.amount > 0.0 {
            let vil = &mut w.villages[v as usize];
            let crew = (vil.pop.min(12) as f32 / 12.0).max(0.0);
            taken = (dp.kind.rate() * (0.25 + 0.75 * crew) * dt).min(dp.amount);
            if dp.kind.food() {
                vil.food += taken;
            } else {
                vil.goods += taken;
                w.factions[o as usize].prod_acc += taken;
            }
        }
        let dpm = &mut w.res.deps[k];
        dpm.owner = if ok { o } else { NO_F };
        dpm.village = if ok { v } else { NO_V };
        dpm.worked = taken;
        dpm.amount -= taken;
        if dpm.kind.renewable() {
            dpm.amount = (dpm.amount + dpm.kind.regen() * dpm.size * dt * if dpm.spent { 0.25 } else { 1.0 }).min(dpm.size);
            if dpm.spent && dpm.amount > 0.33 * dpm.size {
                dpm.spent = false;
            }
        }
        if dpm.amount <= 0.0 && !dpm.spent {
            dpm.amount = 0.0;
            dpm.spent = true;
            spent.push(k);
        }
        let dp = w.res.deps[k];
        if ok && !dp.spent {
            w.res.held[o as usize][dp.kind as usize] += 1;
            if w.factions[o as usize].age >= dp.kind.unlock() {
                if let Some(s) = slot_of_kind(dp.kind) {
                    let cur = w.res.vdep[v as usize][s];
                    // Miners work the most valuable ore their age allows.
                    let better = cur == NO_DEP || {
                        let c = &w.res.deps[cur as usize];
                        (dp.kind as u8, (dp.share() * 100.0) as u32) > (c.kind as u8, (c.share() * 100.0) as u32)
                    };
                    if better {
                        w.res.vdep[v as usize][s] = k as u16;
                    }
                }
            }
        }
    }
    for k in spent {
        let dp = w.res.deps[k];
        w.res.stats.spent += 1;
        if dp.owner != NO_F && (dp.kind.strategic() || dp.kind == Res::Timber) {
            w.log_x(rev::SPENT, dp.owner, NO_F, dp.x, dp.y, dp.kind as i32, k as i32);
        }
    }
    // ---- needs ----
    for a in 0..MAX_FACTIONS as u8 {
        w.res.need[a as usize] = need(w, a);
    }
    // ---- access: held, then traded between neighbours at ease, shared between allies ----
    for f in 0..MAX_FACTIONS {
        for k in 0..KINDS {
            w.res.access[f][k] = if w.res.held[f][k] > 0 { 2 } else { 0 };
        }
    }
    w.res.trade.iter_mut().for_each(|t| *t = 0);
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    for (ia, &a) in alive.iter().enumerate() {
        for &b in &alive[ia + 1..] {
            let (r, rb) = (*w.rel(a, b), *w.rel(b, a));
            if !r.met || r.war != 0 || rb.war != 0 {
                continue;
            }
            if crate::pacts::allied(w, a, b) {
                for k in 0..KINDS {
                    if Res::from(k as u8).strategic() {
                        for (p, q) in [(a, b), (b, a)] {
                            if w.res.held[p as usize][k] > 0 && w.res.access[q as usize][k] == 0 {
                                w.res.access[q as usize][k] = 1;
                            }
                        }
                    }
                }
            }
            if !(r.near || crate::pacts::allied(w, a, b)) || r.tension > 0.25 || rb.tension > 0.25 {
                continue;
            }
            // Each gives what it holds and the other lacks and wants most.
            let gives = |p: u8, q: u8| -> Option<usize> {
                (0..KINDS)
                    .filter(|&k| w.res.held[p as usize][k] > 0 && w.res.held[q as usize][k] == 0 && w.res.need[q as usize][k] >= 0.3)
                    .max_by(|&x, &y| w.res.need[q as usize][x].total_cmp(&w.res.need[q as usize][y]).then(y.cmp(&x)))
            };
            let (Some(ka), Some(kb)) = (gives(a, b), gives(b, a)) else { continue };
            w.res.access[b as usize][ka] = w.res.access[b as usize][ka].max(1);
            w.res.access[a as usize][kb] = w.res.access[a as usize][kb].max(1);
            w.res.trade[a as usize * MAX_FACTIONS + b as usize] = kb as u8 + 1;
            w.res.trade[b as usize * MAX_FACTIONS + a as usize] = ka as u8 + 1;
            w.rel_mut(a, b).traded_t = 0.0;
            w.rel_mut(b, a).traded_t = 0.0;
            let pk = a as usize * MAX_FACTIONS + b as usize;
            // News of a new trade, not of every caravan.
            let deal = (ka as u16 + 1) | (kb as u16 + 1) << 8;
            if w.res.trade_deal[pk] != deal && now - w.res.trade_news_t[pk] > 150.0 {
                w.res.trade_news_t[pk] = now;
                w.res.trade_deal[pk] = deal;
                w.res.stats.trades += 1;
                let p = w.near_pairs[pk];
                let (x, y) = if p.1 != NO_V { (w.villages[p.1 as usize].x, w.villages[p.1 as usize].y) } else { w.factions[a as usize].start };
                w.log_x(rev::BARTER, a, b, x, y, 0, ka as i32 | (kb as i32) << 8);
            }
        }
    }
    // ---- arms ----
    for f in 0..MAX_FACTIONS {
        let age = w.factions[f].age;
        let mut m = 1.0f32;
        let mut metal = None;
        for k in Res::ALL {
            if k.arms() <= 0.0 || age < k.unlock() {
                continue;
            }
            m *= 1.0
                + k.arms()
                    * match w.res.access[f][k as usize] {
                        2 => 1.0,
                        1 => 0.6,
                        _ => 0.0,
                    };
            if matches!(k, Res::Copper | Res::Iron) {
                metal = Some(k);
            }
        }
        // Bronze or iron age without the metal: worse weapons.
        if let Some(k) = metal {
            if w.res.access[f][k as usize] == 0 {
                m *= 0.88;
            }
        }
        w.res.arms[f] = m.clamp(0.8, 1.6);
    }
    // ---- covetousness: the best deposit each neighbour holds near one of its villages ----
    // Which peoples have a village in each 32-tile cell (a bit each).
    let (gw, gh) = ((w.d.w as i32 + 31) / 32, (w.d.h as i32 + 31) / 32);
    let mut vmask = std::mem::take(&mut w.res.vmask);
    vmask.clear();
    vmask.resize((gw * gh) as usize, 0);
    for v in w.villages.iter().filter(|v| v.alive) {
        vmask[((v.y / 32) * gw + v.x / 32) as usize] |= 1 << v.owner;
    }
    w.res.covet.iter_mut().for_each(|c| *c = 0.0);
    w.res.want.iter_mut().for_each(|c| *c = NO_DEP);
    for k in 0..w.res.deps.len() {
        let dp = w.res.deps[k];
        let b = dp.owner;
        if b == NO_F || dp.spent {
            continue;
        }
        // Nearest cell (in cells) with a village of each people, up to 2 cells off.
        let (cx, cy) = (dp.x / 32, dp.y / 32);
        let mut near = [u8::MAX; MAX_FACTIONS];
        let mut any = 0u32;
        for oy in -2..=2i32 {
            for ox in -2..=2i32 {
                let m = vmask[((cy + oy).rem_euclid(gh) * gw + (cx + ox).rem_euclid(gw)) as usize] & !(1 << b);
                if m == 0 {
                    continue;
                }
                any |= m;
                let ring = ox.abs().max(oy.abs()) as u8;
                let mut bits = m;
                while bits != 0 {
                    let f = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    near[f] = near[f].min(ring);
                }
            }
        }
        while any != 0 {
            let a = any.trailing_zeros() as usize;
            any &= any - 1;
            if !w.factions[a].alive {
                continue;
            }
            let n = w.res.need[a][dp.kind as usize] * if w.res.access[a][dp.kind as usize] == 1 { 0.5 } else { 1.0 };
            if n <= 0.05 {
                continue;
            }
            let pk = a * MAX_FACTIONS + b as usize;
            let r = &w.rel[pk];
            // Met, and able to march there (a village pair on one landmass).
            if !r.met || w.near_pairs[pk].1 == NO_V {
                continue;
            }
            let v = n * (0.4 + 0.6 * dp.share()) * (1.0 - 0.25 * near[a] as f32);
            if v > w.res.covet[pk] {
                w.res.covet[pk] = v;
                w.res.want[pk] = k as u16;
            }
        }
    }
    w.res.vmask = vmask;
    // ---- war goals: kept while the war lasts and the deposit is still to be had ----
    for a in 0..MAX_FACTIONS {
        for b in 0..MAX_FACTIONS {
            let pk = a * MAX_FACTIONS + b;
            let g = w.res.goal[pk];
            if g == NO_DEP {
                continue;
            }
            let o = w.res.deps[g as usize].owner;
            if w.rel[pk].war != 2 || (o != b as u8 && o != a as u8) {
                w.res.goal[pk] = NO_DEP;
                if w.rel[pk].war == 2 {
                    w.res.stats.goals_given_up += 1;
                }
                continue;
            }
            w.res.deps[g as usize].fought_t = now;
        }
    }
    // ---- a people learns to use a strategic kind it has none of ----
    for &a in &alive {
        let (age, seen) = (w.factions[a as usize].age, w.res.seen_age[a as usize]);
        if age <= seen {
            w.res.seen_age[a as usize] = age;
            continue;
        }
        w.res.seen_age[a as usize] = age;
        for k in Res::ALL {
            if !k.strategic() || k == Res::Horses || k.unlock() != age || w.res.held[a as usize][k as usize] > 0 {
                continue;
            }
            let (cx, cy) = w.villages.get(w.factions[a as usize].capital as usize).filter(|v| v.alive).map(|v| (v.x, v.y)).unwrap_or(w.factions[a as usize].start);
            let near = (0..w.res.deps.len())
                .filter(|&i| w.res.deps[i].kind == k && !w.res.deps[i].spent)
                .min_by(|&i, &j| {
                    let (p, q) = (&w.res.deps[i], &w.res.deps[j]);
                    w.d.dist(cx as f32, cy as f32, p.x as f32, p.y as f32).total_cmp(&w.d.dist(cx as f32, cy as f32, q.x as f32, q.y as f32))
                });
            let (x, y) = near.map(|i| (w.res.deps[i].x, w.res.deps[i].y)).unwrap_or((cx, cy));
            w.log_x(rev::LEARN, a, NO_F, x, y, k as i32, near.map_or(-1, |i| i as i32));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        w.res.prof_ms += t0.elapsed().as_secs_f64() * 1000.0;
    }
}

/// The deposit a would go to war on b for now, if it wants one badly and is strong enough to
/// take it (the rules' resource war, before tension alone would start one).
pub fn war_target(w: &World, a: u8, b: u8) -> Option<u16> {
    let pk = a as usize * MAX_FACTIONS + b as usize;
    let (c, want) = (w.res.covet[pk], w.res.want[pk]);
    if want == NO_DEP || c < 0.35 || w.rel(a, b).tension < 0.55 {
        return None;
    }
    let (fa, fb) = (&w.factions[a as usize], &w.factions[b as usize]);
    let strength = |f: &Faction| (f.pop as f32 + 2.0 * f.warriors as f32) * w.res.arms[f.id as usize];
    (strength(fa) >= 1.15 * strength(fb)).then_some(want)
}

/// The deposit a would name as its goal on going to war with b (its strongest want), if any.
pub fn claim_for(w: &World, a: u8, b: u8) -> Option<u16> {
    let pk = a as usize * MAX_FACTIONS + b as usize;
    let want = w.res.want[pk];
    (want != NO_DEP && w.res.covet[pk] >= 0.08).then_some(want)
}

/// What rebels `c` rising against `parent` would fight for: the best deposit the parent
/// still holds near the rebels' new capital.
pub fn rebel_claim(w: &World, c: u8, parent: u8) -> Option<u16> {
    let cap = w.villages.get(w.factions[c as usize].capital as usize).filter(|v| v.alive)?;
    let age = w.factions[parent as usize].age;
    let mut best: Option<(f32, u16)> = None;
    for (k, dp) in w.res.deps.iter().enumerate() {
        if dp.spent || dp.kind.unlock() > age + 1 || w.owner[w.d.idx(dp.x, dp.y)] != parent {
            continue;
        }
        let dist = w.d.dist(cap.x as f32, cap.y as f32, dp.x as f32, dp.y as f32);
        if dist > 80.0 {
            continue;
        }
        let v = if dp.kind.strategic() { 1.0 } else { 0.5 } * (0.4 + 0.6 * dp.share()) * (1.0 - dist / 100.0);
        if best.map_or(true, |b| v > b.0) {
            best = Some((v, k as u16));
        }
    }
    best.map(|b| b.1)
}

/// a holds what it went to war on b for.
pub fn goal_won(w: &World, a: u8, b: u8) -> bool {
    w.res.goal_of(a, b).map_or(false, |g| w.res.deps.get(g as usize).map_or(false, |d| d.owner == a))
}

/// Is deposit `d` something a can claim from b (b holds it)?
pub fn claimable(w: &World, b: u8, d: u16) -> bool {
    w.res.deps.get(d as usize).map_or(false, |dp| w.owner[w.d.idx(dp.x, dp.y)] == b)
}

/// Make deposit `d` a's war goal against b.
pub fn set_goal(w: &mut World, a: u8, b: u8, d: u16) {
    w.res.goal[a as usize * MAX_FACTIONS + b as usize] = d;
    w.res.deps[d as usize].fought_t = w.time;
    w.res.stats.claims += 1;
}

/// A village changed hands in war: deposits on its land go with it.
pub fn on_capture(w: &mut World, v: u16, old: u8, by: u8) {
    for k in 0..w.res.deps.len() {
        let dp = w.res.deps[k];
        let i = w.d.idx(dp.x, dp.y);
        if w.claim[i] != v {
            continue;
        }
        w.res.deps[k].owner = by;
        let goal = w.res.goal_of(by, old) == Some(k as u16);
        if goal || (dp.kind.strategic() && w.factions[by as usize].age + 1 >= dp.kind.unlock() && !dp.spent) {
            w.res.stats.seized += 1;
            w.log_x(rev::SEIZE, by, old, dp.x, dp.y, dp.kind as i32, k as i32);
        }
    }
}

/// a and b made peace: a war goal the claimant was winning is ceded with its village.
pub fn on_peace(w: &mut World, a: u8, b: u8) {
    for (p, q) in [(a, b), (b, a)] {
        let pk = p as usize * MAX_FACTIONS + q as usize;
        let g = w.res.goal[pk];
        w.res.goal[pk] = NO_DEP;
        if g == NO_DEP {
            continue;
        }
        let dp = w.res.deps[g as usize];
        if dp.owner == p {
            continue;
        }
        let (fp, fq) = (&w.factions[p as usize], &w.factions[q as usize]);
        let winning = fq.recent_losses > 1.3 * fp.recent_losses.max(0.5) || fp.pop as f32 > 1.6 * fq.pop as f32;
        let i = w.d.idx(dp.x, dp.y);
        let v = w.claim[i];
        let ok = winning && v != NO_V && w.villages[v as usize].alive && w.villages[v as usize].owner == q && w.factions[q as usize].capital != v && w.factions[q as usize].villages > 1;
        if !ok {
            w.res.stats.goals_given_up += 1;
            continue;
        }
        cede_village(w, v, p);
        w.res.deps[g as usize].owner = p;
        w.res.stats.ceded += 1;
        w.log_x(rev::CEDE, p, q, dp.x, dp.y, dp.kind as i32, g as i32);
    }
}

/// Hand a village (and its land and people) to another people by treaty.
pub fn cede_village(w: &mut World, v: u16, to: u8) {
    let old = w.villages[v as usize].owner;
    if old == to {
        return;
    }
    {
        let age = w.factions[to as usize].age;
        let vil = &mut w.villages[v as usize];
        vil.owner = to;
        vil.siege = 0.0;
        vil.besieger = NO_F;
        vil.unrest = vil.unrest.max(0.3);
        vil.camp = 0;
        vil.side = 0;
        vil.fx = crate::jobs::village_fx(vil.built, age);
    }
    w.transfer_land(v, to);
    for a in w.agents.iter_mut() {
        if a.alive && a.faction == old && a.home == v {
            a.faction = to;
            a.camp = 0;
            a.target = NO_V;
            if matches!(a.state, State::March | State::Siege) {
                a.state = State::ToHome;
            }
        }
    }
    w.rel_mut(old, to).grudge += 0.2;
    w.recount();
}

/// How much a settler of people `f` likes a site at (x, y) for the deposits nobody holds near
/// it that `f` needs (settlers go where the ore is).
pub fn site_bonus(w: &World, f: u8, x: i32, y: i32) -> f32 {
    let r = &w.res;
    if r.deps.is_empty() {
        return 0.0;
    }
    let age = w.factions[f as usize].age;
    let (x, y) = (w.d.wrap_x(x), w.d.wrap_y(y));
    let (cx, cy) = (x / CELL, y / CELL);
    let mut best = 0.0f32;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let c = ((cy + oy).rem_euclid(r.ch) * r.cw + (cx + ox).rem_euclid(r.cw)) as usize;
            for &i in &r.cell_items[r.cell_start[c] as usize..r.cell_start[c + 1] as usize] {
                let dp = &r.deps[i as usize];
                if dp.owner != NO_F || dp.spent || dp.kind.unlock() > age + 2 {
                    continue;
                }
                let dist = w.d.dist(x as f32, y as f32, dp.x as f32, dp.y as f32);
                if dist > 9.0 || dist < 2.0 {
                    continue;
                }
                let want = r.need[f as usize][dp.kind as usize].max(if dp.kind.strategic() { 0.4 } else { 0.15 });
                best = best.max(want * 1.6);
            }
        }
    }
    best
}

/// Fighting multiplier from the strategic kinds a people has.
#[inline]
pub fn arms(w: &World, f: u8) -> f32 {
    w.res.arms.get(f as usize).copied().unwrap_or(1.0)
}

// ---------------------------------------------------------------------------------------
// Words.

fn dep_words(w: &World, k: i32) -> String {
    match w.res.deps.get(k.max(0) as usize) {
        Some(dp) if k >= 0 => format!("{} of {}", dp.kind.words(), place_name(&w.terrain, dp)),
        _ => "their land".to_string(),
    }
}

/// A war that has a reason says it: a march for a deposit, or allies keeping a pact.
pub(crate) fn war_reason(w: &World, e: &Event, a: &str, b: &str, l: &mut NewsLine) {
    if e.a < 0 || e.b < 0 {
        return;
    }
    let (fa, fb) = (e.a as u8, e.b as u8);
    if let Some(ally) = crate::pacts::joined_for(w, fa, fb) {
        let ally = &w.factions[ally as usize].name;
        l.text = format!("WAR: the {a} join their allies the {ally} against the {b}");
        l.log_text = format!("War: {a} join {ally} / {b}");
        l.say = format!("The {a} keep their pact with the {ally}: war on the {b}. Alliances are expensive, my lord.");
        l.key = "civ-war-pact".to_string();
    } else if let Some(g) = w.res.goal_of(fa, fb) {
        let what = dep_words(w, g as i32);
        let kind = w.res.deps[g as usize].kind.key();
        l.text = format!("WAR: the {a} march on the {b} for {what}");
        l.log_text = format!("War for {kind}: {a} / {b}");
        l.say = format!("The {a} march on the {b} for {what}. It was always going to be about the {kind}, my lord.");
        l.key = "civ-war-res".to_string();
    }
}

/// News lines of the #165 events (`rev`).
pub(crate) fn news(w: &World, e: &Event, a: &str, b: &str, l: &mut NewsLine) {
    let say = |l: &mut NewsLine, key: &str, text: String, priority: u8, cooldown: u16| {
        l.key = key.to_string();
        l.say = text;
        l.priority = priority;
        l.cooldown = cooldown;
    };
    let log = |l: &mut NewsLine, text: String| {
        l.log = true;
        l.log_text = text;
    };
    match e.kind {
        rev::SEIZE => {
            l.tone = TONE_WARN;
            let what = dep_words(w, e.extra);
            l.text = format!("The {a} seize {what} from the {b}");
            log(l, format!("{a} take {} from {b}", Res::from(e.n as u8).key()));
            say(l, "civ-seize", format!("The {a} have taken {what} from the {b}. Possession is nine tenths of the war, my lord."), 1, 25);
        }
        rev::CEDE => {
            l.tone = TONE_WARN;
            let what = dep_words(w, e.extra);
            l.text = format!("Peace terms: the {b} cede {what} to the {a}");
            log(l, format!("{b} cede {} to {a}", Res::from(e.n as u8).key()));
        }
        rev::PACT => {
            l.tone = TONE_GOOD;
            l.text = format!("The {a} and the {b} swear a defensive pact");
            log(l, format!("Pact: {a} + {b}"));
            say(l, "civ-pact", format!("The {a} and the {b} have sworn to stand together. Their neighbours have noticed."), 1, 30);
        }
        rev::BETRAY => {
            l.tone = TONE_BAD;
            let what = w.res.want.get(e.a.max(0) as usize * MAX_FACTIONS + e.b.max(0) as usize).copied().filter(|&d| d != NO_DEP).map(|d| format!(", eyeing {}", dep_words(w, d as i32))).unwrap_or_default();
            l.text = format!("BETRAYAL: the {a} break their pact with the {b}{what}");
            log(l, format!("Betrayal: {a} turn on {b}"));
            say(l, "civ-betray", format!("The {a} have turned on their allies the {b}, my lord. The balance shifted and so did their loyalty."), 2, 20);
        }
        rev::LAPSE => {
            l.text = format!("The pact between the {a} and the {b} lapses");
            log(l, format!("Pact ends: {a} / {b}"));
        }
        rev::SPENT => {
            let k = Res::from(e.n as u8);
            l.tone = TONE_WARN;
            let place = w.res.deps.get(e.extra.max(0) as usize).map(|d| place_name(&w.terrain, d)).unwrap_or_default();
            l.text = match k {
                Res::Timber => format!("The {a} have cut {place} bare"),
                _ => format!("The {} of {place} run out: the {a} must look elsewhere", k.key()),
            };
        }
        rev::LEARN => {
            let k = Res::from(e.n as u8);
            l.tone = TONE_WARN;
            l.text = if e.extra >= 0 {
                format!("The {a} learn to use {}, but hold none: {} lies nearest", k.key(), dep_words(w, e.extra))
            } else {
                format!("The {a} learn to use {}, but hold none", k.key())
            };
            if w.factions.get(e.a.max(0) as usize).map_or(false, |f| f.is_player) {
                say(l, &format!("civ-learn-{}", k.key()), format!("Your {a} want {} now, my lord, and have none. Their neighbours have some.", k.key()), 2, 60);
            }
        }
        rev::GREEN => {
            l.tone = TONE_GOOD;
            l.text = match e.n {
                0 => format!("The {a} plough fields and their land greens"),
                1 => format!("The {a} dig irrigation canals: the plains bloom"),
                2 => format!("The {a} cut terraces into the hills"),
                3 => format!("The {a} replant their forests"),
                _ => format!("The {a} reclaim wasteland for farming"),
            };
            if w.factions.get(e.a.max(0) as usize).map_or(false, |f| f.is_player) && w.inputs.calm > 0.5 {
                say(l, "civ-green", format!("The {a} tend their land well, my lord. Calm is good for crops."), 1, 60);
            }
        }
        rev::ERODE => {
            l.tone = TONE_BAD;
            l.text = format!("The {a} strip their land: forests felled, fields turned to dust");
            log(l, format!("{a}: the land wears out"));
            say(l, "civ-erode", format!("The {a} are wearing out their land, my lord. Hungry years follow."), 1, 45);
        }
        rev::BARTER => {
            l.tone = TONE_GOOD;
            let (ka, kb) = (Res::from((e.extra & 0xff) as u8), Res::from(((e.extra >> 8) & 0xff) as u8));
            let of = if b.ends_with('s') { format!("{b}'") } else { format!("{b}'s") };
            l.text = format!("The {a} trade {} for the {of} {}", ka.key(), kb.key());
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------
// Observation features for a future council observation (#115: OBS_VERSION 2). Pure
// functions of the world; nothing reads them yet.

pub const OBS_SELF_NAMES: [&str; 5] = ["res_metal", "res_need", "res_held", "land_fertility", "pacts"];
pub const OBS_NEIGH_NAMES: [&str; 7] = ["covet_mine", "covet_theirs", "goal_mine", "goal_theirs", "trading", "allied", "affinity"];

/// Per-civilisation features: has the metal of its age (1 held, 0.5 traded), its greatest
/// need, deposits held (scaled), its land's fertility (1 = as made), allies (scaled).
pub fn obs_self(w: &World, a: u8) -> [f32; 5] {
    let ai = a as usize;
    let age = w.factions[ai].age;
    let metal = [Res::Iron, Res::Copper].into_iter().find(|k| age >= k.unlock()).map_or(1.0, |k| w.res.access[ai][k as usize] as f32 / 2.0);
    let need = w.res.need[ai].iter().copied().fold(0.0f32, f32::max);
    let held: u32 = w.res.held[ai].iter().map(|&h| h as u32).sum();
    [metal, need, (held as f32 / 8.0).min(1.5), w.land.cfert[ai] / 128.0, (crate::pacts::count(w, a) as f32 / 3.0).min(1.0)]
}

/// Per-neighbour features of b as seen by a.
pub fn obs_neigh(w: &World, a: u8, b: u8) -> [f32; 7] {
    let (pk, qk) = (a as usize * MAX_FACTIONS + b as usize, b as usize * MAX_FACTIONS + a as usize);
    [
        w.res.covet[pk],
        w.res.covet[qk],
        (w.res.goal[pk] != NO_DEP) as u8 as f32,
        (w.res.goal[qk] != NO_DEP) as u8 as f32,
        (w.res.trade[pk] != 0) as u8 as f32,
        crate::pacts::allied(w, a, b) as u8 as f32,
        crate::pacts::affinity(w, a, b).clamp(-1.0, 2.0) / 2.0,
    ]
}

// ---------------------------------------------------------------------------------------
// Drawing: chunky 5×5 icons with a dark outline, and marks on the overview map.

/// 5×5 icons, top row first: '.' clear, letters a palette (`icon_colour`).
fn sprite(k: Res) -> [&'static str; 5] {
    match k {
        Res::Soil => [".g.g.", "ggggg", ".ggg.", "..g..", "nnnnn"],
        Res::Timber => ["..G..", ".GgG.", "GgGgG", "..n..", ".nnn."],
        Res::Stone => [".....", "..S..", ".sSs.", "sssSs", "sssss"],
        Res::Fish => [".....", "b.bb.", "bbbwb", "b.bb.", "....."],
        Res::Copper => [".....", ".sos.", "soOos", "sssos", ".sss."],
        Res::Horses => ["...nw", "nnnn.", ".nnn.", ".n.n.", ".n.n."],
        Res::Iron => [".....", "iiiii", ".iii.", "..i..", ".iii."],
        Res::Coal => [".....", "..kK.", ".kkkk", "kKkkk", ".kkk."],
        Res::Oil => ["..k..", ".kkk.", "kkpkk", "kkkkk", ".kkk."],
        Res::Uranium => ["..u..", ".uUu.", "uUuUu", ".uUu.", "..u.."],
    }
}

fn icon_colour(c: u8) -> Option<[u8; 3]> {
    Some(match c {
        b'g' => [120, 220, 80],
        b'G' => [70, 170, 70],
        b'n' => [130, 86, 48],
        b'w' => [245, 245, 245],
        b's' => [150, 150, 160],
        b'S' => [210, 210, 220],
        b'b' => [130, 190, 240],
        b'o' => [225, 130, 60],
        b'O' => [90, 200, 160],
        b'i' => [170, 180, 200],
        b'k' => [36, 36, 44],
        b'K' => [130, 130, 150],
        b'p' => [150, 100, 210],
        b'u' => [150, 255, 90],
        b'U' => [60, 170, 60],
        _ => return None,
    })
}

const OUTLINE: [u8; 4] = [18, 16, 26, 255];

impl World {
    /// Deposits in view, as icons (drawn under the villages).
    pub fn draw_deposits(&mut self) {
        use moc_planet::{VH, VW};
        let blink = (self.time * 3.0) as i32 % 2 == 0;
        for k in 0..self.res.deps.len() {
            let dp = self.res.deps[k];
            let (sx, sy) = self.cam.to_screen(dp.x as f32 + 0.5, dp.y as f32 + 0.5);
            let (sx, sy) = (sx.floor() as i32 - 2, sy.floor() as i32 - 2);
            if sx < -8 || sy < -10 || sx > VW as i32 + 2 || sy > VH as i32 + 2 {
                continue;
            }
            // Not yet of use to anyone: faint. Run dry: grey.
            let known = self.res.max_age + 1 >= dp.kind.unlock();
            let grey = dp.spent;
            let spr = sprite(dp.kind);
            let alpha = if known { 255 } else { 110 };
            for (ry, row) in spr.iter().enumerate() {
                for (rx, ch) in row.bytes().enumerate() {
                    if icon_colour(ch).is_none() {
                        continue;
                    }
                    for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let (nx, ny) = (rx as i32 + ox, ry as i32 + oy);
                        let clear = nx < 0 || ny < 0 || nx > 4 || ny > 4 || icon_colour(spr[ny as usize].as_bytes()[nx as usize]).is_none();
                        if clear {
                            self.view.px(sx + nx, sy + ny, [OUTLINE[0], OUTLINE[1], OUTLINE[2], alpha]);
                        }
                    }
                }
            }
            for (ry, row) in spr.iter().enumerate() {
                for (rx, ch) in row.bytes().enumerate() {
                    let Some(c) = icon_colour(ch) else { continue };
                    let c = if grey {
                        let g = ((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3 / 2 + 50) as u8;
                        [g, g, g]
                    } else {
                        c
                    };
                    self.view.px(sx + rx as i32, sy + ry as i32, [c[0], c[1], c[2], alpha]);
                }
            }
            // Held: a pennant in the holder's colour.
            if dp.owner != NO_F {
                let c = self.factions[dp.owner as usize].color;
                self.view.px(sx + 5, sy - 2, OUTLINE);
                self.view.px(sx + 5, sy - 1, OUTLINE);
                self.view.px(sx + 6, sy - 2, [c[0], c[1], c[2], 255]);
                self.view.px(sx + 7, sy - 2, [c[0], c[1], c[2], 255]);
                self.view.px(sx + 6, sy - 1, [c[0], c[1], c[2], 255]);
            }
            // Worked: a glint now and then.
            if dp.worked > 0.0 && hash(k as u32 ^ (self.time * 4.0) as u32) % 5 == 0 {
                let h = hash(k as u32 * 7 + (self.time * 4.0) as u32);
                self.view.px(sx + (h % 5) as i32, sy - 1 - ((h >> 4) % 2) as i32, [255, 250, 200, 255]);
            }
            // Fought over: crossed swords above, flashing.
            if self.time - dp.fought_t < 3.0 && blink {
                for (x, y) in [(0, 0), (4, 0), (1, 1), (3, 1), (2, 2), (1, 3), (3, 3), (0, 4), (4, 4)] {
                    self.view.px(sx + x, sy - 7 + y, [255, 70, 50, 255]);
                }
            }
        }
    }

    /// Strategic deposits on the overview map: a dot once some people can use them, flashing
    /// while fought over.
    pub fn draw_deposits_minimap(&mut self) {
        use crate::render::{MINI_H, MINI_W};
        if self.minimap.len() != MINI_W * MINI_H * 4 {
            return;
        }
        let blink = (self.time * 2.0) as i32 % 2 == 0;
        for dp in &self.res.deps {
            if !dp.kind.strategic() || dp.spent {
                continue;
            }
            let fought = self.time - dp.fought_t < 3.0;
            if self.res.max_age + 1 < dp.kind.unlock() && !fought {
                continue;
            }
            let (mx, my) = (dp.x as usize * MINI_W / self.d.w, dp.y as usize * MINI_H / self.d.h);
            let o = (my.min(MINI_H - 1) * MINI_W + mx.min(MINI_W - 1)) * 4;
            let c = if fought { if blink { [255, 60, 40] } else { [255, 255, 255] } } else { dp.kind.colour() };
            self.minimap[o..o + 3].copy_from_slice(&c);
        }
    }
}

/// Does a village have a building active (for `develop`)?
pub(crate) fn has(w: &World, v: &Village, b: Building) -> bool {
    crate::jobs::has(v, b, w.factions[v.owner as usize].age)
}
