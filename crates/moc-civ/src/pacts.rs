//! Defensive pacts, coalitions and betrayal (#165).
//!
//! **Affinity.** Every pair of peoples that have met keeps an affinity, updated once a second
//! ([`tick`]). It rises with:
//! - shared rivals: a third people both are at war with, or tense with;
//! - trade in complementary resources (`resources`);
//! - friendly temperaments.
//!
//! It falls with tension between them, and war resets it.
//!
//! **Pacts.** A people with enough affinity for a neighbour offers it a defensive pact
//! (`Decision::ProposePact`), which the other accepts by its own affinity ([`accepts`]). Allies:
//! - never raid or declare war on each other;
//! - cool toward each other;
//! - share their strategic resources;
//! - feel their ally's enemies as their own.
//!
//! When someone attacks an ally, a people goes to its aid (`Decision::JoinWar`). It also joins
//! an ally's own war when it has a grievance of its own against the enemy: a coalition.
//!
//! **Pacts break** (`Decision::BreakPact`):
//! - **Betrayal**, when the balance shifts: a people grown far stronger than its ally that covets
//!   what the ally holds turns on it. That costs it a grudge, and goodwill with every other
//!   people.
//! - **Lapse**, when the friendship has soured.
//!
//! The rules decide all of this ([`decide`]). The decisions go through the `politics` seam like
//! any other, so a trained council that never emits them leaves pacts to the rules' peoples.

use crate::politics::Decision;
use crate::resources::rev;
use crate::world::*;

/// The age from which peoples swear pacts (Villages: they have territory to defend).
pub const PACT_AGE: u8 = 5;
/// Most allies a people keeps.
pub const MAX_PACTS: usize = 3;
/// Affinity needed to offer a pact, and to accept one.
pub const OFFER_AT: f32 = 0.6;
pub const ACCEPT_AT: f32 = 0.45;

#[derive(Clone, Copy, Debug, Default)]
pub struct PactStats {
    pub formed: u32,
    pub refused: u32,
    pub betrayed: u32,
    pub lapsed: u32,
    pub joined: u32,
    pub coalitions: u32,
}

pub struct Pacts {
    /// 1 while a and b are allies (symmetric, MAX_FACTIONS²).
    pub on: Vec<u8>,
    pub since: Vec<f32>,
    /// Goodwill between a and b (symmetric), -1..2.
    pub affinity: Vec<f32>,
    /// a went to war on b for an ally (ally + 1; 0 none).
    pub joined: Vec<u8>,
    /// a started its war with b.
    pub aggressor: Vec<bool>,
    /// When each people last betrayed an ally (others are wary for a while).
    pub betrayed_t: [f32; MAX_FACTIONS],
    born: [f32; MAX_FACTIONS],
    pub stats: PactStats,
}

impl Default for Pacts {
    fn default() -> Pacts {
        let n = MAX_FACTIONS * MAX_FACTIONS;
        Pacts {
            on: vec![0; n],
            since: vec![0.0; n],
            affinity: vec![0.0; n],
            joined: vec![0; n],
            aggressor: vec![false; n],
            betrayed_t: [-1e9; MAX_FACTIONS],
            born: [f32::NAN; MAX_FACTIONS],
            stats: PactStats::default(),
        }
    }
}

#[inline]
fn k(a: u8, b: u8) -> usize {
    a as usize * MAX_FACTIONS + b as usize
}

/// Are a and b allies?
#[inline]
pub fn allied(w: &World, a: u8, b: u8) -> bool {
    a != b && (a as usize) < MAX_FACTIONS && (b as usize) < MAX_FACTIONS && w.pacts.on[k(a, b)] != 0
}

pub fn affinity(w: &World, a: u8, b: u8) -> f32 {
    w.pacts.affinity[k(a, b)]
}

/// How many allies a has.
pub fn count(w: &World, a: u8) -> usize {
    (0..MAX_FACTIONS as u8).filter(|&b| allied(w, a, b)).count()
}

/// The ally a went to war on b for, if it did.
pub fn joined_for(w: &World, a: u8, b: u8) -> Option<u8> {
    let j = w.pacts.joined[k(a, b)];
    (j != 0).then(|| j - 1)
}

/// A people's might: its numbers, its fighters, its arms.
pub fn strength(w: &World, f: u8) -> f32 {
    let fac = &w.factions[f as usize];
    (fac.pop as f32 + 2.0 * fac.warriors as f32) * crate::resources::arms(w, f)
}

/// Extra tension per second of a toward b: allies cool, an ally's enemy heats.
pub fn pressure(w: &World, a: u8, b: u8) -> f32 {
    if allied(w, a, b) {
        return -0.6;
    }
    let ally_at_war = (0..MAX_FACTIONS as u8).any(|c| allied(w, a, c) && w.rel(c, b).war == 2);
    if ally_at_war {
        0.3
    } else {
        0.0
    }
}

/// a is hostile to c: at war, or tense.
fn rival(w: &World, a: u8, c: u8) -> bool {
    let r = w.rel(a, c);
    r.met && (r.war == 2 || r.tension >= 0.6)
}

/// Once a second, before diplomacy: affinities drift; dead peoples' pacts end.
pub fn tick(w: &mut World, dt: f32) {
    for f in 0..MAX_FACTIONS {
        let fac = &w.factions[f];
        if !fac.alive || w.pacts.born[f].to_bits() != fac.born.to_bits() {
            w.pacts.born[f] = if fac.alive { fac.born } else { f32::NAN };
            for o in 0..MAX_FACTIONS {
                for i in [f * MAX_FACTIONS + o, o * MAX_FACTIONS + f] {
                    w.pacts.on[i] = 0;
                    w.pacts.affinity[i] = 0.0;
                    w.pacts.joined[i] = 0;
                    w.pacts.aggressor[i] = false;
                }
            }
            if !fac.alive {
                w.pacts.betrayed_t[f] = -1e9;
            }
        }
    }
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    for (ia, &a) in alive.iter().enumerate() {
        for &b in &alive[ia + 1..] {
            let (r, rb) = (*w.rel(a, b), *w.rel(b, a));
            if !r.met {
                continue;
            }
            let i = k(a, b);
            if r.war == 2 || rb.war == 2 {
                w.pacts.affinity[i] = -1.0;
                w.pacts.affinity[k(b, a)] = -1.0;
                continue;
            }
            let shared = alive.iter().filter(|&&c| c != a && c != b && rival(w, a, c) && rival(w, b, c)).count() as f32;
            let trading = (w.res.trade[i] != 0) as u8 as f32;
            let fr = 0.5 * (w.factions[a as usize].traits.friendliness + w.factions[b as usize].traits.friendliness);
            let quarrel = (0.5 * (r.tension + rb.tension) - 0.1).max(0.0);
            let grudge = 0.5 * (r.grudge + rb.grudge).min(2.0);
            let mut v = w.pacts.affinity[i];
            // Long peace between neighbours at ease breeds familiarity.
            let easy = (r.near && r.tension < 0.0 && rb.tension < 0.0) as u8 as f32;
            v += dt * (0.035 * shared.min(3.0) + 0.02 * trading + 0.012 * easy + 0.01 * (fr - 0.45) - 0.05 * quarrel - 0.01 * grudge);
            v *= 1.0 - 0.004 * dt;
            v = v.clamp(-1.0, 2.0);
            w.pacts.affinity[i] = v;
            w.pacts.affinity[k(b, a)] = v;
        }
    }
}

/// Would b accept a pact from a?
pub fn accepts(w: &World, b: u8, a: u8) -> bool {
    let (fa, fb) = (&w.factions[a as usize], &w.factions[b as usize]);
    if !fa.alive || !fb.alive || a == b || allied(w, a, b) || fb.age < PACT_AGE {
        return false;
    }
    let (r, rb) = (w.rel(a, b), w.rel(b, a));
    if r.war != 0 || rb.war != 0 || count(w, b) >= MAX_PACTS {
        return false;
    }
    // A known traitor needs much more goodwill.
    let wary = if w.time - w.pacts.betrayed_t[a as usize] < 300.0 { 0.6 } else { 0.0 };
    affinity(w, a, b) >= ACCEPT_AT + wary && rb.tension <= 0.35
}

/// The rules for pacts (part of `politics::decide`): offer, keep, call in and break them.
pub fn decide(w: &World, a: u8, out: &mut Vec<Decision>) {
    let fa = &w.factions[a as usize];
    if !fa.alive || fa.age < PACT_AGE {
        return;
    }
    let roll = |b: u8, salt: u32| crate::politics::roll(w, a, b, salt);
    let allies = count(w, a);
    let wars = (0..MAX_FACTIONS as u8).filter(|&c| w.rel(a, c).war == 2).count();
    let mut joined = false;
    for b in 0..MAX_FACTIONS as u8 {
        let fb = &w.factions[b as usize];
        if b == a || !fb.alive {
            continue;
        }
        if allied(w, a, b) {
            // Called to arms: someone is at war with my ally.
            if !joined && wars < 2 && fa.pop >= 12 {
                for c in 0..MAX_FACTIONS as u8 {
                    if c == a || c == b || !w.factions[c as usize].alive || allied(w, a, c) || w.rel(b, c).war != 2 || w.rel(a, c).war == 2 {
                        continue;
                    }
                    let reach = w.near_pairs[k(a, c)].1 != NO_V;
                    let defend = w.pacts.aggressor[k(c, b)];
                    let grievance = w.rel(a, c).tension >= 0.3 || w.res.covet_of(a, c) >= 0.3;
                    let bound = w.factions[c as usize].overlord == a || fa.overlord == c;
                    if reach && !bound && (defend || grievance) && roll(c, 21) < if defend { 0.5 } else { 0.2 } {
                        out.push(Decision::JoinWar { ally: b, against: c });
                        joined = true;
                        break;
                    }
                }
            }
            // Betrayal: the balance has shifted, and the ally holds what it wants.
            let since = w.time - w.pacts.since[k(a, b)];
            if w.res.covet_of(a, b) >= 0.3 && since > 60.0 && fa.traits.aggressiveness >= 0.4 && strength(w, a) >= 1.8 * strength(w, b) && roll(b, 22) < 0.03 {
                out.push(Decision::BreakPact(b));
                continue;
            }
            // Lapse: the friendship has soured.
            if affinity(w, a, b) < -0.25 && a < b {
                out.push(Decision::BreakPact(b));
            }
            continue;
        }
        // Courting: goodwill enough, at peace, room for another ally.
        let (r, rb) = (w.rel(a, b), w.rel(b, a));
        if allies < MAX_PACTS
            && r.met
            && fb.age >= PACT_AGE
            && r.war == 0
            && rb.war == 0
            && r.tension <= 0.2
            && affinity(w, a, b) >= OFFER_AT
            && fb.overlord != a
            && fa.overlord != b
            && roll(b, 23) < 0.15
        {
            out.push(Decision::ProposePact(b));
        }
    }
    // A coalition war whose ally has made peace ends too.
    for c in 0..MAX_FACTIONS as u8 {
        if let Some(ally) = joined_for(w, a, c) {
            let r = w.rel(a, c);
            if r.war == 2 && r.war_t > 10.0 && (!w.factions[ally as usize].alive || w.rel(ally, c).war != 2) {
                out.push(Decision::SueForPeace { with: c, vassal: false });
            }
        }
    }
}

fn at(w: &World, a: u8, b: u8) -> (i32, i32) {
    let p = w.near_pairs[k(a, b)];
    if p.1 != NO_V {
        (w.villages[p.1 as usize].x, w.villages[p.1 as usize].y)
    } else {
        w.factions[a as usize].start
    }
}

/// a offers b a pact (`Decision::ProposePact`).
pub fn propose(w: &mut World, a: u8, b: u8) {
    if allied(w, a, b) || w.factions[a as usize].age < PACT_AGE || count(w, a) >= MAX_PACTS {
        return;
    }
    if !accepts(w, b, a) {
        w.pacts.stats.refused += 1;
        for i in [k(a, b), k(b, a)] {
            w.pacts.affinity[i] -= 0.15;
        }
        return;
    }
    for i in [k(a, b), k(b, a)] {
        w.pacts.on[i] = 1;
        w.pacts.since[i] = w.time;
    }
    for (p, q) in [(a, b), (b, a)] {
        let r = w.rel_mut(p, q);
        r.tension = (r.tension - 0.2).max(-1.0);
    }
    w.pacts.stats.formed += 1;
    let (x, y) = at(w, a, b);
    w.log(rev::PACT, a, b, x, y, 0);
}

/// Can a join ally's war against c now?
pub fn may_join(w: &World, a: u8, ally: u8, c: u8) -> bool {
    allied(w, a, ally) && !allied(w, a, c) && a != c && w.rel(ally, c).war == 2 && w.rel(a, c).war != 2 && w.near_pairs[k(a, c)].1 != NO_V
}

/// a goes to war on c for its ally (`Decision::JoinWar`): mark it, so the news says why.
pub fn mark_joined(w: &mut World, a: u8, c: u8, ally: u8) {
    w.pacts.joined[k(a, c)] = ally + 1;
    w.pacts.stats.joined += 1;
    if !w.pacts.aggressor[k(c, ally)] {
        w.pacts.stats.coalitions += 1;
    }
}

/// a breaks its pact with b (`Decision::BreakPact`): a betrayal if a covets what b holds or has
/// far outgrown it, else a lapse.
pub fn break_pact(w: &mut World, a: u8, b: u8) {
    if !allied(w, a, b) {
        return;
    }
    for i in [k(a, b), k(b, a)] {
        w.pacts.on[i] = 0;
    }
    let (x, y) = at(w, a, b);
    let betrayal = w.res.covet_of(a, b) >= 0.3 || strength(w, a) >= 2.0 * strength(w, b);
    if betrayal {
        w.pacts.betrayed_t[a as usize] = w.time;
        w.pacts.stats.betrayed += 1;
        {
            let r = w.rel_mut(b, a);
            r.grudge += 0.8;
            r.tension = (r.tension + 0.6).min(2.0);
        }
        {
            // The traitor means it: war follows.
            let r = w.rel_mut(a, b);
            r.tension = r.tension.max(WAR_TENSION);
            r.cooldown = 0.0;
        }
        // Everyone else trusts it less.
        for o in 0..MAX_FACTIONS as u8 {
            if o != a && o != b {
                for i in [k(a, o), k(o, a)] {
                    w.pacts.affinity[i] = (w.pacts.affinity[i] - 0.3).max(-1.0);
                }
            }
        }
        w.log(rev::BETRAY, a, b, x, y, 0);
    } else {
        w.pacts.stats.lapsed += 1;
        w.log(rev::LAPSE, a, b, x, y, 0);
    }
}

/// a declared war on b.
pub fn on_war(w: &mut World, a: u8, b: u8) {
    w.pacts.aggressor[k(a, b)] = true;
    w.pacts.aggressor[k(b, a)] = false;
}

/// a and b made peace.
pub fn on_peace(w: &mut World, a: u8, b: u8) {
    for i in [k(a, b), k(b, a)] {
        w.pacts.aggressor[i] = false;
        w.pacts.joined[i] = 0;
    }
}
