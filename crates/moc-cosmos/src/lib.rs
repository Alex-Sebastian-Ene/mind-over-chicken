//! Pure Rust cosmos and galaxy simulation for Mind over Chicken (#24).
//!
//! Simulates four brain-seeded worlds (calm, focus, pulse, blink) orbiting in a 3D galaxy.

pub mod galaxy;
pub mod math;
pub mod planet_sim;
pub mod wasm_api;

pub use galaxy::{Biome, Galaxy, Planet, Ship, MAX_PLANETS, STAR_X, STAR_Y};
pub use planet_sim::{
    arrive_ship, blink_planet, step_planet, BrainEffects, Channel, EffortSplit, EventKind,
    MindSnapshot, PlanetState, SimEvent, StepResult, ERA_COUNT, ERA_SECONDS, ERA_TITLES, POP_CAP,
};
