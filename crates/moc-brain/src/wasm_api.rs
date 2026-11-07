//! WebAssembly and extern "C" bindings for browser runtime interoperability.

use crate::brain::Brain;
use crate::calibration::{
    random_seed_traits, traits_from_features, SensorFeatures,
};
use crate::model::{Bands, Class};
use crate::sim_source::{BrainFrame, SimSource};

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Plain extern "C" WebAssembly API (works without wasm-bindgen if needed)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn moc_brain_new() -> *mut Brain {
    Box::into_raw(Box::new(Brain::new()))
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_free(ptr: *mut Brain) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_set_cue(ptr: *mut Brain, cue_id: i32) {
    if let Some(brain) = ptr.as_mut() {
        let cue = match cue_id {
            0 => Some(Class::Rest),
            1 => Some(Class::Calm),
            2 => Some(Class::Focus),
            _ => None,
        };
        brain.set_cue(cue);
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_update(ptr: *mut Brain, dt: f32) -> u32 {
    if let Some(brain) = ptr.as_mut() {
        if brain.update(dt) {
            1
        } else {
            0
        }
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_ingest(
    ptr: *mut Brain,
    delta: f32,
    theta: f32,
    alpha: f32,
    beta: f32,
    gamma: f32,
    blink: u32,
    hr: f32,
    tilt_x: f32,
    tilt_y: f32,
    motion: f32,
    quality: f32,
) -> u32 {
    if let Some(brain) = ptr.as_mut() {
        let frame = BrainFrame {
            bands: Bands {
                delta,
                theta,
                alpha,
                beta,
                gamma,
            },
            blink: blink != 0,
            hr: if hr > 0.0 { Some(hr) } else { None },
            tilt: Some((tilt_x, tilt_y)),
            motion,
            quality,
        };
        if brain.ingest(&frame) {
            1
        } else {
            0
        }
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_get_calm(ptr: *const Brain) -> f32 {
    ptr.as_ref().map(|b| b.calm).unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_get_focus(ptr: *const Brain) -> f32 {
    ptr.as_ref().map(|b| b.focus).unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_get_hr_delta(ptr: *const Brain) -> f32 {
    ptr.as_ref().map(|b| b.hr_delta()).unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_get_stress(ptr: *const Brain) -> f32 {
    ptr.as_ref().map(|b| b.stress).unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_get_agitation(ptr: *const Brain) -> f32 {
    ptr.as_ref().map(|b| b.agitation).unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_get_model_accuracy(ptr: *const Brain) -> f32 {
    ptr.as_ref().map(|b| b.model.accuracy).unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn moc_brain_get_model_trust(ptr: *const Brain, class_id: i32) -> f32 {
    if let Some(b) = ptr.as_ref() {
        match class_id {
            0 => b.model.trust(Class::Rest),
            1 => b.model.trust(Class::Calm),
            2 => b.model.trust(Class::Focus),
            _ => 0.0,
        }
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// SimSource C-API
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn moc_sim_new(seed: u32) -> *mut SimSource {
    Box::into_raw(Box::new(SimSource::new(seed)))
}

#[no_mangle]
pub unsafe extern "C" fn moc_sim_free(ptr: *mut SimSource) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_sim_set_input(
    ptr: *mut SimSource,
    calm: u32,
    focus: u32,
    panic: u32,
) {
    if let Some(sim) = ptr.as_mut() {
        sim.input.calm = calm != 0;
        sim.input.focus = focus != 0;
        sim.input.panic = panic != 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_sim_blink(ptr: *mut SimSource) {
    if let Some(sim) = ptr.as_mut() {
        sim.blink();
    }
}

// ---------------------------------------------------------------------------
// Calibration C-API
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn moc_calibration_traits_random(
    seed: u32,
    out_traits: *mut f32, // [friendliness, aggressiveness, violence]
) {
    if out_traits.is_null() {
        return;
    }
    let t = random_seed_traits(seed);
    unsafe {
        *out_traits.add(0) = t.friendliness;
        *out_traits.add(1) = t.aggressiveness;
        *out_traits.add(2) = t.violence;
    }
}

#[no_mangle]
pub extern "C" fn moc_calibration_traits_from_sensors(
    calm: f32,
    focus: f32,
    stress: f32,
    blink_rate: f32,
    joy: f32,
    anger: f32,
    out_traits: *mut f32, // [friendliness, aggressiveness, violence]
) {
    if out_traits.is_null() {
        return;
    }
    let features = SensorFeatures {
        calm,
        focus,
        stress,
        blink_rate,
        joy,
        anger,
        surprise: 0.0,
        sadness: 0.0,
        face_seen: joy > 0.0 || anger > 0.0,
    };
    let t = traits_from_features(&features);
    unsafe {
        *out_traits.add(0) = t.friendliness;
        *out_traits.add(1) = t.aggressiveness;
        *out_traits.add(2) = t.violence;
    }
}
