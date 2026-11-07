//! Planetary layer for Mind over Chicken.
//!
//! The world is a planet: an equirectangular `PW`×`PH` map (2:1, so it maps onto a sphere
//! without stretching) that wraps east–west, with poles at the top and bottom. The player
//! sees it through a `VW`×`VH` pixel camera that pans around the globe.
//!
//! This crate holds only the maths that makes that work, so it can drop into the engine's
//! terrain and renderer (`crates/engine`, #1) without overlapping them:
//! - [`rng`]: mulberry32, bit-for-bit the same sequence as the JS build
//! - [`noise`]: value noise that tiles east–west (no seam in generated terrain)
//! - [`camera`]: pan/centre/convert between planet and screen across the seam
//! - [`fb`]: a view-sized RGBA buffer and a seam-safe planet blit
//! - [`path`]: wrap-aware BFS distance fields for walkers (land) and boats (sea)

pub mod camera;
pub mod color;
pub mod fb;
pub mod noise;
pub mod path;
pub mod rng;

/// Planet width in pixels (360° of longitude). Twice the JS build's 320, as asked for.
pub const PW: usize = 640;
/// Planet height in pixels (pole to pole).
pub const PH: usize = 320;
/// View (screen) width in pixels.
pub const VW: usize = 320;
/// View (screen) height in pixels.
pub const VH: usize = 180;

/// Wrap an x coordinate around the planet.
#[inline]
pub fn wrap_x(x: i32) -> i32 {
    x.rem_euclid(PW as i32)
}

/// Wrap a float x coordinate around the planet.
#[inline]
pub fn wrap_xf(x: f32) -> f32 {
    x.rem_euclid(PW as f32)
}

/// Wrap a y coordinate around the torus (#111).
#[inline]
pub fn wrap_y(y: i32) -> i32 {
    y.rem_euclid(PH as i32)
}

/// Wrap a float y coordinate around the torus (#111).
#[inline]
pub fn wrap_yf(y: f32) -> f32 {
    y.rem_euclid(PH as f32)
}

/// Shortest signed east–west distance from `a` to `b` on the planet.
#[inline]
pub fn dx_wrap(a: f32, b: f32) -> f32 {
    let w = PW as f32;
    let mut d = (b - a).rem_euclid(w);
    if d > w / 2.0 {
        d -= w;
    }
    d
}

/// Shortest signed north–south distance from `a` to `b` on the torus (#111).
#[inline]
pub fn dy_wrap(a: f32, b: f32) -> f32 {
    let h = PH as f32;
    let mut d = (b - a).rem_euclid(h);
    if d > h / 2.0 {
        d -= h;
    }
    d
}

/// Distance between two points, taking the short way round the torus (#111).
#[inline]
pub fn dist_wrap(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = dx_wrap(ax, bx);
    let dy = dy_wrap(ay, by);
    (dx * dx + dy * dy).sqrt()
}

/// Map a planet pixel to (longitude, latitude) in radians, for the globe shader and for
/// anything that needs real spherical geometry. Longitude 0 is the map's left edge.
#[inline]
pub fn lon_lat(x: f32, y: f32) -> (f32, f32) {
    let lon = wrap_xf(x) / PW as f32 * std::f32::consts::TAU;
    let lat = (0.5 - (y + 0.5) / PH as f32) * std::f32::consts::PI;
    (lon, lat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_takes_the_short_way_round() {
        let w = PW as f32;
        assert_eq!(wrap_x(-1), PW as i32 - 1);
        assert_eq!(wrap_x(PW as i32), 0);
        assert!((dx_wrap(w - 10.0, 5.0) - 15.0).abs() < 1e-4);
        assert!((dx_wrap(5.0, w - 10.0) + 15.0).abs() < 1e-4);
        assert!((dist_wrap(w - 2.0, 10.0, 2.0, 10.0) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn lon_lat_covers_the_sphere() {
        let (lon0, lat_top) = lon_lat(0.0, 0.0);
        let (_, lat_bottom) = lon_lat(0.0, PH as f32 - 1.0);
        assert_eq!(lon0, 0.0);
        assert!(lat_top > 1.5 && lat_bottom < -1.5);
        let (lon_mid, lat_eq) = lon_lat(PW as f32 / 2.0, PH as f32 / 2.0 - 0.5);
        assert!((lon_mid - std::f32::consts::PI).abs() < 1e-5 && lat_eq.abs() < 1e-5);
    }
}
