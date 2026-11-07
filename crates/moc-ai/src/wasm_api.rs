//! WebAssembly and extern "C" bindings for the MLP Policy AI God.

use crate::policy::{MLPPolicy, NUM_ACTIONS, NUM_INPUTS};

#[no_mangle]
pub extern "C" fn moc_ai_policy_new() -> *mut MLPPolicy {
    Box::into_raw(Box::new(MLPPolicy::from_embedded()))
}

#[no_mangle]
pub unsafe extern "C" fn moc_ai_policy_free(ptr: *mut MLPPolicy) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_ai_policy_act(
    ptr: *const MLPPolicy,
    obs_ptr: *const f32,
    greedy: u32,
    sample_val: f32,
) -> u32 {
    let policy = match ptr.as_ref() {
        Some(p) => p,
        None => return 0,
    };
    if obs_ptr.is_null() {
        return 0;
    }

    let obs_slice = std::slice::from_raw_parts(obs_ptr, NUM_INPUTS);
    policy.act(obs_slice, greedy != 0, sample_val) as u32
}

#[no_mangle]
pub unsafe extern "C" fn moc_ai_policy_probs(
    ptr: *const MLPPolicy,
    obs_ptr: *const f32,
    out_probs: *mut f32,
) {
    let policy = match ptr.as_ref() {
        Some(p) => p,
        None => return,
    };
    if obs_ptr.is_null() || out_probs.is_null() {
        return;
    }

    let obs_slice = std::slice::from_raw_parts(obs_ptr, NUM_INPUTS);
    let probs = policy.probs(obs_slice);
    for i in 0..NUM_ACTIONS {
        *out_probs.add(i) = probs[i];
    }
}

#[no_mangle]
pub unsafe extern "C" fn moc_ai_policy_self_check(ptr: *const MLPPolicy) -> f32 {
    ptr.as_ref().map(|p| p.self_check()).unwrap_or(-1.0)
}
