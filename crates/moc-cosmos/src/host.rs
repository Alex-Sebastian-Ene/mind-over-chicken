//! Flat buffers and words for a host that keeps its planets elsewhere (#166): the browser's
//! galaxy reaches the war through `RustGalaxy` (crates/engine) with these, so the JS side only
//! copies numbers in and out and draws. Every stride is a constant here.

use crate::war::{Order, PowerKind, SpaceWar, WorldStats, BATTLE_SECONDS, MIN_LAUNCH};

/// Planet input, per planet: alive, pop, era, tier, progress, stock, stability, temper,
/// hand-played.
pub const BODY_IN: usize = 9;
/// Fleets out, per fleet: from, to, progress 0..1, order, owner, strength, bend, id.
pub const FLEET_OUT: usize = 8;
/// Battles out, per battle: planet, age 0..1, order, won, attacker, id.
pub const BATTLE_OUT: usize = 6;
/// Planets out, per planet: ruler (−1 none), shields, defences, garrison, the three caps,
/// seconds since its last battle.
pub const BODY_OUT: usize = 8;
/// Powers out, per power: alive, kind, home, space, fleet, fleet cap, tier, progress to the
/// next tier 0..1, dominance, coalition, worlds, key, stance, wins, losses, r, g, b.
pub const POWER_OUT: usize = 18;
/// What the war did, per planet: population ×, progress −, stores ×, taken by (−1), settled.
pub const FX_OUT: usize = 5;

impl SpaceWar {
    /// Sync every planet from a flat buffer (`BODY_IN` per planet); names change only with
    /// `sync_name`.
    pub fn sync_packed(&mut self, data: &[f32]) {
        for (i, c) in data.chunks_exact(BODY_IN).enumerate() {
            let stats = WorldStats {
                alive: c[0] > 0.5,
                pop: c[1].max(0.0),
                era: c[2].clamp(0.0, 255.0) as u8,
                tier: c[3].clamp(0.0, 255.0) as u8,
                progress: c[4],
                stock: c[5].max(0.0),
                stability: c[6],
                temper: c[7],
                hand_played: c[8] > 0.5,
            };
            let name = self.bodies.get(i).map(|b| b.name.clone()).unwrap_or_default();
            self.sync_body(i, &name, stats);
        }
    }

    /// Name planet `i` (and its own power, if it has one and was named after it).
    pub fn sync_name(&mut self, i: usize, name: &str) {
        let stats = self.bodies.get(i).map(|b| b.stats).unwrap_or_default();
        let old = self.bodies.get(i).map(|b| b.name.clone()).unwrap_or_default();
        self.sync_body(i, name, stats);
        for p in self.powers.iter_mut() {
            if p.kind == PowerKind::World && p.key == i as u32 && (p.name == old || p.name.is_empty()) {
                p.name = name.to_string();
            }
        }
    }

    pub fn pack_fleets(&self, out: &mut Vec<f32>) {
        out.clear();
        for s in &self.fleets {
            let Some(m) = s.mission else { continue };
            out.extend_from_slice(&[s.from_planet as f32, s.to_planet as f32, SpaceWar::fleet_progress(s), m.order as u8 as f32, m.owner as f32, m.strength, m.bend, m.id as f32]);
        }
    }

    pub fn pack_battles(&self, out: &mut Vec<f32>) {
        out.clear();
        for b in &self.battles {
            out.extend_from_slice(&[b.body as f32, b.t / BATTLE_SECONDS, b.order as u8 as f32, b.won as u8 as f32, b.attacker as f32, b.id as f32]);
        }
    }

    pub fn pack_bodies(&self, out: &mut Vec<f32>) {
        out.clear();
        for (i, b) in self.bodies.iter().enumerate() {
            let cap = self.caps(i);
            out.extend_from_slice(&[b.owner.map(|o| o as f32).unwrap_or(-1.0), b.shields, b.defences, b.garrison, cap.shields, cap.defences, cap.garrison, b.battle_t.min(1e6)]);
        }
    }

    pub fn pack_powers(&self, out: &mut Vec<f32>) {
        out.clear();
        for p in &self.powers {
            out.extend_from_slice(&[
                p.alive as u8 as f32,
                p.kind as u8 as f32,
                p.home as f32,
                p.space as u8 as f32,
                p.fleet,
                p.fleet_cap,
                p.tier as f32,
                crate::endgame::tier_progress(p),
                p.dominance,
                p.coalition as u8 as f32,
                p.worlds as f32,
                p.key as f32,
                p.stance,
                p.wins as f32,
                p.losses as f32,
                p.color[0] as f32,
                p.color[1] as f32,
                p.color[2] as f32,
            ]);
        }
    }

    /// Take every planet's war effects (`FX_OUT` per planet) and forget them.
    pub fn take_fx_packed(&mut self, out: &mut Vec<f32>) {
        out.clear();
        for i in 0..self.bodies.len() {
            let f = self.take_fx(i);
            out.extend_from_slice(&[f.pop_mul, f.progress_loss, f.stock_mul, f.captured_by as f32, f.settled as u8 as f32]);
        }
    }

    /// One line about planet `i` for the galaxy's planet list ("" when the war has nothing to say).
    pub fn planet_meta(&self, i: usize) -> String {
        let Some(b) = self.bodies.get(i) else { return String::new() };
        let Some(o) = b.owner else { return String::new() };
        let p = &self.powers[o as usize];
        if p.home != i && p.kind != PowerKind::Civ {
            let who = if p.kind == PowerKind::Player { "your colony".to_string() } else { format!("colony of {}", p.name) };
            return who;
        }
        if !p.space {
            return String::new();
        }
        let tier = crate::endgame::tier_name(p.tier);
        let mut s = format!("fleet {:.0} · shields {:.0}", p.fleet, b.shields);
        if p.worlds > 1 {
            s.push_str(&format!(" · {} worlds", p.worlds));
        }
        if !tier.is_empty() {
            s = format!("{tier} · {s}");
        }
        s
    }

    /// The orders panel's line about a target planet, for power `p` (the player).
    pub fn target_line(&self, p: u16, i: usize) -> String {
        let (Some(b), Some(pw)) = (self.bodies.get(i), self.powers.get(p as usize)) else { return String::new() };
        if !b.stats.alive {
            return format!("{} is still forming.", b.name);
        }
        if b.owner == Some(p) || pw.home == i {
            return format!("{} is yours.", b.name);
        }
        if !b.inhabited() {
            return format!("{}: barren. Invade to land colonists. Your fleet {:.0}.", b.name, pw.fleet);
        }
        let d = self.defence(i, b.owner);
        let ruler = b.owner.map(|o| self.powers[o as usize].name.clone()).unwrap_or_default();
        let held = if !ruler.is_empty() && ruler != b.name { format!(" (held by {ruler})") } else { String::new() };
        let truce = b.owner.is_some_and(|o| self.friendly(p, o));
        format!(
            "{}{held}: shields {:.0} · defences {:.0} · garrison {:.0}. Your fleet {:.0}{}",
            b.name,
            d.shields,
            d.defences,
            d.garrison,
            pw.fleet,
            if truce { ". You have a truce." } else if pw.fleet < MIN_LAUNCH { ", still building." } else { "." }
        )
    }

    /// The HUD rows for the player's power: "label\tvalue\tbad" lines (the galaxy's leader,
    /// coalition and dominance when the player has no world in space).
    pub fn hud_rows(&self) -> String {
        let Some(p) = self.player() else { return crate::endgame::galaxy_rows(self) };
        let pw = &self.powers[p as usize];
        if !pw.space {
            return crate::endgame::galaxy_rows(self);
        }
        let home = &self.bodies[pw.home];
        let mut s = String::new();
        let ready = pw.fleet >= MIN_LAUNCH;
        s.push_str(&format!("Fleet\t{:.0}/{:.0}\t{}\n", pw.fleet, pw.fleet_cap, (!ready) as u8));
        s.push_str(&format!("Shields\t{:.0} · garrison {:.0}\t0\n", home.shields, home.garrison));
        let incoming = self.fleets.iter().filter(|f| f.to_planet == pw.home && f.mission.is_some_and(|m| m.owner != p)).count();
        if incoming > 0 {
            s.push_str(&format!("Incoming\t{incoming} fleet{}\t1\n", if incoming == 1 { "" } else { "s" }));
        }
        s.push_str(&crate::endgame::hud_rows(self, p));
        s
    }

    /// The order a button sends, by its index (0 raid, 1 bombard, 2 invade).
    pub fn order_index(&mut self, p: u16, target: usize, order: u8) -> Result<u32, &'static str> {
        let Some(o) = Order::from_u8(order) else { return Err("No such order.") };
        self.order(p, target, o)
    }
}
