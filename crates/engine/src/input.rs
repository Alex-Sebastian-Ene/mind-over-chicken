//! Pointer gestures on the map: with the main button or a finger, a **tap** blinks (lightning at
//! the aim) and a **drag** pans the planet left and right (and north–south on the vast worlds).
//!
//! The page forwards raw pointer events; this decides what they mean. A press only becomes a
//! drag once it has moved more than `threshold_px` screen pixels, so a slightly shaky click is
//! still a blink. A tap fires on release, never on press, so starting a drag never blinks.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct PointerGesture {
    threshold_px: f32,
    active: bool,
    id: i32,
    // Where the press started, in screen (CSS) pixels.
    start_x: f32,
    start_y: f32,
    // Last position in world units, for the pan delta.
    last_wx: f32,
    last_wy: f32,
    panning: bool,
    pan_dx: f32,
    pan_dy: f32,
}

#[wasm_bindgen]
impl PointerGesture {
    /// `threshold_px`: how far (screen pixels) a press must move before it counts as a drag.
    #[wasm_bindgen(constructor)]
    pub fn new(threshold_px: f32) -> PointerGesture {
        PointerGesture {
            threshold_px: threshold_px.max(0.0),
            active: false,
            id: -1,
            start_x: 0.0,
            start_y: 0.0,
            last_wx: 0.0,
            last_wy: 0.0,
            panning: false,
            pan_dx: 0.0,
            pan_dy: 0.0,
        }
    }

    /// A main-button or touch press. `sx, sy` screen pixels; `wx, wy` world units.
    pub fn down(&mut self, id: i32, sx: f32, sy: f32, wx: f32, wy: f32) {
        self.active = true;
        self.id = id;
        self.start_x = sx;
        self.start_y = sy;
        self.last_wx = wx;
        self.last_wy = wy;
        self.panning = false;
        self.pan_dx = 0.0;
        self.pan_dy = 0.0;
    }

    /// Pointer moved. Returns true when this move pans the map; read the delta with
    /// `pan_dx()` / `pan_dy()` (world units since the previous move). Returns false while the
    /// press is still a possible tap, or for another pointer.
    pub fn moved(&mut self, id: i32, sx: f32, sy: f32, wx: f32, wy: f32) -> bool {
        if !self.owns(id) {
            return false;
        }
        if !self.panning {
            let (dx, dy) = (sx - self.start_x, sy - self.start_y);
            if (dx * dx + dy * dy).sqrt() > self.threshold_px {
                self.panning = true;
            }
        }
        if !self.panning {
            self.pan_dx = 0.0;
            self.pan_dy = 0.0;
            return false;
        }
        self.pan_dx = wx - self.last_wx;
        self.pan_dy = wy - self.last_wy;
        self.last_wx = wx;
        self.last_wy = wy;
        true
    }

    pub fn pan_dx(&self) -> f32 {
        self.pan_dx
    }

    pub fn pan_dy(&self) -> f32 {
        self.pan_dy
    }

    /// Press ended. Returns true when it was a tap (blink). A cancelled pointer never taps.
    pub fn up(&mut self, id: i32, cancelled: bool) -> bool {
        if !self.owns(id) {
            return false;
        }
        let tap = !self.panning && !cancelled;
        self.active = false;
        self.id = -1;
        self.panning = false;
        tap
    }

    /// Is this pointer the press being tracked?
    pub fn owns(&self, id: i32) -> bool {
        self.active && self.id == id
    }

    pub fn is_panning(&self) -> bool {
        self.active && self.panning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_without_moving_is_a_tap() {
        let mut g = PointerGesture::new(6.0);
        g.down(1, 100.0, 100.0, 50.0, 40.0);
        assert!(g.up(1, false));
    }

    #[test]
    fn small_jitter_is_still_a_tap() {
        let mut g = PointerGesture::new(6.0);
        g.down(1, 100.0, 100.0, 50.0, 40.0);
        assert!(!g.moved(1, 103.0, 102.0, 51.0, 40.5));
        assert_eq!((g.pan_dx(), g.pan_dy()), (0.0, 0.0));
        assert!(g.up(1, false));
    }

    #[test]
    fn a_drag_pans_and_does_not_tap() {
        let mut g = PointerGesture::new(6.0);
        g.down(1, 100.0, 100.0, 50.0, 40.0);
        assert!(g.moved(1, 120.0, 100.0, 57.0, 40.0)); // past the threshold: pans from the press point
        assert_eq!(g.pan_dx(), 7.0);
        assert!(g.moved(1, 130.0, 104.0, 60.0, 41.0));
        assert_eq!((g.pan_dx(), g.pan_dy()), (3.0, 1.0));
        // Coming back near the start doesn't turn it back into a tap.
        assert!(g.moved(1, 101.0, 100.0, 50.5, 40.0));
        assert!(!g.up(1, false));
    }

    #[test]
    fn cancel_and_other_pointers_never_tap() {
        let mut g = PointerGesture::new(6.0);
        g.down(1, 0.0, 0.0, 0.0, 0.0);
        assert!(!g.moved(2, 50.0, 0.0, 10.0, 0.0));
        assert!(!g.up(2, false));
        assert!(g.owns(1));
        assert!(!g.up(1, true));
        assert!(!g.owns(1));
    }
}
