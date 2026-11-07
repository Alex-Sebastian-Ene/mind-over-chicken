//! Pure-Rust AI God and PPO actor runtime for Mind over Chicken (#25, #37, #43).
//!
//! Evaluates the 64x64 MLP policy exported from Stable-Baselines3 in release WebAssembly
//! or native environments with sub-microsecond inference latency.

pub mod observe;
pub mod policy;
pub mod wasm_api;

pub use observe::{StoneAgeRawState, clamp};
pub use policy::{
    MLPPolicy, PolicySpec, ACTION_NAMES, NUM_ACTIONS, NUM_INPUTS, OBS_NAMES,
};
