//! Daily life (#69, `data/ages.json` `daily_life`), kept cheap: most of it is village state
//! that agents read as they go.
//!
//! - **Night and sleep** (from the Hearth Clans, age 2): past `SLEEP_NIGHT` agents go home and
//!   sleep (`agents::update`); street lights (age 17) keep towns working.
//! - **Meals** at the hearth: cooks and a campfire make each mouthful go further; markets feed
//!   passers-by.
//! - **Festivals** when the player smiles (camera joy) or is calm, and after an age-up: the
//!   village gathers, unrest falls, faith rises, food is eaten.
//! - **Mourning** after a violent or untimely death (`World::kill`): unrest rises a little,
//!   faith too, and mourners stay close to home.
//! - **Families**: every agent has a clan and a household (`World::spawn_agent`); clans feud.
//! - **Migration** when hungry or at war: people leave a starving or besieged village for a
//!   better one of their own.
//! - Food **spoils** above what a village can store (granaries, mills, huts store more), and
//!   **plague** (from age 12, spread along trade) sickens crowded villages until wells,
//!   aqueducts, monks and the player's focus clear it.

use crate::jobs::storage;
use crate::world::*;

/// Village-level daily life, every bookkeeping tick.
pub fn villages(w: &mut World, dt: f32) {
    let joy = w.inputs.joy.clamp(0.0, 1.0);
    let calm = w.inputs.calm.clamp(0.0, 1.0);
    let focus = w.inputs.focus.clamp(0.0, 1.0);
    let festive = joy.max(calm - 0.15);
    let mut migrate: Vec<u16> = Vec::new();
    for vi in 0..w.villages.len() {
        if !w.villages[vi].alive {
            continue;
        }
        let owner = w.villages[vi].owner as usize;
        let age = w.factions[owner].age;
        let player = w.factions[owner].is_player;
        let capital = w.factions[owner].capital as usize == vi;
        let v = &mut w.villages[vi];
        v.mourn_t = (v.mourn_t - dt).max(0.0);
        v.festival_t = (v.festival_t - dt).max(0.0);
        // Festivals: a joyful or calm god, a full larder.
        let mood = if player { festive } else { festive * 0.6 };
        let mut fest = false;
        if v.festival_t <= 0.0 && age >= 2 && v.food >= 4.0 && mood > 0.5 && w.rng.chance(dt * 0.025 * mood) {
            v.festival_t = 12.0;
            fest = capital;
        }
        if v.festival_t > 0.0 {
            v.unrest = (v.unrest - 0.03 * dt).max(0.0);
            v.faith = (v.faith + 0.004 * dt).min(1.0);
            v.food = (v.food - 0.05 * dt).max(0.0);
        }
        if v.mourn_t > 0.0 {
            v.unrest = (v.unrest + 0.004 * dt).min(1.5);
        }
        // Food spoils above what the village can store.
        let store = storage(&v.fx, v.pop);
        if v.food > store {
            v.food -= (v.food - store) * 0.05 * dt;
        }
        // Plague: crowding feeds it; wells, aqueducts, monks and focus fight it.
        if v.plague > 0.0 {
            let crowd = (v.pop as f32 / v.cap.max(1) as f32).min(1.5);
            let heal = v.fx.heal + if age >= 12 { 0.01 * focus } else { 0.0 };
            v.plague = (v.plague + dt * (0.012 * crowd - 0.014 - heal)).clamp(0.0, 1.0);
            v.unrest = (v.unrest + v.plague * 0.01 * dt).min(1.5);
        }
        // Migration: hungry or under attack.
        let hungry = v.hungry as f32 / v.pop.max(1) as f32 > 0.35 && v.food < 1.5;
        let war = v.siege > 0.0 || w.time - v.raided_t < 10.0;
        if (hungry || war) && v.pop >= 3 && w.rng.chance(dt * 0.15) {
            migrate.push(vi as u16);
        }
        if fest {
            w.stats.festivals += 1;
            let (x, y) = (w.villages[vi].x, w.villages[vi].y);
            w.log(ev::FESTIVAL, owner as u8, NO_F, x, y, 0);
        }
    }
    for v in migrate.into_iter().take(4) {
        move_people(w, v);
    }
}

/// Two civilians leave village `from` for a better-fed, safe village of their own people on the
/// same land.
fn move_people(w: &mut World, from: u16) {
    let (f, x, y) = {
        let v = &w.villages[from as usize];
        (v.owner, v.x, v.y)
    };
    let land = w.terrain.land(x, y);
    let mut best: Option<(f32, u16)> = None;
    for (i, v) in w.villages.iter().enumerate() {
        if i as u16 == from || !v.alive || v.owner != f || v.siege > 0.0 || v.food < 6.0 || v.pop >= v.cap || w.terrain.land(v.x, v.y) != land {
            continue;
        }
        let d = w.d.dist(x as f32, y as f32, v.x as f32, v.y as f32);
        if d < 60.0 && best.map_or(true, |b| d < b.0) {
            best = Some((d, i as u16));
        }
    }
    let Some((_, to)) = best else { return };
    let mut moved = 0;
    for a in w.agents.iter_mut() {
        if moved >= 2 {
            break;
        }
        if a.alive && a.faction == f && a.home == from && !a.job.fights() && a.carry <= 0.0 && a.prisoner <= 0.0 {
            a.home = to;
            a.state = State::ToHome;
            moved += 1;
        }
    }
}

/// Plague creeps to neighbouring villages (and along trade, in `politics::trade`).
pub fn spread_plague(w: &mut World, dt: f32) {
    let sick: Vec<(i32, i32, f32, u16)> = w
        .villages
        .iter()
        .filter(|v| v.alive && v.plague > 0.25)
        .map(|v| (v.x, v.y, v.plague, w.terrain.land(v.x, v.y)))
        .collect();
    if sick.is_empty() {
        return;
    }
    for i in 0..w.villages.len() {
        let v = &w.villages[i];
        if !v.alive || v.plague > 0.05 {
            continue;
        }
        let land = w.terrain.land(v.x, v.y);
        let mut p = 0.0;
        for &(x, y, pl, l) in &sick {
            if l == land && w.d.dist(x as f32, y as f32, v.x as f32, v.y as f32) < 28.0 {
                p += pl * 0.02;
            }
        }
        if p > 0.0 && w.rng.chance((p * dt).min(0.5)) {
            w.villages[i].plague = 0.15;
        }
    }
}

/// Is it sleeping time for this civilisation? (Night falls for the Hearth Clans on.)
#[inline]
pub fn asleep(age: u8, night: f32, lights: bool) -> bool {
    age >= 2 && night > SLEEP_NIGHT && !lights
}
