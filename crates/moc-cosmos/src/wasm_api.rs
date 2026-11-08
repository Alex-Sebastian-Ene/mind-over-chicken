//! WebAssembly and extern "C" bindings for the Galaxy & Cosmos simulator.

use crate::galaxy::Galaxy;
use crate::planet_sim::BrainEffects;

#[no_mangle]
pub extern "C" fn moc_galaxy_new(seed: u32) -> *mut Galaxy {
    Box::into_raw(Box::new(Galaxy::new(seed)))
}

#[no_mangle]
pub unsafe extern "C" fn moc_galaxy_free(ptr: *mut Galaxy) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_galaxy_step(
    ptr: *mut Galaxy,
    dt: f32,
    calm: f32,
    focus: f32,
    stress: f32,
    blinks_last_10s: u32,
    blinks_total: u32,
) {
    if let Some(galaxy) = ptr.as_mut() {
        let fx = BrainEffects::from_signals(calm, focus, stress, blinks_last_10s);
        galaxy.step(dt, &fx, blinks_total);
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_galaxy_blink(ptr: *mut Galaxy) {
    if let Some(galaxy) = ptr.as_mut() {
        galaxy.on_blink();
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_galaxy_set_attended(ptr: *mut Galaxy, planet_idx: i32) {
    if let Some(galaxy) = ptr.as_mut() {
        galaxy.attended = if planet_idx >= 0 {
            Some(planet_idx as usize)
        } else {
            None
        };
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_galaxy_planet_count(ptr: *const Galaxy) -> u32 {
    ptr.as_ref().map(|g| g.planets.len() as u32).unwrap_or(0)
}

/// Query planet metrics into a float buffer:
/// [pop, era_index, progress, market, storms, alive (1.0/0.0), forming (1.0/0.0), posX, posY]
#[no_mangle]
pub unsafe extern "C" fn moc_galaxy_planet_metrics(
    ptr: *const Galaxy,
    idx: u32,
    out: *mut f32,
) -> u32 {
    if out.is_null() {
        return 0;
    }
    let galaxy = match ptr.as_ref() {
        Some(g) => g,
        None => return 0,
    };

    let m = match galaxy.planet_metrics(idx as usize) {
        Some(m) => m,
        None => return 0,
    };

    for (i, v) in m.iter().enumerate() {
        *out.add(i) = *v;
    }

    1
}
