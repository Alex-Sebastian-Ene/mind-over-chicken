use crate::math3d::{Mat4, Vec3};

/// Default view: a steep (~60 degree) look down a narrow 30 degree lens from far back, so the
/// island reads like a tilted diorama of the 2D map instead of a low wide-angle shot.
pub const DEFAULT_PITCH: f32 = 1.05;
pub const DEFAULT_DISTANCE: f32 = 300.0;
/// Close enough to pick out single villagers (Alex: "zoom in further like I was before").
pub const MIN_DISTANCE: f32 = 12.0;
pub const MAX_DISTANCE: f32 = 600.0;

#[derive(Clone, Debug)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,      // Radians around Y axis
    pub pitch: f32,    // Radians above XZ plane
    pub distance: f32, // Distance from target
    pub fov: f32,      // Vertical field of view in radians
    pub near: f32,
    pub far: f32,
}

impl OrbitCamera {
    pub fn new(target_x: f32, target_z: f32) -> Self {
        Self {
            target: Vec3::new(target_x, 0.0, target_z),
            yaw: 0.0,
            pitch: DEFAULT_PITCH,
            distance: DEFAULT_DISTANCE,
            fov: std::f32::consts::FRAC_PI_6, // 30 degrees: little perspective distortion
            near: 1.0,
            far: 2000.0,
        }
    }

    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        // Clamp pitch so camera cannot flip upside down
        let min_pitch = 0.15; // ~8 degrees
        let max_pitch = 1.55; // ~89 degrees: straight down, for the hand-over to the globe
        self.pitch = (self.pitch + delta_pitch).clamp(min_pitch, max_pitch);
    }

    pub fn zoom(&mut self, factor: f32) {
        // Zoom in/out clamped between close ground view and high orbit
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn set_distance(&mut self, d: f32) {
        self.distance = d.clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn set_pitch(&mut self, p: f32) {
        let min_pitch = 0.15;
        let max_pitch = 1.55;
        self.pitch = p.clamp(min_pitch, max_pitch);
    }

    pub fn set_yaw(&mut self, y: f32) {
        self.yaw = y;
    }

    pub fn set_pose(&mut self, yaw: f32, pitch: f32, distance: f32) {
        self.set_yaw(yaw);
        self.set_pitch(pitch);
        self.set_distance(distance);
    }

    pub fn pan(&mut self, dx: f32, dz: f32) {
        let cos_y = self.yaw.cos();
        let sin_y = self.yaw.sin();
        // Camera-relative forward and right on the XZ plane
        let right = Vec3::new(cos_y, 0.0, -sin_y);
        let forward = Vec3::new(sin_y, 0.0, cos_y);

        self.target = self.target.add(right.scale(dx)).add(forward.scale(dz));
    }

    pub fn eye_position(&self) -> Vec3 {
        let cos_p = self.pitch.cos();
        let sin_p = self.pitch.sin();
        let cos_y = self.yaw.cos();
        let sin_y = self.yaw.sin();

        Vec3::new(
            self.target.x + sin_y * cos_p * self.distance,
            self.target.y + sin_p * self.distance,
            self.target.z + cos_y * cos_p * self.distance,
        )
    }

    pub fn compute_matrices(&self, aspect: f32) -> (Mat4, Mat4, Mat4) {
        let eye = self.eye_position();
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at(eye, self.target, up);
        let proj = Mat4::perspective(self.fov, aspect, self.near, self.far);
        let view_proj = proj.mul(&view);
        (view, proj, view_proj)
    }

    // Casts a ray from screen coordinates (0..w, 0..h) to plane Y = ground_y
    /// Where a screen point meets the flat plane y = `ground_y` (the engine now raycasts onto
    /// the terrain with `screen_to_terrain`; this stays as the flat reference).
    #[allow(dead_code)]
    pub fn screen_to_ground(
        &self,
        screen_x: f32,
        screen_y: f32,
        screen_w: f32,
        screen_h: f32,
        ground_y: f32,
    ) -> Option<[f32; 2]> {
        let (o, d) = self.screen_ray(screen_x, screen_y, screen_w, screen_h)?;
        if d.y.abs() < 1e-6 {
            return None; // Parallel to ground plane
        }
        let t = (ground_y - o.y) / d.y;
        if t < 0.0 {
            return None; // Behind camera
        }
        Some([o.x + d.x * t, o.z + d.z * t])
    }

    /// Project a 3D world point [x, y, z] to screen coordinates (0..screen_w, 0..screen_h).
    /// Used for placing 2D UI overlays, speech bubbles, and reticles exactly over their 3D positions.
    pub fn world_to_screen(
        &self,
        world_x: f32,
        world_y: f32,
        world_z: f32,
        screen_w: f32,
        screen_h: f32,
    ) -> Option<[f32; 2]> {
        if screen_w <= 0.0 || screen_h <= 0.0 {
            return None;
        }
        let (_view, _proj, view_proj) = self.compute_matrices(screen_w / screen_h);
        let clip = view_proj.transform_vec4([world_x, world_y, world_z, 1.0]);
        if clip[3] <= 0.01 {
            return None; // Behind camera
        }
        let ndc_x = clip[0] / clip[3];
        let ndc_y = clip[1] / clip[3];
        let sx = (ndc_x * 0.5 + 0.5) * screen_w;
        let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * screen_h;
        Some([sx, sy])
    }

    /// The world ray under a screen point: (origin on the near plane, unit direction).
    pub fn screen_ray(&self, screen_x: f32, screen_y: f32, screen_w: f32, screen_h: f32) -> Option<(Vec3, Vec3)> {
        if screen_w <= 0.0 || screen_h <= 0.0 {
            return None;
        }
        let ndc_x = (screen_x / screen_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / screen_h) * 2.0;
        let (_view, _proj, view_proj) = self.compute_matrices(screen_w / screen_h);
        let inv_vp = view_proj.inverse()?;
        let n = inv_vp.transform_vec4([ndc_x, ndc_y, -1.0, 1.0]);
        let f = inv_vp.transform_vec4([ndc_x, ndc_y, 1.0, 1.0]);
        if n[3].abs() < 1e-6 || f[3].abs() < 1e-6 {
            return None;
        }
        let near_w = Vec3::new(n[0] / n[3], n[1] / n[3], n[2] / n[3]);
        let far_w = Vec3::new(f[0] / f[3], f[1] / f[3], f[2] / f[3]);
        Some((near_w, far_w.sub(near_w).normalize()))
    }

    /// Where a screen point lands on a heightfield, so a click on a hillside strikes the
    /// hillside rather than the sea-level plane behind it. `ground_at(x, z)` is the surface
    /// height in world units; `max_y` bounds it. Marches the ray, then bisects the crossing.
    pub fn screen_to_terrain(
        &self,
        screen_x: f32,
        screen_y: f32,
        screen_w: f32,
        screen_h: f32,
        max_y: f32,
        ground_at: impl Fn(f32, f32) -> f32,
    ) -> Option<[f32; 2]> {
        let (o, d) = self.screen_ray(screen_x, screen_y, screen_w, screen_h)?;
        if d.y > -1e-4 {
            return None; // Looking at or above the horizon
        }
        let above = |t: f32| {
            let p = o.add(d.scale(t));
            p.y - ground_at(p.x, p.z)
        };
        // Start where the ray drops below the highest possible ground; end at y = 0.
        let t_start = ((max_y - o.y) / d.y).max(0.0);
        let t_end = (0.0 - o.y) / d.y;
        if t_end <= t_start {
            return None;
        }
        let step = 0.5;
        let mut t_prev = t_start;
        let mut t = t_start;
        while t <= t_end {
            if above(t) <= 0.0 {
                let (mut lo, mut hi) = (t_prev, t);
                for _ in 0..12 {
                    let mid = 0.5 * (lo + hi);
                    if above(mid) > 0.0 { lo = mid } else { hi = mid }
                }
                let p = o.add(d.scale(hi));
                return Some([p.x, p.z]);
            }
            t_prev = t;
            t += step;
        }
        let p = o.add(d.scale(t_end));
        Some([p.x, p.z])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orbit_camera_matrices_and_raycast() {
        let mut cam = OrbitCamera::new(160.0, 90.0);
        assert_eq!(cam.target.x, 160.0);
        assert_eq!(cam.target.z, 90.0);

        // Raycast center of screen should hit near target
        let hit = cam.screen_to_ground(400.0, 300.0, 800.0, 600.0, 0.0);
        assert!(hit.is_some());
        let [hx, hz] = hit.unwrap();
        assert!((hx - cam.target.x).abs() < 5.0, "hx was {}", hx);
        assert!((hz - cam.target.z).abs() < 5.0, "hz was {}", hz);

        // Orbit test
        cam.orbit(0.2, 0.1);
        assert!(cam.pitch > DEFAULT_PITCH);
    }

    #[test]
    fn terrain_raycast_matches_plane_on_flat_ground() {
        let cam = OrbitCamera::new(160.0, 90.0);
        let plane = cam.screen_to_ground(300.0, 350.0, 800.0, 600.0, 8.0).unwrap();
        let terr = cam.screen_to_terrain(300.0, 350.0, 800.0, 600.0, 24.0, |_, _| 8.0).unwrap();
        assert!((plane[0] - terr[0]).abs() < 0.05 && (plane[1] - terr[1]).abs() < 0.05, "{:?} vs {:?}", plane, terr);
    }

    #[test]
    fn terrain_raycast_stops_on_a_hill_in_front() {
        // A 20-unit plateau for z > 80: the ray from the screen centre must stop on it,
        // nearer the camera than the sea-level plane hit, and on its surface.
        let cam = OrbitCamera::new(160.0, 90.0);
        let ground = |_: f32, z: f32| if z > 80.0 { 20.0 } else { 8.0 };
        let plane = cam.screen_to_ground(400.0, 300.0, 800.0, 600.0, 8.0).unwrap();
        let hit = cam.screen_to_terrain(400.0, 300.0, 800.0, 600.0, 24.0, ground).unwrap();
        assert!(hit[1] > 80.0, "hit {:?} should be on the plateau", hit);
        // The camera sits south (+z) looking north, so the ray meets the plateau south of
        // where it would meet the sea-level plane.
        assert!(hit[1] > plane[1], "plateau should be hit before the plane ({:?} vs {:?})", hit, plane);
    }

    #[test]
    fn test_camera_set_pose_and_clamping() {
        let mut cam = OrbitCamera::new(160.0, 90.0);
        cam.set_pose(1.23, 1.0, 150.0);
        assert!((cam.yaw - 1.23).abs() < 1e-4);
        assert!((cam.pitch - 1.0).abs() < 1e-4);
        assert!((cam.distance - 150.0).abs() < 1e-4);

        // Clamp checks
        cam.set_distance(9999.0);
        assert_eq!(cam.distance, MAX_DISTANCE);
        cam.set_distance(-10.0);
        assert_eq!(cam.distance, MIN_DISTANCE);

        cam.set_pitch(3.0);
        assert_eq!(cam.pitch, 1.55);
        cam.set_pitch(-1.0);
        assert_eq!(cam.pitch, 0.15);
    }

    #[test]
    fn test_world_to_screen_roundtrip() {
        let cam = OrbitCamera::new(160.0, 90.0);
        let screen = cam.world_to_screen(160.0, 0.0, 90.0, 800.0, 600.0).unwrap();
        // Camera target should project to the screen center (400, 300)
        assert!((screen[0] - 400.0).abs() < 1.0, "screen_x was {}", screen[0]);
        assert!((screen[1] - 300.0).abs() < 1.0, "screen_y was {}", screen[1]);
    }
}
