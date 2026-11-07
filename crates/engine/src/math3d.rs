// 3D vector and 4x4 matrix math for WebGL rendering and raycasting

#[derive(Clone, Copy, Debug)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    pub fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let l = self.len();
        if l > 1e-6 {
            Self::new(self.x / l, self.y / l, self.z / l)
        } else {
            Self::new(0.0, 1.0, 0.0)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
    pub m: [f32; 16], // Column-major for WebGL
}

impl Mat4 {
    #[allow(dead_code)]
    pub fn identity() -> Self {
        #[rustfmt::skip]
        let m = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        Self { m }
    }

    pub fn mul(&self, b: &Mat4) -> Mat4 {
        let mut r = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                r[col * 4 + row] = self.m[row] * b.m[col * 4]
                    + self.m[4 + row] * b.m[col * 4 + 1]
                    + self.m[8 + row] * b.m[col * 4 + 2]
                    + self.m[12 + row] * b.m[col * 4 + 3];
            }
        }
        Mat4 { m: r }
    }

    pub fn perspective(fov_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_rad * 0.5).tan();
        let nf = 1.0 / (near - far);
        #[rustfmt::skip]
        let m = [
            f / aspect, 0.0, 0.0, 0.0,
            0.0, f, 0.0, 0.0,
            0.0, 0.0, (far + near) * nf, -1.0,
            0.0, 0.0, (2.0 * far * near) * nf, 0.0,
        ];
        Mat4 { m }
    }

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = target.sub(eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);

        #[rustfmt::skip]
        let m = [
            s.x, u.x, -f.x, 0.0,
            s.y, u.y, -f.y, 0.0,
            s.z, u.z, -f.z, 0.0,
            -s.dot(eye), -u.dot(eye), f.dot(eye), 1.0,
        ];
        Mat4 { m }
    }

    pub fn inverse(&self) -> Option<Mat4> {
        let m = &self.m;
        let mut inv = [0.0f32; 16];

        inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
            + m[9] * m[7] * m[14] + m[13] * m[6] * m[11] - m[13] * m[7] * m[10];
        inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
            - m[8] * m[7] * m[14] - m[12] * m[6] * m[11] + m[12] * m[7] * m[10];
        inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
            + m[8] * m[7] * m[13] + m[12] * m[5] * m[11] - m[12] * m[7] * m[9];
        inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
            - m[8] * m[6] * m[13] - m[12] * m[5] * m[10] + m[12] * m[6] * m[9];

        inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
            - m[9] * m[3] * m[14] - m[13] * m[2] * m[11] + m[13] * m[3] * m[10];
        inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
            + m[8] * m[3] * m[14] + m[12] * m[2] * m[11] - m[12] * m[3] * m[10];
        inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
            - m[8] * m[3] * m[13] - m[12] * m[1] * m[11] + m[12] * m[3] * m[9];
        inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
            + m[8] * m[2] * m[13] + m[12] * m[1] * m[10] - m[12] * m[2] * m[9];

        inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
            + m[5] * m[3] * m[14] + m[13] * m[2] * m[7] - m[13] * m[3] * m[6];
        inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
            - m[4] * m[3] * m[14] - m[12] * m[2] * m[7] + m[12] * m[3] * m[6];
        inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
            + m[4] * m[3] * m[13] + m[12] * m[1] * m[7] - m[12] * m[3] * m[5];
        inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
            - m[4] * m[2] * m[13] - m[12] * m[1] * m[6] + m[12] * m[2] * m[5];

        inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
            - m[5] * m[3] * m[10] - m[9] * m[2] * m[7] + m[9] * m[3] * m[6];
        inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
            + m[4] * m[3] * m[10] + m[8] * m[2] * m[7] - m[8] * m[3] * m[6];
        inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
            - m[4] * m[3] * m[9] - m[8] * m[1] * m[7] + m[8] * m[3] * m[5];
        inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
            + m[4] * m[2] * m[9] + m[8] * m[1] * m[6] - m[8] * m[2] * m[5];

        let det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
        if det.abs() < 1e-8 {
            return None;
        }

        let inv_det = 1.0 / det;
        for val in inv.iter_mut() {
            *val *= inv_det;
        }
        Some(Mat4 { m: inv })
    }

    pub fn transform_vec4(&self, v: [f32; 4]) -> [f32; 4] {
        [
            self.m[0] * v[0] + self.m[4] * v[1] + self.m[8] * v[2] + self.m[12] * v[3],
            self.m[1] * v[0] + self.m[5] * v[1] + self.m[9] * v[2] + self.m[13] * v[3],
            self.m[2] * v[0] + self.m[6] * v[1] + self.m[10] * v[2] + self.m[14] * v[3],
            self.m[3] * v[0] + self.m[7] * v[1] + self.m[11] * v[2] + self.m[15] * v[3],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_and_mul() {
        let i = Mat4::identity();
        let p = Mat4::perspective(std::f32::consts::FRAC_PI_4, 1.777, 0.1, 100.0);
        let res = p.mul(&i);
        assert!((res.m[0] - p.m[0]).abs() < 1e-5);
    }

    #[test]
    fn test_invert() {
        let eye = Vec3::new(10.0, 20.0, 30.0);
        let target = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at(eye, target, up);
        let inv = view.inverse().expect("View matrix must be invertible");
        let ident = view.mul(&inv);
        for row in 0..4 {
            for col in 0..4 {
                let expected = if row == col { 1.0 } else { 0.0 };
                let diff = (ident.m[col * 4 + row] - expected).abs();
                assert!(diff < 1e-4, "Mismatch at row {} col {}: {}", row, col, diff);
            }
        }
    }
}
