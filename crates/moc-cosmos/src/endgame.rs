//! The endgame (#166): what comes after the last age.
//!
//! Stage 1 only measures who leads the galaxy (the share of its strength each power holds), so
//! the councils know whom to fear. Tiers, coalitions and galactic dominance come later.

use crate::war::{SpaceWar, TierFx};

/// Multipliers of endgame tier `t` (none yet).
pub fn tier_fx(_t: u8) -> TierFx {
    TierFx { fleet: 1.0, shields: 1.0, econ: 1.0 }
}

/// A power's weight in the galaxy: its fleet, its economy and the worlds it rules.
pub fn score(w: &SpaceWar, p: usize) -> f32 {
    let pw = &w.powers[p];
    if !pw.alive {
        return 0.0;
    }
    pw.fleet + 3.0 * pw.econ.index() + 5.0 * pw.worlds as f32
}

/// Once a second: everyone's share of the galaxy, and its leader.
pub fn council(w: &mut SpaceWar, _dt: f32) {
    let scores: Vec<f32> = (0..w.powers.len()).map(|p| score(w, p)).collect();
    let total: f32 = scores.iter().sum::<f32>().max(1e-3);
    let mut leader: Option<(usize, f32)> = None;
    for (p, &s) in scores.iter().enumerate() {
        w.powers[p].dominance = s / total;
        if s > 0.0 && leader.is_none_or(|(_, b)| s > b) {
            leader = Some((p, s));
        }
    }
    w.leader = leader.map(|(p, _)| p as u16);
}
