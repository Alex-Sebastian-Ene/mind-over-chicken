//! Agent-against-agent conflict (#69): the escalation ladder, fights inside one
//! civilisation, and what comes after a fight. `data/ages.json` `conflict` is the design.
//!
//! **Between civilisations**, a pair's level follows the hotter side's tension
//! (`Rel::tension`, which the player's stress and anger raise and calm lowers):
//! scuffle at 0.15 (agents brawl, nobody dies), feud at 0.35 (fighters kill each other on
//! sight, violent civilisations ambush civilians), raid (a war band marches to steal food;
//! `politics::diplomacy` decides it, at 0.55), battle at 0.8 (both armies take the field
//! between them; nobody captures villages) and war (age 7+ declares it at 1.0, or any age
//! after two battles; villages change hands). Each level falls back when tension drops 0.08
//! below its threshold.
//!
//! **Inside one civilisation** ([`Strife`]), by age (`ages_table::INSIDE_ONE_CIV`): clan feuds
//! (1–6), succession disputes when the chief dies (7+), province revolts (11+), peasant revolts
//! and plague riots (12+), revolutions (16+) and misinformation riots (18+); tax revolts (9)
//! and schisms (10) come as their ages' crises. A strife's heat climbs with its grievance and
//! the player's stress and anger, falls with calm, cohesion, leaders, law, faith and
//! parliaments, and jumps with every death. Its level runs scuffle 0 → feud 0.3 → raid 0.55 →
//! battle 0.78 → civil war 1.0. A civil war ends when one side has had no fighters for 8 s,
//! when the rebels hold half the villages, or after 70 s by numbers: the rebels split off as
//! a new civilisation, take over (a revolution: the elite lose their goats, the capital moves),
//! or are crushed. A strife whose heat stays under 0.05 for 8 s ends in a truce.
//!
//! **Aftermath:** captives (from age 4, merciful victors take the beaten alive as forced
//! labour; peace ransoms them home for goats), truces (peace leaves a cooldown), vassalage (a
//! badly beaten civilisation, a weak friendly neighbour of a friendly kingdom, or a province
//! that won its revolt pays tribute) and grudges (they decay slowly and feed tension).
//!
//! **Formations** from age 11 (`FORMATIONS_FROM_AGE`): a march forms a line across the
//! approach with flankers at both ends and reserves behind; see [`formation_goal`].

use crate::ages_table::{FORMATIONS_FROM_AGE, INSIDE_ONE_CIV, WAR_FROM_AGE};
use crate::jobs::{has, info, Building, Job, Role};
use crate::politics::{declare_war, split_set};
use crate::world::*;

pub const NONE: u8 = 0;
pub const SCUFFLE: u8 = 1;
pub const FEUD: u8 = 2;
pub const RAID: u8 = 3;
pub const BATTLE: u8 = 4;
pub const WAR: u8 = 5;
pub const CIVIL_WAR: u8 = 6;
/// The ladder's names, by level (0 = peace).
pub const LEVELS: [&str; 7] = ["peace", "scuffle", "feud", "raid", "battle", "war", "civil war"];

/// Tension at which a pair of civilisations reaches scuffle, feud and battle.
pub const SCUFFLE_AT: f32 = 0.15;
pub const FEUD_AT: f32 = 0.35;
pub const BATTLE_AT: f32 = 0.8;
const HYST: f32 = 0.08;
/// Strife heat for feud, raid, battle and civil war.
pub const STRIFE_AT: [f32; 4] = [0.3, 0.55, 0.78, 1.0];
pub const MAX_STRIFES: usize = 24;
/// A civil war neither side has won after this long is settled: the rebels leave with what they hold.
pub const CIVIL_WAR_SETTLES: f32 = 45.0;

/// What a civilisation is fighting itself over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StrifeKind {
    ClanFeud = 0,
    Succession = 1,
    ProvinceRevolt = 2,
    PeasantRevolt = 3,
    PlagueRiot = 4,
    Revolution = 5,
    MisinfoRiot = 6,
    TaxRevolt = 7,
    Schism = 8,
}

impl StrifeKind {
    pub const NAMES: [&'static str; 9] =
        ["clan feud", "succession dispute", "province revolt", "peasant revolt", "plague riot", "revolution", "misinformation riot", "tax revolt", "schism"];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// The ages it can break out in: `ages_table::INSIDE_ONE_CIV` (tax revolts and schisms
    /// are their own ages' crises).
    pub fn ages(self) -> (u8, u8) {
        let r = |i: usize| (INSIDE_ONE_CIV[i].from_age, INSIDE_ONE_CIV[i].to_age);
        match self {
            StrifeKind::ClanFeud => r(0),
            StrifeKind::Succession => r(1),
            StrifeKind::ProvinceRevolt => r(2),
            StrifeKind::PeasantRevolt | StrifeKind::PlagueRiot => r(3),
            StrifeKind::Revolution => r(4),
            StrifeKind::MisinfoRiot => r(5),
            StrifeKind::TaxRevolt => (9, 20),
            StrifeKind::Schism => (10, 20),
        }
    }

    pub fn allowed(self, age: u8) -> bool {
        let (a, b) = self.ages();
        age >= a && age <= b
    }
}

/// A fight inside one civilisation. Side 0 is the crown (or the first clan), side 1 the rebels.
#[derive(Clone, Debug)]
pub struct Strife {
    pub alive: bool,
    pub civ: u8,
    pub kind: StrifeKind,
    pub level: u8,
    pub peak: u8,
    pub heat: f32,
    /// Clan-based (feuds, successions): the two clans. Otherwise sides are village tags.
    pub clans: [u8; 2],
    pub by_village: bool,
    pub x: i32,
    pub y: i32,
    pub t: f32,
    pub civil_t: f32,
    pub quiet_t: f32,
    pub deaths: [u32; 2],
    pub seen_deaths: u32,
    pub fighters: [u32; 2],
    pub people: [u32; 2],
    pub vills: [u32; 2],
    pub lost_t: [f32; 2],
    /// The crown has set out to crush it (`politics::Decision::CrushRebels`).
    pub committed: bool,
}

/// Level of a pair of civilisations from tension, before raids and wars.
fn heat_level(heat: f32, cur: u8, near: bool) -> u8 {
    let up = [(BATTLE, BATTLE_AT), (FEUD, FEUD_AT), (SCUFFLE, SCUFFLE_AT)];
    for (lv, at) in up {
        if lv == BATTLE && !near {
            continue;
        }
        let at = if cur >= lv { at - HYST } else { at };
        if heat >= at {
            return lv;
        }
    }
    NONE
}

fn strife_level(heat: f32, cur: u8) -> u8 {
    let lv = [FEUD, RAID, BATTLE, CIVIL_WAR];
    let mut out = SCUFFLE;
    for (k, &l) in lv.iter().enumerate() {
        let at = if cur >= l { STRIFE_AT[k] - HYST } else { STRIFE_AT[k] };
        if heat >= at {
            out = l;
        }
    }
    out
}

/// Recompute who fights whom: the stance matrix, `hostile`, `busy` and `fight_level`.
pub fn refresh(w: &mut World) {
    for a in 0..MAX_FACTIONS {
        w.at_war[a] = false;
        w.busy[a] = false;
        w.fight_level[a] = 0;
    }
    for a in 0..MAX_FACTIONS {
        for b in 0..MAX_FACTIONS {
            let k = a * MAX_FACTIONS + b;
            if a == b {
                w.stance[k] = 0;
                w.hostile[k] = false;
                continue;
            }
            let (r, rb) = (&w.rel[k], &w.rel[b * MAX_FACTIONS + a]);
            let lethal = r.war > 0 || rb.war > 0 || r.level >= BATTLE;
            let st = if lethal {
                3
            } else if r.level >= FEUD {
                2
            } else if r.level >= SCUFFLE {
                1
            } else {
                0
            };
            w.stance[k] = st;
            w.hostile[k] = lethal;
            if lethal {
                w.at_war[a] = true;
            }
            if st > 0 {
                w.busy[a] = true;
                w.fight_level[a] = w.fight_level[a].max(r.level.max(if r.war == 2 || rb.war == 2 { WAR } else if lethal { RAID } else { 0 }));
            }
        }
    }
    for s in w.strifes.iter().filter(|s| s.alive) {
        let c = s.civ as usize;
        w.busy[c] = true;
        w.fight_level[c] = w.fight_level[c].max(s.level);
    }
}

/// How agent `i` treats agent `j` of the same civilisation: their strife's stance (0 none).
#[inline]
pub fn inner_stance(w: &World, a: &Agent, b: &Agent) -> u8 {
    if a.camp == 0 || a.camp != b.camp || a.side == b.side {
        return 0;
    }
    match w.strifes.get(a.camp as usize - 1) {
        Some(s) if s.alive => match s.level {
            SCUFFLE => 1,
            FEUD => 2,
            _ => 3,
        },
        _ => 0,
    }
}

/// Levels between civilisations follow tension; battles come and go; grudges fade.
pub fn update_levels(w: &mut World, dt: f32) {
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    for &a in &alive {
        for &b in &alive {
            if a >= b {
                continue;
            }
            let (ra, rb) = (*w.rel(a, b), *w.rel(b, a));
            for (p, q) in [(a, b), (b, a)] {
                let r = w.rel_mut(p, q);
                r.grudge = (r.grudge - dt * 0.003).max(0.0);
                r.traded_t += dt;
            }
            if !ra.met {
                continue;
            }
            let heat = ra.tension.max(rb.tension);
            let mut lv = if ra.cooldown > 0.0 && rb.cooldown > 0.0 { heat_level(heat, ra.level, false).min(SCUFFLE) } else { heat_level(heat, ra.level, ra.near) };
            if ra.war == 2 || rb.war == 2 {
                lv = WAR;
            } else if ra.war == 1 || rb.war == 1 {
                lv = lv.max(RAID);
            }
            // A battle runs its course: after 25 s (or when one side's army is spent) the
            // field is decided, both sides draw breath, and the pair has one more battle behind it.
            if lv == BATTLE {
                let bt = ra.battle_t + dt;
                if bt > 25.0 {
                    let (fa, fb) = (&w.factions[a as usize], &w.factions[b as usize]);
                    let (la, lb) = (fa.recent_losses, fb.recent_losses);
                    let (win, lose) = if la <= lb { (a, b) } else { (b, a) };
                    let n = (la + lb).round() as i32;
                    for (p, q) in [(a, b), (b, a)] {
                        let r = w.rel_mut(p, q);
                        r.battle_t = 0.0;
                        r.battles = r.battles.saturating_add(1);
                        r.tension -= 0.18;
                    }
                    w.rel_mut(lose, win).grudge += 0.15;
                    w.stats.battles += 1;
                    let (x, y) = battle_field(w, a, b);
                    w.log(ev::BATTLE, win, lose, x, y, n);
                    lv = heat_level(heat - 0.18, BATTLE, ra.near);
                } else {
                    w.rel_mut(a, b).battle_t = bt;
                    w.rel_mut(b, a).battle_t = bt;
                }
            }
            if lv != ra.level {
                let (x, y) = battle_field(w, a, b);
                if lv > ra.level {
                    w.stats.escalations += 1;
                    if lv >= FEUD && lv != WAR {
                        w.log(ev::ESCALATE, a, b, x, y, lv as i32);
                    }
                } else if ra.level >= FEUD && ra.level != WAR {
                    w.log(ev::DEESCALATE, a, b, x, y, lv as i32);
                }
                if lv < BATTLE {
                    w.rel_mut(a, b).battle_t = 0.0;
                    w.rel_mut(b, a).battle_t = 0.0;
                }
                w.rel_mut(a, b).level = lv;
                w.rel_mut(b, a).level = lv;
            }
        }
    }
}

/// May `a` declare war on `b`? From age 7, or after several battles (`escalation.war.from`).
pub fn may_declare(w: &World, a: u8, b: u8) -> bool {
    w.factions[a as usize].age >= WAR_FROM_AGE || w.rel(a, b).battles >= 2
}

/// The ground between two civilisations' nearest villages.
pub fn battle_field(w: &World, a: u8, b: u8) -> (i32, i32) {
    let p = w.near_pairs[a as usize * MAX_FACTIONS + b as usize];
    if p.1 != NO_V && p.2 != NO_V {
        let (va, vb) = (&w.villages[p.1 as usize], &w.villages[p.2 as usize]);
        let (dx, dy) = (w.d.dxi(va.x, vb.x), w.d.dyi(va.y, vb.y));
        (w.d.wrap_x(va.x + dx / 2), w.d.wrap_y(va.y + dy / 2))
    } else {
        w.factions[a as usize].start
    }
}

/// Where a fighter stands in its army's formation (age 11+): the line across the approach,
/// flankers at its ends, reserves behind. Earlier armies just head for the target.
pub fn formation_goal(age: u8, id: u32, m: &March) -> (i32, i32) {
    if age < FORMATIONS_FROM_AGE {
        return (m.x, m.y);
    }
    let slot = slot_of(id);
    let (dx, dy) = m.dir;
    let (px, py) = (-dy, dx);
    let (cx, cy) = (m.x as f32 - dx * 3.0, m.y as f32 - dy * 3.0);
    let (x, y) = if slot < 10 {
        let o = (slot as f32 - 4.5) * 1.1;
        (cx + px * o, cy + py * o)
    } else if slot < 13 {
        let s = if slot % 2 == 0 { 1.0 } else { -1.0 };
        let o = s * (6.5 + (slot - 10) as f32);
        (cx + px * o + dx * 2.5, cy + py * o + dy * 2.5)
    } else {
        let o = (slot as f32 - 14.0) * 1.5;
        (m.x as f32 - dx * 9.0 + px * o, m.y as f32 - dy * 9.0 + py * o)
    };
    (x.round() as i32, y.round() as i32)
}

/// 0–9 the line, 10–12 flankers, 13–15 reserves.
#[inline]
pub fn slot_of(id: u32) -> u32 {
    id.wrapping_mul(2_654_435_761) >> 28
}

#[inline]
pub fn is_reserve(id: u32) -> bool {
    slot_of(id) >= 13
}

#[inline]
pub fn is_flank(id: u32) -> bool {
    (10..13).contains(&slot_of(id))
}

// ---------------------------------------------------------------------------------------
// Strife inside one civilisation.

fn mood_weight(f: &Faction) -> f32 {
    if f.is_player {
        1.0
    } else {
        0.6
    }
}

/// Begin a strife. `rebels`: villages of side 1 (village-based), else `clans` decide.
fn start(w: &mut World, civ: u8, kind: StrifeKind, level: u8, clans: [u8; 2], rebels: &[u16], at: (i32, i32)) -> Option<usize> {
    if w.strifes.iter().filter(|s| s.alive).count() >= MAX_STRIFES {
        return None;
    }
    if w.strifes.iter().filter(|s| s.alive && s.civ == civ).count() >= 2 {
        return None;
    }
    let by_village = !rebels.is_empty();
    if by_village && w.strifes.iter().any(|s| s.alive && s.civ == civ && s.by_village) {
        return None;
    }
    let heat = match level {
        SCUFFLE => 0.12,
        FEUD => STRIFE_AT[0] + 0.04,
        RAID => STRIFE_AT[1] + 0.04,
        BATTLE => STRIFE_AT[2] + 0.04,
        _ => STRIFE_AT[3],
    };
    let s = Strife {
        alive: true,
        civ,
        kind,
        level,
        peak: level,
        heat,
        clans,
        by_village,
        x: at.0,
        y: at.1,
        t: 0.0,
        civil_t: 0.0,
        quiet_t: 0.0,
        deaths: [0; 2],
        seen_deaths: 0,
        fighters: [0; 2],
        people: [0; 2],
        vills: [0; 2],
        lost_t: [0.0; 2],
        committed: false,
    };
    let k = match w.strifes.iter().position(|s| !s.alive) {
        Some(k) => {
            w.strifes[k] = s;
            k
        }
        None => {
            w.strifes.push(s);
            w.strifes.len() - 1
        }
    };
    if by_village {
        let tag = k as u8 + 1;
        for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == civ) {
            v.camp = tag;
            v.side = 0;
        }
        for &v in rebels {
            w.villages[v as usize].side = 1;
        }
    }
    w.stats.strifes += 1;
    w.factions[civ as usize].strife_cd = 25.0;
    if by_village && level >= RAID {
        arm(w, k);
    }
    w.log_x(ev::STRIFE, civ, civ, at.0, at.1, level as i32, kind as i32);
    if level == CIVIL_WAR {
        civil_war_begins(w, k);
    }
    Some(k)
}

fn civil_war_begins(w: &mut World, k: usize) {
    let (civ, kind) = (w.strifes[k].civ, w.strifes[k].kind);
    if !w.strifes[k].by_village {
        // A clan war: each village sides with its founding clan.
        let [_, c1] = w.strifes[k].clans;
        let tag = k as u8 + 1;
        if w.strifes.iter().enumerate().any(|(j, s)| j != k && s.alive && s.civ == civ && s.by_village) {
            return;
        }
        let cap = w.factions[civ as usize].capital;
        let mut any = false;
        for (i, v) in w.villages.iter_mut().enumerate() {
            if v.alive && v.owner == civ {
                v.camp = tag;
                v.side = (v.clan == c1 && i as u16 != cap) as u8;
                any |= v.side == 1;
            }
        }
        if !any {
            // The rebel clan has no village of its own: it takes the most restless one.
            if let Some((i, _)) = w.villages.iter().enumerate().filter(|(i, v)| v.alive && v.owner == civ && *i as u16 != cap).max_by(|a, b| a.1.unrest.total_cmp(&b.1.unrest)) {
                w.villages[i].side = 1;
            }
        }
        w.strifes[k].by_village = true;
    }
    let n = w.villages.iter().filter(|v| v.alive && v.owner == civ && v.camp == k as u8 + 1 && v.side == 1).count();
    arm(w, k);
    w.stats.civil_wars += 1;
    let (x, y) = (w.strifes[k].x, w.strifes[k].y);
    w.log_x(ev::CIVIL_WAR, civ, civ, x, y, n as i32, kind as i32);
}

/// Start, heat, tag and settle the fights inside civilisations. Runs every second.
pub fn strife_tick(w: &mut World, dt: f32) {
    tag(w);
    let calm = w.inputs.calm.clamp(0.0, 1.0);
    let agitation = w.inputs.agitation();
    let anger = w.inputs.anger.clamp(0.0, 1.0);
    // ---- outbreaks ----
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    let mut clan_n = [[0u16; 8]; MAX_FACTIONS];
    for a in w.agents.iter().filter(|a| a.alive && a.prisoner <= 0.0) {
        clan_n[a.faction as usize][(a.clan & 7) as usize] += 1;
    }
    for &c in &alive {
        let f = w.factions[c as usize].clone();
        w.factions[c as usize].strife_cd -= dt;
        let age = f.age;
        let m = f.metrics;
        let wgt = mood_weight(&f);
        let crowd = w.villages.iter().filter(|v| v.alive && v.owner == c).map(|v| (v.pop as f32 / v.cap.max(1) as f32 - 0.85).max(0.0)).fold(0.0, f32::max);
        let friction = f.hungry * 0.9 + crowd * 1.5 + (1.0 - m.stability) * 0.6 + f.traits.violence * 0.25 + wgt * (agitation * 0.6 + anger * 0.8) - calm * 0.7;
        // A chief's death: succession, settled peacefully or fought over.
        if f.succession {
            w.factions[c as usize].succession = false;
            if StrifeKind::Succession.allowed(age) {
                let heat = 0.15 + 0.7 * (1.0 - m.stability) + wgt * (0.3 * agitation + 0.3 * anger) - 0.4 * calm - 0.2 * f.traits.cohesion + 0.2 * f.traits.aggressiveness;
                let (x, y) = cap_xy(w, c);
                let clans = two_clans(&clan_n[c as usize]);
                if heat > 0.2 && clans.is_some() {
                    let lv = if heat > 0.7 { BATTLE } else if heat > 0.45 { RAID } else { FEUD };
                    w.log_x(ev::SUCCESSION, c, NO_F, x, y, 0, 1);
                    start(w, c, StrifeKind::Succession, lv, clans.unwrap(), &[], (x, y));
                    continue;
                }
                w.log_x(ev::SUCCESSION, c, NO_F, x, y, 0, 0);
            }
        }
        if f.strife_cd > 0.0 || f.villages == 0 {
            continue;
        }
        // Clan feuds: hunger, crowding and a hot temper turn insults into fights.
        if StrifeKind::ClanFeud.allowed(age) && friction > 0.3 && w.rng.chance(dt * 0.006 * friction) {
            if let Some(clans) = two_clans(&clan_n[c as usize]) {
                let at = cap_xy(w, c);
                let lv = if friction > 1.0 { FEUD } else { SCUFFLE };
                start(w, c, StrifeKind::ClanFeud, lv, clans, &[], at);
                continue;
            }
        }
        // Province revolts: far, restless provinces rise.
        if StrifeKind::ProvinceRevolt.allowed(age) && f.villages >= 4 {
            let cap = f.capital;
            let seed = w
                .villages
                .iter()
                .enumerate()
                .filter(|(i, v)| v.alive && v.owner == c && *i as u16 != cap && v.unrest >= 0.55 && (cap == NO_V || w.village_dist(*i as u16, cap) >= 30.0))
                .max_by(|a, b| a.1.unrest.total_cmp(&b.1.unrest))
                .map(|(i, _)| i as u16);
            if let Some(s) = seed {
                if w.rng.chance(dt * 0.15) {
                    let set = restless_round(w, c, s, 35.0, 0.35);
                    let at = (w.villages[s as usize].x, w.villages[s as usize].y);
                    start(w, c, StrifeKind::ProvinceRevolt, RAID, [0, 0], &set, at);
                    continue;
                }
            }
        }
        // Peasants against lords: inequality and hunger.
        if StrifeKind::PeasantRevolt.allowed(age) && m.inequality >= 0.38 && (f.hungry > 0.12 || m.stability < 0.5) && w.rng.chance(dt * 0.02) {
            let set = most_restless(w, c, 0.5);
            if !set.is_empty() {
                let at = (w.villages[set[0] as usize].x, w.villages[set[0] as usize].y);
                start(w, c, StrifeKind::PeasantRevolt, FEUD, [0, 0], &set, at);
                continue;
            }
        }
        // Plague riots.
        if StrifeKind::PlagueRiot.allowed(age) {
            let sick = w.villages.iter().enumerate().filter(|(_, v)| v.alive && v.owner == c && v.plague >= 0.35).max_by(|a, b| a.1.plague.total_cmp(&b.1.plague)).map(|(i, _)| i as u16);
            if let Some(v) = sick {
                if w.rng.chance(dt * 0.05) && v != f.capital {
                    let at = (w.villages[v as usize].x, w.villages[v as usize].y);
                    start(w, c, StrifeKind::PlagueRiot, SCUFFLE, [0, 0], &[v], at);
                    continue;
                }
            }
        }
        // Revolution: an unequal, unstable realm.
        if StrifeKind::Revolution.allowed(age) && m.stability < 0.45 && m.inequality >= 0.4 && w.rng.chance(dt * 0.03 * (1.0 + 2.0 * wgt * anger)) {
            let set = most_restless(w, c, 0.5);
            if !set.is_empty() {
                let at = (w.villages[set[0] as usize].x, w.villages[set[0] as usize].y);
                start(w, c, StrifeKind::Revolution, BATTLE, [0, 0], &set, at);
                continue;
            }
        }
        // Misinformation riots: stress spreads panic down the wires.
        if StrifeKind::MisinfoRiot.allowed(age) && w.rng.chance(dt * (0.002 + 0.02 * wgt * agitation)) {
            let town = w.villages.iter().enumerate().filter(|(i, v)| v.alive && v.owner == c && *i as u16 != f.capital).max_by_key(|(_, v)| v.pop).map(|(i, _)| i as u16);
            if let Some(v) = town {
                let at = (w.villages[v as usize].x, w.villages[v as usize].y);
                start(w, c, StrifeKind::MisinfoRiot, SCUFFLE, [0, 0], &[v], at);
            }
        }
    }
    // ---- heat, levels, endings ----
    for k in 0..w.strifes.len() {
        if !w.strifes[k].alive {
            continue;
        }
        let civ = w.strifes[k].civ;
        if !w.factions[civ as usize].alive {
            end(w, k, None);
            continue;
        }
        let f = w.factions[civ as usize].clone();
        let m = f.metrics;
        let wgt = mood_weight(&f);
        let s = w.strifes[k].clone();
        let rebel_unrest = {
            let (mut u, mut n) = (0.0, 0.0);
            for v in w.villages.iter().filter(|v| v.alive && v.owner == civ && v.camp == k as u8 + 1 && v.side == 1) {
                u += v.unrest + v.plague;
                n += 1.0;
            }
            if n > 0.0 {
                u / n
            } else {
                0.0
            }
        };
        let grievance = match s.kind {
            StrifeKind::ClanFeud => 0.3 + 0.7 * f.hungry + 0.3 * f.traits.violence + 0.4 * (1.0 - m.stability),
            StrifeKind::Succession => 0.35 + 0.6 * (1.0 - m.stability) + 0.2 * f.traits.aggressiveness,
            StrifeKind::ProvinceRevolt | StrifeKind::TaxRevolt => 0.2 + rebel_unrest,
            StrifeKind::PeasantRevolt => m.inequality + f.hungry + 0.2 * rebel_unrest,
            StrifeKind::PlagueRiot => 0.2 + rebel_unrest,
            StrifeKind::Revolution => m.inequality + 0.8 * (1.0 - m.stability),
            StrifeKind::MisinfoRiot => 0.3 + 1.2 * agitation,
            StrifeKind::Schism => 0.3 + 0.8 * (1.0 - m.faith),
        };
        let cap = w.villages.get(f.capital as usize).filter(|v| v.alive && v.owner == civ);
        let lords = |b: Building| cap.map_or(false, |v| has(v, b, f.age));
        let mut soothe = 0.4 + 0.5 * f.traits.cohesion + 0.25 * m.faith;
        if w.agents.iter().any(|a| a.alive && a.faction == civ && a.job == Job::Chief) {
            soothe += if s.kind == StrifeKind::Succession { 0.25 } else { 0.08 };
        }
        if lords(Building::Palace) {
            soothe += 0.08;
        }
        if lords(Building::Castle) {
            soothe += 0.08 * (1.0 + calm);
        }
        if lords(Building::Parliament) {
            soothe += if s.kind == StrifeKind::Revolution { 0.3 } else { 0.12 };
        }
        let mood = wgt * (0.7 * agitation + 0.8 * anger) - 0.9 * calm;
        let deaths = s.deaths[0] + s.deaths[1];
        let fresh = deaths.saturating_sub(s.seen_deaths) as f32;
        let mut heat = s.heat + dt * 0.045 * (grievance + mood - soothe) + 0.05 * fresh.min(6.0);
        heat = heat.clamp(0.0, 1.3);
        let level = strife_level(heat, s.level);
        {
            let st = &mut w.strifes[k];
            st.heat = heat;
            st.seen_deaths = deaths;
            st.t += dt;
            st.quiet_t = if heat < 0.05 { st.quiet_t + dt } else { 0.0 };
        }
        if level != s.level {
            w.strifes[k].level = level;
            if level > s.level {
                w.stats.escalations += 1;
                w.log_x(ev::ESCALATE, civ, civ, s.x, s.y, level as i32, s.kind as i32 + 1);
                w.strifes[k].peak = w.strifes[k].peak.max(level);
                if level == CIVIL_WAR {
                    // Sides are drawn up now; the war is decided from the next tick on.
                    civil_war_begins(w, k);
                    continue;
                }
            } else {
                w.log_x(ev::DEESCALATE, civ, civ, s.x, s.y, level as i32, s.kind as i32 + 1);
                w.strifes[k].civil_t = 0.0;
            }
        }
        if w.strifes[k].quiet_t >= 8.0 {
            w.log_x(ev::TRUCE, civ, civ, s.x, s.y, 0, s.kind as i32);
            end(w, k, None);
            continue;
        }
        // A side with no fighters left for a while has lost; so has one that never had any
        // villages (village-based) or people (clan-based).
        let st = w.strifes[k].clone();
        for side in 0..2 {
            w.strifes[k].lost_t[side] = if st.fighters[side] == 0 && st.level >= RAID { st.lost_t[side] + dt } else { 0.0 };
        }
        if st.level >= CIVIL_WAR {
            w.strifes[k].civil_t += dt;
            let ct = w.strifes[k].civil_t;
            let lost = w.strifes[k].lost_t;
            let rebels_hold = st.vills[1] >= st.vills[0].max(1) && ct > 20.0;
            if st.vills[1] == 0 || (lost[1] >= 8.0 && ct > 15.0) {
                crushed(w, k);
            } else if lost[0] >= 8.0 || rebels_hold {
                rebels_win(w, k);
            }
            // A war neither side can finish is the crown's to concede (`politics::decide`).
        } else if st.by_village && st.vills[1] == 0 {
            end(w, k, None);
        } else if st.t > 240.0 && st.level <= FEUD {
            // Old quarrels burn out.
            w.log_x(ev::TRUCE, civ, civ, s.x, s.y, 0, s.kind as i32);
            end(w, k, None);
        }
    }
    tag(w);
}

/// A rising takes up arms: about a third of the rebels' villagers (and a tenth of the crown's)
/// become fighters.
fn arm(w: &mut World, k: usize) {
    let tag = k as u8 + 1;
    let civ = w.strifes[k].civ;
    let age = w.factions[civ as usize].age;
    for i in 0..w.agents.len() {
        let a = &w.agents[i];
        if !a.alive || a.faction != civ || a.job.fights() || a.prisoner > 0.0 || a.job.role() == crate::jobs::Role::Lead {
            continue;
        }
        let Some(v) = w.villages.get(a.home as usize).filter(|v| v.alive && v.camp == tag) else { continue };
        let p = if v.side == 1 { 0.35 } else { 0.1 };
        if w.rng.chance(p) {
            let mut r = std::mem::replace(&mut w.rng, moc_planet::rng::Rng::new(0));
            let j = crate::jobs::fighter(age, &mut r, false, false);
            w.rng = r;
            crate::jobs::set_job(w, i, j);
        }
    }
}

/// The rebels lost: they lay down their arms; their villages calm, a grudge remains.
fn crushed(w: &mut World, k: usize) {
    let s = w.strifes[k].clone();
    w.stats.crushed += 1;
    w.log_x(ev::CRUSHED, s.civ, s.civ, s.x, s.y, s.deaths[1] as i32, s.kind as i32);
    for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == s.civ && v.camp == k as u8 + 1) {
        v.unrest = if v.side == 1 { 0.1 } else { v.unrest * 0.6 };
    }
    end(w, k, None);
}

/// The crown commits to crushing a rising: more of its people take up arms, and the fight
/// grows hotter (`politics::Decision::CrushRebels`).
pub fn crush_rebels(w: &mut World, civ: u8, k: usize) {
    let Some(s) = w.strifes.get(k) else { return };
    if !s.alive || s.civ != civ || s.committed {
        return;
    }
    w.strifes[k].committed = true;
    w.strifes[k].heat = (w.strifes[k].heat + 0.1).min(1.3);
    let tag = k as u8 + 1;
    let age = w.factions[civ as usize].age;
    for i in 0..w.agents.len() {
        let a = &w.agents[i];
        if !a.alive || a.faction != civ || a.job.fights() || a.prisoner > 0.0 || a.camp != tag || a.side != 0 || a.job.role() == crate::jobs::Role::Lead {
            continue;
        }
        if w.rng.chance(0.15) {
            let mut r = std::mem::replace(&mut w.rng, moc_planet::rng::Rng::new(0));
            let j = crate::jobs::fighter(age, &mut r, false, false);
            w.rng = r;
            crate::jobs::set_job(w, i, j);
        }
    }
}

/// The crown gives way (`politics::Decision::ConcedeToRebels`): in a civil war the rebels keep
/// what they hold and go their own way; before one, a truce with concessions.
pub fn concede(w: &mut World, civ: u8, k: usize) {
    let Some(s) = w.strifes.get(k) else { return };
    if !s.alive || s.civ != civ {
        return;
    }
    let (x, y, kind) = (s.x, s.y, s.kind as i32);
    if s.level >= CIVIL_WAR {
        rebels_win(w, k);
    } else {
        let tag = k as u8 + 1;
        for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == civ && v.camp == tag && v.side == 1) {
            v.unrest *= 0.5;
        }
        w.log_x(ev::TRUCE, civ, civ, x, y, 0, kind);
        end(w, k, None);
    }
}

/// The rebels won: they break away as a new civilisation, or (a revolution, or when no new
/// civilisation can be made) take over this one.
fn rebels_win(w: &mut World, k: usize) {
    let s = w.strifes[k].clone();
    let tag = k as u8 + 1;
    let set: Vec<u16> = (0..w.villages.len() as u16).filter(|&i| {
        let v = &w.villages[i as usize];
        v.alive && v.owner == s.civ && v.camp == tag && v.side == 1
    }).collect();
    let cap = w.factions[s.civ as usize].capital;
    let slots = w.alive_factions().count() < MAX_FACTIONS;
    let takeover = s.kind == StrifeKind::Revolution || !slots || set.contains(&cap) || set.len() >= w.factions[s.civ as usize].villages as usize;
    if takeover || set.is_empty() {
        revolution(w, k, &set);
        end(w, k, None);
        return;
    }
    end(w, k, None);
    let c = split_set(w, s.civ, &set, false);
    // Rebels who leave a much stronger crown still pay it tribute: a province.
    if w.factions[s.civ as usize].pop > 2 * w.factions[c as usize].pop && w.factions[s.civ as usize].overlord == NO_F && w.rng.chance(0.7) {
        make_vassal(w, c, s.civ, 2);
    } else if w.rng.chance(0.35 + 0.4 * w.factions[c as usize].traits.aggressiveness) {
        declare_war(w, c, s.civ);
    }
}

/// The rebels take the realm: the elite lose their goats, the capital moves, tempers cool and
/// the people's temper changes.
fn revolution(w: &mut World, k: usize, set: &[u16]) {
    let s = w.strifes[k].clone();
    let civ = s.civ;
    w.stats.revolutions += 1;
    let mut pot = 0.0;
    for a in w.agents.iter_mut().filter(|a| a.alive && a.faction == civ) {
        if info(a.job).elite {
            pot += a.goats * 0.7;
            a.goats *= 0.3;
            if a.job == Job::Chief {
                a.job = Job::Forage;
                a.state = State::Idle;
            }
        }
    }
    let n = w.agents.iter().filter(|a| a.alive && a.faction == civ && !info(a.job).elite).count().max(1) as f32;
    for a in w.agents.iter_mut().filter(|a| a.alive && a.faction == civ && !info(a.job).elite) {
        a.goats += pot / n;
    }
    if let Some(&v) = set.iter().max_by_key(|&&v| w.villages[v as usize].pop) {
        w.factions[civ as usize].capital = v;
    }
    {
        let t = &mut w.factions[civ as usize].traits;
        t.friendliness = (t.friendliness + 0.08).min(0.98);
        t.cohesion = (t.cohesion + 0.1).min(0.98);
        t.aggressiveness = (t.aggressiveness - 0.05).max(0.02);
    }
    for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == civ) {
        v.unrest *= 0.3;
    }
    w.factions[civ as usize].succession = false;
    w.log_x(ev::REVOLUTION, civ, civ, s.x, s.y, set.len() as i32, s.kind as i32);
}

/// Close a strife and untag its people and villages.
pub fn end(w: &mut World, k: usize, _why: Option<u8>) {
    let tag = k as u8 + 1;
    w.strifes[k].alive = false;
    for v in w.villages.iter_mut().filter(|v| v.camp == tag) {
        v.camp = 0;
        v.side = 0;
    }
    for a in w.agents.iter_mut().filter(|a| a.camp == tag) {
        a.camp = 0;
        a.side = 0;
        if matches!(a.state, State::March | State::Siege) {
            a.state = State::ToHome;
        }
    }
}

/// Which side of which strife every agent is on, and the tallies per side.
pub fn tag(w: &mut World) {
    let any = w.strifes.iter().any(|s| s.alive);
    if !any {
        if w.agents.iter().any(|a| a.camp != 0) {
            for a in w.agents.iter_mut() {
                a.camp = 0;
            }
        }
        return;
    }
    let mut per_civ: [[u8; 2]; MAX_FACTIONS] = [[0; 2]; MAX_FACTIONS];
    for (k, s) in w.strifes.iter_mut().enumerate() {
        s.fighters = [0; 2];
        s.people = [0; 2];
        s.vills = [0; 2];
        if s.alive {
            let slot = &mut per_civ[s.civ as usize];
            if slot[0] == 0 {
                slot[0] = k as u8 + 1;
            } else {
                slot[1] = k as u8 + 1;
            }
        }
    }
    // Villages of a strife's civilisation that changed hands leave it.
    for v in w.villages.iter_mut() {
        if v.camp != 0 {
            let s = &w.strifes[v.camp as usize - 1];
            if !v.alive || !s.alive || s.civ != v.owner {
                v.camp = 0;
                v.side = 0;
            }
        }
    }
    for v in w.villages.iter().filter(|v| v.alive && v.camp != 0) {
        w.strifes[v.camp as usize - 1].vills[v.side as usize] += 1;
    }
    // New villages of a civilisation in a realm-wide strife side with the crown.
    for vi in 0..w.villages.len() {
        let v = &w.villages[vi];
        if !v.alive || v.camp != 0 {
            continue;
        }
        for &t in &per_civ[v.owner as usize] {
            if t != 0 && w.strifes[t as usize - 1].by_village {
                w.villages[vi].camp = t;
                w.villages[vi].side = 0;
            }
        }
    }
    for i in 0..w.agents.len() {
        let a = &w.agents[i];
        if !a.alive {
            continue;
        }
        let mut camp = 0u8;
        let mut side = 0u8;
        if a.prisoner <= 0.0 {
            for &t in &per_civ[a.faction as usize] {
                if t == 0 {
                    continue;
                }
                let s = &w.strifes[t as usize - 1];
                if s.by_village {
                    let home = w.villages.get(a.home as usize).filter(|v| v.alive && v.owner == a.faction && v.camp == t);
                    let mut sd = home.map(|v| v.side).unwrap_or(0);
                    // Lords and the rich stand with the crown against peasants and revolutionaries.
                    if matches!(s.kind, StrifeKind::PeasantRevolt | StrifeKind::Revolution) && info(a.job).elite {
                        sd = 0;
                    }
                    if a.job == Job::Agitator && s.kind == StrifeKind::Revolution {
                        sd = 1;
                    }
                    camp = t;
                    side = sd;
                    break;
                } else if a.clan == s.clans[0] || a.clan == s.clans[1] {
                    camp = t;
                    side = (a.clan == s.clans[1]) as u8;
                    break;
                }
            }
        }
        let fights = a.job.fights();
        let a = &mut w.agents[i];
        if a.camp != camp && matches!(a.state, State::March | State::Siege) {
            a.state = State::ToHome;
        }
        a.camp = camp;
        a.side = side;
        if camp != 0 {
            let s = &mut w.strifes[camp as usize - 1];
            s.people[side as usize] += 1;
            if fights {
                s.fighters[side as usize] += 1;
            }
        }
    }
}

/// Marches for strifes at raid level and up: each side's fighters on the other side's nearest
/// village (raid, civil war) or onto the field between them (battle).
pub fn strife_marches(w: &mut World) {
    for k in 0..w.strifes.len() {
        let s = &w.strifes[k];
        if !s.alive || s.level < RAID {
            continue;
        }
        let (civ, level, tag, sx, sy) = (s.civ, s.level, k as u8 + 1, s.x, s.y);
        for side in 0..2u8 {
            // The enemy side's villages (village-based), or the other clan's.
            let mut best: Option<(f32, u16)> = None;
            let mut own: Option<(f32, u16)> = None;
            for (i, v) in w.villages.iter().enumerate() {
                if !v.alive || v.owner != civ {
                    continue;
                }
                let theirs = if s.by_village { v.camp == tag && v.side != side } else { v.clan == s.clans[1 - side as usize] };
                let d = w.d.dist(sx as f32, sy as f32, v.x as f32, v.y as f32);
                if theirs {
                    if best.map_or(true, |b| d < b.0) {
                        best = Some((d, i as u16));
                    }
                } else if own.map_or(true, |b| d < b.0) {
                    own = Some((d, i as u16));
                }
            }
            let Some((_, tv)) = best else { continue };
            let (tx, ty) = (w.villages[tv as usize].x, w.villages[tv as usize].y);
            let (ox, oy) = own.map(|(_, o)| (w.villages[o as usize].x, w.villages[o as usize].y)).unwrap_or((sx, sy));
            let field = level == BATTLE;
            let (x, y) = if field { (w.d.wrap_x(ox + w.d.dxi(ox, tx) / 2), w.d.wrap_y(oy + w.d.dyi(oy, ty) / 2)) } else { (tx, ty) };
            let (ddx, ddy) = (w.d.dxi(ox, tx) as f32, w.d.dyi(oy, ty) as f32);
            let len = (ddx * ddx + ddy * ddy).sqrt().max(1e-3);
            w.marches[civ as usize].push(March {
                enemy: civ,
                village: if field { NO_V } else { tv },
                x,
                y,
                raid: level == RAID,
                field,
                camp: tag,
                side,
                dir: (ddx / len, ddy / len),
            });
        }
    }
}

/// Another strife-side village captured inside a civil war: it changes sides.
pub fn turn_village(w: &mut World, v: u16, to_side: u8) {
    let (civ, x, y) = (w.villages[v as usize].owner, w.villages[v as usize].x, w.villages[v as usize].y);
    let vil = &mut w.villages[v as usize];
    vil.side = to_side;
    vil.siege = 0.0;
    vil.besieger = NO_F;
    vil.unrest = (vil.unrest + 0.1).min(1.5);
    w.stats.captures += 1;
    w.log_x(ev::CAPTURE, civ, civ, x, y, 0, 1);
}

fn cap_xy(w: &World, c: u8) -> (i32, i32) {
    let f = &w.factions[c as usize];
    w.villages.get(f.capital as usize).filter(|v| v.alive).map(|v| (v.x, v.y)).unwrap_or(f.start)
}

/// The two biggest clans, if both have at least three people.
fn two_clans(n: &[u16; 8]) -> Option<[u8; 2]> {
    let mut idx: Vec<usize> = (0..8).collect();
    idx.sort_by(|&a, &b| n[b].cmp(&n[a]).then(a.cmp(&b)));
    if n[idx[1]] >= 3 {
        Some([idx[0] as u8, idx[1] as u8])
    } else {
        None
    }
}

/// Restless villages round `seed` (at most half the civilisation).
fn restless_round(w: &World, c: u8, seed: u16, r: f32, min_unrest: f32) -> Vec<u16> {
    let cap = w.factions[c as usize].capital;
    let max_n = (w.factions[c as usize].villages as usize / 2).max(1);
    let mut set = vec![seed];
    let mut cands: Vec<(u16, f32)> = w
        .villages
        .iter()
        .enumerate()
        .filter(|(i, v)| v.alive && v.owner == c && *i as u16 != cap && *i as u16 != seed && v.unrest >= min_unrest && w.village_dist(*i as u16, seed) <= r)
        .map(|(i, v)| (i as u16, v.unrest))
        .collect();
    cands.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    for (v, _) in cands {
        if set.len() >= max_n {
            break;
        }
        set.push(v);
    }
    set
}

/// The most restless non-capital villages, up to `share` of the civilisation.
fn most_restless(w: &World, c: u8, share: f32) -> Vec<u16> {
    let cap = w.factions[c as usize].capital;
    let mut cands: Vec<(u16, f32)> = w.villages.iter().enumerate().filter(|(i, v)| v.alive && v.owner == c && *i as u16 != cap).map(|(i, v)| (i as u16, v.unrest)).collect();
    cands.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let n = ((w.factions[c as usize].villages as f32 * share) as usize).max(1).min(cands.len());
    cands.into_iter().take(n).map(|c| c.0).collect()
}

/// A crisis of the right age can start a strife directly (tax revolts, schisms).
pub fn crisis_strife(w: &mut World, c: u8, kind: StrifeKind) -> bool {
    if !kind.allowed(w.factions[c as usize].age) || w.factions[c as usize].villages < 2 {
        return false;
    }
    let set = most_restless(w, c, 0.4);
    if set.is_empty() {
        return false;
    }
    let at = (w.villages[set[0] as usize].x, w.villages[set[0] as usize].y);
    start(w, c, kind, if kind == StrifeKind::TaxRevolt { RAID } else { FEUD }, [0, 0], &set, at).is_some()
}

/// Begin a strife by hand (tests and tools).
pub fn begin(w: &mut World, civ: u8, kind: StrifeKind, level: u8, clans: [u8; 2], rebels: &[u16]) -> Option<usize> {
    let at = rebels.first().map(|&v| (w.villages[v as usize].x, w.villages[v as usize].y)).unwrap_or_else(|| cap_xy(w, civ));
    let k = start(w, civ, kind, level, clans, rebels, at);
    tag(w);
    refresh(w);
    k
}

// ---------------------------------------------------------------------------------------
// Aftermath.

/// Peace between a and b: captives are ransomed home for goats; a badly beaten side becomes a
/// vassal; the pair keeps a grudge.
pub fn on_peace(w: &mut World, a: u8, b: u8, battles: u8) {
    for (captor, home) in [(a, b), (b, a)] {
        let mut freed = 0;
        for i in 0..w.agents.len() {
            let ag = &w.agents[i];
            if !ag.alive || ag.faction != captor || ag.prisoner <= 0.0 || ag.origin != home {
                continue;
            }
            if w.factions[home as usize].treasury >= 2.0 {
                w.factions[home as usize].treasury -= 2.0;
                w.factions[captor as usize].treasury += 2.0;
            } else if !w.rng.chance(0.5) {
                continue;
            }
            let ag = &mut w.agents[i];
            ag.faction = home;
            ag.prisoner = 0.0;
            ag.home = NO_V;
            ag.state = State::Idle;
            freed += 1;
        }
        let _ = freed;
    }
    for (p, q) in [(a, b), (b, a)] {
        let r = w.rel_mut(p, q);
        r.grudge += 0.06 * battles as f32;
        r.battles = 0;
        r.level = NONE;
    }
    let (pa, pb) = (w.factions[a as usize].pop, w.factions[b as usize].pop);
    let (la, lb) = (w.factions[a as usize].losses, w.factions[b as usize].losses);
    if w.factions[a as usize].overlord == NO_F && w.factions[b as usize].overlord == NO_F {
        if pa * 20 < pb * 9 && la > lb / 2 {
            make_vassal(w, a, b, 0);
        } else if pb * 20 < pa * 9 && lb > la / 2 {
            make_vassal(w, b, a, 0);
        }
    }
}

pub fn make_vassal(w: &mut World, vassal: u8, lord: u8, how: i32) {
    if vassal == lord || w.factions[lord as usize].overlord == vassal || w.factions[vassal as usize].overlord == lord {
        return;
    }
    w.factions[vassal as usize].overlord = lord;
    for (p, q) in [(vassal, lord), (lord, vassal)] {
        let r = w.rel_mut(p, q);
        r.war = 0;
        r.tension = r.tension.min(0.0);
        r.cooldown = r.cooldown.max(30.0);
        r.met = true;
    }
    w.stats.vassals += 1;
    let (x, y) = cap_xy(w, vassal);
    w.log_x(ev::VASSAL, vassal, lord, x, y, 0, how);
    refresh(w);
}

/// Tribute flows up; vassals grow restless and break free.
pub fn vassals(w: &mut World, dt: f32) {
    for c in 0..MAX_FACTIONS as u8 {
        let f = &w.factions[c as usize];
        if !f.alive {
            continue;
        }
        let lord = f.overlord;
        if lord == NO_F {
            continue;
        }
        if !w.factions[lord as usize].alive {
            w.factions[c as usize].overlord = NO_F;
            continue;
        }
        // Tribute: a share of the treasury and of the granaries.
        let herds = if w.factions[c as usize].age >= crate::jobs::Job::Herd.unlock_age() { 0.01 } else { 0.0 };
        let goats = w.factions[c as usize].treasury * 0.05 * dt + herds * w.factions[c as usize].villages as f32 * dt;
        w.factions[c as usize].treasury = (w.factions[c as usize].treasury - goats).max(0.0);
        w.factions[lord as usize].treasury += goats;
        let lcap = w.factions[lord as usize].capital;
        let mut food = 0.0;
        for v in w.villages.iter_mut().filter(|v| v.alive && v.owner == c) {
            let t = v.food * 0.01 * dt;
            v.food -= t;
            food += t;
        }
        if let Some(v) = w.villages.get_mut(lcap as usize) {
            v.food += food;
        }
        {
            let r = w.rel_mut(c, lord);
            r.tension = (r.tension - 0.01 * dt).max(-1.0);
        }
        // Independence: a vassal that outgrows its lord, or comes to hate it.
        let (pc, pl) = (w.factions[c as usize].pop, w.factions[lord as usize].pop);
        if (pc as f32 > 0.9 * pl as f32 && w.rng.chance(dt * 0.05)) || w.rel(c, lord).tension > 0.9 {
            w.factions[c as usize].overlord = NO_F;
            let (x, y) = cap_xy(w, c);
            w.log(ev::INDEPENDENCE, c, lord, x, y, 0);
            if w.rel(c, lord).tension > 0.9 && may_declare(w, c, lord) {
                declare_war(w, c, lord);
            }
        }
    }
    // (Treaties of protection are a civilisation's decision: `politics::Decision::OfferAlliance`.)
}

/// A captive taken: forced labour for the captor until peace ransoms it home.
pub fn take_captive(w: &mut World, i: usize, captor: u8) {
    let (x, y, victim) = {
        let a = &w.agents[i];
        (a.x, a.y, a.faction)
    };
    let home = w.nearest_village(captor, x, y).unwrap_or(NO_V);
    if home == NO_V {
        return;
    }
    let a = &mut w.agents[i];
    a.faction = captor;
    a.prisoner = 60.0;
    a.home = home;
    a.job = Job::Build;
    a.state = State::ToHome;
    a.engage = None;
    a.camp = 0;
    a.morale = 0.3;
    a.target = NO_V;
    w.stats.captives += 1;
    w.factions[victim as usize].losses += 1;
    w.factions[victim as usize].recent_losses += 1.0;
    w.rel_mut(victim, captor).grudge += 0.02;
    w.log(ev::CAPTIVES, captor, victim, x as i32, y as i32, 1);
}

/// Leaders steady the line: generals and chiefs (and knights) are who a wounded or dead
/// leader's comrades look to.
#[inline]
pub fn is_leader(job: Job) -> bool {
    matches!(info(job).role, Role::Lead) || matches!(job, Job::General | Job::Knight)
}
