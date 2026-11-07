//! RGBA colours as plain byte arrays.

pub type Rgba = [u8; 4];

pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    [r, g, b, 255]
}

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    [r, g, b, a]
}

/// Scale the colour channels, clamping to 0..=255.
pub fn shade(c: Rgba, k: f32) -> Rgba {
    let s = |v: u8| ((v as f32) * k).clamp(0.0, 255.0) as u8;
    [s(c[0]), s(c[1]), s(c[2]), c[3]]
}
