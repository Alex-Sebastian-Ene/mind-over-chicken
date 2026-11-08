pub mod agents;
pub mod app;
pub mod audio;
pub mod biosignal;
pub mod camera;
mod camera3d;
pub mod eras;
pub mod input; // pointer gestures: tap blinks, drag pans
mod font;
mod gl;
mod gl3d;
mod math3d;
mod mesh3d;
pub mod particles;
pub mod shell;
mod sprites;
mod terrain;

pub use app::RustApp;
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
        let h_norm = self.get_terrain_height_at(tx.rem_euclid(W as f32), tz.rem_euclid(H as f32));
        self.camera_3d.target.y = (h_norm * 24.0).max(7.92);
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
        // Land on the terrain surface (hills included), never below the sea. Seamless torus (#111).
        let ground_at = |x: f32, z: f32| (self.get_terrain_height_at(x.rem_euclid(W as f32), z.rem_euclid(H as f32)) * 24.0).max(7.92);
        if let Some([hx, hz]) = self.camera_3d.screen_to_terrain(sx, sy, cw, ch, 24.0, ground_at) {
            let hx = hx.rem_euclid(W as f32);
            let hz = hz.rem_euclid(H as f32);
            result.set_index(0, hx);
            result.set_index(1, hz);
        } else {
            result.set_index(0, -1.0);
            result.set_index(1, -1.0);
        }
        result
    }

    // Project 3D terrain world coordinates [wx, wz] to screen pixels [sx, sy]
    pub fn project_to_screen(&self, wx: f32, wz: f32, cw: f32, ch: f32) -> js_sys::Float32Array {
        let result = js_sys::Float32Array::new_with_length(2);
        let mut wrapped_x = wx.rem_euclid(W as f32);
        let mut wrapped_z = wz.rem_euclid(H as f32);
        // Torus seam wrapping relative to camera target (#111)
        if wrapped_x - self.camera_3d.target.x > (W as f32) * 0.5 {
            wrapped_x -= W as f32;
        } else if self.camera_3d.target.x - wrapped_x > (W as f32) * 0.5 {
            wrapped_x += W as f32;
        }
        if wrapped_z - self.camera_3d.target.z > (H as f32) * 0.5 {
            wrapped_z -= H as f32;
        } else if self.camera_3d.target.z - wrapped_z > (H as f32) * 0.5 {
            wrapped_z += H as f32;
        }
        let h_norm = self.get_terrain_height_at(wrapped_x.rem_euclid(W as f32), wrapped_z.rem_euclid(H as f32));
        let wy = (h_norm * 24.0).max(7.92);
        if let Some([sx, sy]) = self.camera_3d.world_to_screen(wrapped_x, wy, wrapped_z, cw, ch) {
            result.set_index(0, sx);
            result.set_index(1, sy);
        } else {
            result.set_index(0, -9999.0);
            result.set_index(1, -9999.0);
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
    /// raycast and where billboards stand.
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

    /// Generate a home world with moc-terrain (#109) and make it the engine's: heights, tiles,
    /// colours, sites and the 3D relief. `width`/`height` 0 (or left out) mean the engine's
    /// 320×180; the engine only draws worlds of its own size, so another size only updates the
    /// 3D mesh (until the game supports bigger maps, #129).
    pub fn generate_terrain(&mut self, seed: u32, width: u32, height: u32) -> Result<(), JsValue> {
        let (w, h) = if width == 0 || height == 0 { (W, H) } else { (width as usize, height as usize) };
        let map = moc_terrain::generate(&moc_terrain::TerrainParams::home_world_sized(seed, w, h));
        if (w, h) == (W, H) {
            let mut t = Terrain::new_empty(seed);
            t.apply_map(&map);
            t.bake_era(self.current_era);
            self.terrain = Some(t);
        }
        if let Some(r3d) = &mut self.renderer_3d {
            let mesh = TerrainMesh::build(&map.heights, w, h, w as f32, h as f32, 24.0, 0.33);
            r3d.upload_mesh(&mesh);
            if let Some(t) = &self.terrain {
                r3d.upload_terrain_texture(&t.pixels, W as i32, H as i32);
            }
        }
        Ok(())
    }

    /// Standalone helper to generate a civ planet RGBA texture (#109).
    pub fn generate_civ_terrain(seed: u32, width: usize, height: usize) -> Vec<u8> {
        let map = moc_terrain::TerrainMap::generate_civ_planet(seed, width, height);
        map.rgba
    }

    /// The orbit centre rides on the ground (as `set_camera_target` places it, eased here so
    /// `pan_camera` follows the land too) and the eye stays 3 units above the land under it, so
    /// zooming right in (`MIN_DISTANCE`) never puts the camera inside a hill.
    fn settle_camera_on_ground(&mut self, dt: f32) {
        const SEA_Y: f32 = 7.92;
        let ground = |e: &Self, x: f32, z: f32| (e.get_terrain_height_at(x, z) * 24.0).max(SEA_Y);
        let (tx, tz) = (self.camera_3d.target.x, self.camera_3d.target.z);
        let want = ground(self, tx, tz);
        let k = (dt * 8.0).clamp(0.0, 1.0);
        let mut y = self.camera_3d.target.y + (want - self.camera_3d.target.y) * k;
        let mut probe = self.camera_3d.clone();
        probe.target.y = y;
        let eye = probe.eye_position();
        let lift = ground(self, eye.x, eye.z) + 3.0 - eye.y;
        if lift > 0.0 {
            y += lift;
        }
        self.camera_3d.target.y = y;
    }

    fn get_terrain_height_at(&self, x: f32, z: f32) -> f32 {
        if let Some(t) = &self.terrain {
            let map_w = W as f32;
            let map_h = H as f32;
            let wx = x.rem_euclid(map_w);
            let wz = z.rem_euclid(map_h);
            let x0 = wx.floor() as usize;
            let z0 = wz.floor() as usize;
            let x1 = (x0 + 1) % W;
            let z1 = (z0 + 1) % H;
            let fx = wx - wx.floor();
            let fz = wz - wz.floor();
            let h00 = t.h[z0 * W + x0];
            let h10 = t.h[z0 * W + x1];
            let h01 = t.h[z1 * W + x0];
            let h11 = t.h[z1 * W + x1];
            let h0 = h00 * (1.0 - fx) + h10 * fx;
            let h1 = h01 * (1.0 - fx) + h11 * fx;
            h0 * (1.0 - fz) + h1 * fz
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
        self.add_billboard_y(sprite_id, x, 0.0, z, facing, frame, variant);
    }

    pub fn add_billboard_y(
        &mut self,
        sprite_id: u32,
        x: f32,
        y: f32,
        z: f32,
        facing: i32,
        frame: u32,
        variant: u32,
    ) {
        let map_w = W as f32;
        let map_h = H as f32;
        let wx = x.rem_euclid(map_w);
        let wz = z.rem_euclid(map_h);
        let h_norm = self.get_terrain_height_at(wx, wz);

        let ground_y = (h_norm * 24.0).max(7.92) + 0.15; // Ground elevation or at water level (7.92)
        let center = [wx, ground_y + y, wz];
        let ground_center = [wx, ground_y, wz];

        // Soft ground contact shadow beneath entities, trees, and structures (fades out as entity ascends)
        let shadow_variant = if y < 15.0 {
            match sprite_id {
                sprites::SPRITE_VILLAGER | sprites::SPRITE_CHICKEN | sprites::SPRITE_GOAT => Some(0),
                sprites::SPRITE_MAMMOTH
                | sprites::SPRITE_HUT
                | sprites::SPRITE_HOUSE
                | sprites::SPRITE_TOWER
                | sprites::SPRITE_ROCKET
                | sprites::SPRITE_TREE => Some(1),
                sprites::SPRITE_FACTORY
                | sprites::SPRITE_EXCHANGE
                | sprites::SPRITE_HANGAR
                | sprites::SPRITE_PAD => Some(2),
                _ => None,
            }
        } else {
            None
        };

        if let Some(sv) = shadow_variant {
            let s_entry = sprites::get_sprite_atlas_entry(sprites::SPRITE_SHADOW, 1, 0, sv);
            let shw = s_entry.world_w * 0.5;
            let shd = s_entry.world_h * 0.5;
            let s_tl = BillboardVertex {
                center: ground_center,
                offset: [-shw, 0.02, -shd],
                uv: [s_entry.u0, s_entry.v0],
            };
            let s_bl = BillboardVertex {
                center: ground_center,
                offset: [-shw, 0.02, shd],
                uv: [s_entry.u0, s_entry.v1],
            };
            let s_tr = BillboardVertex {
                center: ground_center,
                offset: [shw, 0.02, -shd],
                uv: [s_entry.u1, s_entry.v0],
            };
            let s_br = BillboardVertex {
                center: ground_center,
                offset: [shw, 0.02, shd],
                uv: [s_entry.u1, s_entry.v1],
            };
            self.billboards_3d
                .extend_from_slice(&[s_tl, s_bl, s_tr, s_tr, s_bl, s_br]);
        }

        let entry = sprites::get_sprite_atlas_entry(sprite_id, facing, frame, variant);
        let hw = entry.world_w * 0.5;
        let h = entry.world_h;
        let (u0, v0, u1, v1) = (entry.u0, entry.v0, entry.u1, entry.v1);

        let v_tl = BillboardVertex {
            center,
            offset: [-hw, h, 0.0],
            uv: [u0, v0],
        };
        let v_bl = BillboardVertex {
            center,
            offset: [-hw, 0.0, 0.0],
            uv: [u0, v1],
        };
        let v_tr = BillboardVertex {
            center,
            offset: [hw, h, 0.0],
            uv: [u1, v0],
        };
        let v_br = BillboardVertex {
            center,
            offset: [hw, 0.0, 0.0],
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
            offset: [-hw, h, 0.0],
            uv: [u0, v0],
        };
        let v_bl = BillboardVertex {
            center,
            offset: [-hw, 0.0, 0.0],
            uv: [u0, v1],
        };
        let v_tr = BillboardVertex {
            center,
            offset: [hw, h, 0.0],
            uv: [u1, v0],
        };
        let v_br = BillboardVertex {
            center,
            offset: [hw, 0.0, 0.0],
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
        // Torus wrap camera target along both axes (#111)
        self.camera_3d.target.x = self.camera_3d.target.x.rem_euclid(W as f32);
        self.camera_3d.target.z = self.camera_3d.target.z.rem_euclid(H as f32);
        self.settle_camera_on_ground(dt);

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
            .unwrap_or(116)
    }
    pub fn get_site_village_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.village.y)
            .unwrap_or(100)
    }

    pub fn get_site_pad_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.pad.x).unwrap_or(198)
    }
    pub fn get_site_pad_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.pad.y).unwrap_or(104)
    }

    pub fn get_site_cave_x(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.cave.x).unwrap_or(96)
    }
    pub fn get_site_cave_y(&self) -> i32 {
        self.terrain.as_ref().map(|t| t.sites.cave.y).unwrap_or(80)
    }

    pub fn get_site_cave_mouth_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.cave_mouth.x)
            .unwrap_or(96)
    }
    pub fn get_site_cave_mouth_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.cave_mouth.y)
            .unwrap_or(80)
    }

    pub fn get_site_factory_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.factory.x)
            .unwrap_or(70)
    }
    pub fn get_site_factory_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.factory.y)
            .unwrap_or(126)
    }

    pub fn get_site_exchange_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.exchange.x)
            .unwrap_or(134)
    }
    pub fn get_site_exchange_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.exchange.y)
            .unwrap_or(90)
    }

    pub fn get_site_hangar_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.hangar.x)
            .unwrap_or(172)
    }
    pub fn get_site_hangar_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.hangar.y)
            .unwrap_or(114)
    }

    pub fn get_site_fire_x(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.fire.x)
            .unwrap_or(116)
    }
    pub fn get_site_fire_y(&self) -> i32 {
        self.terrain
            .as_ref()
            .map(|t| t.sites.fire.y)
            .unwrap_or(100)
    }

    pub fn get_terrain_field(&mut self, tx: i32, ty: i32) -> Vec<i16> {
        if let Some(t) = &mut self.terrain {
            t.field(tx, ty).to_vec()
        } else {
            vec![-1i16; 320 * 180]
        }
    }

    pub fn get_terrain_sea_field(&mut self, tx: i32, ty: i32) -> Vec<i16> {
        if let Some(t) = &mut self.terrain {
            t.sea_field(tx, ty).to_vec()
        } else {
            vec![-1i16; 320 * 180]
        }
    }

    pub fn is_terrain_reachable(&mut self, x: i32, y: i32) -> bool {
        if let Some(t) = &mut self.terrain {
            t.reachable(x, y)
        } else {
            false
        }
    }

    pub fn set_terrain_biome(&mut self, biome_name: &str) {
        let biome = match biome_name {
            "desert" => terrain::BiomeType::Desert,
            "ice" => terrain::BiomeType::Ice,
            "jungle" => terrain::BiomeType::Jungle,
            "alien" => terrain::BiomeType::Alien,
            _ => terrain::BiomeType::Temperate,
        };
        if let Some(t) = &mut self.terrain {
            t.biome = biome;
            t.palette = terrain::BiomePalette::for_biome(biome);
            // The planet's climate and colours change the land itself (moc-terrain).
            t.generate();
            t.bake_era(self.game.era_index as u8);
            if let Some(r3d) = &mut self.renderer_3d {
                r3d.upload_terrain_texture(&t.pixels, 320, 180);
            }
        }
    }

    pub fn get_tree_count(&self) -> usize {
        self.terrain.as_ref().map(|t| t.trees.len()).unwrap_or(0)
    }

    pub fn get_tree_x(&self, idx: usize) -> i32 {
        self.terrain.as_ref().and_then(|t| t.trees.get(idx)).map(|tr| tr.x).unwrap_or(0)
    }

    pub fn get_tree_y(&self, idx: usize) -> i32 {
        self.terrain.as_ref().and_then(|t| t.trees.get(idx)).map(|tr| tr.y).unwrap_or(0)
    }

    pub fn get_tree_variant(&self, idx: usize) -> u32 {
        self.terrain.as_ref().and_then(|t| t.trees.get(idx)).map(|tr| tr.variant).unwrap_or(0)
    }

    pub fn get_continent_count(&self) -> usize {
        self.terrain.as_ref().map(|t| t.continents.len()).unwrap_or(0)
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

    /// One tick of the Rust eras. `stress` is the JS brain's `brain.stress`
    /// (EEG agitation or face anger, 0..1, about 0.15 at rest).
    pub fn game_tick(&mut self, dt: f32, calm: f32, focus: f32, stress: f32, blinks: u32) {
        let brain = eras::BrainInput {
            calm,
            focus,
            stress,
            blinks,
        };

        let events = self.game.tick(dt, &brain);
        self.dispatch_era_events(&events);

        self.advisor.update(dt);
        self.hud.update_clock(self.game.stats.runtime);
        self.hud.update_meters(calm, None, focus, None, stress, blinks);
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

    pub fn draw_era_3d(&mut self) {
        let lights = self.game.lights_3d();
        for l in lights {
            self.add_3d_light(l.x, l.y, l.z, l.radius, l.intensity);
        }
        let billboards = self.game.billboards_3d();
        for b in billboards {
            self.add_billboard(b.sprite_id, b.x, b.z, b.facing, b.frame, b.variant);
        }
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

    pub fn advisor_set_era(&mut self, era: &str) {
        self.advisor.set_era(era);
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

    pub fn get_game_darkness(&self) -> f32 {
        self.game.era.darkness()
    }

    pub fn is_game_era_complete(&self) -> bool {
        self.game.era.is_complete()
    }

    pub fn is_game_era_failed(&self) -> Option<String> {
        self.game.era.is_failed()
    }
}

#[wasm_bindgen]
pub fn rust_villager_look(era: &str, seed: u32) -> js_sys::Array {
    let mut rng = crate::terrain::Mulberry32::new(seed);
    let look = crate::agents::villager_look(era, &mut rng);
    let arr = js_sys::Array::new();
    let to_hex = |c: (u8, u8, u8)| format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2);
    arr.push(&JsValue::from_str(&to_hex(look.shirt)));
    arr.push(&JsValue::from_str(&to_hex(look.hair)));
    arr.push(&JsValue::from_str(&to_hex(look.skin)));
    arr
}

#[wasm_bindgen]
pub fn rust_animate_agent(clock: f32, dt: f32, rate: f32) -> js_sys::Float32Array {
    let arr = js_sys::Float32Array::new_with_length(2);
    let new_clock = clock + dt * rate;
    let frame = new_clock.floor();
    arr.set_index(0, new_clock);
    arr.set_index(1, frame);
    arr
}

/// A generated world for JS (#109, from #112): the JS `World` is a thin wrapper that fills its
/// arrays from this. `new(seed)` is the 320×180 home world; `home(seed, w, h)` any size;
/// `planet(seed, biome)` a galaxy planet's 640×360 surface with its 320×180 capital (the AI
/// tribe's world) cut from it at (0, 90). Accessors without `surface_` describe the world (the
/// capital for a planet); `surface_*` describe the whole surface.
#[wasm_bindgen]
pub struct RustWorldGen {
    map: moc_terrain::TerrainMap,
    surface: Option<moc_terrain::TerrainMap>,
    ms: f64,
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn bools(v: &[bool]) -> js_sys::Uint8Array {
    let b: Vec<u8> = v.iter().map(|&x| x as u8).collect();
    js_sys::Uint8Array::from(&b[..])
}

/// Island sizes (landmasses that aren't major continents), landform and biome shares, as JSON.
fn stats_json(m: &moc_terrain::TerrainMap) -> String {
    let mut isl: Vec<usize> = m.continents.iter().filter(|c| !c.major).map(|c| c.area).collect();
    isl.sort();
    let land = m.biome.iter().filter(|&&b| moc_terrain::climate::biome::is_land(b)).count().max(1);
    let water = m.width * m.height - land;
    let majors = m.continents.iter().filter(|c| c.major).count();
    format!(
        "{{\"width\":{},\"height\":{},\"land\":{},\"seaShare\":{:.3},\"continents\":{},\"islands\":{},\"islandMin\":{},\"islandMedian\":{},\"islandMax\":{},\"minIsland\":{}}}",
        m.width,
        m.height,
        land,
        water as f32 / (m.width * m.height) as f32,
        majors,
        isl.len(),
        isl.first().copied().unwrap_or(0),
        isl.get(isl.len() / 2).copied().unwrap_or(0),
        isl.last().copied().unwrap_or(0),
        m.stats.min_island
    )
}

#[wasm_bindgen]
impl RustWorldGen {
    /// The 320×180 home world.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        Self::home(seed, 0, 0, "temperate")
    }

    /// A home world of any size (0 × 0: the game's 320×180) for a planet biome (temperate,
    /// desert, ice, jungle, alien).
    pub fn home(seed: u32, width: u32, height: u32, biome: &str) -> RustWorldGen {
        let t0 = now_ms();
        let (w, h) = if width == 0 || height == 0 { (W, H) } else { (width as usize, height as usize) };
        let mut p = moc_terrain::TerrainParams::home_world_sized(seed, w, h);
        p.set_biome(biome);
        let map = moc_terrain::generate(&p);
        RustWorldGen { map, surface: None, ms: now_ms() - t0 }
    }

    /// A galaxy planet: its 640×360 surface, and the 320×180 capital at (0, 90) as the world.
    pub fn planet(seed: u32, biome: &str) -> RustWorldGen {
        let t0 = now_ms();
        let surface = moc_terrain::TerrainMap::generate_planet_surface(seed, biome);
        let map = surface.crop(0, 90, W, H, true);
        RustWorldGen { map, surface: Some(surface), ms: now_ms() - t0 }
    }

    /// Generation time, ms.
    pub fn ms(&self) -> f64 {
        self.ms
    }
    pub fn width(&self) -> u32 {
        self.map.width as u32
    }
    pub fn height(&self) -> u32 {
        self.map.height as u32
    }
    /// Heights 0..1, sea level 0.33 (like the old maps, so the 3D water level is unchanged).
    pub fn heights(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(&self.map.heights[..])
    }
    /// Game tiles (the JS `T` codes).
    pub fn types(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(&self.map.types[..])
    }
    /// The base colour layer, RGBA.
    pub fn rgba(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(&self.map.rgba[..])
    }
    pub fn biomes(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(&self.map.biome[..])
    }
    pub fn rivers(&self) -> js_sys::Uint8Array {
        bools(&self.map.river_mask)
    }
    pub fn lakes(&self) -> js_sys::Uint8Array {
        bools(&self.map.lake_mask)
    }
    /// Campaign landmass per tile (index into `continents_json`), 255 for none.
    pub fn owner(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(&self.map.owner()[..])
    }
    pub fn sites_json(&self) -> String {
        self.map.sites_json()
    }
    pub fn continents_json(&self) -> String {
        self.map.continents_json()
    }
    pub fn stats_json(&self) -> String {
        stats_json(&self.map)
    }

    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }
    pub fn surface_width(&self) -> u32 {
        self.surface.as_ref().map(|s| s.width as u32).unwrap_or(0)
    }
    pub fn surface_height(&self) -> u32 {
        self.surface.as_ref().map(|s| s.height as u32).unwrap_or(0)
    }
    pub fn surface_types(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.surface.as_ref().map(|s| &s.types[..]).unwrap_or(&[]))
    }
    pub fn surface_heights(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.surface.as_ref().map(|s| &s.heights[..]).unwrap_or(&[]))
    }
    pub fn surface_rgba(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.surface.as_ref().map(|s| &s.rgba[..]).unwrap_or(&[]))
    }
    pub fn surface_stats_json(&self) -> String {
        self.surface.as_ref().map(stats_json).unwrap_or_else(|| "{}".into())
    }
    /// The surface's landmasses and settlement spots (capital first, then nearest first), as
    /// the galaxy's surface view expects them: {tile, continents, spots}.
    pub fn surface_json(&self) -> String {
        let Some(s) = &self.surface else { return "{}".into() };
        let (tx, ty) = (0i32, 90i32);
        let d = s.dims();
        let cap = self.map.campaign_sites.as_ref().map(|c| (c.village.0 as i32 + tx, c.village.1 as i32 + ty)).unwrap_or((W as i32 / 2, ty + H as i32 / 2));
        let mut conts = Vec::new();
        for (k, c) in s.continents.iter().enumerate() {
            let v = s.sites.iter().find(|q| q.continent == k).map(|q| (q.x, q.y)).unwrap_or((c.cx as i32, c.cy as i32));
            conts.push(format!(
                "{{\"id\":\"c{}\",\"cx\":{:.1},\"cy\":{:.1},\"rx\":{:.1},\"ry\":{:.1},\"area\":{},\"village\":{{\"x\":{},\"y\":{}}}}}",
                k, c.cx, c.cy, c.bw as f32 * 0.5, c.bh as f32 * 0.5, c.area, v.0, v.1
            ));
        }
        // Spots: the capital, then the ranked sites, spread out, nearest the capital first.
        let mut spots: Vec<(i32, i32, usize)> = Vec::new();
        let cap_c = s.continent[d.idx(cap.0, cap.1).unwrap_or(0)] as usize;
        let mut rest: Vec<(i32, i32, usize)> = Vec::new();
        for q in s.sites.iter() {
            let far = |a: &(i32, i32, usize)| d.dist2(a.0 as f32, a.1 as f32, q.x as f32, q.y as f32) > 26.0 * 26.0;
            if d.dist2(cap.0 as f32, cap.1 as f32, q.x as f32, q.y as f32) > 26.0 * 26.0 && rest.iter().all(far) {
                rest.push((q.x, q.y, q.continent + 1));
            }
        }
        rest.sort_by(|a, b| d.dist2(cap.0 as f32, cap.1 as f32, a.0 as f32, a.1 as f32).total_cmp(&d.dist2(cap.0 as f32, cap.1 as f32, b.0 as f32, b.1 as f32)));
        spots.push((cap.0, cap.1, cap_c));
        spots.extend(rest);
        let spots: Vec<String> = spots
            .iter()
            .enumerate()
            .map(|(k, &(x, y, c))| {
                let cont = if k == 0 { "home".to_string() } else if c == 0 { "sea".to_string() } else { format!("c{}", c - 1) };
                format!("{{\"x\":{},\"y\":{},\"cont\":\"{}\"{}}}", x, y, cont, if k == 0 { ",\"capital\":true" } else { "" })
            })
            .collect();
        format!("{{\"tile\":{{\"x\":{},\"y\":{}}},\"continents\":[{}],\"spots\":[{}]}}", tx, ty, conts.join(","), spots.join(","))
    }
}

/// The era's land use (farms, roads, paving: tiles whose type differs from the base type)
/// painted over the base colour layer, in a planet biome's colours.
#[wasm_bindgen]
pub fn paint_land_use(base: &[u8], types: &[u8], base_types: &[u8], width: u32, biome: &str) -> Vec<u8> {
    let b = match biome {
        "desert" => terrain::BiomeType::Desert,
        "ice" => terrain::BiomeType::Ice,
        "jungle" => terrain::BiomeType::Jungle,
        "alien" => terrain::BiomeType::Alien,
        _ => terrain::BiomeType::Temperate,
    };
    terrain::paint_land_use(base, types, base_types, width as usize, &terrain::BiomePalette::for_biome(b))
}

/// Walking distance (8-neighbour steps, no wrap) from every tile to (tx, ty) over walkable
/// tiles, or over water with `sea`; -1 where unreachable. The JS `World.field`/`seaField`.
#[wasm_bindgen]
pub fn walk_field(types: &[u8], width: u32, height: u32, tx: i32, ty: i32, sea: bool) -> Vec<i16> {
    let (w, h) = (width as i32, height as i32);
    let mut f = vec![-1i16; types.len()];
    if w <= 0 || h <= 0 || types.len() < (w * h) as usize {
        return f;
    }
    // The map is a torus (#111): the target and every step wrap both ways.
    let (tx, ty) = (tx.rem_euclid(w), ty.rem_euclid(h));
    let ok = |t: u8| if sea { t <= terrain::T_SHALLOW } else { t >= terrain::T_SAND && t != terrain::T_MOUNTAIN && t != terrain::T_SNOW };
    let mut q = std::collections::VecDeque::new();
    let s0 = (ty * w + tx) as usize;
    f[s0] = 0;
    q.push_back(s0);
    while let Some(i) = q.pop_front() {
        let (x, y) = ((i as i32) % w, (i as i32) / w);
        let d = f[i].saturating_add(1);
        for oy in -1..=1 {
            for ox in -1..=1 {
                if ox == 0 && oy == 0 {
                    continue;
                }
                let (nx, ny) = ((x + ox).rem_euclid(w), (y + oy).rem_euclid(h));
                let j = (ny * w + nx) as usize;
                if f[j] != -1 || !ok(types[j]) {
                    continue;
                }
                f[j] = d;
                q.push_back(j);
            }
        }
    }
    f
}

/// Standalone WebAssembly Cast Controller for camera god powers (#33, #108)
#[wasm_bindgen]
pub struct RustCastController {
    inner: camera::casts::CastController,
}

#[wasm_bindgen]
impl RustCastController {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: camera::casts::CastController::default(),
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn spell_for(&self, gesture: &str) -> Option<String> {
        self.inner.spell_for(gesture).map(|s| s.as_str().to_string())
    }

    pub fn update(&mut self, gesture: &str, score: f64, now: f64) -> js_sys::Array {
        let u = self.inner.update(gesture, score, now);
        let arr = js_sys::Array::new();
        arr.push(&JsValue::from(u.held.map(|s| s.as_str())));
        arr.push(&JsValue::from_f64(u.progress));
        arr.push(&JsValue::from(u.cast.map(|s| s.as_str())));
        arr.push(&JsValue::from_bool(u.aiming));
        arr
    }

    pub fn trigger(&mut self, spell_id: &str, now: f64) -> bool {
        if let Some(id) = camera::spells::SpellId::from_str(spell_id) {
            self.inner.trigger(id, now)
        } else {
            false
        }
    }

    pub fn ready(&self, spell_id: &str, now: f64) -> bool {
        if let Some(id) = camera::spells::SpellId::from_str(spell_id) {
            self.inner.ready(id, now)
        } else {
            false
        }
    }

    pub fn cooldown_left(&self, spell_id: &str, now: f64) -> f64 {
        if let Some(id) = camera::spells::SpellId::from_str(spell_id) {
            self.inner.cooldown_left(id, now)
        } else {
            0.0
        }
    }

    pub fn cooldown_frac(&self, spell_id: &str, now: f64) -> f64 {
        if let Some(id) = camera::spells::SpellId::from_str(spell_id) {
            self.inner.cooldown_frac(id, now)
        } else {
            0.0
        }
    }
}

/// Standalone WebAssembly Finger Controller for smoothing, deadzone and pinching (#33, #67, #108)
#[wasm_bindgen]
pub struct RustFingerController {
    inner: camera::fingers::FingerController,
}

#[wasm_bindgen]
impl RustFingerController {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: camera::fingers::FingerController::default(),
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn update(
        &mut self,
        landmarks: Option<Vec<f64>>,
        gesture: &str,
        score: f64,
        now: f64,
        aspect: f64,
    ) -> js_sys::Array {
        let landmarks_vec = landmarks.and_then(|coords| {
            if coords.len() >= 63 {
                let mut v = Vec::with_capacity(21);
                for i in 0..21 {
                    v.push(camera::fingers::Landmark {
                        x: coords[i * 3],
                        y: coords[i * 3 + 1],
                        z: coords[i * 3 + 2],
                    });
                }
                Some(v)
            } else {
                None
            }
        });

        let u = self.inner.update(
            landmarks_vec.as_deref(),
            gesture,
            score,
            now,
            aspect,
        );

        let arr = js_sys::Array::new();
        let mode_str = match u.mode {
            camera::fingers::FingerMode::None => "none",
            camera::fingers::FingerMode::Point => "point",
            camera::fingers::FingerMode::Pinch => "pinch",
            camera::fingers::FingerMode::Release => "release",
        };
        arr.push(&JsValue::from_str(mode_str));
        if let Some((ax, ay)) = u.aim {
            let aim_arr = js_sys::Array::new();
            aim_arr.push(&JsValue::from_f64(ax));
            aim_arr.push(&JsValue::from_f64(ay));
            arr.push(&aim_arr);
        } else {
            arr.push(&JsValue::NULL);
        }
        arr.push(&JsValue::from_bool(u.cast));
        arr.push(&JsValue::from_bool(u.cancelled));
        arr.push(&JsValue::from_bool(u.suppress));
        arr.push(&JsValue::from_f64(u.progress));
        arr
    }
}

/// Standalone WebAssembly Face Emotion Tracker (#33, #108)
#[wasm_bindgen]
pub struct RustEmotionTracker {
    inner: camera::emotion::EmotionTracker,
}

#[wasm_bindgen]
impl RustEmotionTracker {
    #[wasm_bindgen(constructor)]
    pub fn new(calibrate_seconds: f64) -> Self {
        Self {
            inner: camera::emotion::EmotionTracker::new(calibrate_seconds),
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn update_direct(
        &mut self,
        has_face: bool,
        mouth_smile_left: f64,
        mouth_smile_right: f64,
        brow_down_left: f64,
        brow_down_right: f64,
        mouth_press_left: f64,
        mouth_press_right: f64,
        jaw_open: f64,
        brow_inner_up: f64,
        eye_wide_left: f64,
        eye_wide_right: f64,
        mouth_frown_left: f64,
        mouth_frown_right: f64,
        dt: f64,
    ) -> js_sys::Array {
        let mut map = std::collections::HashMap::new();
        if has_face {
            map.insert("mouthSmileLeft".to_string(), mouth_smile_left);
            map.insert("mouthSmileRight".to_string(), mouth_smile_right);
            map.insert("browDownLeft".to_string(), brow_down_left);
            map.insert("browDownRight".to_string(), brow_down_right);
            map.insert("mouthPressLeft".to_string(), mouth_press_left);
            map.insert("mouthPressRight".to_string(), mouth_press_right);
            map.insert("jawOpen".to_string(), jaw_open);
            map.insert("browInnerUp".to_string(), brow_inner_up);
            map.insert("eyeWideLeft".to_string(), eye_wide_left);
            map.insert("eyeWideRight".to_string(), eye_wide_right);
            map.insert("mouthFrownLeft".to_string(), mouth_frown_left);
            map.insert("mouthFrownRight".to_string(), mouth_frown_right);
        }

        let (mood, ev) = self.inner.update(if has_face { Some(&map) } else { None }, dt);

        let arr = js_sys::Array::new();
        arr.push(&JsValue::from_str(mood.dominant.as_str()));
        arr.push(&JsValue::from_f64(mood.joy));
        arr.push(&JsValue::from_f64(mood.anger));
        arr.push(&JsValue::from_f64(mood.surprise));
        arr.push(&JsValue::from_f64(mood.sadness));
        arr.push(&JsValue::from_f64(mood.neutral));
        arr.push(&JsValue::from_f64(mood.intensity));
        arr.push(&JsValue::from_bool(mood.face));
        let ev_str = match ev {
            Some(camera::emotion::EmotionEvent::Change) => "change",
            Some(camera::emotion::EmotionEvent::Strong) => "strong",
            None => "",
        };
        arr.push(&JsValue::from_str(ev_str));
        arr
    }
}

/// Standalone WebAssembly Galaxy & Multi-World Simulator (#24, #108)
#[wasm_bindgen]
pub struct RustGalaxy {
    inner: moc_cosmos::galaxy::Galaxy,
}

#[wasm_bindgen]
impl RustGalaxy {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let mut inner = moc_cosmos::galaxy::Galaxy::new(seed);
        // The browser's galaxy (src/cosmos/galaxy.js) has more planets than this one and syncs
        // every one of them into the war itself (#166).
        inner.war_auto = false;
        Self { inner }
    }

    pub fn step(
        &mut self,
        dt: f32,
        calm: f32,
        focus: f32,
        stress: f32,
        blinks_last_10s: u32,
        blinks_total: u32,
    ) {
        let fx = moc_cosmos::planet_sim::BrainEffects::from_signals(calm, focus, stress, blinks_last_10s);
        self.inner.step(dt, &fx, blinks_total);
    }

    pub fn on_blink(&mut self) {
        self.inner.on_blink();
    }

    pub fn set_attended(&mut self, planet_idx: i32) {
        self.inner.attended = if planet_idx >= 0 {
            Some(planet_idx as usize)
        } else {
            None
        };
    }

    pub fn get_attended(&self) -> i32 {
        self.inner.attended.map(|idx| idx as i32).unwrap_or(-1)
    }

    pub fn planet_count(&self) -> usize {
        self.inner.planets.len()
    }

    pub fn planet_pop(&self, idx: usize) -> f32 {
        self.inner.planets.get(idx).map(|p| p.state.pop).unwrap_or(0.0)
    }

    pub fn planet_era(&self, idx: usize) -> usize {
        self.inner.planets.get(idx).map(|p| p.state.era_index).unwrap_or(0)
    }

    pub fn planet_progress(&self, idx: usize) -> f32 {
        self.inner.planets.get(idx).map(|p| p.state.progress).unwrap_or(0.0)
    }

    pub fn planet_market(&self, idx: usize) -> f32 {
        self.inner.planets.get(idx).map(|p| p.state.market).unwrap_or(100.0)
    }

    pub fn planet_is_alive(&self, idx: usize) -> bool {
        self.inner.planets.get(idx).map(|p| p.alive).unwrap_or(false)
    }

    pub fn planet_is_forming(&self, idx: usize) -> bool {
        self.inner.planets.get(idx).map(|p| p.forming).unwrap_or(false)
    }

    pub fn planet_name(&self, idx: usize) -> String {
        self.inner.planets.get(idx).map(|p| p.name.clone()).unwrap_or_default()
    }

    pub fn planet_biome(&self, idx: usize) -> String {
        self.inner.planets.get(idx).map(|p| p.biome.name().to_string()).unwrap_or_else(|| "forming".to_string())
    }

    pub fn planet_channel(&self, idx: usize) -> String {
        self.inner.planets.get(idx).map(|p| p.state.channel.name().to_string()).unwrap_or_else(|| "mixed".to_string())
    }

    pub fn planet_pos_x(&self, idx: usize) -> f32 {
        self.inner.planets.get(idx).map(|p| p.position(self.inner.time).0).unwrap_or(0.0)
    }

    pub fn planet_pos_y(&self, idx: usize) -> f32 {
        self.inner.planets.get(idx).map(|p| p.position(self.inner.time).1).unwrap_or(0.0)
    }

    pub fn is_mind_done(&self) -> bool {
        self.inner.mind.done
    }

    // ---- the interplanetary war (#166): moc_cosmos::war, strides in moc_cosmos::host ----

    /// Every planet of the browser's galaxy, `war_body_stride()` floats each.
    pub fn war_sync(&mut self, data: &[f32]) {
        self.inner.war.sync_packed(data);
    }

    pub fn war_name(&mut self, idx: usize, name: &str) {
        self.inner.war.sync_name(idx, name);
    }

    /// The campaign's home (−1: none, as in the sandboxes).
    pub fn war_set_player_home(&mut self, idx: i32) {
        self.inner.war.set_player_home(if idx >= 0 { Some(idx as usize) } else { None });
    }

    /// A planet several civilisations share (the civ sandbox's).
    pub fn war_set_shared(&mut self, idx: usize, shared: bool) {
        self.inner.war.set_shared(idx, shared);
    }

    /// Difficulty seam: how hard the AI powers press the player, ×how often they launch at the
    /// player's world and ×how hard those fleets hit (1 default; Easy about 0.5, Hard about 1.6;
    /// clamped to 0.1..=3).
    pub fn war_set_player_pressure(&mut self, x: f32) {
        self.inner.war.set_player_pressure(x);
    }

    pub fn war_player_pressure(&self) -> f32 {
        self.inner.war.player_pressure
    }

    pub fn war_step(&mut self, dt: f32, calm: f32, focus: f32, stress: f32) {
        self.inner.war.step(dt, moc_cosmos::war::Mind { calm, focus, stress });
    }

    /// The player orders an attack (0 raid, 1 bombard, 2 invade): "" if it is away, else why not.
    pub fn war_order(&mut self, target: usize, order: u8) -> String {
        let Some(p) = self.inner.war.player() else { return "You have no world in space yet.".to_string() };
        match self.inner.war.order_index(p, target, order) {
            Ok(_) => String::new(),
            Err(e) => e.to_string(),
        }
    }

    /// The player's power (−1 none).
    pub fn war_player(&self) -> i32 {
        self.inner.war.player().map(|p| p as i32).unwrap_or(-1)
    }

    pub fn war_body_stride() -> usize {
        moc_cosmos::host::BODY_IN
    }

    pub fn war_fleets(&self) -> Vec<f32> {
        let mut v = Vec::new();
        self.inner.war.pack_fleets(&mut v);
        v
    }

    pub fn war_battles(&self) -> Vec<f32> {
        let mut v = Vec::new();
        self.inner.war.pack_battles(&mut v);
        v
    }

    pub fn war_bodies(&self) -> Vec<f32> {
        let mut v = Vec::new();
        self.inner.war.pack_bodies(&mut v);
        v
    }

    pub fn war_powers(&self) -> Vec<f32> {
        let mut v = Vec::new();
        self.inner.war.pack_powers(&mut v);
        v
    }

    pub fn war_power_name(&self, p: usize) -> String {
        self.inner.war.powers.get(p).map(|x| x.name.clone()).unwrap_or_default()
    }

    /// What the war did to every planet since the last call (`moc_cosmos::host::FX_OUT` each).
    /// The planets this galaxy also simulates take it here too, so their next sync agrees.
    pub fn war_take_fx(&mut self) -> Vec<f32> {
        let mut v = Vec::new();
        self.inner.war.take_fx_packed(&mut v);
        for (i, c) in v.chunks_exact(moc_cosmos::host::FX_OUT).enumerate() {
            let fx = moc_cosmos::war::BodyFx { pop_mul: c[0], progress_loss: c[1], stock_mul: c[2], captured_by: c[3] as i32, settled: c[4] > 0.5 };
            self.inner.apply_war_fx(i, &fx);
        }
        v
    }

    /// News from line `from` on: [seq, tone, about the player, about a civ sandbox
    /// civilisation, text, …].
    pub fn war_news(&self, from: u32) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for n in self.inner.war.news_since(from) {
            arr.push(&JsValue::from_f64(n.seq as f64));
            arr.push(&JsValue::from_f64(n.tone as f64));
            arr.push(&JsValue::from_bool(n.player));
            arr.push(&JsValue::from_bool(n.civ));
            arr.push(&JsValue::from_str(&n.text));
        }
        arr
    }

    pub fn war_planet_meta(&self, idx: usize) -> String {
        self.inner.war.planet_meta(idx)
    }

    pub fn war_target_line(&self, idx: usize) -> String {
        match self.inner.war.player() {
            Some(p) => self.inner.war.target_line(p, idx),
            None => String::new(),
        }
    }

    /// HUD rows for the player's power (or the galaxy's): "label\tvalue\tbad" lines.
    pub fn war_hud(&self) -> String {
        self.inner.war.hud_rows()
    }

    // ---- civ sandbox civilisations in space (#166 stage 4; moc-civ's space.rs) ----

    /// Register or update a civilisation in space living on planet `home` (by the host's key).
    /// Returns its power id (−1 if the galaxy has no room, or it was retired).
    #[allow(clippy::too_many_arguments)]
    pub fn war_upsert_civ(&mut self, key: u32, name: &str, r: u8, g: u8, b: u8, home: usize, production: f32, knowledge: f32, resources: f32, fleet: f32, shields: f32, defences: f32, garrison: f32, tier: u8, temper: f32, alive: bool) -> i32 {
        let econ = moc_cosmos::war::Economy { production, knowledge, resources };
        let defence = moc_cosmos::war::Defence { shields, defences, garrison };
        match self.inner.war.upsert_external(key, name, [r, g, b], home, econ, fleet, defence, tier, temper, alive) {
            Some(p) if alive => p as i32,
            _ => -1,
        }
    }

    /// The planets power `power` may strike (`moc_cosmos::host::TARGET_OUT` each).
    pub fn war_targets_for(&self, power: u16) -> Vec<f32> {
        let mut v = Vec::new();
        self.inner.war.targets_for(power, &mut v);
        v
    }

    /// A civilisation's fleet, already taken from its dock, flies at planet `target`. Returns
    /// the fleet's id (−1: refused).
    pub fn war_launch_external(&mut self, power: u16, target: usize, order: u8, strength: f32, motive: u8) -> i32 {
        let Some(o) = moc_cosmos::war::Order::from_u8(order) else { return -1 };
        let m = moc_cosmos::war::Motive::from_u8(motive);
        self.inner.war.launch_external(power, target, o, strength, m).map(|id| id as i32).unwrap_or(-1)
    }

    /// What happened to a civilisation's power since the last call (`moc_cosmos::host::RESULT_OUT`).
    pub fn war_take_results(&mut self, power: u16) -> Vec<f32> {
        self.inner.war.take_results_packed(power).to_vec()
    }
}

/// Standalone WebAssembly MLP Policy AI God (#25, #108)
#[wasm_bindgen]
pub struct RustAiPolicy {
    inner: moc_ai::policy::MLPPolicy,
}

#[wasm_bindgen]
impl RustAiPolicy {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: moc_ai::policy::MLPPolicy::from_embedded(),
        }
    }

    pub fn act(&self, obs: &[f32], greedy: bool, sample_val: f32) -> u32 {
        if obs.len() < moc_ai::policy::NUM_INPUTS {
            return 0;
        }
        self.inner.act(obs, greedy, sample_val) as u32
    }

    pub fn probs(&self, obs: &[f32]) -> js_sys::Float32Array {
        if obs.len() < moc_ai::policy::NUM_INPUTS {
            return js_sys::Float32Array::new_with_length(moc_ai::policy::NUM_ACTIONS as u32);
        }
        let probs = self.inner.probs(obs);
        js_sys::Float32Array::from(&probs[..])
    }

    pub fn self_check(&self) -> f32 {
        self.inner.self_check()
    }
}

/// Standalone WebAssembly Brain signal processor and personal model (#20, #108)
#[wasm_bindgen]
pub struct RustBrain {
    inner: moc_brain::brain::Brain,
}

#[wasm_bindgen]
impl RustBrain {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: moc_brain::brain::Brain::new(),
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.inner.update(dt)
    }

    pub fn set_cue(&mut self, cue_id: i32) {
        let cue = match cue_id {
            0 => Some(moc_brain::model::Class::Rest),
            1 => Some(moc_brain::model::Class::Calm),
            2 => Some(moc_brain::model::Class::Focus),
            _ => None,
        };
        self.inner.set_cue(cue);
    }

    pub fn ingest(
        &mut self,
        delta: f32,
        theta: f32,
        alpha: f32,
        beta: f32,
        gamma: f32,
        blink: bool,
        hr: f32,
        tilt_x: f32,
        tilt_y: f32,
        motion: f32,
        quality: f32,
    ) -> bool {
        let frame = moc_brain::sim_source::BrainFrame {
            bands: moc_brain::model::Bands {
                delta,
                theta,
                alpha,
                beta,
                gamma,
            },
            blink,
            hr: if hr > 0.0 { Some(hr) } else { None },
            tilt: Some((tilt_x, tilt_y)),
            motion,
            quality,
        };
        self.inner.ingest(&frame)
    }

    pub fn calm(&self) -> f32 {
        self.inner.calm
    }

    pub fn focus(&self) -> f32 {
        self.inner.focus
    }

    pub fn stress(&self) -> f32 {
        self.inner.stress
    }

    pub fn anger(&self) -> f32 {
        self.inner.anger
    }

    pub fn blinks(&self) -> u32 {
        self.inner.stats.blinks
    }

    pub fn retrain(&mut self) -> bool {
        self.inner.model.maybe_train()
    }

    pub fn accuracy(&self) -> f32 {
        self.inner.model.accuracy
    }
}

/// Standalone WebAssembly Simulated Headset Source (#20, #108)
#[wasm_bindgen]
pub struct RustSimSource {
    inner: moc_brain::sim_source::SimSource,
}

#[wasm_bindgen]
impl RustSimSource {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        Self {
            inner: moc_brain::sim_source::SimSource::new(seed),
        }
    }

    pub fn set_input(&mut self, calm: bool, focus: bool, panic: bool) {
        self.inner.input.calm = calm;
        self.inner.input.focus = focus;
        self.inner.input.panic = panic;
    }

    pub fn blink(&mut self) {
        self.inner.blink();
    }

    pub fn step(&mut self, dt: f32) -> js_sys::Float32Array {
        let mut out = [0.0f32; 11];
        self.inner.update(dt, |f| {
            out = [
                f.bands.delta,
                f.bands.theta,
                f.bands.alpha,
                f.bands.beta,
                f.bands.gamma,
                if f.blink { 1.0 } else { 0.0 },
                f.hr.unwrap_or(0.0),
                f.tilt.map(|t| t.0).unwrap_or(0.0),
                f.tilt.map(|t| t.1).unwrap_or(0.0),
                f.motion,
                f.quality,
            ];
        });
        js_sys::Float32Array::from(&out[..])
    }
}

/// Standalone WebAssembly Calibration traits (#54, #108)
#[wasm_bindgen]
pub struct RustCalibration;

#[wasm_bindgen]
impl RustCalibration {
    pub fn random_traits(seed: u32) -> js_sys::Float32Array {
        let t = moc_brain::calibration::random_seed_traits(seed);
        let out = [t.friendliness, t.aggressiveness, t.violence];
        js_sys::Float32Array::from(&out[..])
    }

    pub fn traits_from_sensors(
        calm: f32,
        focus: f32,
        stress: f32,
        blink_rate: f32,
        joy: f32,
        anger: f32,
    ) -> js_sys::Float32Array {
        let features = moc_brain::calibration::SensorFeatures {
            calm,
            focus,
            stress,
            blink_rate,
            joy,
            anger,
            surprise: 0.0,
            sadness: 0.0,
            face_seen: joy > 0.0 || anger > 0.0,
        };
        let t = moc_brain::calibration::traits_from_features(&features);
        let out = [t.friendliness, t.aggressiveness, t.violence];
        js_sys::Float32Array::from(&out[..])
    }
}

#[cfg(test)]
mod walk_field_tests {
    use super::*;

    #[test]
    fn walk_field_wraps_both_ways_on_the_torus() {
        // A 10×6 sea with one walkable row; the BFS from x=1 reaches x=9 the short way, across
        // the east-west seam (2 steps, not 8), and a target off the map wraps onto it (#111).
        let (w, h) = (10usize, 6usize);
        let mut types = vec![terrain::T_DEEP; w * h];
        for x in 0..w {
            types[2 * w + x] = terrain::T_GRASS;
        }
        let f = walk_field(&types, w as u32, h as u32, 1, 2, false);
        assert_eq!(f[2 * w + 9], 2);
        assert_eq!(f[2 * w + 5], 4);
        let g = walk_field(&types, w as u32, h as u32, 11, 2 + h as i32, false);
        assert_eq!(g, f);
        // Over water the seam wraps north-south too.
        let s = walk_field(&types, w as u32, h as u32, 0, 0, true);
        assert_eq!(s[5 * w], 1);
    }
}
