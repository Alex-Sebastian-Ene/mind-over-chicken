use crate::math3d::{Mat4, Vec3};

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
            pitch: 0.65, // ~37 degrees tilt down
            distance: 120.0,
            fov: std::f32::consts::FRAC_PI_4, // 45 degrees
            near: 1.0,
            far: 2000.0,
        }
    }

    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        // Clamp pitch so camera cannot flip upside down
        let min_pitch = 0.15; // ~8 degrees
        let max_pitch = 1.45; // ~83 degrees
        self.pitch = (self.pitch + delta_pitch).clamp(min_pitch, max_pitch);
    }

    pub fn zoom(&mut self, factor: f32) {
        // Zoom in/out clamped between close ground view and high orbit
        self.distance = (self.distance * factor).clamp(25.0, 600.0);
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
    pub fn screen_to_ground(
        &self,
        screen_x: f32,
        screen_y: f32,
        screen_w: f32,
        screen_h: f32,
        ground_y: f32,
    ) -> Option<[f32; 2]> {
        if screen_w <= 0.0 || screen_h <= 0.0 {
            return None;
        }

        // Convert screen (top-left 0,0) to NDC (-1..1, bottom-to-top)
        let ndc_x = (screen_x / screen_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / screen_h) * 2.0;

        let aspect = screen_w / screen_h;
        let (_view, _proj, view_proj) = self.compute_matrices(aspect);
        let inv_vp = view_proj.inverse()?;

        // Unproject near plane (z = -1) and far plane (z = 1)
        let p_near = inv_vp.transform_vec4([ndc_x, ndc_y, -1.0, 1.0]);
        let p_far = inv_vp.transform_vec4([ndc_x, ndc_y, 1.0, 1.0]);

        if p_near[3].abs() < 1e-6 || p_far[3].abs() < 1e-6 {
            return None;
        }

        let near_w = Vec3::new(
            p_near[0] / p_near[3],
            p_near[1] / p_near[3],
            p_near[2] / p_near[3],
        );
        let far_w = Vec3::new(
            p_far[0] / p_far[3],
            p_far[1] / p_far[3],
            p_far[2] / p_far[3],
        );

        let ray_dir = far_w.sub(near_w).normalize();
        if ray_dir.y.abs() < 1e-6 {
            return None; // Parallel to ground plane
        }

        let t = (ground_y - near_w.y) / ray_dir.y;
        if t < 0.0 {
            return None; // Behind camera
        }

        let hit_x = near_w.x + ray_dir.x * t;
        let hit_z = near_w.z + ray_dir.z * t;
        Some([hit_x, hit_z])
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
        assert!(cam.pitch > 0.65);
    }
}
