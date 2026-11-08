//! Pure Rust cosmos and galaxy simulation for Mind over Chicken (#24).
//!
//! Simulates four brain-seeded worlds (calm, focus, pulse, blink) orbiting in a 3D galaxy,
//! and the interplanetary war between space-capable worlds ([`war`], [`endgame`], #166).

pub mod endgame;
pub mod galaxy;
pub mod host;
pub mod math;
pub mod planet_sim;
pub mod war;
#[cfg(feature = "ffi")]
pub mod wasm_api;

pub use galaxy::{Biome, Galaxy, Planet, Ship, MAX_PLANETS, STAR_X, STAR_Y};
pub use planet_sim::{
    arrive_ship, blink_planet, step_planet, BrainEffects, Channel, EffortSplit, EventKind,
    MindSnapshot, PlanetState, SimEvent, StepResult, ERA_COUNT, ERA_SECONDS, ERA_TITLES, POP_CAP,
};
pub use war::{Defence, Economy, Launch, Mind, Mission, Motive, Order, Power, PowerKind, SpacePolicy, SpaceWar, WorldStats};
