//! The army commander (#100): operations, per civilisation at war, every second.
//!
//! The rules march every warrior on the enemy village nearest home. A commander instead picks,
//! each second, from four objectives ([`targets`]): the nearest enemy village, the weakest-held
//! one, the enemy capital, or home (the own village under the heaviest threat); how to split
//! the field army (all on the objective, a main force plus flankers on a second objective, or a
//! main force with a reserve sent home); and a posture: assault, stage (form up on the best
//! ground short of the objective, on hills where there are any and on this side of a river,
//! and wait for the numbers; from age 11 the army forms its line there), or fall back and
//! regroup at home.
//!
//! Orders become `World::marches` entries for the wars (raids, pitched battles below war and
//! strife inside a civilisation keep the rules' marches). Warriors choose among their
//! civilisation's marches by id, so listing the main objective twice and a flank once sends a
//! third of the army to the flank; staging and assault are the same village march with a
//! different rally point, so an army can wait and then go in without walking home first.
//!
//! [`scripted`] is the hand-written commander (the default under a trained council when no
//! commander network is loaded); a trained one reads [`observe`] (`OBS_SIZE` floats).

use crate::conflict;
use crate::council::{is_fighter, Agg, Council, Goal, TRAVEL};
use crate::terrain::{defence, HILLS};
use crate::world::*;

pub const OBS_VERSION: u32 = 1;
/// target (nearest, weakest, capital, home), split (all, flank, home reserve), posture
/// (assault, stage, regroup).
pub const HEADS: [usize; 3] = [4, 3, 3];
pub const N_LOGITS: usize = 10;
pub const SELF_N: usize = 12;
pub const TARGET_N: usize = 11;
pub const OBS_SIZE: usize = SELF_N + 5 + 3 + 4 * TARGET_N + 2;
pub const TARGETS: [&str; 4] = ["nearest", "weakest", "capital", "home"];
pub const SPLITS: [&str; 3] = ["all", "flank", "home_reserve"];
pub const POSTURES: [&str; 3] = ["assault", "stage", "regroup"];

/// Fingerprint of the commander's observation layout and heads for `OBS_VERSION` 1 (see
/// `council::LAYOUT_FINGERPRINT`).
pub const LAYOUT_FINGERPRINT: u32 = 3_887_697_527;

pub fn layout_fingerprint() -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    let mut eat = |b: u8| {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    };
    for n in obs_names() {
        n.bytes().for_each(&mut eat);
        eat(b'|');
    }
    for &k in &HEADS {
        eat(k as u8);
    }
    h
}

pub fn obs_names() -> Vec<String> {
    let mut v: Vec<String> = [
        "age", "formations", "warrior_share", "warriors", "in_field", "morale", "recent_losses", "fronts", "home_threat",
        "home_threat_ratio", "villages", "strength_share",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(crate::council::GOALS.iter().map(|g| format!("goal_{g}")));
    v.extend(["prev_target", "prev_split", "prev_posture"].iter().map(|s| s.to_string()));
    for t in TARGETS {
        for n in ["present", "distance", "defenders", "ours_near", "odds", "hills", "walls", "river", "siege", "capital", "threat"] {
            v.push(format!("{t}_{n}"));
        }
    }
    v.push("player_calm".into());
    v.push("player_stress".into());
    v
}

/// Who commands a civilisation's armies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmdBrain {
    /// The rules' marches, untouched.
    Rules,
    /// The hand-written commander ([`scripted`]).
    Scripted,
    /// A commander network (`Council::cmd_nets[i]`).
    Net(u8),
    /// Actions from the gym.
    External,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CmdOrders {
    pub target: u8,
    pub split: u8,
    pub posture: u8,
}

impl CmdOrders {
    /// Orders from an action, with unavailable objectives falling back to the nearest.
    pub fn from(a: [u8; 3], t: &Targets) -> CmdOrders {
        let mut target = a[0].min(3);
        if t.t[target as usize].is_none() {
            target = if t.t[0].is_some() { 0 } else { 3 };
        }
        let mut split = a[1].min(2);
        if split == 1 && t.flank(target).is_none() {
            split = 0;
        }
        CmdOrders { target, split, posture: a[2].min(2) }
    }
    pub fn to_action(self) -> [u8; 3] {
        [self.target, self.split, self.posture]
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Target {
    pub village: u16,
    pub enemy: u8,
    pub x: i32,
    pub y: i32,
    /// Tiles from the nearest own village on the same landmass.
    pub dist: f32,
    pub from: u16,
    pub defenders: f32,
    pub ours: f32,
    pub hills: bool,
    pub walls: bool,
    pub river: bool,
    pub siege: f32,
    pub capital: bool,
    pub threat: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Targets {
    /// Nearest enemy village, weakest-held one, the main enemy's capital, home.
    pub t: [Option<Target>; 4],
    pub main_enemy: u8,
    pub fronts: u8,
    pub enemy_fighters: f32,
}

impl Targets {
    /// A second enemy objective for flankers, different from `target`'s.
    pub fn flank(&self, target: u8) -> Option<Target> {
        let main = self.t.get(target as usize).copied().flatten()?;
        (0..3).filter_map(|i| self.t[i]).find(|t| t.village != main.village)
    }
}

/// Fighters of `f` (`own`) and hostile to `f` within `r` tiles of (x, y).
fn count_near(w: &World, f: u8, x: i32, y: i32, r: f32) -> (f32, f32) {
    let (mut own, mut foe) = (0.0, 0.0);
    w.grid.near(x as f32, y as f32, r, |j| {
        let b = &w.agents[j];
        if !b.alive || !is_fighter(b.job) || w.d.dist(b.x, b.y, x as f32, y as f32) > r {
            return;
        }
        if b.faction == f {
            own += 1.0;
        } else if w.is_hostile(b.faction, f) {
            foe += 1.0;
        }
    });
    (own, foe)
}

/// Does the straight way from (ax, ay) to (bx, by) cross a river?
fn crosses_river(w: &World, ax: i32, ay: i32, bx: i32, by: i32) -> bool {
    let dx = w.d.dxi(ax, bx) as f32;
    let dy = (by - ay) as f32;
    (1..16).any(|k| {
        let t = k as f32 / 16.0;
        let (x, y) = ((ax as f32 + dx * t).round() as i32, (ay as f32 + dy * t).round() as i32);
        w.terrain.river_at(w.d.wrap_x(x), y)
    })
}

/// The four objectives for civilisation `a`.
pub fn targets(w: &World, g: &Agg, a: u8, goal: Goal, c: &Council) -> Targets {
    let mut out = Targets { main_enemy: NO_F, ..Targets::default() };
    let ai = a as usize;
    let enemies: Vec<u8> = (0..MAX_FACTIONS as u8).filter(|&b| b != a && w.factions[b as usize].alive && (w.rel(a, b).war == 2 || w.rel(b, a).war == 2)).collect();
    out.fronts = enemies.len() as u8;
    out.enemy_fighters = enemies.iter().map(|&b| g.fighters[b as usize] as f32).sum();
    let own: Vec<u16> = (0..w.villages.len() as u16).filter(|&i| w.villages[i as usize].alive && w.villages[i as usize].owner == a).collect();
    if own.is_empty() {
        return out;
    }
    // Defend an ally: its attacker is the main enemy.
    let ally_foe = if goal == Goal::DefendAlly {
        enemies.iter().copied().find(|&x| (0..MAX_FACTIONS as u8).any(|b| c.allied(a, b) && w.rel(b, x).war == 2))
    } else {
        None
    };
    let mut cands: Vec<Target> = Vec::new();
    for (vi, v) in w.villages.iter().enumerate() {
        if !v.alive || !enemies.contains(&v.owner) {
            continue;
        }
        let land = w.terrain.land(v.x, v.y);
        let mut best = (f32::MAX, NO_V);
        for &o in &own {
            let ov = &w.villages[o as usize];
            if w.terrain.land(ov.x, ov.y) != land {
                continue;
            }
            let d = w.d.dist(ov.x as f32, ov.y as f32, v.x as f32, v.y as f32);
            if d < best.0 {
                best = (d, o);
            }
        }
        if best.1 == NO_V {
            continue;
        }
        cands.push(Target {
            village: vi as u16,
            enemy: v.owner,
            x: v.x,
            y: v.y,
            dist: best.0,
            from: best.1,
            capital: w.factions[v.owner as usize].capital == vi as u16,
            hills: v.hills,
            walls: v.fx.walls,
            siege: v.siege,
            ..Target::default()
        });
    }
    if cands.is_empty() {
        out.t[3] = home(w, g, a, &own);
        return out;
    }
    let nearest = *cands.iter().filter(|t| ally_foe.map_or(true, |x| t.enemy == x)).min_by(|x, y| x.dist.total_cmp(&y.dist)).unwrap_or(&cands[0]);
    out.main_enemy = nearest.enemy;
    // Count fighters only round the shortlist (the nearest few), not every enemy village.
    cands.sort_by(|x, y| x.dist.total_cmp(&y.dist));
    cands.truncate(8);
    let reach = nearest.dist * 2.0 + 60.0;
    for t in cands.iter_mut() {
        let (own_n, foe_n) = count_near(w, a, t.x, t.y, 6.0);
        t.ours = own_n;
        t.defenders = foe_n;
        let fv = &w.villages[t.from as usize];
        t.river = crosses_river(w, fv.x, fv.y, t.x, t.y);
    }
    let fill = |t: &Target| *cands.iter().find(|c| c.village == t.village).unwrap_or(t);
    out.t[0] = Some(fill(&nearest));
    out.t[1] = cands.iter().filter(|t| t.dist <= reach).min_by(|x, y| (x.defenders + x.dist / 50.0 + if x.river { 1.5 } else { 0.0 }).total_cmp(&(y.defenders + y.dist / 50.0 + if y.river { 1.5 } else { 0.0 }))).copied();
    let cap = w.factions[out.main_enemy as usize].capital;
    out.t[2] = cands.iter().find(|t| t.village == cap).copied().or_else(|| {
        // The capital may lie beyond the shortlist: measure it on its own.
        let v = w.villages.get(cap as usize)?;
        if !v.alive || v.owner != out.main_enemy {
            return None;
        }
        let land = w.terrain.land(v.x, v.y);
        let (d, from) = own.iter().filter(|&&o| w.terrain.land(w.villages[o as usize].x, w.villages[o as usize].y) == land).map(|&o| (w.d.dist(w.villages[o as usize].x as f32, w.villages[o as usize].y as f32, v.x as f32, v.y as f32), o)).min_by(|x, y| x.0.total_cmp(&y.0))?;
        let (own_n, foe_n) = count_near(w, a, v.x, v.y, 6.0);
        let fv = &w.villages[from as usize];
        Some(Target { village: cap, enemy: v.owner, x: v.x, y: v.y, dist: d, from, defenders: foe_n, ours: own_n, hills: v.hills, walls: v.fx.walls, river: crosses_river(w, fv.x, fv.y, v.x, v.y), siege: v.siege, capital: true, threat: 0.0 })
    });
    out.t[3] = home(w, g, a, &own);
    let _ = ai;
    out
}

/// The own village under the heaviest threat (or the one facing the main enemy).
fn home(w: &World, g: &Agg, a: u8, own: &[u16]) -> Option<Target> {
    let (threat, vi) = g.worst_threat[a as usize];
    let v = if vi != NO_V && threat > 0 { vi } else { *own.iter().max_by_key(|&&o| (w.villages[o as usize].pop, std::cmp::Reverse(o)))? };
    let vil = &w.villages[v as usize];
    Some(Target {
        village: v,
        enemy: a,
        x: vil.x,
        y: vil.y,
        dist: 0.0,
        from: v,
        defenders: threat as f32,
        ours: g.own_near_worst[a as usize] as f32,
        hills: vil.hills,
        walls: vil.fx.walls,
        river: false,
        siege: vil.siege,
        capital: w.factions[a as usize].capital == v,
        threat: threat as f32,
    })
}

/// The hand-written commander: defend home when it is under real threat, else go for the war
/// goal's objective; wait on good ground while outnumbered, fall back when badly outnumbered.
pub fn scripted(w: &World, g: &Agg, a: u8, t: &Targets, goal: Goal) -> CmdOrders {
    let ai = a as usize;
    let ours = g.fighters[ai] as f32;
    if let Some(h) = t.t[3] {
        if h.threat >= 3.0 && h.threat > 0.5 * (h.ours + 1.0) {
            return CmdOrders { target: 3, split: 0, posture: 0 };
        }
    }
    let target = match goal {
        Goal::Capital if t.t[2].is_some() => 2,
        Goal::Punish if t.t[1].is_some() => 1,
        _ => 0,
    };
    let Some(main) = t.t[target as usize] else {
        return CmdOrders { target: 3, split: 0, posture: 2 };
    };
    let field = ours * 0.6;
    let posture = if field < 0.3 * main.defenders {
        2
    } else if field < 0.8 * main.defenders {
        1
    } else {
        0
    };
    let split = if t.fronts >= 2 { 2 } else { 0 };
    let _ = w;
    CmdOrders { target, split, posture }
}

/// The commander's observation (`OBS_SIZE` floats).
pub fn observe(w: &World, g: &Agg, a: u8, t: &Targets, goal: Goal, prev: CmdOrders, out: &mut [f32]) {
    out.iter_mut().for_each(|x| *x = 0.0);
    let ai = a as usize;
    let f = &w.factions[ai];
    let ln = |x: f32, top: f32| (x.max(0.0).ln_1p() / top.ln_1p()).min(1.5);
    let fighters = g.fighters[ai] as f32;
    let home_threat = t.t[3].map_or(0.0, |h| h.threat);
    let home_ours = t.t[3].map_or(0.0, |h| h.ours);
    let s = [
        f.age as f32 / 20.0,
        if f.age >= crate::ages_table::FORMATIONS_FROM_AGE { 1.0 } else { 0.0 },
        fighters / (f.pop as f32).max(1.0),
        ln(fighters, 150.0),
        g.in_field[ai] as f32 / fighters.max(1.0),
        g.morale[ai],
        (f.recent_losses / (fighters + 5.0)).min(2.0),
        (t.fronts as f32 / 3.0).min(1.0),
        ln(home_threat, 30.0),
        (home_threat / (home_ours + 1.0) / 3.0).min(1.0),
        ln(f.villages as f32, 60.0),
        fighters / (fighters + t.enemy_fighters + 1.0),
    ];
    out[..SELF_N].copy_from_slice(&s);
    out[SELF_N + goal as usize] = 1.0;
    let p = SELF_N + 5;
    out[p] = prev.target as f32 / 3.0;
    out[p + 1] = prev.split as f32 / 2.0;
    out[p + 2] = prev.posture as f32 / 2.0;
    for (k, tt) in t.t.iter().enumerate() {
        let Some(x) = tt else { continue };
        let at = p + 3 + k * TARGET_N;
        let v = [
            1.0,
            (x.dist / TRAVEL).min(2.0),
            ln(x.defenders, 40.0),
            ln(x.ours, 40.0),
            x.ours / (x.ours + x.defenders + 1.0),
            if x.hills { 1.0 } else { 0.0 },
            if x.walls { 1.0 } else { 0.0 },
            if x.river { 1.0 } else { 0.0 },
            (x.siege / SIEGE_TO_CAPTURE).min(1.0),
            if x.capital { 1.0 } else { 0.0 },
            ln(x.threat, 30.0),
        ];
        out[at..at + TARGET_N].copy_from_slice(&v);
    }
    let m = p + 3 + 4 * TARGET_N;
    out[m] = w.inputs.calm.clamp(0.0, 1.0);
    out[m + 1] = w.inputs.agitation();
}

pub fn masks(t: &Targets, out: &mut [bool]) {
    out.iter_mut().for_each(|x| *x = true);
    for k in 0..4 {
        out[k] = t.t[k].is_some();
    }
    if !out[..4].iter().any(|&b| b) {
        out[3] = true;
    }
    // A flank needs a second objective.
    out[4 + 1] = (0..3).any(|k| t.flank(k).is_some());
}

/// Rally point short of (tx, ty) coming from (fx, fy): `back` tiles out, on the best ground
/// within 4 tiles (hills over forest over open ground), off river fords.
fn rally(w: &World, fx: i32, fy: i32, tx: i32, ty: i32, back: f32) -> (i32, i32) {
    let dx = w.d.dxi(tx, fx) as f32;
    let dy = (fy - ty) as f32;
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let k = (back / len).min(0.8);
    let (cx, cy) = ((tx as f32 + dx * k).round() as i32, (ty as f32 + dy * k).round() as i32);
    let mut best = (f32::MIN, cx, cy);
    for oy in -4..=4 {
        for ox in -4..=4 {
            let (x, y) = (w.d.wrap_x(cx + ox), cy + oy);
            if !w.d.in_y(y) || !w.terrain.walkable(x, y) || w.terrain.river_at(x, y) {
                continue;
            }
            let kind = w.terrain.kind_at(x, y);
            let s = defence(kind) * 2.0 + if kind == HILLS { 0.5 } else { 0.0 } - 0.05 * ((ox * ox + oy * oy) as f32).sqrt();
            if s > best.0 {
                best = (s, x, y);
            }
        }
    }
    // Stay this side of a river: if the best ground means crossing one, rally at the centre.
    if crosses_river(w, best.1, best.2, tx, ty) && !crosses_river(w, cx, cy, tx, ty) {
        return (w.d.wrap_x(cx), cy.clamp(0, w.d.h as i32 - 1));
    }
    (best.1, best.2.clamp(0, w.d.h as i32 - 1))
}

fn heading(w: &World, fx: i32, fy: i32, x: i32, y: i32) -> (f32, f32) {
    let (dx, dy) = (w.d.dxi(fx, x) as f32, (y - fy) as f32);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-3 {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

/// Turn orders into the civilisation's war marches (raids, battles below war and strife
/// marches are left as the rules planned them).
pub fn apply(w: &mut World, a: u8, t: &Targets, o: CmdOrders) {
    let ai = a as usize;
    let at_war = |w: &World, b: u8| w.rel(a, b).war == 2 || w.rel(b, a).war == 2;
    let keep: Vec<March> = w.marches[ai].iter().copied().filter(|m| m.camp != 0 || !at_war(w, m.enemy)).collect();
    let mut ms = keep;
    let village_march = |w: &World, x: &Target, stage: bool| -> March {
        let fv = &w.villages[x.from as usize];
        let (px, py) = if stage { rally(w, fv.x, fv.y, x.x, x.y, 9.0) } else { (x.x, x.y) };
        March { enemy: x.enemy, village: x.village, x: px, y: py, raid: false, field: false, camp: 0, side: 0, dir: heading(w, fv.x, fv.y, x.x, x.y) }
    };
    let home_march = |w: &World, h: &Target| -> March {
        let dir = t.t[0].map(|e| heading(w, h.x, h.y, e.x, e.y)).unwrap_or((1.0, 0.0));
        March { enemy: t.main_enemy, village: NO_V, x: h.x, y: h.y, raid: false, field: true, camp: 0, side: 0, dir }
    };
    if o.posture == 2 || o.target == 3 {
        // Regroup at home, or defend it: everyone to the threatened village.
        if let Some(h) = t.t[3] {
            ms.push(home_march(w, &h));
        }
    } else if let Some(main) = t.t[o.target as usize] {
        let stage = o.posture == 1;
        let m = village_march(w, &main, stage);
        ms.push(m);
        ms.push(m);
        match o.split {
            1 => {
                if let Some(fl) = t.flank(o.target) {
                    ms.push(village_march(w, &fl, stage));
                } else {
                    ms.push(m);
                }
            }
            2 => {
                if let Some(h) = t.t[3] {
                    ms.push(home_march(w, &h));
                }
            }
            _ => ms.push(m),
        }
    }
    w.marches[ai] = ms;
    let _ = conflict::NONE;
}
