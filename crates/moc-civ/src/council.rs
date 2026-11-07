//! PPO war councils (#100): a civilisation's strategy, decided by a trained policy.
//!
//! Every ~4 s of sim time (`COUNCIL_DT`) each civilisation's council reads a normalised,
//! versioned observation ([`observe`]) and chooses a MultiDiscrete action ([`HEADS`]):
//!
//! | head | options |
//! |---|---|
//! | stance toward neighbour 1, 2, 3 | peace, trade, alliance, rivalry, war, submit |
//! | war goal | raid for food, take a village, take the capital, punish, defend an ally |
//! | coalition | none, join an ally's war, gang up on the strongest |
//! | mobilisation | 4 %, 12 %, 25 %, 40 %, 60 % of the people under arms |
//! | rebels | the rules, crush, concede |
//!
//! Neighbours are the three nearest civilisations it has met. Between decisions the council's
//! standing orders are carried out each second through the #69 seam
//! (`politics::Policy`, `politics::Decision`): it declares war or raids when its stance says
//! war, sues for peace when it says peace, joins coalitions; the army commander
//! ([`crate::commander`]) turns the war goal into marches. Opportunism (striking a rival at war
//! elsewhere, in a Dark Age or split by strife) is not an action of its own: the observation
//! shows it and the stats count it.
//!
//! **The rules still enforce the physics** ([`masks`]): war only on a civilisation it has met
//! and can reach on foot, from age 7 (`WAR_FROM_AGE`) or after two battles, and only with a
//! grievance (tension at least `WAR_GATE`, which the player's calm lowers and stress raises);
//! raids from any age with tension at least `RAID_GATE`; never through a truce the player's
//! peace cast imposed. A truce from an ordinary peace can be broken, at a price: reputation
//! with everyone, and a grudge. Peace takes two: the other side accepts ([`accepts`]) by its own
//! policy's stance, or, under the rules, by war weariness; exhaustion forces it.
//!
//! **The player's mind** (stress, calm, anger) is in every observation, and still moves the
//! world under the policy: tension (and so which wars are possible), weariness, unrest. The
//! policy only drives simulated civilisations, never the player's inputs, and the player's
//! casts apply on top.
//!
//! Without valid weights ([`crate::policy_rt`]) every civilisation keeps the rules.

use std::cell::RefCell;
use std::rc::Rc;

use moc_planet::rng::Rng;

use crate::ages_table::WAR_FROM_AGE;
use crate::commander::{self, CmdBrain, CmdOrders};
use crate::conflict::{self, CIVIL_WAR, RAID};
use crate::jobs::Job;
use crate::politics::{self, Decision, Policy};
use crate::policy_rt::{Kind, Mlp};
use crate::world::*;

/// Observation layout version: bump on any change to `observe` (exported weights carry it).
pub const OBS_VERSION: u32 = 1;
/// Seconds of sim time between a council's decisions.
pub const COUNCIL_DT: f32 = 4.0;
/// Neighbours a council looks at and acts on.
pub const K_NEIGH: usize = 3;
pub const SELF_N: usize = 24;
pub const MOOD_N: usize = 3;
pub const NEIGH_N: usize = 28;
pub const GLOBAL_N: usize = 5;
pub const OBS_SIZE: usize = SELF_N + MOOD_N + K_NEIGH * NEIGH_N + GLOBAL_N;
/// MultiDiscrete: three stances, war goal, coalition, mobilisation, rebels.
pub const HEADS: [usize; 7] = [6, 6, 6, 5, 3, 5, 3];
pub const N_LOGITS: usize = 34;
/// Distances are measured in marching time: tiles / (warrior speed × 20 s). Independent of the
/// planet's size, so a policy trained on a small planet reads a big one the same way.
pub const TRAVEL: f32 = crate::agents::SPEED_WARRIOR * 20.0;
/// Tension a council needs toward a civilisation before its people follow it to war or a raid.
pub const WAR_GATE: f32 = 0.35;
pub const RAID_GATE: f32 = 0.15;
/// Seconds a war must last before peace can be made.
pub const MIN_WAR: f32 = 10.0;
/// Share of the people under arms, by mobilisation level.
pub const MOBILISE: [f32; 5] = [0.04, 0.12, 0.25, 0.4, 0.6];

pub const OBS_NAMES_SELF: [&str; SELF_N] = [
    "age", "pop", "warrior_share", "warriors", "food_days", "stability", "territory_share", "villages", "morale", "strife_heat",
    "strife_level", "friendliness", "aggressiveness", "violence", "fertility", "cohesion", "dark_age", "wars", "recent_losses",
    "reputation", "is_vassal", "vassals", "hungry", "mobilisation",
];
pub const OBS_NAMES_MOOD: [&str; MOOD_N] = ["player_stress", "player_calm", "player_anger"];
pub const OBS_NAMES_NEIGH: [&str; NEIGH_N] = [
    "present", "strength_share", "pop_share", "tension_mine", "tension_theirs", "conflict_level", "war_mine", "war_theirs",
    "distance", "reachable", "near", "grudge_mine", "grudge_theirs", "trade", "age_gap", "at_war_elsewhere", "dark_age",
    "strife_level", "truce", "divine_truce", "allied", "vassal_link", "weariness", "aggressiveness", "friendliness",
    "reputation", "battles", "may_declare",
];
pub const OBS_NAMES_GLOBAL: [&str; GLOBAL_N] = ["known_civs", "ally_at_war", "gang_up_open", "strength_share_all", "home_threat"];
pub const HEAD_NAMES: [&str; 7] = ["stance_1", "stance_2", "stance_3", "war_goal", "coalition", "mobilisation", "rebels"];
pub const STANCES: [&str; 6] = ["peace", "trade", "alliance", "rivalry", "war", "submit"];
pub const GOALS: [&str; 5] = ["raid", "village", "capital", "punish", "defend_ally"];
pub const COALITIONS: [&str; 3] = ["none", "join_ally", "gang_up"];
pub const REBELS: [&str; 3] = ["rules", "crush", "concede"];

/// All observation names, in order.
pub fn obs_names() -> Vec<String> {
    let mut v: Vec<String> = OBS_NAMES_SELF.iter().map(|s| s.to_string()).collect();
    v.extend(OBS_NAMES_MOOD.iter().map(|s| s.to_string()));
    for k in 0..K_NEIGH {
        v.extend(OBS_NAMES_NEIGH.iter().map(|s| format!("n{}_{}", k + 1, s)));
    }
    v.extend(OBS_NAMES_GLOBAL.iter().map(|s| s.to_string()));
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    Peace = 0,
    Trade = 1,
    Alliance = 2,
    Rivalry = 3,
    War = 4,
    Submit = 5,
}

impl Stance {
    pub fn from(i: u8) -> Stance {
        match i {
            1 => Stance::Trade,
            2 => Stance::Alliance,
            3 => Stance::Rivalry,
            4 => Stance::War,
            5 => Stance::Submit,
            _ => Stance::Peace,
        }
    }
    pub fn friendly(self) -> bool {
        matches!(self, Stance::Peace | Stance::Trade | Stance::Alliance | Stance::Submit)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Raid = 0,
    Village = 1,
    Capital = 2,
    Punish = 3,
    DefendAlly = 4,
}

impl Goal {
    pub fn from(i: u8) -> Goal {
        match i {
            0 => Goal::Raid,
            2 => Goal::Capital,
            3 => Goal::Punish,
            4 => Goal::DefendAlly,
            _ => Goal::Village,
        }
    }
}

/// A council's standing orders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orders {
    pub stance: [Stance; K_NEIGH],
    /// The neighbours the stances are about (NO_F = none).
    pub toward: [u8; K_NEIGH],
    pub goal: Goal,
    pub coalition: u8,
    pub mobilise: u8,
    pub rebels: u8,
}

impl Default for Orders {
    fn default() -> Orders {
        Orders { stance: [Stance::Peace; K_NEIGH], toward: [NO_F; K_NEIGH], goal: Goal::Village, coalition: 0, mobilise: 1, rebels: 0 }
    }
}

impl Orders {
    pub fn stance_toward(&self, b: u8) -> Option<Stance> {
        (0..K_NEIGH).find(|&k| self.toward[k] == b).map(|k| self.stance[k])
    }
    pub fn to_action(&self) -> [u8; 7] {
        [self.stance[0] as u8, self.stance[1] as u8, self.stance[2] as u8, self.goal as u8, self.coalition, self.mobilise, self.rebels]
    }
}

/// Who decides for a civilisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brain {
    /// `politics::decide`: today's rules.
    Rules,
    /// A council network (`Council::nets[i]`), run here.
    Net(u8),
    /// Actions handed in from outside (the training gym).
    External,
}

/// What each civilisation's council did, for the evaluation.
#[derive(Clone, Copy, Debug, Default)]
pub struct CivStats {
    pub decisions: u32,
    /// Decisions by stance (toward any neighbour), summed over the three neighbours.
    pub stance_counts: [u32; 6],
    pub goal_counts: [u32; 5],
    pub mobilise_sum: f32,
    pub wars_declared: u32,
    pub raids: u32,
    pub coalition_joins: u32,
    pub gang_ups: u32,
    pub opportunistic: u32,
    pub peace_sued: u32,
    pub peace_sued_losing: u32,
    pub truces_broken: u32,
    pub alliances: u32,
    /// Seconds at war on two fronts or more, and of those, seconds with an army sent home.
    pub multi_front_secs: f32,
    pub multi_front_defend_secs: f32,
    pub war_secs: f32,
}

#[derive(Clone, Copy, Debug)]
struct Mind {
    brain: Brain,
    cmd: CmdBrain,
    /// `Faction::born` when this mind was made (a reused slot is a new civilisation).
    born: f32,
    used: bool,
    orders: Orders,
    next_t: f32,
    /// A decision was made this second (one-off acts: breaking truces, coalitions, nudges).
    fresh: bool,
    pending: Option<[u8; 7]>,
    reputation: f32,
    cmd_orders: CmdOrders,
    cmd_pending: Option<[u8; 3]>,
}

impl Mind {
    fn new(brain: Brain, cmd: CmdBrain, born: f32, t: f32) -> Mind {
        Mind { brain, cmd, born, used: true, orders: Orders::default(), next_t: t, fresh: false, pending: None, reputation: 0.0, cmd_orders: CmdOrders::default(), cmd_pending: None }
    }
}

/// Per-civilisation numbers gathered once per politics tick (morale, threats near home).
#[derive(Clone, Debug, Default)]
pub struct Agg {
    pub time: f32,
    pub fighters: [u32; MAX_FACTIONS],
    pub in_field: [u32; MAX_FACTIONS],
    pub morale: [f32; MAX_FACTIONS],
    /// Hostile fighters within 10 tiles of each village (by village index).
    pub threat: Vec<u16>,
    /// Most hostile fighters round any one of a civilisation's villages, and that village.
    pub worst_threat: [(u16, u16); MAX_FACTIONS],
    pub own_near_worst: [u16; MAX_FACTIONS],
}

pub fn is_fighter(job: Job) -> bool {
    job.fights() && job != Job::Hunt
}

/// Gather the per-tick numbers.
pub fn aggregate(w: &World, g: &mut Agg) {
    g.time = w.time;
    g.fighters = [0; MAX_FACTIONS];
    g.in_field = [0; MAX_FACTIONS];
    let mut morale = [0.0f32; MAX_FACTIONS];
    for a in w.agents.iter().filter(|a| a.alive && is_fighter(a.job)) {
        let f = a.faction as usize;
        g.fighters[f] += 1;
        morale[f] += a.morale;
        if matches!(a.state, State::March | State::Siege) {
            g.in_field[f] += 1;
        }
    }
    for f in 0..MAX_FACTIONS {
        g.morale[f] = if g.fighters[f] > 0 { morale[f] / g.fighters[f] as f32 } else { 1.0 };
    }
    g.threat.clear();
    g.threat.resize(w.villages.len(), 0);
    g.worst_threat = [(0, NO_V); MAX_FACTIONS];
    g.own_near_worst = [0; MAX_FACTIONS];
    for (vi, v) in w.villages.iter().enumerate() {
        if !v.alive || !w.at_war[v.owner as usize] {
            continue;
        }
        let (mut hostile, mut own) = (0u16, 0u16);
        w.grid.near(v.x as f32, v.y as f32, 10.0, |j| {
            let b = &w.agents[j];
            if !b.alive || !is_fighter(b.job) || w.d.dist(b.x, b.y, v.x as f32, v.y as f32) > 10.0 {
                return;
            }
            if b.faction == v.owner {
                own += 1;
            } else if w.is_hostile(b.faction, v.owner) {
                hostile += 1;
            }
        });
        g.threat[vi] = hostile;
        let wt = &mut g.worst_threat[v.owner as usize];
        if hostile > wt.0 {
            *wt = (hostile, vi as u16);
            g.own_near_worst[v.owner as usize] = own;
        }
    }
}

/// The war-and-peace policy of every civilisation: rules, a council network, or the gym.
pub struct Council {
    pub nets: Vec<Mlp>,
    pub cmd_nets: Vec<Mlp>,
    /// Brains for civilisations born later (rebels, new arrivals).
    pub new_brain: Brain,
    pub new_cmd: CmdBrain,
    /// Pick the most likely legal option (true, the default: it plays better than sampling in
    /// the evaluation) or sample the policy with a deterministic RNG (the training league).
    pub greedy: bool,
    /// Leave the player's own civilisation to the rules.
    pub spare_player: bool,
    minds: [Mind; MAX_FACTIONS],
    allied: [bool; MAX_FACTIONS * MAX_FACTIONS],
    divine_until: [f32; MAX_FACTIONS * MAX_FACTIONS],
    /// Truces broken this second (civ, victim): costs are charged in `command`.
    broken: Vec<(u8, u8)>,
    pub stats: [CivStats; MAX_FACTIONS],
    pub agg: Agg,
    rng: Rng,
    /// Wall-clock cost of decisions (native builds only; ms total and count).
    pub decide_ms: f64,
    pub decide_n: u64,
    obs: Vec<f32>,
    mask: Vec<bool>,
}

impl Council {
    pub fn new(seed: u32) -> Council {
        Council {
            nets: Vec::new(),
            cmd_nets: Vec::new(),
            new_brain: Brain::Rules,
            new_cmd: CmdBrain::Rules,
            greedy: true,
            spare_player: true,
            minds: [Mind::new(Brain::Rules, CmdBrain::Rules, -1.0, 0.0); MAX_FACTIONS],
            allied: [false; MAX_FACTIONS * MAX_FACTIONS],
            divine_until: [-1.0; MAX_FACTIONS * MAX_FACTIONS],
            broken: Vec::new(),
            stats: [CivStats::default(); MAX_FACTIONS],
            agg: Agg { time: -1.0, ..Agg::default() },
            rng: Rng::new(seed ^ 0x00c0_4ac1),
            decide_ms: 0.0,
            decide_n: 0,
            obs: vec![0.0; OBS_SIZE],
            mask: vec![true; N_LOGITS],
        }
    }

    /// Load a council network (`MOCP` bytes). Returns its index, or why it was rejected.
    pub fn load_net(&mut self, bytes: &[u8]) -> Result<u8, crate::policy_rt::LoadError> {
        let m = Mlp::load(bytes, Kind::Council, OBS_VERSION, OBS_SIZE, &HEADS)?;
        self.nets.push(m);
        Ok((self.nets.len() - 1) as u8)
    }

    pub fn load_cmd_net(&mut self, bytes: &[u8]) -> Result<u8, crate::policy_rt::LoadError> {
        let m = Mlp::load(bytes, Kind::Commander, commander::OBS_VERSION, commander::OBS_SIZE, &commander::HEADS)?;
        self.cmd_nets.push(m);
        Ok((self.cmd_nets.len() - 1) as u8)
    }

    /// Bring the minds in line with the world's civilisations (new ones, reused slots).
    pub fn sync(&mut self, w: &World) {
        for f in 0..MAX_FACTIONS {
            let fac = &w.factions[f];
            let m = &mut self.minds[f];
            if fac.alive && (!m.used || (m.born - fac.born).abs() > 1e-3) {
                let player = fac.is_player && self.spare_player;
                let brain = if player { Brain::Rules } else { self.new_brain };
                let cmd = if player { CmdBrain::Rules } else { self.new_cmd };
                *m = Mind::new(brain, cmd, fac.born, w.time + (f % 4) as f32);
                self.stats[f] = CivStats::default();
                for o in 0..MAX_FACTIONS {
                    self.allied[f * MAX_FACTIONS + o] = false;
                    self.allied[o * MAX_FACTIONS + f] = false;
                }
            } else if !fac.alive && m.used {
                m.used = false;
            }
        }
    }

    pub fn brain(&self, civ: u8) -> Brain {
        self.minds[civ as usize].brain
    }

    pub fn cmd_brain(&self, civ: u8) -> CmdBrain {
        self.minds[civ as usize].cmd
    }

    pub fn set_brain(&mut self, civ: u8, b: Brain) {
        let m = &mut self.minds[civ as usize];
        m.brain = b;
        m.pending = None;
    }

    pub fn set_cmd_brain(&mut self, civ: u8, b: CmdBrain) {
        let m = &mut self.minds[civ as usize];
        m.cmd = b;
        m.cmd_pending = None;
    }

    /// Hand in the gym's council action for `civ` (applied at its next politics tick).
    pub fn set_action(&mut self, civ: u8, a: [u8; 7]) {
        self.minds[civ as usize].pending = Some(a);
    }

    pub fn set_cmd_action(&mut self, civ: u8, a: [u8; 3]) {
        self.minds[civ as usize].cmd_pending = Some(a);
    }

    pub fn orders(&self, civ: u8) -> Orders {
        self.minds[civ as usize].orders
    }

    pub fn cmd_orders(&self, civ: u8) -> CmdOrders {
        self.minds[civ as usize].cmd_orders
    }

    pub fn reputation(&self, civ: u8) -> f32 {
        self.minds[civ as usize].reputation
    }

    pub fn allied(&self, a: u8, b: u8) -> bool {
        self.allied[a as usize * MAX_FACTIONS + b as usize]
    }

    pub fn divine(&self, w: &World, a: u8, b: u8) -> bool {
        self.divine_until[a as usize * MAX_FACTIONS + b as usize] > w.time
    }

    /// Policy-driven (a network or the gym), not the rules?
    pub fn driven(&self, civ: u8) -> bool {
        self.minds[civ as usize].brain != Brain::Rules
    }

    fn refresh_agg(&mut self, w: &World) {
        if (self.agg.time - w.time).abs() > 1e-4 {
            aggregate(w, &mut self.agg);
        }
    }

    /// The stance `b` takes toward `a`: its council's standing orders, or (under the rules) one
    /// read from its temper.
    pub fn stance_of(&self, w: &World, b: u8, a: u8) -> Stance {
        if self.driven(b) {
            if let Some(s) = self.minds[b as usize].orders.stance_toward(a) {
                return s;
            }
        }
        let r = w.rel(b, a);
        if r.war == 2 {
            Stance::War
        } else if r.tension >= 0.8 {
            Stance::Rivalry
        } else if r.tension < -0.05 {
            Stance::Trade
        } else {
            Stance::Peace
        }
    }

    /// Does `to` accept peace offered by `from`?
    pub fn accepts(&self, w: &World, from: u8, to: u8, vassal: bool) -> bool {
        let r = w.rel(from, to);
        if r.war_t < MIN_WAR && r.war == 2 {
            return false;
        }
        let (fa, fb) = (&w.factions[from as usize], &w.factions[to as usize]);
        let calm = w.inputs.calm.clamp(0.0, 1.0);
        let agitation = w.inputs.agitation();
        let anger = w.inputs.anger.clamp(0.0, 1.0);
        let fr = (fa.traits.friendliness + fb.traits.friendliness) * 0.5;
        let mood = (1.0 + 1.6 * calm - 0.6 * agitation - 0.6 * anger).max(0.1);
        let tired = r.weariness * (0.4 + fr) * mood;
        // Exhaustion forces peace on both.
        if tired > 2.0 || r.war_t > 150.0 {
            return true;
        }
        if self.driven(to) {
            let s = self.minds[to as usize].orders.stance_toward(from);
            return match s {
                Some(s) => s.friendly() || (vassal && s != Stance::War),
                None => w.rel(to, from).tension < 0.8 || vassal,
            };
        }
        tired > 0.5 || (vassal && fa.pop * 10 < fb.pop * 6) || r.war_t > 90.0
    }

    // ---------------------------------------------------------------------------------
    // Decisions.

    fn decide_civ(&mut self, w: &World, a: u8, out: &mut Vec<Decision>) {
        self.sync(w);
        self.refresh_agg(w);
        let ai = a as usize;
        match self.minds[ai].brain {
            Brain::Rules => {
                politics::decide_into(w, a, out);
                // Peace takes two: a policy-driven enemy may refuse.
                out.retain(|d| match *d {
                    Decision::SueForPeace { with, vassal } if self.driven(with) => self.accepts(w, a, with, vassal),
                    _ => true,
                });
                return;
            }
            Brain::Net(i) => {
                if w.time >= self.minds[ai].next_t {
                    self.minds[ai].next_t = w.time + COUNCIL_DT;
                    let t0 = now();
                    let mut obs = std::mem::take(&mut self.obs);
                    let mut mask = std::mem::take(&mut self.mask);
                    observe(w, self, a, &mut obs);
                    masks(w, self, a, &mut mask);
                    let mut act = [0u8; 7];
                    let u: [f32; 7] = std::array::from_fn(|_| self.rng.f32());
                    if let Some(net) = self.nets.get(i as usize) {
                        net.act(&obs, &mask, self.greedy, &u, &mut act);
                    }
                    self.obs = obs;
                    self.mask = mask;
                    self.set_orders(w, a, act);
                    self.decide_ms += now() - t0;
                    self.decide_n += 1;
                } else {
                    self.minds[ai].fresh = false;
                }
            }
            Brain::External => {
                if let Some(act) = self.minds[ai].pending.take() {
                    self.set_orders(w, a, act);
                } else {
                    self.minds[ai].fresh = false;
                }
            }
        }
        self.carry_out(w, a, out);
    }

    /// Take a new action as the standing orders (illegal parts fall back to safe options).
    fn set_orders(&mut self, w: &World, a: u8, act: [u8; 7]) {
        let mut mask = std::mem::take(&mut self.mask);
        masks(w, self, a, &mut mask);
        let neigh = neighbours(w, self, a);
        let mut at = 0;
        let mut legal = [0u8; 7];
        for (h, &n) in HEADS.iter().enumerate() {
            let c = (act[h] as usize).min(n - 1);
            legal[h] = if mask[at + c] { c as u8 } else { 0 };
            at += n;
        }
        self.mask = mask;
        let o = Orders {
            stance: [Stance::from(legal[0]), Stance::from(legal[1]), Stance::from(legal[2])],
            toward: neigh,
            goal: Goal::from(legal[3]),
            coalition: legal[4],
            mobilise: legal[5].min(4),
            rebels: legal[6],
        };
        let st = &mut self.stats[a as usize];
        st.decisions += 1;
        for k in 0..K_NEIGH {
            if neigh[k] != NO_F {
                st.stance_counts[o.stance[k] as usize] += 1;
            }
        }
        st.goal_counts[o.goal as usize] += 1;
        st.mobilise_sum += MOBILISE[o.mobilise as usize];
        let m = &mut self.minds[a as usize];
        m.orders = o;
        m.fresh = true;
    }

    /// Standing orders into this second's decisions.
    fn carry_out(&mut self, w: &World, a: u8, out: &mut Vec<Decision>) {
        let ai = a as usize;
        let o = self.minds[ai].orders;
        let fresh = self.minds[ai].fresh;
        let fa = &w.factions[ai];
        for k in 0..K_NEIGH {
            let b = o.toward[k];
            if b == NO_F || !w.factions[b as usize].alive {
                continue;
            }
            let (r, rb) = (*w.rel(a, b), *w.rel(b, a));
            let at_war = r.war == 2 || rb.war == 2;
            match o.stance[k] {
                Stance::War => {
                    if at_war || r.war == 1 {
                        continue;
                    }
                    let (war_ok, raid_ok) = (can_war(w, self, a, b), can_raid(w, self, a, b));
                    if (o.goal == Goal::Raid || !war_ok) && raid_ok && rb.war == 0 {
                        out.push(Decision::Raid(b));
                        self.stats[ai].raids += 1;
                    } else if war_ok && (r.cooldown <= 0.0 || fresh) {
                        if r.cooldown > 0.0 {
                            self.broken.push((a, b));
                        }
                        self.count_war(w, a, b, false);
                        out.push(Decision::DeclareWar(b));
                    }
                }
                Stance::Peace | Stance::Trade | Stance::Alliance | Stance::Submit => {
                    if r.war > 0 || rb.war > 0 {
                        let vassal = o.stance[k] == Stance::Submit && fa.overlord == NO_F && w.factions[b as usize].overlord != a;
                        if self.accepts(w, a, b, vassal) {
                            out.push(Decision::SueForPeace { with: b, vassal });
                            let lose = losing(w, self, a, b);
                            let st = &mut self.stats[ai];
                            st.peace_sued += 1;
                            if lose {
                                st.peace_sued_losing += 1;
                            }
                        }
                    }
                }
                Stance::Rivalry => {}
            }
        }
        // Coalitions: join an ally's war, or gang up on the strongest.
        if fresh && (o.coalition == 1 || o.goal == Goal::DefendAlly) {
            if let Some(x) = ally_enemy(w, self, a) {
                out.push(Decision::DeclareWar(x));
                self.stats[ai].coalition_joins += 1;
                self.count_war(w, a, x, true);
            }
        }
        if fresh && o.coalition == 2 {
            if let Some(s) = gang_up_target(w, self, a) {
                out.push(Decision::DeclareWar(s));
                self.stats[ai].gang_ups += 1;
                self.count_war(w, a, s, true);
            }
        }
        // A levée en masse mans the walls when danger is near.
        if o.mobilise == 4 && fa.fortify_t <= 0.0 && self.agg.worst_threat[ai].0 > 0 {
            out.push(Decision::Fortify);
        }
        // Rebels.
        match o.rebels {
            1 | 2 => {
                for (k, s) in w.strifes.iter().enumerate() {
                    if !s.alive || s.civ != a {
                        continue;
                    }
                    if o.rebels == 1 && s.level >= RAID && !s.committed {
                        out.push(Decision::CrushRebels(k as u16));
                    } else if o.rebels == 2 && s.level >= CIVIL_WAR {
                        out.push(Decision::ConcedeToRebels(k as u16));
                    }
                }
            }
            _ => {
                let mut rules = Vec::new();
                politics::decide_into(w, a, &mut rules);
                out.extend(rules.into_iter().filter(|d| matches!(d, Decision::CrushRebels(_) | Decision::ConcedeToRebels(_))));
            }
        }
    }

    fn count_war(&mut self, w: &World, a: u8, b: u8, coalition: bool) {
        let st = &mut self.stats[a as usize];
        st.wars_declared += 1;
        let fb = &w.factions[b as usize];
        let elsewhere = (0..MAX_FACTIONS as u8).any(|c| c != a && c != b && w.factions[c as usize].alive && w.rel(b, c).war == 2);
        let strife = w.strifes.iter().any(|s| s.alive && s.civ == b && s.level >= conflict::FEUD);
        if !coalition && (elsewhere || fb.dark_t > 0.0 || strife) {
            st.opportunistic += 1;
        }
    }

    /// The companion seam: what `Decision` can't say. Runs after the rules planned the marches.
    fn command_all(&mut self, w: &mut World) {
        self.sync(w);
        // The player's peace cast: a truce no council may break.
        for a in 0..MAX_FACTIONS {
            for b in 0..MAX_FACTIONS {
                let k = a * MAX_FACTIONS + b;
                if w.rel[k].cooldown > 30.5 {
                    self.divine_until[k] = self.divine_until[k].max(w.time + w.rel[k].cooldown);
                }
            }
        }
        // Broken truces cost reputation with everyone and a grudge with the victim.
        for (a, b) in std::mem::take(&mut self.broken) {
            self.minds[a as usize].reputation = (self.minds[a as usize].reputation - 0.25).max(-1.0);
            self.stats[a as usize].truces_broken += 1;
            w.rel_mut(b, a).grudge += 0.5;
            for c in 0..MAX_FACTIONS as u8 {
                if c != a && w.factions[c as usize].alive && w.rel(c, a).met {
                    w.rel_mut(c, a).tension += 0.15;
                }
            }
            self.set_allied(a, b, false);
        }
        let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
        for &a in &alive {
            let ai = a as usize;
            let m = self.minds[ai];
            let rep = &mut self.minds[ai].reputation;
            *rep *= 0.995;
            if m.brain == Brain::Rules {
                continue;
            }
            let o = m.orders;
            // Diplomacy that warms or cools relations, once per decision.
            if m.fresh {
                for k in 0..K_NEIGH {
                    let b = o.toward[k];
                    if b == NO_F || !w.factions[b as usize].alive || w.rel(a, b).war > 0 || w.rel(b, a).war > 0 {
                        continue;
                    }
                    match o.stance[k] {
                        Stance::Trade | Stance::Alliance => {
                            w.rel_mut(a, b).tension -= 0.06;
                            w.rel_mut(b, a).tension -= 0.03 * (1.0 + m.reputation).max(0.0);
                        }
                        Stance::Rivalry | Stance::War => {
                            w.rel_mut(a, b).tension += 0.06;
                            self.set_allied(a, b, false);
                        }
                        _ => {}
                    }
                    if o.stance[k] == Stance::Alliance && !self.allied(a, b) {
                        let theirs = self.stance_of(w, b, a);
                        let fb = &w.factions[b as usize];
                        let yes = if self.driven(b) { theirs == Stance::Alliance } else { w.rel(b, a).tension <= 0.0 && fb.traits.friendliness >= 0.4 && m.reputation > -0.3 };
                        if yes {
                            self.set_allied(a, b, true);
                            self.stats[ai].alliances += 1;
                        }
                    } else if o.stance[k] != Stance::Alliance && self.driven(a) && self.allied(a, b) && !matches!(o.stance[k], Stance::Trade | Stance::Peace) {
                        self.set_allied(a, b, false);
                    }
                }
            }
            // Allies at war with each other are allies no more.
            for b in 0..MAX_FACTIONS as u8 {
                if self.allied(a, b) && (w.rel(a, b).war > 0 || !w.factions[b as usize].alive) {
                    self.set_allied(a, b, false);
                }
            }
        }
        // The army commanders.
        self.refresh_agg(w);
        for &a in &alive {
            let ai = a as usize;
            let fronts = (0..MAX_FACTIONS).filter(|&b| b != ai && w.factions[b].alive && (w.rel[ai * MAX_FACTIONS + b].war == 2 || w.rel[b * MAX_FACTIONS + ai].war == 2)).count();
            if fronts > 0 {
                self.stats[ai].war_secs += 1.0;
            }
            let cb = self.minds[ai].cmd;
            if cb == CmdBrain::Rules || fronts == 0 {
                continue;
            }
            let targets = commander::targets(w, &self.agg, a, self.minds[ai].orders.goal, self);
            let prev = self.minds[ai].cmd_orders;
            let ord = match cb {
                CmdBrain::Scripted | CmdBrain::Rules => commander::scripted(w, &self.agg, a, &targets, self.minds[ai].orders.goal),
                CmdBrain::Net(i) => {
                    let t0 = now();
                    let mut obs = [0.0f32; commander::OBS_SIZE];
                    let mut mask = [true; commander::N_LOGITS];
                    commander::observe(w, &self.agg, a, &targets, self.minds[ai].orders.goal, prev, &mut obs);
                    commander::masks(&targets, &mut mask);
                    let u: [f32; 3] = std::array::from_fn(|_| self.rng.f32());
                    let mut act = [0u8; 3];
                    if let Some(net) = self.cmd_nets.get(i as usize) {
                        net.act(&obs, &mask, self.greedy, &u, &mut act);
                    }
                    self.decide_ms += now() - t0;
                    self.decide_n += 1;
                    CmdOrders::from(act, &targets)
                }
                CmdBrain::External => match self.minds[ai].cmd_pending.take() {
                    Some(act) => CmdOrders::from(act, &targets),
                    None => prev,
                },
            };
            self.minds[ai].cmd_orders = ord;
            if fronts >= 2 {
                self.stats[ai].multi_front_secs += 1.0;
                if ord.target == 3 || ord.split == 2 {
                    self.stats[ai].multi_front_defend_secs += 1.0;
                }
            }
            commander::apply(w, a, &targets, ord);
        }
    }

    fn set_allied(&mut self, a: u8, b: u8, on: bool) {
        self.allied[a as usize * MAX_FACTIONS + b as usize] = on;
        self.allied[b as usize * MAX_FACTIONS + a as usize] = on;
    }

    /// Share of the people this civilisation's council wants under arms (None: the rules).
    pub fn mobilise_share(&self, civ: u8) -> Option<f32> {
        let m = &self.minds[civ as usize];
        if m.brain == Brain::Rules || !m.used {
            None
        } else {
            Some(MOBILISE[m.orders.mobilise.min(4) as usize])
        }
    }

    pub fn mean_decide_ms(&self) -> f64 {
        if self.decide_n == 0 {
            0.0
        } else {
            self.decide_ms / self.decide_n as f64
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn now() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

#[cfg(target_arch = "wasm32")]
fn now() -> f64 {
    0.0
}

/// The `politics::Policy` the world holds: a shared handle on a [`Council`], so the gym and the
/// browser glue can reach it between steps.
pub struct Hook(pub Rc<RefCell<Council>>);

impl Policy for Hook {
    fn decide(&mut self, w: &World, civ: u8, out: &mut Vec<Decision>) {
        self.0.borrow_mut().decide_civ(w, civ, out);
    }
    fn command(&mut self, w: &mut World) {
        self.0.borrow_mut().command_all(w);
    }
    fn mobilise(&self, civ: u8) -> Option<f32> {
        self.0.try_borrow().ok().and_then(|c| c.mobilise_share(civ))
    }
}

/// Plug a council into a world. Returns the shared handle.
pub fn install(w: &mut World, c: Council) -> Rc<RefCell<Council>> {
    let rc = Rc::new(RefCell::new(c));
    w.policy = Some(Box::new(Hook(rc.clone())));
    rc
}

// ---------------------------------------------------------------------------------------
// Physics: what a council may do.

fn reachable(w: &World, a: u8, b: u8) -> bool {
    let r = w.rel(a, b);
    r.met && w.near_pairs[a as usize * MAX_FACTIONS + b as usize].2 != NO_V
}

fn bound(w: &World, a: u8, b: u8) -> bool {
    w.factions[a as usize].overlord == b || w.factions[b as usize].overlord == a
}

/// May `a` go to war with `b` now (age, reach, grievance, no divine truce, no vassal bond)?
pub fn can_war(w: &World, c: &Council, a: u8, b: u8) -> bool {
    let r = w.rel(a, b);
    a != b
        && w.factions[b as usize].alive
        && reachable(w, a, b)
        && !bound(w, a, b)
        && !c.divine(w, a, b)
        && (w.factions[a as usize].age >= WAR_FROM_AGE || r.battles >= 2)
        && (r.tension >= WAR_GATE || w.rel(b, a).war > 0)
}

pub fn can_raid(w: &World, c: &Council, a: u8, b: u8) -> bool {
    let r = w.rel(a, b);
    a != b && w.factions[b as usize].alive && reachable(w, a, b) && !bound(w, a, b) && !c.divine(w, a, b) && r.cooldown <= 0.0 && r.tension >= RAID_GATE
}

fn strength(w: &World, c: &Council, f: u8) -> f32 {
    c.agg.fighters[f as usize] as f32 + 0.15 * w.factions[f as usize].pop as f32
}

fn losing(w: &World, c: &Council, a: u8, b: u8) -> bool {
    let (fa, fb) = (&w.factions[a as usize], &w.factions[b as usize]);
    strength(w, c, a) < 0.8 * strength(w, c, b) || fa.recent_losses > fb.recent_losses * 1.3 + 1.0
}

/// An enemy of an ally that `a` could join the war against.
fn ally_enemy(w: &World, c: &Council, a: u8) -> Option<u8> {
    for b in 0..MAX_FACTIONS as u8 {
        if !c.allied(a, b) || !w.factions[b as usize].alive {
            continue;
        }
        for x in 0..MAX_FACTIONS as u8 {
            if x == a || x == b || !w.factions[x as usize].alive || !(w.rel(b, x).war == 2 || w.rel(x, b).war == 2) {
                continue;
            }
            if w.rel(a, x).war == 2 || c.allied(a, x) {
                continue;
            }
            let r = w.rel(a, x);
            if reachable(w, a, x) && !bound(w, a, x) && !c.divine(w, a, x) && (w.factions[a as usize].age >= WAR_FROM_AGE || r.battles >= 2) {
                return Some(x);
            }
        }
    }
    None
}

/// The strongest civilisation `a` knows, if others are already fighting it and `a` could join.
fn gang_up_target(w: &World, c: &Council, a: u8) -> Option<u8> {
    let s = (0..MAX_FACTIONS as u8).filter(|&b| b != a && w.factions[b as usize].alive && w.rel(a, b).met).max_by(|&x, &y| strength(w, c, x).total_cmp(&strength(w, c, y)))?;
    if strength(w, c, s) <= strength(w, c, a) * 0.9 || w.rel(a, s).war == 2 || c.allied(a, s) {
        return None;
    }
    let fought = (0..MAX_FACTIONS as u8).any(|x| x != a && x != s && w.factions[x as usize].alive && w.rel(x, s).war == 2);
    let r = w.rel(a, s);
    (fought && reachable(w, a, s) && !bound(w, a, s) && !c.divine(w, a, s) && (w.factions[a as usize].age >= WAR_FROM_AGE || r.battles >= 2)).then_some(s)
}

/// The three nearest civilisations `a` has met (NO_F where there are fewer).
pub fn neighbours(w: &World, _c: &Council, a: u8) -> [u8; K_NEIGH] {
    let mut list: Vec<(f32, u8)> = (0..MAX_FACTIONS as u8)
        .filter(|&b| b != a && w.factions[b as usize].alive && w.rel(a, b).met)
        .map(|b| (dist_to(w, a, b), b))
        .collect();
    list.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let mut out = [NO_F; K_NEIGH];
    for (k, &(_, b)) in list.iter().take(K_NEIGH).enumerate() {
        out[k] = b;
    }
    out
}

/// Distance between two civilisations in tiles: their nearest villages on one landmass, or
/// (across the sea) their capitals plus a sea crossing.
fn dist_to(w: &World, a: u8, b: u8) -> f32 {
    let p = w.near_pairs[a as usize * MAX_FACTIONS + b as usize];
    if p.1 != NO_V {
        return p.0;
    }
    let cap = |f: u8| {
        let c = w.factions[f as usize].capital;
        w.villages.get(c as usize).map(|v| (v.x as f32, v.y as f32)).unwrap_or((w.factions[f as usize].start.0 as f32, w.factions[f as usize].start.1 as f32))
    };
    let ((ax, ay), (bx, by)) = (cap(a), cap(b));
    w.d.dist(ax, ay, bx, by) + TRAVEL
}

// ---------------------------------------------------------------------------------------
// Observation and masks.

/// The council's observation for civilisation `a` (`OBS_SIZE` floats, version `OBS_VERSION`).
pub fn observe(w: &World, c: &Council, a: u8, out: &mut Vec<f32>) {
    out.clear();
    out.resize(OBS_SIZE, 0.0);
    let ai = a as usize;
    let f = &w.factions[ai];
    if !f.alive {
        return;
    }
    let ln = |x: f32, top: f32| (x.max(0.0).ln_1p() / top.ln_1p()).min(1.5);
    let fighters = c.agg.fighters[ai] as f32;
    let pop = f.pop as f32;
    let neigh = neighbours(w, c, a);
    let known: Vec<u8> = (0..MAX_FACTIONS as u8).filter(|&b| b != a && w.factions[b as usize].alive && w.rel(a, b).met).collect();
    let tiles_known: f32 = known.iter().map(|&b| w.owned_tiles[b as usize] as f32).sum::<f32>() + w.owned_tiles[ai] as f32;
    let (strife_heat, strife_level) = w.strifes.iter().filter(|s| s.alive && s.civ == a).fold((0.0f32, 0u8), |acc, s| (acc.0.max(s.heat), acc.1.max(s.level)));
    let wars = (0..MAX_FACTIONS).filter(|&b| b != ai && w.factions[b].alive && (w.rel[ai * MAX_FACTIONS + b].war == 2 || w.rel[b * MAX_FACTIONS + ai].war == 2)).count();
    let vassals = w.alive_factions().filter(|x| x.overlord == a).count();
    let t = f.traits;
    let m = &c.minds[ai];
    let s = [
        f.age as f32 / 20.0,
        ln(pop, 400.0),
        fighters / pop.max(1.0),
        ln(fighters, 150.0),
        (f.metrics.food_days / 5.0).clamp(0.0, 2.0),
        f.metrics.stability,
        if tiles_known > 0.0 { w.owned_tiles[ai] as f32 / tiles_known } else { 1.0 },
        ln(f.villages as f32, 60.0),
        c.agg.morale[ai],
        strife_heat.min(1.5),
        strife_level as f32 / 6.0,
        t.friendliness,
        t.aggressiveness,
        t.violence,
        t.fertility,
        t.cohesion,
        if f.dark_t > 0.0 { 1.0 } else { 0.0 },
        (wars as f32 / 3.0).min(1.0),
        (f.recent_losses / (pop + 5.0)).min(1.0),
        m.reputation,
        if f.overlord != NO_F { 1.0 } else { 0.0 },
        (vassals as f32 / 3.0).min(1.0),
        f.hungry,
        m.orders.mobilise as f32 / 4.0,
    ];
    out[..SELF_N].copy_from_slice(&s);
    out[SELF_N] = w.inputs.agitation();
    out[SELF_N + 1] = w.inputs.calm.clamp(0.0, 1.0);
    out[SELF_N + 2] = w.inputs.anger.clamp(0.0, 1.0);
    for (k, &b) in neigh.iter().enumerate() {
        if b == NO_F {
            continue;
        }
        let bi = b as usize;
        let fb = &w.factions[bi];
        let (r, rb) = (w.rel(a, b), w.rel(b, a));
        let fb_fighters = c.agg.fighters[bi] as f32;
        let elsewhere = (0..MAX_FACTIONS as u8).any(|x| x != a && x != b && w.factions[x as usize].alive && w.rel(b, x).war == 2);
        let bstrife = w.strifes.iter().filter(|s| s.alive && s.civ == b).map(|s| s.level).max().unwrap_or(0);
        let reach = w.near_pairs[ai * MAX_FACTIONS + bi].2 != NO_V;
        let war = |x: u8| match x {
            2 => 1.0,
            1 => 0.5,
            _ => 0.0,
        };
        let vassal_link = if fb.overlord == a {
            1.0
        } else if f.overlord == b {
            -1.0
        } else {
            0.0
        };
        let n = [
            1.0,
            fb_fighters / (fighters + fb_fighters + 1.0),
            fb.pop as f32 / (pop + fb.pop as f32 + 1.0),
            (r.tension / 2.0).clamp(-0.5, 1.0),
            (rb.tension / 2.0).clamp(-0.5, 1.0),
            r.level as f32 / 6.0,
            war(r.war),
            war(rb.war),
            (dist_to(w, a, b) / TRAVEL).min(2.0),
            if reach { 1.0 } else { 0.0 },
            if r.near { 1.0 } else { 0.0 },
            r.grudge.min(1.5),
            rb.grudge.min(1.5),
            (-r.traded_t / 40.0).exp(),
            ((fb.age as f32 - f.age as f32) / 10.0).clamp(-1.0, 1.0),
            if elsewhere { 1.0 } else { 0.0 },
            if fb.dark_t > 0.0 { 1.0 } else { 0.0 },
            bstrife as f32 / 6.0,
            (r.cooldown.max(0.0) / 45.0).min(1.0),
            if c.divine(w, a, b) { 1.0 } else { 0.0 },
            if c.allied(a, b) { 1.0 } else { 0.0 },
            vassal_link,
            (r.weariness / 2.0).min(1.0),
            fb.traits.aggressiveness,
            fb.traits.friendliness,
            c.minds[bi].reputation,
            (r.battles as f32 / 5.0).min(1.0),
            if f.age >= WAR_FROM_AGE || r.battles >= 2 { 1.0 } else { 0.0 },
        ];
        let at = SELF_N + MOOD_N + k * NEIGH_N;
        out[at..at + NEIGH_N].copy_from_slice(&n);
    }
    let g = SELF_N + MOOD_N + K_NEIGH * NEIGH_N;
    let ally_war = (0..MAX_FACTIONS as u8).any(|b| c.allied(a, b) && (0..MAX_FACTIONS as u8).any(|x| x != b && w.rel(b, x).war == 2));
    let total_strength: f32 = known.iter().map(|&b| strength(w, c, b)).sum::<f32>() + strength(w, c, a);
    let (threat, _) = c.agg.worst_threat[ai];
    out[g] = (known.len() as f32 / 12.0).min(1.5);
    out[g + 1] = if ally_war { 1.0 } else { 0.0 };
    out[g + 2] = if gang_up_target(w, c, a).is_some() { 1.0 } else { 0.0 };
    out[g + 3] = if total_strength > 0.0 { strength(w, c, a) / total_strength } else { 1.0 };
    out[g + 4] = (threat as f32 / (fighters + 1.0)).min(2.0);
}

/// Legal options (`N_LOGITS`, head by head): the physics a council can't break.
pub fn masks(w: &World, c: &Council, a: u8, out: &mut Vec<bool>) {
    out.clear();
    out.resize(N_LOGITS, true);
    let neigh = neighbours(w, c, a);
    for k in 0..K_NEIGH {
        let at = k * 6;
        let b = neigh[k];
        if b == NO_F {
            for i in 1..6 {
                out[at + i] = false;
            }
            continue;
        }
        let (r, rb) = (w.rel(a, b), w.rel(b, a));
        let at_war = r.war > 0 || rb.war > 0;
        out[at + Stance::War as usize] = at_war || can_war(w, c, a, b) || can_raid(w, c, a, b);
        out[at + Stance::Submit as usize] = at_war;
    }
    // War goal: defending an ally needs an ally at war.
    out[18 + Goal::DefendAlly as usize] = ally_enemy(w, c, a).is_some();
    out[23 + 1] = ally_enemy(w, c, a).is_some();
    out[23 + 2] = gang_up_target(w, c, a).is_some();
    // Mobilisation: always free (indices 26..31). Rebels: only with rebels to deal with.
    let strife = w.strifes.iter().filter(|s| s.alive && s.civ == a);
    let (mut crush, mut concede) = (false, false);
    for s in strife {
        crush |= s.level >= RAID && !s.committed;
        concede |= s.level >= CIVIL_WAR;
    }
    out[31 + 1] = crush;
    out[31 + 2] = concede;
}

/// Raw council observation and mask of `a` (the gym, tests).
pub fn observe_civ(w: &World, c: &Council, a: u8) -> (Vec<f32>, Vec<bool>) {
    let mut o = Vec::new();
    let mut m = Vec::new();
    observe(w, c, a, &mut o);
    masks(w, c, a, &mut m);
    (o, m)
}

impl Council {
    /// Recompute the per-tick numbers now (the gym observes between ticks).
    pub fn refresh(&mut self, w: &World) {
        self.sync(w);
        aggregate(w, &mut self.agg);
    }
}
