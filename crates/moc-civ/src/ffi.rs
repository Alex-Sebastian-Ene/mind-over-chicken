//! Plain `extern "C"` WebAssembly interface (no wasm-bindgen). JS keeps the `*mut World`
//! handle as a number, calls these functions, and reads buffers through typed-array views on
//! `memory.buffer` (re-created after any call that may grow memory: `civ_new`, `civ_step`,
//! `civ_render_view`, the casts).
//!
//! Buffers:
//! - view: VW×VH RGBA8 (`civ_view_ptr`, `civ_view_len`); overview map (`civ_minimap_ptr`)
//! - factions: `civ_faction_slots` × `civ_faction_stride` f32 (see `World::refresh_info`):
//!   alive(1)/dead(-1)/unused(0), pop, villages, r, g, b, capital x, capital y, friendliness,
//!   aggressiveness, violence, fertility, cohesion, parent (-1 none), wars, raids, food, kills,
//!   losses, mean unrest, seed source (0 run, 1 calibration, 2 mind, 3 rebellion), region,
//!   is player, warriors; then (#69) age, seconds in it, stability, knowledge, wealth, faith,
//!   production, links, territory, inequality, vassals, colonies, Dark Age seconds left,
//!   overlord (-1 none), strife level inside it, conflict level, highest age reached, food days,
//!   coastal, contact, collapses, seconds the next age's conditions have held
//! - names: slots × `civ_name_len` bytes, ASCII, NUL-padded
//! - villages: `civ_village_count` × `civ_village_stride` f32: x, y, owner, pop, food, unrest,
//!   capital, state (0 dead, 1 alive, 2 under siege), buildings, plague, strife side (0 none,
//!   1 loyal, 2 rebel), mood (1 festival, -1 mourning)
//! - text: `civ_text_ptr(kind, i)` / `civ_text_len(kind, i)`, UTF-8 (see `civ_text_ptr`)
//! - events: `civ_events_len` × 8 i32: kind, a, b, x, y, n, t_ms, extra (`civ_events_clear`)
//! - stats: `civ_stats_len` f32 (see `World::refresh_info`)
//! - space (#166): `civ_space_info_ptr`, slots × `civ_space_stride` f32 (see `space::INFO_STRIDE`);
//!   launches `civ_space_launches_ptr` × `space::LAUNCH_STRIDE` (`civ_space_launches_clear`)

use crate::powers;
use crate::world::*;

fn w<'a>(p: *mut World) -> Option<&'a mut World> {
    // SAFETY: JS only passes handles returned by civ_new and not yet freed.
    unsafe { p.as_mut() }
}

/// Run a change, then refresh the buffers JS reads so it sees the change at once.
fn refreshed<T>(w: &mut World, f: impl FnOnce(&mut World) -> T) -> T {
    let r = f(w);
    w.refresh_info();
    r
}

#[no_mangle]
pub extern "C" fn civ_new(seed: u32) -> *mut World {
    Box::into_raw(Box::new(World::new(seed)))
}

/// A run on a planet of `w`×`h` tiles (0 for the default 1280×640).
#[no_mangle]
pub extern "C" fn civ_new_sized(seed: u32, w: u32, h: u32) -> *mut World {
    let d = if w == 0 || h == 0 { crate::planet::Dims::DEFAULT } else { crate::planet::Dims::new(w as usize, h as usize) };
    Box::into_raw(Box::new(World::new_sized(seed, d)))
}

#[no_mangle]
pub extern "C" fn civ_planet_w(p: *mut World) -> u32 {
    w(p).map(|w| w.d.w as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_planet_h(p: *mut World) -> u32 {
    w(p).map(|w| w.d.h as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_free(p: *mut World) {
    if !p.is_null() {
        // SAFETY: p came from Box::into_raw in civ_new.
        unsafe { drop(Box::from_raw(p)) };
    }
}

/// The player this step, all 0..1: calm, focus, stress (0 at rest: the brain's `stress`, or
/// the sandbox's fallback from calm and anger) and anger.
#[no_mangle]
pub extern "C" fn civ_set_inputs(p: *mut World, calm: f32, focus: f32, stress: f32, anger: f32) {
    if let Some(w) = w(p) {
        w.inputs = Inputs { calm, focus, stress, anger, joy: w.inputs.joy };
    }
}

/// A smile on camera, 0..1 (festivals).
#[no_mangle]
pub extern "C" fn civ_set_joy(p: *mut World, joy: f32) {
    if let Some(w) = w(p) {
        w.inputs.joy = joy;
    }
}

#[no_mangle]
pub extern "C" fn civ_step(p: *mut World, dt: f32) {
    if let Some(w) = w(p) {
        w.step(dt);
    }
}

/// Run `dt` seconds of real time at the world's speed, in steps of at most 0.1 s.
#[no_mangle]
pub extern "C" fn civ_advance(p: *mut World, dt: f32) {
    if let Some(w) = w(p) {
        w.advance(dt);
    }
}

/// Time speed: 1, 4 or 16 in the sandbox (clamped to 0.25..32).
#[no_mangle]
pub extern "C" fn civ_set_speed(p: *mut World, k: f32) {
    if let Some(w) = w(p) {
        w.set_speed(k);
    }
}

#[no_mangle]
pub extern "C" fn civ_speed(p: *mut World) -> f32 {
    w(p).map(|w| w.speed).unwrap_or(1.0)
}

#[no_mangle]
pub extern "C" fn civ_age_count() -> usize {
    crate::ages_table::AGE_COUNT
}

fn text(kind: u32, i: u32) -> &'static str {
    use crate::ages_table::{Building, Job, AGES, CRISES};
    let i = i as usize;
    match kind {
        0 => AGES.get(i.wrapping_sub(1)).map(|a| a.name).unwrap_or(""),
        1 => AGES.get(i.wrapping_sub(1)).map(|a| a.key).unwrap_or(""),
        2 => CRISES.get(i).copied().unwrap_or(""),
        3 => crate::conflict::StrifeKind::NAMES.get(i).copied().unwrap_or(""),
        4 => Job::ALL.get(i).map(|j| j.key()).unwrap_or(""),
        5 => Building::ALL.get(i).map(|b| b.key()).unwrap_or(""),
        6 => AGES.get(i.wrapping_sub(1)).map(|a| a.weapon.name).unwrap_or(""),
        7 => crate::conflict::LEVELS.get(i).copied().unwrap_or(""),
        8 => AGES.get(i.wrapping_sub(1)).map(|a| a.brain).unwrap_or(""),
        _ => "",
    }
}

/// Static text: kind 0 age name (i = 1..20), 1 age key, 2 crisis key (event `extra`), 3 strife
/// kind, 4 job key, 5 building key, 6 the age's weapon, 7 conflict level name, 8 the age's
/// brain moment.
#[no_mangle]
pub extern "C" fn civ_text_ptr(kind: u32, i: u32) -> *const u8 {
    text(kind, i).as_ptr()
}

#[no_mangle]
pub extern "C" fn civ_text_len(kind: u32, i: u32) -> usize {
    text(kind, i).len()
}

#[no_mangle]
pub extern "C" fn civ_render_view(p: *mut World) {
    if let Some(w) = w(p) {
        w.render();
    }
}

/// Render the whole planet into the overview map (`civ_minimap_w`×`civ_minimap_h` RGBA).
#[no_mangle]
pub extern "C" fn civ_render_minimap(p: *mut World) {
    if let Some(w) = w(p) {
        w.render_minimap();
    }
}

#[no_mangle]
pub extern "C" fn civ_minimap_ptr(p: *mut World) -> *const u8 {
    w(p).map(|w| w.minimap.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_minimap_w() -> usize {
    crate::render::MINI_W
}

#[no_mangle]
pub extern "C" fn civ_minimap_h() -> usize {
    crate::render::MINI_H
}

#[no_mangle]
pub extern "C" fn civ_view_ptr(p: *mut World) -> *const u8 {
    w(p).map(|w| w.view.data.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_view_len(p: *mut World) -> usize {
    w(p).map(|w| w.view.data.len()).unwrap_or(0)
}

/// Lightning at a planet point. Returns how many it killed.
#[no_mangle]
pub extern "C" fn civ_lightning(p: *mut World, x: f32, y: f32) -> i32 {
    w(p).map(|w| refreshed(w, |w| powers::lightning(w, x, y))).unwrap_or(0)
}

/// A cast at a planet point: 1 fireball, 2 food, 3 peace, 4 disaster, 5 bless, 6 rain.
#[no_mangle]
pub extern "C" fn civ_cast(p: *mut World, kind: u32, x: f32, y: f32) -> i32 {
    w(p).map(|w| refreshed(w, |w| powers::cast(w, kind, x, y))).unwrap_or(0)
}

/// Calibration hook (#54): the player's civilisation from friendliness, aggressiveness, violence.
#[no_mangle]
pub extern "C" fn civ_set_player_traits(p: *mut World, friendliness: f32, aggressiveness: f32, violence: f32) -> i32 {
    w(p).map(|w| refreshed(w, |w| powers::set_player_traits(w, friendliness, aggressiveness, violence) as i32)).unwrap_or(0)
}

/// The mind read: mean calm, focus and stress, and the blink count, from the first 10 s.
#[no_mangle]
pub extern "C" fn civ_seed_player_mind(p: *mut World, calm: f32, focus: f32, stress: f32, blinks: f32) -> i32 {
    w(p).map(|w| refreshed(w, |w| powers::seed_player_mind(w, calm, focus, stress, blinks) as i32)).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_cam_pan(p: *mut World, dx: f32, dy: f32) {
    if let Some(w) = w(p) {
        w.pan_camera(dx, dy);
    }
}

#[no_mangle]
pub extern "C" fn civ_cam_centre(p: *mut World, x: f32, y: f32) {
    if let Some(w) = w(p) {
        w.centre_camera(x, y);
    }
}

#[no_mangle]
pub extern "C" fn civ_cam_x(p: *mut World) -> f32 {
    w(p).map(|w| w.cam.x).unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn civ_cam_y(p: *mut World) -> f32 {
    w(p).map(|w| w.cam.y).unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn civ_faction_slots() -> usize {
    MAX_FACTIONS
}

#[no_mangle]
pub extern "C" fn civ_faction_stride() -> usize {
    FSTRIDE
}

#[no_mangle]
pub extern "C" fn civ_factions_ptr(p: *mut World) -> *const f32 {
    w(p).map(|w| w.info_f.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_name_len() -> usize {
    NAME_LEN
}

#[no_mangle]
pub extern "C" fn civ_names_ptr(p: *mut World) -> *const u8 {
    w(p).map(|w| w.names.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_faction_seed(p: *mut World, f: u32) -> u32 {
    w(p).and_then(|w| w.factions.get(f as usize).map(|f| f.seed)).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_faction_start_x(p: *mut World, f: u32) -> i32 {
    w(p).and_then(|w| w.factions.get(f as usize).map(|f| f.start.0)).unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn civ_faction_start_y(p: *mut World, f: u32) -> i32 {
    w(p).and_then(|w| w.factions.get(f as usize).map(|f| f.start.1)).unwrap_or(-1)
}

/// 0 peace, 1 a raids b, 2 war.
#[no_mangle]
pub extern "C" fn civ_war_state(p: *mut World, a: u32, b: u32) -> i32 {
    w(p).filter(|_| (a as usize) < MAX_FACTIONS && (b as usize) < MAX_FACTIONS).map(|w| w.rel(a as u8, b as u8).war as i32).unwrap_or(0)
}

/// a's tension toward b (war at 1.0).
#[no_mangle]
pub extern "C" fn civ_tension(p: *mut World, a: u32, b: u32) -> f32 {
    w(p).filter(|_| (a as usize) < MAX_FACTIONS && (b as usize) < MAX_FACTIONS).map(|w| w.rel(a as u8, b as u8).tension).unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn civ_village_stride() -> usize {
    VSTRIDE
}

#[no_mangle]
pub extern "C" fn civ_village_count(p: *mut World) -> usize {
    w(p).map(|w| w.villages.len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_villages_ptr(p: *mut World) -> *const f32 {
    w(p).map(|w| w.info_v.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_event_stride() -> usize {
    8
}

#[no_mangle]
pub extern "C" fn civ_events_len(p: *mut World) -> usize {
    w(p).map(|w| w.events.len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_events_ptr(p: *mut World) -> *const Event {
    w(p).map(|w| w.events.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_events_clear(p: *mut World) {
    if let Some(w) = w(p) {
        w.events.clear();
    }
}

#[no_mangle]
pub extern "C" fn civ_stats_len() -> usize {
    STATS_LEN
}

#[no_mangle]
pub extern "C" fn civ_stats_ptr(p: *mut World) -> *const f32 {
    w(p).map(|w| w.stats_buf.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_agent_count(p: *mut World) -> usize {
    w(p).map(|w| w.agents.len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_owner_at(p: *mut World, x: f32, y: f32) -> i32 {
    w(p).map(|w| w.owner_at(x, y)).map(|o| if o == NO_F { -1 } else { o as i32 }).unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn civ_time(p: *mut World) -> f32 {
    w(p).map(|w| w.time).unwrap_or(0.0)
}

// ---- Words for the sandbox (#69): Rust writes them, JS draws them. ----

/// ×1 → ×4 → ×16 → ×1; returns the new speed.
#[no_mangle]
pub extern "C" fn civ_cycle_speed(p: *mut World) -> f32 {
    w(p).map(|w| w.cycle_speed()).unwrap_or(1.0)
}

/// The number the next news line will get (lines `civ_news_first`..this exist).
#[no_mangle]
pub extern "C" fn civ_news_next(p: *mut World) -> u32 {
    w(p).map(|w| w.news.next).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_news_first(p: *mut World) -> u32 {
    w(p).and_then(|w| w.news.lines.front().map(|l| l.seq)).unwrap_or(0)
}

/// A news line's number fields: 0 event kind, 1 tone (0 info, 1 good, 2 bad, 3 warning), 2 in
/// the log (1/0), 3 advisor priority, 4 advisor cooldown (s), 5 x, 6 y. -1 if gone.
#[no_mangle]
pub extern "C" fn civ_news_meta(p: *mut World, seq: u32, field: u32) -> i32 {
    let Some(l) = w(p).and_then(|w| w.news.get(seq)) else { return -1 };
    match field {
        0 => l.kind,
        1 => l.tone as i32,
        2 => l.log as i32,
        3 => l.priority as i32,
        4 => l.cooldown as i32,
        5 => l.x,
        6 => l.y,
        _ => -1,
    }
}

fn news_str<'a>(w: &'a World, seq: u32, which: u32) -> &'a str {
    match w.news.get(seq) {
        Some(l) => match which {
            0 => &l.text,
            1 => &l.log_text,
            2 => &l.say,
            3 => &l.key,
            _ => "",
        },
        None => "",
    }
}

/// A news line's text: 0 headline, 1 log line, 2 advisor line, 3 advisor key (UTF-8).
#[no_mangle]
pub extern "C" fn civ_news_str_ptr(p: *mut World, seq: u32, which: u32) -> *const u8 {
    w(p).map(|w| news_str(w, seq, which).as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_news_str_len(p: *mut World, seq: u32, which: u32) -> usize {
    w(p).map(|w| news_str(w, seq, which).len()).unwrap_or(0)
}

/// Write the HUD rows (a line per people: label, value, bad, tab-separated, with its age)
/// and return their
/// length; read them at `civ_text_buf_ptr`.
#[no_mangle]
pub extern "C" fn civ_hud_build(p: *mut World) -> usize {
    w(p).map(|w| {
        w.text_buf = w.hud_rows();
        w.text_buf.len()
    })
    .unwrap_or(0)
}

/// Write the label over civilisation `f`'s capital and return its length.
#[no_mangle]
pub extern "C" fn civ_label_build(p: *mut World, f: u32) -> usize {
    w(p).filter(|_| (f as usize) < MAX_FACTIONS)
        .map(|w| {
            w.text_buf = w.capital_label(f as u8);
            w.text_buf.len()
        })
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_text_buf_ptr(p: *mut World) -> *const u8 {
    w(p).map(|w| w.text_buf.as_ptr()).unwrap_or(std::ptr::null())
}

/// How many steps the last `civ_advance` took.
#[no_mangle]
pub extern "C" fn civ_last_steps(p: *mut World) -> u32 {
    w(p).map(|w| w.last_steps).unwrap_or(0)
}

// ---- Staggered, player-initialised civilisations (#98) ----

/// A staged run: the planet with four civilisations waiting to be founded one by one (0 for the
/// default size).
#[no_mangle]
pub extern "C" fn civ_new_staged(seed: u32, w: u32, h: u32) -> *mut World {
    let d = if w == 0 || h == 0 { crate::planet::Dims::DEFAULT } else { crate::planet::Dims::new(w as usize, h as usize) };
    Box::into_raw(Box::new(World::new_staged(seed, d)))
}

/// 0 off, 1 choose, 2 place, 3 seed, 4 done.
#[no_mangle]
pub extern "C" fn civ_setup_phase(p: *mut World) -> u32 {
    w(p).map(|w| w.setup.phase as u32).unwrap_or(0)
}

/// The civilisation being set up (-1 none).
#[no_mangle]
pub extern "C" fn civ_setup_slot(p: *mut World) -> i32 {
    w(p).map(|w| if w.setup.slot == NO_F { -1 } else { w.setup.slot as i32 }).unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn civ_setup_select(p: *mut World, f: u32) -> i32 {
    w(p).map(|w| refreshed(w, |w| w.setup_select(f.min(255) as u8) as i32)).unwrap_or(0)
}

/// Tab: the next waiting civilisation.
#[no_mangle]
pub extern "C" fn civ_setup_next(p: *mut World) {
    if let Some(w) = w(p) {
        w.setup_next();
    }
}

/// Where the player aims while placing (planet coordinates); 1 if a civilisation may begin there.
#[no_mangle]
pub extern "C" fn civ_setup_hover(p: *mut World, x: f32, y: f32) -> i32 {
    w(p).map(|w| w.setup_hover(x, y) as i32).unwrap_or(0)
}

/// A click or blink on the planet during setup; returns the phase after it.
#[no_mangle]
pub extern "C" fn civ_setup_click(p: *mut World, x: f32, y: f32) -> u32 {
    w(p).map(|w| refreshed(w, |w| w.setup_click(x, y) as u32)).unwrap_or(0)
}

/// Enter during setup; returns the phase after it.
#[no_mangle]
pub extern "C" fn civ_setup_enter(p: *mut World) -> u32 {
    w(p).map(|w| refreshed(w, |w| w.setup_enter() as u32)).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_setup_blink(p: *mut World) {
    if let Some(w) = w(p) {
        w.setup_blink();
    }
}

/// Seeding progress 0..1.
#[no_mangle]
pub extern "C" fn civ_setup_progress(p: *mut World) -> f32 {
    w(p).map(|w| w.setup_progress()).unwrap_or(0.0)
}

/// Write the setup's words (what to do, then a line per civilisation) and return their length;
/// read them at `civ_text_buf_ptr`.
#[no_mangle]
pub extern "C" fn civ_setup_text_build(p: *mut World) -> usize {
    w(p).map(|w| {
        w.text_buf = w.setup_text();
        w.text_buf.len()
    })
    .unwrap_or(0)
}

/// Found civilisation `f` at (x, y) with these traits directly (tools and tests; the sandbox
/// goes through the seeding window).
#[no_mangle]
pub extern "C" fn civ_found(p: *mut World, f: u32, x: i32, y: i32, friendliness: f32, aggressiveness: f32, violence: f32, fertility: f32, cohesion: f32) -> i32 {
    w(p).map(|w| refreshed(w, |w| w.found_civ(f.min(255) as u8, x, y, Traits { friendliness, aggressiveness, violence, fertility, cohesion }) as i32)).unwrap_or(0)
}

/// A staged run with `players` civilisations for the player to found and `rivals` arriving on
/// their own (#101); 0×0 for the default planet size.
#[no_mangle]
pub extern "C" fn civ_new_staged_with(seed: u32, w: u32, h: u32, players: u32, rivals: u32) -> *mut World {
    let d = if w == 0 || h == 0 { crate::planet::Dims::DEFAULT } else { crate::planet::Dims::new(w as usize, h as usize) };
    Box::into_raw(Box::new(World::new_staged_with(seed, d, players as usize, rivals as usize)))
}

// ---- the space age (#166): `space.rs`. The galaxy's war is the host's (moc-cosmos, through
// RustGalaxy); these move its targets in and the civilisations' launches and losses out. ----

/// Floats per civilisation in the space buffer (`space::INFO_STRIDE`; slots × stride).
#[no_mangle]
pub extern "C" fn civ_space_stride() -> usize {
    crate::space::INFO_STRIDE
}

#[no_mangle]
pub extern "C" fn civ_space_info_ptr(p: *mut World) -> *const f32 {
    w(p).map(|w| w.space.info.as_ptr()).unwrap_or(std::ptr::null())
}

/// Launches since the last clear, `space::LAUNCH_STRIDE` floats each: civilisation, planet,
/// order (0 raid, 1 bombard, 2 invade), strength, motive.
#[no_mangle]
pub extern "C" fn civ_space_launch_count(p: *mut World) -> usize {
    w(p).map(|w| w.space.launches.len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_space_launches_ptr(p: *mut World) -> *const f32 {
    w(p).map(|w| w.space.out.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn civ_space_launches_clear(p: *mut World) {
    if let Some(w) = w(p) {
        w.space.launches.clear();
        w.space.out.clear();
    }
}

/// Forget civilisation `f`'s targets (then add the galaxy's with `civ_space_target_add`).
#[no_mangle]
pub extern "C" fn civ_space_targets_clear(p: *mut World, f: u32) {
    if let Some(t) = w(p).and_then(|w| w.space.targets.get_mut(f as usize)) {
        t.clear();
    }
}

/// A planet civilisation `f` may strike: its index in the galaxy, shields + defences, garrison,
/// value, and who rules it (0 nobody, 1 a world, 2 a civilisation, 3 the player).
#[no_mangle]
pub extern "C" fn civ_space_target_add(p: *mut World, f: u32, body: u32, orbit: f32, garrison: f32, value: f32, kind: u32) {
    if let Some(t) = w(p).and_then(|w| w.space.targets.get_mut(f as usize)) {
        if t.len() < crate::space::MAX_TARGETS {
            t.push(crate::space::Target { body: body.min(u16::MAX as u32) as u16, orbit, garrison, value, kind: kind.min(255) as u8 });
        }
    }
}

/// What the galaxy did to civilisation `f` (see `space::SpaceResult`).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn civ_space_result(p: *mut World, f: u32, loot: f32, knowledge: f32, hits: f32, shield_loss: f32, colonies: i32, returned: f32, colony_econ: f32) {
    if let Some(w) = w(p) {
        let r = crate::space::SpaceResult { loot, knowledge, hits, shield_loss, colonies, returned, colony_econ };
        crate::space::apply_result(w, f as usize, &r);
    }
}

/// Test and `?debug` hook: put civilisation `f` in space whatever its age (on = 0 undoes it).
/// Returns 1 if it is alive.
#[no_mangle]
pub extern "C" fn civ_debug_space(p: *mut World, f: u32, on: u32) -> i32 {
    let Some(w) = w(p) else { return 0 };
    let Some(prog) = w.space.programs.get_mut(f as usize) else { return 0 };
    prog.forced = on != 0;
    w.factions.get(f as usize).is_some_and(|x| x.alive) as i32
}
