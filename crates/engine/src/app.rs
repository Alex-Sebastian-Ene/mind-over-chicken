//! Pure Rust Application Shell (`app.rs`).
//!
//! Owns the game loop (via `requestAnimationFrame`), input handling (`web-sys`),
//! scene & era progression, audio synthesis, camera controls, and WebGL rendering.
//!
//! Part of the Rust Cutover (Wave 3 of #108, Refs #17).

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, KeyboardEvent, PointerEvent, WheelEvent};

use crate::camera::casts::CastController;
use crate::camera::emotion::EmotionTracker;
use crate::camera::fingers::{FingerController, Landmark};
use crate::camera::spells::SpellId;
use crate::input::PointerGesture;
use crate::Engine;
use moc_brain::{Brain, SimSource};
use moc_cosmos::Galaxy;

pub struct App {
    pub engine: Engine,
    pub brain: Brain,
    pub sim_source: SimSource,
    pub galaxy: Galaxy,
    pub casts: CastController,
    pub fingers: FingerController,
    pub emotion: EmotionTracker,
    pub pointer: PointerGesture,
    pub canvas_w: f32,
    pub canvas_h: f32,
    pub last_ts: f64,
    pub running: bool,
    pub aim: [f32; 2],
    pub sun_angle: f32,
}

impl App {
    pub fn new(canvas_id: &str, canvas_w: f32, canvas_h: f32) -> Result<Self, JsValue> {
        let mut engine = Engine::new(canvas_id)?;
        let _ = engine.bind_dom_shell();
        let _ = engine.init_3d();
        engine.set_3d_mode(true);
        engine.init_world(0x1337);
        engine.game_start_era(0);

        Ok(Self {
            engine,
            brain: Brain::new(),
            sim_source: SimSource::new(0x42),
            galaxy: Galaxy::new(0x99),
            casts: CastController::default(),
            fingers: FingerController::default(),
            emotion: EmotionTracker::default(),
            pointer: PointerGesture::new(6.0),
            canvas_w,
            canvas_h,
            last_ts: 0.0,
            running: true,
            aim: [160.0, 90.0],
            sun_angle: 0.0,
        })
    }

    pub fn frame(&mut self, ts: f64) {
        let dt = if self.last_ts > 0.0 {
            ((ts - self.last_ts) / 1000.0).clamp(0.001, 0.05) as f32
        } else {
            0.016
        };
        self.last_ts = ts;

        // 1. Headset simulation & Brain
        let mut frame_opt = None;
        self.sim_source.update(dt, |f| {
            frame_opt = Some(f);
        });
        if let Some(f) = frame_opt {
            self.brain.ingest(&f);
        }
        self.brain.update(dt);

        // 2. Galaxy simulation
        let fx = moc_cosmos::BrainEffects::from_signals(
            self.brain.calm,
            self.brain.focus,
            self.brain.stress,
            0,
        );
        self.galaxy.step(dt, &fx, self.brain.stats.blinks);

        // 3. Era game tick & DOM shell updates
        self.engine.game_tick(
            dt,
            self.brain.calm,
            self.brain.focus,
            self.brain.stress,
            self.brain.stats.blinks,
        );

        // 4. Era progression check
        if self.engine.is_game_era_complete() {
            let cur = self.engine.get_game_era_index();
            if cur < 2 {
                self.engine.game_start_era(cur + 1);
            }
        }

        // 5. 3D era billboards & lights
        self.engine.draw_era_3d();

        // 6. 3D WebGL scene rendering
        self.sun_angle += dt * 0.15;
        let sun_x = self.sun_angle.cos();
        let sun_y = 0.85;
        let sun_z = self.sun_angle.sin();
        let dark = self.engine.get_game_darkness();

        self.engine.render_3d(
            dt,
            sun_x,
            sun_y,
            sun_z,
            dark,
            0.0,
            0.0,
            0.0,
            0.0,
            self.canvas_w as i32,
            self.canvas_h as i32,
        );
    }

    pub fn on_pointer_down(&mut self, id: i32, sx: f32, sy: f32) {
        let ground = self.engine.screen_to_ground(sx, sy, self.canvas_w, self.canvas_h);
        let wx = ground.get_index(0);
        let wz = ground.get_index(1);
        let (wx, wz) = if wx >= 0.0 {
            (wx, wz)
        } else {
            (sx / self.canvas_w * 320.0, sy / self.canvas_h * 180.0)
        };
        self.aim = [wx, wz];
        self.pointer.down(id, sx, sy, wx, wz);
    }

    pub fn on_pointer_move(&mut self, id: i32, sx: f32, sy: f32) {
        let ground = self.engine.screen_to_ground(sx, sy, self.canvas_w, self.canvas_h);
        let wx = ground.get_index(0);
        let wz = ground.get_index(1);
        let (wx, wz) = if wx >= 0.0 {
            (wx, wz)
        } else {
            (sx / self.canvas_w * 320.0, sy / self.canvas_h * 180.0)
        };
        self.aim = [wx, wz];
        if self.pointer.moved(id, sx, sy, wx, wz) {
            let dx = self.pointer.pan_dx();
            let dy = self.pointer.pan_dy();
            let cur_x = self.engine.get_camera_target_x();
            let cur_z = self.engine.get_camera_target_z();
            self.engine.set_camera_target(
                (cur_x - dx).rem_euclid(320.0),
                (cur_z - dy).clamp(0.0, 179.0),
            );
        }
    }

    pub fn on_pointer_up(&mut self, id: i32, cancelled: bool) {
        let is_tap = self.pointer.up(id, cancelled);
        if is_tap {
            self.engine.game_on_blink(self.aim[0], self.aim[1]);
            self.sim_source.blink();
        }
    }

    pub fn on_wheel(&mut self, delta_y: f64) {
        let factor = if delta_y > 0.0 { 1.1 } else { 0.9 };
        self.engine.zoom_camera(factor);
    }

    pub fn on_key_down(&mut self, key: &str, now: f64) {
        match key {
            // The three core moves (#122, #159). Key 1 (pinch grab) needs the JS shell's scene
            // grab; this shell has no grab target, so it does nothing here.
            "2" => {
                if self.casts.trigger(SpellId::Palm, now) {
                    self.engine.game_on_cast("palm", self.aim[0], self.aim[1]);
                }
            }
            "3" => {
                if self.casts.trigger(SpellId::Fireball, now) {
                    self.engine.game_on_cast("fireball", self.aim[0], self.aim[1]);
                }
            }
            " " => {
                self.engine.game_on_blink(self.aim[0], self.aim[1]);
                self.sim_source.blink();
            }
            "Shift" => {
                self.sim_source.input.panic = true;
            }
            _ => {}
        }
    }

    pub fn on_key_up(&mut self, key: &str) {
        if key == "Shift" {
            self.sim_source.input.panic = false;
        }
    }

    pub fn on_hand_landmarks(&mut self, coords: Option<&[f64]>, aspect: f64, now: f64) {
        let landmarks_vec = coords.and_then(|c| {
            if c.len() >= 63 {
                let mut v = Vec::with_capacity(21);
                for i in 0..21 {
                    v.push(Landmark {
                        x: c[i * 3],
                        y: c[i * 3 + 1],
                        z: c[i * 3 + 2],
                    });
                }
                Some(v)
            } else {
                None
            }
        });

        let u = self
            .fingers
            .update(landmarks_vec.as_deref(), "None", 0.0, now, aspect);

        if let Some(aim) = u.aim {
            self.aim = [aim.0 as f32 * 320.0, aim.1 as f32 * 180.0];
        }
        if u.cast {
            self.engine.game_on_cast("fireball", self.aim[0], self.aim[1]);
        }
    }

    pub fn on_face_blendshapes(
        &mut self,
        has_face: bool,
        smile_l: f32,
        smile_r: f32,
        brow_down_l: f32,
        brow_down_r: f32,
        brow_inner_up: f32,
        eye_wide_l: f32,
        eye_wide_r: f32,
        jaw_open: f32,
        mouth_frown_l: f32,
        mouth_frown_r: f32,
        dt: f32,
    ) {
        let mut map = std::collections::HashMap::new();
        if has_face {
            map.insert("mouthSmileLeft".to_string(), smile_l as f64);
            map.insert("mouthSmileRight".to_string(), smile_r as f64);
            map.insert("browDownLeft".to_string(), brow_down_l as f64);
            map.insert("browDownRight".to_string(), brow_down_r as f64);
            map.insert("browInnerUp".to_string(), brow_inner_up as f64);
            map.insert("eyeWideLeft".to_string(), eye_wide_l as f64);
            map.insert("eyeWideRight".to_string(), eye_wide_r as f64);
            map.insert("jawOpen".to_string(), jaw_open as f64);
            map.insert("mouthFrownLeft".to_string(), mouth_frown_l as f64);
            map.insert("mouthFrownRight".to_string(), mouth_frown_r as f64);
        }
        let (mood, _) = self.emotion.update(if has_face { Some(&map) } else { None }, dt as f64);
        if mood.anger > 0.0 {
            self.brain.stress = self.brain.stress.max(mood.anger as f32);
        }
    }
}

/// WebAssembly entry point wrapping `App` for browser usage (#108).
#[wasm_bindgen]
pub struct RustApp {
    inner: Rc<RefCell<App>>,
    canvas_id: String,
}

#[wasm_bindgen]
impl RustApp {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<RustApp, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window found"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("No document found"))?;
        let element = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("Canvas #{} not found", canvas_id)))?;
        let canvas: HtmlCanvasElement = element
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("Element is not a canvas"))?;

        let w = canvas.width() as f32;
        let h = canvas.height() as f32;
        let app = App::new(canvas_id, w, h)?;

        Ok(RustApp {
            inner: Rc::new(RefCell::new(app)),
            canvas_id: canvas_id.to_string(),
        })
    }

    pub fn start(&self) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window found"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("No document found"))?;
        let element = document
            .get_element_by_id(&self.canvas_id)
            .ok_or_else(|| JsValue::from_str("Canvas element not found"))?;
        let canvas: HtmlCanvasElement = element
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("Element is not a canvas"))?;

        attach_dom_listeners(&canvas, self.inner.clone());
        start_raf_loop(self.inner.clone());

        Ok(())
    }

    pub fn stop(&self) {
        self.inner.borrow_mut().running = false;
    }

    pub fn step_frame(&self, ts: f64) {
        self.inner.borrow_mut().frame(ts);
    }

    pub fn on_hand_landmarks(&self, coords: Option<Vec<f64>>, aspect: f64, now: f64) {
        self.inner
            .borrow_mut()
            .on_hand_landmarks(coords.as_deref(), aspect, now);
    }

    pub fn on_face_blendshapes(
        &self,
        has_face: bool,
        smile_l: f32,
        smile_r: f32,
        brow_down_l: f32,
        brow_down_r: f32,
        brow_inner_up: f32,
        eye_wide_l: f32,
        eye_wide_r: f32,
        jaw_open: f32,
        mouth_frown_l: f32,
        mouth_frown_r: f32,
        dt: f32,
    ) {
        self.inner.borrow_mut().on_face_blendshapes(
            has_face,
            smile_l,
            smile_r,
            brow_down_l,
            brow_down_r,
            brow_inner_up,
            eye_wide_l,
            eye_wide_r,
            jaw_open,
            mouth_frown_l,
            mouth_frown_r,
            dt,
        );
    }
}

fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    if let Some(w) = web_sys::window() {
        let _ = w.request_animation_frame(f.as_ref().unchecked_ref());
    }
}

fn start_raf_loop(app: Rc<RefCell<App>>) {
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move |ts: f64| {
        let running = {
            let mut a = app.borrow_mut();
            a.frame(ts);
            a.running
        };
        if running {
            request_animation_frame(f.borrow().as_ref().unwrap());
        }
    }) as Box<dyn FnMut(f64)>));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

fn attach_dom_listeners(canvas: &HtmlCanvasElement, app: Rc<RefCell<App>>) {
    // pointerdown
    {
        let app_clone = app.clone();
        let closure = Closure::wrap(Box::new(move |e: PointerEvent| {
            let sx = e.client_x() as f32;
            let sy = e.client_y() as f32;
            app_clone
                .borrow_mut()
                .on_pointer_down(e.pointer_id(), sx, sy);
        }) as Box<dyn FnMut(_)>);
        let _ = canvas
            .add_event_listener_with_callback("pointerdown", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // pointermove
    {
        let app_clone = app.clone();
        let closure = Closure::wrap(Box::new(move |e: PointerEvent| {
            let sx = e.client_x() as f32;
            let sy = e.client_y() as f32;
            app_clone
                .borrow_mut()
                .on_pointer_move(e.pointer_id(), sx, sy);
        }) as Box<dyn FnMut(_)>);
        let _ = canvas
            .add_event_listener_with_callback("pointermove", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // pointerup
    {
        let app_clone = app.clone();
        let closure = Closure::wrap(Box::new(move |e: PointerEvent| {
            app_clone.borrow_mut().on_pointer_up(e.pointer_id(), false);
        }) as Box<dyn FnMut(_)>);
        let _ =
            canvas.add_event_listener_with_callback("pointerup", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // wheel (zoom)
    {
        let app_clone = app.clone();
        let closure = Closure::wrap(Box::new(move |e: WheelEvent| {
            app_clone.borrow_mut().on_wheel(e.delta_y());
        }) as Box<dyn FnMut(_)>);
        let _ = canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // keydown & keyup on window
    if let Some(w) = web_sys::window() {
        {
            let app_clone = app.clone();
            let closure = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                let now = if let Some(w) = web_sys::window() {
                    w.performance().map(|p| p.now()).unwrap_or(0.0)
                } else {
                    0.0
                };
                app_clone.borrow_mut().on_key_down(&e.key(), now);
            }) as Box<dyn FnMut(_)>);
            let _ = w.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
            closure.forget();
        }

        {
            let app_clone = app.clone();
            let closure = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                app_clone.borrow_mut().on_key_up(&e.key());
            }) as Box<dyn FnMut(_)>);
            let _ = w.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }
}
