//! The endgame (#166): what comes after the last age.
//!
//! **Tiers.** A space-faring power keeps climbing after the Space Age (or, in the civ sandbox,
//! after age 20): [`TIERS`] — orbital industry, interplanetary colonies, a stellar empire,
//! megastructures. Each tier is earned with experience (knowledge and production while in
//! space, plus what raids steal) and, from the second on, colonies on other planets; each
//! multiplies the power's fleet, shields and economy ([`TierFx`]). Colonies feed back twice:
//! their economy adds to the ruler's, and they unlock the tiers.
//!
//! **Dominance.** Every council measures each power's share of the galaxy's strength (fleet,
//! economy, worlds held). Holding [`DOMINANCE_AT`] of it for [`DOMINANCE_HOLD`] seconds is
//! galactic dominance: the goal of the endgame.
//!
//! **Coalitions.** A leader that runs away with the galaxy ([`COALITION_AT`], well ahead of
//! the next power) is not left alone: everyone else forms a coalition against it. Members make
//! peace with each other, build faster, dig in, and send their fleets at the leader, until it
//! falls back below [`COALITION_END`]. The player can be its target, or one of its members.

use crate::war::{PowerKind, SpaceWar, TierFx, TONE_BAD, TONE_GOOD, TONE_INFO, TONE_WARN};

/// One step of the endgame ladder.
pub struct TierDef {
    pub name: &'static str,
    /// Experience needed to enter it.
    pub xp: f32,
    /// Colonies (planets held besides the capital) needed to enter it.
    pub colonies: u32,
    pub fx: TierFx,
}

pub const TIERS: [TierDef; 5] = [
    TierDef { name: "Spacefaring", xp: 0.0, colonies: 0, fx: TierFx { fleet: 1.0, shields: 1.0, econ: 1.0 } },
    TierDef { name: "Orbital industry", xp: 120.0, colonies: 0, fx: TierFx { fleet: 1.2, shields: 1.2, econ: 1.15 } },
    TierDef { name: "Interplanetary colonies", xp: 320.0, colonies: 1, fx: TierFx { fleet: 1.4, shields: 1.4, econ: 1.3 } },
    TierDef { name: "Stellar empire", xp: 640.0, colonies: 2, fx: TierFx { fleet: 1.65, shields: 1.6, econ: 1.45 } },
    TierDef { name: "Megastructures", xp: 1100.0, colonies: 3, fx: TierFx { fleet: 1.9, shields: 1.85, econ: 1.6 } },
];

/// Experience per second in space: this much per point of (0.6 knowledge + 0.4 production).
pub const XP_RATE: f32 = 0.25;
/// Chance per second, per colony the ruler holds, that a colony rises and wins its freedom.
pub const REVOLT_RATE: f32 = 0.003;
/// Share of the galaxy's strength that is dominance, and how long it must be held.
pub const DOMINANCE_AT: f32 = 0.6;
pub const DOMINANCE_HOLD: f32 = 45.0;
/// A coalition forms against a leader above this share that is this far ahead of the next…
pub const COALITION_AT: f32 = 0.42;
pub const COALITION_LEAD: f32 = 1.3;
/// …and breaks up when it falls below this, after standing at least `COALITION_MIN` seconds.
pub const COALITION_END: f32 = 0.32;
pub const COALITION_MIN: f32 = 30.0;

/// Multipliers of endgame tier `t`.
pub fn tier_fx(t: u8) -> TierFx {
    TIERS[(t as usize).min(TIERS.len() - 1)].fx
}

/// A tier's name ("" for the first, plain space flight).
pub fn tier_name(t: u8) -> &'static str {
    if t == 0 {
        ""
    } else {
        TIERS[(t as usize).min(TIERS.len() - 1)].name
    }
}

/// Progress toward the next tier, 0..1 (1 at the top).
pub fn tier_progress(p: &crate::war::Power) -> f32 {
    let t = p.tier as usize;
    if t + 1 >= TIERS.len() {
        return 1.0;
    }
    let (a, b) = (TIERS[t].xp, TIERS[t + 1].xp);
    ((p.tier_xp - a) / (b - a)).clamp(0.0, 1.0)
}

/// The tier a power with `xp` experience and `colonies` colonies has earned (never more than one
/// step above `from` at a time).
pub fn earned_tier(from: u8, xp: f32, colonies: u32) -> u8 {
    let next = from as usize + 1;
    if next < TIERS.len() && xp >= TIERS[next].xp && colonies >= TIERS[next].colonies {
        next as u8
    } else {
        from
    }
}

/// A power's weight in the galaxy: its fleet, its economy and the worlds it rules.
pub fn score(w: &SpaceWar, p: usize) -> f32 {
    let pw = &w.powers[p];
    if !pw.alive {
        return 0.0;
    }
    pw.fleet + 3.0 * pw.econ.index() + 5.0 * pw.worlds as f32
}

/// Once a second: tiers, revolts, everyone's share of the galaxy, its leader, coalitions,
/// dominance.
pub fn council(w: &mut SpaceWar, dt: f32) {
    climb(w, dt);
    revolts(w, dt);
    let scores: Vec<f32> = (0..w.powers.len()).map(|p| score(w, p)).collect();
    let total: f32 = scores.iter().sum::<f32>().max(1e-3);
    let (mut first, mut second): (Option<(usize, f32)>, f32) = (None, 0.0);
    for (p, &s) in scores.iter().enumerate() {
        w.powers[p].dominance = s / total;
        if s <= 0.0 {
            continue;
        }
        match first {
            Some((_, b)) if s <= b => second = second.max(s),
            _ => {
                if let Some((_, b)) = first {
                    second = second.max(b);
                }
                first = Some((p, s));
            }
        }
    }
    w.leader = first.map(|(p, _)| p as u16);
    coalitions(w, first, second);
    dominance(w, dt);
}

/// Experience and tiers for every power in space (civ powers get theirs from the host).
fn climb(w: &mut SpaceWar, dt: f32) {
    let focus = w.mind.focus.clamp(0.0, 1.0);
    for p in 0..w.powers.len() {
        let pw = &w.powers[p];
        if !pw.alive || !pw.space || pw.external.is_some() {
            continue;
        }
        let colonies = pw.worlds.saturating_sub(1);
        let gain = (0.6 * pw.econ.knowledge + 0.4 * pw.econ.production) * XP_RATE * (1.0 + 0.5 * focus) * dt;
        let pw = &mut w.powers[p];
        pw.tier_xp += gain;
        // Experience waiting on colonies doesn't bank past the next tier.
        if let Some(next) = TIERS.get(pw.tier as usize + 1) {
            if colonies < next.colonies {
                pw.tier_xp = pw.tier_xp.min(next.xp);
            }
        }
        let t = earned_tier(pw.tier, pw.tier_xp, colonies);
        if t != pw.tier {
            pw.tier = t;
            let d = &TIERS[t as usize];
            let player = pw.kind == PowerKind::Player;
            let who = if player { "Your world".to_string() } else { pw.name.clone() };
            let text = format!(
                "{who} enters the age of {}: fleets ×{:.2}, shields ×{:.2}, economy ×{:.2}.",
                d.name.to_lowercase(),
                d.fx.fleet,
                d.fx.shields,
                d.fx.econ
            );
            w.say(if player { TONE_GOOD } else { TONE_INFO }, player, text);
        }
    }
}

/// Colonies rise against overstretched rulers: more often the more colonies it holds, the
/// thinner their garrisons, under a coalition's pressure and the player's stress; less under
/// the player's calm.
fn revolts(w: &mut SpaceWar, dt: f32) {
    let h = w.mind.hostility();
    for i in 0..w.bodies.len() {
        let Some(o) = w.bodies[i].owner else { continue };
        let ruler = &w.powers[o as usize];
        if !ruler.alive || ruler.home == i || w.bodies[i].shared {
            continue;
        }
        let colonies = if ruler.external.is_some() { ruler.worlds.saturating_sub(1) } else { ruler.worlds.saturating_sub(1) }.max(1);
        let cap = w.caps(i).garrison.max(1.0);
        let held = (w.bodies[i].garrison / cap).min(1.0);
        let pressure = if w.coalition_against == Some(o) { 3.0 } else { 1.0 };
        let chance = REVOLT_RATE * colonies as f32 * pressure * (1.2 - 0.6 * held) * (1.0 + 0.5 * h.max(0.0)) * (1.0 - 0.5 * (-h).max(0.0)) * dt;
        if w.roll(o, i as u16, 11) >= chance {
            continue;
        }
        w.powers[o as usize].results.colonies -= 1;
        let player = w.is_player(o);
        let ruler = who(w, o);
        let world = w.bodies[i].name.clone();
        // Its own people, if they still rule somewhere, take it back; else it goes its own way.
        match w.exiles_of(i) {
            Some(e) => {
                w.bodies[i].owner = Some(e);
                w.powers[e as usize].home = i;
                let back = who(w, e);
                w.say(if player { TONE_BAD } else { TONE_WARN }, player || w.is_player(e), format!("{world} rises against {ruler} and returns to {back}."));
            }
            None => {
                w.bodies[i].owner = None;
                w.say(if player { TONE_BAD } else { TONE_WARN }, player, format!("{world} rises against {ruler} and wins back its freedom."));
            }
        }
    }
}

/// A runaway leader draws a coalition of everyone else; it breaks up when the leader falls back.
fn coalitions(w: &mut SpaceWar, first: Option<(usize, f32)>, second: f32) {
    let alive_space = w.powers.iter().filter(|p| p.alive && p.space).count();
    if let Some(target) = w.coalition_against {
        let t = target as usize;
        w.coalition_t += 1.0;
        // A coalition holds at least half a minute once formed.
        let over = !w.powers[t].alive || (w.powers[t].dominance < COALITION_END && w.coalition_t > COALITION_MIN);
        if over {
            w.coalition_against = None;
            for p in w.powers.iter_mut() {
                p.coalition = false;
            }
            let name = who(w, target);
            let player = w.is_player(target);
            w.say(TONE_INFO, player, format!("The coalition against {name} breaks up: {name} no longer threatens the galaxy."));
        } else {
            // Newcomers to space join the coalition.
            for p in w.powers.iter_mut() {
                p.coalition = p.alive && p.id != target;
            }
        }
        return;
    }
    let Some((l, s)) = first else { return };
    if alive_space < 3 || !w.powers[l].space || w.powers[l].dominance < COALITION_AT || s < COALITION_LEAD * second {
        return;
    }
    w.coalition_against = Some(l as u16);
    w.coalition_t = 0.0;
    let mut members = Vec::new();
    for p in 0..w.powers.len() {
        let join = w.powers[p].alive && p != l;
        w.powers[p].coalition = join;
        if join {
            members.push(if w.powers[p].kind == PowerKind::Player { "you".to_string() } else { w.powers[p].name.clone() });
            // Members stop fighting each other.
            for q in 0..w.powers.len() {
                if q != l && q != p {
                    w.rel_mut(p as u16, q as u16).grudge *= 0.5;
                }
            }
        }
    }
    let name = who(w, l as u16);
    let pct = w.powers[l].dominance * 100.0;
    let worlds = w.powers[l].worlds;
    let player_target = w.is_player(l as u16);
    let text = format!(
        "A coalition forms against {name}: {} band together. {} {} {worlds} world{} and {pct:.0}% of the galaxy's strength.",
        join_names(&members),
        if player_target { "You" } else { name.as_str() },
        if player_target { "hold" } else { "holds" },
        if worlds == 1 { "" } else { "s" }
    );
    let player_in = members.iter().any(|m| m == "you");
    w.say(if player_target { TONE_BAD } else { TONE_WARN }, player_target || player_in, text);
}

/// Galactic dominance: the leader holding `DOMINANCE_AT` for `DOMINANCE_HOLD` seconds.
fn dominance(w: &mut SpaceWar, dt: f32) {
    let Some(l) = w.leader else {
        w.dominance_t = 0.0;
        return;
    };
    if w.powers[l as usize].dominance >= DOMINANCE_AT && rules_enough(w, l) {
        w.dominance_t += dt;
    } else {
        w.dominance_t = (w.dominance_t - 2.0 * dt).max(0.0);
    }
    if w.dominance_t >= DOMINANCE_HOLD && w.dominant != Some(l) {
        w.dominant = Some(l);
        let player = w.is_player(l);
        let text = if player {
            "Galactic dominance: your world rules the galaxy. The others will not forgive it.".to_string()
        } else {
            format!("Galactic dominance: {} rules the galaxy.", w.powers[l as usize].name)
        };
        w.say(if player { TONE_GOOD } else { TONE_WARN }, true, text);
    }
}

/// Worlds power `p` rules, and how many dominance asks for: half the galaxy's inhabited
/// worlds (at least two), so a galaxy can't be dominated before there is anything to rule.
pub fn worlds_goal(w: &SpaceWar, p: u16) -> (u32, u32) {
    let inhabited = w.bodies.iter().filter(|b| b.inhabited()).count() as u32;
    (w.powers[p as usize].worlds, inhabited.div_ceil(2).max(2))
}

fn rules_enough(w: &SpaceWar, p: u16) -> bool {
    let (held, need) = worlds_goal(w, p);
    held >= need
}

fn who(w: &SpaceWar, p: u16) -> String {
    if w.is_player(p) {
        "your world".to_string()
    } else {
        w.powers[p as usize].name.clone()
    }
}

fn join_names(v: &[String]) -> String {
    match v.len() {
        0 => "nobody".to_string(),
        1 => v[0].clone(),
        n => format!("{} and {}", v[..n - 1].join(", "), v[n - 1]),
    }
}

/// The endgame's HUD rows for power `p` ("label\tvalue\tbad" lines): its tier, the galaxy's
/// leader, the dominance goal and any coalition.
pub fn hud_rows(w: &SpaceWar, p: u16) -> String {
    let pw = &w.powers[p as usize];
    let mut s = String::new();
    let next = TIERS.get(pw.tier as usize + 1);
    let tier = if pw.tier == 0 { "Spacefaring" } else { TIERS[pw.tier as usize].name };
    match next {
        Some(n) if pw.worlds.saturating_sub(1) < n.colonies && tier_progress(pw) >= 1.0 => {
            s.push_str(&format!("Tier\t{tier} · needs {} colon{}\t0\n", n.colonies, if n.colonies == 1 { "y" } else { "ies" }))
        }
        Some(_) => s.push_str(&format!("Tier\t{tier} · {:.0}%\t0\n", tier_progress(pw) * 100.0)),
        None => s.push_str(&format!("Tier\t{tier}\t0\n")),
    }
    match w.leader {
        Some(l) if l == p => s.push_str(&format!("Galaxy\tyou lead · {:.0}%\t0\n", pw.dominance * 100.0)),
        Some(l) => s.push_str(&format!("Galaxy\t{} leads · {:.0}%\t0\n", w.powers[l as usize].name, w.powers[l as usize].dominance * 100.0)),
        None => {}
    }
    if w.dominant == Some(p) {
        s.push_str("Goal\tgalactic dominance ✓\t0\n");
    } else if w.leader == Some(p) && pw.dominance >= DOMINANCE_AT && rules_enough(w, p) {
        s.push_str(&format!("Goal\tdominance held {:.0}/{:.0} s\t0\n", w.dominance_t, DOMINANCE_HOLD));
    } else {
        let (held, need) = worlds_goal(w, p);
        s.push_str(&format!("Goal\trule {held}/{need} worlds · {:.0}/{:.0}%\t0\n", pw.dominance * 100.0, DOMINANCE_AT * 100.0));
    }
    match w.coalition_against {
        Some(t) if t == p => s.push_str("Coalition\tagainst you!\t1\n"),
        Some(t) if pw.coalition => s.push_str(&format!("Coalition\tyou're in it, against {}\t0\n", w.powers[t as usize].name)),
        _ => {}
    }
    s
}

/// HUD rows about the whole galaxy, for a host without a player world in space (the sandboxes):
/// who leads, the coalition against it, and who holds galactic dominance.
pub fn galaxy_rows(w: &SpaceWar) -> String {
    let mut s = String::new();
    let space = w.powers.iter().filter(|p| p.alive && p.space).count();
    if space == 0 {
        return s;
    }
    s.push_str(&format!("In space\t{space} power{}\t0\n", if space == 1 { "" } else { "s" }));
    if let Some(l) = w.leader {
        let p = &w.powers[l as usize];
        let tier = if p.tier > 0 { format!(" · {}", tier_name(p.tier).to_lowercase()) } else { String::new() };
        s.push_str(&format!("Leader\t{} · {:.0}%{tier}\t0\n", p.name, p.dominance * 100.0));
    }
    if let Some(t) = w.coalition_against {
        let n = w.powers.iter().filter(|p| p.alive && p.coalition).count();
        s.push_str(&format!("Coalition\t{n} against {}\t1\n", w.powers[t as usize].name));
    }
    if let Some(d) = w.dominant {
        s.push_str(&format!("Dominance\t{} ✓\t0\n", w.powers[d as usize].name));
    }
    s
}
