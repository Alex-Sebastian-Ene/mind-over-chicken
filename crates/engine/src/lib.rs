pub mod audio;
pub mod biosignal;
mod camera3d;
pub mod eras;
mod font;
mod gl;
mod gl3d;
mod math3d;
mod mesh3d;
pub mod shell;
mod sprites;
mod terrain;

pub use biosignal::{RustBlinkClassifier, RustImuTracker};
use camera3d::OrbitCamera;
use gl::{Light, WebGlRenderer};
use gl3d::{BillboardVertex, Light3D, WebGl3DRenderer};
use math3d::Vec3;
use mesh3d::TerrainMesh;
use sprites::*;
use terrain::{Mulberry32, Terrain};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

#[wasm_bindgen]
pub struct Engine {
    renderer: WebGlRenderer,
    renderer_3d: Option<WebGl3DRenderer>,
    camera_3d: OrbitCamera,
    terrain: Option<Terrain>,
    frame_pixels: Vec<u8>,
    lights: Vec<Light>,
    lights_3d: Vec<Light3D>,
    billboards_3d: Vec<BillboardVertex>,
    current_era: u8,
    is_3d_mode: bool,
    time_3d: f32,
    audio: audio::AudioSynth,
    hud: shell::HudDriver,
    advisor: shell::AdvisorDriver,
    game: eras::game::GameFlow,
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

        // Initialize 3D renderer if WebGL supports it
        let mut renderer_3d = WebGl3DRenderer::new(renderer.gl.clone()).ok();
        if let Some(r3d) = &mut renderer_3d {
            let atlas = sprites::create_sprite_atlas();
            r3d.upload_atlas_texture(
                &atlas,
                sprites::ATLAS_SIZE as i32,
                sprites::ATLAS_SIZE as i32,
            );
        }

        let camera_3d = OrbitCamera::new(160.0, 90.0);
        let audio = audio::AudioSynth::new();
        let hud = shell::HudDriver::new();
        let advisor = shell::AdvisorDriver::new();
        let game = eras::game::GameFlow::new(0);

        Ok(Engine {
            renderer,
            renderer_3d,
            camera_3d,
            terrain: None,
            frame_pixels: vec![0; W * H * 4],
            lights: Vec::new(),
            lights_3d: Vec::new(),
            billboards_3d: Vec::new(),
            current_era: 0,
            is_3d_mode: true,
            time_3d: 0.0,
            audio,
            hud,
            advisor,
            game,
        })
    }

    pub fn set_3d_mode(&mut self, enabled: bool) {
        self.is_3d_mode = enabled && self.renderer_3d.is_some();
    }

    pub fn is_3d_mode(&self) -> bool {
        self.is_3d_mode && self.renderer_3d.is_some()
    }

    pub fn init_3d(&mut self) -> Result<(), JsValue> {
        if self.renderer_3d.is_none() {
            let mut r3d = WebGl3DRenderer::new(self.renderer.gl.clone())
                .map_err(|e| JsValue::from_str(&e))?;
            let atlas = sprites::create_sprite_atlas();
            r3d.upload_atlas_texture(
                &atlas,
                sprites::ATLAS_SIZE as i32,
                sprites::ATLAS_SIZE as i32,
            );
            if let Some(t) = &self.terrain {
                let mesh = TerrainMesh::build(&t.h, W, H, W as f32, H as f32, 24.0, 0.33);
                r3d.upload_mesh(&mesh);
                r3d.upload_terrain_texture(&t.pixels, W as i32, H as i32);
            }
            self.renderer_3d = Some(r3d);
        }
        self.is_3d_mode = true;
        Ok(())
    }

    pub fn init_world(&mut self, seed: u32) {
        let terrain = Terrain::new(seed);
        self.terrain = Some(terrain);
        self.set_era(self.current_era);

        // Build 3D heightmap terrain mesh
        if let (Some(r3d), Some(t)) = (&mut self.renderer_3d, &self.terrain) {
            let mesh = TerrainMesh::build(&t.h, W, H, W as f32, H as f32, 24.0, 0.33);
            r3d.upload_mesh(&mesh);
            r3d.upload_terrain_texture(&t.pixels, W as i32, H as i32);
        }
    }

    pub fn set_era(&mut self, era: u8) {
        self.current_era = era;
        if let Some(t) = &mut self.terrain {
            t.bake_era(era);
            if let Some(r3d) = &self.renderer_3d {
                r3d.upload_terrain_texture(&t.pixels, W as i32, H as i32);
            }
        }
    }

    // Camera 3D controls
    pub fn orbit_camera(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.camera_3d.orbit(delta_yaw, delta_pitch);
    }

    pub fn zoom_camera(&mut self, factor: f32) {
        self.camera_3d.zoom(factor);
    }

    pub fn pan_camera(&mut self, dx: f32, dz: f32) {
        self.camera_3d.pan(dx, dz);
    }

    pub fn set_camera_target(&mut self, tx: f32, tz: f32) {
        self.camera_3d.target.x = tx;
        self.camera_3d.target.z = tz;
    }

    pub fn get_camera_target_x(&self) -> f32 {
        self.camera_3d.target.x
    }

    pub fn get_camera_target_z(&self) -> f32 {
        self.camera_3d.target.z
    }

    pub fn get_camera_distance(&self) -> f32 {
        self.camera_3d.distance
    }

    pub fn get_camera_yaw(&self) -> f32 {
        self.camera_3d.yaw
    }

    pub fn get_camera_pitch(&self) -> f32 {
        self.camera_3d.pitch
    }

    // Unproject screen coordinates to 3D terrain surface for click aiming & lightning
    pub fn screen_to_ground(&self, sx: f32, sy: f32, cw: f32, ch: f32) -> js_sys::Float32Array {
        let result = js_sys::Float32Array::new_with_length(2);
        // Land on the terrain surface (hills included), never below the sea. The map repeats
        // east-west, so wrap x; beyond the poles, clamp to the edge row.
        let ground_at = |x: f32, z: f32| (self.get_terrain_height_at(x.rem_euclid(W as f32), z) * 24.0).max(7.92);
        if let Some([hx, hz]) = self.camera_3d.screen_to_terrain(sx, sy, cw, ch, 24.0, ground_at) {
            let hx = hx.rem_euclid(W as f32);
            let hz = hz.clamp(0.0, H as f32 - 1.0);
            result.set_index(0, hx);
            result.set_index(1, hz);
        } else {
            result.set_index(0, -1.0);
            result.set_index(1, -1.0);
        }
        result
    }

    // 3D Lighting & Billboards
    pub fn add_3d_light(&mut self, x: f32, y: f32, z: f32, radius: f32, intensity: f32) {
        self.lights_3d.push(Light3D {
            pos: Vec3::new(x, y, z),
            radius,
            intensity,
        });
    }

    pub fn clear_3d_billboards(&mut self) {
        self.billboards_3d.clear();
    }

    /// The colour around the map in 3D (the world's deep sea, 0..255), so the view's edges read
    /// as open ocean. JS passes its palette's deep-water colour.
    pub fn set_backdrop(&mut self, r: u8, g: u8, b: u8) {
        if let Some(r3d) = &mut self.renderer_3d {
            // The open sea renders ~1.45x its palette colour once lit and glazed (measured), so
            // match that and the pole edges disappear into it.
            let k = 1.45 / 255.0;
            r3d.backdrop = [
                (r as f32 * k).min(1.0),
                (g as f32 * k).min(1.0),
                (b as f32 * k).min(1.0),
            ];
        }
    }

    /// Use the JS world's heights (row-major W×H, 0..1, sea level 0.33) for the 3D mesh, the
    /// raycast and where billboards stand. The engine's own `Terrain` is still the old single
    /// island, so without this the relief doesn't match the map the simulation runs on.
    /// Call after `init_world`.
    pub fn set_heightmap(&mut self, heights: &[f32]) -> Result<(), JsValue> {
        if heights.len() != W * H {
            return Err(JsValue::from_str(&format!(
                "set_heightmap: expected {} heights ({}x{}), got {}",
                W * H, W, H, heights.len()
            )));
        }
        if let Some(t) = &mut self.terrain {
            t.h.copy_from_slice(heights);
        }
        if let Some(r3d) = &mut self.renderer_3d {
            let mesh = TerrainMesh::build(heights, W, H, W as f32, H as f32, 24.0, 0.33);
            r3d.upload_mesh(&mesh);
        }
        Ok(())
    }

    fn get_terrain_height_at(&self, x: f32, z: f32) -> f32 {
        if let Some(t) = &self.terrain {
            let px = (x.round().max(0.0) as usize).min(W - 1);
            let pz = (z.round().max(0.0) as usize).min(H - 1);
            t.h[pz * W + px]
        } else {
            0.33
        }
    }

    pub fn add_billboard(
        &mut self,
        sprite_id: u32,
        x: f32,
        z: f32,
        facing: i32,
        frame: u32,
        variant: u32,
    ) {
        let h_norm = self.get_terrain_height_at(x, z);
        let ground_y = (h_norm * 24.0).max(7.92); // Never sink below sea level

        let entry = sprites::get_sprite_atlas_entry(sprite_id, facing, frame, variant);
        let center = [x, ground_y, z];
        let hw = entry.world_w * 0.5;
        let h = entry.world_h;
        let (u0, v0, u1, v1) = (entry.u0, entry.v0, entry.u1, entry.v1);

        let v_tl = BillboardVertex {
            center,
            offset: [-hw, h],
            uv: [u0, v0],
        };
        let v_bl = BillboardVertex {
            center,
            offset: [-hw, 0.0],
            uv: [u0, v1],
        };
        let v_tr = BillboardVertex {
            center,
            offset: [hw, h],
            uv: [u1, v0],
        };
        let v_br = BillboardVertex {
            center,
            offset: [hw, 0.0],
            uv: [u1, v1],
        };

        self.billboards_3d
            .extend_from_slice(&[v_tl, v_bl, v_tr, v_tr, v_bl, v_br]);
    }

    pub fn add_custom_billboard(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) {
        let center = [x, y, z];
        let hw = w * 0.5;

        let v_tl = BillboardVertex {
            center,
            offset: [-hw, h],
            uv: [u0, v0],
        };
        let v_bl = BillboardVertex {
            center,
            offset: [-hw, 0.0],
            uv: [u0, v1],
        };
        let v_tr = BillboardVertex {
            center,
            offset: [hw, h],
            uv: [u1, v0],
        };
        let v_br = BillboardVertex {
            center,
            offset: [hw, 0.0],
            uv: [u1, v1],
        };

        self.billboards_3d
            .extend_from_slice(&[v_tl, v_bl, v_tr, v_tr, v_bl, v_br]);
    }

    pub fn render_3d(
        &mut self,
        dt: f32,
        sun_x: f32,
        sun_y: f32,
        sun_z: f32,
        night: f32,
        flash: f32,
        fade: f32,
        shake_x: f32,
        shake_y: f32,
        canvas_w: i32,
        canvas_h: i32,
    ) {
        self.time_3d += dt;
        // East-west wrap camera target
        self.camera_3d.target.x = self.camera_3d.target.x.rem_euclid(W as f32);

        let sun_dir = Vec3::new(sun_x, sun_y, sun_z).normalize();

        if let Some(r3d) = &mut self.renderer_3d {
            r3d.render_3d(
                &self.camera_3d,
                self.time_3d,
                sun_dir,
                night,
                &self.lights_3d,
                &self.billboards_3d,
            );

            // Blit 320x180 low-res target to screen canvas with nearest-neighbor upscale
            // The FBO's first row is the bottom of the image, so flip it on the way out.
            self.renderer.render_scene_texture(
                &r3d.fbo_texture,
                true,
                0.0,
                flash,
                fade,
                shake_x,
                shake_y,
                &[],
                canvas_w,
                canvas_h,
            );
        }

        self.lights_3d.clear();
        self.billboards_3d.clear();
    }

    pub fn begin_frame(&mut self) {
        if let Some(t) = &self.terrain {
            self.frame_pixels.copy_from_slice(&t.pixels);
        } else {
            self.frame_pixels.fill(0);
        }
        self.lights.clear();
        self.lights_3d.clear();
        self.billboards_3d.clear();
    }

    pub fn add_light(&mut self, x: f32, y: f32, radius: f32, intensity: f32) {
        self.lights.push(Light {
            x,
            y,
            radius,
            intensity,
        });
        let h_norm = self.get_terrain_height_at(x, y);
        let ground_y = (h_norm * 24.0).max(7.92);
        self.lights_3d.push(Light3D {
            pos: Vec3::new(x, ground_y + 2.0, y),
            radius,
            intensity,
        });
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
                sprites::fill_rect(
                    &mut self.frame_pixels,
                    px as i32 - 1,
                    py as i32,
                    3,
                    1,
                    160,
                    200,
                    255,
                    (a as f32 * 0.6) as u8,
                );
                sprites::put_pixel(
                    &mut self.frame_pixels,
                    px as i32,
                    py as i32,
                    255,
                    255,
                    255,
                    a,
                );
            }

            cur_x = next_x;
            cur_y = next_y;
        }
    }

    pub fn draw_reticle(&mut self, x: f32, y: f32, t: f32) {
        let rx = x.round() as i32;
        let ry = y.round() as i32;
        let r = 3 + (((t * 6.0).sin() + 1.0) * 0.5).round() as i32;
        sprites::fill_rect(
            &mut self.frame_pixels,
            rx - r - 1,
            ry,
            2,
            1,
            255,
            224,
            102,
            255,
        );
        sprites::fill_rect(
            &mut self.frame_pixels,
            rx + r,
            ry,
            2,
            1,
            255,
            224,
            102,
            255,
        );
        sprites::fill_rect(
            &mut self.frame_pixels,
            rx,
            ry - r - 1,
            1,
            2,
            255,
            224,
            102,
            255,
        );
        sprites::fill_rect(
            &mut self.frame_pixels,
            rx,
            ry + r,
            1,
            2,
            255,
            224,
            102,
            255,
        );
        sprites::put_pixel(&mut self.frame_pixels, rx, ry, 255, 224, 102, 255);
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8, a: u8) {
        sprites::fill_rect(&mut self.frame_pixels, x, y, w, h, r, g, b, a);
    }

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, r: u8, g: u8, b: u8, a: u8) {
        font::draw_text(&mut self.frame_pixels, x, y, text, r, g, b, a);
    }

    pub fn draw_text_centered(
        &mut self,
        cx: i32,
        y: i32,
        text: &str,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) {
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
        self.renderer.upload_canvas(canvas)?;
        if let Some(r3d) = &self.renderer_3d {
            let gl = &r3d.gl;
            gl.bind_texture(
                web_sys::WebGlRenderingContext::TEXTURE_2D,
                Some(&r3d.terrain_texture),
            );
            gl.tex_image_2d_with_u32_and_u32_and_canvas(
                web_sys::WebGlRenderingContext::TEXTURE_2D,
                0,
                web_sys::WebGlRenderingContext::RGBA as i32,
                web_sys::WebGlRenderingContext::RGBA,
                web_sys::WebGlRenderingContext::UNSIGNED_BYTE,
                canvas,
            )?;
        }
        Ok(())
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
        self.renderer
            .upload_pixels(&self.frame_pixels, W as i32, H as i32);
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
        self.terrain
            .as_ref()
            .map(|t| t.walkable(x, y))
            .unwrap_or(false)
    }

    pub fn get_terrain_type(&self, x: i32, y: i32) -> u8 {
        self.terrain.as_ref().map(|t| t.get(x, y)).unwrap_or(0)
    }

    pub fn get_site_village_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.village.x)
            .unwrap_or(150)
    }
    pub fn get_site_village_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.village.y)
            .unwrap_or(102)
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
        self.terrain
            .as_ref()
            .map(|t| t.sites.cave_mouth.x)
            .unwrap_or(130)
    }
    pub fn get_site_cave_mouth_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.cave_mouth.y)
            .unwrap_or(82)
    }

    pub fn get_site_factory_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.factory.x)
            .unwrap_or(104)
    }
    pub fn get_site_factory_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.factory.y)
            .unwrap_or(128)
    }

    pub fn get_site_exchange_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.exchange.x)
            .unwrap_or(168)
    }
    pub fn get_site_exchange_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.exchange.y)
            .unwrap_or(92)
    }

    pub fn get_site_hangar_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.hangar.x)
            .unwrap_or(236)
    }
    pub fn get_site_hangar_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.hangar.y)
            .unwrap_or(114)
    }

    pub fn bind_dom_shell(&mut self) -> Result<(), JsValue> {
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window found"))?;
            let document = window
                .document()
                .ok_or_else(|| JsValue::from_str("No document found"))?;
            self.hud.bind_dom(&document);
            self.advisor.bind_dom(&document);
        }
        Ok(())
    }

    pub fn audio_unlock(&mut self) {
        self.audio.unlock();
    }

    pub fn audio_toggle(&mut self) -> bool {
        self.audio.toggle()
    }

    pub fn audio_play(&mut self, sound: &str) {
        match sound {
            "thunder" => self.audio.thunder(),
            "ignite" => self.audio.ignite(),
            "blip" => self.audio.blip(),
            "good" => self.audio.good(),
            "bad" => self.audio.bad(),
            "pop" => self.audio.pop(),
            "coin" => self.audio.coin(),
            "bell" => self.audio.bell(),
            "crash" => self.audio.crash(),
            "tick" => self.audio.tick(),
            "rumble" => self.audio.rumble(0.8),
            "fanfare" => self.audio.fanfare(),
            _ => {}
        }
    }

    pub fn audio_heartbeat(&mut self, now: f64, bpm: f32, intensity: f32) {
        self.audio.heartbeat(now, bpm, intensity);
    }

    pub fn game_start_era(&mut self, index: usize) {
        self.game.start_era(index);
        self.set_era(index as u8);
        self.hud.set_scene(self.game.era.label(), self.game.era.powers());
        self.advisor.set_era(self.game.era.id());
    }

    pub fn game_restart_era(&mut self) {
        self.game.restart_current_era();
        self.set_era(self.game.era_index as u8);
        self.hud.set_scene(self.game.era.label(), self.game.era.powers());
        self.advisor.set_era(self.game.era.id());
    }

    pub fn game_tick(
        &mut self,
        dt: f32,
        calm: f32,
        focus: f32,
        panic: f32,
        hr: f32,
        rest_hr: f32,
        hr_delta: f32,
        blinks: u32,
    ) {
        let brain = eras::BrainInput {
            calm,
            focus,
            panic,
            hr,
            rest_hr,
            hr_delta,
            blinks,
        };

        let events = self.game.tick(dt, &brain);
        self.dispatch_era_events(&events);

        self.advisor.update(dt);
        self.hud.update_clock(self.game.stats.runtime);
        self.hud.update_meters(calm, None, focus, None, hr, rest_hr, hr_delta, blinks, panic);
        self.hud.update_objectives(&self.game.era.objectives());
    }

    pub fn game_on_blink(&mut self, aim_x: f32, aim_y: f32) {
        let events = self.game.on_blink(aim_x, aim_y);
        self.dispatch_era_events(&events);
    }

    pub fn game_on_cast(&mut self, spell: &str, aim_x: f32, aim_y: f32) {
        let events = self.game.on_cast(spell, aim_x, aim_y);
        self.dispatch_era_events(&events);
    }

    fn dispatch_era_events(&mut self, events: &[eras::EraEvent]) {
        for ev in events {
            match ev {
                eras::EraEvent::AdvisorSay {
                    key,
                    text,
                    mood,
                    priority,
                    cooldown,
                } => {
                    self.advisor.say(key, text, mood, *priority, *cooldown);
                }
                eras::EraEvent::PlaySound(sound) => match sound {
                    eras::SoundKind::Thunder => self.audio.thunder(),
                    eras::SoundKind::Ignite => self.audio.ignite(),
                    eras::SoundKind::Blip => self.audio.blip(),
                    eras::SoundKind::Good => self.audio.good(),
                    eras::SoundKind::Bad => self.audio.bad(),
                    eras::SoundKind::Pop => self.audio.pop(),
                    eras::SoundKind::Coin => self.audio.coin(),
                    eras::SoundKind::Bell => self.audio.bell(),
                    eras::SoundKind::Crash => self.audio.crash(),
                    eras::SoundKind::Tick => self.audio.tick(),
                    eras::SoundKind::Rumble => self.audio.rumble(0.8),
                    eras::SoundKind::Fanfare => self.audio.fanfare(),
                },
                _ => {}
            }
        }
    }

    pub fn advisor_say(
        &mut self,
        key: &str,
        text: &str,
        mood: &str,
        priority: u8,
        cooldown: f32,
    ) -> bool {
        self.advisor.say(key, text, mood, priority, cooldown)
    }

    pub fn get_game_era_index(&self) -> usize {
        self.game.era_index
    }

    pub fn get_game_era_id(&self) -> String {
        self.game.era.id().to_string()
    }

    pub fn get_game_era_label(&self) -> String {
        self.game.era.label().to_string()
    }

    pub fn is_game_finished(&self) -> bool {
        self.game.finished
    }

    pub fn is_game_failed(&self) -> Option<String> {
        self.game.era.is_failed()
    }

    pub fn get_game_runtime(&self) -> f32 {
        self.game.stats.runtime
    }

    pub fn get_game_lightning_stat(&self) -> u32 {
        self.game.stats.lightning
    }

    pub fn get_game_smitten_stat(&self) -> u32 {
        self.game.stats.smitten
    }

    pub fn get_game_births_stat(&self) -> u32 {
        self.game.stats.births
    }

    pub fn get_game_deaths_stat(&self) -> u32 {
        self.game.stats.deaths
    }

    pub fn get_game_recessions_stat(&self) -> u32 {
        self.game.stats.recessions
    }

    pub fn get_game_restarts_stat(&self) -> u32 {
        self.game.stats.restarts
    }
}
