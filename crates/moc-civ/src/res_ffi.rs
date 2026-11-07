//! WebAssembly exports for resources, pacts and the land (#165), beside `ffi`.
//!
//! - `civ_res_stat(w, k)`: running counts, by `k`:
//!   - 0 deposits, 1 held, 2 run dry;
//!   - 3 resource wars declared, 4 seized in war, 5 ceded at peace, 6 trades;
//!   - 7 pacts sworn, 8 betrayals, 9 lapses, 10 joined an ally's war;
//!   - 11 tiles improved, 12 tiles worn, 13 forest stripped, 14 grass turned to dust,
//!     15 irrigated, 16 terraced, 17 replanted, 18 reclaimed;
//!   - 19 milliseconds in the resource and land ticks (native only), 20 pacts standing,
//!     21 war goals standing.
//! - `civ_res_deposit(w, i, field)`, one deposit at a time, by `field`:
//!   - 0 x, 1 y, 2 kind (`resources::Res`), 3 share left;
//!   - 4 holder (-1 none), 5 fought over (1/0), 6 run dry (1/0).
//! - `civ_res_access(w, f, kind)`: 0 none, 1 traded, 2 held.
//! - `civ_res_pact(w, a, b)`: 1 allied.
//! - `civ_res_fert(w, x, y)`: a tile's fertility (128 natural).

use crate::world::*;

fn w<'a>(p: *mut World) -> Option<&'a mut World> {
    // SAFETY: JS only passes handles returned by civ_new and not yet freed.
    unsafe { p.as_mut() }
}

#[no_mangle]
pub extern "C" fn civ_res_stat(p: *mut World, k: u32) -> f32 {
    let Some(w) = w(p) else { return 0.0 };
    let (r, pa, l) = (&w.res.stats, &w.pacts.stats, &w.land.stats);
    match k {
        0 => w.res.deps.len() as f32,
        1 => w.res.deps.iter().filter(|d| d.owner != NO_F).count() as f32,
        2 => w.res.deps.iter().filter(|d| d.spent).count() as f32,
        3 => r.claims as f32,
        4 => r.seized as f32,
        5 => r.ceded as f32,
        6 => r.trades as f32,
        7 => pa.formed as f32,
        8 => pa.betrayed as f32,
        9 => pa.lapsed as f32,
        10 => pa.joined as f32,
        11 => l.improved as f32,
        12 => l.worn as f32,
        13 => l.stripped as f32,
        14 => l.eroded as f32,
        15 => l.irrigated as f32,
        16 => l.terraced as f32,
        17 => l.reforested as f32,
        18 => l.reclaimed as f32,
        19 => w.res.prof_ms as f32,
        20 => (w.pacts.on.iter().filter(|&&o| o != 0).count() / 2) as f32,
        21 => w.res.goal.iter().filter(|&&g| g != crate::resources::NO_DEP).count() as f32,
        _ => 0.0,
    }
}

#[no_mangle]
pub extern "C" fn civ_res_deposit(p: *mut World, i: u32, field: u32) -> f32 {
    let Some(w) = w(p) else { return 0.0 };
    let Some(d) = w.res.deps.get(i as usize) else { return 0.0 };
    match field {
        0 => d.x as f32,
        1 => d.y as f32,
        2 => d.kind as u8 as f32,
        3 => d.share(),
        4 => {
            if d.owner == NO_F {
                -1.0
            } else {
                d.owner as f32
            }
        }
        5 => (w.time - d.fought_t < 3.0) as u8 as f32,
        6 => d.spent as u8 as f32,
        _ => 0.0,
    }
}

#[no_mangle]
pub extern "C" fn civ_res_access(p: *mut World, f: u32, kind: u32) -> u32 {
    w(p).and_then(|w| w.res.access.get(f as usize).and_then(|a| a.get(kind as usize)).copied()).unwrap_or(0) as u32
}

#[no_mangle]
pub extern "C" fn civ_res_pact(p: *mut World, a: u32, b: u32) -> u32 {
    w(p).map(|w| (a < MAX_FACTIONS as u32 && b < MAX_FACTIONS as u32 && crate::pacts::allied(w, a as u8, b as u8)) as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn civ_res_fert(p: *mut World, x: i32, y: i32) -> u32 {
    w(p).map(|w| w.food.fert[w.d.idx(x, y)] as u32).unwrap_or(0)
}
