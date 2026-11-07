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
//!   is player, warriors
//! - names: slots × `civ_name_len` bytes, ASCII, NUL-padded
//! - villages: `civ_village_count` × `civ_village_stride` f32: x, y, owner, pop, food, unrest,
//!   capital, state (0 dead, 1 alive, 2 under siege)
//! - events: `civ_events_len` × 8 i32: kind, a, b, x, y, n, t_ms, extra (`civ_events_clear`)
//! - stats: `civ_stats_len` f32 (see `World::refresh_info`)

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

/// The player this step: calm, focus, stress (`max(1 − calm, anger)`), anger, all 0..1.
#[no_mangle]
pub extern "C" fn civ_set_inputs(p: *mut World, calm: f32, focus: f32, stress: f32, anger: f32) {
    if let Some(w) = w(p) {
        w.inputs = Inputs { calm, focus, stress, anger };
    }
}

#[no_mangle]
pub extern "C" fn civ_step(p: *mut World, dt: f32) {
    if let Some(w) = w(p) {
        w.step(dt);
    }
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
