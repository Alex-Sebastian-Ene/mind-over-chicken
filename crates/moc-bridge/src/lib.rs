//! Native Rust WebSocket headset bridge for Mind over Chicken (#25).
//!
//! Emits 10 Hz telemetry frames matching the `bridge/muse_bridge.py` protocol.

pub mod frame;
pub mod server;

pub use frame::{BandPowers, BridgeFrame, HeadOrientation, StatusMessage, SyntheticGenerator};
pub use server::BridgeServer;
