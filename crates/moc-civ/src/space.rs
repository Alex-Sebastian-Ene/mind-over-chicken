//! The space age of the civ sandbox (#166): civilisations at age 19+ build fleets and strike
//! other planets of the galaxy.
//!
//! A civilisation that reaches the Space Age (age 19) starts a **space program**: shipyards
//! turn its economy (production, knowledge and wealth, [`economy`]) into a fleet, and its
//! capital raises shields, orbital defences and a garrison. Launch pads and starports speed the
//! yards. From Interstellar (age 20) it keeps climbing the endgame tiers of
//! `moc_cosmos::endgame::TIERS` (orbital industry … megastructures), which multiply its fleet,
//! shields and economy; planets it takes off-world are colonies, which feed its economy, count
//! toward the ages' `colonies` metric, and open the higher tiers.
//!
//! The galaxy itself (other planets, battles, coalitions) is `moc_cosmos::war`, run by the host
//! (the browser's galaxy). The host tells each civilisation which planets it can strike
//! ([`set_targets`]), takes its launches ([`Space::launches`]) and hands back what happened
//! ([`apply_result`]). Same scales and rules as the galaxy's worlds: the formulas come from
//! `moc_cosmos`.
//!
//! **Decisions** go through their own seam, [`SpacePolicy`] (rules: [`SpaceRules`]), not
//! `politics::Decision`, so the war councils of #100/#115 are untouched. The player's mind
//! tilts them the same way: stress makes chiefs reckless, calm makes them dig in.
//!
//! The program runs on real seconds (`World::advance`'s), like the galaxy, so the ×16 speed
//! doesn't flood space with fleets; it does its work once a second, a few hundred operations.

use crate::world::*;
use moc_cosmos::endgame::{earned_tier, tier_fx, TIERS, XP_RATE};
use moc_cosmos::war::{Defence, Economy, Motive, Order, DEFENCE_PER_PROD, DEFENCE_REGEN, FLEET_BUILD, FLEET_PER_ECON, GARRISON_BASE, GARRISON_REGEN, MIN_LAUNCH, SHIELD_PER_KNOW, SHIELD_REGEN};

/// The age that opens the space program, and the one from which the endgame tiers climb.
pub const SPACE_AGE: u8 = 19;
pub const TIER_AGE: u8 = 20;
/// Floats per civilisation in the buffer JS reads (`civ_space_info_ptr`): in space (0/1),
/// fleet, fleet cap, shields, defences, garrison, tier, progress to the next tier, colonies,
/// stance, production, knowledge, resources, launches, people lost to attacks, temper.
pub const INFO_STRIDE: usize = 16;
/// Floats per launch (`civ_space_launches_ptr`): civilisation, planet, order, strength, motive.
pub const LAUNCH_STRIDE: usize = 5;
/// Most targets kept per civilisation.
pub const MAX_TARGETS: usize = 16;

/// A planet a civilisation may strike, as the galaxy sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    pub body: u16,
    /// Shields plus orbital defences, and the garrison.
    pub orbit: f32,
    pub garrison: f32,
    /// Its economy index (what there is to take).
    pub value: f32,
    /// Who rules it: 0 nobody, 1 a world, 2 a civilisation, 3 the player.
    pub kind: u8,
}

/// One civilisation's space program.
#[derive(Clone, Copy, Debug, Default)]
pub struct Program {
    pub active: bool,
    /// Test and `?debug` hook: in space whatever its age (`civ_debug_space`).
    pub forced: bool,
    pub econ: Economy,
    pub fleet: f32,
    pub fleet_cap: f32,
    pub shields: f32,
    pub defences: f32,
    pub garrison: f32,
    pub tier: u8,
    pub tier_xp: f32,
    /// Planets it holds off-world, and the economy index they bring.
    pub colonies: u32,
    pub colony_econ: f32,
    pub cooldown: f32,
    /// 0 dove … 1 hawk, now (traits tilted by the player's mind).
    pub stance: f32,
    pub launches: u32,
    pub loot: f32,
    /// People lost to attacks from space.
    pub lost: u32,
}

/// A fleet a civilisation sends (the host flies it in the galaxy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub civ: u8,
    pub body: u16,
    pub order: Order,
    pub strength: f32,
    pub motive: Motive,
}

/// What a civilisation decides about space this second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpaceDecision {
    Hold,
    /// Send `commit` of the docked fleet against target `target` (an index into its targets).
    Launch { target: usize, order: Order, commit: f32, motive: Motive },
}

/// A space council for one civilisation. The rules ([`SpaceRules`]) are the default; a trained
/// policy implements this and is set in `World::space.policy`. Its own seam, apart from
/// `politics::Policy`.
pub trait SpacePolicy {
    fn decide(&mut self, w: &World, civ: u8, out: &mut Vec<SpaceDecision>);
}

/// Everything about space in the sandbox.
pub struct Space {
    pub programs: Vec<Program>,
    pub targets: Vec<Vec<Target>>,
    /// Launches since the host last took them.
    pub launches: Vec<Launch>,
    pub policy: Option<Box<dyn SpacePolicy>>,
    pub acc: f32,
    pub councils: u32,
    /// Buffers JS reads.
    pub info: Vec<f32>,
    pub out: Vec<f32>,
    decisions: Vec<SpaceDecision>,
}

impl Default for Space {
    fn default() -> Self {
        Space {
            programs: vec![Program::default(); MAX_FACTIONS],
            targets: vec![Vec::new(); MAX_FACTIONS],
            launches: Vec::new(),
            policy: None,
            acc: 0.0,
            councils: 0,
            info: vec![0.0; MAX_FACTIONS * INFO_STRIDE],
            out: Vec::new(),
            decisions: Vec::new(),
        }
    }
}

/// The economy of an age-19 civilisation that `civ_debug_space` lends a younger one.
const FORCED_FLOOR: Economy = Economy { production: 8.0, knowledge: 9.0, resources: 6.0 };

/// A civilisation's economy in the galaxy's strength units (a fresh Space Age civilisation,
/// knowledge 450 and 200 goods a minute, is about 8–9 in each).
/// Seam (#165): when civilisations have resources, `resources` should read them, not wealth.
pub fn economy(f: &Faction) -> Economy {
    let m = &f.metrics;
    Economy {
        production: (m.production.max(0.0) / 200.0).sqrt() * 8.0,
        knowledge: (m.knowledge.max(0.0) / 450.0).sqrt() * 9.0,
        resources: m.wealth.max(0.0).sqrt() * 0.25,
    }
}

/// How a civilisation's chiefs lean in space, 0 dove … 1 hawk.
pub fn temper(t: &Traits) -> f32 {
    (0.6 * t.aggressiveness + 0.4 * t.violence).clamp(0.0, 1.0)
}

/// Launch pads and starports in its lands: faster yards and a bigger fleet.
fn yards(w: &World, f: usize) -> f32 {
    let (pad, port) = (1u64 << Building::LaunchPad as u8, 1u64 << Building::Starport as u8);
    let (mut pads, mut ports) = (0u32, 0u32);
    for v in w.villages.iter().filter(|v| v.alive && v.owner as usize == f) {
        pads += (v.built & pad != 0) as u32;
        ports += (v.built & port != 0) as u32;
    }
    1.0 + 0.1 * pads.min(3) as f32 + 0.25 * ports.min(2) as f32
}

/// Once a second of real time: programs start and stop with the ages, yards build, defences
/// regrow, tiers climb, and every civilisation in space holds its council.
pub fn tick(w: &mut World, dt: f32) {
    if !(dt > 0.0) {
        return;
    }
    w.space.acc += dt;
    if w.space.acc < 1.0 {
        return;
    }
    let dt = std::mem::take(&mut w.space.acc).min(5.0);
    w.space.councils += 1;
    let h = w.inputs.hostility().clamp(-1.0, 1.0);
    let calm = (-h).clamp(0.0, 1.0);
    let focus = w.inputs.focus.clamp(0.0, 1.0);
    let mut any = false;
    for f in 0..MAX_FACTIONS {
        let fac = &w.factions[f];
        let forced = w.space.programs[f].forced;
        let on = fac.alive && !fac.pending && (fac.age >= SPACE_AGE || forced);
        if !on {
            let p = &mut w.space.programs[f];
            if p.active {
                // Fallen out of the Space Age (a Dark Age, or gone): the yards fall silent.
                *p = Program { forced: p.forced, colonies: p.colonies, ..Program::default() };
            }
            continue;
        }
        any = true;
        let mut econ = economy(fac);
        if forced {
            econ = Economy { production: econ.production.max(FORCED_FLOOR.production), knowledge: econ.knowledge.max(FORCED_FLOOR.knowledge), resources: econ.resources.max(FORCED_FLOOR.resources) };
        }
        let climbing = fac.age >= TIER_AGE || forced;
        let (temper, warriors, pop) = (temper(&fac.traits), fac.warriors, fac.pop);
        let yard = yards(w, f);
        let p = &mut w.space.programs[f];
        p.active = true;
        let fx = tier_fx(p.tier);
        p.econ = econ.scaled(fx.econ);
        let index = p.econ.index() + p.colony_econ;
        p.fleet_cap = FLEET_PER_ECON * index * fx.fleet * yard;
        // Stress puts the yards on a war footing; calm spends on shields instead.
        let pace = 1.0 + 0.3 * h;
        if p.fleet < p.fleet_cap {
            p.fleet = (p.fleet + p.fleet_cap * FLEET_BUILD * pace * yard * dt).min(p.fleet_cap);
        }
        let dig_in = 1.0 + 0.25 * calm;
        let cap = Defence {
            shields: SHIELD_PER_KNOW * p.econ.knowledge * fx.shields * dig_in,
            defences: DEFENCE_PER_PROD * p.econ.production * fx.shields * dig_in,
            garrison: (GARRISON_BASE + 0.3 * warriors as f32 + 2.0 * (pop as f32 / 10.0).sqrt()) * (1.0 + 0.2 * p.tier as f32),
        };
        let grow = |v: f32, cap: f32, rate: f32| if v < cap { (v + cap * rate * dt).min(cap) } else { cap };
        p.shields = grow(p.shields, cap.shields, SHIELD_REGEN * (1.0 + calm));
        p.defences = grow(p.defences, cap.defences, DEFENCE_REGEN * (1.0 + 0.5 * calm));
        p.garrison = grow(p.garrison, cap.garrison, GARRISON_REGEN);
        // The endgame: tiers from Interstellar on (colonies open the higher ones).
        if climbing {
            p.tier_xp += (0.6 * p.econ.knowledge + 0.4 * p.econ.production) * XP_RATE * (1.0 + 0.5 * focus) * dt;
            if let Some(next) = TIERS.get(p.tier as usize + 1) {
                if p.colonies < next.colonies {
                    p.tier_xp = p.tier_xp.min(next.xp);
                }
            }
            p.tier = earned_tier(p.tier, p.tier_xp, p.colonies);
        }
        p.stance = (temper + 0.55 * h).clamp(0.0, 1.0);
        p.cooldown = (p.cooldown - dt).max(0.0);
    }
    if any {
        councils(w);
    }
    refresh_info(w);
}

/// Every civilisation in space with its fleet ready decides (in id order).
fn councils(w: &mut World) {
    let mut policy = w.space.policy.take();
    let mut out = std::mem::take(&mut w.space.decisions);
    for f in 0..MAX_FACTIONS as u8 {
        let p = w.space.programs[f as usize];
        if !p.active || p.cooldown > 0.0 || p.fleet < MIN_LAUNCH || w.space.targets[f as usize].is_empty() {
            continue;
        }
        out.clear();
        match policy.as_mut() {
            Some(pol) => pol.decide(w, f, &mut out),
            None => SpaceRules.decide(w, f, &mut out),
        }
        for d in out.iter() {
            if let SpaceDecision::Launch { target, order, commit, motive } = *d {
                let Some(t) = w.space.targets[f as usize].get(target).copied() else { continue };
                let p = &mut w.space.programs[f as usize];
                let strength = (p.fleet * commit.clamp(0.0, 1.0)).max(MIN_LAUNCH.min(p.fleet));
                p.fleet -= strength;
                p.cooldown = 10.0 + 8.0 * (1.0 - p.stance);
                p.launches += 1;
                w.space.launches.push(Launch { civ: f, body: t.body, order, strength, motive });
                break;
            }
        }
    }
    w.space.decisions = out;
    w.space.policy = policy;
}

/// A deterministic die (as `politics::roll`): 0..1 for every council, civilisation and question.
pub fn roll(w: &World, f: u8, salt: u32) -> f32 {
    let mut h = w.seed ^ w.space.councils.wrapping_mul(0x9e37_79b9) ^ (f as u32).wrapping_mul(0x85eb_ca6b) ^ salt.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The default space council: strike when the fleet is ready, at the planet worth most and
/// defended least; invade what the fleet can take, bombard rival civilisations, raid the rest.
pub struct SpaceRules;

impl SpacePolicy for SpaceRules {
    fn decide(&mut self, w: &World, civ: u8, out: &mut Vec<SpaceDecision>) {
        let p = &w.space.programs[civ as usize];
        let h = w.inputs.hostility().clamp(-1.0, 1.0);
        let ready = (0.55 - 0.25 * h).clamp(0.25, 0.85);
        if p.fleet < ready * p.fleet_cap {
            return;
        }
        let calm = (-h).clamp(0.0, 1.0);
        if roll(w, civ, 1) > (0.06 + 0.3 * p.stance) * (1.0 - 0.8 * calm) {
            return;
        }
        let mut best: Option<(f32, SpaceDecision)> = None;
        for (k, t) in w.space.targets[civ as usize].iter().enumerate() {
            let total = t.orbit + t.garrison;
            let weakness = p.fleet / (p.fleet + total + 1.0);
            let rival = if t.kind == 2 { 1.3 } else { 1.0 };
            let score = (t.value + 1.0) * weakness * rival * (0.85 + 0.3 * roll(w, civ, 2 + k as u32));
            let invade_needs = t.orbit * 1.1 + t.garrison * 1.5 + 2.0;
            let order = if t.kind != 3 && p.fleet * Order::Invade.commit() > invade_needs && p.stance > 0.35 {
                Order::Invade
            } else if t.kind == 2 || (p.stance > 0.65 && p.fleet * Order::Bombard.commit() > t.orbit) {
                Order::Bombard
            } else {
                Order::Raid
            };
            let motive = match order {
                Order::Invade => Motive::Expansion,
                Order::Bombard => Motive::Rivalry,
                Order::Raid => Motive::Plunder,
            };
            let d = SpaceDecision::Launch { target: k, order, commit: order.commit(), motive };
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, d));
            }
        }
        if let Some((_, d)) = best {
            out.push(d);
        }
    }
}

/// The planets civilisation `f` may strike (from the host; at most `MAX_TARGETS`).
pub fn set_targets(w: &mut World, f: usize, targets: &[Target]) {
    if let Some(t) = w.space.targets.get_mut(f) {
        t.clear();
        t.extend_from_slice(&targets[..targets.len().min(MAX_TARGETS)]);
    }
}

/// What the galaxy did to civilisation `f` since the last sync.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpaceResult {
    /// Stores and knowledge won (strength units; negative: lost to raiders).
    pub loot: f32,
    pub knowledge: f32,
    /// Share of its people killed by bombardment and beachheads.
    pub hits: f32,
    /// Shields and defences knocked out.
    pub shield_loss: f32,
    /// Off-world colonies gained (+) or lost (−).
    pub colonies: i32,
    /// Ships that came home.
    pub returned: f32,
    /// The economy index its colonies bring now.
    pub colony_econ: f32,
}

/// Apply what the galaxy did: loot into the treasury and knowledge, people killed from orbit,
/// shields knocked down, colonies won and lost, ships home.
pub fn apply_result(w: &mut World, f: usize, r: &SpaceResult) {
    if f >= MAX_FACTIONS || !w.factions[f].alive {
        return;
    }
    {
        let p = &mut w.space.programs[f];
        p.fleet += r.returned.max(0.0);
        p.colonies = (p.colonies as i32 + r.colonies).max(0) as u32;
        p.colony_econ = r.colony_econ.max(0.0);
        p.loot += r.loot.max(0.0);
        let orbit = p.shields + p.defences;
        if r.shield_loss > 0.0 && orbit > 0.0 {
            let k = (1.0 - r.shield_loss / orbit).clamp(0.0, 1.0);
            p.shields *= k;
            p.defences *= k;
        }
    }
    let fac = &mut w.factions[f];
    // A strength unit of loot is worth five goats and five points of knowledge.
    fac.treasury = (fac.treasury + 5.0 * r.loot).max(0.0);
    fac.metrics.knowledge += 5.0 * r.knowledge.max(0.0);
    if r.hits > 0.0 {
        let want = ((fac.pop as f32) * r.hits.min(0.5)).round() as usize;
        let mut killed = 0;
        // Deterministic: every k-th of its people, from a rolling start.
        let mine: Vec<usize> = w.agents.iter().enumerate().filter(|(_, a)| a.alive && a.faction as usize == f).map(|(i, _)| i).collect();
        if !mine.is_empty() && want > 0 {
            let step = (mine.len() / want.max(1)).max(1);
            let start = (w.space.councils as usize) % step;
            for &i in mine.iter().skip(start).step_by(step).take(want) {
                w.kill(i, Cause::Disaster);
                killed += 1;
            }
        }
        w.space.programs[f].lost += killed;
        // Fire from the sky over its capital.
        if let Some(v) = w.villages.get(w.factions[f].capital as usize).filter(|v| v.alive) {
            let (x, y) = (v.x as f32 + 0.5, v.y as f32 + 0.5);
            w.effect(x, y, 1, 1.4, 12.0);
        }
    }
    refresh_info(w);
}

/// Colonies off-world (for the ages' `colonies` metric: Interstellar needs one).
pub fn colonies(w: &World, f: usize) -> u32 {
    w.space.programs.get(f).map(|p| p.colonies).unwrap_or(0)
}

/// Refill the buffer JS reads (`INFO_STRIDE` per civilisation).
pub fn refresh_info(w: &mut World) {
    let info = &mut w.space.info;
    info.resize(MAX_FACTIONS * INFO_STRIDE, 0.0);
    for f in 0..MAX_FACTIONS {
        let p = &w.space.programs[f];
        let o = f * INFO_STRIDE;
        let next = TIERS.get(p.tier as usize + 1).map(|n| ((p.tier_xp - TIERS[p.tier as usize].xp) / (n.xp - TIERS[p.tier as usize].xp)).clamp(0.0, 1.0)).unwrap_or(1.0);
        info[o..o + INFO_STRIDE].copy_from_slice(&[
            p.active as u8 as f32,
            p.fleet,
            p.fleet_cap,
            p.shields,
            p.defences,
            p.garrison,
            p.tier as f32,
            next,
            p.colonies as f32,
            p.stance,
            p.econ.production,
            p.econ.knowledge,
            p.econ.resources,
            p.launches as f32,
            p.lost as f32,
            temper(&w.factions[f].traits),
        ]);
    }
    let out = &mut w.space.out;
    out.clear();
    for l in &w.space.launches {
        out.extend_from_slice(&[l.civ as f32, l.body as f32, l.order as u8 as f32, l.strength, l.motive as u8 as f32]);
    }
}
