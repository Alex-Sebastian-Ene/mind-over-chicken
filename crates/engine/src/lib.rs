mod font;
mod gl;
mod sprites;
mod terrain;

use gl::{Light, WebGlRenderer};
use sprites::*;
use terrain::{Terrain, Mulberry32};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

#[wasm_bindgen]
pub struct Engine {
    renderer: WebGlRenderer,
    terrain: Option<Terrain>,
    frame_pixels: Vec<u8>,
    lights: Vec<Light>,
    current_era: u8,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<Engine, JsValue> {
        console_error_panic_hook::set_once();

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

        let renderer = WebGlRenderer::new(&canvas).map_err(|e| JsValue::from_str(&e))?;

        Ok(Engine {
            renderer,
            terrain: None,
            frame_pixels: vec![0; W * H * 4],
            lights: Vec::new(),
            current_era: 0,
        })
    }

    pub fn init_world(&mut self, seed: u32) {
        let terrain = Terrain::new(seed);
        self.terrain = Some(terrain);
        self.set_era(self.current_era);
    }

    pub fn set_era(&mut self, era: u8) {
        self.current_era = era;
        if let Some(t) = &mut self.terrain {
            t.bake_era(era);
        }
    }

    pub fn begin_frame(&mut self) {
        if let Some(t) = &self.terrain {
            self.frame_pixels.copy_from_slice(&t.pixels);
        } else {
            self.frame_pixels.fill(0);
        }
        self.lights.clear();
    }

    pub fn add_light(&mut self, x: f32, y: f32, radius: f32, intensity: f32) {
        self.lights.push(Light { x, y, radius, intensity });
    }

    pub fn draw_person(
        &mut self,
        x: i32,
        y: i32,
        frame: u32,
        pose: u8,
        shirt_r: u8,
        shirt_g: u8,
        shirt_b: u8,
    ) {
        sprites::draw_person(
            &mut self.frame_pixels,
            x,
            y,
            frame,
            pose,
            (shirt_r, shirt_g, shirt_b),
            (0xe0, 0xb4, 0x8a),
            (0x3b, 0x2a, 0x1e),
        );
    }

    pub fn draw_mammoth(&mut self, x: i32, y: i32, facing: i32, frame: u32, dead: bool) {
        sprites::draw_mammoth(&mut self.frame_pixels, x, y, facing, frame, dead);
    }

    pub fn draw_goat(&mut self, x: i32, y: i32, facing: i32) {
        sprites::draw_goat(&mut self.frame_pixels, x, y, facing);
    }

    pub fn draw_chicken(&mut self, x: i32, y: i32, facing: i32, frame: u32, halo: bool) {
        sprites::draw_chicken(&mut self.frame_pixels, x, y, facing, frame, halo);
    }

    pub fn draw_fire(&mut self, x: i32, y: i32, heat: f32, t: f32) {
        sprites::draw_fire(&mut self.frame_pixels, x, y, heat, t);
    }

    pub fn draw_rocket(&mut self, x: i32, y: i32, progress: f32, t: f32, flame: f32) {
        sprites::draw_rocket(&mut self.frame_pixels, x, y, progress, t, flame);
    }

    pub fn draw_lightning(&mut self, tx: f32, ty: f32, seed: u32, alpha: f32) {
        let mut rng = Mulberry32::new(seed);
        let start_x = tx + (rng.next() - 0.5) * 30.0;
        let mut cur_x = start_x;
        let mut cur_y = -4.0f32;
        let steps = 9;

        let a = (alpha * 255.0).clamp(0.0, 255.0) as u8;

        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let next_x = if i == steps {
                tx
            } else {
                start_x + (tx - start_x) * t + (rng.next() - 0.5) * 9.0
            };
            let next_y = ty * t;

            let n = ((next_x - cur_x).abs().max((next_y - cur_y).abs())).ceil() as i32;
            for s in 0..=n {
                let frac = if n > 0 { s as f32 / n as f32 } else { 0.0 };
                let px = cur_x + (next_x - cur_x) * frac;
                let py = cur_y + (next_y - cur_y) * frac;

                // Glow and core
                sprites::fill_rect(&mut self.frame_pixels, px as i32 - 1, py as i32, 3, 1, 160, 200, 255, (a as f32 * 0.6) as u8);
                sprites::put_pixel(&mut self.frame_pixels, px as i32, py as i32, 255, 255, 255, a);
            }

            cur_x = next_x;
            cur_y = next_y;
        }
    }

    pub fn draw_reticle(&mut self, x: f32, y: f32, t: f32) {
        let rx = x.round() as i32;
        let ry = y.round() as i32;
        let r = 3 + (((t * 6.0).sin() + 1.0) * 0.5).round() as i32;
        sprites::fill_rect(&mut self.frame_pixels, rx - r - 1, ry, 2, 1, 255, 224, 102, 255);
        sprites::fill_rect(&mut self.frame_pixels, rx + r, ry, 2, 1, 255, 224, 102, 255);
        sprites::fill_rect(&mut self.frame_pixels, rx, ry - r - 1, 1, 2, 255, 224, 102, 255);
        sprites::fill_rect(&mut self.frame_pixels, rx, ry + r, 1, 2, 255, 224, 102, 255);
        sprites::put_pixel(&mut self.frame_pixels, rx, ry, 255, 224, 102, 255);
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8, a: u8) {
        sprites::fill_rect(&mut self.frame_pixels, x, y, w, h, r, g, b, a);
    }

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, r: u8, g: u8, b: u8, a: u8) {
        font::draw_text(&mut self.frame_pixels, x, y, text, r, g, b, a);
    }

    pub fn draw_text_centered(&mut self, cx: i32, y: i32, text: &str, r: u8, g: u8, b: u8, a: u8) {
        font::draw_text_centered(&mut self.frame_pixels, cx, y, text, r, g, b, a);
    }

    pub fn draw_text_badge(
        &mut self,
        cx: i32,
        y: i32,
        text: &str,
        text_r: u8,
        text_g: u8,
        text_b: u8,
        bg_r: u8,
        bg_g: u8,
        bg_b: u8,
        bg_a: u8,
    ) {
        font::draw_text_badge(
            &mut self.frame_pixels,
            cx,
            y,
            text,
            (text_r, text_g, text_b),
            (bg_r, bg_g, bg_b, bg_a),
        );
    }

    pub fn draw_pixel(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
        sprites::put_pixel(&mut self.frame_pixels, x, y, r, g, b, a);
    }

    pub fn upload_canvas(&mut self, canvas: &HtmlCanvasElement) -> Result<(), JsValue> {
        self.renderer.upload_canvas(canvas)
    }

    pub fn end_frame(
        &mut self,
        night: f32,
        flash: f32,
        fade: f32,
        shake_x: f32,
        shake_y: f32,
        canvas_w: i32,
        canvas_h: i32,
    ) {
        self.renderer.upload_pixels(&self.frame_pixels, W as i32, H as i32);
        self.renderer.render_scene(
            night,
            flash,
            fade,
            shake_x,
            shake_y,
            &self.lights,
            canvas_w,
            canvas_h,
        );
    }

    /// Render the uploaded canvas with this frame's lights, then clear them so the next
    /// frame starts fresh (lights are added per frame with `add_light`).
    pub fn render_uploaded_canvas(
        &mut self,
        night: f32,
        flash: f32,
        fade: f32,
        shake_x: f32,
        shake_y: f32,
        canvas_w: i32,
        canvas_h: i32,
    ) {
        self.renderer.render_scene(
            night,
            flash,
            fade,
            shake_x,
            shake_y,
            &self.lights,
            canvas_w,
            canvas_h,
        );
        self.lights.clear();
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.terrain.as_ref().map(|t| t.walkable(x, y)).unwrap_or(false)
    }

    pub fn get_terrain_type(&self, x: i32, y: i32) -> u8 {
        self.terrain.as_ref().map(|t| t.get(x, y)).unwrap_or(0)
    }

    pub fn get_site_village_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.village.x).unwrap_or(150)
    }
    pub fn get_site_village_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.village.y).unwrap_or(102)
    }

    pub fn get_site_pad_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.pad.x).unwrap_or(262)
    }
    pub fn get_site_pad_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.pad.y).unwrap_or(104)
    }

    pub fn get_site_cave_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.cave.x).unwrap_or(130)
    }
    pub fn get_site_cave_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.cave.y).unwrap_or(82)
    }

    pub fn get_site_cave_mouth_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.cave_mouth.x).unwrap_or(130)
    }
    pub fn get_site_cave_mouth_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.cave_mouth.y).unwrap_or(82)
    }

    pub fn get_site_factory_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.factory.x).unwrap_or(104)
    }
    pub fn get_site_factory_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.factory.y).unwrap_or(128)
    }

    pub fn get_site_exchange_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.exchange.x).unwrap_or(168)
    }
    pub fn get_site_exchange_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.exchange.y).unwrap_or(92)
    }

    pub fn get_site_hangar_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.hangar.x).unwrap_or(236)
    }
    pub fn get_site_hangar_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.hangar.y).unwrap_or(114)
    }
}
