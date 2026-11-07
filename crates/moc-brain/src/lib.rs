//! Pure Rust brain processing, machine learning personal model, calibration, and simulated headset.
//!
//! Part of Mind over Chicken (#20, #54).

pub mod brain;
pub mod calibration;
pub mod math;
pub mod model;
pub mod sim_source;
pub mod wasm_api;

pub use brain::{Brain, BrainStats};
pub use calibration::{
    random_seed_traits, traits_from_features, Calibration, CalibrationConfig, CalibrationMode,
    CalibrationResult, CivTraits, MoodInput, SensorFeatures, CALIBRATION_SECONDS,
};
pub use model::{
    features_from_bands, Bands, Class, PersonalModel, Predictions, FEATURE_NAMES, NUM_FEATURES,
};
pub use sim_source::{BrainFrame, SimFingerprint, SimInput, SimLatent, SimSource, FRAME_DT};
