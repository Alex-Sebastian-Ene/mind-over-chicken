//! Global climate simulation: latitude temperature gradient, lapse-rate altitude cooling,
//! prevailing trade winds with orographic rain shadows, and Whittaker biome classification.

pub const T_DEEP: u8 = 0;
pub const T_SHALLOW: u8 = 1;
pub const T_SAND: u8 = 2;
pub const T_GRASS: u8 = 3;
pub const T_FOREST: u8 = 4;
pub const T_HILL: u8 = 5;
pub const T_MOUNTAIN: u8 = 6;
pub const T_SNOW: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WhittakerBiome {
    PolarIce,
    Tundra,
    Taiga,
    TemperateForest,
    TemperateRainforest,
    Grassland,
    Desert,
    Savanna,
    TropicalRainforest,
}

pub struct ClimateModel {
    pub sea_level: f32,
    pub lapse_rate: f32, // Temperature decrease with altitude
}

impl Default for ClimateModel {
    fn default() -> Self {
        Self {
            sea_level: 0.33,
            lapse_rate: 0.55,
        }
    }
}

impl ClimateModel {
    /// Computes temperature in [0, 1] based on latitude and altitude.
    /// `v` in [0, 1] is latitude (0 = North Pole, 0.5 = Equator, 1 = South Pole).
    pub fn temperature(&self, v: f32, height: f32) -> f32 {
        // Solar insolation: maximum at equator (v = 0.5), drops to 0 at poles
        let lat_factor = (1.0 - (2.0 * v - 1.0).abs()).max(0.0);
        let base_temp = 0.05 + 0.90 * (lat_factor * std::f32::consts::PI * 0.5).sin();

        // Lapse rate: mountains are colder than coastal plains
        let altitude_above_sea = (height - self.sea_level).max(0.0);
        let cooled = base_temp - altitude_above_sea * self.lapse_rate;
        cooled.clamp(0.0, 1.0)
    }

    /// Determines the prevailing wind direction along longitude (-1 = East-to-West, +1 = West-to-East).
    /// Follows Earth-like atmospheric circulation cells:
    /// - Hadley cells (0.35..0.65): Trade winds blow East-to-West (-1.0)
    /// - Ferrel cells (0.15..0.35 and 0.65..0.85): Westerlies blow West-to-East (+1.0)
    /// - Polar cells (0.0..0.15 and 0.85..1.0): Polar easterlies blow East-to-West (-1.0)
    pub fn prevailing_wind(&self, v: f32) -> f32 {
        if (0.35..=0.65).contains(&v) {
            -1.0 // Easterly trade winds
        } else if (0.15..=0.85).contains(&v) {
            1.0 // Mid-latitude westerlies
        } else {
            -1.0 // Polar easterlies
        }
    }

    /// Whittaker biome lookup table given temperature [0, 1] and moisture [0, 1].
    pub fn whittaker_biome(&self, temp: f32, moisture: f32) -> WhittakerBiome {
        if temp < 0.18 {
            WhittakerBiome::PolarIce
        } else if temp < 0.32 {
            if moisture < 0.40 {
                WhittakerBiome::Tundra
            } else {
                WhittakerBiome::Taiga
            }
        } else if temp < 0.65 {
            if moisture < 0.28 {
                WhittakerBiome::Desert
            } else if moisture < 0.55 {
                WhittakerBiome::Grassland
            } else if moisture < 0.80 {
                WhittakerBiome::TemperateForest
            } else {
                WhittakerBiome::TemperateRainforest
            }
        } else {
            // Tropical
            if moisture < 0.30 {
                WhittakerBiome::Desert
            } else if moisture < 0.60 {
                WhittakerBiome::Savanna
            } else {
                WhittakerBiome::TropicalRainforest
            }
        }
    }

    /// Maps height, temperature, moisture, and slope to engine tile type (0..7).
    pub fn classify_tile(&self, height: f32, temp: f32, moisture: f32, slope: f32) -> u8 {
        if height < self.sea_level - 0.09 {
            T_DEEP
        } else if height < self.sea_level {
            T_SHALLOW
        } else if height < self.sea_level + 0.04 {
            // Coastal beach / shelf
            if temp < 0.20 {
                T_SNOW
            } else {
                T_SAND
            }
        } else if height > 0.82 || (height > 0.68 && temp < 0.25) {
            T_SNOW // Alpine snow caps / polar glaciers
        } else if height > 0.72 || slope > 0.38 {
            T_MOUNTAIN // Rugged crags
        } else if height > 0.58 || slope > 0.22 {
            T_HILL // Uplands and foothills
        } else {
            // Lowlands: classified by Whittaker climate
            let biome = self.whittaker_biome(temp, moisture);
            match biome {
                WhittakerBiome::PolarIce | WhittakerBiome::Tundra => T_SNOW,
                WhittakerBiome::Taiga => T_FOREST,
                WhittakerBiome::TemperateForest | WhittakerBiome::TropicalRainforest | WhittakerBiome::TemperateRainforest => T_FOREST,
                WhittakerBiome::Grassland | WhittakerBiome::Savanna => T_GRASS,
                WhittakerBiome::Desert => T_SAND,
            }
        }
    }
}
