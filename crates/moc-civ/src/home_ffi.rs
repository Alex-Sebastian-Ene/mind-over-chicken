//! Browser interface for the rivals on the home world (#212, [`crate::homeworld`]), in the
//! style of [`crate::ffi`] and [`crate::war_ffi`]: plain `extern "C"`, a handle JS keeps as a
//! number, flat buffers read through typed-array views on `memory.buffer`.
//!
//! - Inputs go through `civ_home_buf(kind, len)`: 0 the home World's tiles (u8 per tile),
//!   1 its heights (f32 per tile), 2 its continents (7 f32 each: cx, cy, rx, ry, village x,
//!   village y, area), 3 the player's settlements (2 f32 each, camp first), 4 the player's
//!   boats (2 f32 each). Fill, then call `civ_home_new` / `civ_home_set_player`.
//! - `civ_home_world(h)` is the simulation's `*mut World`: every `civ_*` export of `ffi.rs`
//!   and `war_ffi.rs` works on it (inputs, factions, names, stats, news). Free the home world
//!   with `civ_home_free`, never `civ_free`.
//! - Buffers after each `civ_home_advance`: rival agents (`civ_home_agent_stride` f32: x, y,
//!   faction, id, kind 0 worker / 1 fighter / 2 at sea / 3 settler), villages (x, y, owner,
//!   pop, radius, capital, flags, age, buildings; flags 1 fields, 2 walls, 4 harbour, 8 under
//!   siege, 16 launch pad, 32 factory, 64 just raided), boats (x, y, rival, raid, facing),
//!   and a row per faction slot (`civ_home_rival_stride`: state 0 unused / 1 alive / 2 waiting
//!   / 3 the player, pop, villages, age, r, g, b, stance toward the player, tension, war,
//!   met, capital x, capital y, launch progress, land, parent).
//! - Lines (`homeworld::HomeLine`) numbered from `civ_home_line_first` to `civ_home_line_next`:
//!   `civ_home_line_meta(h, seq, k)` with k 0 kind, 1 tone, 2 rival, 3 x, 4 y, 5 food, 6 hurt,
//!   7 market, 8 n, 9 priority, 10 cooldown; `civ_home_line_str_*` with k 0 ticker text,
//!   1 advisor line, 2 advisor key.

use std::cell::RefCell;

use crate::homeworld::{self, Continent, Difficulty, HomeWorld};
use crate::world::World;

thread_local! {
    static BYTES: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static FLOATS: RefCell<[Vec<f32>; 5]> = const { RefCell::new([Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()]) };
}

fn hw<'a>(p: *mut HomeWorld) -> Option<&'a mut HomeWorld> {
    // SAFETY: JS only passes handles returned by civ_home_new and not yet freed.
    unsafe { p.as_mut() }
}

fn floats(kind: usize) -> Vec<f32> {
    FLOATS.with(|f| f.borrow()[kind].clone())
}

/// A buffer of `len` items for input `kind` (see the module notes); u8 for kind 0, else f32.
#[no_mangle]
pub extern "C" fn civ_home_buf(kind: u32, len: usize) -> *mut u8 {
    if kind == 0 {
        return BYTES.with(|b| {
            let mut b = b.borrow_mut();
            b.clear();
            b.resize(len, 0);
            b.as_mut_ptr()
        });
    }
    let k = (kind as usize).min(4);
    FLOATS.with(|f| {
        let mut f = f.borrow_mut();
        f[k].clear();
        f[k].resize(len, 0.0);
        f[k].as_mut_ptr() as *mut u8
    })
}

/// The rivals of a `w`×`h` home world from the tiles, heights and `n_cont` continents in the
/// input buffers: `rivals` peoples (0: by the world's size, plus `offset`, the difficulty's).
#[no_mangle]
pub extern "C" fn civ_home_new(seed: u32, w: u32, h: u32, n_cont: u32, rivals: u32, offset: i32) -> *mut HomeWorld {
    let (w, h) = (w as usize, h as usize);
    let types = BYTES.with(|b| b.borrow().clone());
    let heights = floats(1);
    let c = floats(2);
    let conts: Vec<Continent> = (0..n_cont as usize)
        .filter(|k| c.len() >= k * 7 + 7)
        .map(|k| {
            let o = k * 7;
            Continent { cx: c[o], cy: c[o + 1], rx: c[o + 2], ry: c[o + 3], village: (c[o + 4] as i32, c[o + 5] as i32) }
        })
        .collect();
    if types.len() < w * h || w == 0 || h == 0 {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(HomeWorld::new(seed, w, h, &types, &heights, &conts, rivals as usize, offset)))
}

#[no_mangle]
pub extern "C" fn civ_home_free(p: *mut HomeWorld) {
    if !p.is_null() {
        // SAFETY: p came from Box::into_raw in civ_home_new.
        unsafe { drop(Box::from_raw(p)) };
    }
}

/// The simulation inside, for the `civ_*` exports.
#[no_mangle]
pub extern "C" fn civ_home_world(p: *mut HomeWorld) -> *mut World {
    hw(p).map(|h| &mut h.w as *mut World).unwrap_or(std::ptr::null_mut())
}

/// The war councils for the rivals (`civ_ppo_enable` mode: 0 rules, 1 councils), with the
/// player's people kept out of them. Returns `civ_ppo_enable`'s status.
#[no_mangle]
pub extern "C" fn civ_home_ppo(p: *mut HomeWorld, mode: u32) -> i32 {
    let Some(h) = hw(p) else { return 0 };
    let s = crate::war_ffi::civ_ppo_enable(&mut h.w as *mut World, mode.min(1));
    h.guard_policy();
    s
}

/// The difficulty's numbers (`homeworld::Difficulty`).
#[no_mangle]
pub extern "C" fn civ_home_set_difficulty(p: *mut HomeWorld, aggression: f32, raids: f32, damage: f32, growth: f32) {
    if let Some(h) = hw(p) {
        h.difficulty = Difficulty { aggression, raids, damage, growth }.clamped();
    }
}

/// The player this frame: `n_sites` settlements and `n_boats` boats from input buffers 3 and
/// 4, people, the larder, and (Space Age) launch progress and whether the rocket has flown.
#[no_mangle]
pub extern "C" fn civ_home_set_player(p: *mut HomeWorld, n_sites: u32, n_boats: u32, pop: u32, food: f32, launch: f32, launched: u32) {
    let Some(h) = hw(p) else { return };
    let s = floats(3);
    let b = floats(4);
    let pl = &mut h.player;
    pl.sites = (0..n_sites as usize).filter(|k| s.len() >= k * 2 + 2).map(|k| (s[k * 2].round() as i32, s[k * 2 + 1].round() as i32)).collect();
    pl.boats = (0..n_boats as usize).filter(|k| b.len() >= k * 2 + 2).map(|k| (b[k * 2], b[k * 2 + 1])).collect();
    pl.pop = pop;
    pl.food = if food.is_finite() { food } else { 0.0 };
    pl.launch = launch.clamp(0.0, 1.0);
    pl.launched = launched != 0;
}

/// The player's stage: ages `lo..=hi` (1..=20), `fast` 1 in a cutscene (#222's stages; today's
/// scenes have defaults, `civ_home_scene_ages`).
#[no_mangle]
pub extern "C" fn civ_home_set_stage(p: *mut HomeWorld, lo: u32, hi: u32, fast: u32) {
    if let Some(h) = hw(p) {
        h.set_stage(lo.min(255) as u8, hi.min(255) as u8, fast != 0);
    }
}

/// Today's scene `k` (0 Stone Age … 4 Space Age): its lowest (`hi` 0) or highest (`hi` 1) age.
#[no_mangle]
pub extern "C" fn civ_home_scene_ages(k: u32, hi: u32) -> u32 {
    let a = homeworld::SCENE_AGES[(k as usize).min(homeworld::ERAS - 1)];
    (if hi != 0 { a.1 } else { a.0 }) as u32
}

/// Lift rivals that have fallen behind this scene's times. Returns how many.
#[no_mangle]
pub extern "C" fn civ_home_catch_up(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| h.catch_up() as u32).unwrap_or(0)
}

/// Run `sim_dt` simulated seconds (JS paces it: `civ_home_speed` per real second).
#[no_mangle]
pub extern "C" fn civ_home_advance(p: *mut HomeWorld, sim_dt: f32) {
    if let Some(h) = hw(p) {
        h.advance(if sim_dt.is_finite() { sim_dt } else { 0.0 });
    }
}

/// Simulated seconds per real second for the current stage (JS paces `civ_home_advance`).
#[no_mangle]
pub extern "C" fn civ_home_speed(p: *mut HomeWorld) -> f32 {
    hw(p).map(|h| h.speed()).unwrap_or(1.0)
}

/// Rival peoples a `w`×`h` home world gets by its size (before the land check).
#[no_mangle]
pub extern "C" fn civ_home_rival_count(w: u32, h: u32) -> u32 {
    homeworld::rival_count(w as usize, h as usize) as u32
}

#[no_mangle]
pub extern "C" fn civ_home_mirror(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| h.mirror as u32).unwrap_or(255)
}

#[no_mangle]
pub extern "C" fn civ_home_time(p: *mut HomeWorld) -> f32 {
    hw(p).map(|h| h.w.time).unwrap_or(0.0)
}

/// Rival peoples placed (arrived or waiting).
#[no_mangle]
pub extern "C" fn civ_home_rivals_total(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| h.rival_total() as u32).unwrap_or(0)
}

/// Rival peoples alive now (with any rebels split from them).
#[no_mangle]
pub extern "C" fn civ_home_rivals_alive(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| h.rivals_alive() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_home_rival_stride() -> u32 {
    homeworld::RIVAL_STRIDE as u32
}

#[no_mangle]
pub extern "C" fn civ_home_rivals_ptr(p: *mut HomeWorld) -> *const f32 {
    hw(p).map(|h| h.rivals_buf.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_home_agent_stride() -> u32 {
    homeworld::AGENT_STRIDE as u32
}

#[no_mangle]
pub extern "C" fn civ_home_agent_count(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| (h.agents_buf.len() / homeworld::AGENT_STRIDE) as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_home_agents_ptr(p: *mut HomeWorld) -> *const f32 {
    hw(p).map(|h| h.agents_buf.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_home_village_stride() -> u32 {
    homeworld::VILLAGE_STRIDE as u32
}

#[no_mangle]
pub extern "C" fn civ_home_village_count(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| (h.villages_buf.len() / homeworld::VILLAGE_STRIDE) as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_home_villages_ptr(p: *mut HomeWorld) -> *const f32 {
    hw(p).map(|h| h.villages_buf.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_home_boat_stride() -> u32 {
    homeworld::BOAT_STRIDE as u32
}

#[no_mangle]
pub extern "C" fn civ_home_boat_count(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| (h.boats_buf.len() / homeworld::BOAT_STRIDE) as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_home_boats_ptr(p: *mut HomeWorld) -> *const f32 {
    hw(p).map(|h| h.boats_buf.as_ptr()).unwrap_or(std::ptr::null())
}

/// Paint the rivals' territory, one pixel per `step`×`step` tiles (RGBA at
/// `civ_home_territory_ptr`, `civ_home_territory_w` × `_h`).
#[no_mangle]
pub extern "C" fn civ_home_territory(p: *mut HomeWorld, step: u32) -> u32 {
    hw(p).map(|h| h.render_territory(step as usize).0 as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_home_territory_ptr(p: *mut HomeWorld) -> *const u8 {
    hw(p).map(|h| h.territory.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_home_territory_len(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| h.territory.len() as u32).unwrap_or(0)
}

/// "Ashkai · Bronze Age (8) · wary of you" into the text buffer; returns its length.
#[no_mangle]
pub extern "C" fn civ_home_label(p: *mut HomeWorld, f: u32) -> u32 {
    hw(p).map(|h| h.label(f.min(255) as u8).len() as u32).unwrap_or(0)
}

/// A people's name into the text buffer; returns its length.
#[no_mangle]
pub extern "C" fn civ_home_name(p: *mut HomeWorld, f: u32) -> u32 {
    let Some(h) = hw(p) else { return 0 };
    h.text.clear();
    if let Some(fac) = h.w.factions.get(f as usize) {
        h.text.push_str(&fac.name);
    }
    h.text.len() as u32
}

#[no_mangle]
pub extern "C" fn civ_home_text_ptr(p: *mut HomeWorld) -> *const u8 {
    hw(p).map(|h| h.text.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_home_line_next(p: *mut HomeWorld) -> u32 {
    hw(p).map(|h| h.next_seq).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_home_line_first(p: *mut HomeWorld) -> u32 {
    hw(p).and_then(|h| h.lines.front().map(|l| l.seq)).unwrap_or(0)
}

fn line(h: &HomeWorld, seq: u32) -> Option<&homeworld::HomeLine> {
    let first = h.lines.front()?.seq;
    h.lines.get(seq.checked_sub(first)? as usize)
}

/// A number of line `seq` (see the module notes); -1e9 when the line is gone.
#[no_mangle]
pub extern "C" fn civ_home_line_meta(p: *mut HomeWorld, seq: u32, k: u32) -> f32 {
    let Some(h) = hw(p) else { return -1e9 };
    let Some(l) = line(h, seq) else { return -1e9 };
    match k {
        0 => l.kind as f32,
        1 => l.tone as f32,
        2 => l.rival as f32,
        3 => l.x as f32,
        4 => l.y as f32,
        5 => l.food,
        6 => l.hurt as f32,
        7 => l.market,
        8 => l.n as f32,
        9 => l.priority as f32,
        10 => l.cooldown as f32,
        _ => -1e9,
    }
}

fn line_str(h: &HomeWorld, seq: u32, k: u32) -> &str {
    match line(h, seq) {
        Some(l) => match k {
            0 => &l.text,
            1 => &l.say,
            2 => &l.key,
            _ => "",
        },
        None => "",
    }
}

#[no_mangle]
pub extern "C" fn civ_home_line_str_ptr(p: *mut HomeWorld, seq: u32, k: u32) -> *const u8 {
    hw(p).map(|h| line_str(h, seq, k).as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_home_line_str_len(p: *mut HomeWorld, seq: u32, k: u32) -> u32 {
    hw(p).map(|h| line_str(h, seq, k).len() as u32).unwrap_or(0)
}
