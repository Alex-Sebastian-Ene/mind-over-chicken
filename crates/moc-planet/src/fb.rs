//! The 320×180 view the simulation draws into. Renderers upload `data` as an RGBA8 texture.
//! Screen coordinates clip; world objects are converted with [`crate::camera::Camera`].

use crate::camera::Camera;
use crate::color::Rgba;
use crate::{wrap_x, PH, PW, VH, VW};

#[derive(Clone)]
pub struct Framebuffer {
    pub w: usize,
    pub h: usize,
    /// Row-major RGBA8, top row first.
    pub data: Vec<u8>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new(VW, VH)
    }
}

impl Framebuffer {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, data: vec![0; w * h * 4] }
    }

    pub fn clear(&mut self, c: Rgba) {
        for px in self.data.chunks_exact_mut(4) {
            px.copy_from_slice(&c);
        }
    }

    #[inline]
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return None;
        }
        Some((y as usize * self.w + x as usize) * 4)
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Rgba> {
        self.index(x, y).map(|i| [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]])
    }

    /// Set one pixel, alpha-blending over what is there. Off-screen pixels are ignored.
    #[inline]
    pub fn px(&mut self, x: i32, y: i32, c: Rgba) {
        let Some(i) = self.index(x, y) else { return };
        if c[3] == 255 {
            self.data[i..i + 4].copy_from_slice(&c);
            return;
        }
        let a = c[3] as u32;
        for k in 0..3 {
            let d = self.data[i + k] as u32;
            self.data[i + k] = ((c[k] as u32 * a + d * (255 - a)) / 255) as u8;
        }
        self.data[i + 3] = 255;
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px(xx, yy, c);
            }
        }
    }

    /// Copy the part of a full-planet layer (PW×PH RGBA) that the camera sees.
    /// The planet wraps east–west, so the view can straddle the seam.
    pub fn blit_planet(&mut self, layer: &[u8], cam: &Camera) {
        debug_assert_eq!(layer.len(), PW * PH * 4);
        let (cx, cy) = (cam.ix(), cam.iy());
        for y in 0..self.h {
            let py = (cy + y as i32).clamp(0, PH as i32 - 1) as usize;
            let src_row = &layer[py * PW * 4..(py + 1) * PW * 4];
            let dst_row = &mut self.data[y * self.w * 4..(y + 1) * self.w * 4];
            let mut x = 0usize;
            while x < self.w {
                let sx = wrap_x(cx + x as i32) as usize;
                let run = (PW - sx).min(self.w - x);
                dst_row[x * 4..(x + run) * 4].copy_from_slice(&src_row[sx * 4..(sx + run) * 4]);
                x += run;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::rgb;

    #[test]
    fn screen_drawing_clips() {
        let mut fb = Framebuffer::default();
        fb.px(-1, 5, rgb(9, 8, 7));
        fb.px(3, VH as i32, rgb(1, 1, 1));
        fb.px(4, 6, rgb(9, 8, 7));
        assert_eq!(fb.get(4, 6), Some(rgb(9, 8, 7)));
        assert_eq!(fb.get(VW as i32 - 1, 5), Some([0, 0, 0, 0]));
    }

    #[test]
    fn blit_straddles_the_seam() {
        let mut layer = vec![0u8; PW * PH * 4];
        let put = |l: &mut Vec<u8>, x: usize, y: usize, c: Rgba| l[(y * PW + x) * 4..(y * PW + x) * 4 + 4].copy_from_slice(&c);
        put(&mut layer, PW - 1, 50, rgb(200, 0, 0)); // last column
        put(&mut layer, 0, 50, rgb(0, 200, 0)); // first column
        let cam = Camera { x: (PW - 10) as f32, y: 40.0 };
        let mut fb = Framebuffer::default();
        fb.blit_planet(&layer, &cam);
        assert_eq!(fb.get(9, 10), Some(rgb(200, 0, 0)));
        assert_eq!(fb.get(10, 10), Some(rgb(0, 200, 0)));
    }
}
