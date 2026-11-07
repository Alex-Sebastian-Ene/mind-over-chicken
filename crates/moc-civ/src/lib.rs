//! Warring civilisations for Mind over Chicken (#50, also #52).
//!
//! Four civilisations start on the four continents of a wrapping planet ([`moc_planet`]), with
//! one village and four agents each. Villages turn food surplus into new agents; agents gather,
//! settle new villages, claim land, starve, age and die. Civilisations that meet trade or
//! raid; when tension boils over they go to war, and warriors fight in pixel skirmishes and
//! capture villages. Unrest from crowding, hunger, distance from the capital and war losses
//! (plus the player's anger and stress) splits villages off as new civilisations, which
//! usually fight their parent.
//!
//! The player is a god, not a general: calm, focus, stress and anger tilt every civilisation
//! each step, and lightning and casts act on the world. One civilisation's traits can be set
//! from a calibration payload (#54) or from the player's first ten seconds of brain data.
//!
//! Everything is deterministic for a run seed and a sequence of steps and inputs.
//!
//! The browser loads this crate as a plain `extern "C"` WebAssembly module ([`ffi`]); build
//! it with `crates/moc-civ/build-wasm.sh` (no wasm-bindgen needed).

pub mod agents;
pub mod ffi;
pub mod planet;
pub mod politics;
pub mod powers;
pub mod render;
pub mod terrain;
pub mod world;

pub use planet::{Cam, Dims};
pub use terrain::Terrain;
pub use world::*;
