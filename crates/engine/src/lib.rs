pub mod agents;
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

    /// Generate realistic procedural terrain using moc-terrain (#109).
    /// Updates the engine's internal heightmap, 3D relief mesh, and base pixel texture.
    pub fn generate_terrain(&mut self, seed: u32) -> Result<(), JsValue> {
        let map = moc_terrain::TerrainMap::generate_home_world(seed);
        if let Some(t) = &mut self.terrain {
            t.h.copy_from_slice(&map.heights);
            t.tile_type.copy_from_slice(&map.types);
            t.pixels.copy_from_slice(&map.rgba);
        }
        if let Some(r3d) = &mut self.renderer_3d {
            let mesh = TerrainMesh::build(&map.heights, W, H, W as f32, H as f32, 24.0, 0.33);
            r3d.upload_mesh(&mesh);
        }
        Ok(())
    }

    /// Standalone helper to generate a civ planet RGBA texture (#109).
    pub fn generate_civ_terrain(seed: u32, width: usize, height: usize) -> Vec<u8> {
        let map = moc_terrain::TerrainMap::generate_civ_planet(seed, width, height);
        map.rgba
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
        let map_w = W as f32;
        let map_h = H as f32;
        let wx = x.rem_euclid(map_w);
        let wz = z.rem_euclid(map_h);
        let h_norm = self.get_terrain_height_at(wx, wz);

        // Never let land entities (villagers, mammoths, chickens, goats, fires) stand or walk on water
        let is_land_entity = matches!(
            sprite_id,
            sprites::SPRITE_VILLAGER
                | sprites::SPRITE_MAMMOTH
                | sprites::SPRITE_CHICKEN
                | sprites::SPRITE_GOAT
                | sprites::SPRITE_FIRE
        );
        let (wx, wz, h_norm) = if is_land_entity && h_norm < 0.33 {
            // Find nearest land point on the coast
            let mut best_x = wx;
            let mut best_z = wz;
            let mut best_h = h_norm;
            'search_land: for r in 1..=8 {
                let rf = r as f32;
                for &(dx, dz) in &[
                    (rf, 0.0), (-rf, 0.0), (0.0, rf), (0.0, -rf),
                    (rf * 0.707, rf * 0.707), (-rf * 0.707, rf * 0.707),
                    (rf * 0.707, -rf * 0.707), (-rf * 0.707, -rf * 0.707),
                ] {
                    let tx = (wx + dx).rem_euclid(map_w);
                    let tz = (wz + dz).rem_euclid(map_h);
                    let th = self.get_terrain_height_at(tx, tz);
                    if th >= 0.33 {
                        best_x = tx;
                        best_z = tz;
                        best_h = th;
                        break 'search_land;
                    }
                }
            }
            (best_x, best_z, best_h)
        } else {
            (wx, wz, h_norm)
        };

        let ground_y = (h_norm * 24.0).max(7.92) + 0.15; // Slightly above ground so feet never clip into slopes
        let center = [wx, ground_y, wz];

        // Soft ground contact shadow beneath entities, trees, and structures
        let shadow_variant = match sprite_id {
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
        };

        if let Some(sv) = shadow_variant {
            let s_entry = sprites::get_sprite_atlas_entry(sprites::SPRITE_SHADOW, 1, 0, sv);
            let shw = s_entry.world_w * 0.5;
            let shd = s_entry.world_h * 0.5;
            let s_tl = BillboardVertex {
                center,
                offset: [-shw, 0.02, -shd],
                uv: [s_entry.u0, s_entry.v0],
            };
            let s_bl = BillboardVertex {
                center,
                offset: [-shw, 0.02, shd],
                uv: [s_entry.u0, s_entry.v1],
            };
            let s_tr = BillboardVertex {
                center,
                offset: [shw, 0.02, -shd],
                uv: [s_entry.u1, s_entry.v0],
            };
            let s_br = BillboardVertex {
                center,
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

/// Standalone WebAssembly World Generator providing zero-copy buffers to JS (#109, #19, #128)
#[wasm_bindgen]
pub struct RustWorldGen {
    terrain: crate::terrain::Terrain,
}

#[wasm_bindgen]
impl RustWorldGen {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        Self {
            terrain: crate::terrain::Terrain::new(seed),
        }
    }

    pub fn heights(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(&self.terrain.h[..])
    }

    pub fn types(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(&self.terrain.tile_type[..])
    }

    pub fn rgba(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(&self.terrain.pixels[..])
    }

    pub fn sites_json(&self) -> String {
        self.terrain.sites_json()
    }

    pub fn continents_json(&self) -> String {
        self.terrain.continents_json()
    }
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
