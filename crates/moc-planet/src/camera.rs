//! The player's window onto the planet. Pans freely both ways round the torus (#111).

use crate::{dx_wrap, dy_wrap, wrap_xf, wrap_yf, PH, PW, VH, VW};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Planet x of the view's left edge (wraps).
    pub x: f32,
    /// Planet y of the view's top edge (wraps).
    pub y: f32,
}

impl Camera {
    /// A camera centred on a planet point.
    pub fn centred_on(px: f32, py: f32) -> Self {
        let mut c = Self { x: 0.0, y: 0.0 };
        c.centre(px, py);
        c
    }

    pub fn centre(&mut self, px: f32, py: f32) {
        self.x = wrap_xf(px - VW as f32 / 2.0);
        self.y = wrap_yf(py - VH as f32 / 2.0);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.x = wrap_xf(self.x + dx);
        self.y = wrap_yf(self.y + dy);
    }

    /// Planet → screen. x and y may come out negative or ≥ VW/VH for points just off-screen.
    pub fn to_screen(&self, px: f32, py: f32) -> (f32, f32) {
        let mut sx = dx_wrap(self.x, px);
        // Points up to a view-width west of the camera stay west (negative), the rest east.
        if sx < -(VW as f32) {
            sx += PW as f32;
        }
        let mut sy = dy_wrap(self.y, py);
        if sy < -(VH as f32) {
            sy += PH as f32;
        }
        (sx, sy)
    }

    /// Screen → planet.
    pub fn to_planet(&self, sx: f32, sy: f32) -> (f32, f32) {
        (wrap_xf(self.x + sx), wrap_yf(self.y + sy))
    }

    pub fn ix(&self) -> i32 {
        self.x.floor() as i32
    }

    pub fn iy(&self) -> i32 {
        self.y.floor() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panning_wraps_both_ways() {
        let mut c = Camera::centred_on(5.0, 5.0);
        assert!(c.x > PW as f32 - VW as f32); // centred near x=5 means the left edge wraps west
        assert!(c.y > PH as f32 - VH as f32);
        let y0 = c.y;
        c.pan(0.0, -(PH as f32));
        assert!((c.y - y0).abs() < 1e-4);
    }

    #[test]
    fn screen_and_planet_round_trip_across_the_seam() {
        let c = Camera { x: PW as f32 - 20.0, y: PH as f32 - 20.0 };
        let (sx, sy) = c.to_screen(10.0, 10.0); // just east and south of the seams
        assert!((sx - 30.0).abs() < 1e-4 && (sy - 30.0).abs() < 1e-4);
        let (px, py) = c.to_planet(sx, sy);
        assert!((px - 10.0).abs() < 1e-4 && (py - 10.0).abs() < 1e-4);
    }
}
